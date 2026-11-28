//! An independent check of the finished G-code. It reads the text of the program, line by line, the way a controller would, and knows
//! nothing about how the program was made: it keeps its own idea of the tool's position, the spindle and the feed, and reports what
//! would hurt (cutting with the spindle off, a feed move with no feed, a sideways move before the tool height is known, numbers that
//! are not numbers, a spindle left running). `trace` also measures the program (cutting length, deepest cut) so tests can compare it
//! with what the generator thinks it wrote.
//!
//! It reads absolute G-code with G0 to G3 (arcs by I and J), G20 / G21, G90 / G91, G53, G28, G43 H, G4 P, M3 / M4 / M5, M6 and T,
//! M2 / M30, S and F, with or without line numbers, comments and spaces between words (`G1X74.934Y70.699Z8.200`, `M06T11`), and
//! lines that continue the last motion without a G (`X76.242Y69.401`). Anything else on a line is passed over.
//!
//! What it knows of the post that wrote the text is a small description, [`ReadBack`], plain data that the post package fills in (its
//! comment style, whether I/J are incremental, line numbers, whether motion codes are written on every move, the dwell unit, the
//! decimals of the axes, which tool numbers are pens). It parses the text itself and shares no code with the generator or the post
//! engine (ARCHITECTURE.md), so a broken post cannot hide a wrong move behind the code that wrote it. [`read`] reports what it sees
//! as [`Fact`]s, line by line, for the motion cross-check (`crosscheck`) and the tool-changer checks ([`AtcCheck`]); one pass serves
//! all three.

use crate::i18n::{tr, tr_f};
/// One word of a G-code line: a letter and its number ("X-1.5").
fn words(line: &str) -> Vec<(char, f64)> {
    // comments: (...) and everything after ; (most lines have none, and are read as they are)
    let stripped;
    let clean: &str = if line.contains(['(', ';']) {
        let (mut depth, mut s) = (0u32, String::with_capacity(line.len()));
        for c in line.chars() {
            match c {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                ';' if depth == 0 => break,
                c if depth == 0 => s.push(c),
                _ => {}
            }
        }
        stripped = s;
        &stripped
    } else { line };
    let b = clean.as_bytes();
    let mut out = Vec::with_capacity(8);
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_alphabetic() {
            let mut j = i + 1;
            while j < b.len() && (b[j].is_ascii_digit() || matches!(b[j], b'+' | b'-' | b'.')) { j += 1; }
            if j > i + 1 { if let Ok(v) = clean[i + 1..j].parse::<f64>() { out.push((b[i].to_ascii_uppercase() as char, v)); } }
            i = j.max(i + 1);
        } else { i += 1; }
    }
    out
}

/// How the post that wrote the text writes it: what [`read`] needs to know beyond plain G-code. Plain data, filled in by the post
/// package (`Post::read_back`); the default reads what every built-in writes.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadBack {
    /// The post's comment style. `(…)` and everything after `;` are read as comments in every style (controllers read both, and the
    /// GRBL built-ins write `M0 ; message` beside paren comments); with `None` the post writes no comments, so a pen is only known by
    /// its tool number (`pens`).
    pub comments: Comments,
    /// I and J are the arc centre from the arc's start (true for every post today), else absolute.
    pub ij_incremental: bool,
    /// The lines start with a line number (`N10`), which is read past.
    pub numbered: bool,
    /// The post writes the motion code (G0 G1 G2 G3) on every move. A move line without one then did not come from a move template
    /// the usual way, which the cross-check reports; otherwise a line of axis words continues the last motion, as on any controller.
    pub motion_always: bool,
    /// The unit of `G4 P` (for the dwell total).
    pub dwell: Dwell,
    /// Decimals of the X, Y and Z words in the output units (`None` = not rounded): the cross-check's tolerance.
    pub decimals: [Option<u32>; 3],
    /// Decimals of I and J in the output units.
    pub ij_decimals: Option<u32>,
    /// Tool numbers that are pens: after `M6` with one of these, moves with the spindle off are drawing, not cutting. (The built-ins
    /// also say so with a `(Pen: …)` comment, which needs a post that writes comments.)
    pub pens: Vec<u16>,
}

impl Default for ReadBack {
    fn default() -> ReadBack {
        ReadBack { comments: Comments::Paren, ij_incremental: true, numbered: false, motion_always: true, dwell: Dwell::Seconds, decimals: [Some(3); 3], ij_decimals: Some(3), pens: vec![] }
    }
}

/// A post's comment style, as [`ReadBack`] gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comments { Paren, Semicolon, None }

/// The unit of `G4 P`, as [`ReadBack`] gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dwell { Seconds, Milliseconds, None }

/// The motion of a move line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion { Rapid, Feed, Cw, Ccw }

/// A move as the text says it, in mm and absolute work coordinates (`None`: not known from the text yet).
#[derive(Debug, Clone)]
pub struct TextMove<'a> {
    /// 1-based line number and the line as written.
    pub line: usize,
    pub text: &'a str,
    pub motion: Motion,
    pub from: [Option<f64>; 3],
    pub to: [Option<f64>; 3],
    /// The axes the line writes (bit 0 X, 1 Y, 2 Z).
    pub written: u8,
    /// An arc's centre (X, Y in mm).
    pub center: Option<[f64; 2]>,
    /// The line writes a motion code (G0 to G3) itself.
    pub coded: bool,
    /// The spindle runs, and the tool is a pen (drawing with the spindle off).
    pub spindle: bool,
    pub pen: bool,
}

/// What [`read`] reports, line by line. On one line, everything else comes before its move (a controller runs `M3`, `M6` and `G43`
/// before the motion of the same line).
#[derive(Debug, Clone)]
pub enum Fact<'a> {
    /// A `T` word; `text` is the line.
    Tool { line: usize, text: &'a str, number: f64 },
    /// `M6`: the tool changes to the one `T` selected.
    Change { line: usize, text: &'a str },
    /// The spindle starts (`M3`, `M4`) or stops (`M5`, `M2`, `M30`).
    Spindle { line: usize, on: bool },
    /// `G43`, with its `H` word.
    Offset { line: usize, text: &'a str, h: Option<f64> },
    /// A line that moves in another frame (`G53` machine coordinates, `G28` home, a relative move): the axes it gives are not known
    /// any more.
    Lost { line: usize, axes: u8 },
    Move(TextMove<'a>),
}

/// What the program does, measured from its text.
#[derive(Debug, Default, Clone)]
pub struct Trace {
    /// Length of everything cut with G1, G2 and G3, in mm (a G1 counts in 3D, like the generator's own figure).
    pub cut_len_mm: f64,
    /// The lowest Z of any cutting move, in mm, as the program writes it (its own zero).
    pub deepest_cut_mm: Option<f64>,
    /// Number of cutting moves.
    pub cut_moves: usize,
    /// Seconds of `G4` dwells, in the post's unit (`ReadBack::dwell`).
    pub dwell_s: f64,
    /// Problems, each as a ready sentence starting with a warning sign.
    pub hazards: Vec<String>,
}

#[derive(Default)]
struct Found { first: usize, count: usize }

/// Read the program. `lines` are the lines of the finished G-code, as written to the file.
pub fn trace(lines: &[String]) -> Trace { read(lines, &ReadBack::default(), &mut |_| {}) }

/// Read the program written by a post described by `rb`, and tell `sink` what each line does ([`Fact`]).
pub fn read(lines: &[String], rb: &ReadBack, sink: &mut dyn FnMut(&Fact)) -> Trace {
    let mut t = Trace::default();
    // one entry per kind of problem: the first line it happens on and how many lines have it
    let mut found: Vec<(&'static str, Found)> = Vec::new();
    let mut flag = |kind: &'static str, line: usize| {
        match found.iter_mut().find(|f| f.0 == kind) { Some(f) => f.1.count += 1, None => found.push((kind, Found { first: line, count: 1 })) }
    };

    let mut k = 1.0; // mm per unit
    let mut pos: [Option<f64>; 3] = [None; 3];
    let mut mode = 0u8; // 0..3 once a G word has set it
    let mut mode_set = false;
    let (mut spindle, mut spindle_s): (bool, Option<f64>) = (false, None);
    let mut feed: Option<f64> = None;
    let mut seen_sideways = false;
    // a pen plotter draws with the spindle off: the program says so with a comment (or a pen's tool number), until the next M3
    let mut pen = false;
    let mut tool: Option<f64> = None;
    // relative moves (G91) are not followed: what they move is not known until the program is absolute again
    let mut relative = false;
    let ms = match rb.dwell { Dwell::Milliseconds => 0.001, _ => 1.0 };

    for (n, raw) in lines.iter().enumerate() {
        let line_no = n + 1;
        let text = raw.as_str();
        // (the text of a comment may say anything, so only the part before one is looked at)
        let code = raw.split(['(', ';']).next().unwrap_or("");
        if raw.contains("(Pen:") || raw.contains("; Pen:") || raw.contains("Spindle: by hand") { pen = true; }
        if code.len() >= 3 && (code.contains("NaN") || code.contains("nan") || code.contains("inf") || code.contains("Inf")) { flag("nan", line_no); continue; }
        // the text of a Marlin stop is for the operator, not code
        let raw = raw.find("M0 ").map_or(raw.as_str(), |i| &raw[..i + 2]);
        let w = words(raw);
        let has = |c: char| w.iter().find(|x| x.0 == c).map(|x| x.1);
        let g: Vec<i64> = w.iter().filter(|x| x.0 == 'G').map(|x| x.1.round() as i64).collect();
        let coded = w.iter().any(|x| x.0 == 'G' && x.1 >= 0.0 && x.1 <= 3.0 && x.1.fract() == 0.0);
        if g.contains(&91) { relative = true; }
        if g.contains(&90) { relative = false; }
        if g.contains(&20) { k = 25.4; }
        if g.contains(&21) { k = 1.0; }
        for &c in &g { if (0..=3).contains(&c) { mode = c as u8; mode_set = true; } }
        if g.contains(&4) { if let Some(p) = has('P') { t.dwell_s += p * ms; } }
        let m: Vec<i64> = w.iter().filter(|x| x.0 == 'M').map(|x| x.1.round() as i64).collect();
        for x in &w {
            if x.0 == 'S' { spindle_s = Some(x.1); }
            if x.0 == 'F' { feed = Some(x.1); }
            if x.0 == 'T' { tool = Some(x.1); sink(&Fact::Tool { line: line_no, text, number: x.1 }); }
        }
        if m.contains(&6) {
            if tool.is_some_and(|t| t >= 0.0 && rb.pens.iter().any(|&p| p as f64 == t)) { pen = true; }
            sink(&Fact::Change { line: line_no, text });
        }
        if m.contains(&3) || m.contains(&4) {
            spindle = true;
            pen = false;
            if !spindle_s.is_some_and(|s| s > 0.0) { flag("s0", line_no); }
            sink(&Fact::Spindle { line: line_no, on: true });
        }
        if m.contains(&5) || m.contains(&2) || m.contains(&30) { spindle = false; sink(&Fact::Spindle { line: line_no, on: false }); }
        if g.contains(&43) { sink(&Fact::Offset { line: line_no, text, h: has('H') }); }

        let (x, y, z) = (has('X'), has('Y'), has('Z'));
        let written = x.is_some() as u8 | (y.is_some() as u8) << 1 | (z.is_some() as u8) << 2;
        // a line in machine coordinates (G53), a move home (G28, G30) or a relative move says nothing about the work position, which is
        // then not known any more
        if g.contains(&53) || g.contains(&28) || g.contains(&30) || relative {
            for (a, v) in [x, y, z].iter().enumerate() { if v.is_some() { pos[a] = None; } }
            if g.contains(&28) || g.contains(&30) { pos = [None; 3]; sink(&Fact::Lost { line: line_no, axes: 7 }); } else if written != 0 { sink(&Fact::Lost { line: line_no, axes: written }); }
            continue;
        }
        if !mode_set || written == 0 { continue; }
        let cut = mode >= 1;
        if (x.is_some() || y.is_some()) && !seen_sideways {
            seen_sideways = true;
            // nothing set the tool's height before the first sideways move: it goes wherever the tool stands
            if pos[2].is_none() && z.is_none() { flag("nolift", line_no); }
        }
        if cut {
            if !spindle && !pen { flag("spindle", line_no); }
            if mode == 1 && !feed.is_some_and(|f| f > 0.0) { flag("feed", line_no); }
        }
        let to = [x.map(|v| v * k).or(pos[0]), y.map(|v| v * k).or(pos[1]), z.map(|v| v * k).or(pos[2])];
        let mut center = None;
        if mode >= 2 {
            if let (Some(i), Some(j)) = (has('I'), has('J')) {
                center = if rb.ij_incremental { pos[0].zip(pos[1]).map(|(a, b)| [a + i * k, b + j * k]) } else { Some([i * k, j * k]) };
            }
        }
        if cut {
            t.cut_moves += 1;
            if let Some(z) = to[2] { t.deepest_cut_mm = Some(t.deepest_cut_mm.map_or(z, |d| d.min(z))); }
            if let (Some(a), Some(b)) = (pos[0].zip(pos[1]), to[0].zip(to[1])) {
                let dz = match (pos[2], to[2]) { (Some(p), Some(q)) => q - p, _ => 0.0 };
                if mode == 1 { t.cut_len_mm += ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2) + dz * dz).sqrt(); }
                else if let Some([cx, cy]) = center {
                    let r = (a.0 - cx).hypot(a.1 - cy);
                    let (a0, a1) = ((a.1 - cy).atan2(a.0 - cx), (b.1 - cy).atan2(b.0 - cx));
                    let tau = std::f64::consts::TAU;
                    let mut sweep = if mode == 3 { a1 - a0 } else { a0 - a1 };
                    while sweep <= 1e-9 { sweep += tau; }
                    while sweep > tau + 1e-9 { sweep -= tau; }
                    if (a.0 - b.0).hypot(a.1 - b.1) < 1e-9 { sweep = tau; }
                    let flat = r * sweep;
                    t.cut_len_mm += (flat * flat + dz * dz).sqrt();
                }
            }
        }
        let motion = match mode { 0 => Motion::Rapid, 1 => Motion::Feed, 2 => Motion::Cw, _ => Motion::Ccw };
        sink(&Fact::Move(TextMove { line: line_no, text, motion, from: pos, to, written, center, coded, spindle, pen }));
        pos = to;
    }
    if spindle { flag("running", lines.len()); }
    t.finish(found)
}
impl Trace {
    fn finish(mut self, found: Vec<(&'static str, Found)>) -> Trace {
        for (kind, f) in found {
            let more = if f.count > 1 { crate::i18n::tr_n(f.count as u64 - 1, " ({n} more line like it)", " ({n} more lines like it)") } else { String::new() };
            let msg = match kind {
                "nan" => tr_f("⚠ G-code line {line}: a value that is not a number{more}. Do not run this program.", &[("line", &f.first), ("more", &more)]),
                "spindle" => tr_f("⚠ G-code line {line}: a cutting move while the spindle is off{more}.", &[("line", &f.first), ("more", &more)]),
                "feed" => tr_f("⚠ G-code line {line}: a cutting move with no feed rate set{more}. Most controllers stop with an error.", &[("line", &f.first), ("more", &more)]),
                "s0" => tr_f("⚠ G-code line {line}: the spindle is started with no speed (S){more}.", &[("line", &f.first), ("more", &more)]),
                "nolift" => tr_f("⚠ G-code line {line}: the first sideways move comes before the program has set the tool's height{more}: raise the tool clear of the stock and clamps before you start.", &[("line", &f.first), ("more", &more)]),
                "running" => tr("⚠ The program ends with the spindle still running (no M5, M2 or M30 after the last M3).").to_string(),
                _ => continue,
            };
            self.hazards.push(msg);
        }
        self
    }
}

/// The problems of a program, as warnings for the Notes tab.
pub fn hazards(lines: &[String]) -> Vec<String> { trace(lines).hazards }


/// What the tool-changer checks expect, from the program and the post (plain data).
#[derive(Debug, Clone, Default)]
pub struct AtcExpect {
    /// The post's `stop_spindle_before_tool_change` property is on: no `M6` while the spindle runs.
    pub stop_before_change: bool,
    /// The post writes `G43 H#` (`caps.length_offset`): after every `M6`, a `G43` with that tool's `H` before the first Z move.
    pub length_offset: bool,
    /// The program's tool changes, in order: (tool number, length offset).
    pub tools: Vec<(u16, u16)>,
}

/// The checks of a program for a machine with an automatic tool changer, on the facts of [`read`]: the spindle is stopped before
/// every `M6` (when the post's property says so), every `M6` is followed by its length offset before the first Z move (when the post
/// writes them), the spindle runs before every cutting move (a pen draws with it off), and the tool loaded at each `M6` is the one the
/// program changes to. The first failure is kept: line number, text and what was expected.
pub struct AtcCheck { exp: AtcExpect, spindle: Option<usize>, tool: Option<(f64, usize)>, changes: usize, pending: Option<(u16, usize)>, err: Option<String> }

impl AtcCheck {
    pub fn new(exp: AtcExpect) -> AtcCheck { AtcCheck { exp, spindle: None, tool: None, changes: 0, pending: None, err: None } }

    fn fail(&mut self, msg: String) { if self.err.is_none() { self.err = Some(msg); } }

    pub fn fact(&mut self, f: &Fact) {
        if self.err.is_some() { return; }
        match f {
            Fact::Tool { line, text, number } => {
                if !self.exp.tools.iter().any(|t| t.0 as f64 == *number) {
                    let want = self.exp.tools.iter().map(|t| format!("T{}", t.0)).collect::<Vec<_>>().join(", ");
                    return self.fail(tr_f("G-code line {line} ({text}): T{t} is not a tool of this program (its tools are {want}).", &[("line", line), ("text", &text.trim()), ("t", number), ("want", &want)]));
                }
                self.tool = Some((*number, *line));
            }
            Fact::Change { line, text } => {
                let text = text.trim();
                let k = self.changes;
                self.changes += 1;
                if let Some(on) = self.spindle.filter(|_| self.exp.stop_before_change) {
                    return self.fail(tr_f("G-code line {line} ({text}): M6 (tool change) while the spindle runs (started on line {on}, no M5 since). The post's stop_spindle_before_tool_change is on: write M5 before M6.", &[("line", line), ("text", &text), ("on", &on)]));
                }
                let Some(&(number, offset)) = self.exp.tools.get(k) else {
                    return self.fail(tr_f("G-code line {line} ({text}): an M6 (tool change) the program does not have (it changes tools {n} times).", &[("line", line), ("text", &text), ("n", &self.exp.tools.len())]));
                };
                match self.tool {
                    Some((t, _)) if t == number as f64 => {}
                    Some((t, tl)) => return self.fail(tr_f("G-code line {line} ({text}): M6 loads T{t} (line {tl}), but the program changes to T{want} here.", &[("line", line), ("text", &text), ("t", &t), ("tl", &tl), ("want", &number)])),
                    None => return self.fail(tr_f("G-code line {line} ({text}): M6 with no T word before it; the program changes to T{want} here.", &[("line", line), ("text", &text), ("want", &number)])),
                }
                if self.exp.length_offset { self.pending = Some((offset, *line)); }
            }
            Fact::Spindle { line, on } => self.spindle = if *on { Some(*line) } else { None },
            Fact::Offset { line, text, h } => {
                if let Some((want, _)) = self.pending.take() {
                    if *h != Some(want as f64) {
                        let got = h.map_or("no H".to_string(), |h| format!("H{h}"));
                        self.fail(tr_f("G-code line {line} ({text}): G43 with {got}, but this tool's length offset is H{want}.", &[("line", line), ("text", &text.trim()), ("got", &got), ("want", &want)]));
                    }
                }
            }
            // a machine-coordinate Z (G53 G0 Z0, all the way up) is safe before the offset; G28 is not a work Z either
            Fact::Lost { .. } => {}
            Fact::Move(m) => {
                if let (Some((want, at)), true) = (self.pending, m.written & 4 != 0) {
                    return self.fail(tr_f("G-code line {line} ({text}): a Z move after the M6 on line {at} with no G43 H{want} (length offset) before it.", &[("line", &m.line), ("text", &m.text.trim()), ("at", &at), ("want", &want)]));
                }
                if m.motion != Motion::Rapid && !m.spindle && !m.pen {
                    self.fail(tr_f("G-code line {line} ({text}): a cutting move while the spindle is off.", &[("line", &m.line), ("text", &m.text.trim())]));
                }
            }
        }
    }

    /// The first failure, or that every tool change of the program was in the text.
    pub fn finish(self) -> Result<(), String> {
        if let Some(e) = self.err { return Err(e); }
        if self.changes < self.exp.tools.len() {
            return Err(tr_f("The G-code has {got} tool changes (M6), but the program changes tools {n} times.", &[("got", &self.changes), ("n", &self.exp.tools.len())]));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn l(s: &str) -> Vec<String> { s.lines().map(String::from).collect() }

    #[test]
    fn a_good_program_has_nothing_to_report_and_is_measured() {
        let t = trace(&l("(demo)\nG21 G90 G17\nG0 Z5\nM3 S12000\nG4 P2\nG0 X10 Y10\nG0 Z0.5\nG1 Z-1 F250\nG1 X20 F800\nG1 Y20\nG0 Z5\nM5\nG0 X0 Y0\nM2"));
        assert!(t.hazards.is_empty(), "{:?}", t.hazards);
        assert_eq!(t.cut_moves, 3);
        // 1.5 down (0.5 -> -1), then 10 and 10 along
        assert!((t.cut_len_mm - 21.5).abs() < 1e-9, "{}", t.cut_len_mm);
        assert_eq!(t.deepest_cut_mm, Some(-1.0));
    }

    #[test]
    fn numbered_lines_comments_and_inches_are_read() {
        let t = trace(&l("N10 G20 G90\nN20 G0 Z0.2\nN30 M3 S10000 (on)\nN40 G0 X1 Y1\nN50 G1 Z-0.1 F10 ; plunge\nN60 G1 X2\nN70 M5\nN80 M30"));
        assert!(t.hazards.is_empty(), "{:?}", t.hazards);
        assert!((t.deepest_cut_mm.unwrap() + 2.54).abs() < 1e-9);
        assert!((t.cut_len_mm - (0.3 * 25.4 + 25.4)).abs() < 1e-9, "{}", t.cut_len_mm);
    }

    #[test]
    fn arcs_are_measured() {
        // a half circle of radius 5 (centre 5 to the right of the start), counter-clockwise, then a full circle clockwise
        let t = trace(&l("G21 G90\nG0 Z5\nM3 S1000\nG0 X0 Y0\nG1 Z-1 F100\nG3 X10 Y0 I5 J0\nG2 X10 Y0 I-5 J0\nM5\nM2"));
        assert!(t.hazards.is_empty(), "{:?}", t.hazards);
        let want = 6.0 + std::f64::consts::PI * 5.0 + std::f64::consts::TAU * 5.0;
        assert!((t.cut_len_mm - want).abs() < 1e-9, "{} vs {}", t.cut_len_mm, want);
    }

    #[test]
    fn each_kind_of_problem_is_found() {
        let h = |s: &str| trace(&l(s)).hazards.join("\n");
        assert!(h("G21 G90\nG0 Z5\nG1 X5 F100\nM2").contains("spindle is off"));
        assert!(h("G21 G90\nG0 Z5\nM3 S1000\nG1 X5\nM5").contains("no feed"));
        assert!(h("G21 G90\nG0 Z5\nM3\nM5").contains("no speed"));
        assert!(h("G21 G90\nM3 S1000\nG0 X5 Y5\nM5").contains("before the program has set the tool's height"));
        assert!(h("G21 G90\nG0 Z5\nM3 S1000\nG0 X5 Y5\n").contains("still running"));
        assert!(h("G21 G90\nG0 Z5\nM3 S1000\nG1 X5 Y NaN F100\nM5").contains("not a number"));
        // a G53 lift counts only as far as it goes: the work height is still not known
        assert!(h("G21 G90\nG53 G0 Z0\nM3 S1000\nG0 X5 Y5\nM5").contains("tool's height"));
    }

    #[test]
    fn the_same_problem_on_many_lines_is_one_note() {
        let h = trace(&l("G21 G90\nG0 Z5\nG1 X5 F100\nG1 X6\nG1 X7\nM2")).hazards;
        assert_eq!(h.len(), 1);
        assert!(h[0].contains("line 3") && h[0].contains("2 more lines"), "{}", h[0]);
    }

    #[test]
    fn relative_programs_are_not_judged() {
        assert!(trace(&l("G21 G91\nG1 X5 F100\nM2")).hazards.is_empty());
    }

    // ---------- phase 3: the read-back description, facts and the tool-changer checks ----------

    /// What a TX-35 post writes: no spaces, modal G1, padded numbers, no comments.
    fn tx35() -> ReadBack { ReadBack { comments: Comments::None, motion_always: false, dwell: Dwell::None, ..ReadBack::default() } }

    /// The facts and the trace of a text.
    fn facts(lines: &[String], rb: &ReadBack) -> (Vec<String>, Trace) {
        let mut v = vec![];
        let t = read(lines, rb, &mut |f| v.push(match f {
            Fact::Tool { line, number, .. } => format!("{line}: T{number}"),
            Fact::Change { line, .. } => format!("{line}: M6"),
            Fact::Spindle { line, on } => format!("{line}: spindle {}", if *on { "on" } else { "off" }),
            Fact::Offset { line, h, .. } => format!("{line}: G43 H{}", h.map_or("-".into(), |h| h.to_string())),
            Fact::Lost { line, axes } => format!("{line}: lost {axes}"),
            Fact::Move(m) => format!("{}: {:?}{} {:?} written {} {}", m.line, m.motion, if m.coded { "" } else { " (modal)" }, m.to, m.written, if m.spindle { "spindle" } else { "" }),
        }));
        (v, t)
    }

    fn logo() -> Vec<String> { include_str!("../tests/data/tx35/vcarve/logo.nc").lines().map(String::from).collect() }

    #[test]
    fn tx35_text_is_read_without_spaces_and_with_modal_moves() {
        let lines = logo();
        assert_eq!(lines[12], "G1X74.934Y70.699Z8.200", "the fixture as committed");
        let (f, t) = facts(&lines, &tx35());
        let at = |n: usize| f.iter().filter(|s| s.starts_with(&format!("{n}: "))).cloned().collect::<Vec<_>>();
        // the header: G28 home and G53 machine Z say nothing about the work position
        assert_eq!(at(1), ["1: lost 7"]);
        assert_eq!(at(4), ["4: lost 4"]);
        assert_eq!(at(6), ["6: T11", "6: M6"]);
        assert_eq!(at(8), ["8: spindle on"]);
        assert_eq!(at(9), ["9: G43 H11"]);
        assert_eq!(at(10), ["10: Rapid [Some(74.232), Some(71.412), None] written 3 spindle"]);
        assert_eq!(at(13), ["13: Feed [Some(74.934), Some(70.699), Some(8.2)] written 7 spindle"]);
        // a modal line: no G, still a cut, Z kept
        assert_eq!(at(15), ["15: Feed (modal) [Some(76.242), Some(69.401), Some(8.2)] written 3 spindle"]);
        assert_eq!(at(903), ["903: T10", "903: M6"]);
        assert_eq!(at(904), ["904: G43 H10"]);
        assert_eq!(at(905), ["905: spindle on"]);
        assert_eq!(at(1690), ["1690: spindle off"]);
        // the only hazard is the real one: VCarve's first sideways move comes after a machine-coordinate lift only
        assert_eq!(t.hazards.len(), 1, "{:?}", t.hazards);
        assert!(t.hazards[0].contains("line 7"), "{:?}", t.hazards);
        assert!(t.cut_moves > 1000);
    }

    #[test]
    fn relative_moves_are_passed_over_until_the_program_is_absolute_again() {
        let (f, t) = facts(&l("G21 G91\nG0 Z5\nG90\nG0 Z5\nM3 S1000\nG0 X1 Y1\nG1 Z-1 F100\nM5\nM2"), &ReadBack::default());
        assert!(t.hazards.is_empty(), "{:?}", t.hazards);
        assert!(f.contains(&"2: lost 4".to_string()) && f.iter().any(|s| s.starts_with("4: Rapid [None, None, Some(5.0)]")), "{f:?}");
        assert_eq!(t.cut_moves, 1);
    }

    #[test]
    fn dwells_are_added_up_in_the_posts_unit() {
        let lines = l("G21 G90\nG40 G49 G80\nG4 P2\nG4 P0.13");
        assert!((trace(&lines).dwell_s - 2.13).abs() < 1e-9);
        let ms = ReadBack { dwell: Dwell::Milliseconds, ..ReadBack::default() };
        assert!((read(&l("G4 P130"), &ms, &mut |_| {}).dwell_s - 0.13).abs() < 1e-9);
    }

    #[test]
    fn absolute_arc_centres_are_read_when_the_post_says_so() {
        let abs = ReadBack { ij_incremental: false, ..ReadBack::default() };
        let mut c = None;
        read(&l("G21 G90\nG0 Z1\nM3 S1\nG0 X10 Y0\nG3 X0 Y10 I0 J0 F100\nM5"), &abs, &mut |f| if let Fact::Move(m) = f { if m.motion == Motion::Ccw { c = m.center; } });
        assert_eq!(c, Some([0.0, 0.0]));
        read(&l("G21 G90\nG0 Z1\nM3 S1\nG0 X10 Y0\nG3 X0 Y10 I-10 J0 F100\nM5"), &ReadBack::default(), &mut |f| if let Fact::Move(m) = f { if m.motion == Motion::Ccw { c = m.center; } });
        assert_eq!(c, Some([0.0, 0.0]));
    }

    #[test]
    fn a_pen_is_known_by_its_comment_or_by_its_tool_number() {
        let text = l("G21 G90\nG0 Z5\nT2 M6\nG0 X1 Y1\nG1 Z-0.5 F300\nM2");
        assert!(trace(&text).hazards.iter().any(|h| h.contains("spindle is off")));
        let pens = ReadBack { comments: Comments::None, pens: vec![2], ..ReadBack::default() };
        assert!(read(&text, &pens, &mut |_| {}).hazards.is_empty());
        let mut commented = text.clone();
        commented.insert(3, "(Pen: the spindle stays off)".into());
        assert!(trace(&commented).hazards.is_empty());
    }

    fn atc(text: &str, exp: AtcExpect) -> Result<(), String> {
        let mut c = AtcCheck::new(exp);
        read(&l(text), &ReadBack::default(), &mut |f| c.fact(f));
        c.finish()
    }
    fn exp(stop: bool, h: bool, tools: &[(u16, u16)]) -> AtcExpect { AtcExpect { stop_before_change: stop, length_offset: h, tools: tools.to_vec() } }

    /// A good two-tool program for a changer that writes G43 and stops the spindle first.
    const GOOD: &str = "G21 G90\nG53 G0 Z0\nT5 M6\nG43 H5\nM3 S10000\nG0 Z5\nG0 X1 Y1\nG1 Z-1 F100\nG0 Z5\nM5\nT7 M6\nG43 H17\nM3 S12000\nG0 X2\nG1 Z-1\nG0 Z5\nM5\nM30";

    #[test]
    fn a_good_tool_changer_program_passes_every_check() {
        assert_eq!(atc(GOOD, exp(true, true, &[(5, 5), (7, 17)])), Ok(()));
        // the TX-35 reference itself: G43 after every M6 and the right tools (it does not stop the spindle before M6)
        let mut c = AtcCheck::new(exp(false, true, &[(11, 11), (10, 10)]));
        read(&logo(), &tx35(), &mut |f| c.fact(f));
        assert_eq!(c.finish(), Ok(()));
    }

    #[test]
    fn m6_with_the_spindle_running_fails_when_the_stop_property_is_on() {
        let text = GOOD.replace("G0 Z5\nM5\nT7", "G0 Z5\nT7");
        let e = atc(&text, exp(true, true, &[(5, 5), (7, 17)])).unwrap_err();
        assert!(e.contains("line 10") && e.contains("started on line 5") && e.contains("M5 before M6"), "{e}");
        assert_eq!(atc(&text, exp(false, true, &[(5, 5), (7, 17)])), Ok(()), "only with the property");
        // the TX-35 reference changes tools with the spindle on
        let mut c = AtcCheck::new(exp(true, true, &[(11, 11), (10, 10)]));
        read(&logo(), &tx35(), &mut |f| c.fact(f));
        assert!(c.finish().unwrap_err().contains("line 903"));
    }

    #[test]
    fn every_m6_needs_its_length_offset_before_the_first_z_move() {
        let e = atc(&GOOD.replace("G43 H17\n", ""), exp(true, true, &[(5, 5), (7, 17)])).unwrap_err();
        assert!(e.contains("line 14") && e.contains("G1 Z-1") && e.contains("M6 on line 11") && e.contains("G43 H17"), "{e}");
        let e = atc(&GOOD.replace("G43 H17", "G43 H7"), exp(true, true, &[(5, 5), (7, 17)])).unwrap_err();
        assert!(e.contains("line 12") && e.contains("H7") && e.contains("H17"), "{e}");
        let e = atc(&GOOD.replace("G43 H5", "G43"), exp(true, true, &[(5, 5), (7, 17)])).unwrap_err();
        assert!(e.contains("no H"), "{e}");
        // a post without length offsets is not asked for them
        assert_eq!(atc(&GOOD.replace("G43 H17\n", ""), exp(true, false, &[(5, 5), (7, 17)])), Ok(()));
        // a sideways move does not need it yet, a Z move does
        assert_eq!(atc("G21 G90\nT5 M6\nM3 S1\nG0 X1 Y1\nG43 H5\nG0 Z5\nM5", exp(false, true, &[(5, 5)])), Ok(()));
    }

    #[test]
    fn the_spindle_must_run_for_every_cutting_move_unless_a_pen_draws() {
        let e = atc(&GOOD.replace("M3 S12000\n", ""), exp(true, true, &[(5, 5), (7, 17)])).unwrap_err();
        assert!(e.contains("line 14") && e.contains("spindle is off"), "{e}");
        let pen = "G21 G90\nT5 M6\nG43 H5\n(Pen: the spindle stays off)\nG0 Z5\nG1 Z0 F300\nG1 X5\nM30";
        assert_eq!(atc(pen, exp(true, true, &[(5, 5)])), Ok(()));
    }

    #[test]
    fn the_t_numbers_are_the_programs_tools_in_order() {
        let e = atc(&GOOD.replace("T7 M6", "T6 M6"), exp(true, true, &[(5, 5), (7, 17)])).unwrap_err();
        assert!(e.contains("line 11") && e.contains("T6 is not a tool"), "{e}");
        let e = atc(GOOD, exp(true, true, &[(7, 17), (5, 5)])).unwrap_err();
        assert!(e.contains("line 3") && e.contains("loads T5") && e.contains("T7"), "{e}");
        let e = atc(&GOOD.replace("T7 M6", "M6"), exp(true, true, &[(5, 5), (7, 17)])).unwrap_err();
        assert!(e.contains("loads T5"), "{e}");
        let e = atc(&GOOD.replace("T5 M6", "M6"), exp(true, true, &[(5, 5), (7, 17)])).unwrap_err();
        assert!(e.contains("no T word"), "{e}");
        let e = atc(GOOD, exp(true, true, &[(5, 5), (7, 17), (5, 5)])).unwrap_err();
        assert!(e.contains("2 tool changes") && e.contains("3 times"), "{e}");
        let e = atc(GOOD, exp(true, true, &[(5, 5)])).unwrap_err();
        assert!(e.contains("T7 is not a tool"), "{e}");
    }
}
