//! Streaming CSV reader. The file is read in small pieces and never held as one string.

use std::io::Read;

use kh_core::{Table, TableBuilder, TableError};

/// Limits that keep a hostile or broken file from using all the memory.
#[derive(Debug, Clone)]
pub struct CsvLimits {
    /// Most data rows to read. `None` reads to the end. Use a small number to show a first screen quickly.
    pub max_rows: Option<usize>,
    /// Longest single cell, in bytes. Longer cells fail the read.
    pub max_cell_bytes: usize,
    /// Most columns.
    pub max_columns: usize,
}

impl Default for CsvLimits {
    fn default() -> Self {
        Self {
            max_rows: None,
            max_cell_bytes: 1 << 20,
            max_columns: 4_096,
        }
    }
}

/// What the reader noticed while it read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CsvReport {
    /// Rows that were shorter than the header and were padded with empty cells.
    pub short_rows: usize,
    /// True when reading stopped at `max_rows` and the file has more.
    pub stopped_early: bool,
    /// True when some bytes were not valid UTF-8 and were replaced.
    pub replaced_bad_bytes: bool,
}

/// Why a CSV file could not be read.
#[derive(Debug)]
pub enum CsvError {
    /// The file could not be read.
    Io(std::io::Error),
    /// The file has no header row.
    Empty,
    /// A row has more cells than the header. Row numbers count the header as row 1.
    TooManyCells {
        /// Row number in the file.
        row: usize,
        /// Cells in the header.
        expected: usize,
        /// Cells in this row.
        found: usize,
    },
    /// A cell is longer than the limit.
    CellTooLong {
        /// Row number in the file.
        row: usize,
    },
    /// The file is not shaped like a CSV (for example an unclosed quote swallowed the rest).
    Malformed(String),
    /// The table engine refused the data.
    Table(TableError),
}

impl std::fmt::Display for CsvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CsvError::Io(e) => write!(f, "could not read the file: {e}"),
            CsvError::Empty => write!(f, "the file is empty"),
            CsvError::TooManyCells {
                row,
                expected,
                found,
            } => {
                write!(
                    f,
                    "row {row} has {found} cells but the header has {expected}"
                )
            }
            CsvError::CellTooLong { row } => write!(f, "a cell in row {row} is too long to open"),
            CsvError::Malformed(why) => write!(f, "the file does not look like a table: {why}"),
            CsvError::Table(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CsvError {}

impl From<std::io::Error> for CsvError {
    fn from(e: std::io::Error) -> Self {
        CsvError::Io(e)
    }
}

impl From<TableError> for CsvError {
    fn from(e: TableError) -> Self {
        CsvError::Table(e)
    }
}

/// Picks comma, semicolon or tab from the first few thousand bytes: the one that gives the most
/// consistent, greater-than-one number of fields per line. Falls back to comma.
pub fn sniff_delimiter(sample: &[u8]) -> u8 {
    let text = String::from_utf8_lossy(sample);
    let mut best = (b',', 0usize);
    for &cand in b",;\t" {
        let counts: Vec<usize> = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .take(20)
            .map(|l| l.bytes().filter(|&b| b == cand).count())
            .collect();
        let Some(&first) = counts.first() else {
            continue;
        };
        if first > 0 && counts.iter().all(|&c| c == first) && first > best.1 {
            best = (cand, first);
        }
    }
    best.0
}

/// Reads a CSV from `source`, one record at a time, into a [`Table`].
///
/// The first record is the header. Short rows are padded with empty cells and counted in the report.
/// Rows with more cells than the header fail the read, so no data is dropped quietly.
pub fn read_csv<R: Read>(
    source: R,
    delimiter: u8,
    limits: &CsvLimits,
) -> Result<(Table, CsvReport), CsvError> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .buffer_capacity(256 * 1024)
        .from_reader(source);

    let mut record = csv::ByteRecord::new();
    let mut report = CsvReport::default();

    if !read_record(&mut reader, &mut record)? {
        return Err(CsvError::Empty);
    }
    if record.len() > limits.max_columns {
        return Err(CsvError::Table(TableError::TooManyColumns(record.len())));
    }
    let header: Vec<String> = record.iter().map(|f| lossy(f, &mut report)).collect();
    let width = header.len();
    let mut builder = TableBuilder::new(&header)?;

    let mut cells: Vec<String> = vec![String::new(); width];
    let mut row_no = 1usize;
    while read_record(&mut reader, &mut record)? {
        row_no += 1;
        if limits
            .max_rows
            .is_some_and(|max| builder.row_count() >= max)
        {
            report.stopped_early = true;
            break;
        }
        if record.len() > width {
            return Err(CsvError::TooManyCells {
                row: row_no,
                expected: width,
                found: record.len(),
            });
        }
        if record.len() < width {
            report.short_rows += 1;
        }
        for (i, slot) in cells.iter_mut().enumerate() {
            slot.clear();
            if let Some(field) = record.get(i) {
                if field.len() > limits.max_cell_bytes {
                    return Err(CsvError::CellTooLong { row: row_no });
                }
                match std::str::from_utf8(field) {
                    Ok(s) => slot.push_str(s),
                    Err(_) => {
                        report.replaced_bad_bytes = true;
                        slot.push_str(&String::from_utf8_lossy(field));
                    }
                }
            }
        }
        builder.push_row(&cells)?;
    }
    Ok((builder.finish(), report))
}

fn read_record<R: Read>(
    reader: &mut csv::Reader<R>,
    record: &mut csv::ByteRecord,
) -> Result<bool, CsvError> {
    reader
        .read_byte_record(record)
        .map_err(|e| match e.into_kind() {
            csv::ErrorKind::Io(io) => CsvError::Io(io),
            other => CsvError::Malformed(format!("{other:?}")),
        })
}

fn lossy(field: &[u8], report: &mut CsvReport) -> String {
    match std::str::from_utf8(field) {
        Ok(s) => s.to_owned(),
        Err(_) => {
            report.replaced_bad_bytes = true;
            String::from_utf8_lossy(field).into_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Result<(Table, CsvReport), CsvError> {
        read_csv(text.as_bytes(), b',', &CsvLimits::default())
    }

    #[test]
    fn reads_header_and_rows() {
        let (t, r) = read("market,price\nKisenyi,\"1,200\"\nOwino,900\n").unwrap();
        assert_eq!(t.column_names(), ["market", "price"]);
        assert_eq!(t.row_count(), 2);
        assert_eq!(t.cell(0, 1), Some("1,200"));
        assert_eq!(r, CsvReport::default());
    }

    #[test]
    fn strips_a_byte_order_mark() {
        let (t, _) = read("\u{feff}a,b\n1,2\n").unwrap();
        assert_eq!(t.column_names(), ["a", "b"]);
    }

    #[test]
    fn pads_short_rows_and_counts_them() {
        let (t, r) = read("a,b,c\n1,2\n3,4,5\n").unwrap();
        assert_eq!(t.cell(0, 2), Some(""));
        assert_eq!(r.short_rows, 1);
    }

    #[test]
    fn refuses_long_rows_instead_of_dropping_cells() {
        let err = read("a,b\n1,2,3\n").unwrap_err();
        assert!(matches!(
            err,
            CsvError::TooManyCells {
                row: 2,
                expected: 2,
                found: 3
            }
        ));
    }

    #[test]
    fn empty_file_is_an_error() {
        assert!(matches!(read(""), Err(CsvError::Empty)));
    }

    #[test]
    fn quoted_newlines_stay_in_one_cell() {
        let (t, _) = read("a,b\n\"line one\nline two\",x\n").unwrap();
        assert_eq!(t.row_count(), 1);
        assert_eq!(t.cell(0, 0), Some("line one\nline two"));
    }

    #[test]
    fn max_rows_stops_early() {
        let limits = CsvLimits {
            max_rows: Some(2),
            ..CsvLimits::default()
        };
        let (t, r) = read_csv("a\n1\n2\n3\n4\n".as_bytes(), b',', &limits).unwrap();
        assert_eq!(t.row_count(), 2);
        assert!(r.stopped_early);
    }

    #[test]
    fn long_cells_fail() {
        let limits = CsvLimits {
            max_cell_bytes: 4,
            ..CsvLimits::default()
        };
        let err = read_csv("a\nhello world\n".as_bytes(), b',', &limits).unwrap_err();
        assert!(matches!(err, CsvError::CellTooLong { row: 2 }));
    }

    #[test]
    fn bad_utf8_is_replaced_and_reported() {
        let (t, r) = read_csv(&b"a\n\xff\xfe\n"[..], b',', &CsvLimits::default()).unwrap();
        assert!(r.replaced_bad_bytes);
        assert_eq!(t.cell(0, 0), Some("\u{fffd}\u{fffd}"));
    }

    #[test]
    fn sniffs_delimiters() {
        assert_eq!(sniff_delimiter(b"a;b;c\n1;2;3\n"), b';');
        assert_eq!(sniff_delimiter(b"a\tb\n1\t2\n"), b'\t');
        assert_eq!(sniff_delimiter(b"a,b\n1,2\n"), b',');
        assert_eq!(sniff_delimiter(b"just words\n"), b',');
    }

    #[test]
    fn garbage_does_not_panic() {
        let junk: Vec<u8> = (0..4096u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 24) as u8)
            .collect();
        let _ = read_csv(&junk[..], b',', &CsvLimits::default());
    }
}
