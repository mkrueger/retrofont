//! retrofont: retro terminal font toolkit.
//! Features: TDF parsing/rendering, FIGlet placeholder, conversion stubs.

pub mod convert;
mod error;
pub mod figlet;
mod font;
mod glyph;
pub use glyph::{transform_outline, OUTLINE_CHAR_SET_UNICODE};
pub mod tdf;
pub use error::{FontError, Result};
pub use font::Font;
pub use glyph::{Glyph, GlyphPart, RenderMode, RenderOptions};

// Test utilities
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
    type Error;
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
    fn from(_: std::fmt::Error) -> Self {
        FontError::InvalidGlyph
    }
}
