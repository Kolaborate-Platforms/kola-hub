//! Times opening a CSV and scrolling it, and prints peak memory where the system reports it.
//! Usage: cargo run --release -p kh-formats --example bench_csv -- <file.csv>
//! Numbers from a build machine are NOT reference-machine numbers. Say which machine you used.

use std::time::Instant;

use kh_formats::{CsvLimits, read_csv, sniff_delimiter};

fn peak_memory_mb() -> Option<f64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024.0)
}

fn open(path: &str, limits: &CsvLimits) -> std::io::Result<(kh_core::Table, std::time::Duration)> {
    let mut head = vec![0u8; 64 * 1024];
    let n = std::io::Read::read(&mut std::fs::File::open(path)?, &mut head)?;
    let delimiter = sniff_delimiter(&head[..n]);
    let start = Instant::now();
    let file = std::fs::File::open(path)?;
    let (table, _) = read_csv(file, delimiter, limits).map_err(std::io::Error::other)?;
    Ok((table, start.elapsed()))
}

fn main() -> std::io::Result<()> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "big.csv".to_string());
    let size_mb = std::fs::metadata(&path)?.len() as f64 / 1_048_576.0;
    println!("file: {size_mb:.1} MB");

    let first = CsvLimits {
        max_rows: Some(100),
        ..CsvLimits::default()
    };
    let (t, d) = open(&path, &first)?;
    println!(
        "first screen ({} rows): {:.1} ms",
        t.row_count(),
        d.as_secs_f64() * 1000.0
    );

    let (table, d) = open(&path, &CsvLimits::default())?;
    println!(
        "whole file: {} rows x {} cols in {:.2} s",
        table.row_count(),
        table.column_count(),
        d.as_secs_f64()
    );
    println!(
        "table heap: {:.1} MB",
        table.heap_bytes() as f64 / 1_048_576.0
    );
    if let Some(mb) = peak_memory_mb() {
        println!("peak process memory: {mb:.1} MB");
    }

    // Scroll test: ask for a window of 100 rows at many positions, record the slowest.
    let mut worst = 0.0f64;
    let mut pos = 0usize;
    let step = (table.row_count() / 2_000).max(1);
    let start = Instant::now();
    let mut windows = 0u32;
    while pos < table.row_count() {
        let t0 = Instant::now();
        let w = table.window(pos, 100);
        std::hint::black_box(&w);
        worst = worst.max(t0.elapsed().as_secs_f64() * 1000.0);
        pos += step;
        windows += 1;
    }
    println!(
        "{windows} windows of 100 rows: worst {worst:.3} ms, total {:.1} ms",
        start.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}
