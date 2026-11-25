//! Writes `tests/fixtures/kisenyi_survey.csv` and `tests/fixtures/kisenyi_survey_week4.csv`.
//! Run from the repository root: `cargo run -p kh-clean --example gen_fixture`.
#![allow(missing_docs)]

#[path = "../tests/support/mod.rs"]
mod support;

fn main() -> std::io::Result<()> {
    std::fs::write(
        "tests/fixtures/kisenyi_survey.csv",
        support::generate(support::WEEK3).csv,
    )?;
    std::fs::write(
        "tests/fixtures/kisenyi_survey_week4.csv",
        support::generate(support::WEEK4).csv,
    )
}
