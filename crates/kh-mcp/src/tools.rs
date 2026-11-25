//! Runs one tool call over `kh-service` and turns the outcome into a tool result.

use std::path::Path;

use kh_service::clean::{self, CleanSession};
use kh_service::projects;
use kh_service::{AppError, AppResult, Shell};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::args::Args;

fn to_json(value: &impl Serialize) -> AppResult<Value> {
    serde_json::to_value(value).map_err(|_| AppError::internal())
}

fn text_result(value: &Value, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": value.to_string() }], "isError": is_error })
}

/// A failed tool is a normal result the assistant can read, not a protocol error.
fn failed(error: &AppError) -> Value {
    text_result(
        &json!({ "code": error.code, "message": error.message }),
        true,
    )
}

fn access_off() -> AppError {
    AppError::new(
        "ai-access-off",
        "AI access is turned off in Kola Hub. A person can turn it on in Settings.",
    )
}

/// Reads the settings and makes a new shell for this one call.
fn open_shell(data_dir: &Path) -> AppResult<Shell> {
    if !kh_settings::load(data_dir)?.ai_access {
        return Err(access_off());
    }
    Shell::new(data_dir.to_path_buf())
}

pub(crate) fn run(name: &str, arguments: &Map<String, Value>, data_dir: &Path) -> Value {
    let outcome = open_shell(data_dir).and_then(|shell| dispatch(&shell, name, &Args(arguments)));
    match outcome {
        Ok(value) => text_result(&value, false),
        Err(error) => failed(&error),
    }
}

fn dispatch(shell: &Shell, name: &str, args: &Args) -> AppResult<Value> {
    match name {
        "list_projects" => to_json(&projects::projects_recent(shell)?),
        "list_files" => to_json(&projects::files_list(shell, &args.project()?)?),
        "list_trash" => to_json(&projects::trash_list(shell, &args.project()?)?),
        "move_to_trash" => {
            projects::file_trash(shell, &args.project()?, &args.file("fileId")?)?;
            Ok(json!({ "done": true }))
        }
        "restore_from_trash" => {
            projects::trash_restore(shell, &args.project()?, &args.trash()?)?;
            Ok(json!({ "done": true }))
        }
        _ => with_clean_session(shell, args, |session| {
            clean_tool(shell, name, args, session)
        }),
    }
}

/// Opens the file in Clean, runs `work` and always closes the session again.
fn with_clean_session(
    shell: &Shell,
    args: &Args,
    work: impl FnOnce(&CleanSession) -> AppResult<Value>,
) -> AppResult<Value> {
    let session = clean::clean_open(shell, &args.project()?, &args.file("fileId")?)?;
    let outcome = work(&session);
    let closed = clean::clean_close(shell, &session.session_id);
    let value = outcome?;
    closed?;
    Ok(value)
}

fn clean_tool(shell: &Shell, name: &str, args: &Args, session: &CleanSession) -> AppResult<Value> {
    let id = session.session_id.as_str();
    match name {
        "read_rows" => {
            let (start, count) = args.window()?;
            let window = clean::clean_window(shell, id, start, count)?;
            let mut value = to_json(&window)?;
            value["rowCount"] = json!(session.row_count);
            Ok(value)
        }
        "profile_file" => to_json(&clean::clean_profile(shell, id)?),
        "suggest_spelling_groups" => to_json(&clean::clean_suggest_groups(
            shell,
            id,
            &args.text("column")?,
        )?),
        "preview_step" => {
            let (start, count) = args.window()?;
            to_json(&clean::clean_preview(
                shell,
                id,
                &args.step()?,
                start,
                count,
            )?)
        }
        "list_steps" => to_json(&session.state),
        "add_step" => to_json(&clean::clean_add_step(shell, id, args.step()?)?),
        "remove_step" => to_json(&clean::clean_remove_step(shell, id, args.index("index")?)?),
        "reuse_steps" => to_json(&clean::clean_reuse(shell, id, &args.file("fromFileId")?)?),
        "save_cleaned_copy" => to_json(&clean::clean_save(shell, id)?),
        _ => Err(AppError::internal()),
    }
}
