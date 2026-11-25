# Provenance

Every piece of code or data we did not write ourselves is listed here. See `docs/BUILD_SPEC.md` section 11.3.

Rules in short:

- Permissive licences only (MIT, Apache-2.0, BSD). No GPL or AGPL.
- Code from the maintainer's Agent HQ or hq-suite is copied only after the maintainer signs off, per file or crate. No entry here, no copy.
- Code from storytold/craft-libs is used only after the check below, and never as a git dependency on a moving branch.

## Copied or vendored code

| Date | What | Source repo | Path | Commit | Licence | Sign-off (the maintainer) | Notes |
|------|------|-------------|------|--------|---------|-------------------|-------|
| 2026-11-25 | Atomic file writing | `storytold/craft-libs` | `crates/atomic/src/lib.rs` | `05b99ee5dcd64f2c954a693ef361301214ddc3c7` | MIT OR Apache-2.0 (used under Apache-2.0) | Not needed (not the maintainer's code) | In `crates/kh-store/src/atomic.rs`. One test path changed. Needs `rustix` on macOS |

## Candidate crates from storytold/craft-libs

Checked on 25 November 2026 against `storytold/craft-libs` commit `05b99ee5dcd64f2c954a693ef361301214ddc3c7`
(204 crates; workspace licence `MIT OR Apache-2.0`, with `LICENSE-APACHE`, `LICENSE-MIT` and `NOTICE` at the
repo root, so we may take it under Apache-2.0). Nothing has been copied yet. "Deps" counts unique crates in the
normal dependency tree, including the crate itself. Size cost in bytes is not measured yet; it is measured when
a crate is first taken in.

| Crate | Licence | Lines (src) | Deps | What we would use it for | Verdict |
|-------|---------|-------------|------|--------------------------|---------|
| `craft-atomic` | MIT OR Apache-2.0 | 1,349 (1 file) | 1 (none on Windows and Linux) | Atomic saves: temp file beside target, `sync_all`, rename with Windows retry for antivirus locks, failure injection for kill tests | **Keep.** Vendor the one file into `kh-store` with attribution. Matches spec section 6 exactly. 23 tests pass |
| `craft-settings` | MIT OR Apache-2.0 | 1,117 | 24 (`serde`, `toml_edit`, `dirs`, `craft-atomic`) | Per-user settings file, platform folders, recovery from a bad file, migration | **Keep, probably.** Check `toml_edit` size when first built. 10 tests pass. Fallback: a small own version |
| `craft-csv` | MIT OR Apache-2.0 | 1,350 | 9 | Delimiter sniffing, encoding detection (UTF-8 BOM, Windows-1252), quoting | **Replace for the reader.** Its parser takes the whole file as one `&str` and returns `Vec<Vec<String>>`, so it cannot stream a 200 MB file in 400 MB of memory (spec 5.2). Use the `csv` crate for reading. May borrow `sniff_delimiter` and `decode_text` ideas, or vendor those two functions with attribution |
| `craft-xlsx` | MIT OR Apache-2.0 | 11,413 | 45 (pulls `craft-opc`, `craft-xml`, `craft-drawingml`, `craft-sheet`) | Reading `.xlsx` | **Replace.** Read-only (writer exists only as test support) and drags in drawing and package crates we do not need. Compare `calamine` (read) and `rust_xlsxwriter` (write) on size and zip-bomb limits before deciding |
| `craft-zip` | MIT OR Apache-2.0 | 4,511 | 25 | Zip preflight, entry reads that do not trust declared sizes, duplicate-name detection | **Keep as a design reference, decide later.** Directly useful for spec 5.3 zip-bomb limits. Revisit when XLSX work starts |
| `craft-sheet` | MIT OR Apache-2.0 | 600 | 0 | Cell positions and A1 text | Not needed in Phase 0 to 2 |
| `craft-i18n` | MIT OR Apache-2.0 | 4,176 | 66 (ICU4X data, `postcard` blobs) | Translation lookup with fallback and plurals | **Replace for now.** ICU4X data is a heavy dependency for an app with a 25 MB installer budget. Spec 8 needs only a string catalog with fallback and simple plurals for English, then Luganda and Swahili. Build a small one in `kh-settings`. Revisit if plural rules become hard |
| `craft-update` | MIT OR Apache-2.0 | 23,099 | 68 (`ureq`, `rustls`, `ring`, `egui`, `chrono`) | Update check and self-update | **Replace.** It is built for the Craft launcher and carries a GUI and a TLS stack. Spec 9 needs a signed small feed, an optional check and "install from a file". Write a small `kh-update`. Read it for the Windows install and rollback design |

Other things to know:

- The repo vendors patched third-party code under `vendor/` and keeps notices in `licenses/` and
  `ATTRIBUTION.md`. We copy none of that.
- Depending on a single crate by git means resolving a 204-crate workspace and allowing a git source in
  `deny.toml`. Vendoring a single file (with its licence header and a line above) is simpler and keeps the
  dependency tree ours.
- Several crates carry `MPL-2.0` or `GPL` data only behind optional features (`craft-dwg` and `hypher` Slovak).
  None of the crates above use them.

## Third-party crates we depend on

Each new dependency needs a reason, a licence check and a size check in its PR. `cargo deny check` enforces licences.

| Crate | Version | Licence | Why | Size cost |
|-------|---------|---------|-----|-----------|
| `csv` | 1.x | Unlicense OR MIT | Streaming CSV reader | Not measured yet |
| `tempfile` | 3.x | MIT OR Apache-2.0 | Tests only | Not shipped |
| `proptest` | 1.x | MIT OR Apache-2.0 | Fuzz style tests only | Not shipped |
| `argon2`, `chacha20poly1305`, `aead-stream` | 0.6, 0.11, 0.6 | MIT OR Apache-2.0 | Backup key and STREAM encryption (RustCrypto) | Not measured yet |
| `sha2`, `getrandom`, `zeroize` | 0.11, 0.4, 1.x | MIT OR Apache-2.0 | Backup hashes, random salt, wiping keys | Not measured yet |

## Bundled assets

| Asset | Source | Licence | Notes |
|-------|--------|---------|-------|
| Poppins (to be subset to Latin, two weights) | Google Fonts | OFL-1.1 | Not yet added. OFL permits bundling. Keep the licence text beside the font files |
