//! CSV writer. The caller opens the file (or buffer); this only writes to it.

use std::io::Write;

use kh_core::Table;

/// Options for [`write_csv`].
#[derive(Debug, Clone, Default)]
pub struct CsvWriteOptions {
    /// Zero based columns the person marked as formulas. Their cells are written as they are.
    pub formula_columns: Vec<usize>,
}

/// Why a CSV file could not be written.
#[derive(Debug)]
pub enum CsvWriteError {
    /// The destination refused the bytes.
    Io(std::io::Error),
}

impl std::fmt::Display for CsvWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CsvWriteError::Io(e) => write!(f, "the file could not be saved: {e}"),
        }
    }
}

impl std::error::Error for CsvWriteError {}

impl From<csv::Error> for CsvWriteError {
    fn from(e: csv::Error) -> Self {
        CsvWriteError::Io(std::io::Error::other(e.to_string()))
    }
}

impl From<std::io::Error> for CsvWriteError {
    fn from(e: std::io::Error) -> Self {
        CsvWriteError::Io(e)
    }
}

/// True for a plain negative number such as `-12.5`, which cannot act as a formula.
fn is_negative_number(cell: &str) -> bool {
    let Some(rest) = cell.strip_prefix('-') else {
        return false;
    };
    let mut parts = rest.splitn(2, '.');
    let whole = parts.next().unwrap_or("");
    let frac = parts.next();
    !whole.is_empty()
        && whole.bytes().all(|b| b.is_ascii_digit())
        && frac.is_none_or(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()))
}

/// Makes a cell that starts with `=`, `+`, `-` or `@` safe to open in a spreadsheet, by putting a
/// single quote in front. Leading spaces, tabs and line breaks do not hide the start, and the full width
/// forms of the signs count too. Plain negative numbers are left alone.
pub fn neutralise_cell(cell: &str) -> std::borrow::Cow<'_, str> {
    let start = cell.trim_start_matches(|c: char| c.is_whitespace() || c.is_control());
    let sign = start.starts_with([
        '=', '+', '@', '\u{ff1d}', '\u{ff0b}', '\u{ff0d}', '\u{ff20}',
    ]);
    let minus = start.starts_with('-') && !is_negative_number(start.trim_end());
    if sign || minus || cell.starts_with(['\t', '\r']) {
        std::borrow::Cow::Owned(format!("'{cell}"))
    } else {
        std::borrow::Cow::Borrowed(cell)
    }
}

/// Writes the header and every row as CSV. Cells that could run as formulas are neutralised
/// unless their column is in `options.formula_columns`.
pub fn write_csv<W: Write>(
    table: &Table,
    out: W,
    options: &CsvWriteOptions,
) -> Result<(), CsvWriteError> {
    let mut w = csv::WriterBuilder::new().from_writer(out);
    let formula = |c: usize| options.formula_columns.contains(&c);
    let header = table.column_names().iter().enumerate().map(|(i, n)| {
        if formula(i) {
            n.as_str().into()
        } else {
            neutralise_cell(n)
        }
    });
    w.write_record(header.map(|c| c.into_owned()))?;
    let mut record = Vec::with_capacity(table.column_count());
    for r in 0..table.row_count() {
        record.clear();
        for c in 0..table.column_count() {
            let cell = table.cell(r, c).unwrap_or("");
            record.push(if formula(c) {
                cell.into()
            } else {
                neutralise_cell(cell)
            });
        }
        w.write_record(record.iter().map(|c| c.as_bytes()))?;
    }
    w.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::TableBuilder;

    fn table(rows: &[[&str; 2]]) -> Table {
        let mut b = TableBuilder::new(&["name", "value"]).unwrap();
        for r in rows {
            b.push_row(r).unwrap();
        }
        b.finish()
    }

    fn render(t: &Table, o: &CsvWriteOptions) -> String {
        let mut out = Vec::new();
        write_csv(t, &mut out, o).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn neutralises_formula_starts_but_not_negative_numbers() {
        let t = table(&[["=SUM(A1)", "-5"], ["+1", "-x"], ["@a", "-2.50"]]);
        let text = render(&t, &CsvWriteOptions::default());
        assert_eq!(text, "name,value\n'=SUM(A1),-5\n'+1,'-x\n'@a,-2.50\n");
    }

    #[test]
    fn formula_columns_are_left_alone() {
        let t = table(&[["=A1", "=B1"]]);
        let o = CsvWriteOptions {
            formula_columns: vec![1],
        };
        assert_eq!(render(&t, &o), "name,value\n'=A1,=B1\n");
    }

    #[test]
    fn quotes_commas_and_new_lines_round_trip() {
        let t = table(&[["a, \"b\"", "x\ny"]]);
        let text = render(&t, &CsvWriteOptions::default());
        let (back, _) =
            crate::read_csv(text.as_bytes(), b',', &crate::CsvLimits::default()).unwrap();
        assert_eq!(back.cell(0, 0), Some("a, \"b\""));
        assert_eq!(back.cell(0, 1), Some("x\ny"));
    }

    #[test]
    fn leading_space_or_tab_does_not_hide_a_formula() {
        for bad in ["\t=1+1", " =1+1", "\r\n@x", "\u{ff1d}1", "  -x", "\ttext"] {
            assert!(neutralise_cell(bad).starts_with('\''), "{bad:?}");
        }
        for fine in [" -5", "  text", "a=b", ""] {
            assert_eq!(neutralise_cell(fine), fine);
        }
    }
}
