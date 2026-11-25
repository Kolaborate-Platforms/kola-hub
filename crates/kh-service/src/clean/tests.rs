use std::fs;

use kh_store::{FileId, ProjectId};

use super::*;
use crate::projects::{files_list, tests::shell_with_project};

const SOURCE: &str = "name,amount\n  Ann ,\"UGX 1,000\"\nBob,500\n  Ann ,\"UGX 1,000\"\n";

struct Setup {
    _data: tempfile::TempDir,
    folder: tempfile::TempDir,
    shell: Shell,
    project: ProjectId,
    file: FileId,
}

fn setup() -> Setup {
    let (data, folder, shell, view) = shell_with_project();
    fs::write(folder.path().join("sales.csv"), SOURCE).expect("csv");
    let file = files_list(&shell, &view.id).expect("files")[0].id.clone();
    Setup {
        _data: data,
        folder,
        shell,
        project: view.id,
        file,
    }
}

fn open(s: &Setup) -> CleanSession {
    clean_open(&s.shell, &s.project, &s.file).expect("open")
}

fn trim() -> Step {
    Step::TrimSpaces { columns: None }
}

fn json_of(value: &impl Serialize) -> String {
    serde_json::to_string(value).expect("json")
}

#[test]
fn opening_gives_a_bounded_window_and_no_paths() {
    let s = setup();
    let session = open(&s);
    assert_eq!(session.file_name, "sales.csv");
    assert_eq!(session.row_count, 3);
    assert_eq!(session.columns, ["name", "amount"]);
    let id = &session.session_id;
    let window = clean_window(&s.shell, id, 1, 10_000).expect("window");
    assert_eq!(window.start, 1);
    assert_eq!(window.rows.len(), 2);
    assert!(
        clean_window(&s.shell, id, 99, 5)
            .expect("past end")
            .rows
            .is_empty()
    );
    let profile = clean_profile(&s.shell, id).expect("profile");
    for text in [json_of(&session), json_of(&window), json_of(&profile)] {
        assert!(
            !text.contains(&*s.folder.path().to_string_lossy()),
            "{text}"
        );
    }
}

#[test]
fn steps_change_the_table_are_saved_and_come_back_on_the_next_open() {
    let s = setup();
    let id = open(&s).session_id;
    let state = clean_add_step(&s.shell, &id, trim()).expect("trim");
    assert_eq!(state.steps.len(), 1);
    let dup = clean_add_step(&s.shell, &id, Step::RemoveDuplicateRows).expect("dups");
    assert_eq!(
        (dup.row_count, dup.summary.rows_in, dup.summary.rows_out),
        (2, 3, 2)
    );
    assert_eq!(
        clean_window(&s.shell, &id, 0, 5).expect("w").rows[0][0],
        "Ann"
    );

    let undone = clean_undo(&s.shell, &id).expect("undo");
    assert!(undone.can_redo && undone.row_count == 3);
    let redone = clean_redo(&s.shell, &id).expect("redo");
    assert_eq!(redone.row_count, 2);
    let off = clean_set_enabled(&s.shell, &id, 1, false).expect("off");
    assert_eq!(off.row_count, 3);
    let moved = clean_move_step(&s.shell, &id, 0, 1).expect("move");
    assert_eq!(moved.steps[1].step, trim());
    let removed = clean_remove_step(&s.shell, &id, 0).expect("remove");
    assert_eq!(removed.steps.len(), 1);
    assert!(clean_remove_step(&s.shell, &id, 7).is_err());
    clean_close(&s.shell, &id).expect("close");

    let again = open(&s);
    assert_eq!(again.state.steps.len(), 1);
    assert_eq!(again.state.steps[0].step, trim());
}

#[test]
fn a_step_that_cannot_run_is_refused_and_the_list_stays() {
    let s = setup();
    let id = open(&s).session_id;
    clean_add_step(&s.shell, &id, trim()).expect("trim");
    let bad = Step::MakeDates {
        column: "nope".into(),
        day_first: true,
    };
    let error = clean_add_step(&s.shell, &id, bad).expect_err("refused");
    assert_eq!(error.code, "missing-columns");
    let state = clean_state(&s.shell, &id).expect("state");
    assert_eq!(state.steps.len(), 1);
    assert!(state.blocked.is_none());
    assert_eq!(clean_undo(&s.shell, &id).expect("undo").steps.len(), 0);
    assert_eq!(
        clean_undo(&s.shell, &id).expect("nothing left").steps.len(),
        0
    );
}

#[test]
fn fixing_a_cell_and_saving_writes_a_new_file_and_leaves_the_original() {
    let s = setup();
    let id = open(&s).session_id;
    clean_add_step(
        &s.shell,
        &id,
        Step::TextToNumbers {
            column: "amount".into(),
        },
    )
    .expect("numbers");
    let state = clean_state(&s.shell, &id).expect("state");
    assert_eq!(state.summary.rows_needing_person, 0);
    let fixed = clean_fix_cell(&s.shell, &id, 1, 0, "Bobby").expect("fix");
    assert_eq!(fixed.row_count, 3);
    assert!(clean_fix_cell(&s.shell, &id, 99, 0, "x").is_err());
    let saved = clean_save(&s.shell, &id).expect("save");
    assert_eq!(saved.name, "sales (cleaned).csv");
    let text = fs::read_to_string(s.folder.path().join(&saved.name)).expect("saved");
    assert!(
        text.contains("Bobby,500") && text.contains("1000"),
        "{text}"
    );
    assert_eq!(
        fs::read_to_string(s.folder.path().join("sales.csv")).expect("orig"),
        SOURCE
    );
    let second = clean_save(&s.shell, &id).expect("again");
    assert_eq!(second.name, "sales (cleaned 2).csv");
    assert!(!json_of(&saved).contains(&*s.folder.path().to_string_lossy()));
}

#[test]
fn needs_person_cells_drop_out_when_fixed() {
    let s = setup();
    fs::write(
        s.folder.path().join("sales.csv"),
        "name,amount\nAnn,abc\nBob,5\n",
    )
    .expect("csv");
    let id = open(&s).session_id;
    let state = clean_add_step(
        &s.shell,
        &id,
        Step::TextToNumbers {
            column: "amount".into(),
        },
    )
    .expect("numbers");
    assert_eq!(state.needs_person.len(), 1);
    assert_eq!(state.summary.rows_needing_person, 1);
    let fixed = clean_fix_cell(&s.shell, &id, 0, 1, "7").expect("fix");
    assert!(fixed.needs_person.is_empty());
    assert_eq!(fixed.summary.rows_needing_person, 0);
}

#[test]
fn reuse_runs_when_columns_match_and_reports_when_they_do_not() {
    let s = setup();
    let first = open(&s).session_id;
    clean_add_step(&s.shell, &first, trim()).expect("trim");
    clean_add_step(
        &s.shell,
        &first,
        Step::TextToNumbers {
            column: "amount".into(),
        },
    )
    .expect("numbers");
    clean_close(&s.shell, &first).expect("close");

    fs::write(
        s.folder.path().join("june.csv"),
        "name,amount\n x ,\"UGX 2\"\n",
    )
    .expect("june");
    fs::write(s.folder.path().join("odd.csv"), "name,total\nx,1\n").expect("odd");
    let rows = files_list(&s.shell, &s.project).expect("files");
    let id_of = |name: &str| rows.iter().find(|r| r.name == name).expect(name).id.clone();

    let june = clean_open(&s.shell, &s.project, &id_of("june.csv"))
        .expect("june")
        .session_id;
    let reused = clean_reuse(&s.shell, &june, &s.file).expect("reuse");
    let state = reused.state.expect("ran");
    assert!(reused.report.missing().is_empty());
    assert_eq!(
        clean_window(&s.shell, &june, 0, 5).expect("w").rows[0],
        ["x", "2"]
    );
    assert_eq!(state.steps.len(), 2);

    let odd = clean_open(&s.shell, &s.project, &id_of("odd.csv"))
        .expect("odd")
        .session_id;
    let refused = clean_reuse(&s.shell, &odd, &s.file).expect("report");
    assert!(refused.state.is_none());
    assert_eq!(refused.report.missing(), ["amount"]);
    assert!(clean_state(&s.shell, &odd).expect("state").steps.is_empty());

    let none = clean_reuse(&s.shell, &odd, &id_of("odd.csv")).expect_err("no steps");
    assert_eq!(none.code, "no-steps");
}

#[test]
fn saved_steps_that_no_longer_fit_the_file_open_blocked_and_cannot_be_saved() {
    let s = setup();
    let id = open(&s).session_id;
    clean_add_step(
        &s.shell,
        &id,
        Step::TextToNumbers {
            column: "amount".into(),
        },
    )
    .expect("n");
    clean_close(&s.shell, &id).expect("close");
    fs::write(s.folder.path().join("sales.csv"), "name,total\nAnn,1\n").expect("changed");
    let session = open(&s);
    let blocked = session.state.blocked.expect("blocked");
    assert_eq!(blocked.report.missing(), ["amount"]);
    assert_eq!(session.row_count, 1);
    let id = session.session_id;
    assert_eq!(
        clean_save(&s.shell, &id).expect_err("blocked").code,
        "blocked"
    );
    assert_eq!(
        clean_fix_cell(&s.shell, &id, 0, 0, "x")
            .expect_err("blocked")
            .code,
        "blocked"
    );
    let state = clean_remove_step(&s.shell, &id, 0).expect("remove the step");
    assert!(state.blocked.is_none());
}

#[test]
fn preview_and_spelling_groups_are_bounded_and_check_the_column() {
    let s = setup();
    let id = open(&s).session_id;
    let preview = clean_preview(&s.shell, &id, &trim(), 0, 10_000).expect("preview");
    assert_eq!(preview.rows.len(), 3);
    assert!(clean_suggest_groups(&s.shell, &id, "name").is_ok());
    assert_eq!(
        clean_suggest_groups(&s.shell, &id, "nope")
            .expect_err("column")
            .code,
        "not-found"
    );
    let bad = Step::RenameColumn {
        from: "name".into(),
        to: "amount".into(),
    };
    assert!(clean_preview(&s.shell, &id, &bad, 0, 5).is_err());
}

#[test]
fn ids_from_elsewhere_and_too_many_sessions_are_refused() {
    let s = setup();
    let other = tempfile::tempdir().expect("other");
    fs::write(other.path().join("x.csv"), "a\n1\n").expect("x");
    let view = crate::projects::project_open_folder(&s.shell, other.path()).expect("other");
    let foreign = files_list(&s.shell, &view.id).expect("files")[0].id.clone();
    assert_eq!(
        clean_open(&s.shell, &s.project, &foreign)
            .expect_err("foreign")
            .code,
        "not-found"
    );
    assert_eq!(
        clean_state(&s.shell, "s999").expect_err("unknown").code,
        "not-found"
    );
    let ids: Vec<_> = (0..crate::shell::MAX_SESSIONS)
        .map(|_| open(&s).session_id)
        .collect();
    assert_eq!(
        clean_open(&s.shell, &s.project, &s.file)
            .expect_err("cap")
            .code,
        "too-many-open"
    );
    clean_close(&s.shell, &ids[0]).expect("close");
    clean_close(&s.shell, &ids[0]).expect("closing twice is fine");
    assert_eq!(s.shell.open_sessions(), crate::shell::MAX_SESSIONS - 1);
    open(&s);
}

#[test]
fn files_that_are_not_tables_fail_in_plain_words() {
    let s = setup();
    fs::write(s.folder.path().join("empty.csv"), "").expect("empty");
    let rows = files_list(&s.shell, &s.project).expect("files");
    let id = rows
        .iter()
        .find(|r| r.name == "empty.csv")
        .expect("row")
        .id
        .clone();
    let error = clean_open(&s.shell, &s.project, &id).expect_err("empty");
    assert_eq!(error.code, "cannot-open");
    assert!(!error.message.contains(&*s.folder.path().to_string_lossy()));
}

#[test]
fn an_oversized_step_is_refused_before_it_runs() {
    let s = setup();
    let id = open(&s).session_id;
    clean_add_step(&s.shell, &id, trim()).expect("trim");
    let huge = Step::RenameColumn {
        from: "name".to_owned(),
        to: "x".repeat(MAX_STEPS_BYTES + 1),
    };
    let refused = clean_add_step(&s.shell, &id, huge.clone()).expect_err("too large");
    assert_eq!(refused.code, "steps-too-large");
    assert!(clean_preview(&s.shell, &id, &huge, 0, 5).is_err());
    assert_eq!(clean_state(&s.shell, &id).expect("state").steps.len(), 1);
}

#[test]
fn saved_steps_that_cannot_be_read_are_kept_aside() {
    let s = setup();
    let project = s.shell.project(&s.project).expect("project");
    project
        .save_clean_steps(&s.file, "not a step list")
        .expect("save");
    let session = open(&s);
    assert!(session.state.steps.is_empty());
    let kept = project.blobs_of(&s.file).expect("blobs");
    assert_eq!(kept.len(), 1);
}
