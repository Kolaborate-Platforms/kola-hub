//! Looking at a table without changing it: column types, cells that do not fit, duplicates.

use std::collections::HashSet;

use kh_core::Table;
use serde::Serialize;

use crate::values::{is_clean_date, is_clean_number, parse_date, parse_number};

/// Most highlighted cells kept per issue. The count is always exact.
pub const MAX_ISSUE_CELLS: usize = 200;

/// What a column mostly holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DetectedType {
    /// Every cell is empty.
    Empty,
    /// Mostly numbers.
    Number,
    /// Mostly dates.
    Date,
    /// Anything else.
    Text,
}

/// What is wrong, in plain terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IssueKind {
    /// Rows that are exactly the same as an earlier row.
    DuplicateRows,
    /// Cells with spaces at the start or end.
    ExtraSpaces,
    /// Dates that are not yet in `YYYY-MM-DD` form but can be read.
    DatesNotReal,
    /// Dates that cannot be read without guessing.
    UnreadableDates,
    /// Numbers with currency words or commas.
    NumbersWithText,
    /// Number cells that cannot be read.
    UnreadableNumbers,
    /// Empty cells in a number or date column.
    EmptyCells,
}

/// A cell to highlight. Rows and columns count from zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CellRef {
    /// Row in the table.
    pub row: usize,
    /// Column in the table.
    pub column: usize,
}

/// One kind of problem, in one column (or the whole file).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    /// The kind of problem.
    pub kind: IssueKind,
    /// Column name, or `None` for the whole file.
    pub column: Option<String>,
    /// Exact number of cells (or rows, for duplicates).
    pub count: usize,
    /// The first cells to highlight, at most [`MAX_ISSUE_CELLS`]. For duplicate rows, the first cell of each row.
    pub cells: Vec<CellRef>,
}

/// What the profiler found in one column.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnProfile {
    /// Column name.
    pub name: String,
    /// What the column mostly holds.
    pub detected_type: DetectedType,
    /// Non-empty cells that are not yet in clean form, as a share of all cells (0 to 1).
    pub share_not_fitting: f64,
    /// Cells that are not yet in clean form (for text columns, only cells with extra spaces).
    pub not_fitting: usize,
    /// Different values, counting spacing and case as different.
    pub distinct: usize,
    /// Empty cells.
    pub empty: usize,
}

/// The whole picture. Nothing in the table has changed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// Data rows.
    pub rows: usize,
    /// One entry per column.
    pub columns: Vec<ColumnProfile>,
    /// Cells that look wrong, each counted once.
    pub wrong_cells: usize,
    /// Rows that repeat an earlier row exactly.
    pub duplicate_rows: usize,
    /// Problems, biggest first.
    pub issues: Vec<Issue>,
}

struct Tally {
    count: usize,
    cells: Vec<CellRef>,
}

impl Tally {
    fn new() -> Self {
        Self {
            count: 0,
            cells: Vec::new(),
        }
    }

    fn add(&mut self, row: usize, column: usize) {
        self.count += 1;
        if self.cells.len() < MAX_ISSUE_CELLS {
            self.cells.push(CellRef { row, column });
        }
    }
}

fn detect(table: &Table, c: usize) -> DetectedType {
    let (mut filled, mut dates, mut numbers) = (0usize, 0usize, 0usize);
    for r in 0..table.row_count() {
        let cell = table.cell(r, c).unwrap_or("").trim();
        if cell.is_empty() {
            continue;
        }
        filled += 1;
        dates += usize::from(parse_date(cell, true).is_some());
        numbers += usize::from(parse_number(cell).is_some());
    }
    // More than half the filled cells must read as the type.
    if filled == 0 {
        DetectedType::Empty
    } else if numbers * 2 > filled && numbers >= dates {
        DetectedType::Number
    } else if dates * 2 > filled {
        DetectedType::Date
    } else {
        DetectedType::Text
    }
}

fn profile_column(
    table: &Table,
    c: usize,
    issues: &mut Vec<Issue>,
    wrong: &mut HashSet<(usize, usize)>,
) -> ColumnProfile {
    let name = table.column_names()[c].clone();
    let kind = detect(table, c);
    let mut spaces = Tally::new();
    let (mut messy, mut unreadable, mut empty_cells) = (Tally::new(), Tally::new(), Tally::new());
    let mut distinct: HashSet<&str> = HashSet::new();
    let mut empty = 0;
    for r in 0..table.row_count() {
        let cell = table.cell(r, c).unwrap_or("");
        distinct.insert(cell);
        let t = cell.trim();
        if t.len() != cell.len() {
            spaces.add(r, c);
            wrong.insert((r, c));
        }
        if t.is_empty() {
            empty += 1;
            if matches!(kind, DetectedType::Number | DetectedType::Date) {
                empty_cells.add(r, c);
                wrong.insert((r, c));
            }
            continue;
        }
        let (readable, clean) = match kind {
            DetectedType::Number => (parse_number(t).is_some(), is_clean_number(cell)),
            DetectedType::Date => (parse_date(t, true).is_some(), is_clean_date(cell)),
            _ => continue,
        };
        if !readable {
            unreadable.add(r, c);
        } else if !clean && t.len() == cell.len() {
            messy.add(r, c);
        }
        if !clean {
            wrong.insert((r, c));
        }
    }
    let (messy_kind, bad_kind) = match kind {
        DetectedType::Number => (IssueKind::NumbersWithText, IssueKind::UnreadableNumbers),
        _ => (IssueKind::DatesNotReal, IssueKind::UnreadableDates),
    };
    let found = [
        (IssueKind::ExtraSpaces, spaces),
        (messy_kind, messy),
        (bad_kind, unreadable),
        (IssueKind::EmptyCells, empty_cells),
    ];
    let not_fitting = (0..table.row_count())
        .filter(|&r| wrong.contains(&(r, c)))
        .count();
    for (kind, t) in found {
        if t.count > 0 {
            issues.push(Issue {
                kind,
                column: Some(name.clone()),
                count: t.count,
                cells: t.cells,
            });
        }
    }
    let rows = table.row_count().max(1);
    ColumnProfile {
        name,
        detected_type: kind,
        share_not_fitting: not_fitting as f64 / rows as f64,
        not_fitting,
        distinct: distinct.len(),
        empty,
    }
}

/// Rows that repeat an earlier row exactly, as row numbers.
pub fn duplicate_rows(table: &Table) -> Vec<usize> {
    let mut seen: HashSet<Vec<&str>> = HashSet::with_capacity(table.row_count());
    (0..table.row_count())
        .filter(|&r| {
            let row: Vec<&str> = (0..table.column_count())
                .map(|c| table.cell(r, c).unwrap_or(""))
                .collect();
            !seen.insert(row)
        })
        .collect()
}

/// Profiles a table. Issues are sorted biggest first.
pub fn profile(table: &Table) -> Profile {
    let mut issues = Vec::new();
    let mut wrong = HashSet::new();
    let columns = (0..table.column_count())
        .map(|c| profile_column(table, c, &mut issues, &mut wrong))
        .collect();
    let dups = duplicate_rows(table);
    if !dups.is_empty() {
        issues.push(Issue {
            kind: IssueKind::DuplicateRows,
            column: None,
            count: dups.len(),
            cells: dups
                .iter()
                .take(MAX_ISSUE_CELLS)
                .map(|&row| CellRef { row, column: 0 })
                .collect(),
        });
    }
    issues.sort_by_key(|i| std::cmp::Reverse(i.count));
    Profile {
        rows: table.row_count(),
        columns,
        wrong_cells: wrong.len(),
        duplicate_rows: dups.len(),
        issues,
    }
}
