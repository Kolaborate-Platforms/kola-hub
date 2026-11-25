#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

fn main() {
    let started_at = std::time::Instant::now();
    kh_app::run(started_at);
}
