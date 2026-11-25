# Bench mode

Set `KH_CSV` to a CSV file and `KH_BENCH_OUT` to an output path. The app appends one JSON object per line, flushing
each, and exits. `scripts/measure-shell.ps1` reads these lines.

| Order | Line | Meaning |
|-------|------|---------|
| 1 | `{"event":"ready","ms":123.4,"rows":100000}` | First window of rows is painted. `ms` counts from the start of `main` |
| 2 | `{"event":"scroll","frames":600,"p50_ms":0,"p95_ms":0,"max_ms":0}` | 600 frames, each moving 500 rows further. Gaps between painted frames |
| 3 | `{"event":"edit","ms":0}` | One cell set from code, timed until painted |

If the CSV cannot be read, the app writes `{"event":"error","message":"..."}` and exits with code 1. Without
`KH_BENCH_OUT` it writes nothing. Keep the window visible while measuring.
