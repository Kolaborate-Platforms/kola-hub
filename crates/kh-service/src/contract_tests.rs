//! Writes real command results as TypeScript, so the compiler can compare them with `ui/src/types.ts`.
//! `scripts/check-contract.sh` runs this with `KH_CONTRACT_OUT` set and then type checks the file.

use std::fmt::Write as _;
use std::fs;

use kh_clean::{SpellingGroup, Step, TargetType};
use serde::Serialize;

use crate::projects::{self, tests::shell_with_project};
use crate::{backup, clean};

const SOURCE: &str = "date,market,price\n12/11/2026,Central ,\"UGX 1,000\"\n14-Nov,central,\n\
12 Nov 2026,Central,500\n12 Nov 2026,Central,500\n13/11/2026,North,abc\n";
const PASSPHRASE: &str = "invented passphrase";

struct Samples {
    text: String,
    types: Vec<&'static str>,
}

impl Samples {
    fn add(&mut self, name: &str, ty: &'static str, value: &impl Serialize) {
        let json = serde_json::to_string(value).expect("json");
        writeln!(self.text, "export const {name}: {ty} = {json};").expect("write");
        let base = ty.trim_end_matches("[]");
        if !self.types.contains(&base) {
            self.types.push(base);
        }
    }

    fn finish(self) -> String {
        format!(
            "import type {{ {} }} from \"./src/types.js\";\n{}",
            self.types.join(", "),
            self.text
        )
    }
}

fn every_step() -> Vec<Step> {
    let column = || "market".to_owned();
    vec![
        Step::TrimSpaces { columns: None },
        Step::MakeDates {
            column: "date".to_owned(),
            day_first: true,
        },
        Step::TextToNumbers {
            column: "price".to_owned(),
        },
        Step::MergeSpellings {
            column: column(),
            groups: vec![SpellingGroup {
                canonical: "Central".to_owned(),
                variants: vec!["central".to_owned()],
            }],
        },
        Step::RemoveDuplicateRows,
        Step::RemoveEmptyRows,
        Step::RenameColumn {
            from: column(),
            to: "place".to_owned(),
        },
        Step::ChangeType {
            column: "price".to_owned(),
            to: TargetType::Number,
        },
    ]
}

fn project_samples(out: &mut Samples) {
    let (data, folder, shell, view) = shell_with_project();
    fs::write(folder.path().join("sales.csv"), SOURCE).expect("csv");
    fs::write(folder.path().join("other.csv"), "a\n1\n").expect("csv");
    fs::write(folder.path().join("notes.txt"), "n").expect("txt");
    out.add("appInfo", "AppInfo", &projects::app_info());
    out.add(
        "settings",
        "Settings",
        &projects::settings_get(&shell).expect("settings"),
    );
    out.add("project", "ProjectView", &view);
    out.add(
        "recent",
        "RecentProject[]",
        &projects::projects_recent(&shell).expect("recent"),
    );
    let files = projects::files_list(&shell, &view.id).expect("files");
    out.add("files", "FileRow[]", &files);
    let id_of = |name: &str| {
        let row = files.iter().find(|f| f.name == name).expect("listed");
        row.id.clone()
    };
    projects::file_trash(&shell, &view.id, &id_of("notes.txt")).expect("trash");
    out.add(
        "trash",
        "TrashItem[]",
        &projects::trash_list(&shell, &view.id).expect("list"),
    );

    let archive = data.path().join("a.khbackup");
    let mut progress = Vec::new();
    let ids = std::slice::from_ref(&view.id);
    let created =
        backup::backup_create(&shell, ids, PASSPHRASE, &archive, &mut |p| progress.push(p))
            .expect("create");
    out.add("created", "CreateReport", &created);
    out.add("progress", "BackupProgress[]", &progress);
    let restored =
        backup::backup_restore(&archive, &data.path().join("out"), PASSPHRASE, &mut |_| {})
            .expect("restore");
    out.add("restored", "RestoreReport", &restored);
    let tested = backup::backup_test(&shell, &archive, PASSPHRASE).expect("test");
    out.add("tested", "TestReport", &tested);

    clean_samples(
        out,
        &shell,
        &view.id,
        &id_of("sales.csv"),
        &id_of("other.csv"),
    );
}

fn clean_samples(
    out: &mut Samples,
    shell: &crate::Shell,
    project: &kh_store::ProjectId,
    sales: &kh_store::FileId,
    other: &kh_store::FileId,
) {
    let session = clean::clean_open(shell, project, sales).expect("open");
    let id = session.session_id.clone();
    out.add("session", "CleanSession", &session);
    out.add(
        "profile",
        "Profile",
        &clean::clean_profile(shell, &id).expect("profile"),
    );
    out.add(
        "window",
        "CleanWindow",
        &clean::clean_window(shell, &id, 0, 10).expect("window"),
    );
    let groups = clean::clean_suggest_groups(shell, &id, "market").expect("groups");
    out.add("groups", "SpellingGroup[]", &groups);
    let steps = every_step();
    out.add("steps", "Step[]", &steps);
    let preview = clean::clean_preview(shell, &id, &steps[2], 0, 10).expect("preview");
    out.add("preview", "Preview", &preview);
    for step in steps.into_iter().take(3) {
        clean::clean_add_step(shell, &id, step).expect("step");
    }
    out.add(
        "state",
        "CleanState",
        &clean::clean_state(shell, &id).expect("state"),
    );
    out.add(
        "saved",
        "SavedFile",
        &clean::clean_save(shell, &id).expect("save"),
    );

    let second = clean::clean_open(shell, project, other).expect("other");
    let reuse = clean::clean_reuse(shell, &second.session_id, sales).expect("reuse");
    out.add("reuse", "ReuseResult", &reuse);
    let path = shell.root_of(project).expect("root").join("sales.csv");
    fs::write(path, "a\n1\n").expect("change columns");
    out.add(
        "blocked",
        "CleanSession",
        &clean::clean_open(shell, project, sales).expect("blocked"),
    );
}

#[test]
fn command_results_can_be_written_as_typescript() {
    let mut out = Samples {
        text: String::new(),
        types: Vec::new(),
    };
    project_samples(&mut out);
    let text = out.finish();
    assert!(text.contains("export const blocked"));
    if let Some(path) = std::env::var_os("KH_CONTRACT_OUT") {
        fs::write(path, text).expect("write contract samples");
    }
}
