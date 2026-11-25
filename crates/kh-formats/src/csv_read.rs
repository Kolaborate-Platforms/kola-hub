//! Streaming CSV reader. The file is read in small pieces and never held as one string.

use std::cell::Cell;
use std::io::Read;
use std::rc::Rc;

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
    /// Most bytes one record (one row, even if quoted cells run over several lines) may take in the file.
    pub max_record_bytes: usize,
    /// Most bytes the table's cells may take in memory.
    pub max_total_bytes: usize,
}

impl Default for CsvLimits {
    fn default() -> Self {
        Self {
            max_rows: None,
            max_cell_bytes: 1 << 20,
            max_columns: 4_096,
            max_record_bytes: 8 << 20,
            max_total_bytes: 1 << 30,
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
    /// One row is larger than the limit, for example a quote that is never closed.
    RecordTooLong {
        /// Row number in the file.
        row: usize,
    },
    /// The cell text in the file is more than the limit for one table.
    TooMuchData {
        /// The limit, in bytes.
        limit: usize,
    },
    /// The file is saved as UTF-16, which Kola Hub cannot open as a CSV.
    NotUtf8,
    /// The file is not shaped like a CSV.
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
            CsvError::RecordTooLong { row } => write!(
                f,
                "row {row} is far too long to open. A quote may be missing its partner, or this is not a CSV file"
            ),
            CsvError::TooMuchData { limit } => write!(
                f,
                "this file holds more than {} MB of data, which is more than Kola Hub will open at once",
                limit >> 20
            ),
            CsvError::NotUtf8 => write!(
                f,
                "this file is saved as UTF-16. Save it again as \"CSV UTF-8\" and open that copy"
            ),
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

const SNIFF_SAMPLE: usize = 64 * 1024;

/// Picks comma, semicolon or tab from the first few thousand bytes: the one that gives the most
/// consistent, greater-than-one number of fields per line. Quoted text is ignored and a last line that
/// was cut short is dropped. When semicolon is consistent and every comma sits between digits
/// (`1,5;2,5`), the commas are decimal commas and semicolon wins. Falls back to comma.
pub fn sniff_delimiter(sample: &[u8]) -> u8 {
    let sample = &sample[..sample.len().min(SNIFF_SAMPLE)];
    let mut lines = split_lines(sample);
    if !sample.ends_with(b"\n") && lines.len() > 1 {
        lines.pop();
    }
    let mut best = (b',', 0usize);
    for &cand in b",;\t" {
        let counts: Vec<usize> = lines
            .iter()
            .take(20)
            .map(|l| count_outside_quotes(l, cand))
            .collect();
        let Some(&first) = counts.first() else {
            continue;
        };
        if first == 0 || !counts.iter().all(|&c| c == first) {
            continue;
        }
        let decimal = cand == b';' && best.0 == b',' && commas_are_decimal(&lines);
        if first > best.1 || decimal {
            best = (cand, first);
        }
    }
    best.0
}

/// Lines that are not blank, with quoted newlines kept inside their line.
fn split_lines(sample: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let (mut start, mut quoted) = (0, false);
    for (i, &b) in sample.iter().enumerate() {
        match b {
            b'"' => quoted = !quoted,
            b'\n' if !quoted => {
                lines.push(&sample[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < sample.len() {
        lines.push(&sample[start..]);
    }
    lines.retain(|l| !l.iter().all(u8::is_ascii_whitespace));
    lines
}

fn count_outside_quotes(line: &[u8], cand: u8) -> usize {
    let mut quoted = false;
    let mut n = 0;
    for &b in line {
        if b == b'"' {
            quoted = !quoted;
        } else if b == cand && !quoted {
            n += 1;
        }
    }
    n
}

fn commas_are_decimal(lines: &[&[u8]]) -> bool {
    let mut seen = false;
    for line in lines {
        for (i, &b) in line.iter().enumerate() {
            if b == b',' {
                let before = i.checked_sub(1).and_then(|j| line.get(j));
                let after = line.get(i + 1);
                if !before.is_some_and(u8::is_ascii_digit) || !after.is_some_and(u8::is_ascii_digit)
                {
                    return false;
                }
                seen = true;
            }
        }
    }
    seen
}

/// Passes bytes through, but fails once more than the allowed bytes are read past the end of the last
/// record. The csv crate would otherwise buffer a whole runaway record before any limit applies.
struct RecordCap<R> {
    inner: R,
    served: u64,
    until: Rc<Cell<u64>>,
    tripped: Rc<Cell<bool>>,
    utf16: Rc<Cell<bool>>,
}

impl<R: Read> Read for RecordCap<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let room = self.until.get().saturating_sub(self.served);
        if room == 0 && !buf.is_empty() {
            self.tripped.set(true);
            return Err(std::io::Error::other("record too long"));
        }
        let take = buf.len().min(usize::try_from(room).unwrap_or(usize::MAX));
        let n = self.inner.read(&mut buf[..take])?;
        // Only the very start of the file can hold a byte order mark.
        if self.served == 0 && matches!(buf[..n], [0xFF, 0xFE, ..] | [0xFE, 0xFF, ..]) {
            self.utf16.set(true);
        }
        self.served += n as u64;
        Ok(n)
    }
}

const READ_BUFFER: usize = 256 * 1024;

/// Reads a CSV from `source`, one record at a time, into a [`Table`].
///
/// The first record is the header. Short rows are padded with empty cells and counted in the report.
/// Rows with more cells than the header fail the read, so no data is dropped quietly.
pub fn read_csv<R: Read>(
    source: R,
    delimiter: u8,
    limits: &CsvLimits,
) -> Result<(Table, CsvReport), CsvError> {
    let until = Rc::new(Cell::new(0));
    let guard = Guard {
        until: Rc::clone(&until),
        tripped: Rc::new(Cell::new(false)),
        utf16: Rc::new(Cell::new(false)),
        max_record: limits.max_record_bytes as u64,
    };
    guard.until.set(guard.allowance());
    let source = RecordCap {
        inner: source,
        served: 0,
        until,
        tripped: Rc::clone(&guard.tripped),
        utf16: Rc::clone(&guard.utf16),
    };
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .buffer_capacity(READ_BUFFER)
        .from_reader(source);

    let mut record = csv::ByteRecord::new();
    let mut report = CsvReport::default();

    if !read_record(&mut reader, &mut record, &guard, 1)? {
        return Err(CsvError::Empty);
    }
    if record.len() > limits.max_columns {
        return Err(CsvError::Table(TableError::TooManyColumns(record.len())));
    }
    if record.iter().any(|f| f.len() > limits.max_cell_bytes) {
        return Err(CsvError::CellTooLong { row: 1 });
    }
    let header: Vec<String> = record.iter().map(|f| lossy(f, &mut report)).collect();
    let width = header.len();
    let mut builder = TableBuilder::new(&header)?;

    let mut cells: Vec<String> = vec![String::new(); width];
    let mut row_no = 1usize;
    while read_record(&mut reader, &mut record, &guard, row_no + 1)? {
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
        if builder.used_bytes() > limits.max_total_bytes {
            return Err(CsvError::TooMuchData {
                limit: limits.max_total_bytes,
            });
        }
    }
    Ok((builder.finish(), report))
}

struct Guard {
    until: Rc<Cell<u64>>,
    tripped: Rc<Cell<bool>>,
    utf16: Rc<Cell<bool>>,
    max_record: u64,
}

impl Guard {
    /// The reader fills its buffer ahead of the record it is splitting.
    fn allowance(&self) -> u64 {
        self.max_record.saturating_add(READ_BUFFER as u64)
    }
}

fn read_record<R: Read>(
    reader: &mut csv::Reader<R>,
    record: &mut csv::ByteRecord,
    guard: &Guard,
    row: usize,
) -> Result<bool, CsvError> {
    let start = reader.position().byte();
    guard.until.set(start.saturating_add(guard.allowance()));
    let result = reader.read_byte_record(record);
    if guard.utf16.get() {
        return Err(CsvError::NotUtf8);
    }
    let found = result.map_err(|e| match e.into_kind() {
        csv::ErrorKind::Io(_) if guard.tripped.get() => CsvError::RecordTooLong { row },
        csv::ErrorKind::Io(io) => CsvError::Io(io),
        _ => CsvError::Malformed("the rows could not be split into cells".to_owned()),
    })?;
    // The read ahead lets a record run a little past the cap before the wrapper stops it.
    if reader.position().byte() - start > guard.max_record {
        return Err(CsvError::RecordTooLong { row });
    }
    Ok(found)
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

    fn tight() -> CsvLimits {
        CsvLimits {
            max_record_bytes: 1000,
            ..CsvLimits::default()
        }
    }

    #[test]
    fn unclosed_quote_stops_at_the_record_cap() {
        let mut data = b"a,b\n\"never closed".to_vec();
        data.extend(std::iter::repeat_n(b'x', 5_000_000));
        let err = read_csv(&data[..], b',', &tight()).unwrap_err();
        assert!(matches!(err, CsvError::RecordTooLong { row: 2 }));
    }

    #[test]
    fn a_million_commas_stop_at_the_record_cap() {
        let mut data = vec![b','; 1_000_000];
        data.push(b'\n');
        let err = read_csv(&data[..], b',', &tight()).unwrap_err();
        assert!(matches!(err, CsvError::RecordTooLong { row: 1 }));
    }

    #[test]
    fn a_big_valid_row_just_under_the_cap_opens() {
        let cell = "y".repeat(900);
        let (t, _) = read_csv(format!("a\n{cell}\n").as_bytes(), b',', &tight()).unwrap();
        assert_eq!(t.cell(0, 0), Some(cell.as_str()));
    }

    #[test]
    fn total_text_limit_is_enforced() {
        let limits = CsvLimits {
            max_total_bytes: 100,
            ..CsvLimits::default()
        };
        let data = format!("a\n{}", "0123456789\n".repeat(50));
        let err = read_csv(data.as_bytes(), b',', &limits).unwrap_err();
        assert!(matches!(err, CsvError::TooMuchData { limit: 100 }));
        assert!(err.to_string().contains("MB"));
    }

    #[test]
    fn utf16_files_are_refused_in_plain_words() {
        let err = read_csv(&b"\xff\xfea\0,\0b\0\n\0"[..], b',', &CsvLimits::default()).unwrap_err();
        assert!(matches!(err, CsvError::NotUtf8));
    }

    #[test]
    fn sniffing_ignores_quoted_delimiters_and_cut_lines() {
        assert_eq!(sniff_delimiter(b"a;b\n\"x,y,z\";2\n"), b';');
        assert_eq!(sniff_delimiter(b"a,b\n1,2\n3,"), b',');
        assert_eq!(sniff_delimiter(b"a;b;c\n1;2;3\n4;5"), b';');
    }

    #[test]
    fn sniffing_prefers_semicolon_for_decimal_commas() {
        assert_eq!(sniff_delimiter(b"1,5;2,5\n3,5;4,5\n"), b';');
        assert_eq!(sniff_delimiter(b"a,b;c\n1,2;3\n"), b',');
    }

    #[test]
    fn sniffing_handles_a_huge_sample() {
        let big = vec![b'a'; 10_000_000];
        assert_eq!(sniff_delimiter(&big), b',');
    }

    #[test]
    fn empty_cells_count_against_the_total_limit() {
        let limits = CsvLimits {
            max_total_bytes: 1000,
            ..CsvLimits::default()
        };
        let data = format!("a,b,c,d,e,f,g,h\n{}", ",,,,,,,\n".repeat(100));
        let err = read_csv(data.as_bytes(), b',', &limits).unwrap_err();
        assert!(matches!(err, CsvError::TooMuchData { limit: 1000 }));
    }

    #[test]
    fn a_long_header_cell_is_refused() {
        let limits = CsvLimits {
            max_cell_bytes: 8,
            ..CsvLimits::default()
        };
        let err = read_csv(&b"a_very_long_name\n1\n"[..], b',', &limits).unwrap_err();
        assert!(matches!(err, CsvError::CellTooLong { row: 1 }));
    }

    #[test]
    fn bom_bytes_after_a_short_first_read_are_not_utf16() {
        struct Drip(Vec<Vec<u8>>);
        impl Read for Drip {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.0.is_empty() {
                    return Ok(0);
                }
                let chunk = self.0.remove(0);
                buf[..chunk.len()].copy_from_slice(&chunk);
                Ok(chunk.len())
            }
        }
        let source = Drip(vec![b"a".to_vec(), b"\n".to_vec(), vec![0xFF, 0xFE, b'\n']]);
        let (_, report) = read_csv(source, b',', &CsvLimits::default()).unwrap();
        assert!(report.replaced_bad_bytes);
    }

    #[test]
    fn dictionary_entries_count_against_the_total_limit() {
        // 100 different 20 byte values: 2,400 bytes as text and codes, far more once held twice.
        let limits = CsvLimits {
            max_total_bytes: 4000,
            ..CsvLimits::default()
        };
        let rows: String = (0..100).map(|i| format!("{i:020}\n")).collect();
        let err = read_csv(format!("a\n{rows}").as_bytes(), b',', &limits).unwrap_err();
        assert!(matches!(err, CsvError::TooMuchData { limit: 4000 }));
    }

    #[test]
    fn a_row_just_over_the_cap_is_refused() {
        let cell = "y".repeat(1100);
        let err = read_csv(format!("a\n{cell}\n").as_bytes(), b',', &tight()).unwrap_err();
        assert!(matches!(err, CsvError::RecordTooLong { row: 2 }));
    }
}
