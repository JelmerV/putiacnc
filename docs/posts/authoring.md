# Writing a post package

A *post* turns the program PutiaCNC generates (a list of events: moves, tool changes, spindle, stops, comments) into G-code
for one controller. Every post is a **package**: a TOML manifest with the controller's capabilities, number formats, words,
the motion group and one small text template per event. The five built-in dialects are packages too, embedded in the
binary: [`src/post/builtin/`](../../src/post/builtin) (`grbl`, `grblhal`, `fluidnc`, `mach`, `marlin`). The design is in
[`docs/post-processor-spec.md`](../post-processor-spec.md); this page is the reference for writing one.

Phase 2 status: the engine and the built-ins are in place and a job still picks its built-in with its `post` setting.
Machine folders with their own `post.toml`, property values saved with the machine, Rhai handlers and
`putia-gcode --check-post` come in later phases. Until then, check a package with

```bash
cargo run --example check_post -- path/to/post.toml          # or builtin:grbl, builtin:mach, ...
cargo run --example check_post -- path/to/post.toml --imperial
```

It prints a load error (file, line, what to do) and exits with 1, or the capabilities and the fixed test program
(`post::test_program`: two tools, every kind of event, an arc and a stop) as your package writes it.

## The worked example: GRBL

[`src/post/builtin/grbl/post.toml`](../../src/post/builtin/grbl/post.toml), section by section.

```toml
[post]
format_version = 1
name = "GRBL"            # written in the first comment: (PutiaCNC - GRBL)
extension = "nc"
line_ending = "lf"
```

`format_version` must be `1`: a manifest for another version is refused with a message, before anything else is read.

```toml
[caps]
arcs = "xy"               # G2/G3 in the XY plane
max_arc_sweep = 180
tool_change = "manual"    # the program stops and asks the operator to change the tool
tool_numbers = "first_use" # tools without a rack slot or a number of their own: 1, 2, 3... by first use
length_offset = false
spindle = "controlled"    # M3 / M5
dwell = "seconds"         # G4 P<seconds>
comments = "paren"
wcs_count = 6             # G54 to G59
```

The generator reads the capabilities **before** it generates, so they decide which events exist:

| Key | Values (default first) | What it changes |
|---|---|---|
| `arcs` | `xy`, `helical`, `none` | `none`: no arc fitting at all, and a note ("This post writes no arcs …"). `helical` fits what `xy` fits (XY arcs) for now. |
| `max_arc_sweep` | `180` | Degrees of the longest fitted arc, more than 0 and at most 180. |
| `tool_change` | `manual`, `atc` | `manual`: a stop with "Change tool to …, then touch off Z0 and resume" before each change. `atc`: no stop; the machine stands still at the change (the next move starts from rest in the time estimate). |
| `tool_numbers` | `required` with `atc`, `first_use` with `manual` | How a tool gets the number `{tool.number}` writes when neither the machine's tool rack nor the tool itself gives one (see "Tool numbers" below). `required`: such a tool stops the export, naming it. `first_use`: 1, 2, 3… in order of first use, skipping the numbers numbered tools of the program have. The built-in Mach3 / LinuxCNC is an `atc` post with `first_use`. |
| `length_offset` | `false`, `true` | With `atc`, the default `on_section_tool` adds `G43 H{tool.offset}`, and the export checks that every `M6` is followed by a `G43` with that tool's `H` before the first Z move. With `atc`, `required` and `false`, the export notes that every tool after the first relies on the controller's own length handling. |
| `spindle` | `controlled`, `manual` | `manual`: no spindle commands and no spin-up wait; the program stops and asks the operator to start the spindle (and to stop it before a pause, a tool change and at the end). |
| `dwell` | `seconds`, `milliseconds`, `none` | The unit of `{P}`. `milliseconds`: seconds × 1000, for controllers whose `G4` reads `P` as milliseconds (Marlin); give `P` a format with `decimals` (the Marlin built-in has `decimals = 0`), as seconds × 1000 is not always a whole number in floating point. `none`: no spin-up wait, and `{P}` has no value. |
| `comments` | `paren`, `semicolon`, `none` | How comment lines are written (see Templates). |
| `wcs_count` | `6` | Work offsets the controller has: 1 to 9. The job's choice beyond that is written as G54. |
| `max_line_len` | none | A longer line stops the export. |

```toml
[format]
xyz = { decimals = 3, inch_decimals = 4, length = true }   # X12.7, X0
feed = { decimals = 1, inch_decimals = 2, length = true }
rpm = {}                  # as it is: S18000
seconds = {}              # as the program gives it: P2, P0.13
```

A format: `decimals` (0 to 9; left out: the number as it is, in its shortest form), `inch_decimals` (for inch output; the
built-ins write inches, user posts are mm only), `trim` (default `true`: `1.5`, `0`; `false` keeps the zeros: `1.500`,
`0.000`, and needs `decimals`), `length` (divided by 25.4 in inch output). Numbers are rounded the way PutiaCNC always
rounded them (JavaScript's `toFixed`: an exact tie goes up), and zero is never written `-0`. The four formats above are
the defaults; a package's `[format]` adds formats or replaces them by name.

```toml
[words]
X = { format = "xyz", mode = "as_given" }   # written when the move gives the axis
Y = { format = "xyz", mode = "as_given" }
Z = { format = "xyz", mode = "as_given" }
I = { format = "xyz", incremental = true }
J = { format = "xyz", incremental = true }
F = { format = "feed", mode = "as_given" }  # written when the feed changed, and after a tool change or a pause
S = { format = "rpm" }
P = { format = "seconds" }
```

The words the engine gives values: `X Y Z` (the target of a move; with `:park`, the park position), `I J` (the arc centre
from its start: `incremental = true` is the only kind), `F` (feed), `S` (spindle speed), `P` (dwell). Tool and offset
numbers are variables (`T{tool.number}`), not words. The **mode** decides when a word is written:

| Mode | Written when |
|---|---|
| `as_given` | the event gives it: an axis the move changes, the feed when the generator says it changed (or after a tool or pen selection or a pause). The built-ins use it, so their text is the generator's decision. |
| `modal` (or `modal = true`) | its text differs from what this word wrote last, and always after a section start, a tool change and a stop. |
| `always` (the default, or `modal = false`) | it has a value: an axis writes the full position on every move. |

Defaults when a word is left out: X, Y, Z and F `as_given`, the others `always`, in the formats above.

```toml
[modal]
motion = { codes = ["G0", "G1", "G2", "G3"], always = ["G0", "G1", "G2", "G3"] }
```

`{G}` is the motion code of a move (`G0` rapid, `G1` cut, `G2`/`G3` arc). A code in `always` is written on every move; the
others only when the motion changes (TX-35 style: `always = ["G0", "G2", "G3"]` leaves out repeated `G1`). `motion` is the
only group; left out, every code is written every time.

```toml
[templates]
on_open = "{units} G90 G17 G94 {wcs}"
on_section = ""
on_section_end = ""
on_section_tool = "(Tool: {tool.name|comment})"
on_rapid = "{G} {X} {Y} {Z}"
on_linear = "{G} {X} {Y} {Z} {F}"
on_circular = "{G} {X} {Y} {I} {J} {F}"
on_dwell = "G4 {P}"
on_command = """
?spindle_on: M3 {S}
?spindle_off: M5
?stop: M0 ; {message}
?optional_stop: M1 ; {message}
"""
on_comment = "({text})"
on_close = """
G0 {X:park} {Y:park}
M2
"""
```

What it writes for the test program (`cargo run --example check_post -- builtin:grbl`):

```
(PutiaCNC - test program)
(Stock 100 x 50 x 12 mm; XY0 = lower-left corner of stock, Z0 = top surface)
G21 G90 G17 G94 G54
G0 Z5
(Tool: 1/8in flat -end- mill)
M3 S18000
G4 P2
(Operation 1: outside, tool 3.175 mm)
G0 X10 Y-2.5
G0 Z0.5
G1 Z-1 F250
(Pass 1)
G1 X20 F800
G1 X30 Y10
G3 X20 Y20 I-10 J0
G0 Z5
M5
M0 ; Change tool to 6 mm, then touch off Z0 and resume
(Tool: 6 mm)
M3 S12000
G4 P2
(Operation 2: pocket, tool 6 mm)
G0 X40 Y20
G1 Z-0.5 F300
G4 P0.13
G1 X50 Y20 F1000
G0 Z5
M5
M0 ; Operation 2 done: resume when ready
M5
G0 X0 Y0
M2
```

grblHAL and FluidNC are this file with another `name`. Mach3 / LinuxCNC changes `tool_change` to `atc` (with
`tool_numbers = "first_use"`, so a tool without a rack slot keeps its order-of-first-use number), `wcs_count` to 9,
writes `G40 G49 G80` in `on_open`, `T{tool.number} M6 (Tool {tool.number}: {tool.name|comment})` and `M0 ({message})`.
Marlin sets `spindle = "manual"` and `comments = "semicolon"` and writes its stops `M0 {message|plain}`.

## Handlers

Each event goes through one handler. A handler left out of `[templates]` gets the default below; `""` writes nothing.

| Handler | Called for | Flags (`?name:`) | Variables and words | Default |
|---|---|---|---|---|
| `on_open` | the setup, after the two header comments | | | `{units} G90 G17 {wcs}` |
| `on_section` | an operation starts | | `section.number`, `section.name` | nothing (the generator writes the operation's description as comments) |
| `on_section_tool` | a tool is put in: the first, and each change | `first`, `has_next` | `tool.number`, `tool.offset`, `tool.name`, `next_tool.number`, `next_tool.offset`, `next_tool.name`; `{S}` the speed it starts at | `manual`: `(Tool: {tool.name\|comment})`; `atc`: `T{tool.number} M6 (Tool {tool.number}: {tool.name\|comment})`, and `G43 H{tool.offset}` with `length_offset` |
| `on_section_end` | an operation ends (after its pause) | | `section.number`, `section.name` | nothing |
| `on_rapid` | a rapid move | | `{G} {X} {Y} {Z}` | `{G} {X} {Y} {Z}` |
| `on_linear` | a cutting move | | `{G} {X} {Y} {Z} {F}` | `{G} {X} {Y} {Z} {F}` |
| `on_circular` | an arc (fitted, or an arc event) | | `{G} {X} {Y} {Z} {I} {J} {F}` | `{G} {X} {Y} {Z} {I} {J} {F}` |
| `on_dwell` | the spin-up wait, a requested dwell | `spin_up`, `requested` | `{P}` | `G4 {P}` |
| `on_command` | spindle start and stop, operator stops | `spindle_on`, `spindle_off`, `stop`, `optional_stop`, `coolant_on`, `coolant_off` | `{S}` (spindle_on), `message` (stops) | `M3 {S}`, `M5`, `M0 ({message\|comment})`, `M1 (…)`, `M8`, `M9` |
| `on_comment` | a comment | `info`, `operation`, `pass`, `warning` | `text` | `({text})` |
| `on_close` | the end of the program | | `{X:park} {Y:park} {Z:park}` | `G0 {X:park} {Y:park}`, `M30` |

In every handler: `units` (`G21` or `G20`), `wcs` (the work offset), `prop.<name>` (a property's value), and any true/false
property as a flag. `tool.number` is the tool's resolved number and `tool.offset` its length offset register (see "Tool
numbers").

## Tool numbers

The rack slot belongs to the machine, not the tool. At export, each tool the program uses gets its number, first match wins:

1. the machine's tool rack: an entry `{ "tool_id": "…", "number": 5, "length_offset": 15 }` in the machine's `toolTable`;
2. the tool's own `number` (set in the tool manager);
3. the order of first use (1, 2, 3…), only when `caps.tool_numbers = "first_use"`; these numbers skip every number that
   steps 1 and 2 gave to a tool of the same program, so a numbered tool used later never shares its number.

With `tool_numbers = "required"` a tool that reaches step 3 stops the export with its name. Two different tools of one
program on the same number stop the export with every post. `{tool.offset}` is the rack entry's `length_offset` (when the
number came from the rack), else the tool's own `length_offset`, else the number: `T5` gets `H5`. A program split into one
file per tool numbers each file on its own, so with `first_use` a tool can have another number in its own file than in the
whole program; give the tools rack slots when that matters.

The spindle start, the spindle stop and the stop for the operator must write a line (with `atc`, the tool change too): a
template that writes nothing for one of them stops the export, so a stop can never vanish from a program.

## Template rules

- A template is lines; blank lines and the whitespace around each line are left out.
- `{X}` is a word: its letter and value, or nothing (by its mode). `{X:park}` is the park position (axes the park does not
  give: the current position). `{G}` is the motion code. Write `{S}`, not `S{S}`: the word writes its letter, and a letter
  right before its own word is a load error.
- `{name}` is a variable; `{name|comment}` makes it safe inside a comment (plain ASCII, `(` `)` as `-`, `″` and `"` as
  `in`); `{name|plain}` makes it a plain message (ASCII, no `;` `(` `)`, a space between a letter and a number so "Z0" does
  not read as a move, trimmed). `{{` and `}}` write braces.
- A placeholder with no value takes the whitespace before it with it: `{G} {X} {Y} {Z}` writes `G0 X10 Z5` when Y did not
  change. A line with placeholders of which none has a value is left out (a move that writes nothing, a park the program
  does not have).
- `?name: rest` writes `rest` only when the flag or true/false property `name` is on; `?!name: rest` only when it is off.
- A line that is `( … )` as a whole is a comment, written in the post's comment style: `paren` as it is, `semicolon` as
  `; …` (trimmed), `none` not at all. A `( … )` that ends a command line is an inline comment: ` (…)`, ` ; …` or left out.
  The machine's own start and end lines get the same treatment for whole-line `( … )`. A `;` in a template is plain text,
  so a paren-comment post can still write GRBL's `M0 ; message` for senders that show it.
- Nothing else: no loops, no expressions. Everything is checked when the package loads: an unknown word, variable, filter,
  flag, property, handler or key is an error with the file, the line and the fix, and so is a word or variable used in a
  handler where it has no value (`{tool.number}` in `on_rapid`).

Arcs need no template work: the engine fits runs of short cuts as arcs (on the numbers as your formats write them) and
renders each through `on_circular`, in any format, whenever `caps.arcs` is not `none` and the machine has arcs switched on
(trochoidal loops always). Line numbers (`N10`, step 10) and the machine's own start and end code are added by the engine
after everything else, for every package.

## Properties

```toml
[properties]
stop_spindle_before_tool_change = { type = "bool", default = true, title = "Stop spindle (M5) before M06" }
park_z = { type = "number", default = 30, title = "Park height" }
```

Types: `bool`, `int`, `number`, `text`, `enum` (with `options = ["a", "b"]`). A `bool` property switches `?name:` lines;
any property is written with `{prop.name}`. Phase 4a shows them in the machine manager and saves the values with the
machine; until then the defaults apply.

One property name is **well known**: `stop_spindle_before_tool_change`. When it is on (the machine's value, else the
package's default) and the post is an `atc` post, the export reads the finished text and refuses it if any `M6` comes while
the spindle runs (an `M3` with no `M5` since). The check reads the text, not the template, so a package can write the `M5`
any way it likes; it only has to be there.

## What the export checks in your text

Every program a post writes is read back by `verify`, which parses the final text itself and shares no code with the engine.
What it knows of your package is a small description built from the manifest: the comment style, that `I`/`J` are from the
arc's start, whether every motion code is written on every move (`[modal] motion always`), the dwell unit and the decimals
of `X`/`Y`/`Z`/`I`/`J`. It reads text without spaces (`G1X74.934Y70.699Z8.200`, `M06T11`, `G43H11`), lines that continue the
last motion (`X76.242Y69.401`), padded numbers and line numbers. Two checks stop the export:

- **The moves are the program's moves.** Each move line must go where the program's next move goes, in order, within half a
  unit of your last decimal (0.0005 mm with 3 decimals) plus 1e-9. A fitted arc must end where the cuts it replaces end, with
  every one of them on the arc within the fitter's tolerance (0.005 mm) plus the rounding of the numbers, going round its way.
  A move the program has that goes nowhere at your resolution (a feed-only `G1 F800`, a modal word that does not change) may be
  missing; a template line that goes nowhere (`G0{Z:park}`) may be extra. The machine's own start and end code is not checked
  (the export notes it when it has moves). The error names the line, its text and where the program goes instead.
- **With `tool_change = "atc"`, tool changes are safe:** the `T` loaded at each `M6` is the tool the program changes to (and
  every `T` word is one of its tools); with `length_offset = true`, a `G43` with that tool's `H` follows every `M6` before the
  first Z move; the spindle runs for every cutting move (a pen draws with it off: after `(Pen: …)`, or after an `M6` with a
  pen's tool number when the post writes no comments); with `stop_spindle_before_tool_change` on, no `M6` while it runs.

## Sketch: the TX-35 manifest of the spec

The TX-35 package itself comes in phase 4a. Its manifest as the spec shows it (section "Post packages") loads and renders
today, which the test `the_tx35_manifest_of_the_spec_loads_and_renders` checks against the spec's own text. For the test
program it writes, among others:

```
G0 G28 G91
G54
G17 G80 G49 G0 G40 G90
G53 G0 Z0
G21
G0Z5.000
M5
M06T1
G43H1
S18000M03
...
G1Z-1.000F250.0
X20.000F800.0
X30.000Y10.000
G3X20.000Y20.000I-10.000J0.000
...
G0X0.000Y0.000
M30
```

That is the VCarve output's form: no spaces, padded decimals (`trim = false`), `G1` and `F` written only when they change,
`M06T#` and `G43H#` with the tool's number, no comments. What phase 4a still has to settle (see the implementation log):
the generator's own `SpindleOn` event also reaches `on_command` (`M3 S18000` after the template's `S18000M03`), the
generator's `M5` before a change doubles the template's `?stop_spindle_before_tool_change: M5`, and `{Z:park}` writes the
current height rather than a park height of the machine. The rack slot numbers came with phase 3: with the machine's tool
rack set, `M06T{tool.number}` writes the slot.
