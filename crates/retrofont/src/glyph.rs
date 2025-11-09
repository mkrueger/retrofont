use crate::{
    error::{FontError, Result},
    Cell, FontTarget,
};
// Use CP437 to Unicode mapping from TDF module for consistent Unicode output
use crate::tdf::CP437_TO_UNICODE;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RenderMode {
    Display,
    Edit,
    Raw,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GlyphPart {
    /// Logical line break (CR in TDF or embedded '\n' in Figlet sources)
    NewLine,
    /// End marker ('&') retained only in Edit mode for diagnostics
    EndMarker,
    /// Hard blank (0xFF) placeholder -> space in Display mode, NBSP in Edit mode
    HardBlank,
    /// Internal fill marker ('@') for outline fonts
    FillMarker,
    /// Outline hole marker ('O') becomes space in Display, shown as 'O' in Edit
    OutlineHole,
    /// Outline style placeholder letters 'A'.. mapped via pre-converted Unicode outline table
    OutlinePlaceholder(u8),
    /// Plain Unicode character cell
    Char(char),
    /// Color font cell with per-cell attributes (foreground/background 0-15)
    Colored { ch: char, fg: u8, bg: u8 },
}

#[derive(Clone, Debug)]
pub struct Glyph {
    /// Maximum width of any rendered line in the glyph
    pub width: usize,
    /// Number of lines in the glyph
    pub height: usize,
    /// Ordered glyph parts making up the rendered output
    pub parts: Vec<GlyphPart>,
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
    /// Render this glyph onto a target using the specified render mode.
    ///
    /// Edit mode exposes internal markers (HardBlank NBSP, '@', 'O', '&').
    /// Display mode hides them, treating them largely as spaces.
    pub fn render<T: FontTarget>(&self, target: &mut T, style: RenderMode) -> Result<()> {
        let outline_style = 0usize; // TODO: expose outline style selection via API.
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
                    let ch = if style == RenderMode::Edit { '@' } else { ' ' };
                    target
                        .draw(Cell::new(ch, None, None))
                        .map_err(|_| FontError::InvalidGlyph)?;
                    leading_space = false;
                }
                GlyphPart::OutlineHole => {
                    let ch = if style == RenderMode::Edit { 'O' } else { ' ' };
                    target
                        .draw(Cell::new(ch, None, None))
                        .map_err(|_| FontError::InvalidGlyph)?;
                    leading_space = false;
                }
                GlyphPart::OutlinePlaceholder(b) => {
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
                    if *bg == 0 && *ch == ' ' {
                        continue; // transparency heuristic
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
