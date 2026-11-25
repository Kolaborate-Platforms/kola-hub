//! Table engine: typed columns, windowed row access. No UI, no file-format code, no async runtime.
//!
//! The interface never receives a whole table. It asks for a window of rows (about 100 at a time)
//! with [`Table::window`].
//!
//! Text is held in one of two shapes, chosen while the table is built:
//!
//! - a dictionary when a column repeats a few values a lot (market names, product names), and
//! - one shared byte buffer with offsets when almost every cell is different (names, notes).
//!
//! Neither shape allocates one `String` per cell, which is what keeps a million rows inside the
//! memory budget on a 4 GB laptop.

mod column;
mod table;

pub use column::{Column, ColumnKind};
pub use table::{Table, TableBuilder, TableError, Window};

/// Most columns a table may have. A hostile file with millions of columns is refused early.
pub const MAX_COLUMNS: usize = 16_384;
