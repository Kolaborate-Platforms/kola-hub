# CSV reader keeps the whole table in memory

`kh-formats::read_csv` builds the full table in memory. That fits the budget for the 182 MB test file (169 MB peak),
but a file much larger than 400 MB of table data would not open.

Plan if it matters: keep a row offset index and parse windows from disk on demand, or spill columns to a temporary
file. Do this only if the pilot shows people with files that large.
