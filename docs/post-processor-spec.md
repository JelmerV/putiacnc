# PutiaCNC Post-Processor Plugin System — Design Specification

Nov 27, 2026 · @Jelmer

## Summary

The fork splits PutiaCNC's G-code stage in two: the toolpath generator emits a neutral program (an intermediate representation, IR), and a swappable **post-processor** turns it into text for one controller. Posts follow the Autodesk (Fusion/Inventor) model: event handlers, format and modal objects, declared capabilities and user-editable properties. Simple posts need only templates; anything with logic uses Rhai handlers. Each post lives with its machine configuration.

The first target is the TX-35 automatic tool changer (ATC) machine, whose VCarve post needs three things PutiaCNC cannot do today:

- `M06 T#` (tool change) with the** rack slot** number, not the order of first use.
- `G43 H#` (length offset) after every tool change, so each tool cuts at the right depth.
- A fixed header (`G28` (machine home), `G53 G0 Z0` (Z fully up), `G43 H` (length offset)) and footer that the post defines, not the generator.

This is a fork with no upstream-compatibility constraint, so large refactors are in scope. Scope is the core crate (`src/`), the machine manager and export UI, and the `putia-gcode` CLI. User posts are mm only. Machine control, 4/5-axis and cutter compensation stay out.

## Background: G-code today (v0.7.6)

Toolpath generation and G-code text are one step: `toolpath::generate(&ProjectData, Units) -> Output` writes finished text lines as it walks the operations. Dialects are `if` branches inside that step.

| Where | What it does | Why it blocks a plugin system |
| --- | --- | --- |
| `struct Em` in `src/toolpath.rs` | The emitter: `go()` formats `G0` (rapid move) / `G1` (cutting move) words, feed, decimals and units straight into `Vec<String>` | Number formatting and modal rules are hard-coded |
| `mach` / `marlin` flags | Branches for header, tool change (`T# M6` (tool change) vs `M0` (program pause) with a message), spindle (`M3` (spindle on) vs operator prompt), end (`M30` / `M2` (program end) / none) | Every new controller means more branches in a 2,535-line file |
| `select_tool()` | Numbers tools 1, 2, 3… in order of first use | No rack slot, no `G43 H#` (length offset) |
| `spin_wait()` | Writes `G4 P<seconds>` (dwell pause) | Fanuc-style controllers read `P` as milliseconds |
| `arcs::fit_arcs()` | Rewrites finished text lines into `G2` / `G3` (arc moves) | Works on text, so it is tied to one text format |
| `Machine.start_code` / `end_code` | Free lines inserted after the setup lines and before program end | Cannot replace the header or tool-change block, cannot use variables |
| `replay`, `verify`, `summary`, `split` | Read the G-code text back for the 3D preview, safety checks, summary and per-tool files | Their parsers expect today's dialects |

The text readers are deliberate: `ARCHITECTURE.md` says `replay` and `verify` share no code with `toolpath` "so they catch its mistakes". The design keeps that independence.

The dialect setting also lives in two places (`Machine.post` and the project's `job.post`), with values `""` (GRBL), `grblhal`, `fluidnc`, `mach`, `marlin`.

## Goals and non-goals

**Goals**

1. The TX-35 ATC machine runs a multi-tool job from PutiaCNC with the right rack slot and `G43 H#` (length offset) on every tool change. Output matches its VCarve post except one deliberate change: `M5` (spindle stop) before every `M06` (tool change), so safety does not rest on the controller's tool-change macro.
2. Built-in dialects (GRBL, grblHAL, FluidNC, Mach3/LinuxCNC, Marlin) keep their output unless a change is intended and reviewed through the golden files.
3. A user adds or adapts a controller by editing the post in that machine's configuration: no recompiling, no Rust.
4. Posts cannot touch files, the network or the project: a post only turns a program into text.
5. The toolpath generator holds no dialect branches.
6. `verify` still checks the final text on its own, so a bad post cannot slip a wrong move past the safety checks.

**Non-goals**

- Staying mergeable with upstream PutiaCNC. Upstream fixes are cherry-picked when useful.
- Inch output in user posts (built-in posts keep the inch support they have).
- Sending G-code to the machine (streaming, jogging, probing).
- 4th/5th axis, cutter compensation (`G41`/`G42`), canned drill cycles. The IR leaves room for them; no post uses them in this plan.
- Running Vectric `.pp` or Fusion `.cps` files directly. A `.pp` converter is a stretch item.

## Architecture

Generation becomes two stages joined by the IR. The generator asks the machine's post what the controller supports, emits events, and the post renders them; nothing above the post knows a G-code letter.

&#91;embedded content: G-code pipeline after the split · generator, IR, post, read-back\]

The safety check reads the final text, never the IR, so it stays independent of both the generator and the post. The move list, time estimate and 3D preview come from the IR; the source map ties each text line back to its event for the G-code panel.

## Intermediate representation

The IR is a flat list of events in a new module `src/program.rs`. All values are in mm, mm/min and seconds, absolute work coordinates; posts convert and format.

```rust
pub struct Program {
    pub meta: Meta,            // job name, stock, units wanted, wcs, machine id, used tools
    pub events: Vec<Event>,
}

pub enum Event {
    Comment { text: String, kind: CommentKind },       // Info, Operation, Pass, Warning
    OperationStart { index: usize, name: String, kind: OpKind },
    OperationEnd { index: usize },
    ToolChange { tool: ToolRef, next: Option<ToolRef> }, // next = for ATC pre-select
    SpindleOn { rpm: f64, dir: Dir },
    SpindleOff,
    Dwell { seconds: f64, why: DwellWhy },             // SpinUp, Requested
    Rapid { to: Xyz },
    Feed { to: Xyz, feed: f64, kind: FeedKind },        // Cut, Plunge, Ramp, Lead, Retract
    Arc { to: Xyz, center: [f64; 2], cw: bool, feed: f64 }, // helical when to.z changes
    Stop { message: String, optional: bool },
    ProgramEnd { park: Option<Xyz> },
}

pub struct ToolRef {
    pub id: String, pub name: String, pub number: u16, pub length_offset: Option<u16>,
    pub dia: f64, pub shape: String,
}

pub struct Xyz { pub x: Option<f64>, pub y: Option<f64>, pub z: Option<f64> } // None = unchanged
```

Design rules:

- **Intent, not syntax.** The generator says `ToolChange` and `SpindleOn`; whether that becomes `T5 M6` (tool change), `M0 (Change tool…)` (program pause) or an operator prompt on Marlin is the post's choice. Today's `select_tool()` logic for each dialect moves into the posts.
- **Every event keeps its index.** Posts return which event each output line came from (a source map). The 3D preview, the G-code panel and the time breakdown use that map instead of parsing the user's dialect.
- **Arcs are fitted on the IR**, not on text. The post engine fits runs of `Feed` events (`arcs::fit_items`) on the numbers the post writes (output units, rounded by its X/Y format), before it renders, and writes each arc through `on_circular`; `caps.arcs = none` fits nothing. Since phase 2 this is the only fitter in use; `arcs::fit_arcs` is a thin wrapper that reads text into the same items, and a test compares the two on every golden program.
- **Times stay with the generator.** The `Move` list and time estimate are built from the IR, so they do not depend on the post.

**As built (phases 1 and 2).** `src/program.rs` differs from the sketch above where today's output needed it (details in `docs/implementation-log.md`): an `Event::Setup` for the setup block (`on_open`); `OperationStart { index, name }` (name = the operation's kind) and `OperationEnd { index }`, while the operation's description stays `Comment` events; `ToolChange { tool }` without `next` (the engine finds the next tool); `SpindleOn { rpm }` without a direction; `Feed { to, feed, write_feed }` without `kind`, where `write_feed` gives the feed the way `to` gives axes (the generator's modal rule); `Arc(Box<ArcTo>)` (boxed, so an event stays 48 bytes; the generator writes none yet); `ToolRef.number` is a `u32`, the order of first use until phase 3; `Xyz` is three numbers and a mask. `Meta` holds the units, the resolved work offset, and, until phase 4a makes them properties, `numbered`, `arcs`, `start_code`, `end_code`, `start_at` and the trochoidal ranges. A requested dwell is rounded to 0.01 s by the generator (a value, not a format).

## Post-processor interface

Every post, built-in or user-made, implements one trait in `src/post/mod.rs` and declares what the controller can do. The generator reads the capabilities before it generates; the post renders after.

```rust
pub trait Post: Send + Sync {
    fn name(&self) -> &str;                 // shown in the machine manager
    fn caps(&self) -> &Caps;
    fn properties(&self) -> &[PropDef];     // user options, Fusion-style
    fn render(&self, prog: &Program, ctx: &PostCtx) -> Result<Rendered, PostError>;
}

pub struct Caps {
    pub arcs: ArcSupport,          // None | Xy | Helical
    pub max_arc_sweep: f64,        // degrees; longer arcs are split (Fusion: maximumCircularSweep)
    pub tool_change: ToolChange,   // Manual (stop + message) | Atc
    pub length_offset: bool,       // writes G43 H# itself
    pub spindle: SpindleCtl,       // Controlled | Manual (operator starts it)
    pub dwell: DwellUnit,          // Seconds | Milliseconds | None (controller waits)
    pub comments: CommentStyle,    // Paren | Semicolon | None
    pub wcs_count: u8,             // 6 = G54-G59, 9 adds G59.1-.3
    pub max_line_len: Option<usize>,
}

pub struct PropDef { pub key: String, pub title: String, pub kind: PropKind, pub default: PropValue }
// PropKind: Bool | Int | Number | Text | Enum(Vec<String>)

pub struct PostCtx<'a> {
    pub machine: &'a Machine,
    pub props: &'a BTreeMap<String, PropValue>,   // values saved with the machine
    pub safe_z: f64,
}

pub struct Rendered {
    pub lines: Vec<String>,
    pub source: Vec<Option<usize>>,   // event index per line
    pub extension: String,            // "nc", "gcode", "tap"
    pub line_ending: LineEnding,      // Lf | Crlf
    pub notes: Vec<String>,           // warnings raised by the post
    pub troch_arcs: usize,            // arcs fitted (for the generator's notes)
    pub arcs: usize,
}
```

One engine implements the trait: it loads a post package (manifest, templates, optional Rhai script), described in the next section. Units are always mm in user posts. (Phase 2: `ToolChange` here is the enum `ToolChangeMode` in the code, so that it does not clash with the event; `Package` is the engine's type; `post::test_program(&caps)` is the fixed test program.)

**How the generator uses capabilities**

| Capability | Generator behaviour |
| --- | --- |
| `arcs = None` | No arc fitting; trochoidal loops stay as short lines with a warning (the post engine fits, so the post writes the note) |
| `tool_change = Manual` | Emits a `Stop` with the "change tool, touch off Z0" message before `ToolChange` (today's GRBL behaviour) |
| `tool_change = Atc` | No stop, and the machine stands still at the `ToolChange` (`Move.stop`; Mach3 / LinuxCNC today). Phase 3: requires every used tool to have a number, else an export error |
| `length_offset = false` with `Atc` | Phase 3: warning "every tool after the first relies on the controller's own length handling" (not for the built-in Mach3 / LinuxCNC, whose notes must not change) |
| `spindle = Manual` | `SpindleOn` becomes a `Stop` asking the operator, no `SpindleOff` and no spin-up `Dwell`; the tool-change and pause messages start "Stop the spindle", the end is a stop "Stop the spindle, then resume" (today's Marlin behaviour). A requested `Dwell` (a pen's) stays |
| `dwell = None` | No spin-up `Dwell` events |
| `wcs_count` | The work offset: the job's choice when the controller has it, else G54 |
| the post's name | The first comment: `(PutiaCNC - <name>)` |

**Built-in posts are post packages too.** GRBL, grblHAL, FluidNC, Mach3/LinuxCNC and Marlin ship as packages embedded in the binary (`include_str!`), run by the same engine as user posts. That dogfoods the format, removes the Rust dialect code, and lets any built-in be copied and customised.

**Posts belong to the machine.** A machine either names a built-in (`"post": "builtin:grbl"`) or carries its own package in its folder (`"post": "local"`). There is no global post registry; sharing a post means sharing the machine folder or copying its post files into another machine. A machine whose post fails to load cannot export, and says why.

## Post packages: Autodesk model, template shorthand

A post is a small package: a TOML manifest, text templates for simple handlers, and an optional Rhai script for handlers that need logic. The structure follows the Autodesk post engine used by Fusion and Inventor CAM, because it is the most widely known model and it handles modal codes and user options properly. The Vectric idea that survives is the no-code template for simple machines.

| Feature | Vectric `.pp` | Autodesk `.cps` (Fusion, Inventor) | PutiaCNC |
| --- | --- | --- | --- |
| Structure | Named text blocks | JavaScript entry functions: `onOpen`, `onSection`, `onLinear`, `onCircular`, `onClose`… | Same events as Rhai handlers; a template per handler as shorthand |
| Modal G codes | Separate `FIRST_FEED_MOVE` / `FEED_MOVE` blocks | `createModal` drops repeated codes | Modal groups in the manifest; no "first" blocks |
| Number formats | `VAR` lines: letter, modal flag, decimals | `createFormat` / `createVariable` objects | `[format]` and `[words]` tables |
| User options | Edit the file | `properties`, shown in the post dialog | `[properties]`, edited in the machine manager, saved with the machine |
| Capabilities | Implied by the blocks present | `allowedCircularPlanes`, `allowHelicalMoves`, `maximumCircularSweep` | `[caps]`, read by the generator before it runs |
| Tool context | `[T]` | `tool.number`, `tool.lengthOffset`, `getNextSection()` | `tool`, `next_tool`, `is_first_section()`, `has_next_section()` |
| Machine commands | One block each | `onCommand(COMMAND_STOP_SPINDLE)` etc. | `on_command` with named commands |
| Errors | None | `error()`, `warning()` | `error()` stops export; `warning()` adds an export note |

**Package layout** (in the machine's folder, see Machine profile):

```
machines/tx35/
  machine.json     # work area, rapid rates, tool rack, property values
  post.toml        # manifest: caps, properties, formats, modal groups, templates
  post.rhai        # optional: handlers that override templates
```

**The TX-35 manifest as an example**:

```
[post]
format_version = 1
name = "TX-35 ATC"
extension = "nc"
line_ending = "crlf"

[caps]
arcs = "xy"               # no Z on arc lines: helical ramps become lines
max_arc_sweep = 180
tool_change = "atc"
length_offset = true
spindle = "controlled"
dwell = "none"            # the controller waits for spindle speed
comments = "none"

[properties]              # shown in the machine manager, values saved in machine.json
stop_spindle_before_tool_change = { type = "bool", default = true, title = "Stop spindle (M5) before M06" }
preselect_next_tool = { type = "bool", default = false, title = "Pre-select next tool after M06" }

[format]                  # trim = false keeps the zeros VCarve writes: X74.232, Z14.280, F2500.0
xyz  = { decimals = 3, trim = false }
feed = { decimals = 1, trim = false }
rpm  = { decimals = 0 }

[words]                   # modal = written only when the value changes
X = { format = "xyz", modal = true }
Y = { format = "xyz", modal = true }
Z = { format = "xyz", modal = true }
I = { format = "xyz", incremental = true }
J = { format = "xyz", incremental = true }
F = { format = "feed", modal = true }
S = { format = "rpm" }

[modal]                   # a code is written only when its group changes
motion = { codes = ["G0", "G1", "G2", "G3"], always = ["G0", "G2", "G3"] }

[templates]
on_open = """
G0 G28 G91
G54
G17 G80 G49 G0 G40 G90
G53 G0 Z0
G21
"""
on_section_tool = """
?stop_spindle_before_tool_change: M5
M06T{tool.number}
?preselect_next_tool: T{next_tool.number}
G43H{tool.offset}
{S}M03
"""
on_rapid    = "{G}{X}{Y}{Z}"
on_linear   = "{G}{X}{Y}{Z}{F}"
on_circular = "{G}{X}{Y}{I}{J}{F}"
on_close = """
G0{Z:park}
G0{X:park}{Y:park}
M30
"""
```

One ordering difference from VCarve: the first tool is loaded by `on_section_tool`, so the `G54 X0 Y0` (move to job zero) comes after `M06` (tool change), `G43` (length offset) and `M03` (spindle on), instead of between them. Both are safe; Z stays fully up throughout.

**Template rules.** (Reference with examples: `docs/posts/authoring.md`.)

- `{X}`-style words come from `[words]`: X Y Z (target; `{X:park}` the park position), I J (arc centre from its start), F, S, P (dwell in the `caps.dwell` unit). Each has a format from `[format]` (`decimals`, `inch_decimals`, `trim`, `length`; rounding as `toolpath::to_fixed`, never `-0`) and a mode: `as_given` (written when the event gives it: the axes the move changes, the feed when `Feed.write_feed`; the built-ins' rule), `modal` (`modal = true`: written when its text changes, again after a section start, a tool change and a stop) or `always` (the default). `S{S}` is an error: the word writes its letter.
- `{G}` comes from the modal group `motion` (G0 G1 G2 G3; codes in `always` are written on every move).
- `{name}` is a variable (`units`, `wcs`, `text`, `message`, `section.number`, `section.name`, `tool.number`, `tool.offset`, `tool.name`, `next_tool.*`, `prop.<key>`); `{name|comment}` and `{name|plain}` filter it (safe in a comment; a plain message for controllers that show everything after `M0`).
- A placeholder with no value takes the whitespace before it; a line whose placeholders all have no value is left out.
- A line starting `?name:` is written only when that property or flag is true, `?!name:` only when it is false.
- A whole-line `( … )` is a comment and an ending `( … )` an inline comment, both written in `caps.comments` style (`(…)`, `; …`, nothing), also for whole-line comments in the machine's start and end code. `;` is plain text in a template (GRBL's `M0 ; message`).
- Nothing else: no loops, no expressions. Unknown keys, words, formats, properties, variables, filters, flags or handlers, and a word or variable used where it has no value, are load errors with the file, the line and the fix. A spindle start or stop, an operator stop, or (with `atc`) a tool change that a template renders as nothing is a render error.
- Line numbers (`N10`, step 10) and the machine's start code (before the first lift) and end code (before the last line) are added by the engine after everything else, for every package.

**Handlers**

| Handler | Called | Default when not defined |
| --- | --- | --- |
| `on_open` | Once, at the setup (after the two header comments) | `{units} G90 G17 {wcs}`: units, `G90` (absolute coordinates), `G17` (XY plane), the work offset |
| `on_section(s)` | Each operation start | Nothing (the generator writes the operation's description as `Comment` events) |
| `on_section_tool(s)` | Each `ToolChange`: the first tool, and when the tool changes (flags `first`, `has_next`) | Per `caps`: manual `(Tool: name)` (the `Stop` before it asked the operator); atc `T# M6 (Tool #: name)`, plus `G43 H#` (length offset) with `length_offset`. `M5` and `M3 S` are the separate `SpindleOff` / `SpindleOn` events around it |
| `on_section_end(s)` | Each operation end, after its pause | Nothing |
| `on_rapid` / `on_linear` / `on_circular` | Each move (a fitted arc through `on_circular`) | `{G} {X} {Y} {Z}`, `{G} {X} {Y} {Z} {F}`, `{G} {X} {Y} {Z} {I} {J} {F}` |
| `on_dwell(d)` | Spin-up or requested pause (flags `spin_up`, `requested`) | `G4 {P}` (dwell pause) in the `caps.dwell` unit |
| `on_command(c)` | `spindle_on`, `spindle_off`, `stop`, `optional_stop`, `coolant_on`, `coolant_off`, as `?flag:` lines of one template | `M3 {S}` (spindle on), `M5` (spindle stop), `M0 ({message})` (program pause), `M1` (optional pause), `M8` (coolant on), `M9` (coolant off) |
| `on_comment(t)` | Comments (flags `info`, `operation`, `pass`, `warning`) | `({text})`, written per `caps.comments` |
| `on_close` | The `ProgramEnd` | `G0 {X:park} {Y:park}`, `M30` (program end); `M5` is the `SpindleOff` before it |

**Rhai handlers** override any template. [Rhai](https://rhai.rs) fits the Rust codebase: pure Rust, sandboxed by default, with operation and memory limits. `block()` works like Fusion's `writeBlock`: it joins words, drops empty modal words and adds the line number.

```rust
// post.rhai: a stricter tool change; the other handlers use templates
fn on_section_tool(s, ctx) {
    if prop("stop_spindle_before_tool_change") && ctx.spindle_running { block("M5"); }
    block("G53 G0 Z0");
    block(`M06T${s.tool.number}`);
    if prop("preselect_next_tool") && has_next_section() { block(`T${next_tool().number}`); }
    if s.tool.number > 99 { error("TX-35 rack has no slot above 99"); }
    block(`G43H${s.tool.offset}`);
    block(word("S", s.rpm), "M03");
}
```

Sandbox rules: a handler sees one event and a read-only context, writes lines only through `block()` and `comment()`, and gets a fixed budget (for example 100,000 operations per event). Errors stop the export with the file and line.

## Tool numbers and length offsets

The same cutter can sit in slot 3 on one machine and slot 7 on another, so the slot belongs to the machine, not the tool. The machine gets a tool table; the tool keeps an optional default.

```rust
// model.rs — Tool, new optional fields (serde default = old files still load)
#[serde(default, skip_serializing_if = "Option::is_none")] pub number: Option<u16>,
#[serde(default, skip_serializing_if = "Option::is_none")] pub length_offset: Option<u16>,

// model.rs — Machine, new field
#[serde(default, rename = "toolTable", skip_serializing_if = "Vec::is_empty")]
pub tool_table: Vec<ToolSlot>,

pub struct ToolSlot { pub tool_id: String, pub number: u16, pub length_offset: Option<u16> }
```

**Resolving a tool's number** at generate time, first match wins:

1. The machine's `tool_table` entry for that tool id.
2. The tool's own `number`.
3. Order of first use (today's behaviour), only for posts with `tool_change = Manual`.

With an ATC post, a used tool that resolves to step 3 is an export error naming the tool, and two tools resolving to the same number is an error too. Loading the wrong cutter is worse than not exporting.

`length_offset` defaults to the resolved number (`H5` (offset register) for `T5` (tool number)), which is the usual Fanuc/Mach convention and what the TX-35 post expects.

**UI:** the machine manager gets a "Tool rack" tab listing slots with a tool picker per slot; the tool manager shows each tool's slot on the selected machine. The setup sheet lists slot and H number per tool, so the operator can check the rack before pressing start.

## Machine profile, UI and CLI

**Machine profile.** Each machine becomes a folder under `<config dir>/putiacnc/machines/<id>/` holding `machine.json` and, for a custom post, `post.toml` and `post.rhai`. `machine.json` gains `post` (`"builtin:<name>"` or `"local"`), `post_props` (the property values) and the tool rack. The project's `job.post` is dropped: the project points at a machine, and the machine decides. The old `numbered`, `arcs`, `start_code` and `end_code` settings migrate into properties of the built-in posts on first load.

**Portability.** Exporting a machine writes its whole folder as one `.putiamachine` zip, post included, so importing it on another computer gives the same G-code. A project opened without its machine asks to import the machine or pick another.

**Machine manager**

- **Post tab** per machine: pick a built-in, or "Customise" to copy the built-in into the machine folder as an editable package.
- **Properties form** generated from `[properties]`: checkboxes, numbers, dropdowns, with each property's title.
- **Capability summary** in one line ("ATC, G43 H (length offset), XY arcs, mm").
- **Test post** renders a fixed test program (every event type, two tool changes, an arc, a stop) beside the post source, with "Open folder" and "Reload".
- Load errors listed with file, line and the fix.

**G-code panel and 3D preview** follow the source map: clicking a line selects its step, whatever the dialect.

**Export window** uses the post's extension and line ending, and shows capability warnings (for example "this post has no arcs: circles are written as short lines") before saving.

**CLI**

```bash
putia-gcode project.json --machine ~/.config/putiacnc/machines/tx35 -o job.nc
```

```bash
putia-gcode --check-post ~/.config/putiacnc/machines/tx35
```

The second command lints a post and prints the rendered test program, for use in a post library's CI.

## Verification

The refactor touches the code that decides where a spinning cutter goes, so every phase has a test gate.

1. **Golden files (phase 0, before any refactor).** Generate G-code for every project in `examples/` and the built-in samples, in every built-in dialect, and commit the output under `tests/golden/`. A test regenerates and compares byte for byte. Refactors are free, but any golden change needs a reviewed reason in the commit.
2. **Independent read-back.** `verify` keeps parsing the *final* text, not the IR. It becomes dialect-aware by reading the post's `[words]` and `[modal]` tables, and gains a cross-check: the parsed motion must match the IR's motion within 0.001 mm. A post that drops, reorders or mis-scales a move fails export.
3. **ATC safety checks** on the final text when `tool_change = atc`:
   - the spindle is stopped (`M5`, spindle stop) before every `M6` (tool change) when the stop property is on;
   - every `M6` (tool change) is followed by a length offset before the first Z move, when `length_offset = true`;
   - the spindle is running before every cutting move;
   - every T number matches the resolved rack slot of the tool the IR says is cutting.
4. **Post engine tests.** Each handler, modal group, word format, `?property:` line and Rhai helper has a unit test. The TX-35 package rendered for a fixed two-tool project is compared with a VCarve file for the same job; the only allowed differences are `M5` (spindle stop) before `M06` (tool change) and the first-tool ordering. The original VCarve post is committed as a fixture.
5. **Sandbox tests.** A Rhai handler cannot open files, reach the network or loop forever; each case has a test.
6. **On the machine.** Before the TX-35 post is called done: an air cut 20 mm above the stock with offsets active, then a two-tool job on scrap, checking depth after the tool change.

## Roadmap

Phase 4 is the milestone that matters: the TX-35 runs a multi-tool job. Phases 0–2 are refactors that change no output, so the golden files prove them safe before any new behaviour lands.

&#91;embedded content: Roadmap · 7 phases, each with its gate\]

Each phase merges only when its gate passes; dates are left open until phase 0 shows how many example projects and dialect combinations the goldens cover.

## Risks and decisions

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Refactor changes output by accident | Wrong cuts on users' machines | Golden files before any change; every golden diff needs a reviewed reason |
| Arc fitting on IR differs from text fitting | Subtle arc changes | Keep the text fitter until the IR fitter matches all goldens |
| A machine's post is wrong (bad T tool number, missing G43 length offset) | Crash into stock or wrong tool | ATC checks in `verify`; motion cross-check; air-cut advice in the export notes |
| Fork drifts from upstream | Upstream bug fixes and features missed | Watch upstream releases; cherry-pick fixes in geometry and CAM by hand |
| Rhai handlers become the default for everything | Posts get hard to read and review | Built-in posts use templates wherever they can; scripts only where logic is needed |
| Post format frozen too early | Posts break on upgrade | `format_version` in every manifest; loader rejects unknown versions with a clear message |

**Decisions (27 Nov 2026)**

| Question | Decision | Effect on the plan |
| --- | --- | --- |
| Fork or upstream contribution? | Fork, with no compatibility constraint | Large refactors allowed; built-ins become post packages; `job.post` dropped |
| Where do posts live? | With the machine configuration | Machine folders with `post.toml`; no global post registry |
| Script language? | Rhai | Rhai handlers are part of the post model, not an afterthought |
| `M5` (spindle stop) before `M06` (tool change) on the TX-35? | Yes: do not rely on the tool-change macro | Property `stop_spindle_before_tool_change`, default on; checked by `verify` |
| Inch support in user posts? | No, mm only | No unit handling in templates or handlers |
| Post model to follow? | Autodesk (Fusion/Inventor), with Vectric-style templates as shorthand | Handlers, modal groups, properties, capabilities |

**Licence.** The fork stays GPL-3.0-or-later, as PutiaCNC is. The OpenCNC TX-35 VCarve post is not confidential, so it is committed as a test fixture and the TX-35 package can ship with the fork.

## Sources

- [tonyrh/putiacnc](https://github.com/tonyrh/putiacnc): source read at v0.7.6 (`ARCHITECTURE.md`, `src/toolpath.rs`, `src/model.rs`)
- [PutiaCNC releases](https://github.com/tonyrh/putiacnc/releases)
- [Rhai scripting language](https://rhai.rs)
- `TX-35_G-Code_ATC_Arcs_mm_.tap.pp`: the machine's VCarve post-processor
- [Autodesk CAM post processor reference: entry functions](https://cam.autodesk.com/posts/reference/entry_functions.html)
