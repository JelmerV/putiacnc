//! Arc output: runs of short straight cutting moves that lie on a circle are written as one G2/G3 line.
//!
//! The fitter works on the program line by line, in the output units, with the numbers as they are written (rounded). Its core,
//! [`fit_items`], takes each line as an [`Item`]: a `G1` that changes X and/or Y alone (a cut), or anything else with the X and Y it
//! gives. The post engine builds the items from the program's events (`crate::post`), so any post format gets arcs; [`fit_arcs`]
//! builds them by reading text lines, as the generator did before the IR. Only runs of cuts are looked at; everything else
//! (rapids, plunges, ramps, spindle and comment lines) passes through unchanged. A run becomes an arc only when its points sit on
//! one circle within a very small tolerance AND each straight piece is already a fine approximation of that circle, so a coarse
//! polygon (a hexagon, say) is never turned into a circle.

/// Points must lie this close to the fitted circle (mm). The motion cross-check (`crosscheck`) allows the same.
pub const ON_CIRCLE: f64 = 0.005;
/// Largest gap between a straight piece and the circle it stands for (mm): the app draws circles with short chords, so this lets a
/// drawn circle become an arc while a deliberate polygon stays straight.
const MAX_SAGITTA: f64 = 0.12;
/// What a Grbl-type controller accepts for the difference between the start and end radius (mm); we stay well inside it.
const MAX_RADIUS_GAP: f64 = 0.004;
const MIN_RADIUS: f64 = 0.05;
const MAX_RADIUS: f64 = 1000.0;

fn words(line: &str) -> Vec<(char, &str)> {
    line.split_whitespace().filter_map(|w| { let mut c = w.chars(); c.next().map(|k| (k, c.as_str())) }).collect()
}

/// The X and Y words of a line, as the fitter reads any line that is not a cut: every whitespace-separated word that starts with X
/// or Y and parses as a number, whatever the line is (a stop message "M0 ; move X10 away" gives X10), except a line that starts with
/// `(`. The post engine reads the non-move lines of a program with this too, so that it fits exactly what the text fitter fitted.
pub fn xy(line: &str) -> (Option<f64>, Option<f64>) {
    if line.starts_with('(') { return (None, None); }
    let mut x = None; let mut y = None;
    for (k, v) in words(line) {
        match k { 'X' => x = v.parse().ok(), 'Y' => y = v.parse().ok(), _ => {} }
    }
    (x, y)
}

/// A `G1` line that changes only X and/or Y (an F word is allowed): the axes it gives, and its F text.
fn plain_cut(line: &str) -> Option<(Option<f64>, Option<f64>, Option<&str>)> {
    let mut it = line.split_whitespace();
    if it.next()? != "G1" { return None; }
    let (mut x, mut y, mut f) = (None, None, None);
    for w in it {
        let (k, v) = w.split_at(1);
        match k {
            "X" => x = Some(v.parse().ok()?),
            "Y" => y = Some(v.parse().ok()?),
            "F" => f = Some(v),
            _ => return None,
        }
    }
    if x.is_some() || y.is_some() { Some((x, y, f)) } else { None }
}

/// One line of the program as the fitter sees it, with the numbers as written (output units, rounded).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Item {
    /// A `G1` that changes only X and/or Y: the axes it gives (the others stay where they are), and whether it writes a feed. A feed in
    /// the middle of a run starts a new run.
    Cut { x: Option<f64>, y: Option<f64>, f: bool },
    /// Any other line: the X and Y it gives (none for a comment, a spindle or a tool line).
    Other { x: Option<f64>, y: Option<f64> },
}

impl Item {
    fn xy(&self) -> (Option<f64>, Option<f64>) { match *self { Item::Cut { x, y, .. } | Item::Other { x, y } => (x, y) } }
}

/// An arc that replaces the items `first..=last` (all cuts): it ends at `end`, its centre is `ij` from its start (rounded to the
/// decimals the fitter was given), clockwise or not. It writes the feed when the item `first` does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fitted { pub first: usize, pub last: usize, pub end: [f64; 2], pub ij: [f64; 2], pub cw: bool }

fn circle3(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Option<([f64; 2], f64)> {
    let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
    if d.abs() < 1e-12 { return None; }
    let (a2, b2, c2) = (a[0] * a[0] + a[1] * a[1], b[0] * b[0] + b[1] * b[1], c[0] * c[0] + c[1] * c[1]);
    let ux = (a2 * (b[1] - c[1]) + b2 * (c[1] - a[1]) + c2 * (a[1] - b[1])) / d;
    let uy = (a2 * (c[0] - b[0]) + b2 * (a[0] - c[0]) + c2 * (b[0] - a[0])) / d;
    let r = ((a[0] - ux).powi(2) + (a[1] - uy).powi(2)).sqrt();
    Some(([ux, uy], r))
}

/// Does `p[0..=n]` form one arc of at most `max_sweep` radians (half a circle at most)? Returns (centre, clockwise).
fn fit(p: &[[f64; 2]], k: f64, max_sweep: f64) -> Option<([f64; 2], bool)> {
    let n = p.len();
    if n < 4 { return None; }
    let (c, r) = circle3(p[0], p[n / 2], p[n - 1])?;
    if r < MIN_RADIUS / k || r > MAX_RADIUS / k { return None; }
    let mut sign = 0.0_f64;
    let mut sweep = 0.0_f64;
    for i in 0..n {
        let d = ((p[i][0] - c[0]).powi(2) + (p[i][1] - c[1]).powi(2)).sqrt();
        if (d - r).abs() > ON_CIRCLE / k { return None; }
        if i + 1 < n {
            let l = ((p[i + 1][0] - p[i][0]).powi(2) + (p[i + 1][1] - p[i][1]).powi(2)).sqrt();
            if l < 1e-9 { return None; }
            let sag = r - (r * r - l * l / 4.0).max(0.0).sqrt();
            if sag > MAX_SAGITTA / k { return None; }
            let a0 = (p[i][1] - c[1]).atan2(p[i][0] - c[0]);
            let a1 = (p[i + 1][1] - c[1]).atan2(p[i + 1][0] - c[0]);
            let mut da = a1 - a0;
            while da > std::f64::consts::PI { da -= 2.0 * std::f64::consts::PI; }
            while da < -std::f64::consts::PI { da += 2.0 * std::f64::consts::PI; }
            if da == 0.0 { return None; }
            if sign == 0.0 { sign = da.signum(); } else if da.signum() != sign { return None; }
            sweep += da;
        }
    }
    if sweep.abs() > max_sweep + 1e-9 { return None; }
    Some((c, sign < 0.0))
}

fn fixed(v: f64, dec: usize) -> f64 { let f = 10f64.powi(dec as i32); (v * f).round() / f }
fn txt(v: f64) -> String { let v = if v == 0.0 { 0.0 } else { v }; format!("{}", v) }

/// The largest sweep of one arc for a post's `caps.max_arc_sweep` (degrees): half a circle at most, which is also what the fitter
/// can check (it finds the circle through the first, middle and last point).
pub fn max_sweep(degrees: f64) -> f64 {
    if degrees >= 180.0 || !degrees.is_finite() { std::f64::consts::PI } else { degrees.max(1.0).to_radians() }
}

/// The arcs in `items`. `dec` is the number of decimals of the output (the centre offsets are rounded to it), `k` the length of one
/// output unit in mm (1 for mm, 25.4 for inches; the limits above are in mm, so in output units they are divided by `k`),
/// `max_sweep` the largest sweep of one arc in radians ([`max_sweep`]). The position is unknown until the items have given both X and
/// Y; a run is only looked at from a known position. The arcs come in order and do not overlap.
pub fn fit_items(items: &[Item], dec: usize, k: f64, max_sweep: f64) -> Vec<Fitted> {
    let mut out = Vec::new();
    let mut pos: Option<[f64; 2]> = None;
    let mut run: Vec<[f64; 2]> = Vec::new();
    let mut i = 0;
    while i < items.len() {
        // collect a run of plain cutting moves starting here
        run.clear();
        if let Some(start) = pos {
            let mut cur = start;
            let mut j = i;
            while j < items.len() {
                match items[j] {
                    // a feed change in the middle of a run starts a new run
                    Item::Cut { x, y, f } if !(j > i && f) => { let pt = [x.unwrap_or(cur[0]), y.unwrap_or(cur[1])]; run.push(pt); cur = pt; j += 1; }
                    _ => break,
                }
            }
        }
        if run.len() < 3 {
            let (x, y) = items[i].xy();
            let mut p = pos.unwrap_or([f64::NAN, f64::NAN]);
            if x.is_some() || y.is_some() {
                if let Some(x) = x { p[0] = x; }
                if let Some(y) = y { p[1] = y; }
                if p[0].is_finite() && p[1].is_finite() { pos = Some(p); }
            }
            i += 1;
            continue;
        }
        let start = pos.unwrap();
        let mut pts: Vec<[f64; 2]> = Vec::with_capacity(run.len() + 1);
        pts.push(start);
        pts.extend_from_slice(&run);
        // greedy: the longest arc from point `a`
        let mut a = 0;
        while a + 1 < pts.len() {
            let mut best: Option<(usize, [f64; 2], bool)> = None;
            let mut b = a + 3;
            while b < pts.len() {
                match fit(&pts[a..=b], k, max_sweep) { Some((c, cw)) => { best = Some((b, c, cw)); b += 1; } None => break }
            }
            let mut written = false;
            if let Some((b, c, cw)) = best {
                let (s, e) = (pts[a], pts[b]);
                let (ci, cj) = (fixed(c[0] - s[0], dec), fixed(c[1] - s[1], dec));
                let r0 = (ci * ci + cj * cj).sqrt();
                let r1 = ((e[0] - (s[0] + ci)).powi(2) + (e[1] - (s[1] + cj)).powi(2)).sqrt();
                if (r0 - r1).abs() <= MAX_RADIUS_GAP / k {
                    // the run's item j goes from pts[j] to pts[j + 1], so the arc from pts[a] to pts[b] replaces items a..b
                    out.push(Fitted { first: i + a, last: i + b - 1, end: e, ij: [ci, cj], cw });
                    a = b;
                    written = true;
                }
            }
            if !written { a += 1; }
        }
        i += run.len();
        pos = Some(*pts.last().unwrap());
    }
    out
}

/// Rewrite `lines` with arcs. `dec` is the number of decimals of the output, `k` the length of one output unit in mm (1 for mm, 25.4 for
/// inches); the limits above are in mm, so in output units they are divided by `k`. Returns the new lines and how many arcs were written.
pub fn fit_arcs(lines: &[String], dec: usize, k: f64) -> (Vec<String>, usize) {
    let (out, _, n) = fit_arcs_mapped(lines, dec, k);
    (out, n)
}

/// `fit_arcs`, also giving for each new line the index of the line of `lines` it comes from: itself for a line that is kept, the
/// first line of the run it replaces for an arc. The indices only grow. The lines are read into [`Item`]s and fitted by
/// [`fit_items`]; an arc is written `G2`/`G3` with X, Y, I and J, and the F word of the first line it replaces.
pub fn fit_arcs_mapped(lines: &[String], dec: usize, k: f64) -> (Vec<String>, Vec<usize>, usize) {
    let mut items: Vec<Item> = Vec::with_capacity(lines.len());
    let mut feeds: Vec<Option<&str>> = Vec::with_capacity(lines.len());
    for l in lines {
        match plain_cut(l) {
            Some((x, y, f)) => { items.push(Item::Cut { x, y, f: f.is_some() }); feeds.push(f); }
            None => { let (x, y) = xy(l); items.push(Item::Other { x, y }); feeds.push(None); }
        }
    }
    let arcs = fit_items(&items, dec, k, std::f64::consts::PI);
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut from: Vec<usize> = Vec::with_capacity(lines.len());
    let mut i = 0;
    for a in &arcs {
        while i < a.first { out.push(lines[i].clone()); from.push(i); i += 1; }
        let mut l = format!("{} X{} Y{} I{} J{}", if a.cw { "G2" } else { "G3" }, txt(a.end[0]), txt(a.end[1]), txt(a.ij[0]), txt(a.ij[1]));
        if let Some(f) = feeds[a.first] { l += &format!(" F{f}"); }
        out.push(l);
        from.push(a.first);
        i = a.last + 1;
    }
    while i < lines.len() { out.push(lines[i].clone()); from.push(i); i += 1; }
    (out, from, arcs.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle_lines(cx: f64, cy: f64, r: f64, n: usize) -> Vec<String> {
        let mut v = vec![format!("G0 X{} Y{}", txt(fixed(cx + r, 3)), txt(fixed(cy, 3))), "G1 Z-1 F200".to_string()];
        for i in 1..=n {
            let a = i as f64 / n as f64 * std::f64::consts::TAU;
            v.push(format!("G1 X{} Y{}{}", txt(fixed(cx + r * a.cos(), 3)), txt(fixed(cy + r * a.sin(), 3)), if i == 1 { " F800" } else { "" }));
        }
        v.push("G0 Z5".into());
        v
    }

    #[test]
    fn a_drawn_circle_becomes_arcs() {
        let l = circle_lines(50.0, 40.0, 12.0, 120);
        let (o, n) = fit_arcs(&l, 3, 1.0);
        assert!((2..=6).contains(&n), "{n} arcs");
        assert!(o.len() < 20, "{} lines", o.len());
        assert!(o.iter().any(|l| l.starts_with("G3 ")), "counter-clockwise");
        assert!(o.first().unwrap().starts_with("G0 ") && o.last().unwrap() == "G0 Z5" && o[1] == "G1 Z-1 F200");
        // the first arc keeps the feed that was on the first line of the run
        assert!(o.iter().find(|l| l.starts_with("G3 ")).unwrap().contains("F800"));
        // every arc is consistent: start and end sit on the same circle
        let mut p = [62.0, 40.0];
        for l in &o {
            if l.starts_with("G3 ") || l.starts_with("G2 ") {
                let w = words(l);
                let g = |c: char| w.iter().find(|x| x.0 == c).unwrap().1.parse::<f64>().unwrap();
                let (c, e) = ([p[0] + g('I'), p[1] + g('J')], [g('X'), g('Y')]);
                let (r0, r1) = ((p[0] - c[0]).hypot(p[1] - c[1]), (e[0] - c[0]).hypot(e[1] - c[1]));
                assert!((r0 - 12.0).abs() < 0.01 && (r1 - r0).abs() < 0.004);
                p = e;
            }
        }
        assert!((p[0] - 62.0).abs() < 1e-3 && (p[1] - 40.0).abs() < 1e-3, "ends where it started");
    }

    #[test]
    fn each_line_says_where_it_comes_from() {
        let l = circle_lines(50.0, 40.0, 12.0, 120);
        let (o, from, n) = fit_arcs_mapped(&l, 3, 1.0);
        assert_eq!((o.clone(), n), fit_arcs(&l, 3, 1.0));
        assert_eq!(from.len(), o.len());
        assert!(from.windows(2).all(|w| w[0] < w[1]), "{from:?}");
        // kept lines point at themselves; the first arc starts with the first cut of the circle (line 2, after the rapid and the plunge)
        for (j, line) in o.iter().enumerate() { if !line.starts_with("G2") && !line.starts_with("G3") { assert_eq!(*line, l[from[j]]); } }
        let first_arc = o.iter().position(|x| x.starts_with("G3 ")).unwrap();
        assert_eq!(from[first_arc], 2);
        assert_eq!(*from.last().unwrap(), l.len() - 1);
    }

    #[test]
    fn clockwise_is_g2() {
        let mut l = circle_lines(0.0, 0.0, 10.0, 100);
        let first = l[0].clone();
        let mut pts: Vec<String> = l.drain(2..l.len() - 1).collect();
        pts.reverse();
        // walk the circle the other way: start at the same point
        let mut rev = vec![first, "G1 Z-1 F200".to_string()];
        rev.extend(pts.into_iter().skip(1));
        rev.push("G1 X10 Y0".into());
        let (o, _) = fit_arcs(&rev, 3, 1.0);
        assert!(o.iter().any(|l| l.starts_with("G2 ")) && !o.iter().any(|l| l.starts_with("G3 ")));
    }

    #[test]
    fn polygons_and_lines_stay_straight() {
        for sides in [6usize, 8, 12] {
            let (o, n) = fit_arcs(&circle_lines(0.0, 0.0, 10.0, sides), 3, 1.0);
            assert_eq!(n, 0, "{sides}-gon");
            assert_eq!(o.len(), sides + 3);
        }
        let l: Vec<String> = ["G0 X0 Y0", "G1 X1 Y0 F500", "G1 X2 Y0", "G1 X3 Y0", "G1 X4 Y0", "G1 X5 Y0"].iter().map(|s| s.to_string()).collect();
        let (o, n) = fit_arcs(&l, 3, 1.0);
        assert_eq!((n, o.len()), (0, 6));
    }

    #[test]
    fn plunges_and_comments_pass_through() {
        let l: Vec<String> = ["(Operation 1: X5 Y5)", "G21 G90 G17", "M3 S12000", "G0 X10 Y10", "G1 Z-1 F200", "G1 X20 Y10 Z-1.1"].iter().map(|s| s.to_string()).collect();
        let (o, n) = fit_arcs(&l, 3, 1.0);
        assert_eq!((n, &o), (0, &l));
    }
}
