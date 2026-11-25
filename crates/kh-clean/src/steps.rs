//! The step list: typed steps, saved as versioned JSON, with undo and redo along the list.

use serde::{Deserialize, Serialize};

/// Version written into saved step lists. A file with a newer version is refused.
pub const STEP_LIST_VERSION: u32 = 1;

/// The type a column can be changed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TargetType {
    /// Plain text, with spaces trimmed.
    Text,
    /// A number: currency words and thousands commas are removed.
    Number,
    /// A real date, day first.
    Date,
}

/// One group of spellings the person chose to merge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellingGroup {
    /// The spelling every variant becomes.
    pub canonical: String,
    /// The spellings that change. May include the canonical one.
    pub variants: Vec<String>,
}

/// One replayable operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Step {
    /// Remove spaces at the start and end of cells. `columns` of `None` means every column.
    TrimSpaces {
        /// Columns to trim, by name.
        columns: Option<Vec<String>>,
    },
    /// Turn dates in many shapes into `YYYY-MM-DD`. Unreadable dates are left and flagged.
    MakeDates {
        /// Column name.
        column: String,
        /// Read `12/11/2026` as 12 November. The default.
        day_first: bool,
    },
    /// Turn text such as `UGX 3,500` into the number `3500`. Empty and unreadable cells are flagged.
    TextToNumbers {
        /// Column name.
        column: String,
    },
    /// Give each group of spellings one spelling.
    MergeSpellings {
        /// Column name.
        column: String,
        /// The groups the person ticked.
        groups: Vec<SpellingGroup>,
    },
    /// Keep the first of any rows that are exactly the same.
    RemoveDuplicateRows,
    /// Remove rows where every cell is empty.
    RemoveEmptyRows,
    /// Give a column a new name.
    RenameColumn {
        /// Current name.
        from: String,
        /// New name.
        to: String,
    },
    /// Change what a column holds.
    ChangeType {
        /// Column name.
        column: String,
        /// New type.
        to: TargetType,
    },
}

impl Step {
    /// A short plain name for the step list.
    pub fn default_name(&self) -> String {
        match self {
            Step::TrimSpaces { .. } => "Trim spaces".into(),
            Step::MakeDates { column, .. } => format!("Make dates real in {column}"),
            Step::TextToNumbers { column } => format!("Turn {column} into numbers"),
            Step::MergeSpellings { column, .. } => format!("One spelling per name in {column}"),
            Step::RemoveDuplicateRows => "Remove duplicate rows".into(),
            Step::RemoveEmptyRows => "Remove empty rows".into(),
            Step::RenameColumn { from, to } => format!("Rename {from} to {to}"),
            Step::ChangeType { column, .. } => format!("Change type of {column}"),
        }
    }
}

/// A step with its name and an on/off switch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepEntry {
    /// Name shown in the list.
    pub name: String,
    /// Disabled steps stay in the list but do nothing.
    pub enabled: bool,
    /// The operation.
    pub step: Step,
}

/// Why a saved step list could not be loaded.
#[derive(Debug, PartialEq, Eq)]
pub enum StepListError {
    /// The text is not a step list.
    Unreadable(String),
    /// Saved by a newer Kola Hub.
    TooNew(u32),
}

impl std::fmt::Display for StepListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepListError::Unreadable(why) => write!(f, "these steps could not be read: {why}"),
            StepListError::TooNew(_) => {
                write!(
                    f,
                    "these steps were saved by a newer Kola Hub. Update to open them"
                )
            }
        }
    }
}

impl std::error::Error for StepListError {}

/// An ordered list of steps. Undo takes the last step off the list and keeps it for redo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepList {
    version: u32,
    /// The steps, in the order they run.
    pub steps: Vec<StepEntry>,
    #[serde(skip)]
    redo: Vec<StepEntry>,
}

impl Default for StepList {
    fn default() -> Self {
        Self {
            version: STEP_LIST_VERSION,
            steps: Vec::new(),
            redo: Vec::new(),
        }
    }
}

impl StepList {
    /// An empty list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a step at the end. This clears anything that could have been redone.
    pub fn push(&mut self, step: Step) {
        self.redo.clear();
        self.steps.push(StepEntry {
            name: step.default_name(),
            enabled: true,
            step,
        });
    }

    /// Takes the last step off the list. Returns false when the list is empty.
    pub fn undo(&mut self) -> bool {
        match self.steps.pop() {
            Some(entry) => {
                self.redo.push(entry);
                true
            }
            None => false,
        }
    }

    /// Puts back the step that undo took off. Returns false when there is nothing to redo.
    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(entry) => {
                self.steps.push(entry);
                true
            }
            None => false,
        }
    }

    /// Moves a step to a new place. Returns false when an index is out of range.
    pub fn move_step(&mut self, from: usize, to: usize) -> bool {
        if from >= self.steps.len() || to >= self.steps.len() {
            return false;
        }
        self.redo.clear();
        let entry = self.steps.remove(from);
        self.steps.insert(to, entry);
        true
    }

    /// Turns a step on or off. Returns false when the index is out of range.
    pub fn set_enabled(&mut self, index: usize, enabled: bool) -> bool {
        self.redo.clear();
        self.steps
            .get_mut(index)
            .map(|e| e.enabled = enabled)
            .is_some()
    }

    /// Deletes a step. Returns false when the index is out of range.
    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.steps.len() {
            return false;
        }
        self.redo.clear();
        self.steps.remove(index);
        true
    }

    /// The steps that will run.
    pub fn enabled_steps(&self) -> impl Iterator<Item = &Step> {
        self.steps.iter().filter(|e| e.enabled).map(|e| &e.step)
    }

    /// Saves the list as JSON.
    pub fn to_json(&self) -> String {
        // A list of plain strings and numbers always serialises.
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Loads a list saved by [`StepList::to_json`].
    pub fn from_json(text: &str) -> Result<Self, StepListError> {
        let list: StepList =
            serde_json::from_str(text).map_err(|e| StepListError::Unreadable(e.to_string()))?;
        if list.version > STEP_LIST_VERSION {
            return Err(StepListError::TooNew(list.version));
        }
        Ok(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> StepList {
        let mut l = StepList::new();
        l.push(Step::TrimSpaces { columns: None });
        l.push(Step::RemoveDuplicateRows);
        l.push(Step::RemoveEmptyRows);
        l
    }

    #[test]
    fn undo_and_redo_walk_along_the_list() {
        let mut l = list();
        assert!(l.undo() && l.undo());
        assert_eq!(l.steps.len(), 1);
        assert!(l.redo());
        assert_eq!(l.steps[1].step, Step::RemoveDuplicateRows);
        l.push(Step::RemoveEmptyRows);
        assert!(!l.redo(), "a new step clears redo");
        assert!(!StepList::new().undo());
    }

    #[test]
    fn reorder_disable_delete() {
        let mut l = list();
        assert!(l.move_step(2, 0));
        assert_eq!(l.steps[0].step, Step::RemoveEmptyRows);
        assert!(l.set_enabled(0, false));
        assert_eq!(l.enabled_steps().count(), 2);
        assert!(l.remove(1));
        assert!(!l.remove(5) && !l.move_step(0, 9) && !l.set_enabled(9, true));
    }

    #[test]
    fn json_round_trip_and_version_gate() {
        let mut l = list();
        l.push(Step::MakeDates {
            column: "date".into(),
            day_first: true,
        });
        let back = StepList::from_json(&l.to_json()).unwrap();
        assert_eq!(back, l);
        assert!(l.to_json().contains("\"dayFirst\": true"));
        let newer = l.to_json().replace("\"version\": 1", "\"version\": 2");
        assert_eq!(StepList::from_json(&newer), Err(StepListError::TooNew(2)));
        assert!(matches!(
            StepList::from_json("nope"),
            Err(StepListError::Unreadable(_))
        ));
    }
}
