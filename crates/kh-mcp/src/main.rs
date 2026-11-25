//! The console program an assistant starts. It talks over standard input and output only.

use std::io::{self, BufReader};
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(data_dir) = kh_service::data_dir::default_data_dir() else {
        eprintln!("Kola Hub could not find a folder for its files on this computer.");
        return ExitCode::FAILURE;
    };
    let reader = BufReader::new(io::stdin().lock());
    match kh_mcp::serve(reader, io::stdout().lock(), &data_dir) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
