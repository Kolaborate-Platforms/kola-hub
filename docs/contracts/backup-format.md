# Backup archive format, version 1

Owner crate: `kh-backup`. One sealed file (`.khbackup`) holds one or more project folders.

## Header (not secret, but authenticated)

All numbers little-endian, in this order.

| Field | Bytes | Value |
|-------|-------|-------|
| Magic | 4 | `KHBK` |
| Version | 1 | 1 |
| Argon2id memory, KiB | 4 | 65,536 by default |
| Argon2id passes | 4 | 3 by default |
| Argon2id lanes | 4 | 1 by default |
| Salt | 16 | random per archive |
| Nonce prefix | 7 | random per archive |
| Plaintext chunk size | 4 | 65,536 |

Reading refuses, before deriving any key: wrong magic, unknown version, memory outside 19,456 to 262,144 KiB,
passes outside 1 to 10, lanes outside 1 to 4, chunk size other than 65,536. A hostile header must not be able to
make a 4 GB laptop run out of memory.

## Body

Key: Argon2id(passphrase, salt), 32 bytes, wiped after use. Cipher: ChaCha20-Poly1305 in the STREAM construction
(32-bit big-endian counter, last-block flag), from RustCrypto. Do not hand-roll nonces. The whole header is the
associated data of every chunk. Each chunk is 65,536 bytes of plaintext plus a 16 byte tag; the last may be
shorter. Truncated, reordered, repeated or extended input must fail.

## Plaintext

A sequence of records.

- File: `0x01`, path length `u16`, path (UTF-8, `/` separated, first part is the project label), size `u64`,
  the bytes, then SHA-256 of the bytes (32).
- End: `0x00`, file count `u64`, total bytes `u64`. Nothing may follow.

The per-file hashes and the end record are the manifest. A missing or wrong one is an error.

## Creating

- Passphrase at least 8 characters. The screen asks for it twice and says it cannot be recovered.
- Never overwrite an existing file. Write with `kh_store::atomic` so a power cut leaves no half archive.
- Follow no symbolic links. Skip them and list them in the report.
- A file that changes size while being read is an error.
- Skip leftover temporary files. The caller closes or checkpoints project databases first.

## Restoring (the archive is hostile input)

- Restore into a folder that is empty or does not exist. Write into a temporary folder inside it and move into
  place only after the end record verifies. On any failure remove what this restore wrote.
- Refuse a path that is empty, absolute, or has a part that is empty, `.`, `..`, longer than 255 bytes, ends in a
  dot or space, contains `\`, `:`, `*`, `?`, `"`, `<`, `>`, `|` or a control character, or is a Windows device name
  (`CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9`, `LPT1` to `LPT9`, with or without an extension, any case).
- Refuse more than 32 parts, two paths that differ only by case, more than 200,000 files, or more than the
  caller's byte limit (default 50 GiB). Never allocate from a declared size.
- Create files with create-new. Never follow a link already present.
- Wrong passphrase and damaged archive give one plain message: "The passphrase is wrong or the backup is damaged."

## Testing a backup

"Test the last backup" reads the whole archive, checks every hash and the end record, writes a few sample files
(spread across the archive) to a scratch folder, compares their hashes and removes the folder.
