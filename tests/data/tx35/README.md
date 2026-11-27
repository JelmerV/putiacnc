# TX-35 ATC fixtures

Reference material for the TX-35 post package (see `docs/post-processor-spec.md`, "Verification", item 4).
These files are inputs to tests: do not edit them. They keep their original CRLF line endings (`.gitattributes`).

| File | What it is |
| --- | --- |
| `TX-35_ATC.pp` | The machine's Vectric VCarve post-processor (mm, arcs, `M06 T#` + `G43 H#`). Not confidential, so it is committed as a test fixture. |
| `vcarve/Lowland.nc` | VCarve output, two tools: T11 (14000 rpm) then T10 (18000 rpm), arcs in the profile. The tool change is `M06T10` / `G43H10` / `S18000M03`. |
| `vcarve/Test_LL_logo_rev.nc` | VCarve output, one tool (T11), single-depth logo engraving, one arc. |

What the PutiaCNC TX-35 post is allowed to do differently from these files (spec, goal 1):

- `M5` (spindle stop) before every `M06` (property `stop_spindle_before_tool_change`, default on).
- First tool: `G54 X0 Y0` comes after `M06` / `G43` / `M03` instead of between them.

Note on the VCarve output: the header writes `G43 H11` (with a space) and the tool change `G43H10` (without),
and the first tool's spindle line is `M3S14000` while later ones are `S18000M03`. A comparison test should
normalise spaces and `M3`/`M03` rather than require these quirks.
