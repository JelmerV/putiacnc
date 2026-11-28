//! A post package: the TOML manifest read, checked and turned into what the engine runs ([`Package`]). Everything is checked here,
//! once: unknown keys, sections, words, formats, properties, handlers, variables and flags are errors that name the file, the line
//! and the fix, and so is a `format_version` this engine does not know.
use super::template::{self, Handler, Origin, Template, Word};
use super::text::Format;
use super::{ArcSupport, Caps, CommentStyle, DwellUnit, LineEnding, PostError, PropDef, PropKind, PropValue, SpindleCtl, ToolChangeMode};
use serde::Deserialize;
use std::collections::BTreeMap;
use toml::Spanned;

/// The manifest versions this engine reads.
pub const FORMAT_VERSION: i64 = 1;

/// How a word decides whether it is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Written when the event gives it: an axis when the move's `Xyz` gives it (the generator decided, comparing the raw values),
    /// the feed when the `Feed` says `write_feed`; any other word whenever it has a value. The built-ins' rule: their text is what
    /// the generator decided, by construction.
    AsGiven,
    /// Written when its text differs from what this word last wrote, and after a section start, a tool change or a stop.
    Modal,
    /// Written whenever it has a value (an axis: the full position on every move).
    Always,
}

/// A word of a package: its number format and mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WordDef { pub format: Format, pub mode: Mode }

/// A loaded, checked post package; the engine (`engine.rs`) renders programs with it.
#[derive(Debug, Clone)]
pub struct Package {
    pub(crate) file: String,
    pub(crate) name: String,
    pub(crate) extension: String,
    pub(crate) line_ending: LineEnding,
    pub(crate) caps: Caps,
    pub(crate) props: Vec<PropDef>,
    /// By `Word::idx`.
    pub(crate) words: [WordDef; 8],
    /// The motion codes G0 G1 G2 G3 that are written every time (the others only when the motion changes).
    pub(crate) always: [bool; 4],
    /// By `Handler::idx`.
    pub(crate) templates: Vec<Template>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    post: PostToml,
    #[serde(default)]
    caps: CapsToml,
    #[serde(default)]
    properties: BTreeMap<String, Spanned<PropToml>>,
    #[serde(default)]
    format: BTreeMap<String, Spanned<FormatToml>>,
    #[serde(default)]
    words: BTreeMap<String, Spanned<WordToml>>,
    #[serde(default)]
    modal: BTreeMap<String, Spanned<ModalToml>>,
    #[serde(default)]
    templates: TemplatesToml,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PostToml {
    #[allow(dead_code)]
    format_version: i64,
    name: String,
    #[serde(default)]
    extension: Option<String>,
    #[serde(default)]
    line_ending: Option<LineEnding>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct CapsToml {
    arcs: Option<ArcSupport>,
    max_arc_sweep: Option<Spanned<f64>>,
    tool_change: Option<ToolChangeMode>,
    length_offset: Option<bool>,
    spindle: Option<SpindleCtl>,
    dwell: Option<DwellUnit>,
    comments: Option<CommentStyle>,
    wcs_count: Option<Spanned<u8>>,
    max_line_len: Option<usize>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Value { Bool(bool), Int(i64), Number(f64), Text(String) }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PropToml {
    #[serde(rename = "type")]
    kind: String,
    default: Value,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    options: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FormatToml { decimals: Option<u32>, inch_decimals: Option<u32>, trim: Option<bool>, length: Option<bool> }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WordToml { format: Option<String>, mode: Option<Mode>, modal: Option<bool>, incremental: Option<bool> }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModalToml { codes: Vec<String>, #[serde(default)] always: Vec<String> }

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct TemplatesToml {
    on_open: Option<Spanned<String>>,
    on_section: Option<Spanned<String>>,
    on_section_tool: Option<Spanned<String>>,
    on_section_end: Option<Spanned<String>>,
    on_rapid: Option<Spanned<String>>,
    on_linear: Option<Spanned<String>>,
    on_circular: Option<Spanned<String>>,
    on_dwell: Option<Spanned<String>>,
    on_command: Option<Spanned<String>>,
    on_comment: Option<Spanned<String>>,
    on_close: Option<Spanned<String>>,
}

impl TemplatesToml {
    fn get(&self, h: Handler) -> Option<&Spanned<String>> {
        match h {
            Handler::Open => self.on_open.as_ref(), Handler::Section => self.on_section.as_ref(), Handler::SectionTool => self.on_section_tool.as_ref(),
            Handler::SectionEnd => self.on_section_end.as_ref(), Handler::Rapid => self.on_rapid.as_ref(), Handler::Linear => self.on_linear.as_ref(),
            Handler::Circular => self.on_circular.as_ref(), Handler::Dwell => self.on_dwell.as_ref(), Handler::Command => self.on_command.as_ref(),
            Handler::Comment => self.on_comment.as_ref(), Handler::Close => self.on_close.as_ref(),
        }
    }
}

/// Just the version, read first: a newer manifest may have keys this engine does not know, and the version is the error to report.
#[derive(Deserialize)]
struct VersionOnly { post: Option<VersionPost> }
#[derive(Deserialize)]
struct VersionPost { format_version: Option<Spanned<i64>> }

/// The default formats: what the built-ins write (3 decimals in mm, 4 in inches; feeds 1 and 2; speeds and dwells as they are).
fn default_formats() -> BTreeMap<String, Format> {
    let mut f = BTreeMap::new();
    f.insert("xyz".into(), Format { decimals: Some(3), inch_decimals: Some(4), trim: true, length: true });
    f.insert("feed".into(), Format { decimals: Some(1), inch_decimals: Some(2), trim: true, length: true });
    f.insert("rpm".into(), Format { decimals: None, inch_decimals: None, trim: true, length: false });
    f.insert("seconds".into(), Format { decimals: None, inch_decimals: None, trim: true, length: false });
    f
}

/// The default word of a letter: its format name and mode.
fn default_word(w: Word) -> (&'static str, Mode) {
    match w {
        Word::X | Word::Y | Word::Z => ("xyz", Mode::AsGiven),
        Word::I | Word::J => ("xyz", Mode::Always),
        Word::F => ("feed", Mode::AsGiven),
        Word::S => ("rpm", Mode::Always),
        Word::P => ("seconds", Mode::Always),
    }
}

/// The template of a handler a package leaves out (the spec's defaults, for these capabilities).
fn default_template(h: Handler, caps: &Caps) -> String {
    match h {
        Handler::Open => "{units} G90 G17 {wcs}".into(),
        Handler::Section | Handler::SectionEnd => String::new(),
        Handler::SectionTool => match (caps.tool_change, caps.length_offset) {
            (ToolChangeMode::Manual, _) => "(Tool: {tool.name|comment})".into(),
            (ToolChangeMode::Atc, false) => "T{tool.number} M6 (Tool {tool.number}: {tool.name|comment})".into(),
            (ToolChangeMode::Atc, true) => "T{tool.number} M6 (Tool {tool.number}: {tool.name|comment})\nG43 H{tool.offset}".into(),
        },
        Handler::Rapid => "{G} {X} {Y} {Z}".into(),
        Handler::Linear => "{G} {X} {Y} {Z} {F}".into(),
        Handler::Circular => "{G} {X} {Y} {Z} {I} {J} {F}".into(),
        Handler::Dwell => "G4 {P}".into(),
        Handler::Command => "?spindle_on: M3 {S}\n?spindle_off: M5\n?stop: M0 ({message|comment})\n?optional_stop: M1 ({message|comment})\n?coolant_on: M8\n?coolant_off: M9".into(),
        Handler::Comment => "({text})".into(),
        Handler::Close => "G0 {X:park} {Y:park}\nM30".into(),
    }
}

fn line_of(src: &str, at: usize) -> usize { src[..at.min(src.len())].matches('\n').count() + 1 }

impl Package {
    /// Reads a package. `file` names it in errors ("builtin:grbl/post.toml", or the path of the manifest).
    pub fn load(file: &str, src: &str) -> Result<Package, PostError> {
        let fail = |at: Option<usize>, message: String| PostError { file: file.to_string(), line: at.map(|a| line_of(src, a)), message };
        let toml_err = |e: toml::de::Error, what: &str| {
            let msg = e.message().trim().to_string();
            let fix = if msg.contains("unknown field") { "; check the spelling (docs/posts/authoring.md lists every key)" }
                else if msg.contains("missing field") { "; add it" } else { "" };
            fail(e.span().map(|s| s.start), format!("{what}{msg}{fix}"))
        };
        // the version first
        let v: VersionOnly = toml::from_str(src).map_err(|e| toml_err(e, ""))?;
        match v.post.as_ref().and_then(|p| p.format_version.as_ref()) {
            None => return Err(fail(None, format!("[post] has no format_version: add `format_version = {FORMAT_VERSION}` under [post]"))),
            Some(fv) if *fv.get_ref() != FORMAT_VERSION => {
                return Err(fail(Some(fv.span().start), format!("format_version {} is not one this PutiaCNC reads (it reads {FORMAT_VERSION}): use a newer PutiaCNC, or write the package for version {FORMAT_VERSION}", fv.get_ref())));
            }
            Some(_) => {}
        }
        let m: Manifest = toml::from_str(src).map_err(|e| toml_err(e, ""))?;
        if m.post.name.trim().is_empty() { return Err(fail(None, "[post] name is empty: give the post a name".into())); }

        let d = Caps::default();
        let c = &m.caps;
        let sweep = c.max_arc_sweep.as_ref().map(|s| (*s.get_ref(), s.span().start));
        if let Some((s, at)) = sweep {
            if !(s > 0.0 && s <= 180.0) { return Err(fail(Some(at), format!("caps.max_arc_sweep = {s}: give the degrees of the longest arc, more than 0 and at most 180"))); }
        }
        let wcs = c.wcs_count.as_ref().map(|w| (*w.get_ref(), w.span().start));
        if let Some((w, at)) = wcs {
            if !(1..=9).contains(&w) { return Err(fail(Some(at), format!("caps.wcs_count = {w}: give 1 (one fixed system) to 9 (G54 to G59.3)"))); }
        }
        let caps = Caps {
            arcs: c.arcs.unwrap_or(d.arcs), max_arc_sweep: sweep.map_or(d.max_arc_sweep, |s| s.0), tool_change: c.tool_change.unwrap_or(d.tool_change),
            length_offset: c.length_offset.unwrap_or(d.length_offset), spindle: c.spindle.unwrap_or(d.spindle), dwell: c.dwell.unwrap_or(d.dwell),
            comments: c.comments.unwrap_or(d.comments), wcs_count: wcs.map_or(d.wcs_count, |w| w.0), max_line_len: c.max_line_len.or(d.max_line_len),
        };

        // properties
        let mut props = Vec::new();
        for (key, p) in &m.properties {
            let at = Some(p.span().start);
            let p = p.get_ref();
            if !key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') || !key.starts_with(|c: char| c.is_ascii_lowercase()) {
                return Err(fail(at, format!("property `{key}`: a name is lowercase letters, digits and _, starting with a letter")));
            }
            let bad = |want: &str| fail(at, format!("property `{key}`: the default must be {want}"));
            let (kind, default) = match (p.kind.as_str(), &p.default) {
                ("bool", Value::Bool(b)) => (PropKind::Bool, PropValue::Bool(*b)),
                ("bool", _) => return Err(bad("true or false")),
                ("int", Value::Int(i)) => (PropKind::Int, PropValue::Int(*i)),
                ("int", _) => return Err(bad("a whole number")),
                ("number", Value::Int(i)) => (PropKind::Number, PropValue::Number(*i as f64)),
                ("number", Value::Number(n)) => (PropKind::Number, PropValue::Number(*n)),
                ("number", _) => return Err(bad("a number")),
                ("text", Value::Text(t)) => (PropKind::Text, PropValue::Text(t.clone())),
                ("text", _) => return Err(bad("a text in quotes")),
                ("enum", Value::Text(t)) => {
                    let opts = p.options.clone().unwrap_or_default();
                    if opts.is_empty() { return Err(fail(at, format!("property `{key}`: an enum needs options = [\"a\", \"b\"]"))); }
                    if !opts.contains(t) { return Err(bad(&format!("one of the options ({})", opts.join(", ")))); }
                    (PropKind::Enum(opts), PropValue::Text(t.clone()))
                }
                ("enum", _) => return Err(bad("one of the options, in quotes")),
                (k, _) => return Err(fail(at, format!("property `{key}`: unknown type \"{k}\"; the types are bool, int, number, text and enum"))),
            };
            if p.options.is_some() && !matches!(kind, PropKind::Enum(_)) { return Err(fail(at, format!("property `{key}`: only an enum has options"))); }
            props.push(PropDef { key: key.clone(), title: p.title.clone().unwrap_or_else(|| key.clone()), kind, default });
        }

        // formats and words
        let mut formats = default_formats();
        for (name, f) in &m.format {
            let at = Some(f.span().start);
            let f = f.get_ref();
            for d in [f.decimals, f.inch_decimals].into_iter().flatten() {
                if d > 9 { return Err(fail(at, format!("format `{name}`: {d} decimals is too many; give 0 to 9"))); }
            }
            let trim = f.trim.unwrap_or(true);
            if !trim && f.decimals.is_none() { return Err(fail(at, format!("format `{name}`: trim = false keeps trailing zeros, so it needs decimals = …"))); }
            if f.inch_decimals.is_some() && f.decimals.is_none() { return Err(fail(at, format!("format `{name}`: inch_decimals needs decimals (for mm) too"))); }
            formats.insert(name.clone(), Format { decimals: f.decimals, inch_decimals: f.inch_decimals, trim, length: f.length.unwrap_or(false) });
        }
        let mut words = Word::ALL.map(|w| { let (f, mode) = default_word(w); WordDef { format: formats[f], mode } });
        for (letter, wt) in &m.words {
            let at = Some(wt.span().start);
            let wt = wt.get_ref();
            let Some(w) = Word::from_letter(letter) else {
                let why = match letter.as_str() {
                    "G" => "G is the motion code: give its group under [modal] as `motion`",
                    "N" => "line numbers are a setting of the machine, not a word",
                    "T" | "H" => "write the tool and offset numbers with {tool.number} and {tool.offset}",
                    _ => "the words are X Y Z I J F S P",
                };
                return Err(fail(at, format!("[words] {letter}: not a word the engine knows; {why}")));
            };
            let fname = wt.format.clone().unwrap_or_else(|| default_word(w).0.to_string());
            let Some(format) = formats.get(&fname).copied() else {
                return Err(fail(at, format!("[words] {letter}: no format `{fname}`; declare it under [format] (formats: {})", formats.keys().cloned().collect::<Vec<_>>().join(", "))));
            };
            let mode = match (wt.mode, wt.modal) {
                (Some(_), Some(_)) => return Err(fail(at, format!("[words] {letter}: give mode or modal, not both"))),
                (Some(m), None) => m,
                (None, Some(true)) => Mode::Modal,
                (None, Some(false)) | (None, None) => Mode::Always,
            };
            match (w, wt.incremental) {
                (Word::I | Word::J, Some(false)) => return Err(fail(at, format!("[words] {letter}: absolute arc centres are not supported; I and J are always from the arc's start (incremental = true)"))),
                (Word::I | Word::J, _) | (_, None) => {}
                (_, Some(_)) => return Err(fail(at, format!("[words] {letter}: incremental is only for I and J"))),
            }
            words[w.idx()] = WordDef { format, mode };
        }

        // the motion group
        let mut always = [true; 4];
        const CODES: [&str; 4] = ["G0", "G1", "G2", "G3"];
        for (name, g) in &m.modal {
            let at = Some(g.span().start);
            if name != "motion" { return Err(fail(at, format!("[modal] {name}: the engine knows one group, `motion` (G0 G1 G2 G3)"))); }
            let g = g.get_ref();
            let mut codes = g.codes.clone();
            codes.sort();
            if codes != CODES { return Err(fail(at, "[modal] motion: codes must be G0, G1, G2 and G3".into())); }
            for a in &g.always {
                if !CODES.contains(&a.as_str()) { return Err(fail(at, format!("[modal] motion: `{a}` in always is not one of its codes"))); }
            }
            always = CODES.map(|c| g.always.iter().any(|a| a == c));
        }

        // templates: the package's, or the defaults for its capabilities
        let mut templates = Vec::with_capacity(Handler::ALL.len());
        for h in Handler::ALL {
            let t = match m.templates.get(h) {
                Some(s) => {
                    let start = s.span().start;
                    let raw = &src[start..s.span().end.min(src.len())];
                    // a multi-line string starts on the line after its opening quotes
                    let first = line_of(src, start) + usize::from((raw.starts_with("\"\"\"") || raw.starts_with("'''")) && raw[3..].starts_with(['\n', '\r']));
                    template::parse(s.get_ref(), h, &props, &Origin { file, line: Some(first) })?
                }
                None => template::parse(&default_template(h, &caps), h, &props, &Origin { file, line: None })?,
            };
            templates.push(t);
        }
        if caps.arcs != ArcSupport::None && templates[Handler::Circular.idx()].is_empty() {
            return Err(fail(m.templates.on_circular.as_ref().map(|s| s.span().start), "on_circular is empty, but caps.arcs says the controller does arcs: write the arc move, or set arcs = \"none\"".into()));
        }

        Ok(Package {
            file: file.to_string(), name: m.post.name.clone(), extension: m.post.extension.clone().unwrap_or_else(|| "nc".into()),
            line_ending: m.post.line_ending.unwrap_or_default(), caps, props, words, always, templates,
        })
    }

    pub fn file(&self) -> &str { &self.file }
    pub fn extension(&self) -> &str { &self.extension }
    pub fn line_ending(&self) -> LineEnding { self.line_ending }
}

impl super::Post for Package {
    fn name(&self) -> &str { &self.name }
    fn caps(&self) -> &Caps { &self.caps }
    fn properties(&self) -> &[PropDef] { &self.props }
    fn render(&self, prog: &crate::program::Program, ctx: &super::PostCtx) -> Result<super::Rendered, PostError> { self.render_program(prog, ctx) }
}
