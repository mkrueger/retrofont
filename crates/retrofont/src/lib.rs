#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

pub mod convert;
mod error;
pub mod figlet;
mod font;
mod glyph;
pub use glyph::{OUTLINE_CHAR_SET_UNICODE, transform_outline};
pub mod tdf;
pub use error::{FontError, Result};
pub use font::Font;
pub use glyph::{Glyph, GlyphPart, RenderMode, RenderOptions};

// Test utilities
#[cfg(feature = "test-support")]
pub mod test_support;

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub ch: char,
    pub fg: Option<u8>,
    pub bg: Option<u8>,
    pub blink: bool,
}

impl Cell {
    pub fn new(ch: char, fg: Option<u8>, bg: Option<u8>, blink: bool) -> Self {
        Self { ch, fg, bg, blink }
    }
}

pub trait FontTarget {
    /// Reported through [`FontError::Target`] when rendering fails.
    type Error: std::fmt::Display;
    fn draw(&mut self, cell: Cell) -> std::result::Result<(), Self::Error>;
    fn next_line(&mut self) -> std::result::Result<(), Self::Error>;
    fn line_width_hint(&mut self, _width: usize) {}

    /// Skip a cell (transparent/empty position).
    /// Default implementation draws a space. Implementors can override
    /// to simply advance the cursor without drawing.
    fn skip(&mut self) -> std::result::Result<(), Self::Error> {
        self.draw(Cell::new(' ', None, None, false))
    }
}

impl From<std::fmt::Error> for FontError {
    fn from(e: std::fmt::Error) -> Self {
        FontError::Target(e.to_string())
    }
}
