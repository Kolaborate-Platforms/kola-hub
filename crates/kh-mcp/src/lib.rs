//! The AI server. One JSON message per line in, one per line out. Every tool call checks the settings
//! again, uses a fresh `Shell` and closes the Clean session it opened.

#![forbid(unsafe_code)]

mod args;
mod framing;
mod protocol;
mod schema;
mod tools;

use std::io::{self, BufRead, Write};
use std::path::Path;

use framing::Line;

/// Longest request line accepted, in bytes.
pub const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;

/// Answers requests from `reader` on `writer` until the reader ends. Only protocol messages are written.
pub fn serve(mut reader: impl BufRead, mut writer: impl Write, data_dir: &Path) -> io::Result<()> {
    loop {
        let response = match framing::read_line(&mut reader, MAX_LINE_BYTES)? {
            Line::End => return Ok(()),
            Line::Blank => None,
            Line::TooLong => Some(protocol::too_long()),
            Line::Text(bytes) => protocol::handle(&bytes, data_dir),
        };
        if let Some(message) = response {
            serde_json::to_writer(&mut writer, &message).map_err(io::Error::from)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }
    }
}
