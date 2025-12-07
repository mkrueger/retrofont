//! Conversion stubs (FIGlet -> TDF)
use crate::{
    error::{FontError, Result},
    figlet::FigletFont,
    glyph::{Glyph, GlyphPart},
    tdf::{TdfFont, TdfFontType, MAX_TDF_GLYPH_HEIGHT, MAX_TDF_GLYPH_WIDTH},
};

/// TDF supports printable ASCII range: '!' (0x21) through '~' (0x7E) = 94 characters
const TDF_FIRST_CHAR: char = '!'; // 0x21
const TDF_LAST_CHAR: char = '~'; // 0x7E

/// Check if a FIGlet font is compatible with TDF conversion.
///
/// - It has at least one character in the TDF printable range (! to ~)
/// - All glyphs have width and height that fit in u8 (max 255)
/// - The glyphs don't use features incompatible with the target TDF type
pub fn can_convert_figlet_to_tdf(fig: &FigletFont, _target_type: TdfFontType) -> bool {
    // Check if font has any characters in the TDF range with valid dimensions
    (TDF_FIRST_CHAR..=TDF_LAST_CHAR).any(|code| {
        if let Some(glyph) = fig.glyph(code) {
            glyph.width <= MAX_TDF_GLYPH_WIDTH && glyph.height <= MAX_TDF_GLYPH_HEIGHT
        } else {
            false
        }
    })
}

/// Convert a FIGlet font into a TDF font with the requested target type.
///
/// Only converts characters in the TDF printable range (! through ~).
/// Characters outside this range are skipped.
///
/// Notes:
/// * Spacing is heuristically set to 1; future versions may derive optimal spacing.
/// * Color/outline conversion is currently a straight part copy; smarter outline mapping could
///   collapse placeholder sets.
///
/// # Errors
///
/// Returns an error if:
/// - The target type is not supported (must be Block, Color)
/// - Outline is currently unsupported
/// - The font is incompatible with the target type
pub fn figlet_to_tdf(fig: &FigletFont, target_type: TdfFontType) -> Result<TdfFont> {
    if !matches!(target_type, TdfFontType::Color | TdfFontType::Block) {
        return Err(FontError::UnsupportedType);
    }

    // Check compatibility
    if !can_convert_figlet_to_tdf(fig, target_type) {
        return Err(FontError::ConversionIncompatible);
    }

    let mut tdf = TdfFont::new(fig.name.clone(), target_type, 1);

    // Only convert characters in the TDF printable range: ! (0x21) through ~ (0x7E)
    for code in TDF_FIRST_CHAR..=TDF_LAST_CHAR {
        if let Some(g) = fig.glyph(code) {
            // Skip glyphs that exceed TDF dimension limits
            if g.width > MAX_TDF_GLYPH_WIDTH || g.height > MAX_TDF_GLYPH_HEIGHT {
                continue;
            }

            let mut parts = Vec::new();
            let mut width = 0usize;
            let mut line_width = 0usize;
            let mut lines = 1usize;

            for part in &g.parts {
                match part {
                    GlyphPart::NewLine => {
                        parts.push(GlyphPart::NewLine);
                        width = width.max(line_width);
                        line_width = 0;
                        lines += 1;
                    }
                    // Single-char parts (Char, HardBlank, EndMarker)
                    GlyphPart::Char(c) => {
                        // Convert to Colored if target is Color type, otherwise keep as Char
                        if target_type == TdfFontType::Color {
                            parts.push(GlyphPart::AnsiChar {
                                ch: *c,
                                fg: 7, // Light gray (DOS default foreground)
                                bg: 0, // Black (DOS default background)
                                blink: false,
                            });
                        } else {
                            parts.push(GlyphPart::Char(*c));
                        }
                        line_width += 1;
                    }
                    GlyphPart::HardBlank => {
                        parts.push(GlyphPart::HardBlank);
                        line_width += 1;
                    }
                    GlyphPart::EndMarker => {
                        parts.push(GlyphPart::EndMarker);
                        line_width += 1;
                    }
                    GlyphPart::AnsiChar { ch, fg, bg, blink } => {
                        // If converting to Block or Outline, strip color and use plain Char
                        if target_type == TdfFontType::Color {
                            parts.push(GlyphPart::AnsiChar {
                                ch: *ch,
                                fg: *fg,
                                bg: *bg,
                                blink: *blink,
                            });
                        } else {
                            parts.push(GlyphPart::Char(*ch));
                        }
                        line_width += 1;
                    }
                    GlyphPart::FillMarker => {
                        parts.push(GlyphPart::FillMarker);
                        line_width += 1;
                    }
                    GlyphPart::OutlineHole => {
                        parts.push(GlyphPart::OutlineHole);
                        line_width += 1;
                    }
                    GlyphPart::OutlinePlaceholder(letter) => {
                        parts.push(GlyphPart::OutlinePlaceholder(*letter));
                        line_width += 1;
                    }
                }
            }

            width = width.max(line_width);
            let glyph = Glyph {
                width,
                height: lines,
                parts,
            };
            tdf.add_glyph(code, glyph);
        }
    }

    Ok(tdf)
}
