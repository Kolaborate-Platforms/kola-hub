//! Tables and windowed access.

use crate::MAX_COLUMNS;
use crate::column::{Column, ColumnBuilder, ColumnFull};

/// Why a table could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableError {
    /// More columns than [`MAX_COLUMNS`].
    TooManyColumns(usize),
    /// A row had a different number of cells from the header.
    WrongCellCount {
        /// Cells the header promised.
        expected: usize,
        /// Cells the row had.
        found: usize,
    },
    /// One column's text is larger than the table can hold.
    ColumnFull {
        /// Zero based column index.
        column: usize,
    },
}

impl std::fmt::Display for TableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TableError::TooManyColumns(n) => write!(
                f,
                "this file has {n} columns, which is more than Kola Hub can open"
            ),
            TableError::WrongCellCount { expected, found } => {
                write!(f, "a row has {found} cells but the header has {expected}")
            }
            TableError::ColumnFull { column } => write!(
                f,
                "column {} holds more text than Kola Hub can open",
                column + 1
            ),
        }
    }
}

impl std::error::Error for TableError {}

/// A table of text cells with named columns.
#[derive(Debug, Clone)]
pub struct Table {
    names: Vec<String>,
    columns: Vec<Column>,
    rows: usize,
}

/// A small slice of rows, for the interface to draw. Cells are copied, so a window is cheap to hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// Index of the first row in the table.
    pub start: usize,
    /// The rows, each with one string per column.
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// Column names, in order.
    pub fn column_names(&self) -> &[String] {
        &self.names
    }

    /// Number of columns.
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Number of data rows (the header is not a row).
    pub fn row_count(&self) -> usize {
        self.rows
    }

    /// One column, by index.
    pub fn column(&self, index: usize) -> Option<&Column> {
        self.columns.get(index)
    }

    /// The text of one cell.
    pub fn cell(&self, row: usize, column: usize) -> Option<&str> {
        self.columns.get(column)?.get(row)
    }

    /// Up to `count` rows starting at `start`. A window past the end is shorter, or empty.
    pub fn window(&self, start: usize, count: usize) -> Window {
        let end = start.saturating_add(count).min(self.rows);
        let start = start.min(end);
        let rows = (start..end)
            .map(|r| {
                self.columns
                    .iter()
                    .map(|c| {
                        let cell = c.get(r);
                        debug_assert!(cell.is_some(), "column shorter than the table");
                        cell.unwrap_or("").to_owned()
                    })
                    .collect()
            })
            .collect();
        Window { start, rows }
    }

    /// Bytes of heap memory the data holds, for budget reports.
    pub fn heap_bytes(&self) -> usize {
        self.columns.iter().map(Column::heap_bytes).sum::<usize>()
            + self.names.iter().map(String::len).sum::<usize>()
    }
}

/// Builds a [`Table`] row by row.
#[derive(Debug)]
pub struct TableBuilder {
    names: Vec<String>,
    columns: Vec<ColumnBuilder>,
    rows: usize,
    broken: Option<TableError>,
}

impl TableBuilder {
    /// Starts a table with these column names.
    pub fn new<S: AsRef<str>>(names: &[S]) -> Result<Self, TableError> {
        if names.len() > MAX_COLUMNS {
            return Err(TableError::TooManyColumns(names.len()));
        }
        Ok(Self {
            names: names.iter().map(|n| n.as_ref().to_owned()).collect(),
            columns: names.iter().map(|_| ColumnBuilder::new()).collect(),
            rows: 0,
            broken: None,
        })
    }

    /// Adds one row. It must have exactly one cell per column.
    ///
    /// A [`TableError::ColumnFull`] error can leave some columns one cell ahead, so the builder then
    /// refuses every later row with the same error. Drop it and start again.
    pub fn push_row<S: AsRef<str>>(&mut self, cells: &[S]) -> Result<(), TableError> {
        if let Some(e) = &self.broken {
            return Err(e.clone());
        }
        if cells.len() != self.columns.len() {
            return Err(TableError::WrongCellCount {
                expected: self.columns.len(),
                found: cells.len(),
            });
        }
        for (i, (builder, cell)) in self.columns.iter_mut().zip(cells).enumerate() {
            if let Err(ColumnFull) = builder.push(cell.as_ref()) {
                let e = TableError::ColumnFull { column: i };
                self.broken = Some(e.clone());
                return Err(e);
            }
        }
        self.rows += 1;
        Ok(())
    }

    /// Rows added so far.
    pub fn row_count(&self) -> usize {
        self.rows
    }

    /// Finishes the table.
    pub fn finish(self) -> Table {
        Table {
            names: self.names,
            columns: self
                .columns
                .into_iter()
                .map(ColumnBuilder::finish)
                .collect(),
            rows: self.rows,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColumnKind;

    fn small() -> Table {
        let mut b = TableBuilder::new(&["market", "price"]).unwrap();
        for (m, p) in [("Kisenyi", "1,200"), ("Owino", "900"), ("Kisenyi", "")] {
            b.push_row(&[m, p]).unwrap();
        }
        b.finish()
    }

    #[test]
    fn reads_cells_back() {
        let t = small();
        assert_eq!(t.row_count(), 3);
        assert_eq!(t.column_names(), ["market", "price"]);
        assert_eq!(t.cell(0, 0), Some("Kisenyi"));
        assert_eq!(t.cell(2, 1), Some(""));
        assert_eq!(t.cell(3, 0), None);
        assert_eq!(t.cell(0, 2), None);
    }

    #[test]
    fn windows_are_clamped() {
        let t = small();
        assert_eq!(
            t.window(1, 1).rows,
            vec![vec!["Owino".to_string(), "900".to_string()]]
        );
        assert_eq!(t.window(2, 100).rows.len(), 1);
        assert!(t.window(50, 10).rows.is_empty());
        assert_eq!(t.window(usize::MAX, usize::MAX).rows.len(), 0);
    }

    #[test]
    fn repeated_text_is_a_dictionary() {
        let t = small();
        assert_eq!(t.column(0).unwrap().kind(), ColumnKind::Dictionary);
    }

    #[test]
    fn many_distinct_values_become_plain_and_stay_correct() {
        let mut b = TableBuilder::new(&["id"]).unwrap();
        for i in 0..10_000 {
            b.push_row(&[format!("row-{i}")]).unwrap();
        }
        let t = b.finish();
        assert_eq!(t.column(0).unwrap().kind(), ColumnKind::Plain);
        assert_eq!(t.cell(0, 0), Some("row-0"));
        assert_eq!(t.cell(4_095, 0), Some("row-4095"));
        assert_eq!(t.cell(9_999, 0), Some("row-9999"));
    }

    #[test]
    fn rejects_ragged_rows_and_too_many_columns() {
        let mut b = TableBuilder::new(&["a", "b"]).unwrap();
        assert_eq!(
            b.push_row(&["x"]),
            Err(TableError::WrongCellCount {
                expected: 2,
                found: 1
            })
        );
        let many: Vec<String> = (0..=crate::MAX_COLUMNS).map(|i| i.to_string()).collect();
        assert!(matches!(
            TableBuilder::new(&many),
            Err(TableError::TooManyColumns(_))
        ));
    }

    #[test]
    fn unicode_and_empty_cells_round_trip() {
        let mut b = TableBuilder::new(&["t"]).unwrap();
        for s in ["", "Entebbe — café", "日本語", ""] {
            b.push_row(&[s]).unwrap();
        }
        let t = b.finish();
        assert_eq!(t.cell(1, 0), Some("Entebbe — café"));
        assert_eq!(t.cell(3, 0), Some(""));
    }
}
