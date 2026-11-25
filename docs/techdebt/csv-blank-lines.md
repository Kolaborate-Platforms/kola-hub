# CSV blank lines vanish quietly

The `csv` crate skips blank lines. In a one-column file, a truly empty row disappears with no entry in
`CsvReport`. Fix by counting skipped lines (needs our own line pass or a different parser). Low risk
for now: most files have several columns, where an empty row is not a blank line.
