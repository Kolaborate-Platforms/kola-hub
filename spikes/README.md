# Shell spike (spec 5.4)

Throwaway demos. The losing one is deleted once the maintainer signs the shell decision.

- `tauri-grid/` (the egui demo was removed after Tauri was chosen). It is its own Cargo workspace (`[workspace]` in its manifest), so the
  main workspace, its CI and its `deny.toml` never see these dependencies.
- Both use `kh-core` and `kh-formats` by path. Neither reimplements CSV reading or the table.

## What each demo does

1. Reads the CSV named by the `KH_CSV` environment variable. The interface never receives the path.
2. Shows a virtualised grid of the whole table (100,000 rows in the test), fetching only the visible window
   through `Table::window`.
3. Lets one cell be edited (row 0, column 0). The edit is held in Rust. The source file is never written.
4. Has one button, "Go to last row".
5. No network. No telemetry. 1366 by 768 window.

Generate the test file:

    cargo run --release -p kh-formats --example gen_big_csv -- 100000 100k.csv

## Bench mode

Set `KH_BENCH_OUT` to a file path. The app then appends one JSON object per line, flushing each, and exits.

| Order | Line | Meaning |
|-------|------|---------|
| 1 | `{"event":"ready","ms":123.4,"rows":100000}` | First window of rows is painted. `ms` counts from the start of `main` |
| 2 | `{"event":"scroll","frames":600,"p50_ms":0,"p95_ms":0,"max_ms":0}` | 600 frames, each moving 500 rows further (wrapping). Times are the gap between painted frames |
| 3 | `{"event":"edit","ms":0}` | Cell 0,0 set from code; time until the frame showing it is painted |

If the CSV cannot be read, write `{"event":"error","message":"..."}` and exit with code 1.
Without `KH_BENCH_OUT` the app behaves normally and writes nothing.

Memory and start-up from outside the process are measured by `scripts/measure-shell.ps1`.
