# Golden G-code

These files freeze what PutiaCNC writes today, so a refactor (the post-processor work in
`docs/post-processor-spec.md`) can prove it changes nothing by accident. `tests/golden.rs` (module `matrix`)
regenerates every file and compares it **byte for byte**.

## What is here

One directory per project:

- every `*.project.json` under `examples/` (named after the file, e.g. `oval-sign/`);
- every built-in sample of Generate → Sample projects (`sample-name-plate/`, `sample-box/`, `sample-relief/`,
  `sample-shaped-relief/`, `sample-oval/`).

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
on a separate piece would also get `-plug` files, as the export names them (none of today's projects has one).

Next to each `.nc` is a `.notes.txt`: the generator's notes (`Output.notes`), one per line. Warnings are behaviour too;
an empty file means "no notes".

A G-code file over 1 MB would be stored as `<name>.nc.digest` instead (SHA-256, size, line count, the first and last
200 lines). None is that large today.

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
