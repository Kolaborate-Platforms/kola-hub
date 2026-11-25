# Commands between the interface and the shell

`kh-app` implements these Tauri commands. `ui/` calls them. Arguments and results use `camelCase`. Where a type
below is a crate type, its serde shape in that crate is the contract: read the source, do not guess. Nothing that
crosses this line holds a file path. Failures return `{ "code": "...", "message": "..." }` in plain English.

File and folder choosing happens inside Rust commands. The interface never opens a dialog itself. A command that
shows a dialog returns `null` when the person cancels.

| Command | Arguments | Result |
|---------|-----------|--------|
| `app_info` | | `{ version }` |
| `strings` | | the catalogue object from `kh_settings::ENGLISH_JSON` |
| `settings_get` | | `Settings` |
| `settings_save` | `settings` | `Settings` |
| `projects_recent` | | `RecentProject[]` |
| `project_pick` | | `ProjectView` or null (choose a folder) |
| `project_open` | `projectId` | `ProjectView` |
| `project_forget` | `projectId` | |
| `files_list` | `projectId` | `FileRow[]`: the files at the top of the folder |
| `file_trash` | `projectId`, `fileId` | |
| `trash_list` | `projectId` | `TrashItem[]` |
| `trash_restore` | `projectId`, `trashId` | |
| `trash_empty` | `projectId` | number removed |
| `backup_create` | `projectIds`, `passphrase` | `CreateReport` or null (choose where to save) |
| `backup_restore` | `passphrase` | `RestoreReport` or null (choose archive, then an empty folder) |
| `backup_test` | `passphrase` | `TestReport` or null (choose archive) |
| `clean_open` | `projectId`, `fileId` | `CleanSession` |
| `clean_close` | `sessionId` | |
| `clean_profile` | `sessionId` | `Profile` of the table after the enabled steps |
| `clean_window` | `sessionId`, `start`, `count` | `{ start, columns, rows }` after the enabled steps |
| `clean_preview` | `sessionId`, `step`, `start`, `count` | `Preview` |
| `clean_suggest_groups` | `sessionId`, `column` (name) | `SpellingGroup[]` |
| `clean_state` | `sessionId` | `CleanState` |
| `clean_add_step` | `sessionId`, `step` | `CleanState` |
| `clean_undo`, `clean_redo` | `sessionId` | `CleanState` |
| `clean_move_step` | `sessionId`, `from`, `to` | `CleanState` |
| `clean_set_enabled` | `sessionId`, `index`, `enabled` | `CleanState` |
| `clean_remove_step` | `sessionId`, `index` | `CleanState` |
| `clean_fix_cell` | `sessionId`, `row`, `column`, `value` | `CleanState` |
| `clean_save` | `sessionId` | `SavedFile`, written beside the original |
| `clean_reuse` | `sessionId`, `fromFileId` | `{ report: MatchReport, state: CleanState or null }` |

Events: `backup-progress` with `{ files, bytes }`.

## App types

- `ProjectView`: `{ id, name, recovered: RepairReport, budget: StorageBudget }`
- `FileRow`: `{ id, name, sizeBytes, kind }` where `kind` is `csv` or `other`. Excel files are `other` until XLSX
  support exists; the interface says it cannot open them yet.
- `CleanSession`: `{ sessionId, fileName, rowCount, columns: string[], state: CleanState }`
- `CleanState`: `{ steps: [{ name, enabled, step }], canUndo, canRedo, summary, needsPerson: NeedsPerson[], rowCount, columns, blocked }`
  `blocked` is null, or `{ report, message }` when the saved steps cannot run on this file. The interface then shows
  the message; saving and typing fixes are refused until the steps are changed.
  `needsPerson` holds at most 500 entries; `summary` has the full counts.

## Rules

- The step list of each file is saved in the project database after every change and loaded by `clean_open`.
- A replay that cannot run (missing or repeated columns) returns the match report; nothing is guessed.
- Bench mode (`docs/contracts/bench-mode.md`) keeps working: the commands `get_meta`, `get_window`,
  `set_cell_0_0`, `bench_ready`, `bench_scroll`, `bench_edit`, `bench_error` stay as they are, and the interface
  shows the bench grid only when `get_meta` reports bench mode.
