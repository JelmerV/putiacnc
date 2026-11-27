# CLAUDE.md

PutiaCNC fork: 2.5D CAM in Rust. Core crate (`src/`, lib `putiacnc_core`, CLI `putia-gcode`) and the egui desktop
app (`gui/`, binary `putiacnc`). Current work: the post-processor plugin system in `docs/post-processor-spec.md`.
Read `ARCHITECTURE.md` and `docs/testing.md` before changing generation code.

## Build and test

Needs Rust 1.89+ (`rust-version` in `Cargo.toml`) and a C++ compiler (`clipper-sys` builds Clipper from C++).
On Linux the GUI also needs the window libraries CI installs:
`libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev`.
To *run* the GUI (screenshots under Xvfb) it also needs `libxkbcommon-x11-0`; without it the app panics at start
with "Library libxkbcommon-x11.so could not be loaded" (`scripts/shot.sh` then fails and prints that output).

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Narrower loops while working:

```bash
cargo test --test golden                      # G-code snapshots in tests/data
cargo test -p putiacnc-core                   # core only, no GUI build
cargo run --bin putia-gcode -- tests/data/two_ops_toolchange.project.json -o /tmp/out.nc
```

CI (`.github/workflows/ci.yml`) runs the same two commands; both must pass before a merge.

## Working rules

- **Never update golden files without stating why.** `UPDATE_SNAPSHOTS=1` (and any later `tests/golden/` regeneration)
  is only for intended output changes. Read `git diff tests/data` line by line and put the reason for every changed
  snapshot in the commit message. An unexplained golden change is a bug.
- **Core logic lives in `src/` with a test.** Anything that decides G-code, geometry, tool numbers or post behaviour
  goes in the core crate with a unit or integration test, not in `gui/`. The GUI only displays and edits.
- `verify`/`replay` stay independent of `toolpath` and of the posts: they read the final text. Do not share code
  between them (see `ARCHITECTURE.md`).
- Fixtures in `tests/data/tx35/` (VCarve post and its output, CRLF) are reference inputs: never edit them.
- Phases from the spec land in order, each only when its gate passes; phases 0–2 must not change any output.

## Testing like the end user

Test efficiently, but to the level a user would hit it. In this order:

1. **Unit / integration tests** for the logic (`cargo test`), including the case that failed.
2. **CLI flow**: run `putia-gcode` on a real project (`tests/data/*.project.json`, `examples/projects/`), read the
   G-code it writes, and for TX-35 work diff it against `tests/data/tx35/vcarve/*.nc`. Check the exit code and the
   `note:` lines, not just that a file appeared.
3. **UI**: build the app (`cargo build -p putiacnc-gui`) and look at the affected window with `scripts/shot.sh`
   (Xvfb + ImageMagick, no screen needed; see `docs/testing.md` for options and `PUTIACNC_*` variables). Open the
   screenshot and check it. `shot.sh` exits non-zero and writes no picture when the app dies or the window never
   draws, so a missing picture is a failure, never a pass. Use `scripts/shot-sweep.py` when a change touches many windows.

Report what was run and what was seen; say plainly if a step was skipped.
