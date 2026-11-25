//! Column storage.

use std::collections::HashMap;

/// A dictionary column gives up and becomes a plain text column when it holds this many distinct values.
const MAX_DICT_ENTRIES: usize = 4_096;

/// Which storage shape a column currently uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnKind {
    /// Few distinct values, stored once each. Cells hold a 4 byte code.
    Dictionary,
    /// Many distinct values, stored back to back in one buffer.
    Plain,
}

/// One column of text cells.
///
/// Typed columns (numbers, dates) arrive with the Clean profiler. Until then every cell is text and
/// an empty string is an empty cell.
#[derive(Debug, Clone)]
pub enum Column {
    /// Dictionary encoded.
    Dictionary {
        /// Distinct values, in first-seen order.
        values: Vec<Box<str>>,
        /// One code per row, an index into `values`.
        codes: Vec<u32>,
    },
    /// Plain text in one buffer.
    Plain {
        /// All cell bytes, back to back.
        bytes: Vec<u8>,
        /// `offsets[i]..offsets[i + 1]` is cell `i`. Starts with 0.
        offsets: Vec<u32>,
    },
}

impl Column {
    /// Number of cells.
    pub fn len(&self) -> usize {
        match self {
            Column::Dictionary { codes, .. } => codes.len(),
            Column::Plain { offsets, .. } => offsets.len() - 1,
        }
    }

    /// True when the column has no cells.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Which storage shape this column uses.
    pub fn kind(&self) -> ColumnKind {
        match self {
            Column::Dictionary { .. } => ColumnKind::Dictionary,
            Column::Plain { .. } => ColumnKind::Plain,
        }
    }

    /// The text of cell `row`, or `None` when `row` is out of range.
    pub fn get(&self, row: usize) -> Option<&str> {
        match self {
            Column::Dictionary { values, codes } => {
                let code = *codes.get(row)?;
                values.get(code as usize).map(|v| &**v)
            }
            Column::Plain { bytes, offsets } => {
                let start = *offsets.get(row)? as usize;
                let end = *offsets.get(row + 1)? as usize;
                // Cells are only ever pushed as `&str`, so the slice is always valid UTF-8.
                std::str::from_utf8(bytes.get(start..end)?).ok()
            }
        }
    }

    /// Bytes of heap memory this column holds, for budget reports.
    pub fn heap_bytes(&self) -> usize {
        match self {
            Column::Dictionary { values, codes } => {
                codes.capacity() * 4 + values.iter().map(|v| v.len() + 16).sum::<usize>()
            }
            Column::Plain { bytes, offsets } => bytes.capacity() + offsets.capacity() * 4,
        }
    }
}

/// Builds a [`Column`] one cell at a time. Starts as a dictionary and switches to plain text on its own.
#[derive(Debug)]
pub(crate) struct ColumnBuilder {
    state: State,
}

#[derive(Debug)]
enum State {
    Dict {
        values: Vec<Box<str>>,
        lookup: HashMap<Box<str>, u32>,
        codes: Vec<u32>,
    },
    Plain {
        bytes: Vec<u8>,
        offsets: Vec<u32>,
    },
}

/// Raised when one column's text grows past what 4 byte offsets can address (4 GiB).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ColumnFull;

impl ColumnBuilder {
    pub(crate) fn new() -> Self {
        Self {
            state: State::Dict {
                values: Vec::new(),
                lookup: HashMap::new(),
                codes: Vec::new(),
            },
        }
    }

    pub(crate) fn push(&mut self, cell: &str) -> Result<(), ColumnFull> {
        if let State::Dict {
            values,
            lookup,
            codes,
        } = &mut self.state
        {
            if let Some(&code) = lookup.get(cell) {
                codes.push(code);
                return Ok(());
            }
            if values.len() < MAX_DICT_ENTRIES {
                let code = values.len() as u32;
                let value: Box<str> = cell.into();
                lookup.insert(value.clone(), code);
                values.push(value);
                codes.push(code);
                return Ok(());
            }
            self.make_plain()?;
        }
        if let State::Plain { bytes, offsets } = &mut self.state {
            let end = bytes
                .len()
                .checked_add(cell.len())
                .and_then(|n| u32::try_from(n).ok())
                .ok_or(ColumnFull)?;
            bytes.extend_from_slice(cell.as_bytes());
            offsets.push(end);
        }
        Ok(())
    }

    /// Rewrites a full dictionary as plain text.
    fn make_plain(&mut self) -> Result<(), ColumnFull> {
        let State::Dict { values, codes, .. } = &self.state else {
            return Ok(());
        };
        let mut bytes = Vec::with_capacity(codes.len() * 8);
        let mut offsets = Vec::with_capacity(codes.len() + 1);
        offsets.push(0u32);
        for &code in codes {
            let text = values.get(code as usize).map(|v| &**v).unwrap_or("");
            bytes.extend_from_slice(text.as_bytes());
            offsets.push(u32::try_from(bytes.len()).map_err(|_| ColumnFull)?);
        }
        self.state = State::Plain { bytes, offsets };
        Ok(())
    }

    pub(crate) fn finish(self) -> Column {
        match self.state {
            State::Dict {
                values, mut codes, ..
            } => {
                codes.shrink_to_fit();
                Column::Dictionary { values, codes }
            }
            State::Plain {
                mut bytes,
                mut offsets,
            } => {
                bytes.shrink_to_fit();
                offsets.shrink_to_fit();
                Column::Plain { bytes, offsets }
            }
        }
    }
}
