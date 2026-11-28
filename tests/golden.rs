//! Golden tests: the generator's output for ten sample projects is compared with snapshots kept in tests/data.
//!
//! `NAME.project.json` is the input, `NAME.snapshot.nc` the reviewed G-code, `NAME.snapshot.json` holds the units and
//! the time estimate. When the generator changes on purpose, run
//! `UPDATE_SNAPSHOTS=1 cargo test --test golden`, look at `git diff tests/data` and commit what you accept.
//! The text is first compared byte for byte, which is fast and what normally happens. Only when that fails are
//! coordinates compared with a tolerance, because trig functions can differ in the last digits between platforms;
//! everything else (line count, words, feeds) must still match exactly.
//!
//! The `matrix` module below is the larger, byte-exact set in tests/golden/ (see tests/golden/README.md): `UPDATE_GOLDEN=1`.
use putiacnc_core::{generate, load_project, Units};
use std::fs;
use std::path::PathBuf;

fn data(name: &str) -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join(name) }

fn check(name: &str, units: Units) {
    let proj = load_project(&fs::read_to_string(data(&format!("{name}.project.json"))).unwrap()).unwrap();
    let out = generate(&proj.data, units).unwrap_or_else(|e| panic!("{name}: {e}"));
    let nc = data(&format!("{name}.snapshot.nc"));
    let meta = data(&format!("{name}.snapshot.json"));
    let got = out.lines.join("\n");
    if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        fs::write(&nc, &got).unwrap();
        let u = if units == Units::Imperial { "in" } else { "mm" };
        fs::write(&meta, format!("{{\"units\":\"{u}\",\"time_s\":{:.1}}}\n", out.time_min * 60.0)).unwrap();
        return;
    }
    let want = fs::read_to_string(&nc).unwrap_or_else(|_| panic!("{name}: no snapshot; run with UPDATE_SNAPSHOTS=1"));
    if got != want { compare_with_tolerance(name, units, &out.lines, &want); }
    let m: serde_json::Value = serde_json::from_str(&fs::read_to_string(&meta).unwrap()).unwrap();
    let want_s = m["time_s"].as_f64().unwrap();
    let got_s = out.time_min * 60.0;
    assert!((want_s - got_s).abs() <= 0.5 + want_s * 0.001, "{name}: time {got_s:.1}s vs {want_s:.1}s");
}

/// The fallback when the text is not byte-identical: same lines and words, XYZ on motion lines within rounding.
fn compare_with_tolerance(name: &str, units: Units, lines: &[String], want: &str) {
    let exp: Vec<&str> = want.split('\n').collect();
    assert_eq!(lines.len(), exp.len(), "{name}: line count");
    // 0.0011 mm: coordinates are printed with 3 decimals (4 in inches), so a last-digit rounding flip is the largest legitimate difference
    let tol = 0.0011 / units.factor();
    for (i, (a, b)) in exp.iter().zip(lines.iter()).enumerate() {
        let (ta, tb): (Vec<&str>, Vec<&str>) = (a.split(' ').collect(), b.split(' ').collect());
        assert_eq!(ta.len(), tb.len(), "{name} line {i}: `{a}` vs `{b}`");
        let motion = a.starts_with("G0") || a.starts_with("G1");
        for (x, y) in ta.iter().zip(tb.iter()) {
            if motion && x.chars().next().is_some_and(|c| "XYZ".contains(c)) {
                let (vx, vy): (f64, f64) = (x[1..].parse().unwrap(), y[1..].parse().unwrap());
                assert!((vx - vy).abs() <= tol, "{name} line {i}: `{a}` vs `{b}`");
            } else {
                assert_eq!(x, y, "{name} line {i}: `{a}` vs `{b}`");
            }
        }
    }
}

/// The fallback is only reached when the bytes differ, so it gets its own cases: a last-digit flip passes...
#[test] fn fallback_accepts_a_rounding_flip() {
    compare_with_tolerance("flip", Units::Metric, &["G21".into(), "G1 X10.001 Y5 F800".into()], "G21\nG1 X10.002 Y5 F800");
}
/// ...a real move change does not,
#[test] #[should_panic(expected = "line 1")] fn fallback_rejects_a_moved_point() {
    compare_with_tolerance("moved", Units::Metric, &["G21".into(), "G1 X10.000 Y5 F800".into()], "G21\nG1 X10.002 Y5 F800");
}
/// ...and nothing but coordinates gets a tolerance.
#[test] #[should_panic(expected = "line 0")] fn fallback_is_exact_outside_coordinates() {
    compare_with_tolerance("feed", Units::Metric, &["G1 X10 F801".into()], "G1 X10 F800");
}

#[test] fn outside_ramp() { check("outside_ramp", Units::Metric); }
#[test] fn outside_holes() { check("outside_holes", Units::Metric); }
#[test] fn pocket_island() { check("pocket_island", Units::Metric); }
#[test] fn vcarve_circle() { check("vcarve_circle", Units::Metric); }
#[test] fn tabs() { check("tabs", Units::Metric); }
#[test] fn vbit_profile() { check("vbit_profile", Units::Metric); }
#[test] fn two_ops_toolchange() { check("two_ops_toolchange", Units::Metric); }
#[test] fn imperial_center() { check("imperial_center", Units::Imperial); }

#[test]
fn rejects_other_files() {
    assert!(load_project("{\"app\":\"something-else\",\"data\":{\"stock\":{\"w\":1,\"h\":1,\"t\":1}}}").is_err());
    assert!(load_project("not json").is_err());
}

#[test]
fn vcarve_with_flat_tool_is_an_error() {
    let mut proj = load_project(&fs::read_to_string(data("vcarve_circle.project.json")).unwrap()).unwrap();
    for t in proj.data.tools.iter_mut() { t.shape = "flat".into(); }
    assert!(generate(&proj.data, Units::Metric).unwrap_err().contains("V-carve needs"));
}

/// The flat lap that levels the floor of a ramped profile is flagged, shares the last pass number, and only the last pass has one.
#[test] fn ramp_levelling_lap_is_flagged() {
    let proj = load_project(&fs::read_to_string(data("outside_ramp.project.json")).unwrap()).unwrap();
    let out = generate(&proj.data, Units::Metric).unwrap();
    let last = out.moves.iter().map(|m| m.pass).max().unwrap();
    let lap: Vec<_> = out.moves.iter().filter(|m| m.lap).collect();
    assert!(!lap.is_empty(), "a ramped profile has a levelling lap");
    assert!(lap.iter().all(|m| m.pass == last && m.cut && m.from[2] == m.to[2]), "the lap is flat and belongs to the last pass");
    assert!(out.moves.iter().filter(|m| m.pass < last).all(|m| !m.lap));
}

/// The tool starts at the XY zero, wherever it is: a centre zero must not start the first rapid from the stock's corner.
#[test] fn the_first_rapid_starts_from_the_xy_zero() {
    let mut proj = load_project(&fs::read_to_string(data("outside_ramp.project.json")).unwrap()).unwrap();
    proj.data.stock.zr = "c".into();
    let (w, h) = (proj.data.stock.w, proj.data.stock.h);
    let out = generate(&proj.data, Units::Metric).unwrap();
    let first = out.moves.iter().find(|m| m.from[0] != m.to[0] || m.from[1] != m.to[1]).unwrap();
    assert!((first.from[0] - w / 2.0).abs() < 1e-9 && (first.from[1] - h / 2.0).abs() < 1e-9, "{:?}", first.from);
    proj.data.stock.zr = "ll".into();
    let out = generate(&proj.data, Units::Metric).unwrap();
    let first = out.moves.iter().find(|m| m.from[0] != m.to[0] || m.from[1] != m.to[1]).unwrap();
    assert_eq!([first.from[0], first.from[1]], [0.0, 0.0]);
}

// ================================================================================================================
// The golden matrix (phase 0 of docs/post-processor-spec.md): the full output of every example project and every
// built-in sample, in every built-in dialect, both unit systems, arcs off and on, as one file and split per tool.
// Files live in tests/golden/<project>/; tests/golden/README.md explains them. `UPDATE_GOLDEN=1` rewrites them.
// ================================================================================================================
mod matrix {
    use putiacnc_core::model::{Machine, Operation, PlugPiece, ProjectData, Relief, DIALECTS};
    use putiacnc_core::pieces::{default_piece, file_suffix, generate_parts};
    use putiacnc_core::sample::{fill_kind, Kind};
    use putiacnc_core::split::{generate_files, groups, Split};
    use putiacnc_core::{load_project, setup, shapes, Units};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// A file over this size is stored as a digest (SHA-256, size, first and last lines) instead of in full.
    const BIG: usize = 1 << 20;
    /// Lines kept at each end of a digest.
    const ENDS: usize = 200;

    fn root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")) }
    fn golden_dir() -> PathBuf { root().join("tests").join("golden") }
    fn updating() -> bool { std::env::var_os("UPDATE_GOLDEN").is_some_and(|v| !v.is_empty() && v != "0") }

    /// The settings of the machine and job that the matrix keeps fixed and the option cases (below) change.
    #[derive(Clone, Copy)]
    struct Opts {
        /// Line numbers (`Machine.numbered`, which the app copies to `job.numbered`).
        numbered: bool,
        /// The machine's own start and end lines (`Machine.start_code` / `end_code`).
        start: Option<&'static str>,
        end: Option<&'static str>,
        /// The work coordinate system (`job.wcs`); `None` keeps the project's.
        wcs: Option<u8>,
        /// Seconds the spindle gets to come up to speed (`Machine.spin_up`).
        spin_up: f64,
    }

    /// The fixed machine of the matrix.
    const PLAIN: Opts = Opts { numbered: false, start: None, end: None, wcs: None, spin_up: 2.0 };

    /// The machine every case runs on, so nothing depends on the local setup: the app's default machine with fixed
    /// values. Dialect, units and arcs are the case's; `o` is `PLAIN` except for the option cases.
    fn machine(post: &str, units: Units, arcs: bool, o: &Opts) -> Machine {
        Machine {
            id: "golden".into(), name: "Golden test machine".into(), rapid: 2000.0, tsc: 1.2, x: 300.0, y: 200.0, z: 60.0,
            post: Some(post.into()), numbered: Some(o.numbered), safe: Some(5.0), inches: Some(units == Units::Imperial), arcs: Some(arcs), spin_up: Some(o.spin_up),
            start_code: o.start.map(String::from), end_code: o.end.map(String::from),
            ..Machine::default()
        }
    }

    /// Puts the case's machine on a project the way the app does when a machine is chosen (`apply_machine_job` in
    /// gui/src/machmgr.rs): the machine decides the dialect, the line numbers and the safe Z.
    fn on_machine(d: &ProjectData, post: &str, units: Units, arcs: bool, o: &Opts) -> ProjectData {
        let mut d = d.clone();
        let mut m = machine(post, units, arcs, o);
        // the project's tool rack stays: it belongs to the machine the project was made on (only the rack cases have one)
        if let Some(own) = &d.machine { m.tool_table = own.tool_table.clone(); }
        d.job.post = m.post.clone().unwrap_or_default();
        d.job.numbered = m.numbered.unwrap_or(false);
        d.job.safe = m.safe.unwrap_or(5.0);
        if let Some(w) = o.wcs { d.job.wcs = w; }
        d.machine = Some(m);
        d
    }

    /// The directory name of a built-in sample. No wildcard: a new sample kind must be added here (and so get goldens).
    fn sample_slug(k: Kind) -> &'static str {
        match k {
            Kind::NamePlate => "sample-name-plate",
            Kind::Box => "sample-box",
            Kind::Relief => "sample-relief",
            Kind::ShapedRelief => "sample-shaped-relief",
            Kind::Oval => "sample-oval",
        }
    }

    /// A built-in sample as Generate → Sample projects makes it: a new project with the bundled tools, on the fixed machine,
    /// in the units of the case (the app makes the sample in the units it shows). Tool ids are made from the clock, so they
    /// are replaced with fixed ones; the G-code never shows them, but the project is then the same on every run.
    fn sample(k: Kind, units: Units) -> ProjectData {
        let mut f = shapes::blank_project();
        for (i, t) in f.data.tools.iter_mut().enumerate() { t.id = format!("t{:02}", i + 1); }
        f.data = on_machine(&f.data, "", units, false, &PLAIN);
        fill_kind(&mut f.data, units == Units::Imperial, k).unwrap_or_else(|e| panic!("{}: {e}", sample_slug(k)));
        f.data
    }

    fn dialect_slug(post: &str) -> &str { if post.is_empty() { "grbl" } else { post } }
    fn units_slug(u: Units) -> &'static str { if u == Units::Imperial { "imperial" } else { "metric" } }

    /// What one case writes: (file name, contents) for the G-code and its notes.
    fn case_files(d: &ProjectData, post: &str, units: Units, arcs: bool, o: &Opts) -> Vec<(String, String)> {
        let d = on_machine(d, post, units, arcs, o);
        let base = format!("{}-{}-{}", dialect_slug(post), units_slug(units), if arcs { "arcs" } else { "noarcs" });
        let notes = |n: &[String]| n.iter().map(|x| format!("{x}\n")).collect::<String>();
        let mut out = vec![];
        // one file: what `putia-gcode -o` and File → Export G-code write (the main job, then one file per plug piece)
        let parts = generate_parts(&d, units).unwrap_or_else(|e| panic!("{base}: {e}"));
        for (i, p) in parts.iter().enumerate() {
            let name = format!("{base}{}", file_suffix(&parts, i));
            out.push((format!("{name}.nc"), p.out.gcode()));
            out.push((format!("{name}.notes.txt"), notes(&p.out.notes)));
        }
        // per tool: the export window's "a file per tool", only where it makes more than one file
        if groups(&d, Split::PerTool).len() > 1 {
            let files = generate_files(&d, units, Split::PerTool).unwrap_or_else(|e| panic!("{base} per tool: {e}"));
            for (i, f) in files.iter().enumerate() {
                let name = format!("{base}-tool-{}", i + 1);
                out.push((format!("{name}.nc"), f.out.gcode()));
                out.push((format!("{name}.notes.txt"), notes(&f.out.notes)));
            }
        }
        out
    }

    /// Every case of one project, in the cross product dialect x units x arcs. `make` builds the project for the units.
    fn project_files(make: &dyn Fn(Units) -> ProjectData) -> Vec<(String, String)> {
        files_for(make, &PLAIN, &[Units::Metric, Units::Imperial], &[false, true])
    }

    /// The files of one project in every dialect, for the given units and arcs settings, on the machine with options `o`.
    fn files_for(make: &dyn Fn(Units) -> ProjectData, o: &Opts, units: &[Units], arcs: &[bool]) -> Vec<(String, String)> {
        files_in(make, o, units, arcs, &DIALECTS.map(|d| d.0))
    }

    /// `files_for` in the given dialects only.
    fn files_in(make: &dyn Fn(Units) -> ProjectData, o: &Opts, units: &[Units], arcs: &[bool], posts: &[&str]) -> Vec<(String, String)> {
        let mut all = vec![];
        for &units in units {
            let d = make(units);
            for post in posts {
                for &arcs in arcs { all.extend(case_files(&d, post, units, arcs, o)); }
            }
        }
        all
    }

    // ---------- big files: a digest instead of the text ----------

    /// SHA-256 (FIPS 180-4), small and dependency-free: only used for the few files over `BIG`.
    pub(super) fn sha256(data: &[u8]) -> String {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
            0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
            0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
            0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
            0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
            0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
        ];
        let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
        let mut msg = data.to_vec();
        msg.push(0x80);
        while msg.len() % 64 != 56 { msg.push(0); }
        msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
        for block in msg.chunks_exact(64) {
            let mut w = [0u32; 64];
            for i in 0..16 { w[i] = u32::from_be_bytes([block[4 * i], block[4 * i + 1], block[4 * i + 2], block[4 * i + 3]]); }
            for i in 16..64 {
                let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
                let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
            }
            let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
            for i in 0..64 {
                let t1 = hh.wrapping_add(e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25)).wrapping_add((e & f) ^ (!e & g)).wrapping_add(K[i]).wrapping_add(w[i]);
                let t2 = (a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22)).wrapping_add((a & b) ^ (a & c) ^ (b & c));
                hh = g; g = f; f = e; e = d.wrapping_add(t1); d = c; c = b; b = a; a = t1.wrapping_add(t2);
            }
            for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) { *x = x.wrapping_add(y); }
        }
        h.iter().map(|x| format!("{x:08x}")).collect()
    }

    const CUT: &str = "(... golden digest: middle lines left out ...)";

    /// The stored form of a file over `BIG`: a header with the hash, size and line count, the first and last `ENDS` lines.
    fn digest(text: &str) -> String {
        let lines: Vec<&str> = text.split('\n').collect();
        let mut s = format!("sha256 {}\nbytes {}\nlines {}\n", sha256(text.as_bytes()), text.len(), lines.len());
        for l in &lines[..ENDS.min(lines.len())] { s += l; s.push('\n'); }
        s += CUT; s.push('\n');
        for l in &lines[lines.len().saturating_sub(ENDS)..] { s += l; s.push('\n'); }
        s
    }

    /// The text kept on disk for a generated file, and the name it is kept under.
    fn stored(name: &str, text: &str) -> (String, String) {
        if text.len() > BIG { (format!("{name}.digest"), digest(text)) } else { (name.to_string(), text.to_string()) }
    }

    // ---------- comparing ----------

    /// The first line where `want` and `got` differ, with two lines of context, or `None` when they are equal.
    pub(super) fn first_difference(file: &str, want: &str, got: &str) -> Option<String> {
        if want == got { return None; }
        let (w, g): (Vec<&str>, Vec<&str>) = (want.split('\n').collect(), got.split('\n').collect());
        let i = w.iter().zip(&g).position(|(a, b)| a != b).unwrap_or(w.len().min(g.len()));
        let mut s = format!("{file}: first difference at line {} (expected {} lines, got {})\n", i + 1, w.len(), g.len());
        for (k, l) in w.iter().enumerate().take(i).skip(i.saturating_sub(2)) { s += &format!("           {:>6} | {}\n", k + 1, l); }
        s += &format!("  expected {:>6} | {}\n", i + 1, w.get(i).copied().unwrap_or("<end of file>"));
        s += &format!("  actual   {:>6} | {}\n", i + 1, g.get(i).copied().unwrap_or("<end of file>"));
        Some(s)
    }

    /// Compares (or with UPDATE_GOLDEN rewrites) the files of one project in tests/golden/<slug>/. A file that is expected
    /// but missing, or on disk but no longer made, fails too.
    fn check_project(slug: &str, files: Vec<(String, String)>) {
        let dir = golden_dir().join(slug);
        let want: BTreeMap<String, String> = files.iter().map(|(n, t)| stored(n, t)).collect();
        assert_eq!(want.len(), files.len(), "{slug}: two cases write the same file name");
        if updating() {
            if dir.exists() { fs::remove_dir_all(&dir).unwrap(); }
            fs::create_dir_all(&dir).unwrap();
            for (n, t) in &want { fs::write(dir.join(n), t).unwrap(); }
            return;
        }
        let on_disk: Vec<String> = fs::read_dir(&dir).map(|r| r.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
        let mut bad: Vec<String> = vec![];
        for n in on_disk.iter().filter(|n| !want.contains_key(*n)) { bad.push(format!("{slug}/{n}: on disk but no case makes it")); }
        for (n, got) in &want {
            let path = dir.join(n);
            let Ok(exp) = fs::read_to_string(&path) else { bad.push(format!("{slug}/{n}: missing")); continue };
            if let Some(d) = first_difference(&format!("tests/golden/{slug}/{n}"), &exp, got) { bad.push(d); }
        }
        if !bad.is_empty() {
            let shown = bad.len().min(10);
            panic!("{} of {} golden files differ for {slug} (showing {shown}):\n\n{}\n\nIf the change is intended, run `UPDATE_GOLDEN=1 cargo test --test golden`, \
                    read `git diff tests/golden` and give the reason for every changed file in the commit message (tests/golden/README.md).",
                   bad.len(), want.len(), bad[..shown].join("\n"));
        }
    }

    fn check_sample(k: Kind) { check_project(sample_slug(k), project_files(&|u| sample(k, u))); }

    #[test] fn golden_sample_name_plate() { check_sample(Kind::NamePlate); }
    #[test] fn golden_sample_box() { check_sample(Kind::Box); }
    #[test] fn golden_sample_relief() { check_sample(Kind::Relief); }
    #[test] fn golden_sample_shaped_relief() { check_sample(Kind::ShapedRelief); }
    #[test] fn golden_sample_oval() { check_sample(Kind::Oval); }

    /// Every `*.project.json` under examples/, by its file name without `.project.json`.
    fn example_projects() -> Vec<(String, PathBuf)> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(rd) = fs::read_dir(dir) else { return };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() { walk(&p, out); } else if p.to_string_lossy().ends_with(".project.json") { out.push(p); }
            }
        }
        let mut v = vec![];
        walk(&root().join("examples"), &mut v);
        v.sort();
        v.into_iter().map(|p| (p.file_name().unwrap().to_string_lossy().trim_end_matches(".project.json").to_string(), p)).collect()
    }

    #[test]
    fn golden_examples() {
        let ex = example_projects();
        assert!(!ex.is_empty(), "no *.project.json under examples/");
        for (slug, path) in ex {
            let d = load_project(&fs::read_to_string(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display())).data;
            check_project(&slug, project_files(&|_| d.clone()));
        }
    }

    // ---------- option cases: emitter paths the cross product above never reaches ----------
    //
    // The matrix forces the fixed machine's line numbers, start/end code, spin-up and work offset onto every project, and
    // none of its projects pauses, draws with a pen, uses a Corners tool, cuts trochoidal loops, carves a photo or has a
    // plug on a separate piece. These cases close that gap before the emitter is refactored (phase 1 of the spec). Every
    // dialect; metric only unless the units change text the matrix does not show (see tests/golden/README.md).

    /// One option case: a small project, the machine options it runs with, and the units and arcs settings it is written in.
    struct Case { slug: &'static str, make: fn(Units) -> ProjectData, opts: Opts, units: &'static [Units], arcs: &'static [bool], posts: &'static [&'static str] }

    const ALL: &[&str] = &["", "grblhal", "fluidnc", "mach", "marlin"];
    const MACH: &[&str] = &["mach"];

    const METRIC: &[Units] = &[Units::Metric];
    const NOARCS: &[bool] = &[false];
    const START: &str = "(start code: dust shoe down)\n  M8  \n\nG4 P1";
    const END: &str = "M9\n(end code: dust shoe up)";

    fn cases() -> Vec<Case> {
        vec![
            Case { slug: "opt-numbered", make: |_| small_job(false), opts: Opts { numbered: true, ..PLAIN }, units: METRIC, arcs: &[false, true], posts: ALL },
            Case { slug: "opt-start-end-code", make: |_| small_job(false), opts: Opts { start: Some(START), end: Some(END), ..PLAIN }, units: METRIC, arcs: NOARCS, posts: ALL },
            Case { slug: "opt-start-end-code-numbered", make: |_| small_job(false), opts: Opts { numbered: true, start: Some(START), end: Some(END), ..PLAIN }, units: METRIC, arcs: NOARCS, posts: ALL },
            Case { slug: "opt-wcs-g56-spin0", make: |_| small_job(false), opts: Opts { wcs: Some(2), spin_up: 0.0, ..PLAIN }, units: METRIC, arcs: NOARCS, posts: ALL },
            Case { slug: "opt-wcs-g59-1", make: |_| small_job(false), opts: Opts { wcs: Some(6), ..PLAIN }, units: METRIC, arcs: NOARCS, posts: ALL },
            Case { slug: "pause", make: |_| load("hardware_m0_aircut"), opts: PLAIN, units: METRIC, arcs: NOARCS, posts: ALL },
            Case { slug: "pen", make: pen, opts: PLAIN, units: &[Units::Metric, Units::Imperial], arcs: NOARCS, posts: ALL },
            Case { slug: "pen-tool-change", make: |_| small_job(true), opts: PLAIN, units: METRIC, arcs: NOARCS, posts: ALL },
            Case { slug: "rest-pocket", make: rest_pocket, opts: PLAIN, units: METRIC, arcs: NOARCS, posts: ALL },
            Case { slug: "trochoidal", make: trochoidal, opts: PLAIN, units: METRIC, arcs: NOARCS, posts: ALL },
            Case { slug: "photo-vcarve", make: photo, opts: PLAIN, units: METRIC, arcs: NOARCS, posts: ALL },
            Case { slug: "plug-piece", make: plug_piece, opts: PLAIN, units: METRIC, arcs: NOARCS, posts: ALL },
            // phase 3: tool numbers from the machine's tool rack (T5, T7) on the built-in Mach3 / LinuxCNC, the only built-in that writes them
            Case { slug: "rack", make: |_| load("rack_two_tools"), opts: PLAIN, units: METRIC, arcs: NOARCS, posts: MACH },
            // one tool numbered on the tool (the second one used, T1), the other by first use: it skips 1 and is T2
            Case { slug: "rack-first-use", make: rack_first_use, opts: PLAIN, units: METRIC, arcs: NOARCS, posts: MACH },
        ]
    }

    fn check_case(slug: &str) {
        let c = cases().into_iter().find(|c| c.slug == slug).unwrap_or_else(|| panic!("no case {slug}"));
        check_project(c.slug, files_in(&c.make, &c.opts, c.units, c.arcs, c.posts));
    }

    #[test] fn golden_opt_numbered() { check_case("opt-numbered"); }
    #[test] fn golden_opt_start_end_code() { check_case("opt-start-end-code"); }
    #[test] fn golden_opt_start_end_code_numbered() { check_case("opt-start-end-code-numbered"); }
    #[test] fn golden_opt_wcs_g56_spin0() { check_case("opt-wcs-g56-spin0"); }
    #[test] fn golden_opt_wcs_g59_1() { check_case("opt-wcs-g59-1"); }
    #[test] fn golden_pause() { check_case("pause"); }
    #[test] fn golden_pen() { check_case("pen"); }
    #[test] fn golden_pen_tool_change() { check_case("pen-tool-change"); }
    #[test] fn golden_rest_pocket() { check_case("rest-pocket"); }
    #[test] fn golden_trochoidal() { check_case("trochoidal"); }
    #[test] fn golden_photo_vcarve() { check_case("photo-vcarve"); }
    #[test] fn golden_plug_piece() { check_case("plug-piece"); }
    #[test] fn golden_rack() { check_case("rack"); }
    #[test] fn golden_rack_first_use() { check_case("rack-first-use"); }

    /// The dialect list of the option cases is the matrix's.
    #[test] fn all_is_every_dialect() { assert_eq!(ALL, DIALECTS.map(|d| d.0)); }

    /// Every case has its test above (a case without one would never be compared).
    #[test] fn every_case_has_a_test() {
        let src = include_str!("golden.rs");
        for c in cases() { assert!(src.contains(&format!("check_case(\"{}\")", c.slug)), "case {} has no #[test]", c.slug); }
    }

    fn load(name: &str) -> ProjectData {
        load_project(&fs::read_to_string(super::data(&format!("{name}.project.json"))).unwrap()).unwrap_or_else(|e| panic!("{name}: {e}")).data
    }

    /// A new project with the bundled tools, their ids pinned (the real ones come from the clock).
    fn blank() -> ProjectData {
        let mut d = shapes::blank_project().data;
        for (i, t) in d.tools.iter_mut().enumerate() { t.id = format!("t{:02}", i + 1); }
        d
    }

    /// A small two-tool job on a 60 x 40 x 6 mm stock, 1 mm deep in one pass, no ramp: a 3 mm cutter round a 15 x 10 mm rectangle, then
    /// a 6 mm cutter inside a 16 mm circle (a tool change in every dialect, arcs to fit). With `pen`: a pause after the first
    /// operation (with the default message) and a pen drawing a rectangle between the two cutters, so `select_pen` changes from
    /// a cutter with the spindle stopped and `select_tool` changes back from the pen.
    fn small_job(pen: bool) -> ProjectData {
        let mut d = blank();
        d.stock.w = 60.0; d.stock.h = 40.0; d.stock.t = 6.0;
        let tool = |d: &ProjectData, n: &str| d.tools.iter().find(|t| t.name == n).map(|t| t.id.clone()).unwrap_or_else(|| panic!("no {n}"));
        let op = |id: i64, tool: String, kind: &str| {
            let mut p = shapes::default_op_params(&tool);
            p.kind = kind.into(); p.depth = 1.0; p.pass_depth = Some(1.0); p.ramp = false;
            Operation { id, p }
        };
        let (t3, t6) = (tool(&d, "3 mm flat end mill"), tool(&d, "6 mm flat end mill"));
        d.ops.push(op(1, t3, "outside"));
        d.loops.push(shapes::new_rect([5.0, 5.0], [20.0, 15.0], 0.0, 1));
        if pen {
            let p = load("hardware_pen_test").tools[0].clone();
            d.ops[0].p.pause = Some(String::new());
            d.ops.push(op(2, p.id.clone(), "draw"));
            d.ops[1].p.depth = 0.0;
            d.tools.push(p);
            d.loops.push(shapes::new_rect([25.0, 5.0], [35.0, 15.0], 0.0, 2));
        }
        d.ops.push(op(3, t6, "inside"));
        d.loops.push(shapes::new_circle([45.0, 25.0], 8.0, 3));
        d.oid = 3;
        d
    }

    /// The pen test (an air run, a pause without the spindle, the real drawing), with a pen-down dwell on the second operation
    /// (0.125 s, written with two decimals: `G4 P0.13`).
    fn pen(_: Units) -> ProjectData {
        let mut d = load("hardware_pen_test");
        d.ops[1].p.dwell = Some(0.125);
        d
    }

    /// A pocket with a 6 mm cutter and a 2 mm Corners tool (rest machining), as `PUTIACNC_SHOW_REST` makes it.
    fn rest_pocket(_: Units) -> ProjectData {
        let mut d = blank();
        let by = |d: &ProjectData, n: &str| d.tools.iter().find(|t| t.name == n).map(|t| t.id.clone()).unwrap_or_else(|| panic!("no {n}"));
        let mut p = shapes::default_op_params(&by(&d, "6 mm flat end mill"));
        p.kind = "pocket".into(); p.depth = 4.0; p.rest = Some(by(&d, "2 mm flat end mill"));
        d.stock.w = 100.0; d.stock.h = 80.0; d.stock.t = 10.0;
        d.ops.push(Operation { id: 1, p }); d.oid = 1; d.act = 1;
        d.loops.push(shapes::new_rect([20.0, 20.0], [80.0, 60.0], 0.0, 1));
        d
    }

    /// Trochoidal clearing of the pocket with an island, as tests/trochoidal.rs makes it: with arcs off, only the loops are
    /// fitted as arcs (the per-range fitter).
    fn trochoidal(_: Units) -> ProjectData {
        let mut d = load("pocket_island");
        for o in d.ops.iter_mut() { o.p.troch = true; o.p.ramp = false; o.p.depth = 1.5; o.p.pass_depth = Some(1.5); }
        d
    }

    /// A photo V-carve of a 60 x 40 mm gradient on a 100 x 100 x 12 mm stock, as tests/photo_vcarve.rs makes it.
    fn photo(_: Units) -> ProjectData {
        let mut d = blank();
        d.stock.w = 100.0; d.stock.h = 100.0; d.stock.t = 12.0;
        d.tools = setup::starter_tools(setup::ToolChoice { vbits: true, ..Default::default() });
        for (i, t) in d.tools.iter_mut().enumerate() { t.id = format!("t{:02}", i + 1); }
        let (nx, ny) = (240usize, 160usize);
        let mut b = Vec::new();
        for _ in 0..ny { for i in 0..nx { b.extend(((65535 - i * 65535 / (nx - 1)) as u16).to_le_bytes()); } }
        d.relief = Some(Relief { name: "gradient".into(), nx, ny, res: 0.25, zmax: 3.0, hm: b64(&b), x: 20.0, y: 30.0, scale: 1.0, edge: false });
        shapes::add_operation(&mut d, &Default::default(), "photo").expect("a photo operation");
        d
    }

    fn b64(d: &[u8]) -> String {
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut o = String::new();
        for c in d.chunks(3) {
            let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
            o.push(A[(n >> 18) as usize & 63] as char); o.push(A[(n >> 12) as usize & 63] as char);
            o.push(if c.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
            o.push(if c.len() > 2 { A[n as usize & 63] as char } else { '=' });
        }
        o
    }

    /// The rack project without its rack: the 6 mm end mill (used second) has its own number 1, so the 1/8" end mill (used first) is
    /// numbered by first use and skips 1: T2.
    fn rack_first_use(_: Units) -> ProjectData {
        let mut d = load("rack_two_tools");
        if let Some(m) = d.machine.as_mut() { m.tool_table.clear(); }
        let t = d.tools.iter_mut().find(|t| t.name == "6 mm flat end mill").expect("the 6 mm end mill");
        t.number = Some(1);
        d
    }

    /// An inlay whose plug is cut from a separate 8 mm piece, as the tests in src/pieces.rs make it: a `-plug` file.
    fn plug_piece(_: Units) -> ProjectData {
        let mut d = blank();
        d.loops.push(shapes::new_rect([30.0, 40.0], [50.0, 50.0], 0.0, 0));
        let sel: std::collections::BTreeSet<usize> = [0].into_iter().collect();
        assert!(shapes::add_operation(&mut d, &sel, "inlay").is_none());
        let (pocket, plug) = (d.ops[0].id, d.ops[1].id);
        let piece = PlugPiece { t: 8.0, mat: "darkwood".into(), ..default_piece(&d.stock) };
        shapes::put_plug_on_piece(&mut d, pocket, plug, Some(piece));
        d
    }

    /// Every directory in tests/golden/ belongs to a project that is still checked (a renamed example leaves none behind).
    #[test]
    fn golden_dirs_have_a_project() {
        if updating() { return; } // the other tests are writing them
        let mut known: Vec<String> = example_projects().into_iter().map(|(s, _)| s).collect();
        known.extend([Kind::NamePlate, Kind::Box, Kind::Relief, Kind::ShapedRelief, Kind::Oval].map(|k| sample_slug(k).to_string()));
        known.extend(cases().iter().map(|c| c.slug.to_string()));
        for e in fs::read_dir(golden_dir()).unwrap().flatten().filter(|e| e.path().is_dir()) {
            let n = e.file_name().to_string_lossy().into_owned();
            assert!(known.contains(&n), "tests/golden/{n} has no project: remove it, or add the project back");
        }
    }

    #[test] fn sha256_matches_the_standard_vectors() {
        assert_eq!(sha256(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"), "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
    }

    #[test] fn a_difference_names_the_line() {
        let d = first_difference("f.nc", "G21\nG0 Z5\nG1 X1", "G21\nG0 Z5\nG1 X2").unwrap();
        assert!(d.contains("line 3") && d.contains("expected      3 | G1 X1") && d.contains("actual        3 | G1 X2"), "{d}");
        assert!(first_difference("f.nc", "a\nb", "a").unwrap().contains("<end of file>"));
        assert!(first_difference("f.nc", "a", "a").is_none());
    }
}
