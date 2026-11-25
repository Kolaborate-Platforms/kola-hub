# Kola Hub: build spec

Version 1.0, 25 November 2026.
Repository: `Kolaborate-Platforms/kola-hub`. Licence: Apache-2.0.

This is the document a coding agent builds from. Read it top to bottom once, then work from section 14
(first tasks). When this spec and any other document disagree, this spec wins. When something here is
marked **Proposed**, **Unverified** or **Open**, do not treat it as settled: measure it, check it, or ask a maintainer.

---

## 1. What we are building

Kola Hub is a suite of small desktop tools that work fully offline, are light enough for old, cheap
Windows laptops, and share one look, one project folder and one set of keyboard shortcuts so they feel
like one product. The first two tools are **Clean** (fix a messy table) and **Annotate** (label text,
then images). Sheets, Docs, Slides and Dashboards come later and only after a pilot.

What makes it different from LibreOffice, OpenRefine and Label Studio is not the feature list. It is that
it installs from a USB stick with no internet, starts fast, stays small, and a first-time user can finish a
real job without help, on the machines our users actually own.

### Non-goals (do not build these, and push back if asked to)

- No accounts, sign-in, Firebase, Convex or any link to the Kolaborate platform or Praxis.
- No servers, relay, sync, sharing, teams or live co-editing.
- No telemetry, analytics, crash upload or "phone home" of any kind. The only network call in the whole
  product is an optional update check (section 9).
- No phone or tablet app. Deep data work is desktop work.
- No AI model inside the app in version 1. A local server lets an AI tool the person already uses work with the
  tools (decision 11). It is off until the person turns it on, and it uses no network.
- No plugin system in version 1.

---

## 2. Decisions

These decisions were accepted on 25 November 2026. Status: **Locked** means build on it. **Gate** means build on it unless the Phase 0 spike
disproves it. **Open** means it does not block the build.

| # | Decision | Status | Notes |
|---|----------|--------|-------|
| 1 | First tools: Clean, then Annotate. Annotate starts with text records, then images (boxes first, shapes later) | Locked | Sheets, Docs, Slides, Dashboards wait for the pilot gate |
| 2 | Shell: Tauri with a TypeScript front end over a GUI-free Rust core | Locked | Chosen on 25 November 2026 after the Phase 0 spike (`docs/benchmarks/SHELL_SPIKE.md`). Windows measurements are still to be taken. See 5.4 |
| 3 | One app, tools as modules, each tool its own crate so any can leave later | Locked | One installer, one signature, one update path |
| 4 | Open source, Apache-2.0, public repo | Locked | See 11.3 for reuse and provenance rules |
| 5 | Name Kola Hub, repo `kola-hub` | Locked | |
| 6 | Builder: coding agents, started by a maintainer one phase at a time, and a maintainer reviews every PR | Locked | No human-hours schedule exists. Phases are gated by exit tests, not dates |
| 7 | Who funds upkeep | **Open** | Does not block phases 0 to 3. Must be answered before 1.0 because of the code-signing certificate (yearly cost). |
| 8 | Design | Locked | The desktop design boards exist (section 13). Boards 1 to 21 are the target. Boards 22 onward are web drafts for later tools |
| 9 | Focus: offline behaviour, install size and performance on low-end machines, above feature count | Locked | Enforced by budgets in section 4 and CI gates in section 11 |
| 10 | Windows 10 and 11 first, 64-bit. macOS and Linux follow from the same code | Locked | |
| 11 | A local MCP server (`kola-hub-mcp`) so AI tools can use Kola Hub's tools | Locked | Added on 25 November 2026. Standard input and output only, off by default, only projects already opened in the app, no permanent delete, no backup or passphrases. See `docs/contracts/mcp.md` |

---

## 3. Who it is for, and the hardware we design against

First users: learners and early-career data workers, then small
organisations that cannot buy a tool per task. Typical machine: a shared or second-hand Windows laptop,
patchy or no internet, little free disk, no admin rights.

**Reference machine.** The maintainers will name the slowest real laptop available for testing. Until then, use
this stand-in for every benchmark and say so in the results:

- Windows 10 64-bit, 2 CPU cores at about 1.6 GHz, 4 GB RAM, 1366 by 768 screen
- Spinning hard disk or throttled disk I/O
- Free disk: 5 GB
- No internet, no admin rights, no other software assumed (not even Excel)

Record the real machine in `docs/benchmarks/REFERENCE_MACHINE.md` once it is chosen.

---

## 4. Budgets (Proposed)

These numbers are my starting proposal, not measurements. Phase 0 replaces them with measured baselines and
A maintainer signs off the final budgets. After that, CI fails a PR that breaks a budget (section 11.1).

| Area | Budget |
|------|--------|
| Installer size (Windows, offline installer with WebView2 inside) | 155 MB or less: 25 MB for the app plus about 127 MB for WebView2, accepted on 25 November 2026 |
| Installed size, all phase 1 and 2 tools | 60 MB or less |
| Cold start to usable home screen, reference machine | 2 s or less |
| Idle memory with home screen open | 150 MB or less |
| Open a 20 MB CSV (about 100,000 rows) and show the first screen | 1.5 s or less |
| Open a 200 MB CSV (about 1,000,000 rows) | Usable first screen in 5 s or less, peak memory 400 MB or less, no crash |
| Scroll a 1,000,000 row grid | Frame time 50 ms or less, no blank flashes |
| Apply any single Clean step to 100,000 rows | 1 s or less (preview on the visible window is instant) |
| Label a text record (key press to saved) | 30 ms or less on a spinning disk |
| Network calls other than the update check | Zero |
| Installer works with no internet | Yes, on a clean Windows 10 machine |

**The WebView2 problem (Unverified).** Tauri on Windows draws with Microsoft's WebView2 runtime. Windows 11 and
up-to-date Windows 10 normally have it. An old Windows 10 machine with no internet may not. Tauri's offline
installer mode embeds the runtime, which I believe adds on the order of 100 MB or more to the installer. That
would break the size budget and the USB-stick promise. Phase 0 must settle this with a real test on a clean
machine and write the answer in `docs/benchmarks/WEBVIEW2.md`. If the only offline-safe option is a huge installer,
that is a strong reason to choose egui (5.4). Docs to check: the Tauri v2 Windows installer page and the
`bundle.windows.webviewInstallMode` setting.

---

## 5. Architecture

### 5.1 Workspace layout (Proposed)

One Cargo workspace, `crates/` for all Rust, `ui/` for the TypeScript front end (only if Tauri wins the gate).

```
crates/
  kh-core        table engine: typed columns, streaming, step replay, undo. No UI, no I/O
  kh-formats     CSV, XLSX read/write, image decode, JSON Lines, COCO and YOLO export
  kh-store       project folder, project database, content store, trash, atomic writes, storage budget
  kh-backup      sealed passphrase backup and restore
  kh-settings    settings, language strings, shortcuts, brand tokens
  kh-update      update check and install. The ONLY crate allowed to use an HTTP client
  kh-clean       the Clean tool: profiler, step library, replay
  kh-annotate    the Annotate tool: label sets, queue, review, agreement, export
  kh-app         the window shell and command layer (thin). Replaceable
ui/              TypeScript front end for kh-app (Tauri path only)
docs/            this spec, benchmarks, provenance, design exports
scripts/         cleanup script, CI helpers, fixture generators
```

Rules that keep the shell replaceable:

- Every crate except `kh-app` compiles and tests with no GUI dependency.
- Tools expose commands as plain Rust functions that take and return serialisable types. The shell only
  forwards them. If we ever swap Tauri for egui, only `kh-app` and `ui/` change.
- **The interface never sees file paths.** It gets opaque ids for projects, files and records. All path
  handling stays in Rust.
- Tools register through a small trait (name, icon, commands, "open this file"). Do not build a plugin loader.

### 5.2 Table engine (`kh-core`)

This is the part that decides whether the product feels fast. Design for big files on small machines:

- Stream CSV from disk. Do not read a whole file into a `String`.
- Hold data in typed columns. Strings are interned or dictionary-encoded where it pays off (market names,
  product names repeat heavily).
- The interface asks for a window of rows (about 100 at a time) and never receives the whole table.
- A Clean "preview" runs the step list on the visible window only. The full run happens when the person saves.
- Undo and redo pop steps off the step list. They do not copy data.
- Keep the engine free of Tauri, `serde_json` on hot paths and any async runtime. Plain threads are enough.

### 5.3 Security and untrusted files

Every file a user opens is untrusted input.

- Parse with size and depth limits. XLSX is a zip: cap entry count, uncompressed size and ratio (zip bombs).
- Never execute macros, follow external links or evaluate external references in a spreadsheet.
- On export to CSV or Excel, neutralise cells that begin with `=`, `+`, `-` or `@` (formula injection) unless
  the person is exporting a column they marked as a formula.
- Fuzz the CSV and XLSX readers in CI (`cargo fuzz` or `proptest`) and keep the corpus in the repo.
- Tauri CSP (if used): no remote origins at all. `connect-src 'none'`. Fonts and assets are bundled.

### 5.4 Shell gate (Phase 0)

Build the same small demo twice: a virtualised grid of a 100,000 row CSV with one editable cell and one
button, in Tauri and in egui (eframe). Measure on the reference machine and write the numbers to
`docs/benchmarks/SHELL_SPIKE.md`: installer size, installed size, cold start, idle RAM, scroll frame time,
input latency, and behaviour on a clean Windows 10 machine with no WebView2.

Decision rule: **Tauri wins if it meets every budget in section 4 and installs offline without a huge bundle.
Otherwise egui wins.** Either way the Rust core is identical. A maintainer signs the decision in the PR.

---

## 6. Data and storage

A project is a normal folder the person picks. Their files stay as ordinary CSV, Excel, image and text files.

- **Originals are never changed.** A tool writes a new file or a new version beside the original.
- **Working state** lives in a hidden folder `.kola-hub/` inside the project: one SQLite file
  (`project.sqlite`) with cleaning steps, labels, annotations and history, plus `blobs/` for stored content.
  The layout is Proposed; keep a schema version number from day one and write migrations.
- **Annotations are crash safe.** Use SQLite WAL with `synchronous=FULL` for annotation writes. The interface
  promises "saved after every label, power cuts lose nothing". Measure the cost on a spinning disk (budget in
  section 4).
- **Big files are stored once.** Snapshots and undo history for large files use a content-addressed store
  keyed by SHA-256, so ten versions that share most of their data do not cost ten times the disk.
- **Atomic saves.** Write to a temporary file in the same folder, flush, then rename over the target. A power cut
  mid-save leaves the old file whole.
- **Recently deleted.** Removing an item moves it to a trash area with a journal. Default retention 30 days,
  user settable. Restore and "empty now" exist.
- **Storage budget.** The person sets a limit (default 5 GB). Warn at 80 percent. Never delete anything automatically.
- **Recent projects list** lives in the per-user app data folder. It is never uploaded.
- **No sync.** Moving work to another computer means copying the folder or restoring a backup.
- **Crash recovery.** A small journal of recent changes is replayed on next launch; show "Restored your last changes".

### Backup (`kh-backup`)

One sealed archive for every project, to a USB drive or folder, protected by a passphrase.

- Pure Rust, no network. Authenticated chunked encryption (ChaCha20-Poly1305), key from the passphrase with
  Argon2id, a hash manifest, and hostile-archive checks on restore. Agent HQ's `hq-backup` crate is the reference
  design (section 12).
- If the passphrase is lost the backup cannot be opened by anyone. The create screen must say so in plain words
  and ask the person to type the passphrase twice.
- "Test the last backup" restores a few sample files into a scratch folder and compares hashes.

---

## 7. The tools

Screens are on the design boards (section 13). Board numbers below match their titles.

### 7.1 Clean (Phase 1). Boards 9, 10, 11

Job: open a CSV or Excel file, show what is wrong, fix it step by step, save a clean copy, and replay the same
steps on next week's file.

Capabilities for Phase 1:

1. **Profile.** Per column: detected type, share of cells that do not fit, distinct count, empty count.
   Whole file: row count, cells that look wrong, exact duplicate rows. Show issues biggest first and highlight
   the offending cells. Nothing changes until the person applies a step.
2. **Step library.** Each step is a typed, replayable operation:
   - trim spaces
   - make dates real dates (day first by default; the person can switch; handle at least `12/11/2026`,
     `12 Nov 2026`, `2026-11-13`; report unreadable ones such as `14-Nov` instead of guessing the year)
   - turn text into numbers (remove currency words and symbols such as `UGX`, thousands commas)
   - one spelling per category (cluster near-matches by case, spacing and simple edit distance; the person ticks
     which groups to merge; ambiguous groups are left alone)
   - remove exact duplicate rows
   - remove empty rows, rename column, change type
3. **Preview.** Each step shows old value struck through and new value in green on the visible rows before it is
   applied. Cells that need a person (for example, an empty price) are marked and left unchanged.
4. **Step list.** Ordered, named, editable (reorder, disable, delete). Saved in the project database as JSON with
   a version number. Undo and redo move along the list.
5. **Reuse.** Run a saved step list on a new file with the same columns. Show a match report (columns found or
   missing), a result summary (rows in, rows out, rows needing a person) and a list of rows that need a person with
   an inline fix. If columns do not match, say which and stop; do not guess.
6. **Output.** Save as a new CSV or Excel file next to the original. Offer "Open in Annotate".

Phase 1 does not include: formulas, charts, pivot tables, joins across files, regex editors. Resist adding them.

**Acceptance tests (Phase 1 gate input).** Generate `tests/fixtures/kisenyi_survey.csv` with a seeded script
(`scripts/gen_fixture.py` or a Rust bin). It must contain: a date column in four formats, one market spelled
seven ways, a price column with `UGX` and commas, empty prices, trailing spaces, and a known number of exact
duplicate rows. The test asserts the exact counts the fixture was built with, then asserts that replaying the
saved step list on a second generated file ("week 4") gives the expected output. The numbers in the design
boards are illustrative; the fixture defines the truth.

### 7.2 Annotate (Phase 2). Boards 12, 13, 14, 15

Job: label records quickly with the keyboard, compare two labellers, export.

Capabilities, text first:

1. **Set up.** Pick the file and the column that holds the text. Define labels (name, colour, number key, short
   description). Optional multi-label. Notes for labellers shown beside every record. A free-text "who is
   labelling" name (not an account, never verified).
2. **Label.** One record at a time, large text. Keys 1 to 9 label and advance, Space skips, Z undoes, arrows move,
   F flags for review. Progress bar. Recent history list. Every label is written to disk immediately (section 6).
3. **Second labeller without a network.** "Make a copy for someone" writes a self-contained pack (records and label
   set, no existing labels). They label on their own computer and send back a labels file by any means.
4. **Review.** Load another labeller's file. Show agreement percentage (and Cohen's kappa for single-label sets),
   a list of disagreements with a one-click choice of Labeller A, Labeller B or Other, and record who decided.
5. **Export.** CSV (one row per record) and JSON Lines. Options: keep who labelled each record; only records both
   agree on.

Images (after text is solid): open a folder of photos, draw boxes, assign a label by number key, move and resize,
delete, next photo. Export COCO JSON and YOLO text. Do not build polygons, brushes or auto-labelling in Phase 2.

**Acceptance tests.** Property tests that no label is lost when the process is killed mid-session (kill the
process in a loop in CI). A fixture with two labellers and known disagreements; assert agreement, kappa and
export contents. A keyboard-only run through 100 records.

### 7.3 Later tools (Phase 4, do not start before the pilot gate). Boards 16 to 19

Sheets (table engine plus the HQ calc engine), Docs and Slides (HQ markdown and export crates), Dashboards
(charts from a cleaned table). Boards 22 onward are the old web-app designs for these and for import/export.
They still contain wording about accounts, sharing, Praxis, Google and phones that must be removed when each
tool is built. The HQ calc engine matched Excel on 88 percent of 57 test workbooks, so Sheets parity is a known
gap, not a solved problem.

---

## 8. Shared look and behaviour

- **Brand tokens.** Primary green `#35b544`, secondary `#e1f4e4`, tertiary `#2a8a34`, grey `#424242`, yellow `#fbdf64`.
  Font Poppins. Bundle it (subset to Latin, two weights, woff2). Never load fonts from the internet. Keep the
  tokens in `kh-settings` so every tool reads one source. Text contrast 4.5:1 or better.
- **Language.** English first. All user-visible strings go through one translation layer from day one, so Luganda
  and Swahili can be added without touching code. No string concatenation for sentences.
- **Keyboard first.** Command palette on Ctrl+K, Ctrl+Z and Ctrl+Y for undo and redo, Ctrl+1 to 6 to switch tool.
  Every action reachable without a mouse. Visible focus rings.
- **Rail and home.** The left rail lists Home, Files, the tools, Backup and Settings, and shows "Works offline".
  Home shows recent projects, tool tiles and an update notice. See boards 5 to 8.
- **Plain language.** Written for people who do not know technical words. No jargon in the interface.
- **Low-end display.** Everything must work at 1366 by 768. No animations that cost frames. Respect the system
  "reduce motion" setting.

---

## 9. Install, release and update

- **Per-user install, no admin rights.** No hidden shells, no silent elevation, never replace the app while it
  runs. (Storytold's launcher was flagged by antivirus software for running a hidden elevated PowerShell update
  script. We do not do that.)
- **Signed releases.** Each release publishes a `SHA256SUMS` file. Windows builds are code-signed. Unsigned
  builds trigger SmartScreen warnings, so signing is a real cost to budget (decision 7).
- **Update check is optional.** The app checks a small signed feed we host, shows a notice, and installs only when
  the person agrees, when the app closes. A setting turns the check off. The same installer is offered as a plain
  download for USB sharing and "Install an update from a file" works with no network.
- **Only `kh-update` may use an HTTP client.** Enforce with `cargo deny` (ban the HTTP crates in every other
  crate) and a test that scans the dependency tree.
- **Release testing.** Build on a native runner per operating system. Every installer gets a smoke test: install,
  launch, open the fixture, save, uninstall, on a clean Windows 10 VM.
- **Disk space on the build machine.** Rust debug builds grow very large. Ship `scripts/cargo-gc.sh` (or
  equivalent) on day one and run it. Check free disk before big builds.

---

## 10. Phases and gates

Each gate is a yes or no test. Do not start the next phase until the gate passes and a maintainer says so.

| Phase | Scope | Gate (all must be true) |
|-------|-------|-------------------------|
| 0 Foundations | Repo scaffold, CI on Windows, licence files, provenance file, shell spike (5.4), WebView2 test, craft-libs and HQ crate checks, measured budgets, home screen, settings, project store, backup | Shell chosen and signed. Budgets measured and signed. CI green on Windows. Offline install tested on a clean Windows 10 machine. Backup round trip passes |
| 1 Clean | Clean as in 7.1 | Fixture tests pass. A first-time user, watched, finishes a real cleaning job unaided on the reference machine. Budgets hold |
| 2 Annotate | Text, then images, as in 7.2 | Kill-test passes. Two real annotators do a real job and the comparison matches their own count. Budgets hold |
| 3 Pilot | Ten named pilot users, release downloads, update-check counts, interviews | The three tests in the Plan: (a) a first-time user finishes a real job without help, (b) it runs well on a low-end laptop, (c) the whole suite installs from a USB stick with no internet. Plus a written comparison against LibreOffice, OpenRefine and Label Studio on weight and first-job ease |
| 4 Later tools | Sheets, then Docs, Slides, Dashboards, one at a time | Per tool, to be set after the pilot |

Measuring success without tracking: release download counts, update-check requests to our own feed, and
interviews with ten named pilot users.

---

## 11. Engineering rules

### 11.1 CI gates (set these up in Phase 0)

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`,
  `cargo test --workspace --doc`, `cargo deny check` (licences, bans, advisories).
- Windows runner is required. Linux and macOS jobs may be added.
- A benchmark job compares against the signed budgets and fails the PR on regression.
- A size job fails the PR if the installer or installed size exceeds the budget.
- Front end (Tauri path): typecheck, lint, build.
- No Rust file over 1,500 production lines. Split by concern.

### 11.2 Working agreements

- Work on a branch, open a pull request, keep changes small and focused, add tests for new behaviour.
- Prefer a boring dependency we understand over a clever one. Every new dependency needs a reason in the PR,
  a licence check, and a size check.
- Write the plain-language user string first, then the code.
- A shortcut you knowingly leave goes in `docs/techdebt/<slug>.md`, one file each.
- A user-visible change adds `changelog.d/<slug>.<kind>.md` (do not edit a shared changelog file).
- No secrets, real names, personal paths or real hostnames in code, tests or fixtures. Use `example.com` and
  placeholder names. Test data must be invented.
- Writing style for docs and strings: plain, no stock AI phrases, no em-dash overuse.

### 11.3 Reuse, licensing and provenance (important)

The repo is open source under Apache-2.0. That has consequences for what we may copy.

- **storytold / craft-libs** (Apache-2.0 per the organisation listing, to be confirmed per crate): we may depend on
  or copy crates after a short check. The Plan names crates for CSV, spreadsheets, XLSX, atomic file writes,
  settings, updates and translations (`craft-atomic` is named). Crate names, versions, size and quality are
  **Unverified**: I read the list, not the code. In Phase 0, for each candidate crate record in
  `docs/PROVENANCE.md`: name, version or commit, licence file, what we use, size cost, and a keep/replace verdict.
  Do not take a git dependency on a moving branch; pin a commit or vendor with attribution.
- **Agent HQ / hq-suite**: code copied from there into this repository needs the explicit sign-off of its owner for
  each file or crate, because it is being relicensed under Apache-2.0. Record each in `docs/PROVENANCE.md` with
  source repository, path, commit and the date of sign-off. Without that entry, do not copy it. Reading it for
  design ideas is always fine.
- **Everything else:** only MIT, Apache-2.0, BSD or similarly permissive licences. No GPL or AGPL code. `cargo deny`
  enforces this.
- Candidate well-known crates to evaluate (licences and sizes still to be checked): `csv`, `calamine` (read
  XLSX), `rust_xlsxwriter` (write XLSX), `rusqlite` with bundled SQLite, `sha2`, `chacha20poly1305`, `argon2`,
  `zip` (with limits), `image`.

---

## 12. Reference projects

| Project | Why | Notes |
|---------|-----|-------|
| `storytold/craft-libs` and `storytold/craft-launcher` | Shape of the suite: shared libraries, one launcher, atomic saves, settings, i18n, update checks | Apache-2.0 per the organisation listing. Check each crate. Note the launcher's Windows antivirus incident (section 9) |
| Tauri v2 docs, Windows installer page | WebView2 install modes, per-user install, updater | Check before releases |
| LibreOffice Calc, OpenRefine, Label Studio | Comparison targets for the pilot | We have to beat them on weight and first-job ease on low-end machines |

---

## 13. Design boards

The design boards are not part of this repository. The requirements that matter are written into this spec.

| Boards | Content |
|--------|---------|
| 1 to 4 | The Hub at a glance, the key tools, three jobs start to finish, first run with no sign-up |
| 5 to 8 | Hub home, project browser, Settings, Backup and recently deleted |
| 9 to 11 | Clean: see what is wrong, fix step by step, reuse the steps |
| 12 to 15 | Annotate: set up, label text, label images, review and export |
| 16 to 19 | Later tools: Sheets, Docs, Slides, Dashboards (desktop layouts) |
| 20, 21 | How it is built; offline, saving and recovery |
| 22 onward | Web drafts to adapt for later tools. Not a target for phases 0 to 3 |

---

## 14. First tasks for the agent (Phase 0, in order)

1. Check free disk space. Create the Cargo workspace per 5.1, `rust-toolchain.toml`, `.gitignore`, `.editorconfig`,
   `scripts/cargo-gc.sh`. Add the official Apache-2.0 `LICENSE` text and a `NOTICE`.
2. Set up CI on a Windows runner with the gates in 11.1 (fmt, clippy, nextest, deny). Make it green on an empty
   workspace before adding code.
3. Write `docs/PROVENANCE.md` (empty table) and `CONTRIBUTING.md` (issue first, small PRs, licence rules).
4. Inspect `storytold/craft-libs`. Fill the provenance table for each candidate crate (11.3). Report which of
   CSV, XLSX, atomic write, settings, update and i18n are worth depending on.
5. Build `kh-core` skeleton: streaming CSV reader, typed columns, windowed row access, with a 1,000,000 row
   generated test file and a benchmark.
6. Run the shell spike (5.4) and the WebView2 test. Write the benchmark docs. Stop and ask a maintainer to sign the
   shell decision and the budgets before building further.
7. Build `kh-store` (project folder, SQLite file with schema version, content store, atomic save, trash, storage
   budget) and `kh-backup`, each with tests including kill-during-write.
8. Build `kh-settings` (tokens, language layer, shortcuts) and the home screen, Files browser, Settings and
   Backup screens from boards 5 to 8.
9. Build the installer pipeline and the clean Windows 10 offline-install smoke test.
10. Open a PR per task. Report measured numbers against section 4 in every PR that could change them.

---

## 15. Open items

- Name the reference machine (section 3).
- Decide who pays for upkeep, including the signing certificate (decision 7).
- Pick two real annotators and a real annotation job to design with before Phase 2 starts.
