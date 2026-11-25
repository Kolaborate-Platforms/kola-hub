//! Clean sessions: one table per open file, the step list on top of it, saved after every change.

use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom};
use std::mem;

use kh_clean::{
    MatchReport, NeedsPerson, Preview, Profile, ReplayError, SpellingGroup, Step, StepEntry,
    StepList, StepListError, Summary,
};
use kh_core::Table;
use kh_formats::{CsvError, CsvLimits, read_csv, sniff_delimiter};
use kh_store::clean_steps::MAX_STEPS_BYTES;
use kh_store::{FileId, Project, ProjectId, SavedFile};
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::shell::{Shell, lock_session};

/// Most rows one window request may ask for.
pub const MAX_WINDOW_ROWS: usize = 500;
/// Most cells that need a person sent with the state.
pub const MAX_NEEDS_PERSON: usize = 500;
/// Most spelling groups sent back for one column.
pub const MAX_SPELLING_GROUPS: usize = 200;
/// Longest value a person may type into one cell, matching the reader's cell limit.
pub const MAX_CELL_BYTES: usize = 1 << 20;
/// Most steps one file may have.
pub const MAX_STEPS: usize = 200;
const SNIFF_BYTES: usize = 8192;
/// Label under which a saved step list that cannot be read is kept before anything replaces it.
const UNREADABLE_LABEL: &str = "unreadable-clean-steps";
const SAVE_LABEL: &str = "cleaned";

/// A change the person typed into one cell, applied after the steps.
#[derive(Debug, Clone)]
struct Fixed {
    row: usize,
    column: usize,
    value: String,
}

struct Cleaned {
    table: Table,
    summary: Summary,
    needs_person: Vec<NeedsPerson>,
}

/// Why the saved steps cannot run on this file right now.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Blocked {
    /// Which columns the steps need and whether the file has them.
    pub report: MatchReport,
    /// The reason, in plain English.
    pub message: String,
}

enum View {
    /// No enabled steps and no fixes: the file as it was read, with no second copy.
    Source,
    Cleaned(Cleaned),
    /// The steps cannot run, so the file is shown as it is and nothing is guessed.
    Blocked(Blocked),
}

/// One open file in Clean.
pub struct Session {
    project: Project,
    file: FileId,
    file_name: String,
    source: Table,
    steps: StepList,
    fixes: Vec<Fixed>,
    view: View,
    profile: Option<Profile>,
}

/// What the interface needs to draw the step list and the result.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanState {
    /// The steps with their names and switches.
    pub steps: Vec<StepEntry>,
    /// True when a step can be taken off.
    pub can_undo: bool,
    /// True when a step can be put back.
    pub can_redo: bool,
    /// Rows in, rows out and rows that need a person.
    pub summary: Summary,
    /// Cells that need a person, at most 500.
    pub needs_person: Vec<NeedsPerson>,
    /// Rows after the enabled steps.
    pub row_count: usize,
    /// Column names after the enabled steps.
    pub columns: Vec<String>,
    /// Set when the steps could not run on this file.
    pub blocked: Option<Blocked>,
}

/// A newly opened file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanSession {
    /// Id for every later call.
    pub session_id: String,
    /// The file's name.
    pub file_name: String,
    /// Rows after the saved steps.
    pub row_count: usize,
    /// Column names after the saved steps.
    pub columns: Vec<String>,
    /// The step list and result.
    pub state: CleanState,
}

/// A window of the table after the enabled steps.
#[derive(Debug, Clone, Serialize)]
pub struct WindowView {
    /// Index of the first row returned.
    pub start: usize,
    /// Column names.
    pub columns: Vec<String>,
    /// The rows.
    pub rows: Vec<Vec<String>>,
}

/// The answer to "use the steps from another file".
#[derive(Debug, Clone, Serialize)]
pub struct ReuseResult {
    /// Which columns the steps need and whether this file has them.
    pub report: MatchReport,
    /// The new state, or `None` when the steps could not run and nothing was changed.
    pub state: Option<CleanState>,
}

fn csv_failure(error: &CsvError) -> AppError {
    let message = match error {
        CsvError::Io(_) => "The file could not be read.".to_owned(),
        other => format!("This file cannot be opened as a table: {other}."),
    };
    AppError::new("cannot-open", message)
}

fn replay_failure(error: &ReplayError) -> AppError {
    let code = match error {
        ReplayError::MissingColumns(_) => "missing-columns",
        ReplayError::Step(_) => "step-failed",
    };
    AppError::new(code, format!("The steps cannot run: {error}."))
}

fn steps_failure(error: &StepListError) -> AppError {
    let code = match error {
        StepListError::TooNew(_) => "newer-version",
        StepListError::Unreadable(_) => "steps-unreadable",
    };
    AppError::new(code, format!("The saved steps cannot be used: {error}."))
}

fn read_table(project: &Project, file: &FileId) -> AppResult<Table> {
    let mut handle = project.open_file(file)?;
    let mut sample = vec![0u8; SNIFF_BYTES];
    let read = handle.read(&mut sample)?;
    let delimiter = sniff_delimiter(&sample[..read]);
    handle.seek(SeekFrom::Start(0))?;
    let (table, _report) = read_csv(BufReader::new(handle), delimiter, &CsvLimits::default())
        .map_err(|e| csv_failure(&e))?;
    Ok(table)
}

fn compute(source: &Table, steps: &StepList) -> AppResult<View> {
    if steps.enabled_steps().next().is_none() {
        return Ok(View::Source);
    }
    let result = kh_clean::replay(source, steps).map_err(|e| replay_failure(&e))?;
    Ok(View::Cleaned(Cleaned {
        table: result.table,
        summary: result.summary,
        needs_person: result.needs_person,
    }))
}

fn compute_or_block(source: &Table, steps: &StepList) -> View {
    compute(source, steps).unwrap_or_else(|error| {
        View::Blocked(Blocked {
            report: kh_clean::match_report(source, steps),
            message: error.message,
        })
    })
}

fn rows_needing(list: &[NeedsPerson]) -> usize {
    let mut rows: Vec<usize> = list.iter().map(|n| n.row).collect();
    rows.dedup();
    rows.len()
}

impl Session {
    fn table(&self) -> &Table {
        match &self.view {
            View::Cleaned(c) => &c.table,
            View::Source | View::Blocked(_) => &self.source,
        }
    }

    fn persist(&self) -> AppResult<()> {
        self.project
            .save_clean_steps(&self.file, &self.steps.to_json())?;
        Ok(())
    }

    fn state(&self) -> CleanState {
        let table = self.table();
        let (summary, needs_person) = match &self.view {
            View::Cleaned(c) => (
                c.summary,
                c.needs_person
                    .iter()
                    .take(MAX_NEEDS_PERSON)
                    .cloned()
                    .collect(),
            ),
            View::Source | View::Blocked(_) => (
                Summary {
                    rows_in: self.source.row_count(),
                    rows_out: self.source.row_count(),
                    rows_needing_person: 0,
                },
                Vec::new(),
            ),
        };
        let mut redo = self.steps.clone();
        CleanState {
            steps: self.steps.steps.clone(),
            can_undo: !self.steps.steps.is_empty(),
            can_redo: redo.redo(),
            summary,
            needs_person,
            row_count: table.row_count(),
            columns: table.column_names().to_vec(),
            blocked: match &self.view {
                View::Blocked(b) => Some(b.clone()),
                _ => None,
            },
        }
    }

    /// Edits the list, runs it, and saves it. If anything fails the list and the view go back as they
    /// were. The old result is dropped before the new one is built, so two results never share memory.
    fn change(&mut self, edit: impl FnOnce(&mut StepList) -> bool) -> AppResult<CleanState> {
        let before = self.steps.clone();
        if !edit(&mut self.steps) {
            self.steps = before;
            return Err(AppError::new("bad-step", "There is no such step."));
        }
        // Checked before the replay, which is where a huge step would cost time and memory.
        if let Err(error) = within_limits(&self.steps) {
            self.steps = before;
            return Err(error);
        }
        let fixes = mem::take(&mut self.fixes);
        self.view = View::Source;
        self.profile = None;
        match compute(&self.source, &self.steps).and_then(|view| {
            self.view = view;
            self.persist()
        }) {
            Ok(()) => Ok(self.state()),
            Err(error) => {
                self.steps = before;
                self.view = compute_or_block(&self.source, &self.steps);
                self.fixes = fixes;
                self.reapply_fixes();
                Err(error)
            }
        }
    }

    fn reapply_fixes(&mut self) {
        for fix in mem::take(&mut self.fixes) {
            if self.apply_fix(fix.row, fix.column, &fix.value).is_err() {
                break;
            }
        }
    }

    fn apply_fix(&mut self, row: usize, column: usize, value: &str) -> AppResult<()> {
        if value.len() > MAX_CELL_BYTES {
            return Err(AppError::new(
                "too-long",
                "That value is too long for one cell.",
            ));
        }
        if matches!(self.view, View::Blocked(_)) {
            return Err(AppError::new(
                "blocked",
                "Fix the steps first. They cannot run on this file.",
            ));
        }
        let fixed =
            kh_clean::fix_cell(self.table(), row, column, value).ok_or_else(AppError::not_found)?;
        let rows = self.table().row_count();
        let mut needs = match mem::replace(&mut self.view, View::Source) {
            View::Cleaned(c) => c.needs_person,
            _ => Vec::new(),
        };
        needs.retain(|n| !(n.row == row && n.column == column));
        let summary = Summary {
            rows_in: self.source.row_count(),
            rows_out: rows,
            rows_needing_person: rows_needing(&needs),
        };
        self.view = View::Cleaned(Cleaned {
            table: fixed,
            summary,
            needs_person: needs,
        });
        self.fixes.retain(|f| !(f.row == row && f.column == column));
        self.fixes.push(Fixed {
            row,
            column,
            value: value.to_owned(),
        });
        self.profile = None;
        Ok(())
    }
}

fn too_many_steps() -> AppError {
    AppError::new(
        "steps-too-large",
        "This list of steps is too long. Nothing was changed.",
    )
}

fn within_limits(steps: &StepList) -> AppResult<()> {
    if steps.steps.len() > MAX_STEPS || steps.to_json().len() > MAX_STEPS_BYTES {
        return Err(too_many_steps());
    }
    Ok(())
}

/// Opens a file in Clean and loads its saved steps.
pub fn clean_open(shell: &Shell, project: &ProjectId, file: &FileId) -> AppResult<CleanSession> {
    let project = shell.project(project)?;
    let file_name = project.display_name(file)?;
    let source = read_table(&project, file)?;
    let steps = match project.load_clean_steps(file)? {
        None => StepList::new(),
        Some(text) => match StepList::from_json(&text) {
            Ok(list) => list,
            Err(StepListError::Unreadable(_)) => {
                // The next change writes over the saved text, so a copy is kept first.
                project.store_blob(file, UNREADABLE_LABEL, text.as_bytes())?;
                StepList::new()
            }
            Err(e) => return Err(steps_failure(&e)),
        },
    };
    let view = compute_or_block(&source, &steps);
    let session = Session {
        project,
        file: file.clone(),
        file_name,
        source,
        steps,
        fixes: Vec::new(),
        view,
        profile: None,
    };
    let state = session.state();
    let created = CleanSession {
        file_name: session.file_name.clone(),
        row_count: state.row_count,
        columns: state.columns.clone(),
        state,
        session_id: String::new(),
    };
    let session_id = shell.add_session(session)?;
    Ok(CleanSession {
        session_id,
        ..created
    })
}

/// Closes a session and frees its table.
pub fn clean_close(shell: &Shell, session: &str) -> AppResult<()> {
    shell.remove_session(session)
}

fn with_session<T>(
    shell: &Shell,
    id: &str,
    run: impl FnOnce(&mut Session) -> AppResult<T>,
) -> AppResult<T> {
    let shared = shell.session(id)?;
    let mut session = lock_session(&shared)?;
    run(&mut session)
}

/// The profile of the table after the enabled steps.
pub fn clean_profile(shell: &Shell, id: &str) -> AppResult<Profile> {
    with_session(shell, id, |s| {
        if s.profile.is_none() {
            s.profile = Some(kh_clean::profile(s.table()));
        }
        s.profile.clone().ok_or_else(AppError::internal)
    })
}

/// Up to 500 rows of the table after the enabled steps.
pub fn clean_window(shell: &Shell, id: &str, start: usize, count: usize) -> AppResult<WindowView> {
    with_session(shell, id, |s| {
        let window = s.table().window(start, count.min(MAX_WINDOW_ROWS));
        Ok(WindowView {
            start: window.start,
            columns: s.table().column_names().to_vec(),
            rows: window.rows,
        })
    })
}

/// Shows a step on a window of the original rows. Nothing is applied.
pub fn clean_preview(
    shell: &Shell,
    id: &str,
    step: &Step,
    start: usize,
    count: usize,
) -> AppResult<Preview> {
    let mut alone = StepList::new();
    alone.push(step.clone());
    within_limits(&alone)?;
    with_session(shell, id, |s| {
        kh_clean::preview(&s.source, &s.steps, step, start, count.min(MAX_WINDOW_ROWS))
            .map_err(|e| AppError::new("step-failed", format!("The step cannot run: {e}.")))
    })
}

/// Groups of spellings that look like the same thing in one column.
pub fn clean_suggest_groups(
    shell: &Shell,
    id: &str,
    column: &str,
) -> AppResult<Vec<SpellingGroup>> {
    with_session(shell, id, |s| {
        let names = s.table().column_names();
        let mut matches = names.iter().enumerate().filter(|(_, n)| *n == column);
        let (index, _) = matches.next().ok_or_else(AppError::not_found)?;
        if matches.next().is_some() {
            return Err(AppError::new(
                "repeated-column",
                "More than one column has that name.",
            ));
        }
        let mut groups = kh_clean::suggest_spelling_groups(s.table(), index);
        groups.truncate(MAX_SPELLING_GROUPS);
        Ok(groups)
    })
}

/// The current step list and result.
pub fn clean_state(shell: &Shell, id: &str) -> AppResult<CleanState> {
    with_session(shell, id, |s| Ok(s.state()))
}

/// Adds a step at the end. A step that cannot run is refused and the list stays as it was.
pub fn clean_add_step(shell: &Shell, id: &str, step: Step) -> AppResult<CleanState> {
    with_session(shell, id, |s| {
        s.change(|list| {
            list.push(step);
            true
        })
    })
}

/// Takes the last step off the list.
pub fn clean_undo(shell: &Shell, id: &str) -> AppResult<CleanState> {
    with_session(shell, id, |s| {
        if s.steps.steps.is_empty() {
            return Ok(s.state());
        }
        s.change(StepList::undo)
    })
}

/// Puts back the step that undo took off.
pub fn clean_redo(shell: &Shell, id: &str) -> AppResult<CleanState> {
    with_session(shell, id, |s| {
        if !s.state().can_redo {
            return Ok(s.state());
        }
        s.change(StepList::redo)
    })
}

/// Moves a step to a new place in the list.
pub fn clean_move_step(shell: &Shell, id: &str, from: usize, to: usize) -> AppResult<CleanState> {
    with_session(shell, id, |s| s.change(|list| list.move_step(from, to)))
}

/// Turns a step on or off.
pub fn clean_set_enabled(
    shell: &Shell,
    id: &str,
    index: usize,
    enabled: bool,
) -> AppResult<CleanState> {
    with_session(shell, id, |s| {
        s.change(|list| list.set_enabled(index, enabled))
    })
}

/// Deletes a step.
pub fn clean_remove_step(shell: &Shell, id: &str, index: usize) -> AppResult<CleanState> {
    with_session(shell, id, |s| s.change(|list| list.remove(index)))
}

/// Replaces one cell with what the person typed. The change is lost when the steps change.
pub fn clean_fix_cell(
    shell: &Shell,
    id: &str,
    row: usize,
    column: usize,
    value: &str,
) -> AppResult<CleanState> {
    with_session(shell, id, |s| {
        s.apply_fix(row, column, value)?;
        Ok(s.state())
    })
}

/// Writes the result beside the original as a new CSV. The original is never opened for writing.
pub fn clean_save(shell: &Shell, id: &str) -> AppResult<SavedFile> {
    with_session(shell, id, |s| {
        if matches!(s.view, View::Blocked(_)) {
            return Err(AppError::new(
                "blocked",
                "The steps cannot run on this file, so nothing was saved.",
            ));
        }
        let table = s.table();
        let saved = s.project.save_beside(&s.file, SAVE_LABEL, |file| {
            let mut out = BufWriter::new(file);
            kh_clean::save_csv(table, &mut out).map_err(io::Error::other)?;
            out.into_inner().map_err(io::Error::from)?;
            Ok(())
        })?;
        Ok(saved)
    })
}

/// Uses the steps saved for another file of the project. When this file lacks a column they need,
/// nothing changes and the report says which.
pub fn clean_reuse(shell: &Shell, id: &str, from: &FileId) -> AppResult<ReuseResult> {
    with_session(shell, id, |s| {
        let text = s
            .project
            .load_clean_steps(from)?
            .ok_or_else(|| AppError::new("no-steps", "That file has no saved steps to use."))?;
        let list = StepList::from_json(&text).map_err(|e| steps_failure(&e))?;
        let report = kh_clean::match_report(&s.source, &list);
        if !report.missing().is_empty() {
            return Ok(ReuseResult {
                report,
                state: None,
            });
        }
        let state = s.change(|current| {
            *current = list;
            true
        })?;
        Ok(ReuseResult {
            report,
            state: Some(state),
        })
    })
}

#[cfg(test)]
mod tests;
