//! Drives the server through in-memory readers and writers, the way an assistant would.

use std::cell::RefCell;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

use kh_clean::Step;
use kh_service::{Shell, clean, projects};
use serde_json::{Value, json};

const SOURCE: &str = "name,amount\n  Ann ,\"UGX 1,000\"\nBob,500\n  Ann ,\"UGX 1,000\"\n";
const OVERSIZE: usize = kh_mcp::MAX_LINE_BYTES + 1;

struct Env {
    data: tempfile::TempDir,
    folder: tempfile::TempDir,
    project: String,
    output: RefCell<String>,
}

fn serve_lines(data_dir: &std::path::Path, input: Vec<u8>) -> String {
    let mut out = Vec::new();
    kh_mcp::serve(Cursor::new(input), &mut out, data_dir).expect("serve");
    String::from_utf8(out).expect("utf8")
}

fn request(id: u32, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn env(access: bool) -> Env {
    let data = tempfile::tempdir().expect("data");
    let folder = tempfile::tempdir().expect("folder");
    fs::write(folder.path().join("sales.csv"), SOURCE).expect("csv");
    let shell = Shell::new(data.path().to_path_buf()).expect("shell");
    let view = projects::project_open_folder(&shell, folder.path()).expect("open");
    let mut settings = projects::settings_get(&shell).expect("settings");
    settings.ai_access = access;
    projects::settings_save(&shell, settings).expect("save");
    Env {
        data,
        folder,
        project: serde_json::to_value(&view.id)
            .expect("id")
            .as_str()
            .expect("text")
            .to_owned(),
        output: RefCell::new(String::new()),
    }
}

impl Env {
    fn send(&self, messages: &[Value]) -> Vec<Value> {
        let mut input = Vec::new();
        for message in messages {
            input.extend(message.to_string().into_bytes());
            input.push(b'\n');
        }
        self.raw(input)
    }

    fn raw(&self, input: Vec<u8>) -> Vec<Value> {
        let text = serve_lines(self.data.path(), input);
        self.output.borrow_mut().push_str(&text);
        text.lines()
            .map(|l| serde_json::from_str(l).expect("every output line is JSON"))
            .collect()
    }

    /// The tool's own JSON and whether it reported a failure.
    fn call(&self, tool: &str, arguments: Value) -> (bool, Value) {
        let params = json!({ "name": tool, "arguments": arguments });
        let reply = self.send(&[request(1, "tools/call", params)]).remove(0);
        let result = &reply["result"];
        let text = result["content"][0]["text"].as_str().expect("text");
        (
            result["isError"].as_bool().expect("flag"),
            serde_json::from_str(text).expect("tool json"),
        )
    }

    fn ok(&self, tool: &str, arguments: Value) -> Value {
        let (failed, value) = self.call(tool, arguments);
        assert!(!failed, "{tool} failed: {value}");
        value
    }

    fn file_id(&self, name: &str) -> String {
        let files = self.ok("list_files", json!({ "projectId": self.project }));
        let row = files
            .as_array()
            .expect("list")
            .iter()
            .find(|f| f["name"] == name)
            .expect("file");
        row["id"].as_str().expect("id").to_owned()
    }

    fn sales(&self) -> Value {
        json!({ "projectId": self.project, "fileId": self.file_id("sales.csv") })
    }

    fn assert_no_paths(&self) {
        let output = self.output.borrow();
        for dir in [self.data.path(), self.folder.path()] {
            let plain = dir.to_string_lossy().into_owned();
            let escaped = json!(plain).to_string();
            assert!(!output.contains(&plain), "a path leaked: {plain}");
            assert!(!output.contains(escaped.trim_matches('"')), "a path leaked");
        }
    }
}

fn trim() -> Value {
    json!({ "type": "trimSpaces" })
}

#[test]
fn the_handshake_lists_every_tool_with_its_hints() {
    let env = env(true);
    let replies = env.send(&[
        request(1, "initialize", json!({ "protocolVersion": "2025-03-26" })),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        request(2, "initialize", json!({ "protocolVersion": "1999-01-01" })),
        request(3, "ping", json!({})),
        request(4, "tools/list", json!({})),
        request(5, "nothing/here", json!({})),
        request(6, "tools/call", json!({ "name": "no_such_tool" })),
    ]);
    assert_eq!(replies.len(), 6, "a notification gets no answer");
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "kola-hub-mcp");
    assert_eq!(replies[1]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(replies[2]["result"], json!({}));
    let tools = replies[3]["result"]["tools"].as_array().expect("tools");
    assert_eq!(tools.len(), 14);
    for tool in tools {
        let hints = &tool["annotations"];
        assert!(hints["readOnlyHint"].is_boolean() && hints["destructiveHint"].is_boolean());
        assert_eq!(tool["inputSchema"]["type"], "object");
    }
    let preview = tools
        .iter()
        .find(|t| t["name"] == "preview_step")
        .expect("tool");
    let shapes = preview["inputSchema"]["properties"]["step"]["oneOf"].as_array();
    assert_eq!(shapes.expect("step shapes").len(), 8);
    assert_eq!(replies[4]["error"]["code"], -32601);
    assert_eq!(replies[5]["error"]["code"], -32602);
}

#[test]
fn with_access_off_every_tool_refuses_and_nothing_is_created() {
    let env = env(false);
    let later = env.data.path().join("never-made");
    let names = [
        "list_projects",
        "list_files",
        "read_rows",
        "profile_file",
        "suggest_spelling_groups",
        "preview_step",
        "list_steps",
        "add_step",
        "remove_step",
        "reuse_steps",
        "save_cleaned_copy",
        "list_trash",
        "move_to_trash",
        "restore_from_trash",
    ];
    for name in names {
        let (failed, value) = env.call(name, json!({}));
        assert!(failed, "{name}");
        assert_eq!(value["code"], "ai-access-off", "{name}");
    }
    let calls = [request(1, "tools/call", json!({ "name": "list_projects" }))];
    let text = serve_lines(&later, format!("{}\n", calls[0]).into_bytes());
    assert!(text.contains("ai-access-off"));
    assert!(!later.exists(), "an off server must not create the folder");
    assert!(fs::read_dir(env.folder.path()).expect("dir").count() <= 2);
}

#[test]
fn every_reading_tool_works_and_the_original_stays_the_same() {
    let env = env(true);
    let before = fs::read(env.folder.path().join("sales.csv")).expect("read");
    let projects = env.ok("list_projects", json!({}));
    assert_eq!(projects[0]["id"], env.project.as_str());
    let base = env.sales();
    let rows = env.ok("read_rows", base.clone());
    assert_eq!(rows["rowCount"], 3);
    assert_eq!(rows["columns"], json!(["name", "amount"]));
    let capped = env.sales().as_object().cloned().expect("object");
    let mut big = capped.clone();
    big.insert("count".into(), json!(5000));
    let window = env.ok("read_rows", Value::Object(big));
    assert!(window["rows"].as_array().expect("rows").len() <= 100);
    assert_eq!(env.ok("profile_file", base.clone())["rows"], 3);
    let mut spelling = capped.clone();
    spelling.insert("column".into(), json!("name"));
    env.ok("suggest_spelling_groups", Value::Object(spelling));
    let mut preview = capped.clone();
    preview.insert("step".into(), trim());
    let shown = env.ok("preview_step", Value::Object(preview));
    assert_eq!(shown["rows"][0]["cells"][0]["new"], "Ann");
    assert_eq!(env.ok("list_steps", base)["steps"], json!([]));
    assert_eq!(
        fs::read(env.folder.path().join("sales.csv")).expect("read"),
        before
    );
    env.assert_no_paths();
}

#[test]
fn steps_change_save_reuse_and_the_original_is_never_touched() {
    let env = env(true);
    let before = fs::read(env.folder.path().join("sales.csv")).expect("read");
    let mut add = env.sales().as_object().cloned().expect("object");
    add.insert("step".into(), trim());
    let state = env.ok("add_step", Value::Object(add.clone()));
    assert_eq!(state["steps"].as_array().expect("steps").len(), 1);
    add.insert("step".into(), json!({ "type": "removeDuplicateRows" }));
    let state = env.ok("add_step", Value::Object(add.clone()));
    assert_eq!(state["rowCount"], 2);

    let saved = env.ok("save_cleaned_copy", env.sales());
    assert_eq!(saved["name"], "sales (cleaned).csv");
    let copy = fs::read_to_string(env.folder.path().join("sales (cleaned).csv")).expect("copy");
    assert!(copy.contains("Ann,") && !copy.contains("  Ann"), "{copy}");
    assert_eq!(
        fs::read(env.folder.path().join("sales.csv")).expect("read"),
        before
    );

    fs::write(env.folder.path().join("june.csv"), "name,amount\n x ,5\n").expect("june");
    let june = json!({ "projectId": env.project, "fileId": env.file_id("june.csv"),
        "fromFileId": env.file_id("sales.csv") });
    let reused = env.ok("reuse_steps", june);
    assert_eq!(reused["state"]["steps"].as_array().expect("steps").len(), 2);

    let mut remove = env.sales().as_object().cloned().expect("object");
    remove.insert("index".into(), json!(0));
    let state = env.ok("remove_step", Value::Object(remove.clone()));
    assert_eq!(state["steps"].as_array().expect("steps").len(), 1);
    remove.insert("index".into(), json!(9));
    let (failed, error) = env.call("remove_step", Value::Object(remove));
    assert!(failed && error["code"] == "bad-step", "{error}");
    let (failed, error) = env.call("add_step", json!({ "projectId": env.project,
        "fileId": env.file_id("sales.csv"), "step": { "type": "renameColumn", "from": "zzz", "to": "y" } }));
    assert!(failed && error["code"] == "missing-columns", "{error}");
    assert_eq!(
        fs::read(env.folder.path().join("sales.csv")).expect("read"),
        before
    );
    env.assert_no_paths();
}

#[test]
fn the_trash_tools_round_trip() {
    let env = env(true);
    let sales = env.sales();
    env.ok("move_to_trash", sales);
    assert!(!env.folder.path().join("sales.csv").exists());
    let items = env.ok("list_trash", json!({ "projectId": env.project }));
    let item = items[0]["id"].as_str().expect("id").to_owned();
    env.ok(
        "restore_from_trash",
        json!({ "projectId": env.project, "trashId": item }),
    );
    assert!(env.folder.path().join("sales.csv").exists());
    env.assert_no_paths();
}

#[test]
fn unknown_projects_and_ids_from_another_project_are_refused() {
    let env = env(true);
    let file = env.file_id("sales.csv");
    let (failed, error) = env.call("list_files", json!({ "projectId": "nope" }));
    assert!(failed && error["code"] == "not-found", "{error}");

    let other = tempfile::tempdir().expect("other");
    let shell = Shell::new(env.data.path().to_path_buf()).expect("shell");
    let view = projects::project_open_folder(&shell, other.path()).expect("open");
    let other_id = serde_json::to_value(&view.id).expect("id");
    let (failed, error) = env.call(
        "read_rows",
        json!({ "projectId": other_id, "fileId": file }),
    );
    assert!(failed && error["code"] == "not-found", "{error}");
    let (failed, error) = env.call(
        "move_to_trash",
        json!({ "projectId": other_id, "fileId": file }),
    );
    assert!(failed && error["code"] == "not-found", "{error}");
    assert!(env.folder.path().join("sales.csv").exists());
    let (failed, error) = env.call("read_rows", json!({ "projectId": env.project }));
    assert!(failed && error["code"] == "bad-arguments", "{error}");
    env.assert_no_paths();
}

#[test]
fn bad_lines_get_errors_and_the_server_keeps_going() {
    let env = env(true);
    let ping = request(7, "ping", json!({})).to_string();
    let mut input = b"{not json\n[1,2]\n\n".to_vec();
    input.extend(vec![b'x'; OVERSIZE]);
    input.push(b'\n');
    input.extend([0xff, 0xfe, b'\n']);
    input.extend(format!("{ping}\n").into_bytes());
    let replies = env.raw(input);
    let codes: Vec<_> = replies.iter().map(|r| r["error"]["code"].clone()).collect();
    assert_eq!(
        codes[..4],
        [json!(-32700), json!(-32600), json!(-32600), json!(-32700)]
    );
    assert_eq!(replies[4]["id"], 7);
    assert_eq!(replies[4]["result"], json!({}));
    assert!(replies.iter().all(|r| r["jsonrpc"] == "2.0"));
}

#[test]
fn a_step_list_saved_by_the_window_since_it_was_opened_is_refused() {
    let env = env(true);
    let shell = Shell::new(env.data.path().to_path_buf()).expect("shell");
    let project: kh_store::ProjectId = serde_json::from_value(json!(env.project)).expect("id");
    let file: kh_store::FileId =
        serde_json::from_value(json!(env.file_id("sales.csv"))).expect("id");
    let window = clean::clean_open(&shell, &project, &file)
        .expect("open")
        .session_id;

    let mut add = env.sales().as_object().cloned().expect("object");
    add.insert("step".into(), trim());
    env.ok("add_step", Value::Object(add));

    let error = clean::clean_add_step(&shell, &window, Step::RemoveEmptyRows).expect_err("stale");
    assert_eq!(error.code, "steps-changed-elsewhere");
    let state = env.ok("list_steps", env.sales());
    assert_eq!(state["steps"].as_array().expect("steps").len(), 1);
}

#[test]
fn the_binary_path_is_a_console_program() {
    let manifest =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/main.rs"));
    assert!(!manifest.expect("main").contains("windows_subsystem"));
}
