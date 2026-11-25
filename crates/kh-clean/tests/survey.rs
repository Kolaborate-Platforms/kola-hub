//! Acceptance tests on the seeded survey fixture. The generator decides every number checked here.
#![allow(missing_docs, clippy::unwrap_used)]

mod support;

use kh_clean::*;
use kh_core::Table;
use kh_formats::{CsvLimits, read_csv};
use support::{WEEK3, WEEK4, generate};

fn load(csv: &str) -> Table {
    read_csv(csv.as_bytes(), b',', &CsvLimits::default())
        .unwrap()
        .0
}

fn rows(t: &Table) -> Vec<Vec<String>> {
    t.window(0, t.row_count()).rows
}

fn saved_steps(week3: &Table) -> StepList {
    let market = week3
        .column_names()
        .iter()
        .position(|n| n == "market")
        .unwrap();
    let mut list = StepList::new();
    list.push(Step::TrimSpaces { columns: None });
    list.push(Step::MakeDates {
        column: "date".into(),
        day_first: true,
    });
    list.push(Step::TextToNumbers {
        column: "price".into(),
    });
    list.push(Step::MergeSpellings {
        column: "market".into(),
        groups: suggest_spelling_groups(week3, market),
    });
    list.push(Step::RemoveDuplicateRows);
    list
}

#[test]
fn committed_fixtures_match_the_generator() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/");
    for (file, spec) in [
        ("kisenyi_survey.csv", WEEK3),
        ("kisenyi_survey_week4.csv", WEEK4),
    ] {
        let on_disk = std::fs::read_to_string(format!("{dir}{file}")).unwrap();
        assert_eq!(
            on_disk,
            generate(spec).csv,
            "{file} is stale: run the gen_fixture example"
        );
    }
}

#[test]
fn profile_reports_the_counts_the_fixture_was_built_with() {
    let fx = generate(WEEK3);
    assert_eq!((fx.rows_in, fx.duplicate_rows), (412, 12));
    assert_eq!(fx.market_spellings_seen, 7);
    let p = profile(&load(&fx.csv));
    assert_eq!(p.rows, 412);
    assert_eq!(p.duplicate_rows, 12);
    assert_eq!(p.wrong_cells, fx.wrong_cells);

    let by_name = |n: &str| p.columns.iter().find(|c| c.name == n).unwrap();
    assert_eq!(by_name("date").detected_type, DetectedType::Date);
    assert_eq!(by_name("price").detected_type, DetectedType::Number);
    assert_eq!(by_name("market").detected_type, DetectedType::Text);
    assert_eq!(by_name("price").empty, 9);
    assert_eq!(by_name("market").distinct, 7 + 2);

    let count = |kind: IssueKind, col: Option<&str>| {
        p.issues
            .iter()
            .find(|i| i.kind == kind && i.column.as_deref() == col)
            .map_or(0, |i| i.count)
    };
    assert_eq!(count(IssueKind::UnreadableDates, Some("date")), 5);
    assert_eq!(count(IssueKind::EmptyCells, Some("price")), 9);
    assert_eq!(count(IssueKind::DuplicateRows, None), 12);
    assert_eq!(count(IssueKind::UnreadableNumbers, Some("price")), 0);
    let spaces: usize = p
        .issues
        .iter()
        .filter(|i| i.kind == IssueKind::ExtraSpaces)
        .map(|i| i.count)
        .sum();
    assert_eq!(spaces, fx.space_cells);
    assert!(p.issues.windows(2).all(|w| w[0].count >= w[1].count));
}

#[test]
fn spelling_suggestions_find_the_seven_kisenyi_spellings_and_leave_other_markets() {
    let t = load(&generate(WEEK3).csv);
    let groups = suggest_spelling_groups(&t, 2);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].canonical, "Kisenyi");
    let mut got = groups[0].variants.clone();
    got.sort();
    let mut want: Vec<String> = support::MARKET_SPELLINGS
        .iter()
        .map(|s| s.to_string())
        .collect();
    want.sort();
    assert_eq!(got, want);
}

#[test]
fn replaying_the_saved_steps_on_week_three_and_week_four_gives_the_expected_output() {
    let week3 = generate(WEEK3);
    let t3 = load(&week3.csv);
    let list = saved_steps(&t3);

    let r3 = replay(&t3, &list).unwrap();
    assert_eq!(r3.summary.rows_in, 412);
    assert_eq!(r3.summary.rows_out, 400);
    assert_eq!(r3.summary.rows_needing_person, week3.rows_needing_person);
    assert_eq!(rows(&r3.table), week3.expected);
    assert_eq!(r3.needs_person.len(), 9 + 5);
    assert!(
        r3.needs_person
            .iter()
            .any(|n| n.value.ends_with("-Nov") && n.reason == Reason::UnreadableDate)
    );

    // The steps are saved as JSON and loaded again before the replay, as the project database would.
    let saved = StepList::from_json(&list.to_json()).unwrap();
    let week4 = generate(WEEK4);
    let r4 = replay(&load(&week4.csv), &saved).unwrap();
    assert_eq!(r4.report.missing(), Vec::<&str>::new());
    assert_eq!(r4.summary.rows_in, week4.rows_in);
    assert_eq!(r4.summary.rows_out, 300);
    assert_eq!(r4.summary.rows_needing_person, 6 + 4);
    assert_eq!(rows(&r4.table), week4.expected);
    assert!(r4.needs_person.iter().all(|n| n.row < 300));
}

#[test]
fn unreadable_dates_are_reported_and_never_guessed() {
    let r = replay(
        &load(&generate(WEEK4).csv),
        &saved_steps(&load(&generate(WEEK3).csv)),
    )
    .unwrap();
    let bad: Vec<&NeedsPerson> = r
        .needs_person
        .iter()
        .filter(|n| n.reason == Reason::UnreadableDate)
        .collect();
    assert_eq!(bad.len(), 4);
    for n in bad {
        assert!(n.value.ends_with("-Nov"));
        assert_eq!(r.table.cell(n.row, n.column), Some(n.value.as_str()));
    }
}

#[test]
fn replay_stops_and_names_missing_columns() {
    let t = load("id,date,cost\n1,2026-11-12,5\n");
    let list = saved_steps(&load(&generate(WEEK3).csv));
    match replay(&t, &list) {
        Err(ReplayError::MissingColumns(report)) => {
            assert_eq!(report.missing(), ["price", "market"]);
            assert!(
                ReplayError::MissingColumns(report)
                    .to_string()
                    .contains("price, market")
            );
        }
        other => panic!("expected missing columns, got {other:?}"),
    }
}

#[test]
fn a_rename_earlier_in_the_list_satisfies_a_later_step() {
    let t = load("cost\n\"1,200\"\n");
    let mut list = StepList::new();
    list.push(Step::RenameColumn {
        from: "cost".into(),
        to: "price".into(),
    });
    list.push(Step::TextToNumbers {
        column: "price".into(),
    });
    let r = replay(&t, &list).unwrap();
    assert_eq!(r.table.column_names(), ["price"]);
    assert_eq!(r.table.cell(0, 0), Some("1200"));
}

#[test]
fn disabled_steps_do_nothing_and_undo_removes_the_last_step() {
    let t = load(&generate(WEEK3).csv);
    let mut list = saved_steps(&t);
    list.set_enabled(4, false);
    assert_eq!(replay(&t, &list).unwrap().summary.rows_out, 412);
    list.set_enabled(4, true);
    assert!(list.undo());
    assert_eq!(replay(&t, &list).unwrap().summary.rows_out, 412);
    assert!(list.redo());
    assert_eq!(replay(&t, &list).unwrap().summary.rows_out, 400);
}

#[test]
fn preview_shows_old_and_new_and_marks_cells_that_need_a_person() {
    let t = load("price\n\"UGX 1,500\"\n\"\"\n900\n");
    let step = Step::TextToNumbers {
        column: "price".into(),
    };
    let p = preview(&t, &StepList::new(), &step, 0, 100).unwrap();
    let cells: Vec<&PreviewCell> = p.rows.iter().map(|r| &r.cells[0]).collect();
    assert_eq!(
        (
            cells[0].old.as_str(),
            cells[0].new.as_str(),
            cells[0].changed
        ),
        ("UGX 1,500", "1500", true)
    );
    assert!(cells[1].needs_person && !cells[1].changed);
    assert!(!cells[2].changed && !cells[2].needs_person);
    assert_eq!(t.cell(0, 0), Some("UGX 1,500"), "preview changes nothing");

    let dup = load("a\nx\nx\ny\n");
    let p = preview(&dup, &StepList::new(), &Step::RemoveDuplicateRows, 0, 100).unwrap();
    assert_eq!(
        p.rows.iter().map(|r| r.removed).collect::<Vec<_>>(),
        [false, true, false]
    );
}

#[test]
fn inline_fix_and_save_neutralise_formulas() {
    let r = replay(&load("price\n\"\"\n"), &{
        let mut l = StepList::new();
        l.push(Step::TextToNumbers {
            column: "price".into(),
        });
        l
    })
    .unwrap();
    assert_eq!(r.needs_person[0].reason, Reason::EmptyNumber);
    let fixed = fix_cell(&r.table, 0, 0, "=1+1").unwrap();
    assert!(fix_cell(&r.table, 5, 0, "x").is_none());
    let mut out = Vec::new();
    save_csv(&fixed, &mut out).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "price\n'=1+1\n");
}

#[test]
fn rename_to_an_existing_name_and_unknown_columns_are_refused() {
    let t = load("a,b\n1,2\n");
    let mut list = StepList::new();
    list.push(Step::RenameColumn {
        from: "a".into(),
        to: "b".into(),
    });
    assert!(matches!(
        replay(&t, &list),
        Err(ReplayError::Step(StepError::DuplicateName(_)))
    ));
    let mut list = StepList::new();
    list.push(Step::ChangeType {
        column: "zzz".into(),
        to: TargetType::Number,
    });
    assert!(matches!(
        replay(&t, &list),
        Err(ReplayError::MissingColumns(_))
    ));
}

#[test]
fn remove_empty_rows_and_change_type() {
    let t = load("a,b\n1, UGX 5\n , \n3,x\n");
    let mut list = StepList::new();
    list.push(Step::RemoveEmptyRows);
    list.push(Step::ChangeType {
        column: "b".into(),
        to: TargetType::Number,
    });
    let r = replay(&t, &list).unwrap();
    assert_eq!(r.summary.rows_out, 2);
    assert_eq!(r.table.cell(0, 1), Some("5"));
    assert_eq!(r.needs_person.len(), 1);
    assert_eq!(r.needs_person[0].reason, Reason::UnreadableNumber);
}

#[test]
fn a_file_with_a_repeated_column_name_stops_the_replay() {
    let mut b = kh_core::TableBuilder::new(&["price", "price"]).unwrap();
    b.push_row(&["UGX 1,000", "UGX 2,000"]).unwrap();
    let mut list = StepList::default();
    list.push(Step::TextToNumbers {
        column: "price".to_owned(),
    });
    match replay(&b.finish(), &list) {
        Err(ReplayError::MissingColumns(report)) => assert!(report.columns[0].repeated),
        other => panic!("expected a stop, got {other:?}"),
    }
}
