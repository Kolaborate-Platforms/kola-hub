//! The window shell: forwards commands from the interface to the Rust crates.

#![forbid(unsafe_code)]

use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use kh_core::Table;
use kh_formats::{CsvLimits, read_csv, sniff_delimiter};
use serde::Serialize;
use tauri::{AppHandle, State};

/// Maximum rows returned in a single window request.
const MAX_WINDOW_ROWS: usize = 500;

struct AppState {
    started_at: Instant,
    table: Option<Table>,
    load_error: Option<String>,
    cell_0_0_override: Mutex<Option<String>>,
    bench_out: Option<PathBuf>,
    bench_lock: Mutex<()>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TableMeta {
    columns: Vec<String>,
    row_count: usize,
    bench_mode: bool,
    error: Option<String>,
}

#[derive(Serialize)]
struct WindowPayload {
    start: usize,
    rows: Vec<Vec<String>>,
}

fn load_table(path: &Path) -> Result<Table, String> {
    let mut file = File::open(path).map_err(|_| "Could not open the CSV file.".to_owned())?;
    let mut sample = [0u8; 8192];
    let n = file
        .read(&mut sample)
        .map_err(|_| "Could not read the CSV file.".to_owned())?;
    let delimiter = sniff_delimiter(&sample[..n]);
    file.seek(SeekFrom::Start(0))
        .map_err(|_| "Could not read the CSV file.".to_owned())?;
    let reader = BufReader::new(file);
    let (table, _report) = read_csv(reader, delimiter, &CsvLimits::default())
        .map_err(|e| format!("Could not read the CSV file: {e}"))?;
    Ok(table)
}

fn append_bench_line(path: &Path, line: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("Failed to open benchmark output: {e}"))?;
    writeln!(file, "{line}").map_err(|e| format!("Failed to write benchmark output: {e}"))?;
    file.flush()
        .map_err(|e| format!("Failed to flush benchmark output: {e}"))?;
    Ok(())
}

fn write_bench_event(state: &AppState, value: serde_json::Value) -> Result<(), String> {
    let Some(path) = &state.bench_out else {
        return Ok(());
    };
    let _guard = state
        .bench_lock
        .lock()
        .map_err(|_| "Internal lock error.".to_owned())?;
    let line = serde_json::to_string(&value)
        .map_err(|e| format!("Failed to encode benchmark event: {e}"))?;
    append_bench_line(path, &line)
}

fn round2(val: f64) -> f64 {
    (val * 100.0).round() / 100.0
}

#[tauri::command]
fn get_meta(state: State<'_, AppState>) -> TableMeta {
    match &state.table {
        Some(table) => TableMeta {
            columns: table.column_names().to_vec(),
            row_count: table.row_count(),
            bench_mode: state.bench_out.is_some(),
            error: None,
        },
        None => TableMeta {
            columns: Vec::new(),
            row_count: 0,
            bench_mode: state.bench_out.is_some(),
            error: state.load_error.clone(),
        },
    }
}

#[tauri::command]
fn get_window(state: State<'_, AppState>, start: usize, count: usize) -> WindowPayload {
    let Some(table) = &state.table else {
        return WindowPayload {
            start: 0,
            rows: Vec::new(),
        };
    };
    let clamped_count = count.min(MAX_WINDOW_ROWS);
    let mut win = table.window(start, clamped_count);
    if win.start == 0
        && let Ok(guard) = state.cell_0_0_override.lock()
        && let Some(edited) = guard.as_ref()
        && let Some(first_row) = win.rows.first_mut()
        && let Some(first_cell) = first_row.first_mut()
    {
        *first_cell = edited.clone();
    }
    WindowPayload {
        start: win.start,
        rows: win.rows,
    }
}

#[tauri::command]
fn set_cell_0_0(state: State<'_, AppState>, value: String) -> Result<(), String> {
    let mut guard = state
        .cell_0_0_override
        .lock()
        .map_err(|_| "Internal lock error.".to_owned())?;
    *guard = Some(value);
    Ok(())
}

#[tauri::command]
fn bench_ready(state: State<'_, AppState>) -> Result<(), String> {
    let ms = round2(state.started_at.elapsed().as_secs_f64() * 1000.0);
    let rows = state.table.as_ref().map_or(0, Table::row_count);
    write_bench_event(
        &state,
        serde_json::json!({
            "event": "ready",
            "ms": ms,
            "rows": rows,
        }),
    )
}

#[tauri::command]
fn bench_scroll(
    state: State<'_, AppState>,
    frames: usize,
    p50_ms: f64,
    p95_ms: f64,
    max_ms: f64,
) -> Result<(), String> {
    write_bench_event(
        &state,
        serde_json::json!({
            "event": "scroll",
            "frames": frames,
            "p50_ms": round2(p50_ms),
            "p95_ms": round2(p95_ms),
            "max_ms": round2(max_ms),
        }),
    )
}

#[tauri::command]
fn bench_edit(app: AppHandle, state: State<'_, AppState>, ms: f64) -> Result<(), String> {
    write_bench_event(
        &state,
        serde_json::json!({
            "event": "edit",
            "ms": round2(ms),
        }),
    )?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn bench_error(app: AppHandle, state: State<'_, AppState>, message: String) -> Result<(), String> {
    let _ = write_bench_event(
        &state,
        serde_json::json!({
            "event": "error",
            "message": message,
        }),
    );
    app.exit(1);
    Ok(())
}

fn non_empty_env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Opens the window and runs until it closes. `started_at` is when `main` began, for bench mode.
pub fn run(started_at: Instant) {
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

    let state = AppState {
        started_at,
        table,
        load_error,
        cell_0_0_override: Mutex::new(None),
        bench_out,
        bench_lock: Mutex::new(()),
    };

    let result = tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_meta,
            get_window,
            set_cell_0_0,
            bench_ready,
            bench_scroll,
            bench_edit,
            bench_error,
        ])
        .run(tauri::generate_context!());

    if let Err(err) = result {
        eprintln!("Failed to run application: {err}");
        std::process::exit(1);
    }
}
