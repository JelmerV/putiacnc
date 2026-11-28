//! Lints a post package and prints the fixed test program it writes (docs/posts/authoring.md; phase 4a makes this
//! `putia-gcode --check-post`).
//!
//! `cargo run --example check_post -- <path/to/post.toml | builtin:grbl> [--imperial]`
//!
//! A package that does not load prints its error (file, line, fix) and exits with 1. One that loads prints its capabilities to
//! stderr and the test program (`post::test_program`: every kind of event, two tools, an arc, a stop) to stdout.
use putiacnc_core::model::Machine;
use putiacnc_core::post::{builtin, test_program, Dialect, Package, Post, PostCtx};
use putiacnc_core::Units;
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(what) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("Usage: check_post <path/to/post.toml | builtin:grbl|grblhal|fluidnc|mach|marlin> [--imperial]");
        std::process::exit(2);
    };
    let units = if args.iter().any(|a| a == "--imperial") { Units::Imperial } else { Units::Metric };
    let loaded;
    let post: &Package = match what.strip_prefix("builtin:") {
        Some(key) if Dialect::ALL.iter().any(|d| d.key() == key) => builtin(Dialect::from_post(key)),
        Some(key) => { eprintln!("no built-in post `{key}`: grbl, grblhal, fluidnc, mach, marlin"); std::process::exit(2) }
        None => {
            let src = std::fs::read_to_string(what).unwrap_or_else(|e| { eprintln!("Cannot read {what}: {e}"); std::process::exit(1) });
            loaded = Package::load(what, &src).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(1) });
            &loaded
        }
    };
    let c = post.caps();
    eprintln!("{}: arcs {:?} (at most {} degrees), tool change {:?}{}, spindle {:?}, dwell {:?}, comments {:?}, {} work offsets, .{} files",
        post.name(), c.arcs, c.max_arc_sweep, c.tool_change, if c.length_offset { " with G43 H" } else { "" }, c.spindle, c.dwell, c.comments, c.wcs_count, post.extension());
    for p in post.properties() { eprintln!("property {} = {} ({})", p.key, p.default, p.title); }
    let mut prog = test_program(c);
    prog.meta.units = units;
    let (m, props) = (Machine::default(), BTreeMap::new());
    match post.render(&prog, &PostCtx { machine: &m, props: &props, safe_z: 5.0 }) {
        Ok(r) => {
            for n in &r.notes { eprintln!("note: {n}"); }
            println!("{}", r.lines.join("\n"));
        }
        Err(e) => { eprintln!("{e}"); std::process::exit(1) }
    }
}
