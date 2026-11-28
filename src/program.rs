//! The intermediate representation (IR) between the toolpath generator and the G-code text (phase 1 of
//! `docs/post-processor-spec.md`).
//!
//! `toolpath::generate` asks the job's post (`crate::post`) what the controller can do, walks the operations and pushes
//! [`Event`]s, in program order, into a [`Program`]; the post turns the program into text (phase 2: the post engine with the
//! built-in packages). Every line of the output comes from one event (or from the machine's own start and end code), so the post
//! also returns which event each line came from.
//!
//! Values are millimetres, mm/min and seconds, in absolute work coordinates: what the emitter writes as `x - ox`, `y - oy` and
//! `z + zt`. The post converts to the output units and rounds. Comment and message texts are final text (they can hold
//! values already written in inches).
//!
//! The move list and the time estimate are not rebuilt from the events: the generator keeps computing them where it decides a
//! move, as before (two events have no move: the first lift to safe Z and the park at the end).
use crate::model::Units;

/// A program: what the generator decided, before it is written as text.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Program {
    pub meta: Meta,
    pub events: Vec<Event>,
}

/// What the post needs besides the events: the job settings that today's text depends on. Phase 4a moves `numbered`, `arcs`,
/// `start_code` and `end_code` into the post's properties (see the implementation log).
#[derive(Debug, Clone, PartialEq)]
pub struct Meta {
    /// The output units (the events are always in mm).
    pub units: Units,
    /// The work coordinate system word, already resolved with the post's `caps.wcs_count` (`Caps::wcs_name`).
    pub wcs: &'static str,
    /// Number the lines (N10, N20, ...).
    pub numbered: bool,
    /// Fit arcs (G2/G3) over the whole program (`Machine.arcs`).
    pub arcs: bool,
    /// The machine's own lines, trimmed, without blank ones: inserted before the event at `start_at` and before the last line.
    pub start_code: Vec<String>,
    pub end_code: Vec<String>,
    /// The event the start code goes in front of (the first lift to safe Z, right after the setup).
    pub start_at: usize,
    /// Event ranges (start, end exclusive) of trochoidal loops. With `arcs` off, only these are fitted as arcs; each range starts a
    /// few events before its loops, so the fitter knows where the tool is.
    pub troch: Vec<(usize, usize)>,
}

impl Default for Meta {
    fn default() -> Self {
        Meta { units: Units::Metric, wcs: "G54", numbered: false, arcs: false, start_code: vec![], end_code: vec![], start_at: 0, troch: vec![] }
    }
}

/// One step of the program. Kept small (48 bytes): a big relief has hundreds of thousands of moves, and the GUI clones the output.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A comment line; `text` without the brackets.
    Comment { text: String, kind: CommentKind },
    /// The setup block after the header comments: units, absolute coordinates, plane, feed mode, work offset (the post's `on_open`).
    Setup,
    /// An operation starts (`index` in the project's list, `name` what it is, e.g. "pocket"); the post's `on_section`. What the
    /// operation does is written by the `Comment { kind: Operation }` events that follow, so the built-in posts write nothing here.
    OperationStart { index: usize, name: String },
    /// The operation `index` ends (after its operator pause, if it has one); the post's `on_section_end`.
    OperationEnd { index: usize },
    /// This tool is in the spindle from here on (the first tool, or a change). Whether the operator is asked to change it is a `Stop`
    /// the generator puts before it (`caps.tool_change`).
    ToolChange { tool: Box<ToolRef> },
    /// The spindle starts, clockwise.
    SpindleOn { rpm: f64 },
    SpindleOff,
    Dwell { seconds: f64, why: DwellWhy },
    /// A rapid move. `to` holds the axes the emitter decided to write.
    Rapid { to: Xyz },
    /// A cutting move at `feed` mm/min. `write_feed`: the feed is given, like an axis in `to` (the emitter's modal feed rule: the
    /// value changed, or a tool or pen selection or an operator pause made it forget the feed, even where they wrote nothing). A
    /// post whose F word is `as_given` writes F exactly then; a `modal` F decides for itself. A feed move can change no axis and only
    /// set the feed.
    Feed { to: Xyz, feed: f64, write_feed: bool },
    /// A circular cutting move (`on_circular`). The generator writes none yet: the post fits arcs over runs of short `Feed` moves
    /// itself (the same fitting as before the IR). Boxed, so that a move event stays small.
    Arc(Box<ArcTo>),
    /// The program stops until the operator resumes it.
    Stop { message: String, optional: bool },
    /// The end: the park move (work coordinates) and the end of the program.
    ProgramEnd { park: Option<Xyz> },
}

impl Event {
    /// True when the machine stands still at this event (the spindle starts or stops, a dwell, a stop for the operator, or a tool
    /// change the controller makes itself): the next move starts from rest. `tool_change_halts`: the controller changes the tool
    /// itself (`caps.tool_change = atc`, a `T# M6` command); with a manual change the stop before it is what halts.
    pub fn halts(&self, tool_change_halts: bool) -> bool {
        match self {
            Event::SpindleOn { .. } | Event::SpindleOff | Event::Dwell { .. } | Event::Stop { .. } => true,
            Event::ToolChange { .. } => tool_change_halts,
            _ => false,
        }
    }

    /// An operation marker (`OperationStart`, `OperationEnd`): no move, no machine action. The trochoidal ranges count back over the
    /// events around them without these, so that a range covers the same lines as before the markers existed.
    pub fn is_marker(&self) -> bool { matches!(self, Event::OperationStart { .. } | Event::OperationEnd { .. }) }
}

/// The end, centre and direction of an [`Event::Arc`], in work mm; the start is where the previous move ended.
#[derive(Debug, Clone, PartialEq)]
pub struct ArcTo {
    /// The end point (the axes given, like a `Feed`; Z given makes it a helix).
    pub to: Xyz,
    /// The centre, X and Y.
    pub center: [f64; 2],
    /// Clockwise (G2) or counter-clockwise (G3).
    pub cw: bool,
    pub feed: f64,
    /// The feed is given (as `Feed::write_feed`).
    pub write_feed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKind {
    /// Header and other information for the reader.
    Info,
    /// What an operation (or a part of it) does.
    Operation,
    /// The pass a move belongs to (`(Pass n)`), read back by the 3D replay.
    Pass,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DwellWhy {
    /// The wait for the spindle to come up to speed (`Machine.spin_up`), counted in the time estimate; written as given.
    SpinUp,
    /// A pause asked for by an operation (the pen-down dwell of a Draw operation); written with two decimals.
    Requested,
}

/// A tool as the program refers to it.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolRef {
    pub id: String,
    /// The tool's name as the user typed it (a post makes it plain ASCII for a comment with the `comment` filter).
    pub name: String,
    /// The tool number a post writes (`T#`, `{tool.number}`), resolved by `toolnum::resolve`: the machine's tool table, the tool's
    /// own number, or the order of first use.
    pub number: u16,
    /// The length offset register (`H#`, `{tool.offset}`); `None` = the number (the generator always sets it).
    pub length_offset: Option<u16>,
    /// Where `number` came from (the setup sheet lists rack slots).
    pub from: NumberFrom,
    pub dia: f64,
    pub shape: String,
}

impl ToolRef {
    /// The length offset register: `length_offset`, or the tool number.
    pub fn offset(&self) -> u16 { self.length_offset.unwrap_or(self.number) }
}

/// Where a tool's number came from (`toolnum::resolve`), first match wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumberFrom {
    /// The machine's tool table (`Machine.tool_table`): a rack slot.
    Rack,
    /// The tool's own number (`Tool.number`).
    Tool,
    /// The order of first use in this program (only for posts with `tool_numbers = "first_use"`).
    #[default]
    FirstUse,
}

/// A target position: each axis given or not. Not given = not written (unchanged). Stored as three numbers and a mask, so that
/// a move event stays at 48 bytes.
#[derive(Clone, Copy, Default)]
pub struct Xyz { v: [f64; 3], set: u8 }

impl PartialEq for Xyz {
    fn eq(&self, o: &Xyz) -> bool { self.set == o.set && (0..3).all(|i| self.get(i) == o.get(i)) }
}

impl Xyz {
    /// No axis.
    pub const NONE: Xyz = Xyz { v: [0.0; 3], set: 0 };

    pub fn new(x: Option<f64>, y: Option<f64>, z: Option<f64>) -> Xyz {
        let mut p = Xyz::NONE;
        for (i, a) in [x, y, z].into_iter().enumerate() { if let Some(a) = a { p.put(i, a); } }
        p
    }
    /// Sets axis `i` (0 = X, 1 = Y, 2 = Z).
    pub fn put(&mut self, i: usize, v: f64) { self.v[i] = v; self.set |= 1 << i; }
    /// Axis `i` (0 = X, 1 = Y, 2 = Z), when it is given.
    pub fn get(&self, i: usize) -> Option<f64> { (self.set & (1 << i) != 0).then_some(self.v[i]) }
    pub fn x(&self) -> Option<f64> { self.get(0) }
    pub fn y(&self) -> Option<f64> { self.get(1) }
    pub fn z(&self) -> Option<f64> { self.get(2) }
    pub fn is_empty(&self) -> bool { self.set == 0 }
    /// The axes given (bit 0 X, 1 Y, 2 Z).
    pub fn mask(&self) -> u8 { self.set }
}

impl std::fmt::Debug for Xyz {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Xyz").field("x", &self.x()).field("y", &self.y()).field("z", &self.z()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_axis_is_given_or_not_and_zero_is_a_value() {
        let p = Xyz::new(Some(1.5), None, Some(0.0));
        assert_eq!((p.x(), p.y(), p.z()), (Some(1.5), None, Some(0.0)));
        assert!(!p.is_empty() && Xyz::NONE.is_empty());
        let mut q = Xyz::NONE;
        q.put(1, -0.0);
        assert_eq!(q.get(1), Some(-0.0));
        assert!(q.get(1).unwrap().is_sign_negative(), "the value is kept as it is");
        assert_eq!(format!("{q:?}"), "Xyz { x: None, y: Some(-0.0), z: None }");
        // two positions with the same axes and values are equal, whatever was stored in an axis that is not given
        assert_eq!(Xyz::new(Some(2.0), None, None), Xyz::new(Some(2.0), None, None));
    }

    #[test]
    fn an_event_stays_small() {
        // a move is the most common event: three coordinates, a mask, the feed and a flag
        assert!(std::mem::size_of::<Event>() <= 48, "{} bytes", std::mem::size_of::<Event>());
        assert_eq!(std::mem::size_of::<Xyz>(), 32);
    }

    #[test]
    fn what_halts_the_machine() {
        let tool = Event::ToolChange { tool: Box::new(ToolRef { id: "t".into(), name: "6 mm".into(), number: 1, length_offset: None, from: NumberFrom::FirstUse, dia: 6.0, shape: "flat".into() }) };
        for e in [Event::SpindleOn { rpm: 1000.0 }, Event::SpindleOff, Event::Dwell { seconds: 2.0, why: DwellWhy::SpinUp }, Event::Dwell { seconds: 0.1, why: DwellWhy::Requested },
                  Event::Stop { message: "x".into(), optional: false }] {
            assert!(e.halts(false) && e.halts(true), "{e:?}");
        }
        assert!(tool.halts(true) && !tool.halts(false));
        let arc = Event::Arc(Box::new(ArcTo { to: Xyz::new(Some(1.0), Some(0.0), None), center: [0.0, 0.0], cw: false, feed: 500.0, write_feed: true }));
        for e in [Event::Setup, Event::Comment { text: "Pass 1".into(), kind: CommentKind::Pass }, Event::Rapid { to: Xyz::new(None, None, Some(5.0)) },
                  Event::Feed { to: Xyz::NONE, feed: 800.0, write_feed: true }, Event::ProgramEnd { park: None }, arc,
                  Event::OperationStart { index: 0, name: "pocket".into() }, Event::OperationEnd { index: 0 }] {
            assert!(!e.halts(true), "{e:?}");
        }
    }

    #[test]
    fn only_the_operation_markers_are_markers() {
        assert!(Event::OperationStart { index: 2, name: "drill".into() }.is_marker() && Event::OperationEnd { index: 2 }.is_marker());
        for e in [Event::Setup, Event::SpindleOff, Event::Comment { text: "Operation 3: drill".into(), kind: CommentKind::Operation }, Event::Rapid { to: Xyz::NONE }] {
            assert!(!e.is_marker(), "{e:?}");
        }
    }

    #[test]
    fn the_default_meta_is_metric_g54() {
        let m = Program::default().meta;
        assert_eq!((m.units, m.wcs, m.numbered, m.arcs), (Units::Metric, "G54", false, false));
    }
}
