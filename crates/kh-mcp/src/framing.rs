//! Reads one line at a time without ever holding more than the limit.

use std::io::{self, BufRead};

pub(crate) enum Line {
    Text(Vec<u8>),
    Blank,
    TooLong,
    End,
}

/// A line over `limit` bytes is thrown away as it is read, so memory stays small.
pub(crate) fn read_line(reader: &mut impl BufRead, limit: usize) -> io::Result<Line> {
    let mut line = Vec::new();
    let mut too_long = false;
    let mut seen_any = false;
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            return Ok(finish(line, too_long, seen_any));
        }
        seen_any = true;
        let newline = buffer.iter().position(|b| *b == b'\n');
        let used = newline.map_or(buffer.len(), |i| i + 1);
        let text_len = newline.unwrap_or(buffer.len());
        if !too_long {
            if line.len() + text_len > limit {
                too_long = true;
                line = Vec::new();
            } else {
                line.extend_from_slice(&buffer[..text_len]);
            }
        }
        reader.consume(used);
        if newline.is_some() {
            return Ok(finish(line, too_long, true));
        }
    }
}

fn finish(mut line: Vec<u8>, too_long: bool, seen_any: bool) -> Line {
    if too_long {
        return Line::TooLong;
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    if line.iter().all(u8::is_ascii_whitespace) {
        return if seen_any { Line::Blank } else { Line::End };
    }
    Line::Text(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(input: &[u8], limit: usize) -> Vec<String> {
        let mut reader = input;
        let mut out = Vec::new();
        loop {
            match read_line(&mut reader, limit).expect("read") {
                Line::End => return out,
                Line::Blank => out.push("blank".into()),
                Line::TooLong => out.push("long".into()),
                Line::Text(t) => out.push(String::from_utf8(t).expect("utf8")),
            }
        }
    }

    #[test]
    fn long_lines_are_skipped_and_the_next_line_still_reads() {
        assert_eq!(
            lines(b"ab\r\n\nabcdef\ncd", 4),
            ["ab", "blank", "long", "cd"]
        );
        assert_eq!(lines(b"", 4), Vec::<String>::new());
    }
}
