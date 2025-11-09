use crate::{
    error::{FontError, Result},
    Cell, FontTarget,
};
// Use CP437 to Unicode mapping from TDF module for consistent Unicode output
use crate::tdf::CP437_TO_UNICODE;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FontType {
    Outline,
    Block,
    Color,
    Figlet,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RenderMode {
    Display,
    Edit,
    Raw,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GlyphPart {
    NewLine,                              // logical line break (CR or Figlet '\n')
    EndMarker,                            // '&' marker kept only in Edit mode
    HardBlank,                            // 0xFF placeholder -> ' ' outside Edit, NBSP inside Edit
    FillMarker,                           // '@' internal fill marker for outline fonts
    OutlineHole,                          // 'O' becomes space outside Edit
    OutlinePlaceholder(u8),               // 'A'.. style letters mapped via OUTLINE_CHAR_SET_UNICODE
    Char(char),                           // plain unicode character
    Colored { ch: char, fg: u8, bg: u8 }, // color font cell with per-cell attributes
}

#[derive(Clone, Debug)]
pub struct Glyph {
    pub width: usize,
    pub height: usize,
    pub parts: Vec<GlyphPart>,
    pub font_type: FontType,
}

// Outline style table copied from TheDraw (19 styles, 17 characters each)
// Pre-converted Unicode outline character sets (previously CP437 values converted at runtime)
const OUTLINE_CHAR_SET_UNICODE: [[char; 17]; 19] = [
    [
        '─', '─', '│', '│', '┌', '┐', '┌', '┐', '└', '┘', '└', '┘', '┤', '├', ' ', ' ', ' ',
    ],
    [
        '═', '─', '│', '│', '╒', '╕', '┌', '┐', '╘', '╛', '└', '┘', '╡', '├', ' ', ' ', ' ',
    ],
    [
        '─', '═', '│', '│', '┌', '┐', '╒', '╕', '└', '┘', '╘', '╛', '┤', '╞', ' ', ' ', ' ',
    ],
    [
        '═', '═', '│', '│', '╒', '╕', '╒', '╕', '╘', '╛', '╘', '╛', '╡', '╞', ' ', ' ', ' ',
    ],
    [
        '─', '─', '║', '│', '╓', '┐', '┌', '╖', '└', '╜', '╙', '┘', '╢', '├', ' ', ' ', ' ',
    ],
    [
        '═', '─', '║', '│', '╔', '╕', '┌', '╖', '╘', '╝', '╙', '┘', '╣', '├', ' ', ' ', ' ',
    ],
    [
        '─', '═', '║', '│', '╓', '┐', '╒', '╗', '└', '╜', '╚', '╛', '╢', '╞', ' ', ' ', ' ',
    ],
    [
        '═', '═', '║', '│', '╔', '╕', '╒', '╗', '╘', '╝', '╚', '╛', '╣', '╞', ' ', ' ', ' ',
    ],
    [
        '─', '─', '│', '║', '┌', '╖', '╓', '┐', '╙', '┘', '└', '╜', '┤', '╟', ' ', ' ', ' ',
    ],
    [
        '═', '─', '│', '║', '╒', '╗', '╓', '┐', '╚', '╛', '└', '╜', '╡', '╟', ' ', ' ', ' ',
    ],
    [
        '─', '═', '│', '║', '┌', '╖', '╔', '╕', '╙', '┘', '╘', '╝', '┤', '╠', ' ', ' ', ' ',
    ],
    [
        '═', '═', '│', '║', '╒', '╗', '╔', '╕', '╚', '╛', '╘', '╝', '╡', '╠', ' ', ' ', ' ',
    ],
    [
        '─', '─', '║', '║', '╓', '╖', '╓', '╖', '╙', '╜', '╙', '╜', '╢', '╟', ' ', ' ', ' ',
    ],
    [
        '═', '─', '║', '║', '╔', '╗', '╓', '╖', '╚', '╝', '╙', '╜', '╣', '╟', ' ', ' ', ' ',
    ],
    [
        '─', '═', '║', '║', '╓', '╖', '╔', '╗', '╙', '╜', '╚', '╝', '╢', '╠', ' ', ' ', ' ',
    ],
    [
        '═', '═', '║', '║', '╔', '╗', '╔', '╗', '╚', '╝', '╚', '╝', '╣', '╠', ' ', ' ', ' ',
    ],
    [
        '▄', '▄', '█', '█', '▄', '▄', '▄', '▄', '█', '█', '█', '█', '█', '█', ' ', ' ', ' ',
    ],
    [
        '▀', '▀', '█', '█', '█', '█', '█', '█', '▀', '▀', '▀', '▀', '█', '█', ' ', ' ', ' ',
    ],
    [
        '▀', '▄', '▐', '▌', '▐', '▌', '▄', '▄', '▀', '▀', '▐', '▌', '█', '█', ' ', ' ', ' ',
    ],
];

fn transform_outline(outline_style: usize, ch: u8) -> char {
    if ch > 64 && ch - 64 <= 17 {
        if outline_style >= OUTLINE_CHAR_SET_UNICODE.len() {
            CP437_TO_UNICODE[ch as usize]
        } else {
            OUTLINE_CHAR_SET_UNICODE[outline_style][(ch - 65) as usize]
        }
    } else {
        ' '
    }
}

impl Glyph {
    pub fn render<T: FontTarget>(&self, target: &mut T, style: RenderMode) -> Result<()> {
        let outline_style = 0usize; // future customization point
        let mut leading_space = true;
        for part in &self.parts {
            match part {
                GlyphPart::NewLine => {
                    target.next_line().map_err(|_| FontError::InvalidGlyph)?;
                    leading_space = true;
                }
                GlyphPart::EndMarker => {
                    if style == RenderMode::Edit {
                        target
                            .draw(Cell::new('&', None, None))
                            .map_err(|_| FontError::InvalidGlyph)?;
                    }
                }
                GlyphPart::HardBlank => {
                    // Display outside edit as regular space, edit shows NBSP for clarity
                    let ch = if style == RenderMode::Edit {
                        CP437_TO_UNICODE[0xFF]
                    } else {
                        ' '
                    };
                    target
                        .draw(Cell::new(ch, None, None))
                        .map_err(|_| FontError::InvalidGlyph)?;
                    leading_space = false;
                }
                GlyphPart::FillMarker => {
                    // Display: space, Edit: '@'
                    let ch = if style == RenderMode::Edit { '@' } else { ' ' };
                    target
                        .draw(Cell::new(ch, None, None))
                        .map_err(|_| FontError::InvalidGlyph)?;
                    leading_space = false;
                }
                GlyphPart::OutlineHole => {
                    // Display: space, Edit: 'O'
                    let ch = if style == RenderMode::Edit { 'O' } else { ' ' };
                    target
                        .draw(Cell::new(ch, None, None))
                        .map_err(|_| FontError::InvalidGlyph)?;
                    leading_space = false;
                }
                GlyphPart::OutlinePlaceholder(b) => {
                    // Leading space suppression keeps width but draws space
                    if leading_space && *b == b' ' {
                        target
                            .draw(Cell::new(' ', None, None))
                            .map_err(|_| FontError::InvalidGlyph)?;
                        continue;
                    }
                    leading_space = false;
                    let ch = transform_outline(outline_style, *b);
                    target
                        .draw(Cell::new(ch, None, None))
                        .map_err(|_| FontError::InvalidGlyph)?;
                }
                GlyphPart::Char(c) => {
                    if leading_space && *c == ' ' {
                        target
                            .draw(Cell::new(' ', None, None))
                            .map_err(|_| FontError::InvalidGlyph)?;
                        continue;
                    }
                    leading_space = false;
                    target
                        .draw(Cell::new(*c, None, None))
                        .map_err(|_| FontError::InvalidGlyph)?;
                }
                GlyphPart::Colored { ch, fg, bg } => {
                    // transparency heuristic
                    if *bg == 0 && *ch == ' ' {
                        continue;
                    }
                    target
                        .draw(Cell::new(*ch, Some(*fg), Some(*bg)))
                        .map_err(|_| FontError::InvalidGlyph)?;
                    leading_space = false;
                }
            }
        }
        Ok(())
    }
}
