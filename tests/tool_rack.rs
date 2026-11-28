//! Phase 3 of docs/post-processor-spec.md: tool numbers from the machine's tool rack, the tool or the order of first use; the new
//! fields in old files; and the read-back checks that stop an export (the motion cross-check and the tool-changer checks), with test
//! posts that must fail.
use putiacnc_core::machines::Machines;
use putiacnc_core::model::{ProjectData, ToolSlot, Units};
use putiacnc_core::post::{builtin, Caps, Dialect, Package, Post, PostCtx, PostError, PropDef, PropValue, Rendered};
use putiacnc_core::program::{Event, NumberFrom, Program};
use putiacnc_core::toollib::ToolLibs;
use putiacnc_core::verify::ReadBack;
use putiacnc_core::{generate, generate_with, load_project, save_project, Output};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")) }
fn load(name: &str) -> ProjectData { load_project(&fs::read_to_string(root().join("tests/data").join(format!("{name}.project.json"))).unwrap()).unwrap().data }
/// The two-tool project with its machine's rack: the 1/8" end mill in slot 5, the 6 mm end mill in slot 7 (Mach3 / LinuxCNC).
fn rack() -> ProjectData { load("rack_two_tools") }
const EIGHTH: &str = "1/8\u{2033} flat end mill";
const SIX: &str = "6 mm flat end mill";
fn id(d: &ProjectData, name: &str) -> String { d.tools.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("no {name}")).id.clone() }
fn tool_refs(o: &Output) -> Vec<(String, u16, u16, NumberFrom)> {
    o.program.events.iter().filter_map(|e| match e { Event::ToolChange { tool } => Some((tool.name.clone(), tool.number, tool.offset(), tool.from)), _ => None }).collect()
}
fn no_props() -> BTreeMap<String, PropValue> { BTreeMap::new() }

// ---------- old files ----------

#[test]
fn old_projects_load_and_save_without_the_new_fields() {
    let mut files: Vec<PathBuf> = fs::read_dir(root().join("tests/data")).unwrap().flatten().map(|e| e.path()).collect();
    files.extend(fs::read_dir(root().join("examples/projects")).unwrap().flatten().map(|e| e.path()));
    let mut n = 0;
    for f in files.iter().filter(|f| f.to_string_lossy().ends_with(".project.json") && !f.to_string_lossy().contains("rack_")) {
        let p = load_project(&fs::read_to_string(f).unwrap()).unwrap();
        let saved = save_project(&p).unwrap();
        for key in ["\"toolTable\"", "\"number\"", "\"length_offset\"", "\"tool_id\""] { assert!(!saved.contains(key), "{}: {key}", f.display()); }
        // saved again, the same bytes
        assert_eq!(save_project(&load_project(&saved).unwrap()).unwrap(), saved, "{}", f.display());
        n += 1;
    }
    assert!(n >= 12, "{n} projects");
}

#[test]
fn an_old_machine_list_and_tool_library_save_byte_for_byte() {
    let dir = root().join("tests/fixtures/old-format");
    let m = fs::read_to_string(dir.join("machines.json")).unwrap();
    assert_eq!(Machines::from_json(&m).unwrap().to_json(), m);
    let t = fs::read_to_string(dir.join("toollibs.json")).unwrap();
    assert_eq!(ToolLibs::from_json(&t).unwrap().to_json(), t);
}

#[test]
fn the_new_fields_are_saved_under_their_names_and_load_back() {
    let mut d = rack();
    let six = id(&d, SIX);
    d.tools.iter_mut().find(|t| t.id == six).unwrap().number = Some(3);
    d.tools.iter_mut().find(|t| t.id == six).unwrap().length_offset = Some(13);
    d.machine.as_mut().unwrap().tool_table.push(ToolSlot { tool_id: "x".into(), number: 9, length_offset: Some(19) });
    let mut f = load_project(&fs::read_to_string(root().join("tests/data/rack_two_tools.project.json")).unwrap()).unwrap();
    f.data = d.clone();
    let s = save_project(&f).unwrap();
    assert!(s.contains("\"number\":3,\"length_offset\":13"), "{s}");
    assert!(s.contains("\"toolTable\":[{\"tool_id\":\"tmve317ucph7\",\"number\":5},{\"tool_id\":\"tmve317ucjjb\",\"number\":7},{\"tool_id\":\"x\",\"number\":9,\"length_offset\":19}]"), "{s}");
    assert_eq!(load_project(&s).unwrap().data, d);
    // a machine list keeps the rack too
    let mut ms = Machines::default();
    ms.list[0].tool_table = d.machine.unwrap().tool_table;
    assert_eq!(Machines::from_json(&ms.to_json()).unwrap().list[0].tool_table.len(), 3);
}

// ---------- numbers in the built-in posts ----------

#[test]
fn rack_slots_are_the_t_numbers_of_the_mach3_text() {
    let o = generate(&rack(), Units::Metric).unwrap();
    assert_eq!(tool_refs(&o), vec![(EIGHTH.into(), 5, 5, NumberFrom::Rack), (SIX.into(), 7, 7, NumberFrom::Rack)]);
    let t: Vec<&String> = o.lines.iter().filter(|l| l.contains(" M6")).collect();
    assert_eq!(t, ["T5 M6 (Tool 5: 1/8in flat end mill)", "T7 M6 (Tool 7: 6 mm flat end mill)"]);
    assert!(o.notes.is_empty(), "{:?}", o.notes);
}

#[test]
fn the_tools_own_number_comes_after_the_rack_and_first_use_skips_it() {
    let mut d = rack();
    let (eighth, six) = (id(&d, EIGHTH), id(&d, SIX));
    // the rack wins over the tool's own number
    d.tools.iter_mut().find(|t| t.id == six).unwrap().number = Some(1);
    assert_eq!(tool_refs(&generate(&d, Units::Metric).unwrap())[1].1, 7);
    // without the rack: the 6 mm has 1 of its own, so the 1/8" (used first) skips 1
    d.machine.as_mut().unwrap().tool_table.clear();
    let o = generate(&d, Units::Metric).unwrap();
    assert_eq!(tool_refs(&o), vec![(EIGHTH.into(), 2, 2, NumberFrom::FirstUse), (SIX.into(), 1, 1, NumberFrom::Tool)]);
    // an offset of its own
    d.tools.iter_mut().find(|t| t.id == eighth).unwrap().length_offset = Some(12);
    assert_eq!(tool_refs(&generate(&d, Units::Metric).unwrap())[0], (EIGHTH.into(), 2, 12, NumberFrom::FirstUse));
}

#[test]
fn a_post_that_writes_no_numbers_resolves_them_and_writes_the_same_text() {
    for post in ["", "marlin"] {
        let mut d = rack();
        d.job.post = post.into();
        let with = generate(&d, Units::Metric).unwrap();
        assert_eq!(tool_refs(&with).iter().map(|t| t.1).collect::<Vec<_>>(), [5, 7]);
        d.machine.as_mut().unwrap().tool_table.clear();
        let without = generate(&d, Units::Metric).unwrap();
        assert_eq!(tool_refs(&without).iter().map(|t| t.1).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(with.lines, without.lines, "{post}: the text does not show tool numbers");
    }
}

#[test]
fn two_tools_in_one_slot_stop_the_export_with_every_post() {
    for post in ["", "grblhal", "fluidnc", "mach", "marlin"] {
        let mut d = rack();
        d.job.post = post.into();
        d.machine.as_mut().unwrap().tool_table[1].number = 5;
        let e = generate(&d, Units::Metric).unwrap_err();
        assert!(e.contains("T5 (1/8\u{2033} flat end mill, 6 mm flat end mill)"), "{post}: {e}");
    }
}

// ---------- a tool-changer test package: rack slots required, G43 H# after every change ----------

/// A TX-35-like package: required numbers, `G43 H#`, no spaces, modal words, padded numbers, no comments, M5 before M06 by default.
fn atc_src(section_tool: &str, command: &str) -> String {
    format!(r#"[post]
format_version = 1
name = "Test ATC"
[caps]
tool_change = "atc"
tool_numbers = "required"
length_offset = true
dwell = "none"
comments = "none"
[properties]
stop_spindle_before_tool_change = {{ type = "bool", default = true, title = "M5 before M06" }}
[format]
xyz = {{ decimals = 3, trim = false }}
feed = {{ decimals = 1, trim = false }}
rpm = {{ decimals = 0 }}
[words]
X = {{ format = "xyz", modal = true }}
Y = {{ format = "xyz", modal = true }}
Z = {{ format = "xyz", modal = true }}
I = {{ format = "xyz", incremental = true }}
J = {{ format = "xyz", incremental = true }}
F = {{ format = "feed", modal = true }}
S = {{ format = "rpm" }}
[modal]
motion = {{ codes = ["G0", "G1", "G2", "G3"], always = ["G0", "G2", "G3"] }}
[templates]
on_open = """
{{units}} G90 G17
{{wcs}}
"""
on_section_tool = """
{section_tool}
"""
on_rapid = "{{G}}{{X}}{{Y}}{{Z}}"
on_linear = "{{G}}{{X}}{{Y}}{{Z}}{{F}}"
on_circular = "{{G}}{{X}}{{Y}}{{I}}{{J}}{{F}}"
on_command = """
{command}
"""
on_close = """
G0{{Z:park}}
G0{{X:park}}{{Y:park}}
M30
"""
"#)
}
const TOOL: &str = "M06T{tool.number}\nG43H{tool.offset}";
const COMMAND: &str = "?spindle_on: {S}M03\n?spindle_off: M5\n?stop: M0\n?optional_stop: M1";
fn atc(section_tool: &str, command: &str) -> Package { Package::load("test-atc/post.toml", &atc_src(section_tool, command)).unwrap_or_else(|e| panic!("{e}")) }

#[test]
fn the_atc_package_writes_rack_slots_and_length_offsets_and_passes_its_checks() {
    let p = atc(TOOL, COMMAND);
    let mut d = rack();
    d.machine.as_mut().unwrap().tool_table[1].length_offset = Some(17);
    for arcs in [false, true] {
        d.machine.as_mut().unwrap().arcs = Some(arcs);
        let o = generate_with(&d, Units::Metric, &p, &no_props()).unwrap_or_else(|e| panic!("arcs {arcs}: {e}"));
        let at = |s: &str| o.lines.iter().position(|l| l == s).unwrap_or_else(|| panic!("no {s}"));
        assert!(at("M06T5") + 1 == at("G43H5") && at("M06T7") + 1 == at("G43H17"), "{:?}", &o.lines[..20]);
        assert!(at("M5") < at("M06T7"));
        assert!(o.lines.iter().any(|l| l.starts_with('X') || l.starts_with('Y')), "modal moves without a G");
        if arcs { assert!(o.lines.iter().any(|l| l.starts_with("G2") || l.starts_with("G3"))); }
        // the program has the numbers and offsets the text writes
        assert_eq!(tool_refs(&o).iter().map(|t| (t.1, t.2)).collect::<Vec<_>>(), [(5, 5), (7, 17)]);
        // a TX-35-style post writes G43 itself: no warning about length offsets
        assert!(!o.notes.iter().any(|n| n.contains("length offsets")), "{:?}", o.notes);
    }
}

#[test]
fn a_tool_missing_from_the_rack_stops_the_export_naming_it() {
    let mut d = rack();
    d.machine.as_mut().unwrap().tool_table.remove(1);
    let e = generate_with(&d, Units::Metric, &atc(TOOL, COMMAND), &no_props()).unwrap_err();
    assert!(e.contains("these have none: 6 mm flat end mill.") && !e.contains("1/8"), "{e}");
    // both missing: both named
    d.machine.as_mut().unwrap().tool_table.clear();
    let e = generate_with(&d, Units::Metric, &atc(TOOL, COMMAND), &no_props()).unwrap_err();
    assert!(e.contains("1/8\u{2033} flat end mill, 6 mm flat end mill"), "{e}");
    // a number on the tool itself is enough
    let six = id(&d, SIX);
    let eighth = id(&d, EIGHTH);
    for t in d.tools.iter_mut() { t.number = if t.id == six { Some(2) } else if t.id == eighth { Some(1) } else { None }; }
    assert!(generate_with(&d, Units::Metric, &atc(TOOL, COMMAND), &no_props()).is_ok());
}

#[test]
fn two_tools_on_one_slot_stop_the_atc_export() {
    let mut d = rack();
    d.machine.as_mut().unwrap().tool_table[0].number = 7;
    let e = generate_with(&d, Units::Metric, &atc(TOOL, COMMAND), &no_props()).unwrap_err();
    assert!(e.contains("T7 (1/8\u{2033} flat end mill, 6 mm flat end mill)"), "{e}");
}

#[test]
fn an_atc_post_without_length_offsets_gets_a_note_only_when_it_requires_numbers() {
    let src = atc_src("M06T{tool.number}", COMMAND).replace("length_offset = true", "length_offset = false");
    let o = generate_with(&rack(), Units::Metric, &Package::load("t", &src).unwrap(), &no_props()).unwrap();
    assert!(o.notes.iter().any(|n| n.contains("relies on the controller's own tool length handling")), "{:?}", o.notes);
    let o = generate_with(&rack(), Units::Metric, &Package::load("t", &src.replace("\"required\"", "\"first_use\"")).unwrap(), &no_props()).unwrap();
    assert!(o.notes.is_empty(), "{:?}", o.notes);
    // the built-in Mach3 / LinuxCNC (first use, no G43) never gets it
    let mut d = rack();
    d.machine.as_mut().unwrap().tool_table.clear();
    assert!(generate(&d, Units::Metric).unwrap().notes.is_empty());
}

// ---------- test posts that must fail ----------

#[test]
fn a_post_that_drops_g43_fails() {
    let e = generate_with(&rack(), Units::Metric, &atc("M06T{tool.number}", COMMAND), &no_props()).unwrap_err();
    assert!(e.contains("G-code line") && e.contains("with no G43 H5 (length offset)"), "{e}");
}

#[test]
fn a_post_that_writes_the_wrong_t_fails() {
    let mut d = rack();
    d.machine.as_mut().unwrap().tool_table[1].length_offset = Some(17);
    let e = generate_with(&d, Units::Metric, &atc("M06T{tool.offset}\nG43H{tool.offset}", COMMAND), &no_props()).unwrap_err();
    assert!(e.contains("M06T17") || e.contains("T17 is not a tool of this program"), "{e}");
}

#[test]
fn a_post_that_changes_tools_with_the_spindle_running_fails_while_the_stop_property_is_on() {
    // the spindle stop writes a line that does not stop the spindle
    let command = "?spindle_on: {S}M03\n?spindle_off: G4 P0\n?stop: M0\n?optional_stop: M1";
    let p = atc(TOOL, command);
    // the package's default is on
    let e = generate_with(&rack(), Units::Metric, &p, &no_props()).unwrap_err();
    assert!(e.contains("M6 (tool change) while the spindle runs"), "{e}");
    // set off by the machine: it exports; set on directly in the properties of a package that has no default: it fails
    let off: BTreeMap<String, PropValue> = [("stop_spindle_before_tool_change".to_string(), PropValue::Bool(false))].into();
    assert!(generate_with(&rack(), Units::Metric, &p, &off).is_ok());
    let plain = Package::load("t", &atc_src(TOOL, command).replace("stop_spindle_before_tool_change = { type = \"bool\", default = true, title = \"M5 before M06\" }", "")).unwrap();
    assert!(generate_with(&rack(), Units::Metric, &plain, &no_props()).is_ok(), "no property, no check");
    let on: BTreeMap<String, PropValue> = [("stop_spindle_before_tool_change".to_string(), PropValue::Bool(true))].into();
    assert!(generate_with(&rack(), Units::Metric, &plain, &on).unwrap_err().contains("while the spindle runs"));
}

#[test]
fn a_post_that_writes_mm_numbers_in_an_inch_program_fails() {
    // the package's formats are not lengths (user posts are mm only), so in inches it writes mm numbers under G20
    let e = generate_with(&rack(), Units::Imperial, &atc(TOOL, COMMAND), &no_props()).unwrap_err();
    assert!(e.contains("G-code line") && e.contains("the post does not write the program's moves"), "{e}");
}

/// A post that renders with a built-in, then breaks the text.
struct Broken { inner: &'static Package, how: fn(&mut Vec<String>) }
impl Post for Broken {
    fn name(&self) -> &str { "Broken" }
    fn caps(&self) -> &Caps { self.inner.caps() }
    fn properties(&self) -> &[PropDef] { self.inner.properties() }
    fn render(&self, prog: &Program, ctx: &PostCtx) -> Result<Rendered, PostError> {
        let mut r = self.inner.render(prog, ctx)?;
        (self.how)(&mut r.lines);
        Ok(r)
    }
    fn read_back(&self, units: Units) -> ReadBack { self.inner.read_back(units) }
}

/// The first cut that gives X and Y, and the one after it that does too.
fn cuts_xy(lines: &[String]) -> (usize, usize) {
    let xy = |l: &String| l.starts_with("G1 X") && l.contains(" Y");
    let a = lines.iter().position(xy).unwrap();
    (a, a + 1 + lines[a + 1..].iter().position(xy).unwrap())
}

#[test]
fn a_post_that_exchanges_x_and_y_fails() {
    let swap: fn(&mut Vec<String>) = |lines| {
        for l in lines.iter_mut().filter(|l| l.starts_with("G1 ")) { *l = l.replace('X', "#").replace('Y', "X").replace('#', "Y"); }
    };
    for post in [Dialect::Grbl, Dialect::Mach] {
        let e = generate_with(&load("two_ops_toolchange"), Units::Metric, &Broken { inner: builtin(post), how: swap }, &no_props()).unwrap_err();
        assert!(e.starts_with("G-code line ") && e.contains("the program's next move is a cut to X"), "{post:?}: {e}");
    }
}

#[test]
fn a_post_that_swaps_two_moves_fails() {
    let swap: fn(&mut Vec<String>) = |lines| { let (a, b) = cuts_xy(lines); lines.swap(a, b); };
    let e = generate_with(&load("two_ops_toolchange"), Units::Metric, &Broken { inner: builtin(Dialect::Grbl), how: swap }, &no_props()).unwrap_err();
    assert!(e.starts_with("G-code line ") && e.contains("but the program's next move is a cut to"), "{e}");
}

#[test]
fn the_cross_check_reads_every_move_of_a_real_program() {
    // as a check that it is not idle: counting the text's moves the way the checker pairs them
    let o = generate(&load("two_ops_toolchange"), Units::Metric).unwrap();
    let moves = o.program.events.iter().filter(|e| matches!(e, Event::Rapid { .. } | Event::Feed { .. } | Event::ProgramEnd { park: Some(_) })).count();
    let mut rb = builtin(Dialect::Grbl).read_back(Units::Metric);
    rb.numbered = false;
    let tol = putiacnc_core::crosscheck::Tol::new(&rb, 1.0);
    let mut c = putiacnc_core::crosscheck::Checker::new(&o.program.events, &[], &rb, tol);
    putiacnc_core::verify::read(&o.lines, &rb, &mut |f| c.fact(f));
    let checked = c.checked;
    assert_eq!(c.finish(), Ok(()));
    // every move line but the feed-only `G1 F…` ones is a text move
    let text_moves = o.lines.iter().filter(|l| (l.starts_with("G0 ") || l.starts_with("G1 ")) && l.contains(['X', 'Y', 'Z'])).count();
    assert_eq!(checked, text_moves);
    assert!(checked > 400 && checked <= moves, "{checked} of {moves}");
}
