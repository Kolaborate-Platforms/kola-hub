//! Bench mode: the seven commands the benchmark harness uses, unchanged. See `docs/contracts/bench-mode.md`.

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

pub struct BenchState {
    pub started_at: Instant,
    pub table: Option<Table>,
    pub load_error: Option<String>,
    pub cell_0_0_override: Mutex<Option<String>>,
    pub bench_out: Option<PathBuf>,
    pub bench_lock: Mutex<()>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableMeta {
    pub columns: Vec<String>,
    pub row_count: usize,
    pub bench_mode: bool,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct WindowPayload {
    pub start: usize,
    pub rows: Vec<Vec<String>>,
}

pub fn load_table(path: &Path) -> Result<Table, String> {
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

pub fn append_bench_line(path: &Path, line: &str) -> Result<(), String> {
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

fn write_bench_event(state: &BenchState, value: serde_json::Value) -> Result<(), String> {
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
pub fn get_meta(state: State<'_, BenchState>) -> TableMeta {
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
pub fn get_window(state: State<'_, BenchState>, start: usize, count: usize) -> WindowPayload {
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
pub fn set_cell_0_0(state: State<'_, BenchState>, value: String) -> Result<(), String> {
    let mut guard = state
        .cell_0_0_override
        .lock()
        .map_err(|_| "Internal lock error.".to_owned())?;
    *guard = Some(value);
    Ok(())
}

#[tauri::command]
pub fn bench_ready(state: State<'_, BenchState>) -> Result<(), String> {
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
pub fn bench_scroll(
    state: State<'_, BenchState>,
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
pub fn bench_edit(app: AppHandle, state: State<'_, BenchState>, ms: f64) -> Result<(), String> {
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
pub fn bench_error(
    app: AppHandle,
    state: State<'_, BenchState>,
    message: String,
) -> Result<(), String> {
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

pub fn non_empty_env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}
