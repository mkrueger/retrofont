//! retrofont: retro terminal font toolkit.
//! Features: TDF parsing/rendering, FIGlet placeholder, conversion stubs.

pub mod convert;
mod error;
pub mod figlet;
mod font;
mod glyph;
pub mod tdf;
pub use error::{FontError, Result};
pub use font::Font;
pub use glyph::{FontType, Glyph, GlyphPart, RenderMode};

// Test utilities
pub mod test_support;

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub ch: char,
    pub fg: Option<u8>,
    pub bg: Option<u8>,
    pub bold: bool,
    pub blink: bool,
}

impl Cell {
    pub fn new(ch: char, fg: Option<u8>, bg: Option<u8>) -> Self {
        Self {
            ch,
            fg,
            bg,
            bold: false,
            blink: false,
        }
    }
}

pub trait FontTarget {
    type Error;
    fn draw(&mut self, cell: Cell) -> std::result::Result<(), Self::Error>;
    fn next_line(&mut self) -> std::result::Result<(), Self::Error>;
    fn line_width_hint(&mut self, _width: usize) {}
}

impl From<std::fmt::Error> for FontError {
    fn from(_: std::fmt::Error) -> Self {
        FontError::InvalidGlyph
    }
}
