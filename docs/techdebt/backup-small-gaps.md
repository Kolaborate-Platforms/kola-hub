# Backup: small gaps left on purpose

- Empty folders are not stored. The format has only file records, so a restored project loses folders with no files.
- Restored files are not synced to disk one by one, only moved into place at the end. A power cut during a
  restore can leave a hidden scratch folder, which the next restore into that folder will refuse as non-empty.
- Sample files in "test the last backup" are skipped above 256 MiB. They are still fully hashed in the read.
- Case-only clashes use simple lower-casing. Unicode normalisation differences (for example composed against
  decomposed accents) are not compared.
- Only `.craft_tmp` files are treated as leftover temporary files. Database `-wal` and `-journal` files are the
  caller's job to checkpoint first.
- Files are opened after a link check, not with a no-follow flag, so a file swapped for a link in between is
  followed. Needs `unsafe` or a platform crate to close.
