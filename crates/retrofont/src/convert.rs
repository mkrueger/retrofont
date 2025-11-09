//! Conversion stubs (FIGlet -> TDF)
use crate::{
    error::{FontError, Result},
    figlet::FigletFont,
    glyph::{Glyph, GlyphPart},
    tdf::{FontType, TdfFont},
};

/// Convert a FIGlet font into a TDF font with the requested target type.
///
/// Notes:
/// * Spacing is heuristically set to 1; future versions may derive optimal spacing.
/// * Color/outline conversion is currently a straight part copy; smarter outline mapping could
///   collapse placeholder sets.
pub fn figlet_to_tdf(fig: &FigletFont, target_type: FontType) -> Result<TdfFont> {
    if !matches!(
        target_type,
        FontType::Color | FontType::Block | FontType::Outline
    ) {
        return Err(FontError::UnsupportedType);
    }
    let mut tdf = TdfFont::new(fig.name.clone(), target_type, 1);
    for code in 0u8..=255u8 {
        if let Some(g) = fig.glyph(code) {
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
                    GlyphPart::Char(c) => {
                        parts.push(GlyphPart::Char(*c));
                        line_width += 1;
                    }
                    _ => {
                        parts.push(part.clone());
                        if matches!(
                            part,
                            GlyphPart::Colored { .. }
                                | GlyphPart::HardBlank
                                | GlyphPart::FillMarker
                                | GlyphPart::OutlineHole
                                | GlyphPart::OutlinePlaceholder(_)
                                | GlyphPart::EndMarker
                        ) {
                            line_width += 1;
                        }
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
