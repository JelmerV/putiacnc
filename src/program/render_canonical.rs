//! The canonical renderer: a [`Program`] written as the text of today's built-in dialects, byte for byte what the generator
//! wrote before the IR (phase 1 of `docs/post-processor-spec.md`). Everything that formats a G-code word lives here: units and
//! decimals, the move words, the dialect's setup block, tool change, spindle, dwell, stop and end lines, arc fitting, the
//! machine's own start and end code, Marlin's `;` comments and line numbers.
//!
//! The steps run in the order the generator ran them: every event as text, arc fitting (the trochoidal loops alone, or the whole
//! program when the machine does arcs), the start and end code, Marlin's comments, line numbers. A source map follows the lines
//! through every step: the event each line comes from (an arc: the first event of the run it replaces; start and end code: none).
use super::{Dialect, DwellWhy, Event, Meta, Program, Xyz};
use crate::model::Units;
use crate::toolpath::{to_fixed, tool_label};
use std::fmt::Write;

/// The text of a program and where each line comes from.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rendered {
    pub lines: Vec<String>,
    /// The event index of each line; `None` for the machine's own start and end code.
    pub source: Vec<Option<usize>>,
    /// Arcs written for the trochoidal loops (machine without arcs), and over the whole program (machine with arcs).
    pub troch_arcs: usize,
    pub arcs: usize,
}

/// Decimals of a coordinate in the output units.
fn decimals(u: Units) -> usize { if u == Units::Imperial { 4 } else { 3 } }

/// Writes the program.
pub fn render(p: &Program) -> Rendered {
    let m = &p.meta;
    let (mut lines, mut source) = base(p);
    let (dec, k) = (decimals(m.units), m.units.factor());
    // trochoidal loops are always written as arcs (as plain short lines the machine would slow down at every corner); from the back,
    // so the lines of the ranges before stay where they are
    let mut troch_arcs = 0;
    if !m.arcs {
        for &(a, b) in m.troch.iter().rev() {
            let (la, lb) = (first_line(&source, a), first_line(&source, b));
            let (fitted, from, n) = crate::arcs::fit_arcs_mapped(&lines[la..lb], dec, k);
            if n > 0 {
                let src: Vec<Option<usize>> = from.iter().map(|&j| source[la + j]).collect();
                lines.splice(la..lb, fitted);
                source.splice(la..lb, src);
                troch_arcs += n;
            }
        }
    }
    let mut arcs = 0;
    if m.arcs {
        let (fitted, from, n) = crate::arcs::fit_arcs_mapped(&lines, dec, k);
        if n > 0 {
            source = from.iter().map(|&j| source[j]).collect();
            lines = fitted;
            arcs = n;
        }
    }
    // the machine's own lines go in after the fitting, so it never sees them: after the setup, and before the last line
    if !m.start_code.is_empty() || !m.end_code.is_empty() {
        let start = first_line(&source, m.start_at);
        let last = lines.len().saturating_sub(1);
        for (i, l) in m.end_code.iter().enumerate() { lines.insert(last + i, l.clone()); source.insert(last + i, None); }
        for (i, l) in m.start_code.iter().enumerate() { lines.insert(start + i, l.clone()); source.insert(start + i, None); }
    }
    if m.dialect.marlin() { lines = lines.into_iter().map(|l| marlin_comment(&l)).collect(); }
    if m.numbered { lines = number_lines(lines); }
    Rendered { lines, source, troch_arcs, arcs }
}

/// The first line that comes from event `ev` or a later one (the number of lines when there is none). Only for a source map with
/// no `None` in it, which is ordered (before the start and end code go in).
fn first_line(source: &[Option<usize>], ev: usize) -> usize { source.partition_point(|s| s.is_some_and(|e| e < ev)) }

/// Every event as its text, with no arcs, start or end code, Marlin comments or line numbers: what the generator had written by
/// the time it finished the program. Each event is one line, except the setup block (one to three) and the end (two).
pub fn base(p: &Program) -> (Vec<String>, Vec<Option<usize>>) {
    let mut lines: Vec<String> = Vec::with_capacity(p.events.len() + 4);
    let mut source: Vec<Option<usize>> = Vec::with_capacity(p.events.len() + 4);
    for (i, e) in p.events.iter().enumerate() {
        event_lines(&p.meta, e, &mut lines);
        source.resize(lines.len(), Some(i));
    }
    (lines, source)
}

fn event_lines(m: &Meta, e: &Event, out: &mut Vec<String>) {
    let imp = m.units == Units::Imperial;
    match e {
        Event::Comment { text, .. } => out.push(format!("({text})")),
        Event::Setup => {
            let units = if imp { "G20" } else { "G21" };
            match m.dialect {
                Dialect::Marlin => {
                    // what Marlin knows: units and absolute moves. It has no work coordinate systems to pick, so the zero is where the operator sets it.
                    out.push(format!("{units} G90"));
                    out.push("(Marlin: jog the tool to the zero of the job and set it there with G92 X0 Y0 Z0 before you run this file)".into());
                    out.push("(Spindle: by hand. The program stops to ask you to start and stop it)".into());
                }
                Dialect::Mach => {
                    // metric or imperial, absolute, XY plane, feed per minute; cancel cutter compensation, tool length and canned cycles
                    out.push(format!("{units} G90 G17 G94"));
                    out.push("G40 G49 G80".into());
                    out.push(m.wcs.into());
                }
                // metric or imperial, absolute, XY plane, feed per minute, the work coordinate system: nothing is left over from before
                Dialect::Grbl | Dialect::GrblHal | Dialect::FluidNc => out.push(format!("{units} G90 G17 G94 {}", m.wcs)),
            }
        }
        // Mach3 and some LinuxCNC setups choke on anything but plain ASCII inside a comment
        Event::ToolChange { tool } => out.push(if m.dialect.mach() {
            format!("T{n} M6 (Tool {n}: {})", tool_label(&tool.name), n = tool.number)
        } else {
            format!("(Tool: {})", tool_label(&tool.name))
        }),
        Event::SpindleOn { rpm } => { let mut s = String::from("M3 S"); push_num(&mut s, *rpm); out.push(s); }
        Event::SpindleOff => out.push("M5".into()),
        Event::Dwell { seconds, why } => {
            let mut s = String::from("G4 P");
            push_num(&mut s, match why { DwellWhy::SpinUp => *seconds, DwellWhy::Requested => to_fixed(*seconds, 2) });
            out.push(s);
        }
        Event::Rapid { to } => out.push(motion("G0", to, None, m)),
        Event::Feed { to, feed, write_feed } => out.push(motion("G1", to, write_feed.then_some(*feed), m)),
        Event::Stop { message, .. } => out.push(match m.dialect {
            Dialect::Marlin => marlin_stop(message),
            d => stop_line(d.mach(), message),
        }),
        Event::ProgramEnd { park } => {
            if let Some(p) = park { out.push(motion("G0", p, None, m)); }
            // Marlin has no end-of-program word (M30 would delete a file on its SD card)
            out.push(match m.dialect { Dialect::Marlin => "(End of program)", Dialect::Mach => "M30", _ => "M2" }.into());
        }
    }
}

/// `G0` / `G1` with the axes the move gives, in the output units, and the feed when it is written.
fn motion(g: &str, to: &Xyz, feed: Option<f64>, m: &Meta) -> String {
    let (k, imp) = (m.units.factor(), m.units == Units::Imperial);
    let dec = decimals(m.units);
    let mut s = String::with_capacity(32);
    s.push_str(g);
    for (i, c) in ['X', 'Y', 'Z'].into_iter().enumerate() {
        if let Some(v) = to.get(i) { s.push(' '); s.push(c); push_num(&mut s, to_fixed(v / k, dec)); }
    }
    if let Some(f) = feed { s.push_str(" F"); push_num(&mut s, to_fixed(f / k, if imp { 2 } else { 1 })); }
    s
}

/// `toolpath::num` written into `s`: "0" for zero (also -0), else Rust's shortest form.
fn push_num(s: &mut String, v: f64) {
    if v == 0.0 { s.push('0'); } else { let _ = write!(s, "{}", v); }
}

/// Prefix every line with a line number: N10, N20, ...
pub fn number_lines(lines: Vec<String>) -> Vec<String> {
    lines.into_iter().enumerate().map(|(i, l)| format!("N{} {}", (i + 1) * 10, l)).collect()
}

/// A program stop with a message the operator can read. GRBL senders such as gSender drop (bracketed) comments before sending and
/// show a `;` comment instead, so GRBL gets `M0 ; message`; Mach3 has no `;` comments, so it gets `M0 (message)`.
pub fn stop_line(mach: bool, msg: &str) -> String { if mach { format!("M0 ({msg})") } else { format!("M0 ; {msg}") } }

/// A whole-line comment `(text)` written as `; text` (Marlin has no brackets for comments).
pub(crate) fn marlin_comment(line: &str) -> String {
    let t = line.trim();
    match t.strip_prefix('(').and_then(|r| r.strip_suffix(')')) { Some(inner) => format!("; {}", inner.trim()), None => line.to_string() }
}

/// A stop for Marlin: `M0 text`, the text plain ASCII with no letter touching a number (a reader would take "Z0" for a move).
pub fn marlin_stop(msg: &str) -> String {
    let mut t = String::new();
    let mut prev_alpha = false;
    for c in msg.chars().filter(|c| c.is_ascii() && !c.is_ascii_control() && !matches!(c, ';' | '(' | ')')) {
        if prev_alpha && (c.is_ascii_digit() || c == '.' || c == '-') { t.push(' '); }
        prev_alpha = c.is_ascii_alphabetic();
        t.push(c);
    }
    format!("M0 {}", t.trim())
}

/// The message of a line written by `stop_line` (either form), when it is one.
pub fn stop_message(line: &str) -> Option<&str> {
    let line = line.trim_end();
    if let Some(m) = line.strip_prefix("M0 ; ") { return Some(m); }
    if let Some(m) = line.strip_prefix("M0 (") { return Some(m.strip_suffix(')').unwrap_or(m)); }
    line.strip_prefix("M0 ").filter(|m| !m.is_empty())
}

#[cfg(test)]
mod tests {
    use super::super::{CommentKind, ToolRef};
    use super::*;

    fn tool(n: u32, name: &str) -> Event {
        Event::ToolChange { tool: Box::new(ToolRef { id: format!("t{n}"), name: name.into(), number: n, length_offset: None, dia: 3.175, shape: "flat".into() }) }
    }
    fn c(text: &str, kind: CommentKind) -> Event { Event::Comment { text: text.into(), kind } }
    fn rapid(x: Option<f64>, y: Option<f64>, z: Option<f64>) -> Event { Event::Rapid { to: Xyz::new(x, y, z) } }
    fn feed(x: Option<f64>, y: Option<f64>, z: Option<f64>, f: Option<f64>) -> Event {
        Event::Feed { to: Xyz::new(x, y, z), feed: f.unwrap_or(800.0), write_feed: f.is_some() }
    }

    /// A small two-tool program as the generator would make it for the given dialect: the tool change with a stop for the
    /// operator except on Mach3, and the spindle by hand on Marlin.
    fn program(dialect: Dialect, units: Units) -> Program {
        use CommentKind::*;
        let manual = !dialect.mach();
        let marlin = dialect.marlin();
        let mut ev = vec![c("PutiaCNC - test", Info), c("Stock 100 x 50 x 12 mm; XY0 = lower-left corner of stock, Z0 = top surface", Info), Event::Setup];
        let start_at = ev.len();
        ev.push(rapid(None, None, Some(5.0)));
        ev.push(tool(1, "1/8\u{2033} flat (end) mill"));
        ev.push(if marlin { Event::Stop { message: "Start the spindle at 18000 rpm, then resume".into(), optional: false } } else { Event::SpindleOn { rpm: 18000.0 } });
        if !marlin { ev.push(Event::Dwell { seconds: 2.0, why: DwellWhy::SpinUp }); }
        ev.extend([c("Operation 1: outside, tool 3.175 mm", Operation), rapid(Some(10.0), Some(-2.5), None), rapid(None, None, Some(0.5)),
                   feed(None, None, Some(0.0), Some(250.0)), c("Pass 1", Pass), feed(None, None, None, Some(800.0)),
                   feed(Some(20.0), None, Some(-1.0), None), feed(Some(12.7), Some(0.0004), None, None), rapid(None, None, Some(5.0))]);
        if !marlin { ev.push(Event::SpindleOff); }
        if manual { ev.push(Event::Stop { message: "Change tool to 6 mm, then touch off Z0 and resume".into(), optional: false }); }
        ev.push(tool(2, "6 mm"));
        ev.push(Event::Dwell { seconds: 0.125, why: DwellWhy::Requested });
        if !marlin { ev.push(Event::SpindleOff); }
        ev.push(Event::ProgramEnd { park: Some(Xyz::new(Some(0.0), Some(0.0), None)) });
        let wcs = if dialect.marlin() { "G54" } else { "G56" };
        Program { meta: Meta { dialect, units, wcs, start_at, ..Meta::default() }, events: ev }
    }

    fn text(p: &Program) -> String { render(p).lines.join("\n") }

    #[test]
    fn grbl_metric() {
        let want = "\
(PutiaCNC - test)
(Stock 100 x 50 x 12 mm; XY0 = lower-left corner of stock, Z0 = top surface)
G21 G90 G17 G94 G56
G0 Z5
(Tool: 1/8in flat -end- mill)
M3 S18000
G4 P2
(Operation 1: outside, tool 3.175 mm)
G0 X10 Y-2.5
G0 Z0.5
G1 Z0 F250
(Pass 1)
G1 F800
G1 X20 Z-1
G1 X12.7 Y0
G0 Z5
M5
M0 ; Change tool to 6 mm, then touch off Z0 and resume
(Tool: 6 mm)
G4 P0.13
M5
G0 X0 Y0
M2";
        for d in [Dialect::Grbl, Dialect::GrblHal, Dialect::FluidNc] { assert_eq!(text(&program(d, Units::Metric)), want, "{d:?}"); }
    }

    #[test]
    fn grbl_imperial() {
        let t = text(&program(Dialect::Grbl, Units::Imperial));
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(lines[2], "G20 G90 G17 G94 G56");
        assert_eq!(lines[3], "G0 Z0.1969");
        // 4 decimals, feeds with 2, and a value that rounds to nothing is written as 0
        assert_eq!(&lines[8..15], &["G0 X0.3937 Y-0.0984", "G0 Z0.0197", "G1 Z0 F9.84", "(Pass 1)", "G1 F31.5", "G1 X0.7874 Z-0.0394", "G1 X0.5 Y0"]);
        assert_eq!(&lines[lines.len() - 2..], &["G0 X0 Y0", "M2"]);
        // comment and message texts are not converted: they are final text
        assert!(t.contains("(Operation 1: outside, tool 3.175 mm)") && t.contains("M0 ; Change tool to 6 mm,"));
    }

    #[test]
    fn mach_metric() {
        let want = "\
(PutiaCNC - test)
(Stock 100 x 50 x 12 mm; XY0 = lower-left corner of stock, Z0 = top surface)
G21 G90 G17 G94
G40 G49 G80
G56
G0 Z5
T1 M6 (Tool 1: 1/8in flat -end- mill)
M3 S18000
G4 P2
(Operation 1: outside, tool 3.175 mm)
G0 X10 Y-2.5
G0 Z0.5
G1 Z0 F250
(Pass 1)
G1 F800
G1 X20 Z-1
G1 X12.7 Y0
G0 Z5
M5
T2 M6 (Tool 2: 6 mm)
G4 P0.13
M5
G0 X0 Y0
M30";
        assert_eq!(text(&program(Dialect::Mach, Units::Metric)), want);
        let i = text(&program(Dialect::Mach, Units::Imperial));
        assert!(i.starts_with("(PutiaCNC - test)\n(Stock 100 x 50 x 12 mm;") && i.contains("\nG20 G90 G17 G94\nG40 G49 G80\nG56\nG0 Z0.1969\n"), "{i}");
        // a stop on Mach3 has its message in brackets
        let mut p = program(Dialect::Mach, Units::Metric);
        p.events.insert(p.events.len() - 1, Event::Stop { message: "Operation 1 done: resume when ready".into(), optional: false });
        assert!(text(&p).contains("\nM0 (Operation 1 done: resume when ready)\n"));
    }

    #[test]
    fn marlin_metric() {
        let want = "\
; PutiaCNC - test
; Stock 100 x 50 x 12 mm; XY0 = lower-left corner of stock, Z0 = top surface
G21 G90
; Marlin: jog the tool to the zero of the job and set it there with G92 X0 Y0 Z0 before you run this file
; Spindle: by hand. The program stops to ask you to start and stop it
G0 Z5
; Tool: 1/8in flat -end- mill
M0 Start the spindle at 18000 rpm, then resume
; Operation 1: outside, tool 3.175 mm
G0 X10 Y-2.5
G0 Z0.5
G1 Z0 F250
; Pass 1
G1 F800
G1 X20 Z-1
G1 X12.7 Y0
G0 Z5
M0 Change tool to 6 mm, then touch off Z 0 and resume
; Tool: 6 mm
G4 P0.13
G0 X0 Y0
; End of program";
        assert_eq!(text(&program(Dialect::Marlin, Units::Metric)), want);
        assert!(text(&program(Dialect::Marlin, Units::Imperial)).contains("\nG20 G90\n; Marlin: jog"));
    }

    #[test]
    fn numbers_and_the_machines_own_code_in_every_dialect() {
        for (post, _) in crate::model::DIALECTS {
            let d = Dialect::from_post(post);
            for units in [Units::Metric, Units::Imperial] {
                let plain = render(&program(d, units));
                let mut p = program(d, units);
                p.meta.start_code = vec!["(start: dust shoe)".into(), "M8".into()];
                p.meta.end_code = vec!["M9".into()];
                let r = render(&p);
                // after the setup, before the first lift; the end code just before the last line
                let at = plain.lines.iter().position(|l| l.starts_with("G0 Z")).unwrap();
                let n = plain.lines.len();
                assert_eq!(r.lines.len(), n + 3);
                assert_eq!(&r.lines[..at], &plain.lines[..at], "{post}");
                let start = if d.marlin() { "; start: dust shoe" } else { "(start: dust shoe)" };
                assert_eq!(&r.lines[at..at + 3], &[start.to_string(), "M8".into(), plain.lines[at].clone()], "{post}");
                assert_eq!(&r.lines[n + 1..], &["M9".to_string(), plain.lines[n - 1].clone()], "{post}");
                assert_eq!(&r.source[at..at + 2], &[None, None]);
                assert_eq!(r.source[n + 1], None);
                assert_eq!(r.source[at + 2], Some(p.meta.start_at));
                // line numbers come last and cover everything
                p.meta.numbered = true;
                let num = render(&p);
                assert_eq!(num.lines.len(), r.lines.len());
                for (i, (a, b)) in num.lines.iter().zip(&r.lines).enumerate() { assert_eq!(*a, format!("N{} {}", (i + 1) * 10, b)); }
                assert_eq!(num.source, r.source);
            }
        }
    }

    #[test]
    fn every_line_has_its_event() {
        for (post, _) in crate::model::DIALECTS {
            let p = program(Dialect::from_post(post), Units::Metric);
            let r = render(&p);
            assert_eq!(r.lines.len(), r.source.len());
            assert!(r.source.windows(2).all(|w| w[0] <= w[1]), "{post}: in order");
            // every event writes at least one line, and the setup block and the end say so for all of theirs
            let mut seen: Vec<usize> = r.source.iter().map(|s| s.unwrap()).collect();
            seen.dedup();
            assert_eq!(seen, (0..p.events.len()).collect::<Vec<_>>(), "{post}");
            let setup = p.events.iter().position(|e| *e == Event::Setup).unwrap();
            let n = r.source.iter().filter(|s| **s == Some(setup)).count();
            assert_eq!(n, match Dialect::from_post(post) { Dialect::Mach | Dialect::Marlin => 3, _ => 1 }, "{post}");
            assert_eq!(r.source.iter().filter(|s| **s == Some(p.events.len() - 1)).count(), 2, "{post}: park and end");
        }
    }

    /// A program with a circle of short cuts (48 points, radius 10 around (30, 20)) between a lift and a plunge.
    fn circle_program(arcs: bool) -> (Program, usize) {
        let mut ev = vec![Event::Setup, rapid(None, None, Some(5.0)), rapid(Some(40.0), Some(20.0), None), feed(None, None, Some(-1.0), Some(200.0))];
        let first = ev.len();
        for i in 1..=48 {
            let a = i as f64 / 48.0 * std::f64::consts::TAU;
            ev.push(feed(Some(30.0 + 10.0 * a.cos()), Some(20.0 + 10.0 * a.sin()), None, if i == 1 { Some(800.0) } else { None }));
        }
        ev.push(rapid(None, None, Some(5.0)));
        ev.push(Event::ProgramEnd { park: Some(Xyz::new(Some(0.0), Some(0.0), None)) });
        (Program { meta: Meta { arcs, start_at: 1, ..Meta::default() }, events: ev }, first)
    }

    #[test]
    fn a_fitted_arc_comes_from_the_first_event_of_its_run() {
        let (p, first) = circle_program(true);
        let r = render(&p);
        let (plain, _) = base(&p);
        assert_eq!(r.lines, crate::arcs::fit_arcs(&plain, 3, 1.0).0, "the same text as fitting the lines");
        assert!(r.arcs >= 2 && r.troch_arcs == 0, "{} arcs", r.arcs);
        let arcs: Vec<usize> = (0..r.lines.len()).filter(|&i| r.lines[i].starts_with("G2 ") || r.lines[i].starts_with("G3 ")).collect();
        assert_eq!(arcs.len(), r.arcs);
        // the first arc starts the circle: its source is the first cut of the circle, and its feed is that cut's
        assert_eq!(r.source[arcs[0]], Some(first));
        assert!(r.lines[arcs[0]].ends_with(" F800"), "{}", r.lines[arcs[0]]);
        // each later arc comes from an event further along the circle; the lines around keep their own events
        assert!(arcs.windows(2).all(|w| r.source[w[0]] < r.source[w[1]]));
        assert!(r.source[*arcs.last().unwrap()].unwrap() < first + 48);
        let after = arcs.last().unwrap() + 1;
        assert_eq!((r.lines[after].as_str(), r.source[after]), ("G0 Z5", Some(first + 48)));
        assert_eq!(r.source[..arcs[0]].to_vec(), vec![Some(0), Some(1), Some(2), Some(3)]);
    }

    #[test]
    fn only_the_trochoidal_ranges_are_fitted_without_arcs() {
        let (mut p, first) = circle_program(false);
        let plain = render(&p);
        assert!(plain.lines.iter().all(|l| !l.starts_with("G3 ") && !l.starts_with("G2 ")) && plain.troch_arcs == 0);
        // the range starts a few events before the loops, so the fitter knows where the tool is
        p.meta.troch = vec![(first - 2, first + 48)];
        let r = render(&p);
        assert!(r.troch_arcs >= 2 && r.arcs == 0);
        // (the GRBL setup is one line, so event i is line i up to the end)
        let (base_lines, _) = base(&p);
        let (fitted, _) = crate::arcs::fit_arcs(&base_lines[first - 2..first + 48], 3, 1.0);
        assert_eq!(&r.lines[first - 2..first - 2 + fitted.len()], &fitted[..]);
        assert_eq!(&r.lines[..first - 2], &base_lines[..first - 2]);
        assert_eq!(&r.lines[first - 2 + fitted.len()..], &base_lines[first + 48..]);
        assert!(r.source.windows(2).all(|w| w[0] <= w[1]));
        // a range that holds no arc changes nothing
        p.meta.troch = vec![(0, 3)];
        assert_eq!(render(&p), plain);
    }

    #[test]
    fn stops_are_read_back() {
        assert_eq!(stop_message(&stop_line(false, "Change tool")), Some("Change tool"));
        assert_eq!(stop_message(&stop_line(true, "Change tool")), Some("Change tool"));
        assert_eq!(stop_message(&marlin_stop("Set Z0 (here); then go")), Some("Set Z 0 here then go"));
        assert_eq!(stop_message("M0"), None);
        assert_eq!(marlin_comment("  (a comment )"), "; a comment");
        assert_eq!(marlin_comment("G0 X1 (not whole)"), "G0 X1 (not whole)");
        assert_eq!(number_lines(vec!["G21".into(), "M2".into()]), vec!["N10 G21".to_string(), "N20 M2".to_string()]);
    }
}

/// The text of whole programs from `toolpath::generate`, dialect by dialect (these lived in toolpath.rs before the IR).
#[cfg(test)]
mod generated {
    use crate::model::*;
    use crate::toolpath::generate;

    fn drill_project(depth: f64, pass: Option<f64>) -> ProjectData {
        let mut d = crate::shapes::blank_project().data;
        d.loops.clear();
        d.loops.push(crate::shapes::new_circle([100.0, 20.0], 1.5, 1));
        d.loops.push(crate::shapes::new_circle([10.0, 10.0], 1.5, 1));
        d.loops.push(crate::shapes::new_circle([12.0, 30.0], 1.5, 1));
        d.ops.clear();
        let mut p = crate::shapes::default_op_params(&d.tools[0].id);
        p.kind = "drill".into();
        p.depth = depth;
        p.pass_depth = pass;
        d.ops.push(Operation { id: 1, p });
        d
    }

    #[test]
    fn line_numbers_are_optional() {
        let mut d = drill_project(3.0, None);
        let plain = generate(&d, Units::Metric).unwrap();
        assert!(!plain.lines[0].starts_with('N'));
        d.job.numbered = true;
        let n = generate(&d, Units::Metric).unwrap();
        assert_eq!(n.lines.len(), plain.lines.len());
        assert_eq!(n.lines[0], format!("N10 {}", plain.lines[0]));
        assert!(n.lines[n.lines.len() - 1].starts_with(&format!("N{} M2", n.lines.len() * 10)));
        // same moves and time either way
        assert_eq!(n.moves.len(), plain.moves.len());
        // the setting survives a save and load
        let j = serde_json::to_string(&d).unwrap();
        assert!(serde_json::from_str::<ProjectData>(&j).unwrap().job.numbered);
    }

    #[test]
    fn the_machines_own_start_and_end_code_is_added() {
        let mut d = drill_project(3.0, None);
        let plain = generate(&d, Units::Metric).unwrap();
        let mut m = d.machine.clone().unwrap_or_default();
        m.start_code = Some("M3 S8000\n\n  G4 P2  \n".into());
        m.end_code = Some("G53 G0 X0 Y200".into());
        d.machine = Some(m);
        let o = generate(&d, Units::Metric).unwrap();
        assert_eq!(o.lines.len(), plain.lines.len() + 3);
        // after the setup lines, before the first tool; the end block just before the final M2
        let i = o.lines.iter().position(|l| l == "M3 S8000").unwrap();
        assert_eq!(o.lines[i + 1], "G4 P2");
        assert_eq!(&o.lines[..i], &plain.lines[..i]);
        let n = o.lines.len();
        assert_eq!(&o.lines[n - 3..], &["G0 X0 Y0".to_string(), "G53 G0 X0 Y200".to_string(), "M2".to_string()][..]);
        // the moves and the time do not change, and the notes say so
        assert_eq!(o.moves.len(), plain.moves.len());
        assert!(o.notes.iter().any(|x| x.contains("2 start lines") && x.contains("1 end line")));
        // line numbers cover them too
        d.job.numbered = true;
        let nn = generate(&d, Units::Metric).unwrap();
        assert!(nn.lines[i].ends_with("M3 S8000") && nn.lines[i].starts_with('N'));
        // an empty box adds nothing
        d.job.numbered = false;
        d.machine.as_mut().unwrap().start_code = Some("  \n".into());
        d.machine.as_mut().unwrap().end_code = None;
        assert_eq!(generate(&d, Units::Metric).unwrap().lines, plain.lines);
        // saved with the machine
        let j = serde_json::to_string(d.machine.as_ref().unwrap()).unwrap();
        assert!(!j.contains("endCode") && j.contains("startCode"));
    }

    #[test]
    fn mach_dialect_has_its_own_header_tool_changes_and_end() {
        let mut d = drill_project(3.0, None);
        // a second operation with a different tool
        let t2 = d.tools[1].id.clone();
        let mut p = crate::shapes::default_op_params(&t2);
        p.kind = "drill".into();
        p.depth = 3.0;
        d.ops.push(Operation { id: 2, p });
        d.loops.push(crate::shapes::new_circle([80.0, 40.0], 1.0, 2));
        let grbl = generate(&d, Units::Metric).unwrap();
        assert!(grbl.gcode().contains("M0 ; Change tool") && grbl.lines.last().unwrap() == "M2" && !grbl.gcode().contains("M6"));

        d.job.post = "mach".into();
        let o = generate(&d, Units::Metric).unwrap();
        let g = o.gcode();
        assert!(o.lines[0].contains("Mach3"));
        assert!(g.contains("G21 G90 G17 G94") && g.contains("G40 G49 G80") && g.contains("\nG54"));
        let m6: Vec<&String> = o.lines.iter().filter(|l| l.contains("M6")).collect();
        assert_eq!(m6.len(), 2, "{m6:?}");
        assert!(m6[0].starts_with("T1 M6") && m6[1].starts_with("T2 M6"));
        assert!(m6.iter().all(|l| l.is_ascii()), "{m6:?}");
        assert!(!g.contains("M0"));
        // the spindle starts after the first tool is in
        let (i6, i3) = (o.lines.iter().position(|l| l.starts_with("T1")).unwrap(), o.lines.iter().position(|l| l.starts_with("M3")).unwrap());
        assert!(i6 < i3);
        assert_eq!(o.lines.last().unwrap(), "M30");
        // the same moves as the GRBL output
        assert_eq!(o.moves.len(), grbl.moves.len());
        let j = serde_json::to_string(&d).unwrap();
        assert!(serde_json::from_str::<ProjectData>(&j).unwrap().job.mach());
    }

    #[test]
    fn marlin_output_has_no_spindle_words_and_asks_the_operator() {
        let mut d = drill_project(3.0, None);
        let t2 = d.tools[1].id.clone();
        let mut p = crate::shapes::default_op_params(&t2);
        p.kind = "drill".into();
        p.depth = 3.0;
        d.ops.push(Operation { id: 2, p });
        d.loops.push(crate::shapes::new_circle([80.0, 40.0], 1.0, 2));
        let grbl = generate(&d, Units::Metric).unwrap();
        d.job.post = "marlin".into();
        let o = generate(&d, Units::Metric).unwrap();
        assert!(o.lines[0].contains("Marlin"), "{}", o.lines[0]);
        for bad in ["M3", "M5", "M2", "M30", "M6", "G54", "G17", "G94", "G4 "] { assert!(!o.lines.iter().any(|l| l.contains(bad)), "{bad} in {:?}", o.lines.iter().filter(|l| l.contains(bad)).collect::<Vec<_>>()); }
        assert!(o.lines.iter().all(|l| l.starts_with("; ") || !(l.contains('(') || l.contains(')'))), "a bracket comment is left");
        let stops: Vec<&String> = o.lines.iter().filter(|l| l.starts_with("M0 ")).collect();
        assert!(stops.iter().any(|l| l.contains("Start the spindle at")), "{stops:?}");
        assert!(stops.iter().any(|l| l.contains("Stop the spindle and change tool")), "{stops:?}");
        assert!(stops.last().unwrap().contains("Stop the spindle"), "{stops:?}");
        // a message is plain text with no letter touching a number
        assert!(stops.iter().all(|l| !l.contains(';') && !l.contains("Z0") && l.is_ascii()), "{stops:?}");
        // the same moves as GRBL, and the checker and the preview read it without complaint
        assert_eq!(o.moves.len(), grbl.moves.len());
        assert!(!o.notes.iter().any(|n| n.contains("spindle")), "{:?}", o.notes);
        let r = crate::replay::replay(&o.lines, &d);
        assert!(r.info.iter().any(|i| matches!(&i.kind, crate::replay::Kind::Stop(m) if m.starts_with("Start the spindle"))));
        assert!(r.info.iter().any(|i| matches!(&i.kind, crate::replay::Kind::Tool(_))));
        assert_eq!(d.job.wcs_count(), 1);
        // numbered lines keep working
        d.job.numbered = true;
        let n = generate(&d, Units::Metric).unwrap();
        assert!(n.lines.iter().all(|l| l.starts_with('N')));
        assert!(serde_json::from_str::<ProjectData>(&serde_json::to_string(&d).unwrap()).unwrap().job.marlin());
    }
}
