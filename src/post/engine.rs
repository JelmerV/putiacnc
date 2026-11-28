//! The post engine: a [`Program`] rendered with a [`Package`].
//!
//! Steps, in the order the generator ran them before the IR:
//! 1. **Arcs.** Runs of short cuts are fitted as arcs (`arcs::fit_items`) on the numbers as the post writes them (in the output
//!    units, rounded by the X/Y format): over the whole program with `Meta.arcs`, else over the trochoidal ranges only; nothing when
//!    `caps.arcs = "none"` (with a note). Fitting comes before rendering, so that a post with modal words renders the arcs in order.
//! 2. **Events.** Each event through its handler's template, in order: a fitted arc through `on_circular`, in place of the cuts it
//!    replaces. The engine keeps the full current position in work mm (for `{X:park}`, modal axes and an `Arc` event's centre),
//!    what each modal word last wrote and the last motion code.
//! 3. **The machine's start and end code**: the start code before the first line of the event at `Meta.start_at`, the end code
//!    before the last line, whole-line `(…)` comments in the comment style.
//! 4. **Line numbers** (`N10`, step 10) over everything, when `Meta.numbered`.
//!
//! A source map follows the lines: the event of each line (a fitted arc: the first cut it replaces; start and end code: none).
use super::manifest::{Mode, Package};
use super::template::{Cond, Filter, Flag, Handler, Line, Part, Var, Word};
use super::{text, ArcSupport, CommentStyle, DwellUnit, PostCtx, PostError, PropKind, PropValue, Rendered, SpindleCtl};
use crate::arcs::{self, Item};
use crate::i18n::tr;
use crate::model::Units;
use crate::program::{CommentKind, DwellWhy, Event, Program, ToolRef, Xyz};
use std::borrow::Cow;

/// A fitted arc, in event terms: it replaces the cuts `first..=last`, ends at `end` and has its centre `ij` from its start (both in
/// the output units, as the fitter read and rounded them).
struct Fit { first: usize, last: usize, end: [f64; 2], ij: [f64; 2], cw: bool }

/// What a handler sees of one event.
#[derive(Default)]
struct Ev<'a> {
    flags: u32,
    /// Moves: the target position in work mm (the axes not given: the current position), and which axes are given.
    target: [Option<f64>; 3],
    given: u8,
    /// A fitted arc: its end X and Y and its centre, already in the output units.
    end_units: Option<[f64; 2]>,
    ij: Option<([f64; 2], bool)>,
    motion: usize,
    /// The feed (mm/min) and whether the event gives it.
    feed: Option<(f64, bool)>,
    rpm: Option<f64>,
    dwell: Option<f64>,
    park: Option<&'a Xyz>,
    text: Option<&'a str>,
    message: Option<&'a str>,
    section: Option<(usize, &'a str)>,
    tool: Option<&'a ToolRef>,
    next_tool: Option<&'a ToolRef>,
}

/// What the engine remembers between events.
#[derive(Default, Clone)]
struct State {
    pos: [Option<f64>; 3],
    /// The text each modal word last wrote (`None`: forgotten, the next one is written).
    last: [Option<String>; 8],
    motion: Option<usize>,
    /// The name of the open section (for `on_section_end`).
    section: (usize, String),
}

impl State {
    fn forget(&mut self) { for l in self.last.iter_mut() { *l = None; } self.motion = None; }
    fn moved(&mut self, to: &Xyz) { for i in 0..3 { if let Some(v) = to.get(i) { self.pos[i] = Some(v); } } }
}

struct Renderer<'a> {
    pkg: &'a Package,
    prog: &'a Program,
    props: &'a [PropValue],
    inch: bool,
    k: f64,
    st: State,
    lines: Vec<String>,
    source: Vec<Option<usize>>,
    /// Whether each line is a comment (only kept while reading lines for the arc fitter).
    comments: Option<Vec<bool>>,
    /// The events that are tool changes, in order, and how many have been rendered.
    tools: &'a [usize],
    tool_k: usize,
    scratch: String,
}

const MOTION: [&str; 4] = ["G0", "G1", "G2", "G3"];

impl Package {
    pub(crate) fn render_program(&self, prog: &Program, ctx: &PostCtx) -> Result<Rendered, PostError> {
        let m = &prog.meta;
        let props = self.prop_values(ctx)?;
        let inch = m.units == Units::Imperial;
        let k = m.units.factor();
        let tools: Vec<usize> = prog.events.iter().enumerate().filter(|(_, e)| matches!(e, Event::ToolChange { .. })).map(|(i, _)| i).collect();
        let mut r = Renderer { pkg: self, prog, props: &props, inch, k, st: State::default(), lines: Vec::with_capacity(prog.events.len() + 8),
                               source: Vec::with_capacity(prog.events.len() + 8), comments: None, tools: &tools, tool_k: 0, scratch: String::new() };
        let mut notes = Vec::new();
        let (fits, troch_arcs, arcs) = r.plan_arcs(&mut notes)?;

        let ev = &prog.events;
        let mut fi = 0;
        let mut i = 0;
        while i < ev.len() {
            if let Some(f) = fits.get(fi).filter(|f| f.first == i) {
                r.arc(f)?;
                // the cuts it replaces move the tool; anything between them writes no line (it was not a line for the fitter either)
                for (j, e) in ev.iter().enumerate().take(f.last + 1).skip(f.first) {
                    match e { Event::Feed { to, .. } => r.st.moved(to), _ => r.event(j)? }
                }
                i = f.last + 1;
                fi += 1;
                continue;
            }
            r.event(i)?;
            i += 1;
        }
        let (mut lines, mut source) = (r.lines, r.source);

        // the machine's own lines, after the fitting so it never sees them: before the event at start_at, and before the last line
        let style = self.caps.comments;
        let conv = |v: &[String]| -> Vec<String> { v.iter().filter_map(|l| text::machine_line(style, l)).collect() };
        let (start_code, end_code) = (conv(&m.start_code), conv(&m.end_code));
        if !start_code.is_empty() || !end_code.is_empty() {
            let start = source.partition_point(|s| s.is_some_and(|e| e < m.start_at));
            let last = lines.len().saturating_sub(1);
            for (j, l) in end_code.into_iter().enumerate() { lines.insert(last + j, l); source.insert(last + j, None); }
            for (j, l) in start_code.into_iter().enumerate() { lines.insert(start + j, l); source.insert(start + j, None); }
        }
        if m.numbered { lines = super::number_lines(lines); }
        if let Some(max) = self.caps.max_line_len {
            if let Some((n, l)) = lines.iter().enumerate().find(|(_, l)| l.len() > max) {
                return Err(PostError { file: self.file.clone(), line: None, message: format!("line {} of the program has {} characters, more than the controller reads (caps.max_line_len = {max}): {l}", n + 1, l.len()) });
            }
        }
        Ok(Rendered { lines, source, extension: self.extension.clone(), line_ending: self.line_ending, notes, troch_arcs, arcs })
    }

    /// The property values: the machine's, or the defaults.
    fn prop_values(&self, ctx: &PostCtx) -> Result<Vec<PropValue>, PostError> {
        self.props.iter().map(|p| {
            let Some(v) = ctx.props.get(&p.key) else { return Ok(p.default.clone()) };
            let ok = match (&p.kind, v) {
                (PropKind::Bool, PropValue::Bool(_)) | (PropKind::Int, PropValue::Int(_)) | (PropKind::Number, PropValue::Number(_)) | (PropKind::Text, PropValue::Text(_)) => true,
                (PropKind::Enum(o), PropValue::Text(t)) => o.contains(t),
                _ => false,
            };
            if ok { Ok(v.clone()) } else { Err(PostError { file: self.file.clone(), line: None, message: format!("property `{}`: the machine's value {v} does not fit its type", p.key) }) }
        }).collect()
    }
}

impl<'a> Renderer<'a> {
    fn err(&self, message: String) -> PostError { PostError { file: self.pkg.file.clone(), line: None, message } }

    /// Which runs become arcs (step 1).
    fn plan_arcs(&mut self, notes: &mut Vec<String>) -> Result<(Vec<Fit>, usize, usize), PostError> {
        let m = &self.prog.meta;
        if !m.arcs && m.troch.is_empty() { return Ok((vec![], 0, 0)); }
        if self.pkg.caps.arcs == ArcSupport::None {
            notes.push(tr("This post writes no arcs (G2/G3): circles and trochoidal loops stay as short straight moves.").to_string());
            return Ok((vec![], 0, 0));
        }
        let n = self.prog.events.len();
        let ranges: Vec<(usize, usize)> = if m.arcs { vec![(0, n)] } else { m.troch.clone() };
        let (xf, yf) = (self.pkg.words[Word::X.idx()].format, self.pkg.words[Word::Y.idx()].format);
        let dec = self.pkg.words[Word::I.idx()].format.decimals(self.inch).unwrap_or(12) as usize;
        let sweep = arcs::max_sweep(self.pkg.caps.max_arc_sweep);
        let (inch, k) = (self.inch, self.k);
        let rd = |f: &crate::post::text::Format, v: Option<f64>| v.map(|v| f.value(v, inch, k));
        let mut fits = Vec::new();
        let mut count = 0;
        let mut items: Vec<Item> = Vec::new();
        let mut from: Vec<usize> = Vec::new();
        for (a, b) in ranges {
            items.clear();
            from.clear();
            for i in a..b.min(n) {
                match &self.prog.events[i] {
                    Event::Feed { to, write_feed, .. } if to.z().is_none() && (to.x().is_some() || to.y().is_some()) => {
                        items.push(Item::Cut { x: rd(&xf, to.x()), y: rd(&yf, to.y()), f: *write_feed });
                        from.push(i);
                    }
                    Event::Feed { to, .. } | Event::Rapid { to } => { items.push(Item::Other { x: rd(&xf, to.x()), y: rd(&yf, to.y()) }); from.push(i); }
                    Event::Arc(c) => { items.push(Item::Other { x: rd(&xf, c.to.x()), y: rd(&yf, c.to.y()) }); from.push(i); }
                    // any other event: the lines it writes, read the way the text fitter read them (a comment gives nothing, other lines
                    // the X and Y words in them); an event that writes no line is not there for the fitter
                    _ => {
                        let mut s = Renderer { pkg: self.pkg, prog: self.prog, props: self.props, inch, k, st: State::default(), lines: vec![], source: vec![],
                                               comments: Some(vec![]), tools: self.tools, tool_k: self.tools.partition_point(|&t| t < i), scratch: String::new() };
                        s.event(i)?;
                        for (l, c) in s.lines.iter().zip(s.comments.unwrap_or_default()) {
                            let (x, y) = if c { (None, None) } else { arcs::xy(l) };
                            items.push(Item::Other { x, y });
                            from.push(i);
                        }
                    }
                }
            }
            for f in arcs::fit_items(&items, dec, k, sweep) {
                fits.push(Fit { first: from[f.first], last: from[f.last], end: f.end, ij: f.ij, cw: f.cw });
                count += 1;
            }
        }
        fits.sort_by_key(|f| f.first);
        Ok(if m.arcs { (fits, 0, count) } else { (fits, count, 0) })
    }

    /// A fitted arc through `on_circular`, from the event of its first cut.
    fn arc(&mut self, f: &Fit) -> Result<(), PostError> {
        let Event::Feed { feed, write_feed, .. } = &self.prog.events[f.first] else { return Err(self.err("an arc must start at a cut".into())) };
        let ev = Ev { target: [None, None, self.st.pos[2]], given: 0b011, end_units: Some(f.end), ij: Some((f.ij, true)), motion: if f.cw { 2 } else { 3 },
                      feed: Some((*feed, *write_feed)), ..Ev::default() };
        self.handler(Handler::Circular, &ev, f.first)
    }

    /// Event `i` through its handler (step 2).
    fn event(&mut self, i: usize) -> Result<(), PostError> {
        let (prog, pkg) = (self.prog, self.pkg);
        let caps = &pkg.caps;
        let target = |st: &State, to: &Xyz| { let mut t = st.pos; for (a, v) in t.iter_mut().enumerate() { if let Some(x) = to.get(a) { *v = Some(x); } } t };
        let given = |to: &Xyz| (0..3).fold(0u8, |m, a| if to.get(a).is_some() { m | 1 << a } else { m });
        match &prog.events[i] {
            Event::Comment { text, kind } => {
                let f = match kind { CommentKind::Info => Flag::Info, CommentKind::Operation => Flag::Operation, CommentKind::Pass => Flag::Pass, CommentKind::Warning => Flag::Warning };
                self.handler(Handler::Comment, &Ev { flags: f.bit(), text: Some(text), ..Ev::default() }, i)
            }
            Event::Setup => self.handler(Handler::Open, &Ev::default(), i),
            Event::OperationStart { index, name } => {
                self.st.forget();
                self.st.section = (*index, name.clone());
                self.handler(Handler::Section, &Ev { section: Some((*index, name)), ..Ev::default() }, i)
            }
            Event::OperationEnd { index } => {
                let name = if self.st.section.0 == *index { std::mem::take(&mut self.st.section.1) } else { String::new() };
                self.handler(Handler::SectionEnd, &Ev { section: Some((*index, &name)), ..Ev::default() }, i)
            }
            Event::ToolChange { tool } => {
                self.st.forget();
                let k = self.tool_k;
                self.tool_k += 1;
                let next = self.tools.get(k + 1).and_then(|&j| match &prog.events[j] { Event::ToolChange { tool } => Some(&**tool), _ => None });
                // the speed this tool starts at: the spindle start that follows, before the next move
                let rpm = prog.events[i + 1..].iter().take_while(|e| !matches!(e, Event::Rapid { .. } | Event::Feed { .. } | Event::Arc(_) | Event::ToolChange { .. } | Event::ProgramEnd { .. }))
                    .find_map(|e| match e { Event::SpindleOn { rpm } => Some(*rpm), _ => None });
                let flags = if k == 0 { Flag::First.bit() } else { 0 } | if next.is_some() { Flag::HasNext.bit() } else { 0 };
                let ev = Ev { flags, tool: Some(tool), next_tool: next, rpm, ..Ev::default() };
                // with a manual change the stop before it tells the operator; a tool changer must be told
                if caps.tool_change_halts() { self.must_write(Handler::SectionTool, &ev, i, "changes the tool", "on_section_tool") } else { self.handler(Handler::SectionTool, &ev, i) }
            }
            Event::SpindleOn { rpm } => {
                if caps.spindle == SpindleCtl::Manual { return Err(self.err("the program starts the spindle, but this post's spindle is run by hand (caps.spindle = \"manual\"): the generator must ask the operator with a stop".into())); }
                self.must_write(Handler::Command, &Ev { flags: Flag::SpindleOn.bit(), rpm: Some(*rpm), ..Ev::default() }, i, "starts the spindle", "on_command ?spindle_on")
            }
            Event::SpindleOff => {
                if caps.spindle == SpindleCtl::Manual { return Err(self.err("the program stops the spindle, but this post's spindle is run by hand (caps.spindle = \"manual\")".into())); }
                self.must_write(Handler::Command, &Ev { flags: Flag::SpindleOff.bit(), ..Ev::default() }, i, "stops the spindle", "on_command ?spindle_off")
            }
            Event::Dwell { seconds, why } => {
                let dwell = match caps.dwell { DwellUnit::Seconds => Some(*seconds), DwellUnit::Milliseconds => Some(seconds * 1000.0), DwellUnit::None => None };
                let flags = match why { DwellWhy::SpinUp => Flag::SpinUp, DwellWhy::Requested => Flag::Requested }.bit();
                self.handler(Handler::Dwell, &Ev { flags, dwell, ..Ev::default() }, i)
            }
            Event::Rapid { to } => {
                let ev = Ev { target: target(&self.st, to), given: given(to), motion: 0, ..Ev::default() };
                self.handler(Handler::Rapid, &ev, i)?;
                self.st.moved(to);
                Ok(())
            }
            Event::Feed { to, feed, write_feed } => {
                let ev = Ev { target: target(&self.st, to), given: given(to), motion: 1, feed: Some((*feed, *write_feed)), ..Ev::default() };
                self.handler(Handler::Linear, &ev, i)?;
                self.st.moved(to);
                Ok(())
            }
            Event::Arc(a) => {
                let (Some(x0), Some(y0)) = (self.st.pos[0], self.st.pos[1]) else { return Err(self.err("an arc before the program has given X and Y: its start is unknown".into())) };
                let ev = Ev { target: target(&self.st, &a.to), given: given(&a.to), motion: if a.cw { 2 } else { 3 }, ij: Some(([a.center[0] - x0, a.center[1] - y0], false)),
                              feed: Some((a.feed, a.write_feed)), ..Ev::default() };
                self.handler(Handler::Circular, &ev, i)?;
                self.st.moved(&a.to);
                Ok(())
            }
            Event::Stop { message, optional } => {
                self.st.forget();
                let (f, key) = if *optional { (Flag::OptionalStop, "on_command ?optional_stop") } else { (Flag::Stop, "on_command ?stop") };
                self.must_write(Handler::Command, &Ev { flags: f.bit(), message: Some(message), ..Ev::default() }, i, "stops for the operator", key)
            }
            Event::ProgramEnd { park } => {
                self.handler(Handler::Close, &Ev { park: park.as_ref(), ..Ev::default() }, i)?;
                if let Some(p) = park { self.st.moved(p); }
                Ok(())
            }
        }
    }

    /// `handler` for an event the machine must not miss (a spindle start or stop, a stop for the operator, a tool change the controller
    /// makes): a template that writes nothing for it is an error, not a silent gap in the program.
    fn must_write(&mut self, h: Handler, ev: &Ev, src: usize, what: &str, key: &str) -> Result<(), PostError> {
        let n = self.lines.len();
        self.handler(h, ev, src)?;
        if self.lines.len() == n { return Err(self.err(format!("the program {what} (event {src}), but {key} writes nothing for it: add a line for it"))); }
        Ok(())
    }

    /// The handler's template for one event: its lines, each with source `src`.
    fn handler(&mut self, h: Handler, ev: &Ev, src: usize) -> Result<(), PostError> {
        let pkg = self.pkg;
        let style = pkg.caps.comments;
        for line in &pkg.templates[h.idx()].lines {
            if let Some((c, neg)) = line.cond {
                let on = match c { Cond::Prop(p) => matches!(self.props[p], PropValue::Bool(true)), Cond::Flag(f) => ev.flags & f.bit() != 0 };
                if on == neg { continue; }
            }
            if line.comment && style == CommentStyle::None { continue; }
            if let Some(l) = self.line(line, ev, style) {
                self.lines.push(l);
                self.source.push(Some(src));
                if let Some(c) = self.comments.as_mut() { c.push(line.comment); }
            }
        }
        Ok(())
    }

    /// One template line, or `None` when it is left out (its placeholders have no value).
    fn line(&mut self, line: &Line, ev: &Ev, style: CommentStyle) -> Option<String> {
        let (mut seen, mut any) = (0, false);
        let mut out = String::with_capacity(40);
        if line.comment {
            let mut inner = String::new();
            self.parts(&line.parts, ev, &mut inner, &mut seen, &mut any);
            if seen > 0 && !any { return None; }
            return text::comment_line(style, &inner);
        }
        self.parts(&line.parts, ev, &mut out, &mut seen, &mut any);
        if let Some((gap, inside)) = &line.inline {
            if style != CommentStyle::None {
                let mut inner = String::new();
                self.parts(inside, ev, &mut inner, &mut seen, &mut any);
                out.push_str(gap);
                match style { CommentStyle::Paren => { out.push('('); out.push_str(&inner); out.push(')'); } _ => { out.push_str("; "); out.push_str(inner.trim()); } }
            }
        }
        if seen > 0 && !any { return None; }
        Some(out)
    }

    fn parts(&mut self, parts: &[Part], ev: &Ev, out: &mut String, seen: &mut usize, any: &mut bool) {
        for p in parts {
            match p {
                Part::Lit(s) => out.push_str(s),
                Part::G { lead } => {
                    *seen += 1;
                    let c = ev.motion;
                    if self.pkg.always[c] || self.st.motion != Some(c) {
                        if !out.is_empty() { out.push_str(lead); }
                        out.push_str(MOTION[c]);
                        self.st.motion = Some(c);
                        *any = true;
                    }
                }
                Part::Word { w, park, lead } => { *seen += 1; if self.word(*w, *park, ev, lead, out) { *any = true; } }
                Part::Var { v, filter, lead } => {
                    *seen += 1;
                    if let Some(t) = self.var(*v, ev) {
                        if !out.is_empty() { out.push_str(lead); }
                        match filter { Filter::None => out.push_str(&t), Filter::Comment => out.push_str(&text::comment_text(&t)), Filter::Plain => out.push_str(&text::plain_text(&t)) }
                        *any = true;
                    }
                }
            }
        }
    }

    /// A word: written (with the whitespace before it) or not, by its mode.
    fn word(&mut self, w: Word, park: bool, ev: &Ev, lead: &str, out: &mut String) -> bool {
        let def = self.pkg.words[w.idx()];
        // the value, whether the event gives it, and whether it is already in the output units
        let (v, given, in_units) = match w {
            Word::X | Word::Y | Word::Z => {
                let a = w.idx();
                if park {
                    match ev.park.and_then(|p| p.get(a)) { Some(v) => (v, true, false), None => match self.st.pos[a] { Some(v) => (v, false, false), None => return false } }
                } else if let (Some(e), true) = (ev.end_units, a < 2) {
                    (e[a], true, true)
                } else {
                    match ev.target[a] { Some(v) => (v, ev.given & (1 << a) != 0, false), None => return false }
                }
            }
            Word::I | Word::J => match ev.ij { Some((ij, u)) => (ij[w.idx() - 3], true, u), None => return false },
            Word::F => match ev.feed { Some((f, g)) => (f, g, false), None => return false },
            Word::S => match ev.rpm { Some(r) => (r, true, false), None => return false },
            Word::P => match ev.dwell { Some(d) => (d, true, false), None => return false },
        };
        if def.mode == Mode::AsGiven && !given { return false; }
        let start = out.len();
        if !out.is_empty() { out.push_str(lead); }
        let word_at = out.len();
        out.push(w.letter());
        def.format.write(out, v, self.inch, self.k, in_units);
        if def.mode == Mode::Modal {
            let last = &mut self.st.last[w.idx()];
            if last.as_deref() == Some(&out[word_at..]) { out.truncate(start); return false; }
            match last { Some(s) => { s.clear(); s.push_str(&out[word_at..]); } None => *last = Some(out[word_at..].to_string()) }
        }
        true
    }

    fn var<'b>(&'b mut self, v: Var, ev: &'b Ev) -> Option<Cow<'b, str>> {
        let num = |n: usize| -> Cow<str> { Cow::Owned(n.to_string()) };
        Some(match v {
            Var::Units => Cow::Borrowed(if self.inch { "G20" } else { "G21" }),
            Var::Wcs => Cow::Borrowed(self.prog.meta.wcs),
            Var::Text => Cow::Borrowed(ev.text?),
            Var::Message => Cow::Borrowed(ev.message?),
            Var::SectionNumber => num(ev.section?.0 + 1),
            Var::SectionName => Cow::Borrowed(ev.section?.1),
            Var::ToolNumber => num(ev.tool?.number as usize),
            Var::ToolOffset => { let t = ev.tool?; num(t.length_offset.unwrap_or(t.number) as usize) }
            Var::ToolName => Cow::Borrowed(&ev.tool?.name),
            Var::NextToolNumber => num(ev.next_tool?.number as usize),
            Var::NextToolOffset => { let t = ev.next_tool?; num(t.length_offset.unwrap_or(t.number) as usize) }
            Var::NextToolName => Cow::Borrowed(&ev.next_tool?.name),
            Var::Prop(p) => {
                self.scratch.clear();
                use std::fmt::Write;
                let _ = write!(self.scratch, "{}", self.props[p]);
                Cow::Borrowed(&self.scratch)
            }
        })
    }
}
