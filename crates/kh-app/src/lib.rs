//! The window shell: forwards commands from the interface to the Rust crates.

#![forbid(unsafe_code)]

mod backup;
mod bench;
mod clean;
mod commands;
mod dialogs;
mod error;
mod projects;
mod shell;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use tauri::Manager;

pub use error::AppError;
pub use shell::Shell;

use bench::{BenchState, append_bench_line, load_table, non_empty_env_path};

fn bench_state(started_at: Instant) -> BenchState {
    let bench_out = non_empty_env_path("KH_BENCH_OUT");
    let csv_result = match non_empty_env_path("KH_CSV") {
        Some(path) => load_table(&path),
        None => Err("No CSV file was set. Provide the KH_CSV environment variable.".to_owned()),
    };

    if let Err(ref msg) = csv_result
        && let Some(ref out_path) = bench_out
    {
        let line = serde_json::json!({
            "event": "error",
            "message": msg,
        })
        .to_string();
        let _ = append_bench_line(out_path, &line);
        std::process::exit(1);
    }

    let (table, load_error) = match csv_result {
        Ok(t) => (Some(t), None),
        Err(e) => (None, Some(e)),
    };

    BenchState {
        started_at,
        table,
        load_error,
        cell_0_0_override: Mutex::new(None),
        bench_out,
        bench_lock: Mutex::new(()),
    }
}

/// Opens the window and runs until it closes. `started_at` is when `main` began, for bench mode.
pub fn run(started_at: Instant) {
    let result = tauri::Builder::default()
        // Used from Rust only. The interface is granted none of its permissions.
        .plugin(tauri_plugin_dialog::init())
        .manage(bench_state(started_at))
        .setup(|app| {
            let shell = Shell::new(app.path().app_data_dir()?)?;
            app.manage(Arc::new(shell));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bench::get_meta,
            bench::get_window,
            bench::set_cell_0_0,
            bench::bench_ready,
            bench::bench_scroll,
            bench::bench_edit,
            bench::bench_error,
            commands::app_info,
            commands::strings,
            commands::settings_get,
            commands::settings_save,
            commands::projects_recent,
            commands::project_pick,
            commands::project_open,
            commands::project_forget,
            commands::files_list,
            commands::file_trash,
            commands::trash_list,
            commands::trash_restore,
            commands::trash_empty,
            commands::backup_create,
            commands::backup_restore,
            commands::backup_test,
            commands::clean_open,
            commands::clean_close,
            commands::clean_profile,
            commands::clean_window,
            commands::clean_preview,
            commands::clean_suggest_groups,
            commands::clean_state,
            commands::clean_add_step,
            commands::clean_undo,
            commands::clean_redo,
            commands::clean_move_step,
            commands::clean_set_enabled,
            commands::clean_remove_step,
            commands::clean_fix_cell,
            commands::clean_save,
            commands::clean_reuse,
        ])
        .run(tauri::generate_context!());

    if let Err(err) = result {
        eprintln!("Failed to run application: {err}");
        std::process::exit(1);
    }
}
