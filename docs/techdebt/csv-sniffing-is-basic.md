# Delimiter sniffing and encoding are basic

`sniff_delimiter` looks at up to 20 lines of the first 64 KiB for a consistent count of comma, semicolon or tab,
ignoring quoted text. A stray quote inside an unquoted cell can still confuse it. UTF-16 files are refused with a
message. Other text that is not UTF-8 is replaced with the replacement character and reported, not converted from
Windows-1252.

`craft-csv` (storytold/craft-libs) has a more careful sniffer and a Windows-1252 decoder. See `docs/PROVENANCE.md`.
Revisit when Clean meets real files from the pilot.
