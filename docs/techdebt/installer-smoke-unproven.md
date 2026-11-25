# Installer smoke test is unproven

`installer.yml` and `scripts/smoke-windows.ps1` have never run: GitHub Actions is blocked by a billing lock. The
test opens a file but does not save one yet, because the app cannot save. Code signing is not set up.
