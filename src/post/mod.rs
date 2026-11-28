//! Post-processors: a [`Program`] (the generator's events) written as G-code text for one controller (phase 2 of
//! `docs/post-processor-spec.md`).
//!
//! Every post implements [`Post`] and declares what its controller can do ([`Caps`]); the generator reads the capabilities before
//! it generates, the post renders afterwards. One engine implements the trait: it loads a *post package* (a TOML manifest with
//! capabilities, properties, number formats, words, the motion group and a template per event handler, `docs/posts/authoring.md`)
//! and runs its templates over the events. The five built-in dialects (GRBL, grblHAL, FluidNC, Mach3 / LinuxCNC, Marlin) are such
//! packages, embedded in the binary ([`builtin`]).
//!
//! Module map: `manifest` reads and checks a package ([`Package::load`]); `template` parses and checks the templates once, at load;
//! `engine` renders a program (arc fitting, the handlers, the machine's start and end code, line numbers); `text` holds the
//! number format, the comment styles and the text filters; `builtin` the embedded packages and the choice of one for a job.
use crate::model::Machine;
use crate::program::Program;
use std::collections::BTreeMap;

pub mod builtin;
mod engine;
mod manifest;
mod template;
pub mod text;
#[cfg(test)]
mod tests;

pub use builtin::{builtin, for_job, Dialect};
pub use manifest::Package;

/// A post-processor: what the controller can do, the user's options, and the program written as text.
pub trait Post: Send + Sync {
    /// The post's name, shown in the machine manager and written in the program's first comment.
    fn name(&self) -> &str;
    fn caps(&self) -> &Caps;
    /// The user's options (Fusion's `properties`), edited in the machine manager.
    fn properties(&self) -> &[PropDef];
    fn render(&self, prog: &Program, ctx: &PostCtx) -> Result<Rendered, PostError>;
}

/// What the controller can do. The generator reads it before it generates (which events it emits); the post reads it while it
/// renders (comment style, dwell unit, arc fitting).
#[derive(Debug, Clone, PartialEq)]
pub struct Caps {
    pub arcs: ArcSupport,
    /// Degrees; the fitter writes no arc longer than this (at most 180).
    pub max_arc_sweep: f64,
    pub tool_change: ToolChangeMode,
    /// The post writes `G43 H#` itself after a tool change (default `on_section_tool` with `tool_change = "atc"`).
    pub length_offset: bool,
    pub spindle: SpindleCtl,
    pub dwell: DwellUnit,
    pub comments: CommentStyle,
    /// How many work coordinate systems the controller has: 6 = G54 to G59, 9 adds G59.1 to G59.3, 1 = one fixed system.
    pub wcs_count: u8,
    /// The longest line the controller reads; a longer line stops the export.
    pub max_line_len: Option<usize>,
}

impl Default for Caps {
    fn default() -> Caps {
        Caps { arcs: ArcSupport::Xy, max_arc_sweep: 180.0, tool_change: ToolChangeMode::Manual, length_offset: false, spindle: SpindleCtl::Controlled,
               dwell: DwellUnit::Seconds, comments: CommentStyle::Paren, wcs_count: 6, max_line_len: None }
    }
}

impl Caps {
    /// The G-code word of work coordinate system `wcs` (0 = G54 ... 8 = G59.3), G54 when the controller does not have that one (what
    /// `Job::wcs_name` did for each dialect).
    pub fn wcs_name(&self, wcs: u8) -> &'static str {
        let i = wcs as usize;
        if i < self.wcs_count as usize && i < crate::model::WCS.len() { crate::model::WCS[i] } else { crate::model::WCS[0] }
    }
    /// The machine stands still at a tool change (the controller changes the tool itself, `T# M6`), so the next move starts from rest.
    pub fn tool_change_halts(&self) -> bool { self.tool_change == ToolChangeMode::Atc }
}

/// Arcs the controller runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArcSupport {
    /// No `G2`/`G3`: nothing is fitted, trochoidal loops stay short lines (with a note).
    None,
    /// Arcs in the XY plane.
    Xy,
    /// Arcs that may also move Z (the fitter writes XY arcs only, so today the same as `Xy`).
    Helical,
}

/// How a tool gets into the spindle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolChangeMode {
    /// The program stops and asks the operator to change the tool (a `Stop` before the `ToolChange`).
    Manual,
    /// The controller changes the tool itself (`T# M6`): no stop, and the machine stands still at the change.
    Atc,
}

/// Who runs the spindle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpindleCtl {
    /// The controller (`M3`, `M5`).
    Controlled,
    /// The operator: the program stops and asks for it, and does not wait for the spindle to come up to speed.
    Manual,
}

/// The unit of a dwell (`P` word of `on_dwell`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DwellUnit {
    Seconds,
    Milliseconds,
    /// The controller cannot dwell (or waits for the spindle itself): no spin-up wait, and `P` has no value.
    None,
}

/// How comments are written: a whole-line `(text)` of a template (and of the machine's start and end code) becomes `(text)`,
/// `; text` or nothing; an inline `(text)` at the end of a command line becomes ` (text)`, ` ; text` or nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommentStyle { Paren, Semicolon, None }

/// One user option of a post.
#[derive(Debug, Clone, PartialEq)]
pub struct PropDef { pub key: String, pub title: String, pub kind: PropKind, pub default: PropValue }

#[derive(Debug, Clone, PartialEq)]
pub enum PropKind { Bool, Int, Number, Text, Enum(Vec<String>) }

/// A property value (an `Enum` property holds `Text`).
#[derive(Debug, Clone, PartialEq)]
pub enum PropValue { Bool(bool), Int(i64), Number(f64), Text(String) }

impl std::fmt::Display for PropValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropValue::Bool(b) => write!(f, "{b}"),
            PropValue::Int(i) => write!(f, "{i}"),
            PropValue::Number(n) => f.write_str(&crate::toolpath::num(*n)),
            PropValue::Text(t) => f.write_str(t),
        }
    }
}

/// What a post may read besides the program: the machine, the property values saved with it (a missing key takes the default),
/// and the safe height.
pub struct PostCtx<'a> {
    pub machine: &'a Machine,
    pub props: &'a BTreeMap<String, PropValue>,
    pub safe_z: f64,
}

/// The line ending of a post's files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineEnding { #[default] Lf, Crlf }

impl LineEnding {
    pub fn as_str(self) -> &'static str { match self { LineEnding::Lf => "\n", LineEnding::Crlf => "\r\n" } }
}

/// A program written by a post.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rendered {
    pub lines: Vec<String>,
    /// The event index of each line; `None` for the machine's own start and end code.
    pub source: Vec<Option<usize>>,
    /// The file extension, without the dot ("nc", "gcode", "tap").
    pub extension: String,
    pub line_ending: LineEnding,
    /// Warnings raised by the post (export notes).
    pub notes: Vec<String>,
    /// Arcs fitted over the trochoidal loops (with `Meta.arcs` off), and over the whole program (with it on): for the generator's notes.
    pub troch_arcs: usize,
    pub arcs: usize,
}

/// A post that cannot load or cannot write a program: where, and what to do about it.
#[derive(Debug, Clone, PartialEq)]
pub struct PostError {
    /// The package file ("builtin:grbl/post.toml", or a path).
    pub file: String,
    /// 1-based line in that file, when the error has one.
    pub line: Option<usize>,
    /// What is wrong and the fix.
    pub message: String,
}

impl std::fmt::Display for PostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line { Some(l) => write!(f, "{}:{}: {}", self.file, l, self.message), None => write!(f, "{}: {}", self.file, self.message) }
    }
}

impl std::error::Error for PostError {}

/// Prefix every line with a line number: N10, N20, ... (the engine numbers a program last, after everything else; `summary` numbers
/// again after it adds its header).
pub fn number_lines(lines: Vec<String>) -> Vec<String> {
    lines.into_iter().enumerate().map(|(i, l)| format!("N{} {}", (i + 1) * 10, l)).collect()
}

/// The message of a stop line written by a built-in post (`M0 ; message` on the GRBL family, `M0 (message)` on Mach3 / LinuxCNC,
/// `M0 message` on Marlin), when the line is one.
pub fn stop_message(line: &str) -> Option<&str> {
    let line = line.trim_end();
    if let Some(m) = line.strip_prefix("M0 ; ") { return Some(m); }
    if let Some(m) = line.strip_prefix("M0 (") { return Some(m.strip_suffix(')').unwrap_or(m)); }
    line.strip_prefix("M0 ").filter(|m| !m.is_empty())
}

/// A fixed program with every kind of event, made the way the generator would make it for a post with these capabilities (stops
/// for a manual tool change and a manual spindle, no spin-up wait without spindle control or dwell): two tools, an operation each,
/// passes, a requested dwell, an arc, an operator stop and the end. For linting a post and for its "Test post" view.
pub fn test_program(caps: &Caps) -> Program {
    use crate::program::{ArcTo, CommentKind, DwellWhy, Event, Meta, ToolRef, Xyz};
    let manual_spindle = caps.spindle == SpindleCtl::Manual;
    let wait = !manual_spindle && caps.dwell != DwellUnit::None;
    let tool = |n: u32, name: &str, dia: f64| Event::ToolChange { tool: Box::new(ToolRef { id: format!("t{n}"), name: name.into(), number: n, length_offset: None, dia, shape: "flat".into() }) };
    let c = |text: &str, kind| Event::Comment { text: text.into(), kind };
    let mut ev = vec![c("PutiaCNC - test program", CommentKind::Info), c("Stock 100 x 50 x 12 mm; XY0 = lower-left corner of stock, Z0 = top surface", CommentKind::Info), Event::Setup];
    let start_at = ev.len();
    ev.push(Event::Rapid { to: Xyz::new(None, None, Some(5.0)) });
    let spin = |ev: &mut Vec<Event>, rpm: f64| {
        if manual_spindle { ev.push(Event::Stop { message: format!("Start the spindle at {} rpm, then resume", crate::toolpath::num(rpm)), optional: false }); }
        else { ev.push(Event::SpindleOn { rpm }); }
        if wait { ev.push(Event::Dwell { seconds: 2.0, why: DwellWhy::SpinUp }); }
    };
    ev.push(Event::OperationStart { index: 0, name: "outside".into() });
    ev.push(tool(1, "1/8\u{2033} flat (end) mill", 3.175));
    spin(&mut ev, 18000.0);
    ev.extend([c("Operation 1: outside, tool 3.175 mm", CommentKind::Operation), Event::Rapid { to: Xyz::new(Some(10.0), Some(-2.5), None) },
               Event::Rapid { to: Xyz::new(None, None, Some(0.5)) }, Event::Feed { to: Xyz::new(None, None, Some(-1.0)), feed: 250.0, write_feed: true },
               c("Pass 1", CommentKind::Pass), Event::Feed { to: Xyz::new(Some(20.0), None, None), feed: 800.0, write_feed: true },
               Event::Feed { to: Xyz::new(Some(30.0), Some(10.0), None), feed: 800.0, write_feed: false },
               Event::Arc(Box::new(ArcTo { to: Xyz::new(Some(20.0), Some(20.0), None), center: [20.0, 10.0], cw: false, feed: 800.0, write_feed: false })),
               Event::Rapid { to: Xyz::new(None, None, Some(5.0)) }, Event::OperationEnd { index: 0 }]);
    ev.push(Event::OperationStart { index: 1, name: "draw".into() });
    if !manual_spindle { ev.push(Event::SpindleOff); }
    if caps.tool_change == ToolChangeMode::Manual {
        let pre = if manual_spindle { "Stop the spindle and change" } else { "Change" };
        ev.push(Event::Stop { message: format!("{pre} tool to 6 mm, then touch off Z0 and resume"), optional: false });
    }
    ev.push(tool(2, "6 mm", 6.0));
    spin(&mut ev, 12000.0);
    ev.extend([c("Operation 2: pocket, tool 6 mm", CommentKind::Operation), Event::Rapid { to: Xyz::new(Some(40.0), Some(20.0), None) },
               Event::Feed { to: Xyz::new(None, None, Some(-0.5)), feed: 300.0, write_feed: true }, Event::Dwell { seconds: 0.13, why: DwellWhy::Requested },
               Event::Feed { to: Xyz::new(Some(50.0), Some(20.0), None), feed: 1000.0, write_feed: true }, Event::Rapid { to: Xyz::new(None, None, Some(5.0)) }]);
    if !manual_spindle { ev.push(Event::SpindleOff); }
    ev.push(Event::Stop { message: if manual_spindle { "Stop the spindle. Operation 2 done: resume when ready".into() } else { "Operation 2 done: resume when ready".into() }, optional: false });
    ev.push(Event::OperationEnd { index: 1 });
    ev.push(if manual_spindle { Event::Stop { message: "Stop the spindle, then resume".into(), optional: false } } else { Event::SpindleOff });
    ev.push(Event::ProgramEnd { park: Some(Xyz::new(Some(0.0), Some(0.0), None)) });
    Program { meta: Meta { start_at, ..Meta::default() }, events: ev }
}
