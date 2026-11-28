//! The text pieces of the post engine: number formats, the comment styles and the text filters. Each is a general option of a
//! package, never a dialect: a built-in uses them the way any package can.
use super::CommentStyle;
use crate::toolpath::to_fixed;
use std::fmt::Write;

/// How a number is written (`[format]` of a package).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Format {
    /// Decimals in mm output; `None`: not rounded, written in its shortest form.
    pub decimals: Option<u32>,
    /// Decimals in inch output (the built-ins write inches); `None`: as `decimals`.
    pub inch_decimals: Option<u32>,
    /// Trailing zeros dropped (`1.5`, `0`) or kept (`1.500`, `0.000`).
    pub trim: bool,
    /// A length (or a length per minute): divided by 25.4 in inch output, before it is rounded.
    pub length: bool,
}

impl Format {
    /// The decimals for these units.
    pub fn decimals(&self, inch: bool) -> Option<u32> { if inch { self.inch_decimals.or(self.decimals) } else { self.decimals } }

    /// `v` in the output units and rounded, as the text will read back: what the arc fitter sees.
    pub fn value(&self, v: f64, inch: bool, k: f64) -> f64 {
        let v = if self.length { v / k } else { v };
        match self.decimals(inch) { Some(d) => to_fixed(v, d as usize), None => v }
    }

    /// Writes `v` (in mm, or already in the output units when `in_units`). The order is that of the generator before the IR: the mm
    /// value divided by the units factor, rounded with `to_fixed` (JavaScript's ties), then written.
    pub fn write(&self, out: &mut String, v: f64, inch: bool, k: f64, in_units: bool) {
        let v = if self.length && !in_units { v / k } else { v };
        match self.decimals(inch) {
            None => push_num(out, v),
            Some(d) if self.trim => push_num(out, to_fixed(v, d as usize)),
            Some(d) => {
                let r = to_fixed(v, d as usize);
                let _ = write!(out, "{:.*}", d as usize, if r == 0.0 { 0.0 } else { r });
            }
        }
    }
}

/// `toolpath::num` written into `out`: "0" for zero (also -0), else Rust's shortest form that reads back the same number.
pub fn push_num(out: &mut String, v: f64) {
    if v == 0.0 { out.push('0'); } else { let _ = write!(out, "{}", v); }
}

/// The `comment` filter: text that is safe inside a comment. Plain ASCII without control characters, `(` and `)` as `-`, `″` and `"`
/// as `in` (Mach3 and some LinuxCNC setups choke on more). What tool names in comments always were (`toolpath::tool_label`).
pub fn comment_text(s: &str) -> String {
    s.replace(['″', '"'], "in").replace(['(', ')'], "-").chars().filter(|c| c.is_ascii() && !c.is_ascii_control()).collect()
}

/// The `plain` filter: a message for a controller that reads everything after `M0` (Marlin). Plain ASCII without control characters,
/// no `;`, `(` or `)`, a space between a letter and a number that follows it (a reader would take "Z0" for a move), trimmed.
pub fn plain_text(s: &str) -> String {
    let mut t = String::with_capacity(s.len() + 4);
    let mut prev_alpha = false;
    for c in s.chars().filter(|c| c.is_ascii() && !c.is_ascii_control() && !matches!(c, ';' | '(' | ')')) {
        if prev_alpha && (c.is_ascii_digit() || c == '.' || c == '-') { t.push(' '); }
        prev_alpha = c.is_ascii_alphabetic();
        t.push(c);
    }
    t.trim().to_string()
}

/// A whole-line comment with `inner` inside its brackets, in a comment style: `(inner)`, `; inner` (trimmed) or nothing.
pub fn comment_line(style: CommentStyle, inner: &str) -> Option<String> {
    match style {
        CommentStyle::Paren => Some(format!("({inner})")),
        CommentStyle::Semicolon => Some(format!("; {}", inner.trim())),
        CommentStyle::None => None,
    }
}

/// A line of the machine's own start or end code in a comment style: a whole-line `(…)` is a comment (written as
/// [`comment_line`]); any other line stays as it is.
pub fn machine_line(style: CommentStyle, line: &str) -> Option<String> {
    let t = line.trim();
    match t.strip_prefix('(').and_then(|r| r.strip_suffix(')')) { Some(inner) => comment_line(style, inner), None => Some(line.to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(f: &Format, v: f64, inch: bool) -> String { let mut s = String::new(); f.write(&mut s, v, inch, if inch { 25.4 } else { 1.0 }, false); s }

    #[test]
    fn numbers_trimmed_and_padded() {
        let xyz = Format { decimals: Some(3), inch_decimals: Some(4), trim: true, length: true };
        assert_eq!(w(&xyz, 12.7, false), "12.7");
        assert_eq!(w(&xyz, 0.0004, false), "0", "rounds to nothing: 0, not -0 or 0.000");
        assert_eq!(w(&xyz, -0.0004, false), "0");
        assert_eq!(w(&xyz, 1.0625, false), "1.063", "an exact tie goes up (JavaScript's toFixed), not to even");
        assert_eq!(w(&xyz, 5.0, true), "0.1969", "inches: divided by 25.4, then 4 decimals");
        assert_eq!(w(&xyz, 12.7, true), "0.5");
        let tx = Format { decimals: Some(3), inch_decimals: None, trim: false, length: true };
        assert_eq!(w(&tx, 74.2324, false), "74.232");
        assert_eq!(w(&tx, 14.28, false), "14.280");
        assert_eq!(w(&tx, -0.0001, false), "0.000", "no minus sign on zero");
        assert_eq!(w(&tx, 0.0, false), "0.000");
        assert_eq!(w(&tx, 25.4, true), "1.000", "inch_decimals falls back to decimals");
        let feed = Format { decimals: Some(1), inch_decimals: Some(2), trim: false, length: true };
        assert_eq!(w(&feed, 2500.0, false), "2500.0");
        assert_eq!(w(&feed, 800.0, true), "31.50");
        // not rounded: the shortest form, and no unit conversion without `length`
        let raw = Format { decimals: None, inch_decimals: None, trim: true, length: false };
        assert_eq!(w(&raw, 18000.5, false), "18000.5");
        assert_eq!(w(&raw, 2.0, true), "2");
        assert_eq!(w(&raw, 0.13, false), "0.13");
        // already in the output units: not divided again
        let mut s = String::new();
        xyz.write(&mut s, 0.5, true, 25.4, true);
        assert_eq!(s, "0.5");
        assert_eq!(xyz.value(12.7, true, 25.4), 0.5);
        assert_eq!(xyz.value(1.23456, false, 1.0), 1.235);
    }

    #[test]
    fn the_filters() {
        assert_eq!(comment_text("1/8\u{2033} flat (end) mill"), "1/8in flat -end- mill");
        assert_eq!(comment_text("6 mm \"bit\" é\t"), "6 mm inbitin ");
        assert_eq!(plain_text("Set Z0 (here); then go"), "Set Z 0 here then go");
        assert_eq!(plain_text("  move X-1.5 now  "), "move X -1.5 now");
        assert_eq!(plain_text(";;;"), "");
        assert_eq!(plain_text("Stop the spindle and change tool to 6 mm, then touch off Z0 and resume"), "Stop the spindle and change tool to 6 mm, then touch off Z 0 and resume");
    }

    #[test]
    fn comment_styles() {
        assert_eq!(comment_line(CommentStyle::Paren, "Pass 1"), Some("(Pass 1)".into()));
        assert_eq!(comment_line(CommentStyle::Semicolon, " Tool: 6 mm "), Some("; Tool: 6 mm".into()));
        assert_eq!(comment_line(CommentStyle::None, "Pass 1"), None);
        assert_eq!(machine_line(CommentStyle::Semicolon, "(start: dust shoe)"), Some("; start: dust shoe".into()));
        assert_eq!(machine_line(CommentStyle::Semicolon, "M8"), Some("M8".into()));
        assert_eq!(machine_line(CommentStyle::Semicolon, "G0 X1 (not whole)"), Some("G0 X1 (not whole)".into()));
        assert_eq!(machine_line(CommentStyle::None, "(gone)"), None);
        assert_eq!(machine_line(CommentStyle::Paren, "(kept)"), Some("(kept)".into()));
    }
}
