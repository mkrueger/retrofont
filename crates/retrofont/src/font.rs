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

    pub fn render_char<T: FontTarget>(
        &self,
        target: &mut T,
        ch: char,
        mode: RenderMode,
    ) -> Result<()> {
        match self {
            Font::Figlet(f) => f.render_char(target, ch, mode),
            Font::Tdf(f) => f.render_char(target, ch, mode),
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

    /// Load a font from raw bytes, attempting FIGlet first (header check) then TDF.
    /// Returns the first font if a TDF bundle contains multiple.
    pub fn from_bytes(bytes: &[u8]) -> Result<Font> {
        // Attempt FIGlet: header starts with 'flf2a'
        if bytes.len() >= 5 && &bytes[0..5] == b"flf2a" {
            let fig = FigletFont::from_bytes(bytes)?;
            return Ok(Font::Figlet(fig));
        }
        // Attempt TDF: id length byte followed by 'TheDraw FONTS file'
        if !bytes.is_empty() && bytes[0] as usize == 19 && bytes.len() >= 19 + 1 {
            if bytes.len() >= 20 && &bytes[1..19] == b"TheDraw FONTS file" {
                let fonts = TdfFont::from_bytes(bytes)?;
                if let Some(first) = fonts.into_iter().next() {
                    return Ok(Font::Tdf(first));
                } else {
                    return Err(FontError::Parse("tdf: no fonts in bundle".into()));
                }
            }
        }
        Err(FontError::Parse("unrecognized font format".into()))
    }
}
