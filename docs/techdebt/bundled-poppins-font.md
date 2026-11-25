# Bundle Poppins font locally

- **Where:** `ui/src/theme.css` (`--font-sans: Poppins, "Segoe UI", sans-serif`)
- **Shortcut taken:** No font files were downloaded or bundled in this pass. The UI falls back to `"Segoe UI"` or the system sans-serif font when Poppins is not installed locally.
- **Upgrade path:** Subset Poppins (Latin, weights 400 and 600, WOFF2) into `ui/src/fonts/`, register `@font-face` rules in `ui/src/theme.css`, copy `fonts/` in `ui/copy-static.mjs`, and verify against the installed-size budget in `docs/BUILD_SPEC.md` section 4.
