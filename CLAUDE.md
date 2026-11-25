# CLAUDE.md

Instructions for AI coding agents working in this repository. Humans: see `README.md` and `docs/BUILD_SPEC.md`.

## Start here

Read `docs/BUILD_SPEC.md` first. It is the single source of truth: what we build, what we do not, budgets,
architecture, phases and the first tasks (section 14). If this file and the spec disagree, the spec wins.

## What this is

Kola Hub: a suite of small, offline-first desktop tools (Clean, then Annotate, later Sheets, Docs, Slides,
Dashboards) in Rust, open source under Apache-2.0. Target users have old, low-end Windows laptops, little disk
and little or no internet. Size, start-up time, memory and offline behaviour matter more than feature count.

## Hard rules

- **No network, ever, except the optional update check in `kh-update`.** No telemetry, analytics or crash upload.
  No accounts, sign-in, Firebase, Convex, Praxis or link to the Kolaborate platform. Only `kh-update` may depend
  on an HTTP client; `cargo deny` enforces it.
- **Never overwrite a user's original file.** Write a new file or version beside it. All saves are atomic
  (temp file, flush, rename).
- **The interface never sees file paths.** It gets opaque ids. Paths stay in Rust.
- **Budgets are tests.** Installer size, installed size, start-up, memory and large-file timings are in spec
  section 4. A PR that breaks them fails CI. Report measured numbers in any PR that could change them.
- **Treat every opened file as hostile.** Size limits, zip-bomb limits, no macros, fuzz the parsers.
- **Licence hygiene.** Permissive licences only (MIT, Apache-2.0, BSD). No GPL or AGPL. The only exceptions are the
  MPL-2.0 crates named in `deny.toml`, which Tauri requires. Code from the maintainer's
  Agent HQ or hq-suite may be copied only with an entry in `docs/PROVENANCE.md` showing his sign-off. Code from
  storytold/craft-libs only after the check described in spec section 11.3.
- **Do not add scope.** Two tools until the pilot gate passes. If a task seems to need an account, a server,
  sync, sharing or AI, stop and ask the maintainer.
- No secrets, real names, personal paths or real hostnames in code, tests or fixtures. Invent test data.

## Disk space (important)

the maintainer builds on a laptop. Rust debug builds grow to tens of gigabytes. Check free disk before big builds and
after them (`df -h`), run `scripts/cargo-gc.sh` regularly, and say so in your report if free space drops under
10 GB.

## Commands (once the workspace exists)

```bash
cargo check
cargo nextest run --workspace        # fast local loop; use -p <crate> for one crate
cargo test --workspace --doc         # doctests, which nextest does not run
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo deny check
./scripts/cargo-gc.sh                # clean build artifacts
```

## Working agreements

- Branch, small focused pull requests, tests for new behaviour. One PR per task in spec section 14.
- At the end of each phase, stop and wait for the maintainer to review and say go. Do not start the next phase.
- Shortcuts you knowingly leave: one file in `docs/techdebt/<slug>.md`.
- User-visible changes: one file in `changelog.d/<slug>.<kind>.md`.
- No Rust file over 1,500 production lines.
- Docs and UI strings: plain language, no stock AI phrases, avoid em dashes.
- the maintainer writes for people who are not technical. If a screen or string needs jargon, rewrite it.

## Where to look

| Need | Look at |
|------|---------|
| Decisions, budgets, phases | `docs/BUILD_SPEC.md` |
| Measured numbers | `docs/benchmarks/` |
| What we copied and from where | `docs/PROVENANCE.md` |
| Reference repos | spec section 12 (storytold craft-libs) |
