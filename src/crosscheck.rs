//! The motion cross-check (phase 3 of `docs/post-processor-spec.md`): the moves the post wrote, as `verify` reads them back from the
//! final text, must be the program's moves, in order. A post that drops, swaps, mis-scales or bends a move stops the export.
//!
//! `verify::read` parses the text (it shares no code with the generator or the post engine) and hands each move line to a
//! [`Checker`], which walks the program's motion events alongside: `Rapid`, `Feed`, `Arc`, and the `ProgramEnd` park (the first lift
//! and the park have no `Move` in `Output.moves`, so the events are the reference, not the move list). Moves are paired by order,
//! never through the post's source map, which a broken post could get wrong. The only use of the source map is to pass over the
//! machine's own start and end code (lines with no event), whose moves no event describes.
//!
//! Pairing rules:
//! - A text move and the next program move match when every axis both know is within the tolerance, an axis the program move gives is
//!   in the text, and both are rapids or both cuts.
//! - A program move that goes nowhere at the output's resolution (a feed-only `G1 F…`, a change below the last decimal that a modal
//!   word does not write) may be missing from the text; a text move that goes nowhere (a park template writing the height it is at)
//!   may be extra.
//! - An arc in the text that the program does not have is a fitted arc: it replaces a run of the program's cuts. Its end must match
//!   the last of them, and every point of the run must lie on the arc within the fitter's own tolerance (`arcs::ON_CIRCLE`) plus the
//!   rounding of the numbers, going round the arc's way.
//!
//! Tolerance per axis: half a unit of the output's last decimal, in mm (0.0005 mm for 3 decimals; 0.0254 × 0.5 × 10⁻⁴ mm for 4-decimal
//! inches), plus 1e-9 for the arithmetic.
use crate::i18n::{tr, tr_f};
use crate::program::{Event, Xyz};
use crate::verify::{Fact, Motion, ReadBack, TextMove};

/// The tolerances of one output, in mm.
#[derive(Debug, Clone, Copy)]
pub struct Tol {
    /// X, Y, Z: half a unit of the last decimal written, plus 1e-9.
    pub axis: [f64; 3],
    /// I, J: the same for the arc centre.
    pub ij: f64,
    /// A point of a fitted run against the arc: the fitter's tolerance plus the rounding of the point, the start and the centre.
    pub fit: f64,
}

impl Tol {
    /// The tolerances for text written as `rb` says, in units of `k` mm (1 for mm, 25.4 for inches).
    pub fn new(rb: &ReadBack, k: f64) -> Tol {
        let half = |d: Option<u32>| d.map_or(0.0, |d| 0.5 * 10f64.powi(-(d as i32)) * k) + 1e-9;
        let axis = rb.decimals.map(half);
        let ij = half(rb.ij_decimals);
        // the fitter puts the rounded points within ON_CIRCLE of a circle through the rounded start; the text's centre is that circle's
        // centre rounded (ij on each axis), so its radius differs by as much again; the program's point differs from its rounded form
        let fit = crate::arcs::ON_CIRCLE + std::f64::consts::SQRT_2 * (axis[0].max(axis[1]) + 2.0 * ij) + 1e-9;
        Tol { axis, ij, fit }
    }
}

/// A motion event of the program.
#[derive(Clone, Copy)]
enum Want<'a> { Line { to: &'a Xyz, rapid: bool }, Arc { to: &'a Xyz, center: [f64; 2], cw: bool } }

impl Want<'_> {
    fn to(&self) -> &Xyz { match self { Want::Line { to, .. } | Want::Arc { to, .. } => to } }
}

/// Walks the program's motion events while `verify::read` reports the text's moves ([`Checker::fact`]); the first mismatch is kept.
pub struct Checker<'a> {
    ev: &'a [Event],
    /// The next event to look at.
    i: usize,
    /// Where the program has the tool (work mm), as far as its moves have said.
    pos: [Option<f64>; 3],
    tol: Tol,
    motion_always: bool,
    /// The event of each line (`None`: the machine's own code).
    source: &'a [Option<usize>],
    /// Axes the machine's own code moved: the text's value is not the program's until a move of the post writes it again.
    stale: [bool; 3],
    err: Option<String>,
    /// Moves found in the machine's own start and end code (not checked).
    pub machine_moves: usize,
    /// Text moves checked (for the tests).
    pub checked: usize,
}

fn mm(p: &[Option<f64>; 3]) -> String {
    let s: Vec<String> = p.iter().zip(["X", "Y", "Z"]).filter_map(|(v, a)| v.map(|v| format!("{a}{:.4}", if v == 0.0 { 0.0 } else { v }))).collect();
    if s.is_empty() { "?".into() } else { s.join(" ") }
}

impl<'a> Checker<'a> {
    pub fn new(events: &'a [Event], source: &'a [Option<usize>], rb: &ReadBack, tol: Tol) -> Checker<'a> {
        Checker { ev: events, i: 0, pos: [None; 3], tol, motion_always: rb.motion_always, source, stale: [false; 3], err: None, machine_moves: 0, checked: 0 }
    }

    /// The next motion event from `self.i`: its index and what it asks for.
    fn next(&self) -> Option<(usize, Want<'a>)> {
        let ev = self.ev;
        (self.i..ev.len()).find_map(|j| match &ev[j] {
            Event::Rapid { to } => Some((j, Want::Line { to, rapid: true })),
            Event::Feed { to, .. } => Some((j, Want::Line { to, rapid: false })),
            Event::Arc(a) => Some((j, Want::Arc { to: &a.to, center: a.center, cw: a.cw })),
            Event::ProgramEnd { park: Some(p) } => Some((j, Want::Line { to: p, rapid: true })),
            _ => None,
        })
    }

    fn apply(&self, to: &Xyz) -> [Option<f64>; 3] { let mut p = self.pos; for (a, v) in p.iter_mut().enumerate() { if let Some(x) = to.get(a) { *v = Some(x); } } p }

    /// The text's position matches the program's: every axis both know within the tolerance, and every axis in `given` is in the text.
    fn same(&self, text: &[Option<f64>; 3], want: &[Option<f64>; 3], given: u8) -> bool {
        (0..3).all(|a| match (text[a], want[a]) {
            (Some(t), Some(w)) => (t - w).abs() <= self.tol.axis[a],
            (None, Some(_)) => given & (1 << a) == 0,
            _ => true,
        })
    }

    /// The program's move to `want` goes nowhere at the output's resolution.
    fn idle(&self, want: &[Option<f64>; 3]) -> bool {
        (0..3).all(|a| match (self.pos[a], want[a]) { (Some(p), Some(w)) => (p - w).abs() <= self.tol.axis[a], (None, Some(_)) => false, _ => true })
    }

    fn fail(&mut self, m: &TextMove, why: String) {
        if self.err.is_none() {
            self.err = Some(tr_f("G-code line {line} ({text}): {why} Export stopped: the post does not write the program's moves (check its move templates).",
                                 &[("line", &m.line), ("text", &m.text.trim()), ("why", &why)]));
        }
    }

    pub fn fact(&mut self, f: &Fact) {
        let Fact::Move(m) = f else { return };
        if self.err.is_some() { return; }
        // the machine's own code has no event: its moves are not the program's
        if self.source.get(m.line - 1).is_some_and(|s| s.is_none()) {
            for a in 0..3 { if m.written & (1 << a) != 0 { self.stale[a] = true; } }
            self.machine_moves += 1;
            return;
        }
        for a in 0..3 { if m.written & (1 << a) != 0 { self.stale[a] = false; } }
        let fresh = |p: [Option<f64>; 3], st: [bool; 3]| [0, 1, 2].map(|a| if st[a] { None } else { p[a] });
        let (from, to) = (fresh(m.from, self.stale), fresh(m.to, self.stale));
        self.checked += 1;
        if self.motion_always && !m.coded {
            return self.fail(m, tr_f("a move with no motion code, but this post writes G0 to G3 on every move; the program moves to {want} here.", &[("want", &mm(&self.next().map_or(self.pos, |(_, w)| self.apply(w.to()))))]));
        }
        match m.motion {
            Motion::Rapid | Motion::Feed => self.line(m, from, to),
            Motion::Cw | Motion::Ccw => self.arc(m, from, to),
        }
    }

    fn line(&mut self, m: &TextMove, from: [Option<f64>; 3], to: [Option<f64>; 3]) {
        let rapid = m.motion == Motion::Rapid;
        loop {
            let Some((j, want)) = self.next() else {
                return self.fail(m, tr_f("a move to {got} after the program's last move.", &[("got", &mm(&to))]));
            };
            let w = self.apply(want.to());
            let kind_ok = matches!(want, Want::Line { rapid: r, .. } if r == rapid);
            if kind_ok && self.same(&to, &w, want.to().mask()) {
                self.pos = w;
                self.i = j + 1;
                return;
            }
            if matches!(want, Want::Line { .. }) && self.idle(&w) { self.pos = w; self.i = j + 1; continue; }
            // a text move that goes nowhere (a template writing where the tool already is)
            if (0..3).all(|a| m.written & (1 << a) == 0 || from[a].zip(to[a]).is_some_and(|(p, q)| (p - q).abs() <= 1e-12)) { return; }
            let what = match want { Want::Arc { .. } => tr("an arc"), Want::Line { rapid: true, .. } => tr("a rapid (G0)"), Want::Line { rapid: false, .. } => tr("a cut") };
            return self.fail(m, tr_f("the text goes to {got}, but the program's next move is {what} to {want} (mm).", &[("got", &mm(&to)), ("what", &what), ("want", &mm(&w))]));
        }
    }

    fn arc(&mut self, m: &TextMove, from: [Option<f64>; 3], to: [Option<f64>; 3]) {
        let cw = m.motion == Motion::Cw;
        let (Some(c), Some(x0), Some(y0)) = (m.center, from[0], from[1]) else {
            return self.fail(m, tr_f("an arc whose start or centre is not known from the text; the program's tool is at {want}.", &[("want", &mm(&self.pos))]));
        };
        // an arc of the program: one for one
        if let Some((j, Want::Arc { to: t, center, cw: wcw })) = self.next() {
            let w = self.apply(t);
            let tc = self.tol.ij + self.tol.axis[0].max(self.tol.axis[1]);
            if wcw == cw && self.same(&to, &w, t.mask()) && (c[0] - center[0]).abs() <= tc && (c[1] - center[1]).abs() <= tc { self.pos = w; self.i = j + 1; return; }
            return self.fail(m, tr_f("the arc does not match the program's arc to {want} around X{cx} Y{cy}.", &[("want", &mm(&w)), ("cx", &format!("{:.4}", center[0])), ("cy", &format!("{:.4}", center[1]))]));
        }
        // a fitted arc: it stands for a run of the program's cuts
        let r = (x0 - c[0]).hypot(y0 - c[1]);
        let mut prev = [x0, y0];
        let mut n = 0;
        loop {
            let Some((j, want)) = self.next() else {
                return self.fail(m, tr_f("an arc to {got} after the program's last move.", &[("got", &mm(&to))]));
            };
            let w = self.apply(want.to());
            if !matches!(want, Want::Line { rapid: false, .. }) {
                return self.fail(m, tr_f("an arc where the program's next move is not a cut (it moves to {want}).", &[("want", &mm(&w))]));
            }
            let (Some(px), Some(py)) = (w[0], w[1]) else {
                return self.fail(m, tr_f("an arc, but the program's position {want} is not known in X and Y.", &[("want", &mm(&w))]));
            };
            let off = ((px - c[0]).hypot(py - c[1]) - r).abs();
            let cross = (prev[0] - c[0]) * (py - c[1]) - (prev[1] - c[1]) * (px - c[0]);
            let wrong_way = if cw { cross > r * self.tol.fit } else { cross < -r * self.tol.fit };
            if off > self.tol.fit || wrong_way || w[2].zip(self.pos[2]).is_some_and(|(a, b)| (a - b).abs() > self.tol.axis[2]) {
                return self.fail(m, tr_f("the arc does not follow the program's cuts: its point {want} (cut {n} of the run) is {off} mm off the arc (allowed {tol} mm){way}.",
                    &[("want", &mm(&w)), ("n", &(n + 1)), ("off", &format!("{off:.4}")), ("tol", &format!("{:.4}", self.tol.fit)), ("way", &if wrong_way { tr(", the other way round") } else { "" })]));
            }
            self.pos = w;
            self.i = j + 1;
            n += 1;
            prev = [px, py];
            if self.same(&to, &w, 0b011) { return; }
        }
    }

    /// The first mismatch; or, when the text ended, a program move that never came (one that goes somewhere).
    pub fn finish(mut self) -> Result<(), String> {
        if let Some(e) = self.err.take() { return Err(e); }
        while let Some((j, want)) = self.next() {
            let w = self.apply(want.to());
            if !self.idle(&w) {
                return Err(tr_f("The G-code ends before the program's move to {want} (mm, event {j}). Export stopped: the post does not write the program's moves (check its move templates).", &[("want", &mm(&w)), ("j", &j)]));
            }
            self.pos = w;
            self.i = j + 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program::{ArcTo, Event, Xyz};
    use crate::verify::read;

    fn l(s: &str) -> Vec<String> { s.lines().map(String::from).collect() }
    fn rapid(x: Option<f64>, y: Option<f64>, z: Option<f64>) -> Event { Event::Rapid { to: Xyz::new(x, y, z) } }
    fn cut(x: Option<f64>, y: Option<f64>, z: Option<f64>) -> Event { Event::Feed { to: Xyz::new(x, y, z), feed: 100.0, write_feed: false } }

    /// Checks `text` against `events` (no machine code); returns the number of text moves checked.
    fn check_with(events: &[Event], text: &str, rb: &ReadBack, k: f64, source: &[Option<usize>]) -> Result<usize, String> {
        let lines = l(text);
        let mut c = Checker::new(events, source, rb, Tol::new(rb, k));
        read(&lines, rb, &mut |f| c.fact(f));
        let n = c.checked;
        c.finish().map(|_| n)
    }
    fn check(events: &[Event], text: &str) -> Result<usize, String> { check_with(events, text, &ReadBack::default(), 1.0, &[]) }

    /// A short program: lift, to the start, plunge, two cuts, lift, park.
    fn prog() -> Vec<Event> {
        vec![Event::Setup, rapid(None, None, Some(5.0)), Event::SpindleOn { rpm: 1000.0 }, rapid(Some(10.0), Some(0.0), None), cut(None, None, Some(-1.0)),
             cut(Some(20.0), None, None), cut(None, Some(10.0), None), rapid(None, None, Some(5.0)), Event::SpindleOff, Event::ProgramEnd { park: Some(Xyz::new(Some(0.0), Some(0.0), None)) }]
    }
    const TEXT: &str = "G21 G90\nG0 Z5\nM3 S1000\nG0 X10 Y0\nG1 Z-1 F100\nG1 X20\nG1 Y10\nG0 Z5\nM5\nG0 X0 Y0\nM30";

    #[test]
    fn the_programs_own_text_passes() { assert_eq!(check(&prog(), TEXT), Ok(7)); }

    #[test]
    fn a_dropped_move_is_caught() {
        let e = check(&prog(), &TEXT.replace("G1 X20\n", "")).unwrap_err();
        assert!(e.contains("line 6 (G1 Y10)") && e.contains("X10.0000 Y10.0000 Z-1.0000") && e.contains("cut to X20.0000 Y0.0000 Z-1.0000"), "{e}");
    }

    #[test]
    fn two_swapped_moves_are_caught() {
        let e = check(&prog(), &TEXT.replace("G1 X20\nG1 Y10", "G1 Y10\nG1 X20")).unwrap_err();
        assert!(e.contains("line 6 (G1 Y10)") && e.contains("X20.0000 Y0.0000"), "{e}");
    }

    #[test]
    fn a_move_mis_scaled_by_25_4_is_caught() {
        // the cut written in inches in a metric program
        let e = check(&prog(), &TEXT.replace("G1 X20", "G1 X0.7874")).unwrap_err();
        assert!(e.contains("line 6 (G1 X0.7874)"), "{e}");
        // the whole program in mm numbers under G20
        let e = check_with(&prog(), &TEXT.replace("G21", "G20"), &ReadBack { decimals: [Some(4); 3], ..ReadBack::default() }, 25.4, &[]).unwrap_err();
        assert!(e.contains("line 2 (G0 Z5)") && e.contains("Z127.0000"), "{e}");
    }

    #[test]
    fn a_rapid_for_a_cut_and_an_extra_or_missing_move_are_caught() {
        let e = check(&prog(), &TEXT.replace("G1 X20", "G0 X20")).unwrap_err();
        assert!(e.contains("line 6") && e.contains("next move is a cut"), "{e}");
        let e = check(&prog(), &TEXT.replace("M30", "G0 X5\nM30")).unwrap_err();
        assert!(e.contains("line 11 (G0 X5)") && e.contains("after the program's last move"), "{e}");
        let e = check(&prog(), &TEXT.replace("G0 X0 Y0\n", "")).unwrap_err();
        assert!(e.contains("ends before the program's move to X0.0000 Y0.0000 Z5.0000"), "{e}");
    }

    #[test]
    fn inch_output_passes_at_the_rounding_limit_and_not_beyond() {
        // 3.13563 mm = 0.12345 in exactly: written with 4 decimals it is 0.1235 or 0.1234, half a unit off either way
        let ev = vec![rapid(None, None, Some(5.0 * 25.4)), rapid(Some(3.13563), Some(0.0), None)];
        let rb = ReadBack { decimals: [Some(4); 3], ..ReadBack::default() };
        for x in ["0.1235", "0.1234"] {
            assert_eq!(check_with(&ev, &format!("G20 G90\nG0 Z5\nG0 X{x} Y0"), &rb, 25.4, &[]), Ok(2), "X{x}");
        }
        assert!(check_with(&ev, "G20 G90\nG0 Z5\nG0 X0.1236 Y0", &rb, 25.4, &[]).is_err());
        assert!(check_with(&ev, "G20 G90\nG0 Z5\nG0 X0.1233 Y0", &rb, 25.4, &[]).is_err());
        let t = Tol::new(&rb, 25.4);
        assert!((t.axis[0] - (25.4 * 0.5e-4 + 1e-9)).abs() < 1e-15, "{:?}", t.axis);
        assert!((Tol::new(&ReadBack::default(), 1.0).axis[2] - 0.0005_f64 - 1e-9).abs() < 1e-15);
    }

    /// A quarter circle of radius 10 round (0, 0) from (10, 0) to (0, 10), as 16 cuts.
    fn quarter() -> Vec<Event> {
        let mut ev = vec![rapid(None, None, Some(5.0)), Event::SpindleOn { rpm: 1000.0 }, rapid(Some(10.0), Some(0.0), None), cut(None, None, Some(-1.0))];
        for i in 1..=16 {
            let a = std::f64::consts::FRAC_PI_2 * i as f64 / 16.0;
            ev.push(cut(Some(10.0 * a.cos()), Some(10.0 * a.sin()), None));
        }
        ev
    }
    const HEAD: &str = "G21 G90\nG0 Z5\nM3 S1000\nG0 X10 Y0\nG1 Z-1 F100\n";

    #[test]
    fn a_fitted_arc_passes_and_a_wrong_centre_or_direction_does_not() {
        assert_eq!(check(&quarter(), &format!("{HEAD}G3 X0 Y10 I-10 J0")), Ok(4));
        // the centre 0.05 mm off: the points are off the arc by more than the fitter allows
        let e = check(&quarter(), &format!("{HEAD}G3 X0 Y10 I-10.05 J0.05")).unwrap_err();
        assert!(e.contains("line 6") && e.contains("off the arc"), "{e}");
        // the right centre, the wrong way round
        let e = check(&quarter(), &format!("{HEAD}G2 X0 Y10 I-10 J0")).unwrap_err();
        assert!(e.contains("the other way round"), "{e}");
        // an arc that ends early leaves the rest of the run to the next line
        assert_eq!(check(&quarter(), &format!("{HEAD}G3 X7.071 Y7.071 I-10 J0\nG3 X0 Y10 I-7.071 J-7.071")), Ok(5));
        // ...and an end beyond the run is caught when the program stops cutting round
        let mut ev = quarter();
        ev.push(rapid(None, None, Some(5.0)));
        let e = check(&ev, &format!("{HEAD}G3 X-10 Y0 I-10 J0\nG0 Z5")).unwrap_err();
        assert!(e.contains("not a cut"), "{e}");
    }

    #[test]
    fn an_arc_event_is_matched_one_for_one() {
        let mut ev = vec![rapid(None, None, Some(5.0)), Event::SpindleOn { rpm: 1000.0 }, rapid(Some(10.0), Some(0.0), None), cut(None, None, Some(-1.0))];
        ev.push(Event::Arc(Box::new(ArcTo { to: Xyz::new(Some(0.0), Some(10.0), None), center: [0.0, 0.0], cw: false, feed: 100.0, write_feed: false })));
        assert_eq!(check(&ev, &format!("{HEAD}G3 X0 Y10 I-10 J0")), Ok(4));
        assert!(check(&ev, &format!("{HEAD}G3 X0 Y10 I-9 J1")).unwrap_err().contains("around X0.0000 Y0.0000"));
        assert!(check(&ev, &format!("{HEAD}G2 X0 Y10 I-10 J0")).is_err());
        assert!(check(&ev, &format!("{HEAD}G1 X0 Y10")).unwrap_err().contains("is an arc"));
    }

    #[test]
    fn moves_that_go_nowhere_may_be_missing_or_extra() {
        // a feed-only event and a change below the last decimal have no line of their own in a modal post
        let ev = vec![rapid(None, None, Some(5.0)), Event::Feed { to: Xyz::NONE, feed: 800.0, write_feed: true }, rapid(Some(1.0), Some(1.0), None),
                      rapid(Some(1.0002), None, None), rapid(Some(2.0), None, None)];
        assert_eq!(check(&ev, "G21 G90\nG0 Z5\nG1 F800\nG0 X1 Y1\nG0 X2"), Ok(3));
        // a park template that writes the height the tool is at
        let ev = vec![rapid(None, None, Some(5.0)), rapid(Some(1.0), Some(1.0), None), Event::ProgramEnd { park: Some(Xyz::new(Some(0.0), Some(0.0), None)) }];
        assert_eq!(check(&ev, "G21 G90\nG0 Z5\nG0 X1 Y1\nG0 Z5\nG0 X0 Y0"), Ok(4));
    }

    #[test]
    fn a_post_that_writes_motion_codes_always_cannot_leave_one_out() {
        let e = check(&prog(), &TEXT.replace("G1 Y10", "Y10")).unwrap_err();
        assert!(e.contains("line 7 (Y10)") && e.contains("no motion code"), "{e}");
        let modal = ReadBack { motion_always: false, ..ReadBack::default() };
        assert_eq!(check_with(&prog(), &TEXT.replace("G1 Y10", "Y10"), &modal, 1.0, &[]), Ok(7));
    }

    #[test]
    fn the_machines_own_code_is_passed_over() {
        // a start code line that moves X (source None) before the first lift
        let text = TEXT.replace("G0 Z5\nM3", "G0 X50\nG0 Z5\nM3");
        let mut source: Vec<Option<usize>> = (0..l(&text).len()).map(Some).collect();
        source[1] = None;
        let lines = l(&text);
        let rb = ReadBack::default();
        let ev = prog();
        let mut c = Checker::new(&ev, &source, &rb, Tol::new(&rb, 1.0));
        read(&lines, &rb, &mut |f| c.fact(f));
        assert_eq!(c.machine_moves, 1);
        assert_eq!(c.finish(), Ok(()));
        // without the source entry it is a move the program does not have
        assert!(check(&prog(), &text).is_err());
    }
}
