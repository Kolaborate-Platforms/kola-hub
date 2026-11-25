# Shell spike: Tauri or egui (spec 5.4)

Date: 25 November 2026. Status: **not signed**. The tests that decide it have not been run.

**None of these are reference-machine numbers.** They come from a developer Mac (Apple M4 Pro, 12 cores, 24 GB,
macOS, release builds). The target is a 2 core Windows 10 laptop with 4 GB RAM and a spinning disk.

## What was built

Two demos that follow `spikes/README.md`: a grid over the whole CSV that fetches only the visible rows, one
editable cell, and a "Go to last row" button.

- `spikes/egui-grid`: eframe 0.37 with the OpenGL (`glow`) renderer.
- `spikes/tauri-grid`: Tauri 2.12 with plain TypeScript.

## Measured on the Mac

Median of the runs, with the range or worst value in brackets. `100k.csv` is 6 MB, `1m.csv` 63 MB and
`wide1m.csv` 182 MB (commands in `CORE_CSV.md`). The files were already in the disk cache.

| Measure | File | egui | Tauri |
|---------|------|------|-------|
| Program file size | | 4.29 MB | 4.73 MB |
| Start to rows on screen | 100k (5 runs) | 200 ms (156 to 234) | 386 ms (365 to 397) |
| Start to rows on screen | 1m (3 runs) | 337 ms (333 to 386) | 517 ms (514 to 530) |
| Start to rows on screen | wide1m (3 runs) | 416 ms (399 to 418) | 612 ms (604 to 620) |
| Scroll frame gap, typical (p50) | all | 8.2 to 8.3 ms | 17 ms |
| Scroll frame gap, p95 | all | 13.1 to 14.7 ms | 18 ms |
| Scroll frame gap, worst | all | 28 ms | 45 ms |
| Edit to screen | all | 3 to 13 ms | 33 to 35 ms |

Sizes are in MB of 1,000,000 bytes.

### Read these with care

- **Scroll and edit times are not a fair race.** The typical gaps match two frame rates, not two speeds. 8.3 ms
  is 120 frames a second, this Mac's screen rate. 17 ms is 60 a second, which we take to be the web view's own
  limit. On a 60 Hz laptop screen both would likely sit near 17 ms. Tauri's edit time also includes two full
  frames. Both are inside the 50 ms scroll budget here.
- Tauri was built with the strict `connect-src 'none'` rule from spec 5.3. This may push it onto a slower
  message route (not confirmed), which would be a cost of that rule.
- "Start to rows on screen" counts from the start of `main`, not from the double click, and covers reading the whole
  file. `scripts/measure-shell.ps1` measures from launch on Windows.
- One early Tauri run, made by the worker that built it, took about 49 s to report ready. It did not happen again
  in 11 later runs. The cause is not known. A hidden window that the system slows down is a guess. Keep the window
  visible while measuring and watch for it on Windows.
- Neither demo was clicked through by a person. Typing in the cell, the button and how the grid looks are unchecked.
- Memory was not measured. Tauri runs web content in separate processes, so a single-process number would mislead.

## Not measured (needs Windows)

| Item | Why it matters |
|------|----------------|
| Installer size and installed size | Budgets of 25 MB and 60 MB. Neither demo has a Windows build yet. egui also has no installer set up |
| Cold start and idle memory on the reference laptop | Budgets of 2 s and 150 MB |
| Clean Windows 10 with no internet and no WebView2 | Decides whether Tauri can use `skip`. See `WEBVIEW2.md` |
| Windows WebView2 with `connect-src 'none'` | On macOS Tauri still worked with it, probably by falling back to a slower message route. Unconfirmed on Windows |
| egui on an old graphics driver, a virtual machine or remote desktop | `glow` needs a newer OpenGL than the basic Windows driver offers. If it fails there, egui cannot start at all. This is the largest unknown on the egui side |

`.github/workflows/shell-spike.yml` builds both on a Windows runner and prints sizes. It has never run:
GitHub Actions is blocked by a billing lock on the account.

## Licences and dependency rules

`cargo deny check` with the repo's `deny.toml`, limited to the Windows target:

| Demo | Result | Detail |
|------|--------|--------|
| egui | Licences fail, everything else passes | The built-in fonts add `OFL-1.1` and `Ubuntu-font-1.0`. Turning them off removes both but we must then supply a font. Poppins, which spec 8 asks us to bundle, is `OFL-1.1` too |
| Tauri | Licences fail, everything else passes | Seven crates are `MPL-2.0` (a file-level copyleft licence): `cssparser` (two versions), `cssparser-macros`, `dtoa-short`, `option-ext`, `selectors` (two versions). They come with Tauri and cannot be dropped |

Spec 11.3 allows "MIT, Apache-2.0, BSD or similarly permissive". Whether `MPL-2.0` or the font licences count is
the maintainer's call.

Without the Windows limit, Tauri also fails the network ban: it pulls `reqwest` and `hyper` for phone builds, and
carries an unmaintained-crate notice (`RUSTSEC-2024-0370`). Neither HTTP crate is in the Windows, macOS or Linux
build. If Tauri wins, `deny.toml` needs a `[graph] targets` list.

Crates in the Windows build: egui 100, Tauri 209.

## Large tables

Both demos opened 1,000,000 rows and finished the scripted scroll. This does not prove smooth hand scrolling at
that size. Tauri relies on a 24 million pixel tall element, near what browser engines allow, and egui's scroll
position loses precision at that height. Either way the real grid will need its own scroll mapping.

## Provisional recommendation: egui

The decision rule in spec 5.4: Tauri wins only if it meets every budget **and** installs offline without a huge
bundle. Otherwise egui wins.

- On a machine without WebView2, the offline Tauri installer adds about 127 MB according to Tauri's documentation
  (not measured by us), against a 25 MB budget.
- Speed and program size do not separate the two on this Mac.

So egui leads, unless the clean Windows 10 test shows that the target machines reliably have WebView2, or egui
fails to draw on the reference laptop. Test those two things first. egui also costs more design work to match the
boards, and its text input and screen reader support should be checked before signing.

## What the maintainer needs to do

1. Fix GitHub Actions for the organisation, or build both demos on a Windows machine.
2. On the reference laptop, run `scripts/measure-shell.ps1 -Exe <demo.exe> -Csv 100k.csv` for each demo. The script
   has never been run, so expect to fix it.
3. Follow the test plan in `WEBVIEW2.md` on a clean Windows 10 machine. Start egui there too.
4. Decide on the licence questions above.
5. Sign the shell decision and the budgets.

| Decision | Signed by | Date |
|----------|-----------|------|
| Shell | | |
| Budgets (spec section 4) | | |
