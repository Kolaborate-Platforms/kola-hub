//! Shell spike: a virtualised grid in egui. See `spikes/README.md`.
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::time::Instant;

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use kh_core::{Table, Window};
use kh_formats::{CsvLimits, read_csv, sniff_delimiter};

const ROW_HEIGHT: f32 = 22.0;
const SCROLL_FRAMES: usize = 600;
const ROWS_PER_FRAME: usize = 500;
const FETCH: usize = 64;

/// Edits held in memory, on top of the table. The source file is never written.
#[derive(Default)]
struct Overlay(HashMap<(usize, usize), String>);

impl Overlay {
    fn set(&mut self, row: usize, col: usize, text: String) {
        self.0.insert((row, col), text);
    }
    fn get<'a>(&'a self, row: usize, col: usize, original: &'a str) -> &'a str {
        self.0.get(&(row, col)).map_or(original, String::as_str)
    }
}

/// Percentile by nearest rank, on a slice that need not be sorted. Zero for an empty slice.
fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let rank = (p / 100.0 * v.len() as f64).ceil() as usize;
    v[rank.clamp(1, v.len()) - 1]
}

enum Phase {
    WaitReady,
    Scroll { frame: usize, gaps: Vec<f64> },
    Edit { started: Instant },
    Done,
}

struct Bench {
    out: File,
    phase: Phase,
    last: Option<Instant>,
}

impl Bench {
    fn line(&mut self, text: String) {
        // A failed write cannot be reported anywhere useful in a spike.
        let _ = writeln!(self.out, "{text}");
        let _ = self.out.flush();
    }
}

struct App {
    table: Table,
    overlay: Overlay,
    cache: Window,
    start: Instant,
    jump_last: bool,
    scroll_to: Option<f32>,
    bench: Option<Bench>,
    rows: usize,
}

impl App {
    fn step_bench(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let rows = self.rows;
        let Some(b) = self.bench.as_mut() else { return };
        let gap = b.last.map(|l| now.duration_since(l).as_secs_f64() * 1000.0);
        b.last = Some(now);
        match std::mem::replace(&mut b.phase, Phase::Done) {
            // The first call comes before anything is on screen, so wait for the second frame.
            Phase::WaitReady if gap.is_none() => b.phase = Phase::WaitReady,
            Phase::WaitReady => {
                let ms = self.start.elapsed().as_secs_f64() * 1000.0;
                b.line(format!(r#"{{"event":"ready","ms":{ms:.1},"rows":{rows}}}"#));
                b.phase = Phase::Scroll {
                    frame: 0,
                    gaps: Vec::new(),
                };
            }
            Phase::Scroll { frame, mut gaps } => {
                // The gap seen at frame 0 belongs to the still first screen, not to scrolling.
                if frame > 0 {
                    gaps.extend(gap);
                }
                if frame >= SCROLL_FRAMES {
                    let max = gaps.iter().copied().fold(0.0, f64::max);
                    let line = format!(
                        r#"{{"event":"scroll","frames":{SCROLL_FRAMES},"p50_ms":{:.2},"p95_ms":{:.2},"max_ms":{max:.2}}}"#,
                        percentile(&gaps, 50.0),
                        percentile(&gaps, 95.0)
                    );
                    b.line(line);
                    self.overlay.set(0, 0, "edited".to_owned());
                    self.scroll_to = Some(0.0);
                    b.phase = Phase::Edit {
                        started: Instant::now(),
                    };
                } else {
                    let row = (frame * ROWS_PER_FRAME) % rows.max(1);
                    self.scroll_to = Some(row as f32 * ROW_HEIGHT);
                    b.phase = Phase::Scroll {
                        frame: frame + 1,
                        gaps,
                    };
                }
            }
            Phase::Edit { started } => {
                let ms = started.elapsed().as_secs_f64() * 1000.0;
                b.line(format!(r#"{{"event":"edit","ms":{ms:.2}}}"#));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Phase::Done => {}
        }
        ctx.request_repaint();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // The previous frame has been painted by now, so timings here include its paint.
        self.step_bench(&ctx);
        egui::Panel::top("bar").show(ui, |ui| {
            if ui.button("Go to last row").clicked() {
                self.jump_last = true;
            }
        });
        egui::CentralPanel::default().show(ui, |ui| self.grid(ui));
    }
}

impl App {
    fn grid(&mut self, ui: &mut egui::Ui) {
        let cols = self.table.column_count();
        let rows = self.rows;
        let mut builder = TableBuilder::new(ui)
            .striped(true)
            .max_scroll_height(f32::INFINITY)
            .auto_shrink(false);
        for _ in 0..cols {
            builder = builder.column(Column::initial(110.0).at_least(40.0).clip(true));
        }
        if let Some(y) = self.scroll_to.take() {
            builder = builder.vertical_scroll_offset(y);
        }
        if std::mem::take(&mut self.jump_last) && rows > 0 {
            builder = builder.scroll_to_row(rows - 1, Some(egui::Align::BOTTOM));
        }
        let table = &self.table;
        let overlay = &mut self.overlay;
        let cache = &mut self.cache;
        builder
            .header(ROW_HEIGHT, |mut h| {
                for name in table.column_names() {
                    h.col(|ui| {
                        ui.strong(name);
                    });
                }
            })
            .body(|body| {
                body.rows(ROW_HEIGHT, rows, |mut row| {
                    let r = row.index();
                    if r < cache.start || r >= cache.start + cache.rows.len() {
                        *cache = table.window(r, FETCH);
                    }
                    let cells = &cache.rows[r - cache.start];
                    for (c, text) in cells.iter().enumerate() {
                        row.col(|ui| {
                            if r == 0 && c == 0 {
                                let mut s = overlay.get(0, 0, text).to_owned();
                                if ui.text_edit_singleline(&mut s).changed() {
                                    overlay.set(0, 0, s);
                                }
                            } else {
                                ui.label(overlay.get(r, c, text));
                            }
                        });
                    }
                });
            });
    }
}

fn load(path: &str) -> Result<Table, String> {
    let mut f = File::open(path).map_err(|e| format!("cannot open the file: {e}"))?;
    let mut sample = vec![0u8; 8192];
    let n = f.read(&mut sample).map_err(|e| e.to_string())?;
    let delim = sniff_delimiter(&sample[..n]);
    let f = File::open(path).map_err(|e| e.to_string())?;
    read_csv(f, delim, &CsvLimits::default())
        .map(|(t, _)| t)
        .map_err(|e| e.to_string())
}

fn main() {
    let start = Instant::now();
    let mut bench = std::env::var_os("KH_BENCH_OUT").map(|p| {
        let out = OpenOptions::new().create(true).append(true).open(p);
        out.map(|out| Bench {
            out,
            phase: Phase::WaitReady,
            last: None,
        })
    });
    let loaded = match std::env::var("KH_CSV") {
        Ok(p) => load(&p),
        Err(_) => Err("KH_CSV is not set".to_owned()),
    };
    let table = match loaded {
        Ok(t) => t,
        Err(message) => {
            if let Some(Ok(b)) = bench.as_mut() {
                let message = message
                    .replace('\\', "\\\\")
                    .replace('"', "'")
                    .replace(char::is_control, " ");
                b.line(format!(r#"{{"event":"error","message":"{message}"}}"#));
            }
            eprintln!("{message}");
            std::process::exit(1);
        }
    };
    let bench = bench.and_then(Result::ok);
    let rows = table.row_count();
    let app = App {
        cache: table.window(0, FETCH),
        table,
        overlay: Overlay::default(),
        start,
        jump_last: false,
        scroll_to: None,
        bench,
        rows,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1366.0, 768.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    if let Err(e) = eframe::run_native("Kola Hub grid", options, Box::new(|_| Ok(Box::new(app)))) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile(&v, 50.0), 50.0);
        assert_eq!(percentile(&v, 95.0), 95.0);
        assert_eq!(percentile(&v, 100.0), 100.0);
        assert_eq!(percentile(&[], 50.0), 0.0);
        assert_eq!(percentile(&[3.0, 1.0, 2.0], 50.0), 2.0);
    }

    #[test]
    fn overlay_wins_over_original() {
        let mut o = Overlay::default();
        assert_eq!(o.get(0, 0, "a"), "a");
        o.set(0, 0, "b".into());
        assert_eq!(o.get(0, 0, "a"), "b");
        assert_eq!(o.get(0, 1, "c"), "c");
    }
}
