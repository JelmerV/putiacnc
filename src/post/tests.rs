//! Tests of the post engine: the built-in packages (the phase 1 renderer's tests, unchanged expectations), the template rules
//! (each handler, word mode, number format, the motion group, `?` lines, filters, comment styles), every load error, the render
//! errors, the lint of the built-ins and the TX-35 manifest of the spec.
use super::*;
use crate::model::Units;
use crate::program::{ArcTo, CommentKind, DwellWhy, Event, Meta, Program, ToolRef, Xyz};
use std::sync::LazyLock;

static MACHINE: LazyLock<Machine> = LazyLock::new(Machine::default);
static NO_PROPS: LazyLock<BTreeMap<String, PropValue>> = LazyLock::new(BTreeMap::new);
fn ctx() -> PostCtx<'static> { PostCtx { machine: &MACHINE, props: &NO_PROPS, safe_z: 5.0 } }

fn render_with(p: &Package, prog: &Program) -> Rendered { p.render(prog, &ctx()).unwrap_or_else(|e| panic!("{e}")) }
fn render(d: Dialect, prog: &Program) -> Rendered { render_with(builtin(d), prog) }
fn text(d: Dialect, prog: &Program) -> String { render(d, prog).lines.join("\n") }

/// A package from a manifest body (after a 3-line [post] header, so the body starts on line 4).
fn pkg(body: &str) -> Package { Package::load("t.toml", &format!("[post]\nformat_version = 1\nname = \"T\"\n{body}")).unwrap_or_else(|e| panic!("{e}")) }
fn load_err(body: &str) -> PostError { Package::load("t.toml", &format!("[post]\nformat_version = 1\nname = \"T\"\n{body}")).unwrap_err() }

fn tool(n: u32, name: &str) -> Event {
    Event::ToolChange { tool: Box::new(ToolRef { id: format!("t{n}"), name: name.into(), number: n, length_offset: None, dia: 3.175, shape: "flat".into() }) }
}
fn c(text: &str, kind: CommentKind) -> Event { Event::Comment { text: text.into(), kind } }
fn rapid(x: Option<f64>, y: Option<f64>, z: Option<f64>) -> Event { Event::Rapid { to: Xyz::new(x, y, z) } }
fn feed(x: Option<f64>, y: Option<f64>, z: Option<f64>, f: Option<f64>) -> Event { Event::Feed { to: Xyz::new(x, y, z), feed: f.unwrap_or(800.0), write_feed: f.is_some() } }
fn stop(m: &str) -> Event { Event::Stop { message: m.into(), optional: false } }

/// A small two-tool program as the generator would make it for the dialect's capabilities: a stop for the operator before the tool
/// change unless the controller changes tools, and stops instead of spindle commands when the operator runs the spindle.
fn program(d: Dialect, units: Units) -> Program {
    use CommentKind::*;
    let caps = builtin(d).caps();
    let manual = caps.tool_change == ToolChangeMode::Manual;
    let by_hand = caps.spindle == SpindleCtl::Manual;
    let mut ev = vec![c("PutiaCNC - test", Info), c("Stock 100 x 50 x 12 mm; XY0 = lower-left corner of stock, Z0 = top surface", Info), Event::Setup];
    let start_at = ev.len();
    ev.push(rapid(None, None, Some(5.0)));
    ev.push(tool(1, "1/8\u{2033} flat (end) mill"));
    ev.push(if by_hand { stop("Start the spindle at 18000 rpm, then resume") } else { Event::SpindleOn { rpm: 18000.0 } });
    if !by_hand { ev.push(Event::Dwell { seconds: 2.0, why: DwellWhy::SpinUp }); }
    ev.extend([c("Operation 1: outside, tool 3.175 mm", Operation), rapid(Some(10.0), Some(-2.5), None), rapid(None, None, Some(0.5)),
               feed(None, None, Some(0.0), Some(250.0)), c("Pass 1", Pass), feed(None, None, None, Some(800.0)),
               feed(Some(20.0), None, Some(-1.0), None), feed(Some(12.7), Some(0.0004), None, None), rapid(None, None, Some(5.0))]);
    if !by_hand { ev.push(Event::SpindleOff); }
    if manual { ev.push(stop("Change tool to 6 mm, then touch off Z0 and resume")); }
    ev.push(tool(2, "6 mm"));
    ev.push(Event::Dwell { seconds: 0.13, why: DwellWhy::Requested });
    if !by_hand { ev.push(Event::SpindleOff); }
    ev.push(Event::ProgramEnd { park: Some(Xyz::new(Some(0.0), Some(0.0), None)) });
    Program { meta: Meta { units, wcs: caps.wcs_name(2), start_at, ..Meta::default() }, events: ev }
}

// ---------- the built-ins: the phase 1 renderer's expectations ----------

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
    for d in [Dialect::Grbl, Dialect::GrblHal, Dialect::FluidNc] { assert_eq!(text(d, &program(d, Units::Metric)), want, "{d:?}"); }
}

#[test]
fn grbl_imperial() {
    let t = text(Dialect::Grbl, &program(Dialect::Grbl, Units::Imperial));
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
    assert_eq!(text(Dialect::Mach, &program(Dialect::Mach, Units::Metric)), want);
    let i = text(Dialect::Mach, &program(Dialect::Mach, Units::Imperial));
    assert!(i.starts_with("(PutiaCNC - test)\n(Stock 100 x 50 x 12 mm;") && i.contains("\nG20 G90 G17 G94\nG40 G49 G80\nG56\nG0 Z0.1969\n"), "{i}");
    // a stop on Mach3 has its message in brackets
    let mut p = program(Dialect::Mach, Units::Metric);
    p.events.insert(p.events.len() - 1, stop("Operation 1 done: resume when ready"));
    assert!(text(Dialect::Mach, &p).contains("\nM0 (Operation 1 done: resume when ready)\n"));
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
    assert_eq!(text(Dialect::Marlin, &program(Dialect::Marlin, Units::Metric)), want);
    assert!(text(Dialect::Marlin, &program(Dialect::Marlin, Units::Imperial)).contains("\nG20 G90\n; Marlin: jog"));
}

#[test]
fn numbers_and_the_machines_own_code_in_every_dialect() {
    for d in Dialect::ALL {
        for units in [Units::Metric, Units::Imperial] {
            let plain = render(d, &program(d, units));
            let mut p = program(d, units);
            p.meta.start_code = vec!["(start: dust shoe)".into(), "M8".into()];
            p.meta.end_code = vec!["M9".into()];
            let r = render(d, &p);
            // after the setup, before the first lift; the end code just before the last line
            let at = plain.lines.iter().position(|l| l.starts_with("G0 Z")).unwrap();
            let n = plain.lines.len();
            assert_eq!(r.lines.len(), n + 3);
            assert_eq!(&r.lines[..at], &plain.lines[..at], "{d:?}");
            let start = if d == Dialect::Marlin { "; start: dust shoe" } else { "(start: dust shoe)" };
            assert_eq!(&r.lines[at..at + 3], &[start.to_string(), "M8".into(), plain.lines[at].clone()], "{d:?}");
            assert_eq!(&r.lines[n + 1..], &["M9".to_string(), plain.lines[n - 1].clone()], "{d:?}");
            assert_eq!(&r.source[at..at + 2], &[None, None]);
            assert_eq!(r.source[n + 1], None);
            assert_eq!(r.source[at + 2], Some(p.meta.start_at));
            // line numbers come last and cover everything
            p.meta.numbered = true;
            let num = render(d, &p);
            assert_eq!(num.lines.len(), r.lines.len());
            for (i, (a, b)) in num.lines.iter().zip(&r.lines).enumerate() { assert_eq!(*a, format!("N{} {}", (i + 1) * 10, b)); }
            assert_eq!(num.source, r.source);
        }
    }
}

#[test]
fn every_line_has_its_event() {
    for d in Dialect::ALL {
        let p = program(d, Units::Metric);
        let r = render(d, &p);
        assert_eq!(r.lines.len(), r.source.len());
        assert!(r.source.windows(2).all(|w| w[0] <= w[1]), "{d:?}: in order");
        // every event writes at least one line, and the setup block and the end say so for all of theirs
        let mut seen: Vec<usize> = r.source.iter().map(|s| s.unwrap()).collect();
        seen.dedup();
        assert_eq!(seen, (0..p.events.len()).collect::<Vec<_>>(), "{d:?}");
        let setup = p.events.iter().position(|e| *e == Event::Setup).unwrap();
        let n = r.source.iter().filter(|s| **s == Some(setup)).count();
        assert_eq!(n, match d { Dialect::Mach | Dialect::Marlin => 3, _ => 1 }, "{d:?}");
        assert_eq!(r.source.iter().filter(|s| **s == Some(p.events.len() - 1)).count(), 2, "{d:?}: park and end");
        assert_eq!((r.extension.as_str(), r.line_ending), ("nc", LineEnding::Lf));
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

/// The program as text with no arcs at all (what the text fitter read before the IR).
fn base(d: Dialect, p: &Program) -> Rendered { let mut q = p.clone(); q.meta.arcs = false; q.meta.troch.clear(); render(d, &q) }

#[test]
fn a_fitted_arc_comes_from_the_first_event_of_its_run() {
    let (p, first) = circle_program(true);
    let r = render(Dialect::Grbl, &p);
    let plain = base(Dialect::Grbl, &p).lines;
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
    // in inches too
    let mut pi = p.clone();
    pi.meta.units = Units::Imperial;
    assert_eq!(render(Dialect::Grbl, &pi).lines, crate::arcs::fit_arcs(&base(Dialect::Grbl, &pi).lines, 4, 25.4).0);
}

#[test]
fn only_the_trochoidal_ranges_are_fitted_without_arcs() {
    let (mut p, first) = circle_program(false);
    let plain = render(Dialect::Grbl, &p);
    assert!(plain.lines.iter().all(|l| !l.starts_with("G3 ") && !l.starts_with("G2 ")) && plain.troch_arcs == 0);
    // the range starts a few events before the loops, so the fitter knows where the tool is
    p.meta.troch = vec![(first - 2, first + 48)];
    let r = render(Dialect::Grbl, &p);
    assert!(r.troch_arcs >= 2 && r.arcs == 0);
    // (the GRBL setup is one line, so event i is line i up to the end)
    let base_lines = plain.lines.clone();
    let (fitted, _) = crate::arcs::fit_arcs(&base_lines[first - 2..first + 48], 3, 1.0);
    assert_eq!(&r.lines[first - 2..first - 2 + fitted.len()], &fitted[..]);
    assert_eq!(&r.lines[..first - 2], &base_lines[..first - 2]);
    assert_eq!(&r.lines[first - 2 + fitted.len()..], &base_lines[first + 48..]);
    assert!(r.source.windows(2).all(|w| w[0] <= w[1]));
    // a range that holds no arc changes nothing
    p.meta.troch = vec![(0, 3)];
    assert_eq!(render(Dialect::Grbl, &p), plain);
    // a range that starts right at the loops does not know where the tool is when they start: like the text fitter, it waits for X and Y
    p.meta.troch = vec![(first, first + 48)];
    let (late, _) = crate::arcs::fit_arcs(&base_lines[first..first + 48], 3, 1.0);
    assert_eq!(&render(Dialect::Grbl, &p).lines[first..first + late.len()], &late[..]);
}

#[test]
fn operation_markers_write_nothing_and_do_not_break_a_run() {
    let (mut p, first) = circle_program(true);
    let r0 = render(Dialect::Grbl, &p);
    // markers in the middle of the circle: no line, and the same arcs
    p.events.insert(first + 10, Event::OperationEnd { index: 0 });
    p.events.insert(first + 11, Event::OperationStart { index: 1, name: "pocket".into() });
    let r = render(Dialect::Grbl, &p);
    assert_eq!(r.lines, r0.lines);
    assert_eq!(r.arcs, r0.arcs);
}

#[test]
fn stops_are_read_back() {
    let mut p = Program { meta: Meta::default(), events: vec![stop("Change tool"), stop("Set Z0 (here); then go")] };
    for d in Dialect::ALL {
        let r = render(d, &p);
        assert_eq!(stop_message(&r.lines[0]), Some("Change tool"), "{d:?}");
        let want = if d == Dialect::Marlin { "Set Z 0 here then go" } else { "Set Z0 (here); then go" };
        assert_eq!(stop_message(&r.lines[1]).map(|m| m.trim_end_matches(')')), Some(want.trim_end_matches(')')), "{d:?}: {}", r.lines[1]);
    }
    assert_eq!(render(Dialect::Grbl, &p).lines[0], "M0 ; Change tool");
    assert_eq!(render(Dialect::Mach, &p).lines[0], "M0 (Change tool)");
    assert_eq!(render(Dialect::Marlin, &p).lines[0], "M0 Change tool");
    // a message the plain filter empties still writes its stop, with the space after M0 (as Marlin's stop always did)
    p.events = vec![stop(";;;")];
    assert_eq!(render(Dialect::Marlin, &p).lines, vec!["M0 ".to_string()]);
    assert_eq!(stop_message("M0"), None);
    assert_eq!(number_lines(vec!["G21".into(), "M2".into()]), vec!["N10 G21".to_string(), "N20 M2".to_string()]);
}

// ---------- the template rules ----------

/// Every handler with the engine's defaults (a package with no [templates]).
#[test]
fn the_default_of_every_handler() {
    let p = pkg("");
    let want = "\
(PutiaCNC - test program)
(Stock 100 x 50 x 12 mm; XY0 = lower-left corner of stock, Z0 = top surface)
G21 G90 G17 G54
G0 Z5
(Tool: 1/8in flat -end- mill)
M3 S18000
G4 P2
(Operation 1: outside, tool 3.175 mm)
G0 X10 Y-2.5
G0 Z0.5
G1 Z-1 F250
(Pass 1)
G1 X20 F800
G1 X30 Y10
G3 X20 Y20 I-10 J0
G0 Z5
M5
M0 (Change tool to 6 mm, then touch off Z0 and resume)
(Tool: 6 mm)
M3 S12000
G4 P2
(Operation 2: pocket, tool 6 mm)
G0 X40 Y20
G1 Z-0.5 F300
G4 P0.13
G1 X50 Y20 F1000
G0 Z5
M5
M0 (Operation 2 done: resume when ready)
M5
G0 X0 Y0
M30";
    assert_eq!(render_with(&p, &test_program(p.caps())).lines.join("\n"), want);
    // an ATC with length offsets: T# M6 and G43 H# by default, no stop before the change
    let atc = pkg("[caps]\ntool_change = \"atc\"\nlength_offset = true\n");
    let t = render_with(&atc, &test_program(atc.caps())).lines;
    let i = t.iter().position(|l| l.starts_with("T2 M6")).unwrap();
    assert_eq!(&t[i - 1..i + 3], &["M5".to_string(), "T2 M6 (Tool 2: 6 mm)".into(), "G43 H2".into(), "M3 S12000".into()]);
    assert!(!t.iter().any(|l| l.contains("Change tool")));
}

/// A package with every handler of its own, VCarve TX-35 style: modal words, padded numbers, no spaces, modal G1, no comments.
fn tx_style() -> Package {
    pkg(r#"
[caps]
tool_change = "atc"
length_offset = true
dwell = "none"
comments = "none"
[format]
xyz = { decimals = 3, trim = false, length = true }
feed = { decimals = 1, trim = false, length = true }
rpm = { decimals = 0 }
[words]
X = { format = "xyz", modal = true }
Y = { format = "xyz", modal = true }
Z = { format = "xyz", modal = true }
I = { format = "xyz", incremental = true }
J = { format = "xyz", incremental = true }
F = { format = "feed", modal = true }
S = { format = "rpm" }
[modal]
motion = { codes = ["G0", "G1", "G2", "G3"], always = ["G0", "G2", "G3"] }
[templates]
on_open = "G21"
on_section = "(section {section.number}: {section.name})"
on_section_tool = """
M06T{tool.number}
G43H{tool.offset}
"""
on_section_end = "(end of {section.number})"
on_rapid = "{G}{X}{Y}{Z}"
on_linear = "{G}{X}{Y}{Z}{F}"
on_circular = "{G}{X}{Y}{I}{J}{F}"
on_dwell = "G4 {P}"
on_command = """
?spindle_on: {S}M03
?spindle_off: M5
?stop: M0
"""
on_comment = "({text})"
on_close = """
G0{Z:park}
G0{X:park}{Y:park}
M30
"""
"#)
}

#[test]
fn modal_words_padded_numbers_and_a_modal_motion_code() {
    let p = tx_style();
    let want = "\
G21
G0Z5.000
M06T1
G43H1
S18000M03
G0X10.000Y-2.500Z5.000
G0Z0.500
G1Z-1.000F250.0
X20.000F800.0
X30.000Y10.000
G3X20.000Y20.000I-10.000J0.000
G0Z5.000
M5
M06T2
G43H2
S12000M03
G0X40.000Y20.000Z5.000
G1Z-0.500F300.0
X50.000F1000.0
G0Z5.000
M5
M0
M5
G0Z5.000
G0X0.000Y0.000
M30";
    // comments none: no comment lines (and no section lines, which are comments here); dwell none: P has no value, so G4 is left out;
    // after each tool change and stop the modal words are written again (Z5.000 on the first rapid after a change)
    assert_eq!(render_with(&p, &test_program(p.caps())).lines.join("\n"), want);
}

#[test]
fn as_given_modal_and_always_words() {
    let body = |mode: &str| format!("[words]\nX = {{ format = \"xyz\", mode = \"{mode}\" }}\nY = {{ format = \"xyz\", mode = \"{mode}\" }}\nZ = {{ format = \"xyz\", mode = \"{mode}\" }}\n[templates]\non_rapid = \"G0 {{X}} {{Y}} {{Z}}\"\n");
    let prog = Program { meta: Meta::default(), events: vec![rapid(Some(1.0), Some(2.0), Some(3.0)), rapid(Some(1.0001), None, None), rapid(None, Some(5.0), None),
        stop("x"), rapid(Some(4.0), None, None)] };
    let lines = |mode: &str| render_with(&pkg(&body(mode)), &prog).lines.into_iter().filter(|l| l.starts_with("G0")).collect::<Vec<_>>();
    // as given: exactly the axes the move gives, even when the text is the same (X1 again)
    assert_eq!(lines("as_given"), vec!["G0 X1 Y2 Z3", "G0 X1", "G0 Y5", "G0 X4"]);
    // modal: what changed as text; a move that changes no text writes no line; after a stop everything again
    assert_eq!(lines("modal"), vec!["G0 X1 Y2 Z3", "G0 Y5", "G0 X4 Y5 Z3"]);
    // always: the full position every time
    assert_eq!(lines("always"), vec!["G0 X1 Y2 Z3", "G0 X1 Y2 Z3", "G0 X1 Y5 Z3", "G0 X4 Y5 Z3"]);
    // a feed as given follows write_feed; modal compares the text
    let fp = |mode: &str| pkg(&format!("[words]\nF = {{ format = \"feed\", mode = \"{mode}\" }}\n"));
    let fprog = Program { meta: Meta::default(), events: vec![feed(Some(1.0), None, None, Some(800.0)), feed(Some(2.0), None, None, Some(800.02)), feed(Some(3.0), None, None, None)] };
    assert_eq!(render_with(&fp("as_given"), &fprog).lines, vec!["G1 X1 F800", "G1 X2 F800", "G1 X3"]);
    assert_eq!(render_with(&fp("modal"), &fprog).lines, vec!["G1 X1 F800", "G1 X2", "G1 X3"]);
    assert_eq!(render_with(&fp("always"), &fprog).lines, vec!["G1 X1 F800", "G1 X2 F800", "G1 X3 F800"]);
}

#[test]
fn the_motion_group() {
    let prog = Program { meta: Meta::default(), events: vec![rapid(Some(1.0), None, None), feed(Some(2.0), None, None, Some(100.0)), feed(Some(3.0), None, None, None),
        rapid(None, None, Some(1.0)), feed(Some(4.0), None, None, None)] };
    let g = |always: &str| render_with(&pkg(&format!("[modal]\nmotion = {{ codes = [\"G3\", \"G2\", \"G1\", \"G0\"], always = [{always}] }}\n")), &prog).lines;
    assert_eq!(g(""), vec!["G0 X1", "G1 X2 F100", "X3", "G0 Z1", "G1 X4"]);
    assert_eq!(g("\"G1\""), vec!["G0 X1", "G1 X2 F100", "G1 X3", "G0 Z1", "G1 X4"]);
    assert_eq!(g("\"G0\", \"G1\", \"G2\", \"G3\""), g("\"G1\""));
}

#[test]
fn question_lines_take_properties_and_flags() {
    let body = r#"
[properties]
coolant = { type = "bool", default = false, title = "Coolant on with the spindle" }
park_z = { type = "number", default = 30, title = "Park height" }
[templates]
on_command = """
?spindle_on: M3 {S}
?spindle_on: ?coolant: M8
?spindle_off: M5
?stop: M0
"""
on_comment = """
?pass: (pass)
?!pass: ({text})
"""
on_close = """
G0 Z{prop.park_z}
M30
"""
"#;
    let p = pkg(body);
    assert_eq!(p.properties().len(), 2);
    let prog = Program { meta: Meta::default(), events: vec![c("Pass 1", CommentKind::Pass), c("hello", CommentKind::Info), Event::SpindleOn { rpm: 9000.0 },
        Event::ProgramEnd { park: None }] };
    assert_eq!(render_with(&p, &prog).lines, vec!["(pass)", "(hello)", "M3 S9000", "?coolant: M8", "G0 Z30", "M30"], "a ? line has one condition; the rest is text");
    let mut props = BTreeMap::new();
    props.insert("park_z".to_string(), PropValue::Number(25.5));
    let r = p.render(&prog, &PostCtx { machine: &MACHINE, props: &props, safe_z: 5.0 }).unwrap();
    assert_eq!(r.lines[4], "G0 Z25.5", "the machine's value");
    // a bool property switches a line
    let q = pkg(&body.replace("?spindle_on: ?coolant: M8", "?coolant: M8"));
    assert_eq!(render_with(&q, &prog).lines[3], "G0 Z30");
    props.insert("coolant".to_string(), PropValue::Bool(true));
    let r = q.render(&prog, &PostCtx { machine: &MACHINE, props: &props, safe_z: 5.0 }).unwrap();
    assert_eq!(&r.lines[2..4], &["M3 S9000".to_string(), "M8".into()]);
    // a value of the wrong type is an error
    props.insert("coolant".to_string(), PropValue::Number(1.0));
    let e = q.render(&prog, &PostCtx { machine: &MACHINE, props: &props, safe_z: 5.0 }).unwrap_err();
    assert!(e.message.contains("property `coolant`"), "{e}");
}

#[test]
fn tool_variables_and_flags() {
    let p = pkg(r#"
[caps]
tool_change = "atc"
[templates]
on_section_tool = """
?first: (first tool)
M6 T{tool.number} H{tool.offset} {S} ({tool.name})
?has_next: T{next_tool.number} (next: {next_tool.name|comment})
"""
"#);
    let mut prog = test_program(p.caps());
    if let Event::ToolChange { tool } = &mut prog.events[5] { tool.length_offset = Some(21); } else { panic!("{:?}", prog.events[5]); }
    let t = render_with(&p, &prog).lines;
    let a = t.iter().position(|l| l == "(first tool)").unwrap();
    assert_eq!(&t[a..a + 3], &["(first tool)".to_string(), "M6 T1 H21 S18000 (1/8\u{2033} flat (end) mill)".into(), "T2 (next: 6 mm)".into()]);
    let b = t.iter().position(|l| l.starts_with("M6 T2")).unwrap();
    assert_eq!(t[b], "M6 T2 H2 S12000 (6 mm)", "the offset is the number by default; S is the speed the tool starts at");
    assert!(!t[b + 1].starts_with('T'), "no next tool after the last");
}

#[test]
fn dwell_units() {
    let prog = Program { meta: Meta::default(), events: vec![Event::Dwell { seconds: 2.0, why: DwellWhy::SpinUp }, Event::Dwell { seconds: 0.13, why: DwellWhy::Requested }] };
    assert_eq!(render_with(&pkg(""), &prog).lines, vec!["G4 P2", "G4 P0.13"]);
    assert_eq!(render_with(&pkg("[caps]\ndwell = \"milliseconds\"\n"), &prog).lines, vec!["G4 P2000", "G4 P130"]);
    assert!(render_with(&pkg("[caps]\ndwell = \"none\"\n"), &prog).lines.is_empty(), "P has no value, so the line is left out");
    let flags = pkg("[templates]\non_dwell = \"?spin_up: G4 {P} (spin up)\\n?requested: G4 {P}\"\n");
    assert_eq!(render_with(&flags, &prog).lines, vec!["G4 P2 (spin up)", "G4 P0.13"]);
}

#[test]
fn comment_styles_inline_comments_and_the_machines_code() {
    let src = |style: &str| format!("[caps]\ntool_change = \"atc\"\ncomments = \"{style}\"\n[templates]\non_section_tool = \"T{{tool.number}} M6 (Tool {{tool.number}}: {{tool.name|comment}})\"\n");
    let mut prog = Program { meta: Meta { start_code: vec!["(dust shoe down)".into(), "M8 (air)".into()], end_code: vec!["( off )".into()], ..Meta::default() },
        events: vec![c("  Pass 1 ", CommentKind::Pass), tool(1, "6 mm (2 flutes)"), Event::ProgramEnd { park: None }] };
    prog.meta.start_at = 1;
    assert_eq!(render_with(&pkg(&src("paren")), &prog).lines, vec!["(  Pass 1 )", "(dust shoe down)", "M8 (air)", "T1 M6 (Tool 1: 6 mm -2 flutes-)", "( off )", "M30"]);
    assert_eq!(render_with(&pkg(&src("semicolon")), &prog).lines, vec!["; Pass 1", "; dust shoe down", "M8 (air)", "T1 M6 ; Tool 1: 6 mm -2 flutes-", "; off", "M30"]);
    assert_eq!(render_with(&pkg(&src("none")), &prog).lines, vec!["M8 (air)", "T1 M6", "M30"]);
}

#[test]
fn an_arc_event_through_on_circular() {
    let arc = |cw| Event::Arc(Box::new(ArcTo { to: Xyz::new(Some(20.0), Some(0.0), Some(-1.0)), center: [15.0, 0.0], cw, feed: 600.0, write_feed: true }));
    let prog = Program { meta: Meta::default(), events: vec![rapid(Some(10.0), Some(0.0), None), arc(true), arc(false)] };
    assert_eq!(render(Dialect::Grbl, &prog).lines, vec!["G0 X10 Y0", "G2 X20 Y0 I5 J0 F600", "G3 X20 Y0 I-5 J0 F600"]);
    let mut inch = prog.clone();
    inch.meta.units = Units::Imperial;
    assert_eq!(render(Dialect::Grbl, &inch).lines[1], "G2 X0.7874 Y0 I0.1969 J0 F23.62");
    // the default on_circular writes Z (a helix)
    assert_eq!(render_with(&pkg(""), &prog).lines[1], "G2 X20 Y0 Z-1 I5 J0 F600");
    // an arc needs to know where it starts
    let e = builtin(Dialect::Grbl).render(&Program { meta: Meta::default(), events: vec![arc(true)] }, &ctx()).unwrap_err();
    assert!(e.message.contains("start is unknown"), "{e}");
}

#[test]
fn park_words_and_braces() {
    let p = pkg("[words]\nZ = { format = \"xyz\", modal = true }\n[templates]\non_close = \"G0 {Z:park} ({{park}})\\nG0 {X:park} {Y:park}\\nM30\"\n");
    let prog = Program { meta: Meta::default(), events: vec![rapid(Some(3.0), None, Some(7.0)), stop("x"), Event::ProgramEnd { park: Some(Xyz::new(Some(0.0), Some(0.0), None)) }] };
    // Z:park: the park has no Z, so the current Z (written again: the stop made the modal Z forget); X and Y as the park gives them
    assert_eq!(render_with(&p, &prog).lines[2..], ["G0 Z7 ({park})".to_string(), "G0 X0 Y0".into(), "M30".into()]);
}

#[test]
fn no_arcs_when_the_post_has_none() {
    let (p, _) = circle_program(true);
    let none = pkg("[caps]\narcs = \"none\"\n");
    let r = render_with(&none, &p);
    assert!(r.lines.iter().all(|l| !l.starts_with("G2 ") && !l.starts_with("G3 ")) && r.arcs == 0);
    assert_eq!(r.notes.len(), 1, "{:?}", r.notes);
    assert!(r.notes[0].contains("no arcs"));
    // and a smaller sweep makes more, shorter arcs
    let short = pkg("[caps]\nmax_arc_sweep = 45\n");
    let (a, b) = (render_with(&pkg(""), &p).arcs, render_with(&short, &p).arcs);
    assert!(b > a && b >= 8, "{a} arcs, {b} with 45 degrees");
}

// ---------- errors ----------

#[test]
fn every_load_error_names_the_file_the_line_and_the_fix() {
    let at = |body: &str, line: Option<usize>, has: &str| {
        let e = load_err(body);
        assert_eq!(e.file, "t.toml");
        assert_eq!(e.line, line, "{body}: {e}");
        assert!(e.message.contains(has), "{body}: {e}");
        assert!(e.to_string().starts_with(&match line { Some(l) => format!("t.toml:{l}: "), None => "t.toml: ".into() }));
    };
    // the version
    let e = Package::load("t.toml", "[post]\nname = \"T\"\n").unwrap_err();
    assert!(e.message.contains("add `format_version = 1`"), "{e}");
    let e = Package::load("t.toml", "[post]\nformat_version = 2\nname = \"T\"\n[caps]\nteleport = true\n").unwrap_err();
    assert_eq!(e.line, Some(2));
    assert!(e.message.contains("format_version 2 is not one this PutiaCNC reads"), "the version first, before the unknown key: {e}");
    // syntax, unknown keys and sections, wrong types
    at("[caps\narcs = 1\n", Some(4), "");
    at("[caps]\nteleport = true\n", Some(5), "unknown field `teleport`");
    at("[caps]\narcs = \"spiral\"\n", Some(5), "unknown variant `spiral`");
    at("[cap]\narcs = \"xy\"\n", Some(4), "unknown field `cap`");
    at("[templates]\non_teleport = \"G0\"\n", Some(5), "unknown field `on_teleport`");
    at("[caps]\nlength_offset = \"yes\"\n", Some(5), "invalid type");
    let e = Package::load("t.toml", "[post]\nformat_version = 1\n").unwrap_err();
    assert!(e.message.contains("missing field `name`") && e.message.contains("add it"), "{e}");
    at("[post2]\n", Some(4), "unknown field `post2`");
    let e = Package::load("t.toml", "[post]\nformat_version = 1\nname = \" \"\n").unwrap_err();
    assert!(e.message.contains("name is empty"), "{e}");
    // caps values
    at("[caps]\nmax_arc_sweep = 360\n", Some(5), "at most 180");
    at("[caps]\nwcs_count = 12\n", Some(5), "1 (one fixed system) to 9");
    // properties
    at("\n[properties]\nBad = { type = \"bool\", default = true }\n", Some(6), "lowercase");
    at("[properties]\nx = { type = \"bool\", default = 1 }\n", Some(5), "true or false");
    at("[properties]\nx = { type = \"colour\", default = \"red\" }\n", Some(5), "unknown type \"colour\"");
    at("[properties]\nx = { type = \"enum\", default = \"c\", options = [\"a\", \"b\"] }\n", Some(5), "one of the options (a, b)");
    at("[properties]\nx = { type = \"enum\", default = \"a\" }\n", Some(5), "needs options");
    at("[properties]\nx = { type = \"int\", default = 1, options = [\"a\"] }\n", Some(5), "only an enum has options");
    at("[properties]\nx = { type = \"int\", default = 1.5 }\n", Some(5), "whole number");
    at("[properties]\nx = { type = \"text\", default = 1 }\n", Some(5), "text in quotes");
    at("[properties]\nx = { type = \"bool\", default = true, colour = 1 }\n", Some(5), "unknown field `colour`");
    // formats and words
    at("[format]\nxyz = { decimals = 12 }\n", Some(5), "0 to 9");
    at("[format]\nxyz = { trim = false }\n", Some(5), "needs decimals");
    at("[format]\nxyz = { inch_decimals = 4 }\n", Some(5), "needs decimals (for mm) too");
    at("[words]\nQ = { format = \"xyz\" }\n", Some(5), "the words are X Y Z I J F S P");
    at("[words]\nG = { format = \"xyz\" }\n", Some(5), "[modal]");
    at("[words]\nN = { format = \"xyz\" }\n", Some(5), "line numbers");
    at("[words]\nT = { format = \"xyz\" }\n", Some(5), "{tool.number}");
    at("[words]\nX = { format = \"mm\" }\n", Some(5), "no format `mm`");
    at("[words]\nX = { mode = \"modal\", modal = true }\n", Some(5), "not both");
    at("[words]\nX = { mode = \"sometimes\" }\n", Some(5), "unknown variant `sometimes`");
    at("[words]\nI = { incremental = false }\n", Some(5), "absolute arc centres");
    at("[words]\nF = { incremental = true }\n", Some(5), "only for I and J");
    // the motion group
    at("[modal]\nplane = { codes = [\"G17\"] }\n", Some(5), "one group, `motion`");
    at("[modal]\nmotion = { codes = [\"G0\", \"G1\"] }\n", Some(5), "G0, G1, G2 and G3");
    at("[modal]\nmotion = { codes = [\"G0\", \"G1\", \"G2\", \"G3\"], always = [\"G4\"] }\n", Some(5), "`G4` in always");
    // templates: the line of the template in the file, also inside a multi-line string
    at("[templates]\non_rapid = \"G0 {Q}\"\n", Some(5), "{Q} is not a word");
    at("[templates]\non_close = \"\"\"\nG0 {X:park}\nM30 {tool.number}\n\"\"\"\n", Some(7), "has no value here");
    at("[templates]\non_close = '''\nM30\n{message}'''\n", Some(7), "{message} has no value here");
    at("[templates]\non_command = \"?nope: M0\"\n", Some(5), "not a property or a flag");
    at("[templates]\non_circular = \"\"\n", Some(5), "on_circular is empty");
    // the same template is fine where its words have values
    pkg("[templates]\non_close = \"\"\"\nG0 {X:park}\nM30\n\"\"\"\n");
    pkg("[caps]\narcs = \"none\"\n[templates]\non_circular = \"\"\n");
}

#[test]
fn render_errors_instead_of_silent_gaps() {
    let manual = pkg("[caps]\nspindle = \"manual\"\n");
    let e = manual.render(&Program { meta: Meta::default(), events: vec![Event::SpindleOn { rpm: 1000.0 }] }, &ctx()).unwrap_err();
    assert!(e.message.contains("run by hand"), "{e}");
    let e = manual.render(&Program { meta: Meta::default(), events: vec![Event::SpindleOff] }, &ctx()).unwrap_err();
    assert!(e.message.contains("run by hand"), "{e}");
    // a stop, a spindle command or a tool changer's change that writes nothing
    for (tpl, ev) in [("?spindle_on: M3 {S}\n?spindle_off: M5", stop("x")), ("?stop: M0\n?spindle_off: M5", Event::SpindleOn { rpm: 1.0 }), ("?stop: M0\n?spindle_on: M3", Event::SpindleOff)] {
        let p = pkg(&format!("[templates]\non_command = \"\"\"\n{tpl}\n\"\"\"\n"));
        let e = p.render(&Program { meta: Meta::default(), events: vec![ev.clone()] }, &ctx()).unwrap_err();
        assert!(e.message.contains("writes nothing for it"), "{ev:?}: {e}");
    }
    let atc = pkg("[caps]\ntool_change = \"atc\"\ncomments = \"none\"\n[templates]\non_section_tool = \"(Tool {tool.number})\"\n");
    let e = atc.render(&Program { meta: Meta::default(), events: vec![tool(1, "a")] }, &ctx()).unwrap_err();
    assert!(e.message.contains("changes the tool") && e.message.contains("on_section_tool"), "{e}");
    // a manual change may write nothing: the stop before it told the operator
    assert!(pkg("[caps]\ncomments = \"none\"\n").render(&Program { meta: Meta::default(), events: vec![tool(1, "a")] }, &ctx()).unwrap().lines.is_empty());
    // a line longer than the controller reads
    let short = pkg("[caps]\nmax_line_len = 12\n");
    let e = short.render(&Program { meta: Meta::default(), events: vec![c("a long comment line", CommentKind::Info)] }, &ctx()).unwrap_err();
    assert!(e.message.contains("max_line_len = 12"), "{e}");
}

// ---------- the built-ins: lint ----------

#[test]
fn every_builtin_loads_and_renders_the_test_program() {
    for d in Dialect::ALL {
        let p = builtin(d);
        // loading checked every template against the known words, variables, flags and properties
        let again = Package::load("again", d.source()).unwrap();
        assert_eq!((again.name(), again.caps()), (p.name(), p.caps()));
        let value = crate::model::DIALECTS.iter().find(|(v, _)| Dialect::from_post(v) == d).unwrap();
        assert_eq!(p.name(), value.1, "the post's name is the dialect's name");
        assert_eq!(p.file(), format!("builtin:{}/post.toml", d.key()));
        assert!(p.properties().is_empty());
        for units in [Units::Metric, Units::Imperial] {
            let mut prog = test_program(p.caps());
            prog.meta.units = units;
            let r = p.render(&prog, &ctx()).unwrap_or_else(|e| panic!("{d:?}: {e}"));
            assert!(r.notes.is_empty() && r.lines.len() == r.source.len() && r.lines.len() > prog.events.len() / 2, "{d:?}");
            assert!(r.lines.iter().any(|l| l.starts_with("G3 X")), "{d:?}: the arc");
            assert!(r.lines.iter().all(|l| l.is_ascii() && !l.trim().is_empty()), "{d:?}");
            assert_eq!(r.lines.iter().filter(|l| l.starts_with("M0 ")).count(), if p.caps().spindle == SpindleCtl::Manual { 5 } else if p.caps().tool_change == ToolChangeMode::Manual { 2 } else { 1 }, "{d:?}");
        }
    }
}

#[test]
fn the_builtins_capabilities() {
    let caps = |d| builtin(d).caps().clone();
    for d in [Dialect::Grbl, Dialect::GrblHal, Dialect::FluidNc] { assert_eq!(caps(d), Caps::default(), "{d:?}"); }
    assert_eq!(caps(Dialect::Mach), Caps { tool_change: ToolChangeMode::Atc, wcs_count: 9, ..Caps::default() });
    assert_eq!(caps(Dialect::Marlin), Caps { spindle: SpindleCtl::Manual, comments: CommentStyle::Semicolon, wcs_count: 1, ..Caps::default() });
    // the work offset as Job::wcs_name gave it for each dialect
    for (post, _) in crate::model::DIALECTS {
        for wcs in 0..10u8 {
            let job = crate::model::Job { post: post.into(), wcs, ..Default::default() };
            assert_eq!(for_job(&job).caps().wcs_name(wcs), job.wcs_name(), "{post} {wcs}");
        }
    }
    assert_eq!(caps(Dialect::Mach).wcs_name(6), "G59.1");
    assert_eq!(caps(Dialect::Grbl).wcs_name(6), "G54");
    assert_eq!(caps(Dialect::Marlin).wcs_name(2), "G54");
}

#[test]
fn dialects_are_read_like_the_job_reads_them() {
    assert_eq!(Dialect::from_post(""), Dialect::Grbl);
    assert_eq!(Dialect::from_post("something-else"), Dialect::Grbl);
    for (post, _) in crate::model::DIALECTS {
        let d = Dialect::from_post(post);
        let job = crate::model::Job { post: post.into(), ..Default::default() };
        assert_eq!((d == Dialect::Mach, d == Dialect::Marlin), (job.mach(), job.marlin()), "{post}");
        assert_eq!(d.key(), if post.is_empty() { "grbl" } else { post });
    }
}

// ---------- the TX-35 manifest of the spec ----------

/// The TX-35 manifest as `docs/post-processor-spec.md` shows it.
fn tx35_from_spec() -> String {
    let spec = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/post-processor-spec.md")).unwrap();
    let at = spec.find("**The TX-35 manifest as an example**").expect("the spec has the TX-35 example");
    let block = &spec[at..];
    let start = block.find("```").unwrap();
    let body = &block[start..];
    let body = &body[body.find('\n').unwrap() + 1..];
    body[..body.find("```").unwrap()].to_string()
}

#[test]
fn the_tx35_manifest_of_the_spec_loads_and_renders() {
    let src = tx35_from_spec();
    let p = Package::load("docs/post-processor-spec.md", &src).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((p.name(), p.extension(), p.line_ending()), ("TX-35 ATC", "nc", LineEnding::Crlf));
    let c = p.caps();
    assert_eq!((c.arcs, c.tool_change, c.length_offset, c.spindle, c.dwell, c.comments), (ArcSupport::Xy, ToolChangeMode::Atc, true, SpindleCtl::Controlled, DwellUnit::None, CommentStyle::None));
    assert_eq!(p.properties().iter().map(|p| (p.key.as_str(), p.default.clone())).collect::<Vec<_>>(),
               vec![("preselect_next_tool", PropValue::Bool(false)), ("stop_spindle_before_tool_change", PropValue::Bool(true))]);
    let t = render_with(&p, &test_program(c)).lines;
    // the VCarve header, tool change and footer, padded numbers, modal G1 and F, no comments, no dwell
    assert_eq!(&t[..5], &["G0 G28 G91", "G54", "G17 G80 G49 G0 G40 G90", "G53 G0 Z0", "G21"]);
    let i = t.iter().position(|l| l == "M06T1").unwrap();
    assert_eq!(&t[i - 1..i + 3], &["M5".to_string(), "M06T1".into(), "G43H1".into(), "S18000M03".into()]);
    for l in ["G1Z-1.000F250.0", "X20.000F800.0", "X30.000Y10.000", "G3X20.000Y20.000I-10.000J0.000", "M06T2", "G43H2", "S12000M03", "G0X0.000Y0.000", "M30"] {
        assert!(t.contains(&l.to_string()), "{l} in {t:#?}");
    }
    assert!(t.iter().all(|l| !l.contains('(') && !l.starts_with("G4 ")), "{t:#?}");
}

/// The text of whole programs from `toolpath::generate`, dialect by dialect (in toolpath.rs before the IR, in the canonical renderer in phase 1).
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
