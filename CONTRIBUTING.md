# Contributing to Kola Hub

Kola Hub is open source under the Apache License 2.0. The source is public. To keep the product small and
focused, changes go through the maintainers.

## How to contribute

1. **Open an issue first.** Say what problem you want to solve and for whom. Wait for a maintainer to reply
   before you write code. This saves everyone time.
2. **Keep pull requests small.** One task, one pull request.
3. **Add tests** for any new behaviour.
4. **Run the checks** before you push:

   ```bash
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo nextest run --workspace
   cargo test --workspace --doc
   cargo deny check
   bash scripts/test-privacy-check.sh
   ```

5. **Add a changelog note** for anything a user can see: one file, `changelog.d/<slug>.<kind>.md`.

## Keep personal information out

This repository is public and its history is permanent. Set this up before your first commit.

1. Install [gitleaks](https://github.com/gitleaks/gitleaks) (version 8.30 or newer).
2. Turn on the git hooks. `pnpm install` does it for you, or run `git config core.hooksPath .githooks`.
3. Commit with your GitHub no-reply address: `git config user.email "ID+NAME@users.noreply.github.com"`. GitHub
   shows yours under Settings, Emails.
4. Optional: list private words that must never be committed (your own address, client names, host names), one
   pattern per line, in `.git/info/private-denylist`. That file stays on your computer.

The hooks check each change, each commit message and everything you push. The same check runs in CI. If it stops
you, fix the cause. Do not use `--no-verify`. If a real secret or personal detail was already pushed, change the
secret first, then tell a maintainer privately (see [`SECURITY.md`](SECURITY.md)).

The check cannot recognise everything, for example a person's name in a test file. Read your own changes too.

## Rules we hold firmly

- The app makes no network calls, except the optional update check in `kh-update`. No telemetry, no accounts.
- We never overwrite a user's original file.
- Every file a user opens is treated as hostile input.
- Licences: MIT, Apache-2.0 and BSD code only. No GPL or AGPL.
- If you copy code from somewhere, add a line to `docs/PROVENANCE.md`.
- No secrets, real names, personal paths or real hostnames in code or tests. Use invented data and `example.com`.
- No Rust file over 1,500 lines.

## Writing style

Plain language. Many of our users are not technical, so screens and strings avoid jargon. No stock AI phrases,
and go easy on em dashes.

## Scope

We are building two tools first: Clean and Annotate. Requests for accounts, sync, sharing, servers or AI features
will be declined for now.

By contributing you agree that your work is licensed under Apache-2.0.
