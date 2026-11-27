//! How long `generate()` takes, for comparing before and after a change to the emitter or the renderer
//! (docs/implementation-log.md has the numbers of phase 1).
//!
//! `cargo run --release --example gen_time [arcs]`: the sample-shaped-relief golden case (GRBL, metric, the fixed golden machine).
//! `BUSY=60` instead: the pocket of 60 circles of gui/src/perf.rs (about a million moves). `N=` runs (default 7). Prints the
//! time of each run, of cloning the output and of joining its text, then the minimum, median and maximum.
use putiacnc_core::model::Machine;
use putiacnc_core::sample::{fill_kind, Kind};
use putiacnc_core::{generate, shapes, Units};
use std::time::Instant;

fn main() {
    let units = Units::Metric;
    let mut f = shapes::blank_project();
    for (i, t) in f.data.tools.iter_mut().enumerate() { t.id = format!("t{:02}", i + 1); }
    let m = Machine { id: "golden".into(), name: "Golden test machine".into(), rapid: 2000.0, tsc: 1.2, x: 300.0, y: 200.0, z: 60.0,
        post: Some(String::new()), numbered: Some(false), safe: Some(5.0), inches: Some(false), arcs: Some(false), spin_up: Some(2.0), ..Machine::default() };
    f.data.job.safe = 5.0;
    f.data.machine = Some(m);
    if let Some(n) = std::env::var("BUSY").ok().and_then(|v| v.parse::<usize>().ok()) {
        // gui/src/perf.rs busy_project: a pocket of n circles, 1 mm tool, 20 % stepover, 0.5 mm passes
        let d = &mut f.data;
        d.stock.w = 300.0; d.stock.h = 200.0; d.stock.t = 12.0;
        d.tools[0].dia = 1.0; d.tools[0].so = 20.0; d.tools[0].pd = 0.5;
        let tool = d.tools[0].id.clone();
        let mut p = shapes::default_op_params(&tool);
        p.kind = "pocket".into(); p.depth = 6.0;
        d.ops.push(putiacnc_core::model::Operation { id: 1, p });
        d.loops.clear();
        for i in 0..n { let (col, row) = (i % 18, i / 18); d.loops.push(shapes::new_circle([14.0 + col as f64 * 16.0, 14.0 + row as f64 * 16.0], 7.0, 1)); }
    } else {
        fill_kind(&mut f.data, false, Kind::ShapedRelief).unwrap();
    }
    let arcs = std::env::args().nth(1).as_deref() == Some("arcs");
    if arcs { f.data.machine.as_mut().unwrap().arcs = Some(true); }
    let n: usize = std::env::var("N").ok().and_then(|v| v.parse().ok()).unwrap_or(7);
    let mut ts = vec![];
    let mut info = (0, 0);
    for _ in 0..n {
        let t = Instant::now();
        let o = generate(&f.data, units).unwrap();
        ts.push(t.elapsed().as_secs_f64());
        info = (o.lines.len(), o.moves.len());
        let tg = Instant::now();
        let txt = o.gcode();
        let tgc = tg.elapsed().as_secs_f64();
        std::hint::black_box(&txt);
        let t2 = Instant::now();
        let c = o.clone();
        let tc = t2.elapsed().as_secs_f64();
        std::hint::black_box(&c);
        eprintln!("generate {:.3} s, clone {:.4} s, gcode() {:.4} s", ts.last().unwrap(), tc, tgc);
    }
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("lines {} moves {} arcs {} runs {}: min {:.3} s median {:.3} s max {:.3} s", info.0, info.1, arcs, n, ts[0], ts[n / 2], ts[n - 1]);
}
