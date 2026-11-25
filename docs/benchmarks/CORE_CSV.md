# kh-core and CSV reader: first measurements

Date: 25 November 2026. Phase 0 task 5.

**These are NOT reference-machine numbers.** They were taken in a cloud build container (4 cores, Xeon at
2.1 GHz, 16 GB RAM, fast SSD, Linux). The reference stand-in in spec section 3 is 2 cores at about 1.6 GHz,
4 GB RAM and a spinning disk, so expect real numbers to be several times worse on disk-bound steps. Re-run on
the reference machine and replace this file.

Reproduce:

```bash
cargo build --release -p kh-formats --examples
target/release/examples/gen_big_csv 100000  100k.csv
target/release/examples/gen_big_csv 1000000 1m.csv
target/release/examples/gen_big_csv 1000000 wide1m.csv 120   # about 182 MB
target/release/examples/bench_csv wide1m.csv
```

The generated data is invented and seeded, so every run writes the same file.

| File | Rows | Size | First 100 rows | Whole file | Table in memory | Peak process memory |
|------|------|------|----------------|------------|-----------------|---------------------|
| `100k.csv` | 100,000 | 6.2 MB | 0.3 ms | 0.04 s | 4.4 MB | 8.7 MB |
| `1m.csv` | 1,000,000 | 63 MB | 0.3 ms | 0.38 s | 45 MB | 49 MB |
| `wide1m.csv` | 1,000,000 | 182 MB | 0.4 ms | 0.65 s | 164 MB | 169 MB |

Window access (100 rows at 2,000 positions across the table): slowest single window 0.09 to 0.16 ms.

## Against section 4 (container numbers only)

| Budget | Result here | Verdict |
|--------|-------------|---------|
| Open 20 MB CSV, first screen in 1.5 s | 0.3 ms for the first screen | Passes with a very large margin, but unproven on the reference machine |
| Open 200 MB CSV, usable first screen in 5 s, peak 400 MB | 182 MB file: first screen 0.4 ms, whole file 0.65 s, 169 MB peak | Passes here. A spinning disk will dominate the whole-file time |
| Scroll 1,000,000 rows, frame under 50 ms | Data access is under 0.2 ms per window | The data side is fine. Rendering is the open question and belongs to the shell spike |

## What this does and does not prove

- It proves the data structure (dictionary or packed text, windowed reads) is not the bottleneck and fits the
  memory budget with room to spare.
- It does not prove anything about start-up, rendering, input latency or disk speed on the reference machine.
- The whole file is read before the table is usable. The first-screen number is for `max_rows = 100`. In the
  app, show the first screen from a quick prefix read, then finish the full read on a worker thread.
- Peak memory is read from `/proc/self/status`, so it is only reported on Linux. A Windows equivalent
  (`GetProcessMemoryInfo`) is needed for the reference machine run.
