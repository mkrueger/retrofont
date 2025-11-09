//! Conversion stubs (FIGlet -> TDF)
use crate::{
    error::{FontError, Result},
    figlet::FigletFont,
    glyph::{FontType, Glyph, GlyphPart},
    tdf::TdfFont,
    Font,
};

pub fn figlet_to_tdf(fig: &FigletFont, target_type: FontType) -> Result<TdfFont> {
    if !matches!(
        target_type,
        FontType::Color | FontType::Block | FontType::Outline
    ) {
        return Err(FontError::UnsupportedType);
    }
    // Default spacing 1 for converted fonts (heuristic)
    let mut tdf = TdfFont::new(fig.name(), target_type, 1);
    for code in 0u8..=255u8 {
        // unsafe placeholder; real access would be via safe API
        let maybe = unsafe { fig_get(fig, code) };
        if let Some(g) = maybe {
            // naive row conversion
            let mut parts = Vec::new();
            let mut width = 0;
            let mut current_line_width = 0;
            let mut lines = 1;
            for part in &g.parts {
                match part {
                    GlyphPart::NewLine => {
                        parts.push(GlyphPart::NewLine);
                        width = width.max(current_line_width);
                        current_line_width = 0;
                        lines += 1;
                    }
                    GlyphPart::Char(c) => {
                        parts.push(GlyphPart::Char(*c));
                        current_line_width += 1;
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
                            current_line_width += 1;
                        }
                    }
                }
            }
            width = width.max(current_line_width);
            let glyph = Glyph {
                width,
                height: lines,
                parts,
                font_type: target_type,
            };
            tdf.add_glyph(code, glyph);
        }
    }
    Ok(tdf)
}

unsafe fn fig_get(fig: &FigletFont, idx: u8) -> Option<Glyph> {
    fig.glyphs.get(idx as usize).and_then(|g| g.clone())
}
