//! Each step on 100,000 rows must finish in under a second in a release build.
//! Run: `cargo test -p kh-clean --release --test speed -- --nocapture`
#![allow(missing_docs, clippy::unwrap_used)]

mod support;

use std::time::Instant;

use kh_clean::*;
use kh_formats::{CsvLimits, read_csv};

#[test]
fn every_step_on_100k_rows_runs_under_a_second_in_release() {
    let spec = support::Spec {
        base_rows: 100_000,
        duplicates: 1_000,
        empty_prices: 500,
        unreadable_dates: 500,
        trailing_products: 2_000,
        ..support::WEEK3
    };
    let csv = support::generate(spec).csv;
    let table = read_csv(csv.as_bytes(), b',', &CsvLimits::default())
        .unwrap()
        .0;
    assert_eq!(table.row_count(), 101_000);

    let groups = suggest_spelling_groups(&table, 2);
    let steps = [
        Step::TrimSpaces { columns: None },
        Step::MakeDates {
            column: "date".into(),
            day_first: true,
        },
        Step::TextToNumbers {
            column: "price".into(),
        },
        Step::MergeSpellings {
            column: "market".into(),
            groups: groups.clone(),
        },
        Step::RemoveDuplicateRows,
        Step::RemoveEmptyRows,
        Step::RenameColumn {
            from: "price".into(),
            to: "price_ugx".into(),
        },
        Step::ChangeType {
            column: "price".into(),
            to: TargetType::Number,
        },
    ];
    let mut limit_ok = true;
    let mut report = |name: &str, secs: f64| {
        println!("{name:<24} {:>7.1} ms", secs * 1000.0);
        limit_ok &= secs < 1.0;
    };
    for step in steps {
        let mut list = StepList::new();
        list.push(step.clone());
        let start = Instant::now();
        replay(&table, &list).unwrap();
        report(
            &step.default_name().chars().take(24).collect::<String>(),
            start.elapsed().as_secs_f64(),
        );
    }
    let start = Instant::now();
    profile(&table);
    report("profile", start.elapsed().as_secs_f64());
    let start = Instant::now();
    suggest_spelling_groups(&table, 2);
    report("suggest spellings", start.elapsed().as_secs_f64());
    let start = Instant::now();
    let mut out = Vec::new();
    save_csv(&table, &mut out).unwrap();
    report("save as CSV", start.elapsed().as_secs_f64());

    // Debug builds are far slower, so the limit is only enforced in release.
    assert!(
        limit_ok || cfg!(debug_assertions),
        "a step took a second or more"
    );
}
