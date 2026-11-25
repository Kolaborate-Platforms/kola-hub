//! Running steps on a grid of text, tracking which source row each row came from.

use std::collections::{HashMap, HashSet};

use kh_core::{Table, TableBuilder};
use serde::Serialize;

use crate::steps::{SpellingGroup, Step, TargetType};
use crate::values::{parse_date, parse_number};

/// Why a cell needs a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Reason {
    /// A number column has an empty cell.
    EmptyNumber,
    /// The text could not be read as a number.
    UnreadableNumber,
    /// The text could not be read as a date without guessing.
    UnreadableDate,
}

/// A cell a step left unchanged because a person must decide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Flag {
    /// Zero based row in the file the steps started from.
    pub source_row: usize,
    /// Column name when the step ran.
    pub column: String,
    /// The value, unchanged.
    pub value: String,
    /// Why it needs a person.
    pub reason: Reason,
}

/// Why a step could not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepError {
    /// The step names a column the table does not have.
    MissingColumn(String),
    /// Several columns have this name, so the step cannot know which one is meant.
    RepeatedColumn(String),
    /// Renaming would give two columns the same name.
    DuplicateName(String),
    /// The table could not be rebuilt.
    Table(String),
}

impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepError::RepeatedColumn(name) => {
                write!(f, "there is more than one column called {name}")
            }
            StepError::MissingColumn(c) => write!(f, "there is no column called \"{c}\""),
            StepError::DuplicateName(c) => write!(f, "there is already a column called \"{c}\""),
            StepError::Table(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for StepError {}

/// Column-major text with the source row of every row.
#[derive(Debug, Clone)]
pub(crate) struct Grid {
    pub names: Vec<String>,
    pub cols: Vec<Vec<String>>,
    pub ids: Vec<usize>,
}

impl Grid {
    pub fn from_table(table: &Table, start: usize, count: usize) -> Grid {
        let end = start.saturating_add(count).min(table.row_count());
        let start = start.min(end);
        let cols = (0..table.column_count())
            .map(|c| {
                (start..end)
                    .map(|r| table.cell(r, c).unwrap_or("").to_owned())
                    .collect()
            })
            .collect();
        Grid {
            names: table.column_names().to_vec(),
            cols,
            ids: (start..end).collect(),
        }
    }

    pub fn to_table(&self) -> Result<Table, StepError> {
        let mut b = TableBuilder::new(&self.names).map_err(|e| StepError::Table(e.to_string()))?;
        let mut row: Vec<&str> = Vec::with_capacity(self.cols.len());
        for r in 0..self.ids.len() {
            row.clear();
            row.extend(self.cols.iter().map(|c| c[r].as_str()));
            b.push_row(&row)
                .map_err(|e| StepError::Table(e.to_string()))?;
        }
        Ok(b.finish())
    }

    fn index(&self, name: &str) -> Result<usize, StepError> {
        let mut matches = self.names.iter().enumerate().filter(|(_, n)| *n == name);
        match (matches.next(), matches.next()) {
            (Some((index, _)), None) => Ok(index),
            (Some(_), Some(_)) => Err(StepError::RepeatedColumn(name.to_owned())),
            (None, _) => Err(StepError::MissingColumn(name.to_owned())),
        }
    }

    fn keep_rows(&mut self, keep: &[bool]) {
        for col in &mut self.cols {
            let mut i = 0;
            col.retain(|_| {
                i += 1;
                keep[i - 1]
            });
        }
        let mut i = 0;
        self.ids.retain(|_| {
            i += 1;
            keep[i - 1]
        });
    }
}

/// Columns a step needs, and the name changes it makes, for the match report.
pub(crate) fn needed_columns(step: &Step) -> Vec<&str> {
    match step {
        Step::TrimSpaces { columns } => columns.iter().flatten().map(String::as_str).collect(),
        Step::MakeDates { column, .. }
        | Step::TextToNumbers { column }
        | Step::MergeSpellings { column, .. }
        | Step::ChangeType { column, .. } => vec![column],
        Step::RenameColumn { from, .. } => vec![from],
        Step::RemoveDuplicateRows | Step::RemoveEmptyRows => Vec::new(),
    }
}

fn each_cell(grid: &mut Grid, col: usize, mut f: impl FnMut(usize, &mut String)) {
    for (row, cell) in grid.cols[col].iter_mut().enumerate() {
        f(row, cell);
    }
}

fn make_dates(grid: &mut Grid, col: usize, day_first: bool, flags: &mut Vec<Flag>) {
    let (name, ids) = (grid.names[col].clone(), grid.ids.clone());
    each_cell(grid, col, |row, cell| {
        if cell.trim().is_empty() {
            return;
        }
        match parse_date(cell, day_first) {
            Some(date) => *cell = date,
            None => flags.push(Flag {
                source_row: ids[row],
                column: name.clone(),
                value: cell.clone(),
                reason: Reason::UnreadableDate,
            }),
        }
    });
}

fn to_numbers(grid: &mut Grid, col: usize, flags: &mut Vec<Flag>) {
    let (name, ids) = (grid.names[col].clone(), grid.ids.clone());
    each_cell(grid, col, |row, cell| {
        let reason = if cell.trim().is_empty() {
            Reason::EmptyNumber
        } else if let Some(n) = parse_number(cell) {
            *cell = n;
            return;
        } else {
            Reason::UnreadableNumber
        };
        flags.push(Flag {
            source_row: ids[row],
            column: name.clone(),
            value: cell.clone(),
            reason,
        });
    });
}

fn merge_spellings(grid: &mut Grid, col: usize, groups: &[SpellingGroup]) {
    let map: HashMap<&str, &str> = groups
        .iter()
        .flat_map(|g| {
            g.variants
                .iter()
                .map(|v| (v.as_str(), g.canonical.as_str()))
        })
        .collect();
    each_cell(grid, col, |_, cell| {
        if let Some(&to) = map.get(cell.as_str())
            && to != cell
        {
            *cell = to.to_owned();
        }
    });
}

fn trim(grid: &mut Grid, columns: Option<&[String]>) -> Result<(), StepError> {
    let targets: Vec<usize> = match columns {
        Some(names) => names
            .iter()
            .map(|n| grid.index(n))
            .collect::<Result<_, _>>()?,
        None => (0..grid.cols.len()).collect(),
    };
    for c in targets {
        each_cell(grid, c, |_, cell| {
            let t = cell.trim();
            if t.len() != cell.len() {
                *cell = t.to_owned();
            }
        });
    }
    Ok(())
}

fn remove_duplicates(grid: &mut Grid) {
    let mut seen: HashSet<Vec<&str>> = HashSet::with_capacity(grid.ids.len());
    let keep: Vec<bool> = (0..grid.ids.len())
        .map(|r| seen.insert(grid.cols.iter().map(|c| c[r].as_str()).collect()))
        .collect();
    grid.keep_rows(&keep);
}

fn remove_empty(grid: &mut Grid) {
    let keep: Vec<bool> = (0..grid.ids.len())
        .map(|r| grid.cols.iter().any(|c| !c[r].trim().is_empty()))
        .collect();
    grid.keep_rows(&keep);
}

fn rename(grid: &mut Grid, from: &str, to: &str) -> Result<(), StepError> {
    let i = grid.index(from)?;
    if from != to && grid.names.iter().any(|n| n == to) {
        return Err(StepError::DuplicateName(to.to_owned()));
    }
    grid.names[i] = to.to_owned();
    Ok(())
}

/// Runs one step. Cells that need a person are added to `flags` and left unchanged.
pub(crate) fn apply_step(
    grid: &mut Grid,
    step: &Step,
    flags: &mut Vec<Flag>,
) -> Result<(), StepError> {
    match step {
        Step::TrimSpaces { columns } => trim(grid, columns.as_deref())?,
        Step::MakeDates { column, day_first } => {
            let c = grid.index(column)?;
            make_dates(grid, c, *day_first, flags);
        }
        Step::TextToNumbers { column } => {
            let c = grid.index(column)?;
            to_numbers(grid, c, flags);
        }
        Step::MergeSpellings { column, groups } => {
            let c = grid.index(column)?;
            merge_spellings(grid, c, groups);
        }
        Step::RemoveDuplicateRows => remove_duplicates(grid),
        Step::RemoveEmptyRows => remove_empty(grid),
        Step::RenameColumn { from, to } => rename(grid, from, to)?,
        Step::ChangeType { column, to } => {
            let c = grid.index(column)?;
            match to {
                TargetType::Text => trim(grid, Some(std::slice::from_ref(column)))?,
                TargetType::Number => to_numbers(grid, c, flags),
                TargetType::Date => make_dates(grid, c, true, flags),
            }
        }
    }
    Ok(())
}

/// Runs steps in order. Flags for rows a later step removes are dropped.
pub(crate) fn run_steps<'a>(
    grid: &mut Grid,
    steps: impl IntoIterator<Item = &'a Step>,
) -> Result<Vec<Flag>, StepError> {
    let mut flags = Vec::new();
    for step in steps {
        apply_step(grid, step, &mut flags)?;
    }
    let alive: HashSet<usize> = grid.ids.iter().copied().collect();
    flags.retain(|f| alive.contains(&f.source_row));
    Ok(flags)
}
