//! The arc fitter on the IR against the fitter on text, on the golden programs.
//!
//! Since phase 2 the post engine fits arcs on the program's events: `arcs::fit_items` on the numbers the post writes (output units,
//! rounded), before it renders. Before, the generator (and in phase 1 the canonical renderer) fitted them on its finished text with
//! `arcs::fit_arcs`, then turned Marlin's `(…)` comments into `; …`. This test runs that old pipeline on the text of each program
//! written without arcs and compares it with what the engine writes, for the projects of tests/golden (the example project, every
//! sample, the snapshot projects and the trochoidal case), in GRBL, Mach3 / LinuxCNC and Marlin (grblHAL and FluidNC are GRBL's
//! package with another name), metric and imperial: over the whole program (arcs on) and over the trochoidal ranges alone (arcs
//! off).
use putiacnc_core::model::{Machine, ProjectData};
use putiacnc_core::post::{builtin, text, CommentStyle, Dialect, Package, Post, PostCtx, Rendered};
use putiacnc_core::program::Program;
use putiacnc_core::sample::{fill_kind, Kind};
use putiacnc_core::{arcs, generate, load_project, shapes, Units};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

/// The golden matrix's fixed machine, in a dialect.
fn on_machine(d: &ProjectData, post: &str, units: Units) -> ProjectData {
    let mut d = d.clone();
    let m = Machine { id: "golden".into(), name: "Golden test machine".into(), rapid: 2000.0, tsc: 1.2, x: 300.0, y: 200.0, z: 60.0, post: Some(post.into()),
        numbered: Some(false), safe: Some(5.0), inches: Some(units == Units::Imperial), arcs: Some(false), spin_up: Some(2.0), ..Machine::default() };
    d.job.post = post.into();
    d.job.numbered = false;
    d.job.safe = 5.0;
    d.machine = Some(m);
    d
}

fn load(path: &str) -> ProjectData { load_project(&std::fs::read_to_string(root().join(path)).unwrap()).unwrap().data }

/// Makes a project for the units (the samples are made in the units they are shown in).
type Make = Box<dyn Fn(Units) -> ProjectData>;

/// The projects of tests/golden.
fn projects() -> Vec<(String, Make)> {
    let mut v: Vec<(String, Make)> = vec![("oval-sign".into(), Box::new(|_| load("examples/projects/oval-sign.project.json")))];
    for k in [Kind::NamePlate, Kind::Box, Kind::Relief, Kind::ShapedRelief, Kind::Oval] {
        v.push((format!("sample {k:?}"), Box::new(move |u| {
            let mut f = shapes::blank_project();
            for (i, t) in f.data.tools.iter_mut().enumerate() { t.id = format!("t{:02}", i + 1); }
            f.data = on_machine(&f.data, "", u);
            fill_kind(&mut f.data, u == Units::Imperial, k).unwrap();
            f.data
        })));
    }
    for n in ["outside_ramp", "outside_holes", "pocket_island", "vcarve_circle", "tabs", "vbit_profile", "two_ops_toolchange", "imperial_center", "hardware_m0_aircut", "hardware_pen_test"] {
        v.push((n.into(), Box::new(move |_| load(&format!("tests/data/{n}.project.json")))));
    }
    v.push(("trochoidal".into(), Box::new(|_| {
        let mut d = load("tests/data/pocket_island.project.json");
        for o in d.ops.iter_mut() { o.p.troch = true; o.p.ramp = false; o.p.depth = 1.5; o.p.pass_depth = Some(1.5); }
        d
    })));
    v
}

fn render(p: &Package, prog: &Program) -> Rendered {
    let (m, props) = (Machine::default(), BTreeMap::new());
    p.render(prog, &PostCtx { machine: &m, props: &props, safe_z: 5.0 }).unwrap()
}

/// The first line that comes from event `ev` or a later one (as the phase 1 renderer found a range's lines).
fn first_line(source: &[Option<usize>], ev: usize) -> usize { source.partition_point(|s| s.is_some_and(|e| e < ev)) }

#[test]
fn the_ir_fitter_writes_what_the_text_fitter_wrote() {
    let (mut whole, mut troch) = (0, 0);
    for (name, make) in projects() {
        for d in [Dialect::Grbl, Dialect::Mach, Dialect::Marlin] {
            let post = builtin(d);
            // the text the old fitter read had its comments in brackets: Marlin's were turned into ; after the fitting
            let paren = Package::load("paren", &d.source().replace("comments = \"semicolon\"", "comments = \"paren\"")).unwrap();
            let after = |lines: Vec<String>| -> Vec<String> {
                if post.caps().comments == CommentStyle::Semicolon { lines.iter().map(|l| text::machine_line(CommentStyle::Semicolon, l).unwrap()).collect() } else { lines }
            };
            for units in [Units::Metric, Units::Imperial] {
                let data = on_machine(&make(units), d.key().replace("grbl", "").as_str(), units);
                let p = generate(&data, units).unwrap_or_else(|e| panic!("{name}: {e}")).program;
                let (dec, k) = (if units == Units::Imperial { 4 } else { 3 }, units.factor());
                let mut plain = p.clone();
                plain.meta.arcs = false;
                plain.meta.troch.clear();
                let base = render(&paren, &plain);
                // arcs on: the whole program
                let mut on = p.clone();
                on.meta.arcs = true;
                let (fitted, n) = arcs::fit_arcs(&base.lines, dec, k);
                let r = render(post, &on);
                assert_eq!(r.lines, after(fitted), "{name}, {d:?}, {units:?}, arcs on");
                assert_eq!(r.arcs, n);
                whole += n;
                // arcs off: the trochoidal ranges only, from the back, as the phase 1 renderer did
                let (mut lines, mut source) = (base.lines.clone(), base.source.clone());
                let mut nt = 0;
                for &(a, b) in p.meta.troch.iter().rev() {
                    let (la, lb) = (first_line(&source, a), first_line(&source, b));
                    let (f, from, n) = arcs::fit_arcs_mapped(&lines[la..lb], dec, k);
                    if n > 0 {
                        let src: Vec<Option<usize>> = from.iter().map(|&j| source[la + j]).collect();
                        lines.splice(la..lb, f);
                        source.splice(la..lb, src);
                        nt += n;
                    }
                }
                let r = render(post, &p);
                assert_eq!(r.lines, after(lines), "{name}, {d:?}, {units:?}, trochoidal ranges");
                assert_eq!((r.troch_arcs, r.source), (nt, source), "{name}, {d:?}, {units:?}");
                troch += nt;
            }
        }
    }
    eprintln!("{whole} arcs over whole programs, {troch} in trochoidal ranges");
    // the comparison saw arcs of both kinds
    assert!(whole > 1000 && troch > 100, "{whole} arcs over whole programs, {troch} in trochoidal ranges");
}
