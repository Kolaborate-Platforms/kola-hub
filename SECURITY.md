# Security

## Report a problem privately

Do not open a public issue for a security problem. On the repository page, open the **Security** tab and choose
**Report a vulnerability**. Only the maintainers can read it.

Say what you did, what happened and what you expected. Use invented data. Never attach real files, passphrases or
anything that names a real person.

We aim to reply within seven days. Kola Hub has no released version yet, so fixes go to the main branch.

## What we promise

- The app makes no network calls, except the optional update check in `kh-update`.
- It never overwrites a person's original file.
- Every file it opens is treated as hostile.
- The interface and any connected AI assistant get ids and file names, never file paths.
- AI access is off until the person turns it on. See `docs/contracts/mcp.md`.

A report that shows one of these promises can be broken is a security problem.
