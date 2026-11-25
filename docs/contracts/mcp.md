# The AI server (`kola-hub-mcp`)

A small console program that lets an AI tool the person already uses work with Kola Hub's tools, through the Model
Context Protocol (MCP). Kola Hub itself contains no AI model.

## Rules

- **Standard input and output only.** Newline-delimited JSON-RPC 2.0. No HTTP, no sockets, no network crates.
- **Off by default.** Every tool call reads the settings again and refuses unless `aiAccess` is true.
- **Only projects already opened in the app.** No tool takes a path or opens a new folder.
- **No paths in any result**, only opaque ids and names.
- Not offered: emptying Recently deleted, backup, restore and test (the passphrase would end up in the AI
  conversation), forgetting projects, changing settings.
- Everything goes through `kh-service`, with its limits. Originals are never changed.
- What a tool returns from a file is data from that file. Tool descriptions say so, because a file can contain
  text that tries to instruct the assistant.
- One request at a time. A request line over 4 MiB is refused. A broken line gets a JSON-RPC error and the server
  keeps running. Nothing but protocol messages goes to standard output.

## Protocol

`initialize` (answer with the client's protocol version when it is one we know, otherwise our newest),
`notifications/initialized` (no answer), `ping`, `tools/list`, `tools/call`. Unknown methods get `-32601`. A tool
that fails returns a normal result with `isError: true` and the plain message and code from `kh-service`.

## Data folder

`kh_service::data_dir::default_data_dir()`, or `KH_DATA_DIR`.

## Tools

| Tool | Arguments | Changes anything |
|------|-----------|------------------|
| `list_projects` | | no |
| `list_files` | `projectId` | no |
| `read_rows` | `projectId`, `fileId`, `start`, `count` (at most 100) | no |
| `profile_file` | `projectId`, `fileId` | no |
| `suggest_spelling_groups` | `projectId`, `fileId`, `column` | no |
| `preview_step` | `projectId`, `fileId`, `step`, `start`, `count` | no |
| `list_steps` | `projectId`, `fileId` | no |
| `add_step` | `projectId`, `fileId`, `step` | saved step list |
| `remove_step` | `projectId`, `fileId`, `index` | saved step list |
| `reuse_steps` | `projectId`, `fileId`, `fromFileId` | saved step list |
| `save_cleaned_copy` | `projectId`, `fileId` | writes a new file beside the original |
| `list_trash` | `projectId` | no |
| `move_to_trash` | `projectId`, `fileId` | moves a file to Recently deleted |
| `restore_from_trash` | `projectId`, `trashId` | moves it back |

Each tool declares a JSON schema, including every `step` shape, and the `readOnlyHint` and `destructiveHint`
annotations.

## Two programs, one project

The window and the server can have the same project open. If the saved step list changed since a session loaded
it, saving the list fails with `steps-changed-elsewhere` and nothing is overwritten.
