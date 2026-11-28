# Golden G-code

These files freeze what PutiaCNC writes today, so a refactor (the post-processor work in
`docs/post-processor-spec.md`) can prove it changes nothing by accident. `tests/golden.rs` (module `matrix`)
regenerates every file and compares it **byte for byte**.

## What is here

One directory per project:

- every `*.project.json` under `examples/` (named after the file, e.g. `oval-sign/`);
- every built-in sample of Generate → Sample projects (`sample-name-plate/`, `sample-box/`, `sample-relief/`,
  `sample-shaped-relief/`, `sample-oval/`).
- the option cases (`opt-*/`, `pause/`, `pen/`, `pen-tool-change/`, `rest-pocket/`, `trochoidal/`, `photo-vcarve/`,
  `plug-piece/`): emitter paths the matrix does not reach, see "Option cases" below.

Each project runs on a fixed machine (the app's default machine: 300 × 200 × 60 mm, rapid 2000 mm/min, safe Z 5 mm,
no line numbers, no machine limits), so nothing depends on the local setup. The machine is applied the way the app
applies a chosen machine: it sets the dialect, line numbers and safe Z of the job. A sample is made the way the app
makes it, in the units of the case (an imperial sample has inch-rounded sizes), with the bundled tool list; tool ids are
pinned (`t01`, `t02`, ...) because the real ones come from the clock.

File names: `<dialect>-<units>-<arcs>[-tool-<n>].nc`

| Part | Values |
|---|---|
| dialect | `grbl` (the empty dialect), `grblhal`, `fluidnc`, `mach`, `marlin` |
| units | `metric`, `imperial` |
| arcs | `noarcs`, `arcs` (`machine.arcs`) |
| split | none: the whole program as one file; `-tool-<n>`: file n of "a file per tool", only for projects where that makes more than one file |

The text is what `putia-gcode -o` and File → Export G-code write (`pieces::generate_parts`, `split::generate_files`,
including the summary header from `summary::add_header`), with no newline added at the end. A project with inlay plugs
on a separate piece also gets `-plug` files, as the export names them (the `plug-piece/` case below).

Next to each `.nc` is a `.notes.txt`: the generator's notes (`Output.notes`), one per line. Warnings are behaviour too;
an empty file means "no notes".

A G-code file over 1 MB would be stored as `<name>.nc.digest` instead (SHA-256, size, line count, the first and last
200 lines). None is that large today.

## Option cases

The matrix puts the same fixed machine on every project, and none of the projects above pauses, draws with a pen or
uses some of the other operations, so parts of the emitter never show in it. A short list of explicit cases in
`tests/golden.rs` (`cases()`) covers them, each in its own directory and with the same file names. They were
added before phase 1 of the spec (the emitter refactor), from the unchanged generator; the two `rack` cases came with
phase 3. Every case runs in all 5 dialects, metric, arcs off, unless the table says otherwise; each also writes the per-tool files when it has more
than one tool.

| Directory | Project | What it covers |
|---|---|---|
| `opt-numbered/` | small two-tool job (below) | line numbers (`Machine.numbered`), arcs off and on: numbering runs after arc fitting, and again after the summary header and split comments |
| `opt-start-end-code/` | small two-tool job | the machine's own start and end lines (`start_code` / `end_code`), with a comment line Marlin rewrites to `;`, blank lines and spaces that are dropped |
| `opt-start-end-code-numbered/` | small two-tool job | both: numbering covers the inserted lines |
| `opt-wcs-g56-spin0/` | small two-tool job | `job.wcs` = 2 (G56) and a spin-up of 0 s (no `G4` after `M3`) |
| `opt-wcs-g59-1/` | small two-tool job | `job.wcs` = 6: G59.1 on Mach3 / LinuxCNC, G54 on the dialects that do not have it |
| `pause/` | `tests/data/hardware_m0_aircut.project.json` | an operation pause with the spindle running (`M5`, `M0` with the message), a pause that ends a per-tool file |
| `pen/` | `tests/data/hardware_pen_test.project.json`, with a pen-down dwell of 0.125 s on operation 2 | pen drawing (`select_pen`, `draw_op`, `G4 P0.13` with 2 decimals; `G4 P130` on Marlin, in milliseconds), a pause with no spindle running; metric **and imperial** (the draw comment gives the pen heights in the output units) |
| `pen-tool-change/` | small two-tool job with a pause (default message) after operation 1 and a pen drawing between the cutters | `select_pen` after a cutter, `select_tool` after the pen, the default pause message |
| `rest-pocket/` | a 60 x 40 mm pocket, 6 mm cutter with a 2 mm Corners tool (as `PUTIACNC_SHOW_REST` makes it) | the pocket rest pass (`rest_pass`, "corners and narrow parts") |
| `trochoidal/` | `tests/data/pocket_island.project.json` with trochoidal clearing, as `tests/trochoidal.rs` sets it | with arcs off, only the loops are fitted as G2/G3 (the per-range fitter) |
| `photo-vcarve/` | a gradient picture, as `tests/photo_vcarve.rs` makes it | photo V-carve |
| `plug-piece/` | an inlay with its plug on a separate 8 mm piece, as the tests in `src/pieces.rs` make it | the `-plug` file of `pieces::generate_parts` |
| `rack/` | `tests/data/rack_two_tools.project.json`: the two-tool job of `two_ops_toolchange` on a Mach3 / LinuxCNC machine whose tool rack (`toolTable`) has the 1/8" end mill in slot 5 and the 6 mm end mill in slot 7. **Mach3 / LinuxCNC only** (the one built-in that writes tool numbers) | phase 3: `T5 M6` / `T7 M6` from the rack, also in each per-tool file |
| `rack-first-use/` | the same project without its rack, the 6 mm end mill numbered 1 on the tool itself. **Mach3 / LinuxCNC only** | phase 3: the 1/8" end mill, used first, is numbered by first use and skips the taken 1 (`T2 M6`, then `T1 M6`); in its own per-tool file it is `T1` (each file is numbered on its own) |

The small two-tool job is made in code (`small_job`): a 60 x 40 x 6 mm stock, a 3 mm cutter round a 15 x 10 mm
rectangle, then a 6 mm cutter inside a 16 mm circle, 1 mm deep in one pass, no ramp. The relief detail pass (the
third tool of a relief, `tests/data/relief-ledges.project.json`) is not a case: `sample-relief/` already has it, and
that file alone would add 2 MB.

Two things about Marlin in these files. The pen's dwell is `G4 P` in **milliseconds** (`G4 P130` for 0.13 s), because
Marlin's `G4` reads `P` as milliseconds (`S` as seconds); the other dialects read `P` as seconds and write `G4 P0.13`.
Until the change after phase 2 (see `docs/implementation-log.md`) Marlin got `G4 P0.13` too, so the machine waited 0.13 ms;
only the `marlin-*` files of `pen/` changed then, in their `G4` lines. Marlin has no spindle wait (the operator runs the
spindle), so the pen's dwell is the only `G4` the post writes there; the `G4 P1` in `opt-start-end-code*/` is the
machine's own start code, written as given. And the behaviour that is still kept as it is: on Marlin the `-plug` file's
header still says "Stock", because `pieces.rs` looks for `(Stock ` after Marlin has turned the comment into `; Stock`.

## Regenerating

```bash
UPDATE_GOLDEN=1 cargo test --test golden
```

This rewrites every project directory (stale files are removed). Without it, a mismatch fails and prints the file, the
first differing line with two lines of context, the expected and the actual line. A file that no case makes any more,
or a directory with no project, fails too.

## The rule

**Every golden change needs a reason in the commit message.** Run `git diff --stat tests/golden`, read the diff, and
say for each changed group of files why the output changed (e.g. "mach-*: G40 G49 G80 moved after G54, intended").
A change nobody can explain is a bug. Refactors (phases 0–2 of the spec) must not change any file at all.

## Platform notes

The files are made on Linux (CI). Number formatting is Rust's own and the same on every platform, and Clipper works in
integers, but `sin`, `cos`, `atan2`, `powf` and `exp` come from the platform's maths library and can differ in the last
bit between Linux, macOS and Windows. After rounding to 3 decimals (4 in inches) that very rarely flips a digit: if a
coordinate differs by exactly one unit in the last place on another platform, that is the cause, not a regression.
`tests/golden/** -text` in `.gitattributes` keeps the LF line endings on Windows checkouts.
