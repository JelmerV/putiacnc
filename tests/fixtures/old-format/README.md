# Files as PutiaCNC saved them before phase 3

`machines.json` (a machine list, `Machines::to_json`) and `toollibs.json` (the tool library store, `ToolLibs::to_json`) were
written by `main` at 9381123, before tools had numbers and machines a tool rack. `tests/tool_rack.rs` loads and saves them
and requires the same bytes back, so the new optional fields (`Tool.number`, `Tool.length_offset`, `Machine.toolTable`)
never appear in a file that does not use them. Do not regenerate them with a newer PutiaCNC.
