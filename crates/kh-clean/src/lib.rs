//! The Clean tool engine: profile a table, build a list of steps, preview them on a window of rows,
//! replay them on next week's file and save a new CSV. No interface and no file paths.
//!
//! Every step works on text and never changes the file it was read from. Cells that need a person are
//! left as they are and reported.

mod apply;
mod profile;
mod replay;
mod spelling;
mod steps;
mod values;

pub use apply::{Flag, Reason, StepError};
pub use profile::{
    CellRef, ColumnProfile, DetectedType, Issue, IssueKind, MAX_ISSUE_CELLS, Profile,
    duplicate_rows, profile,
};
pub use replay::{
    ColumnMatch, MatchReport, NeedsPerson, Preview, PreviewCell, PreviewRow, ReplayError,
    ReplayResult, Summary, fix_cell, match_report, preview, replay, save_csv,
};
pub use spelling::suggest_spelling_groups;
pub use steps::{
    STEP_LIST_VERSION, SpellingGroup, Step, StepEntry, StepList, StepListError, TargetType,
};
pub use values::{parse_date, parse_number};
