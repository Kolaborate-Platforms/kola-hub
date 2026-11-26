# Kola Hub

Small desktop tools that work fully offline, start fast and run on old, low-end Windows laptops.

The first two tools are **Clean** (fix a messy table) and **Annotate** (label text, then images). Everything runs
on the computer. There are no accounts and no tracking.

Status: early development. Test builds are on the
[releases page](https://github.com/kolaborateplatform/kola-hub/releases). Clean works for CSV files and has been
used by hand on macOS only. Windows, the main target, has had automated checks only. Annotate and Excel files are
not built.

To run it from source you need Rust (see `rust-toolchain.toml`), Node and pnpm:

```bash
pnpm install
pnpm tauri dev
```

- Build spec: [`docs/BUILD_SPEC.md`](docs/BUILD_SPEC.md)
- Using an AI assistant (off by default): [`docs/mcp.md`](docs/mcp.md)
- Security: [`SECURITY.md`](SECURITY.md)
- Contributing: [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Licence: Apache-2.0 ([`LICENSE`](LICENSE))
