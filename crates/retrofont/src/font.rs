use crate::{figlet::FigletFont, tdf::TdfFont, FontError, FontTarget, RenderMode, Result};

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

    pub fn render_char<T: FontTarget>(
        &self,
        target: &mut T,
        ch: char,
        mode: RenderMode,
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

        match self {
            Font::Figlet(f) => f.render_char(target, char_to_render, mode),
            Font::Tdf(f) => f.render_char(target, char_to_render, mode),
        }
    }

    pub fn render_str<T: FontTarget>(
        &self,
        target: &mut T,
        text: &str,
        mode: RenderMode,
    ) -> Result<()> {
        for ch in text.chars() {
            self.render_char(target, ch, mode)?;
            target.next_line().map_err(|_| FontError::InvalidGlyph)?;
        }
        Ok(())
    }

    /// Load fonts from raw bytes, attempting FIGlet first (header check) then TDF.
    ///
    /// Returns a vector containing:
    /// - A single font for FIGlet files
    /// - Multiple fonts for TDF bundles (which can contain many fonts)
    /// - An error if the format is unrecognized or parsing fails
    pub fn from_bytes(bytes: &[u8]) -> Result<Vec<Font>> {
        // Attempt FIGlet: header starts with 'flf2a'
        if bytes.len() >= 5 && &bytes[0..5] == b"flf2a" {
            let fig = FigletFont::from_bytes(bytes)?;
            return Ok(vec![Font::Figlet(fig)]);
        }
        // Attempt TDF: id length byte followed by 'TheDraw FONTS file'
        if !bytes.is_empty() && bytes[0] as usize == 19 && bytes.len() >= 19 + 1 {
            if bytes.len() >= 20 && &bytes[1..20] == b"TheDraw FONTS file" {
                let fonts = TdfFont::from_bytes(bytes)?;
                if fonts.is_empty() {
                    return Err(FontError::Parse("tdf: no fonts in bundle".into()));
                }
                return Ok(fonts.into_iter().map(Font::Tdf).collect());
            }
        }
        Err(FontError::Parse("unrecognized font format".into()))
    }
}
