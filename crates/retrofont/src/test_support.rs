//! Test support utilities for retrofont.
//!
//! This module provides helper types and functions that are useful for testing
//! font rendering, but are not part of the public API.

use crate::{Cell, FontError, FontTarget};

/// A memory buffer target useful for tests.
///
/// Captures rendered output into a 2D vector of cells that can be inspected.
pub struct BufferTarget {
    pub lines: Vec<Vec<Cell>>,
    cur_line: usize,
}

impl BufferTarget {
    pub fn new() -> Self {
        Self {
            lines: vec![Vec::new()],
            cur_line: 0,
        }
    }
}

impl FontTarget for BufferTarget {
    type Error = FontError;

    fn draw(&mut self, cell: Cell) -> std::result::Result<(), Self::Error> {
        if self.cur_line >= self.lines.len() {
            self.lines.push(Vec::new());
        }
        self.lines[self.cur_line].push(cell);
        Ok(())
    }

    fn next_line(&mut self) -> std::result::Result<(), Self::Error> {
        self.cur_line += 1;
        if self.cur_line >= self.lines.len() {
            self.lines.push(Vec::new());
        }
        Ok(())
    }
}
