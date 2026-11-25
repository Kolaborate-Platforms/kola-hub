//! Preview on a window of rows, replay on a new file, and saving the result.

use std::io::Write;

use kh_core::Table;
use kh_formats::{CsvWriteError, CsvWriteOptions, write_csv};
use serde::Serialize;

use crate::apply::{Flag, Grid, StepError, apply_step, needed_columns, run_steps};
use crate::steps::{Step, StepList};

/// One cell in a preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewCell {
    /// What the cell holds now (struck through in the interface when it changes).
    pub old: String,
    /// What it will hold (shown in green when it changes).
    pub new: String,
    /// True when the step changes the cell.
    pub changed: bool,
    /// True when the cell needs a person and is left unchanged.
    pub needs_person: bool,
}

/// One row in a preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRow {
    /// Row in the table.
    pub source_row: usize,
    /// True when the step would remove this row.
    pub removed: bool,
    /// One entry per column of the table before the step.
    pub cells: Vec<PreviewCell>,
}

/// A step shown on a window of rows. Nothing is applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    /// Column names before the step.
    pub columns: Vec<String>,
    /// Names after the step, which differ only for a rename.
    pub new_columns: Vec<String>,
    /// The rows of the window.
    pub rows: Vec<PreviewRow>,
}

/// Shows `step` on `count` rows from `start`, after the enabled steps in `before` have run on that window.
///
/// Only the window is looked at, so a duplicate row is found only when its twin is in the window too.
pub fn preview(
    table: &Table,
    before: &StepList,
    step: &Step,
    start: usize,
    count: usize,
) -> Result<Preview, StepError> {
    let mut grid = Grid::from_table(table, start, count);
    run_steps(&mut grid, before.enabled_steps())?;
    let old = grid.clone();
    let mut flags = Vec::new();
    apply_step(&mut grid, step, &mut flags)?;
    let rows = old
        .ids
        .iter()
        .enumerate()
        .map(|(r, &source_row)| {
            let new_pos = grid.ids.iter().position(|&i| i == source_row);
            let cells = (0..old.cols.len())
                .map(|c| {
                    let new = new_pos.map_or_else(
                        || old.cols[c][r].clone(),
                        |p| {
                            grid.cols
                                .get(c)
                                .map_or_else(|| old.cols[c][r].clone(), |col| col[p].clone())
                        },
                    );
                    let needs_person = flags
                        .iter()
                        .any(|f| f.source_row == source_row && f.column == old.names[c]);
                    PreviewCell {
                        changed: new != old.cols[c][r],
                        old: old.cols[c][r].clone(),
                        new,
                        needs_person,
                    }
                })
                .collect();
            PreviewRow {
                source_row,
                removed: new_pos.is_none(),
                cells,
            }
        })
        .collect();
    Ok(Preview {
        columns: old.names,
        new_columns: grid.names,
        rows,
    })
}

/// A column a saved step list needs, and whether the new file has it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMatch {
    /// Column name.
    pub name: String,
    /// True when the new file has exactly one column of this name when the step needs it.
    pub found: bool,
    /// True when the file has several columns of this name.
    pub repeated: bool,
}

/// Which columns the steps need and whether the new file has them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchReport {
    /// One entry per needed column, in the order the steps need them.
    pub columns: Vec<ColumnMatch>,
}

impl MatchReport {
    /// Names of columns the new file lacks.
    pub fn missing(&self) -> Vec<&str> {
        self.columns
            .iter()
            .filter(|c| !c.found)
            .map(|c| c.name.as_str())
            .collect()
    }
}

/// Checks a step list against a table without running it. Renames made by earlier steps are followed.
pub fn match_report(table: &Table, steps: &StepList) -> MatchReport {
    let mut names: Vec<String> = table.column_names().to_vec();
    let mut columns: Vec<ColumnMatch> = Vec::new();
    for step in steps.enabled_steps() {
        for need in needed_columns(step) {
            if !columns.iter().any(|c| c.name == need) {
                columns.push(ColumnMatch {
                    name: need.to_owned(),
                    // Two columns with one name would force a guess, so that does not count.
                    found: names.iter().filter(|n| *n == need).count() == 1,
                    repeated: names.iter().filter(|n| *n == need).count() > 1,
                });
            }
        }
        if let Step::RenameColumn { from, to } = step
            && let Some(n) = names.iter_mut().find(|n| *n == from)
        {
            *n = to.clone();
        }
    }
    MatchReport { columns }
}

/// A cell the replay left unchanged for a person to fix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NeedsPerson {
    /// Row in the result table.
    pub row: usize,
    /// Column in the result table.
    pub column: usize,
    /// Row in the file the steps started from.
    pub source_row: usize,
    /// Column name.
    pub column_name: String,
    /// The value, unchanged.
    pub value: String,
    /// Why.
    pub reason: crate::apply::Reason,
}

/// Counts for the result screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Rows in the file.
    pub rows_in: usize,
    /// Rows in the result.
    pub rows_out: usize,
    /// Rows with at least one cell that needs a person.
    pub rows_needing_person: usize,
}

/// What a replay produced.
#[derive(Debug)]
pub struct ReplayResult {
    /// The cleaned table. The original is untouched.
    pub table: Table,
    /// Columns found and missing.
    pub report: MatchReport,
    /// Rows in, rows out, rows needing a person.
    pub summary: Summary,
    /// The cells to show for fixing.
    pub needs_person: Vec<NeedsPerson>,
}

/// Why a replay did not run.
#[derive(Debug)]
pub enum ReplayError {
    /// The new file lacks columns the steps need. Nothing was run and nothing was guessed.
    MissingColumns(MatchReport),
    /// A step failed.
    Step(StepError),
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::MissingColumns(r) => write!(
                f,
                "this file does not have exactly one column for each of these: column{} {}",
                if r.missing().len() == 1 { "" } else { "s" },
                r.missing().join(", ")
            ),
            ReplayError::Step(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ReplayError {}

impl From<StepError> for ReplayError {
    fn from(e: StepError) -> Self {
        ReplayError::Step(e)
    }
}

/// Runs the enabled steps on a table. Stops with the match report when columns are missing.
pub fn replay(table: &Table, steps: &StepList) -> Result<ReplayResult, ReplayError> {
    let report = match_report(table, steps);
    if !report.missing().is_empty() {
        return Err(ReplayError::MissingColumns(report));
    }
    let mut grid = Grid::from_table(table, 0, table.row_count());
    let flags = run_steps(&mut grid, steps.enabled_steps())?;
    let needs_person = needs_person_list(&grid, flags);
    let mut rows: Vec<usize> = needs_person.iter().map(|n| n.row).collect();
    rows.dedup();
    let summary = Summary {
        rows_in: table.row_count(),
        rows_out: grid.ids.len(),
        rows_needing_person: rows.len(),
    };
    Ok(ReplayResult {
        table: grid.to_table()?,
        report,
        summary,
        needs_person,
    })
}

fn needs_person_list(grid: &Grid, flags: Vec<Flag>) -> Vec<NeedsPerson> {
    let mut list: Vec<NeedsPerson> = flags
        .into_iter()
        .filter_map(|f| {
            let row = grid.ids.binary_search(&f.source_row).ok()?;
            let column = grid.names.iter().position(|n| *n == f.column)?;
            Some(NeedsPerson {
                row,
                column,
                source_row: f.source_row,
                column_name: f.column,
                value: f.value,
                reason: f.reason,
            })
        })
        .collect();
    list.sort_by_key(|n| (n.row, n.column));
    list
}

/// Returns a copy of `table` with one cell replaced, for the inline fix on a row that needs a person.
/// Returns `None` when the cell does not exist.
pub fn fix_cell(table: &Table, row: usize, column: usize, value: &str) -> Option<Table> {
    table.cell(row, column)?;
    let mut grid = Grid::from_table(table, 0, table.row_count());
    grid.cols[column][row] = value.to_owned();
    grid.to_table().ok()
}

/// Writes a table as CSV to a writer the caller opened. Cells that start with `=`, `+`, `-` or `@` are
/// neutralised.
pub fn save_csv<W: Write>(table: &Table, out: W) -> Result<(), CsvWriteError> {
    write_csv(table, out, &CsvWriteOptions::default())
}
