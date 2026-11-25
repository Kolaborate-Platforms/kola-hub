# What each Phase 0 and Phase 1 crate offers

The shell (`kh-app`) only forwards calls. Every crate below builds and tests with no GUI dependency. Types that
reach the interface derive `serde::Serialize` or `Deserialize`, use `camelCase`, and **never contain a file path**.
They carry opaque ids and display names. Errors reach the interface as a stable code plus a plain English message.

## kh-service (spec 5.1)

Every tool action as a plain function over the crates below: projects, files, recently deleted, settings,
backup and Clean sessions. `kh-app` (the window) and `kh-mcp` (the AI server) only forward to it. It has no window
code and returns no file paths.

## kh-mcp (spec decision 11, `contracts/mcp.md`)

The AI server, a console program named `kola-hub-mcp`. Hand-written JSON-RPC over standard input and output, no
async runtime and no MCP library. Depends on `kh-service` (never `kh-app`). `kh_mcp::serve(reader, writer,
data_dir)` answers one line at a time, reads the settings again and makes a new `Shell` for every tool call, and
closes the Clean session it opened. Tests drive it through in-memory readers and writers.

## kh-store (spec 6)

- `Project`: open or create the hidden `.kola-hub/` folder (`project.sqlite` with `PRAGMA user_version`,
  migrations, WAL and `synchronous=FULL`; `blobs/`; `trash/`). Refuse a newer version with a plain message.
- `Library`: the recent projects list, kept in a per-user data folder given by the caller. Maps an opaque
  `ProjectId` and `FileId` to paths. Paths stay in Rust.
- Content store keyed by SHA-256 (`blobs/ab/<hash>`), streaming, verified on read.
- Save beside the original and never over it: `name (cleaned).csv`, then `name (cleaned 2).csv`.
- Recently deleted: move to trash with a journal, restore, empty now, purge older than the retention days.
  An interrupted move is repaired on the next open.
- Storage budget: usage, limit (default 5 GB), `Ok`, `Warn` at 80 percent, `Over`. Never delete automatically.
- Atomic writes: the vendored `kh_store::atomic`.

## kh-backup (spec 6)

`create(sources, destination, passphrase, progress)`, `restore(archive, folder, passphrase, limits, progress)`,
`test(archive, passphrase, scratch)`. A source is a label and a folder. Format: `backup-format.md`.

## kh-settings (spec 8)

- Brand tokens (spec 8 colours, font name), exported as CSS variables.
- Catalogue: `strings/en.json`, `{ "key": "text" }` or `{ "key": { "one": "...", "other": "..." } }`, `{name}`
  placeholders, fall back to English, a missing key shows the key. No sentences joined in code.
- Settings file in a folder given by the caller, saved atomically. A bad file is kept aside and defaults load.
  Fields: language, check for updates (off until `kh-update` exists), storage limit, trash retention days,
  labeller name.
- Shortcut table: Ctrl+K, Ctrl+Z, Ctrl+Y, Ctrl+1 to Ctrl+6.

## kh-clean (spec 7.1)

- Profile a `kh_core::Table`: per column detected type, share of cells that do not fit, distinct and empty
  counts; whole file rows, wrong cells, exact duplicate rows. Issues biggest first with the cells to highlight.
- Steps as a versioned, serialisable list: trim spaces, make dates (day first by default), text to numbers,
  merge spellings (the person picks the groups), remove duplicate rows, remove empty rows, rename column,
  change type. Reorder, disable, delete. Undo and redo move along the list.
- Preview on a row window: old value, new value, cells that need a person.
- Replay on a new file: match report, stop when columns are missing, result summary, rows that need a person.
- Output goes to a writer the caller opens. CSV export neutralises cells starting `=`, `+`, `-` or `@`.
