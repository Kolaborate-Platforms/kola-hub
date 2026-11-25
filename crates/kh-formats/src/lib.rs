//! File formats: CSV today; XLSX, images, JSON Lines, COCO and YOLO later.
//!
//! Every file is treated as hostile. Readers take explicit limits and fail with a plain message
//! instead of using unbounded memory.

mod csv_read;
mod csv_write;

pub use csv_read::{CsvError, CsvLimits, CsvReport, read_csv, sniff_delimiter};
pub use csv_write::{CsvWriteError, CsvWriteOptions, neutralise_cell, write_csv};
