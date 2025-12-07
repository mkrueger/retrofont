use std::io::Read;

use crate::{
    figlet::FigletFont, glyph::RenderOptions, tdf::TdfFont, FontError, FontTarget, Result,
};

/// Unified font enum encapsulating all supported font kinds.
///
/// Replaces the trait-based dynamic dispatch with a simple tagged union. This keeps
/// font operations ergonomic without requiring generics or trait objects when only
/// supporting built-in formats.
pub enum Font {
    Figlet(FigletFont),
    Tdf(TdfFont),
}

impl Font {
    pub fn name(&self) -> &str {
        match self {
            Font::Figlet(f) => &f.name,
            Font::Tdf(f) => &f.name,
        }
    }

    pub fn has_char(&self, ch: char) -> bool {
        match self {
            Font::Figlet(f) => f.has_char(ch),
            Font::Tdf(f) => f.has_char(ch),
        }
    }

    pub fn spacing(&self) -> Option<usize> {
        match self {
            Font::Figlet(f) => f.spacing(),
            Font::Tdf(f) => f.spacing(),
        }
    }

    pub fn render_glyph<T: FontTarget>(
        &self,
        target: &mut T,
        ch: char,
        options: &RenderOptions,
    ) -> Result<()> {
        // Special handling for space character if not defined in font
        if ch == ' ' && !self.has_char(' ') {
            // Calculate reasonable space width: use average glyph width or default to 1
            let space_width = self.spacing().unwrap_or(1);

            // Render empty space by drawing spaces for the calculated width
            for _ in 0..space_width {
                target
                    .draw(crate::Cell::new(' ', None, None, false))
                    .map_err(|_| FontError::InvalidGlyph)?;
            }
            return Ok(());
        }

        // Try to find the character or its case variant
        let char_to_render = if self.has_char(ch) {
            ch
        } else if ch.is_alphabetic() {
            // Try the opposite case if the original character is not found
            if ch.is_lowercase() {
                let upper = ch.to_uppercase().next().unwrap_or(ch);
                if self.has_char(upper) {
                    upper
                } else {
                    ch // Fall back to original if uppercase not found
                }
            } else {
                let lower = ch.to_lowercase().next().unwrap_or(ch);
                if self.has_char(lower) {
                    lower
                } else {
                    ch // Fall back to original if lowercase not found
                }
            }
        } else {
            ch
        };

        let glyph = match self {
            Font::Figlet(f) => f.glyph(char_to_render),
            Font::Tdf(f) => f.glyph(char_to_render),
        };
        let Some(glyph) = glyph else {
            return Err(FontError::UnknownChar(ch));
        };
        glyph.render(target, options)
    }

    /// Load fonts from raw bytes, attempting FIGlet first (header check) then TDF.
    ///
    /// Returns a vector containing:
    /// - A single font for FIGlet files
    /// - Multiple fonts for TDF bundles (which can contain many fonts)
    /// - An error if the format is unrecognized or parsing fails
    pub fn load(bytes: &[u8]) -> Result<Vec<Font>> {
        // Attempt FIGlet: header starts with 'flf2a'
        if bytes.len() >= 5 && &bytes[0..5] == b"flf2a" {
            let fig = FigletFont::load(bytes)?;
            return Ok(vec![Font::Figlet(fig)]);
        }
        // Attempt TDF: id length byte (0x13=19) followed by 'TheDraw FONTS file' (18 bytes)
        if bytes.len() >= 19 && bytes[0] == 0x13 && &bytes[1..19] == b"TheDraw FONTS file" {
            let fonts = TdfFont::load(bytes)?;
            if fonts.is_empty() {
                return Err(FontError::TdfEmptyBundle);
            }
            return Ok(fonts.into_iter().map(Font::Tdf).collect());
        }
        Err(FontError::UnrecognizedFormat)
    }

    pub fn read<R: Read>(reader: R) -> Result<Vec<Font>> {
        let mut buf = Vec::new();
        let mut reader = reader;
        reader.read_to_end(&mut buf)?;
        Self::load(&buf)
    }
}
