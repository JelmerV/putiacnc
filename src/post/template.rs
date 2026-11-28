//! Templates: one per handler, parsed and checked once when a package loads (never per event).
//!
//! A template is lines of text. In a line:
//! - `{X}` (a word from `[words]`: X Y Z I J F S P) is the letter and the value, or nothing; `{X:park}` is the park position
//!   (`on_close`); `{G}` is the motion code from `[modal] motion`.
//! - `{name}` is a variable (`{tool.number}`, `{message}`, `{prop.key}` ...), `{name|filter}` the same through a filter
//!   (`comment`, `plain`). `{{` and `}}` are braces.
//! - A placeholder that has no value leaves out the whitespace before it too; a line with placeholders of which none has a value is
//!   left out (a move to where the tool already is, a park the program does not have).
//! - `?name: rest` writes `rest` only when the property or flag `name` is true, `?!name: rest` only when it is false.
//! - A line that is `( ... )` as a whole is a comment, written in the post's comment style; a `( ... )` that ends a command line is
//!   an inline comment, written the same way.
//! - Blank lines and the whitespace around a line are left out.
use super::{PostError, PropDef, PropKind};

/// The event handlers of a package, in the order of `[templates]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Handler { Open, Section, SectionTool, SectionEnd, Rapid, Linear, Circular, Dwell, Command, Comment, Close }

impl Handler {
    pub const ALL: [Handler; 11] = [Handler::Open, Handler::Section, Handler::SectionTool, Handler::SectionEnd, Handler::Rapid, Handler::Linear,
                                    Handler::Circular, Handler::Dwell, Handler::Command, Handler::Comment, Handler::Close];
    pub fn key(self) -> &'static str {
        match self {
            Handler::Open => "on_open", Handler::Section => "on_section", Handler::SectionTool => "on_section_tool", Handler::SectionEnd => "on_section_end",
            Handler::Rapid => "on_rapid", Handler::Linear => "on_linear", Handler::Circular => "on_circular", Handler::Dwell => "on_dwell",
            Handler::Command => "on_command", Handler::Comment => "on_comment", Handler::Close => "on_close",
        }
    }
    pub fn idx(self) -> usize { self as usize }
}

/// A word the engine gives a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Word { X, Y, Z, I, J, F, S, P }

impl Word {
    pub const ALL: [Word; 8] = [Word::X, Word::Y, Word::Z, Word::I, Word::J, Word::F, Word::S, Word::P];
    pub fn letter(self) -> char { ['X', 'Y', 'Z', 'I', 'J', 'F', 'S', 'P'][self as usize] }
    pub fn from_letter(c: &str) -> Option<Word> { Word::ALL.into_iter().find(|w| c.len() == 1 && c.starts_with(w.letter())) }
    pub fn idx(self) -> usize { self as usize }
    /// The handlers where the word has a value.
    fn handlers(self, park: bool) -> &'static [Handler] {
        use Handler::*;
        match self {
            Word::X | Word::Y | Word::Z if park => &[Close],
            Word::X | Word::Y | Word::Z => &[Rapid, Linear, Circular],
            Word::I | Word::J => &[Circular],
            Word::F => &[Linear, Circular],
            Word::S => &[Command, SectionTool],
            Word::P => &[Dwell],
        }
    }
    fn meaning(self) -> &'static str {
        match self {
            Word::X | Word::Y | Word::Z => "the target position (on_rapid, on_linear, on_circular), or with :park the park position (on_close)",
            Word::I | Word::J => "the arc centre (on_circular)",
            Word::F => "the feed (on_linear, on_circular)",
            Word::S => "the spindle speed (on_command spindle_on, and on_section_tool: the speed the tool starts at)",
            Word::P => "the dwell time in the caps.dwell unit (on_dwell)",
        }
    }
}

/// A variable of a template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Var { Units, Wcs, Text, SectionNumber, SectionName, ToolNumber, ToolOffset, ToolName, NextToolNumber, NextToolOffset, NextToolName, Message, Prop(usize) }

/// The variables, and the handlers where each has a value (none listed: every handler).
const VARS: [(&str, Var, &[Handler]); 12] = [
    ("units", Var::Units, &[]),
    ("wcs", Var::Wcs, &[]),
    ("text", Var::Text, &[Handler::Comment]),
    ("section.number", Var::SectionNumber, &[Handler::Section, Handler::SectionEnd]),
    ("section.name", Var::SectionName, &[Handler::Section, Handler::SectionEnd]),
    ("tool.number", Var::ToolNumber, &[Handler::SectionTool]),
    ("tool.offset", Var::ToolOffset, &[Handler::SectionTool]),
    ("tool.name", Var::ToolName, &[Handler::SectionTool]),
    ("next_tool.number", Var::NextToolNumber, &[Handler::SectionTool]),
    ("next_tool.offset", Var::NextToolOffset, &[Handler::SectionTool]),
    ("next_tool.name", Var::NextToolName, &[Handler::SectionTool]),
    ("message", Var::Message, &[Handler::Command]),
];

/// A text filter on a variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Filter {
    None,
    /// Safe inside a comment: plain ASCII, `(` `)` become `-`, `″` and `"` become `in` (what tool names in comments always were).
    Comment,
    /// A plain message for controllers that read anything after `M0`: ASCII, no `;` `(` `)`, a space between a letter and a number
    /// (a reader would take "Z0" for a move), trimmed (Marlin's stop messages).
    Plain,
}

/// A flag for a `?name:` line, and the handler where it is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flag { Info, Operation, Pass, Warning, First, HasNext, SpinUp, Requested, SpindleOn, SpindleOff, Stop, OptionalStop, CoolantOn, CoolantOff }

const FLAGS: [(&str, Flag, Handler); 14] = [
    ("info", Flag::Info, Handler::Comment), ("operation", Flag::Operation, Handler::Comment), ("pass", Flag::Pass, Handler::Comment),
    ("warning", Flag::Warning, Handler::Comment), ("first", Flag::First, Handler::SectionTool), ("has_next", Flag::HasNext, Handler::SectionTool),
    ("spin_up", Flag::SpinUp, Handler::Dwell), ("requested", Flag::Requested, Handler::Dwell), ("spindle_on", Flag::SpindleOn, Handler::Command),
    ("spindle_off", Flag::SpindleOff, Handler::Command), ("stop", Flag::Stop, Handler::Command), ("optional_stop", Flag::OptionalStop, Handler::Command),
    ("coolant_on", Flag::CoolantOn, Handler::Command), ("coolant_off", Flag::CoolantOff, Handler::Command),
];

impl Flag {
    pub fn bit(self) -> u32 { 1 << (self as u32) }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Part {
    Lit(String),
    /// A word, with the whitespace before it (left out with the word when it has no value).
    Word { w: Word, park: bool, lead: String },
    /// The motion code.
    G { lead: String },
    Var { v: Var, filter: Filter, lead: String },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Cond { Prop(usize), Flag(Flag) }

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Line {
    /// `?name:` (the condition, and whether it is negated).
    pub cond: Option<(Cond, bool)>,
    /// The whole line is a comment: `parts` are what is inside the brackets.
    pub comment: bool,
    pub parts: Vec<Part>,
    /// An inline comment at the end: the whitespace before its `(`, and what is inside the brackets.
    pub inline: Option<(String, Vec<Part>)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Template { pub lines: Vec<Line> }

impl Template {
    pub fn is_empty(&self) -> bool { self.lines.is_empty() }
}

/// Where a template comes from, for its errors.
pub(crate) struct Origin<'a> {
    pub file: &'a str,
    /// The file line of the template's first line (`None` for the engine's own defaults).
    pub line: Option<usize>,
}

fn err(o: &Origin, i: usize, msg: String) -> PostError { PostError { file: o.file.to_string(), line: o.line.map(|l| l + i), message: msg } }

/// Parses and checks the template of handler `h`. `props` are the package's properties.
pub(crate) fn parse(src: &str, h: Handler, props: &[PropDef], o: &Origin) -> Result<Template, PostError> {
    let mut lines = Vec::new();
    for (i, raw) in src.lines().enumerate() {
        let mut body = raw.trim();
        if body.is_empty() { continue; }
        let mut cond = None;
        if let Some(rest) = body.strip_prefix('?') {
            let (neg, rest) = match rest.strip_prefix('!') { Some(r) => (true, r), None => (false, rest) };
            let Some((name, after)) = rest.split_once(':') else {
                return Err(err(o, i, format!("{}: a ? line needs a name and a colon, like `?stop: M0`", h.key())));
            };
            let name = name.trim();
            let c = if let Some(f) = FLAGS.iter().find(|f| f.0 == name) {
                if f.2 != h { return Err(err(o, i, format!("{}: the flag ?{name} is only set in {}; flags here: {}", h.key(), f.2.key(), flag_list(h)))); }
                Cond::Flag(f.1)
            } else if let Some(p) = props.iter().position(|p| p.key == name) {
                if props[p].kind != PropKind::Bool { return Err(err(o, i, format!("{}: ?{name} needs a true/false property, but `{name}` is not one (give it type = \"bool\")", h.key()))); }
                Cond::Prop(p)
            } else {
                return Err(err(o, i, format!("{}: ?{name} is not a property or a flag: declare it under [properties] with type = \"bool\" (flags here: {})", h.key(), flag_list(h))));
            };
            body = after.trim();
            if body.is_empty() { return Err(err(o, i, format!("{}: nothing to write after ?{name}:", h.key()))); }
            cond = Some((c, neg));
        }
        let line = if body.len() >= 2 && body.starts_with('(') && body.ends_with(')') {
            Line { cond, comment: true, parts: parts(&body[1..body.len() - 1], h, props, o, i)?, inline: None }
        } else if let Some(p) = inline_start(body) {
            let cmd = &body[..p];
            let trimmed = cmd.trim_end();
            Line { cond, comment: false, parts: parts(trimmed, h, props, o, i)?, inline: Some((cmd[trimmed.len()..].to_string(), parts(&body[p + 1..body.len() - 1], h, props, o, i)?)) }
        } else {
            Line { cond, comment: false, parts: parts(body, h, props, o, i)?, inline: None }
        };
        lines.push(line);
    }
    Ok(Template { lines })
}

fn flag_list(h: Handler) -> String {
    let v: Vec<&str> = FLAGS.iter().filter(|f| f.2 == h).map(|f| f.0).collect();
    if v.is_empty() { "none (only true/false properties)".into() } else { v.join(", ") }
}

/// Where the inline comment of a command line starts: the `(` that matches the line's last `)`, when text comes before it.
fn inline_start(body: &str) -> Option<usize> {
    if !body.ends_with(')') { return None; }
    let mut depth = 0;
    for (i, c) in body.char_indices().rev() {
        match c {
            ')' => depth += 1,
            '(' => { depth -= 1; if depth == 0 { return (!body[..i].trim().is_empty()).then_some(i); } }
            _ => {}
        }
    }
    None
}

/// The parts of a line (or of a comment's inside).
fn parts(s: &str, h: Handler, props: &[PropDef], o: &Origin, i: usize) -> Result<Vec<Part>, PostError> {
    let mut out: Vec<Part> = Vec::new();
    let mut lit = String::new();
    let mut it = s.char_indices().peekable();
    while let Some((at, c)) = it.next() {
        match c {
            '{' if it.peek().map(|p| p.1) == Some('{') => { it.next(); lit.push('{'); }
            '}' if it.peek().map(|p| p.1) == Some('}') => { it.next(); lit.push('}'); }
            '}' => return Err(err(o, i, format!("{}: a }} with no {{ before it; write }}}} for a brace", h.key()))),
            '{' => {
                let Some(end) = s[at..].find('}') else { return Err(err(o, i, format!("{}: a {{ is not closed; write {{{{ for a brace", h.key()))) };
                let name = &s[at + 1..at + end];
                while it.peek().is_some_and(|p| p.0 < at + end + 1) { it.next(); }
                // the whitespace before a placeholder goes with it
                let keep = lit.trim_end().len();
                let lead = lit[keep..].to_string();
                lit.truncate(keep);
                let part = placeholder(name, lead, h, props, o, i)?;
                // `S{S}` would write the letter twice
                if let Part::Word { w, lead, .. } = &part {
                    if lead.is_empty() && lit.ends_with(w.letter()) {
                        return Err(err(o, i, format!("{}: {{{name}}} writes its letter itself: write {{{name}}}, not {}{{{name}}}", h.key(), w.letter())));
                    }
                }
                if !lit.is_empty() { out.push(Part::Lit(std::mem::take(&mut lit))); }
                out.push(part);
            }
            c => lit.push(c),
        }
    }
    if !lit.is_empty() { out.push(Part::Lit(lit)); }
    Ok(out)
}

fn placeholder(name: &str, lead: String, h: Handler, props: &[PropDef], o: &Origin, i: usize) -> Result<Part, PostError> {
    let hk = h.key();
    // a word: one capital letter, maybe with a variant
    let (base, variant) = match name.split_once(':') { Some((a, b)) => (a, Some(b)), None => (name, None) };
    if base.len() == 1 && base.chars().all(|c| c.is_ascii_uppercase()) {
        if base == "G" {
            if variant.is_some() { return Err(err(o, i, format!("{hk}: {{{name}}}: the motion code {{G}} has no variants"))); }
            if !matches!(h, Handler::Rapid | Handler::Linear | Handler::Circular) {
                return Err(err(o, i, format!("{hk}: {{G}} is the motion code of a move; it has a value only in on_rapid, on_linear and on_circular (write the code itself, e.g. G0, elsewhere)")));
            }
            return Ok(Part::G { lead });
        }
        let Some(w) = Word::from_letter(base) else {
            return Err(err(o, i, format!("{hk}: {{{name}}} is not a word the engine knows: the words are X Y Z I J F S P, and {{G}} the motion code")));
        };
        let park = match variant {
            None => false,
            Some("park") if matches!(w, Word::X | Word::Y | Word::Z) => true,
            Some(v) => return Err(err(o, i, format!("{hk}: {{{name}}}: unknown variant :{v}; the only variant is :park, of X, Y and Z (on_close)"))),
        };
        if !w.handlers(park).contains(&h) {
            return Err(err(o, i, format!("{hk}: {{{name}}} has no value here: {} is {}", w.letter(), w.meaning())));
        }
        return Ok(Part::Word { w, park, lead });
    }
    let (vname, filter) = match name.split_once('|') {
        Some((a, "comment")) => (a, Filter::Comment),
        Some((a, "plain")) => (a, Filter::Plain),
        Some((_, f)) => return Err(err(o, i, format!("{hk}: {{{name}}}: unknown filter |{f}; the filters are |comment and |plain"))),
        None => (name, Filter::None),
    };
    if variant.is_some() && filter == Filter::None { return Err(err(o, i, format!("{hk}: {{{name}}}: only X, Y and Z have a variant (:park)"))); }
    if let Some(key) = vname.strip_prefix("prop.") {
        let Some(p) = props.iter().position(|p| p.key == key) else {
            return Err(err(o, i, format!("{hk}: {{{name}}}: there is no property `{key}`; declare it under [properties]")));
        };
        return Ok(Part::Var { v: Var::Prop(p), filter, lead });
    }
    let Some(&(_, v, hs)) = VARS.iter().find(|x| x.0 == vname) else {
        let all: Vec<&str> = VARS.iter().map(|x| x.0).collect();
        return Err(err(o, i, format!("{hk}: {{{name}}} is not a variable; the variables are {}, and prop.<name> for a property", all.join(", "))));
    };
    if !hs.is_empty() && !hs.contains(&h) {
        let w: Vec<&str> = hs.iter().map(|h| h.key()).collect();
        return Err(err(o, i, format!("{hk}: {{{vname}}} has no value here; it is set in {}", w.join(", "))));
    }
    Ok(Part::Var { v, filter, lead })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::post::PropValue;

    fn props() -> Vec<PropDef> {
        vec![PropDef { key: "stop_first".into(), title: "Stop first".into(), kind: PropKind::Bool, default: PropValue::Bool(true) },
             PropDef { key: "park_z".into(), title: "Park Z".into(), kind: PropKind::Number, default: PropValue::Number(30.0) }]
    }
    fn p(src: &str, h: Handler) -> Result<Template, PostError> { parse(src, h, &props(), &Origin { file: "t.toml", line: Some(10) }) }
    fn e(src: &str, h: Handler) -> String { let r = p(src, h).unwrap_err(); assert_eq!(r.file, "t.toml"); r.message }

    #[test]
    fn words_take_the_whitespace_before_them() {
        let t = p("{G} {X} {Y}{Z} F{{x}}", Handler::Linear).unwrap();
        assert_eq!(t.lines[0].parts, vec![Part::G { lead: "".into() }, Part::Word { w: Word::X, park: false, lead: " ".into() },
            Part::Word { w: Word::Y, park: false, lead: " ".into() }, Part::Word { w: Word::Z, park: false, lead: "".into() }, Part::Lit(" F{x}".into())]);
    }

    #[test]
    fn lines_conditions_comments_and_inline_comments() {
        let t = p("  \n?stop_first: M5\n?!first: T{tool.number} M6 (Tool {tool.number}: {tool.name|comment})\n(Tool: {tool.name})\nM0 ; text (no comment\n", Handler::SectionTool).unwrap();
        assert_eq!(t.lines.len(), 4, "the blank line is left out");
        assert_eq!(t.lines[0].cond, Some((Cond::Prop(0), false)));
        assert_eq!(t.lines[0].parts, vec![Part::Lit("M5".into())]);
        let l = &t.lines[1];
        assert_eq!(l.cond, Some((Cond::Flag(Flag::First), true)));
        assert_eq!(l.parts, vec![Part::Lit("T".into()), Part::Var { v: Var::ToolNumber, filter: Filter::None, lead: "".into() }, Part::Lit(" M6".into())]);
        let (gap, inner) = l.inline.as_ref().unwrap();
        assert_eq!(gap, " ");
        assert_eq!(inner[3], Part::Var { v: Var::ToolName, filter: Filter::Comment, lead: " ".into() });
        assert!(t.lines[2].comment && t.lines[2].inline.is_none());
        assert!(!t.lines[3].comment && t.lines[3].inline.is_none(), "an unclosed bracket is text");
        // a whole-line comment with brackets inside is one comment
        assert!(p("(a) b (c)", Handler::Open).unwrap().lines[0].comment);
        assert_eq!(p("{X:park}", Handler::Close).unwrap().lines[0].parts, vec![Part::Word { w: Word::X, park: true, lead: "".into() }]);
        assert_eq!(p("Z{prop.park_z}", Handler::Close).unwrap().lines[0].parts[1], Part::Var { v: Var::Prop(1), filter: Filter::None, lead: "".into() });
    }

    #[test]
    fn every_mistake_is_named_with_its_line() {
        let r = p("G0\n\n{Q}", Handler::Rapid).unwrap_err();
        assert_eq!(r.line, Some(12), "the template's third line");
        assert!(r.message.contains("{Q} is not a word"), "{}", r.message);
        assert!(e("{X}", Handler::Comment).contains("{X} has no value here"));
        assert!(e("{X:park}", Handler::Rapid).contains("has no value here"));
        assert!(e("{X:home}", Handler::Close).contains("unknown variant :home"));
        assert!(e("{G}", Handler::Close).contains("motion code"));
        assert!(e("{G:park}", Handler::Rapid).contains("no variants"));
        assert!(e("{tool.number}", Handler::Rapid).contains("set in on_section_tool"));
        assert!(e("{tool.colour}", Handler::SectionTool).contains("is not a variable"));
        assert!(e("{message|loud}", Handler::Command).contains("unknown filter |loud"));
        assert!(e("{prop.nope}", Handler::Open).contains("no property `nope`"));
        assert!(e("?nope: M5", Handler::Open).contains("not a property or a flag"));
        assert!(e("?park_z: M5", Handler::Open).contains("true/false property"));
        assert!(e("?stop: M0", Handler::Open).contains("only set in on_command"));
        assert!(e("?stop M0", Handler::Command).contains("needs a name and a colon"));
        assert!(e("?stop:", Handler::Command).contains("nothing to write"));
        assert!(e("G0 {X", Handler::Rapid).contains("not closed"));
        assert!(e("G0 X}", Handler::Rapid).contains("with no {"));
        assert!(e("{text:park}", Handler::Comment).contains("only X, Y and Z have a variant"));
        assert!(e("G4 P{P}", Handler::Dwell).contains("writes its letter itself"));
        assert!(e("{S}M3 S{S}", Handler::Command).contains("not S{S}"));
        p("G4 {P}\nM3{S}", Handler::Command).unwrap_err();
        p("G4 {P}", Handler::Dwell).unwrap();
        // the engine's own defaults have no file line
        let d = parse("{Q}", Handler::Rapid, &[], &Origin { file: "builtin", line: None }).unwrap_err();
        assert_eq!(d.line, None);
    }
}
