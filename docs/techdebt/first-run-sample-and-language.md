# First-run screen omits sample project and language picker

- **Where:** `ui/src/screens/home.ts` (`renderFirstRunScreen`)
- **Shortcut taken:** When no projects have been opened yet, the first-run screen explains what Kola Hub does and offers one "Open a folder" button. It does not bundle a sample project or include the language picker shown in the onboarding design.
- **Upgrade path:** Add a bundled offline sample project and wire a real language switcher once Luganda and Swahili string catalogues exist.
