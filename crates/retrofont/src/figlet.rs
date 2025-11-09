//! FIGlet font placeholder.
use crate::{
    error::{FontError, Result},
    glyph::{Glyph, GlyphPart},
};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::{fs, path::Path};
use zip::ZipArchive;

#[derive(Clone)]
pub struct FigletFont {
    pub name: String,
    pub header: String,
    pub comments: Vec<String>,
    pub hard_blank: char,
    pub(crate) glyphs: HashMap<char, Glyph>,
}

impl FigletFont {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            header: String::new(),
            comments: Vec::new(),
            hard_blank: '$',
            glyphs: HashMap::new(),
        }
    }

    /// Safe access to a glyph by byte code (0-255).
    pub fn glyph(&self, ch: char) -> Option<&Glyph> {
        self.glyphs.get(&ch)
    }

    /// Iterate over all defined FIGlet glyphs as (char, &Glyph).
    pub fn iter_glyphs(&self) -> impl Iterator<Item = (char, &Glyph)> {
        self.glyphs.iter().map(|(ch, glyph)| (*ch, glyph))
    }

    pub fn load_file(path: &Path) -> Result<Self> {
        let bytes =
            fs::read(path).map_err(|e| FontError::Parse(format!("figlet read error: {e}")))?;
        Self::from_bytes(&bytes)
    }

    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    /// Calculate the average width of defined glyphs (excluding space if undefined).
    /// Returns None if no glyphs are defined.
    pub(crate) fn spacing(&self) -> Option<usize> {
        if self.glyphs.is_empty() {
            return None;
        }

        let total: usize = self.glyphs.values().map(|g| g.width).sum();
        Some(total / self.glyphs.len())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let data = bytes;
        // Detect gzip signature (1F 8B) and decompress via zip crate fallback if possible.
        if bytes.len() >= 2 && bytes[0] == 0x1F && bytes[1] == 0x8B {
            // The 'zip' crate doesn't natively handle bare .gz streams; attempt to treat as single-file zip when header matches PK.. else manual inflate not available.
            // For now return error to avoid pulling second decompression crate.
            return Err(FontError::Parse(
                "gzip compressed .flf not supported without flate2; provide .flf or zipped archive"
                    .into(),
            ));
        }
        // If file looks like a ZIP (PK\x03\x04) attempt to locate a .flf inside.
        if bytes.len() >= 4 && &bytes[0..4] == b"PK\x03\x04" {
            let mut archive = ZipArchive::new(Cursor::new(bytes))
                .map_err(|e| FontError::Parse(format!("zip open error: {e}")))?;
            let mut found = None;
            for i in 0..archive.len() {
                let mut file = archive
                    .by_index(i)
                    .map_err(|e| FontError::Parse(format!("zip entry error: {e}")))?;
                if file.name().ends_with(".flf") {
                    let mut buf = String::new();
                    file.read_to_string(&mut buf)
                        .map_err(|e| FontError::Parse(format!("zip read flf error: {e}")))?;
                    found = Some(buf);
                    break;
                }
            }
            if let Some(content) = found {
                return FigletFont::parse_content(&content);
            }
            return Err(FontError::Parse("zip archive contained no .flf".into()));
        }
        let content =
            std::str::from_utf8(data).map_err(|e| FontError::Parse(format!("utf8 error: {e}")))?;
        FigletFont::parse_content(content)
    }

    fn parse_content(content: &str) -> Result<Self> {
        let mut lines = content.lines();
        let header_line = lines
            .next()
            .ok_or_else(|| FontError::Parse("missing header".into()))?;
        if !header_line.starts_with("flf2a") {
            return Err(FontError::Parse("not a flf2a header".into()));
        }

        // Extract hard blank character (the character immediately after "flf2a")
        let hard_blank = header_line.chars().nth(5).unwrap_or('$');

        let header_parts: Vec<&str> = header_line.split_whitespace().collect();
        if header_parts.len() < 6 {
            return Err(FontError::Parse("incomplete header".into()));
        }

        // Extract header parameters
        let height: usize = header_parts
            .get(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| FontError::Parse("missing height".into()))?;
        let comment_count: usize = header_parts
            .get(5)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let mut font = FigletFont::new("figlet");
        font.header = header_line.to_string();
        font.hard_blank = hard_blank;

        // Read comment lines
        for _ in 0..comment_count {
            if let Some(c) = lines.next() {
                font.comments.push(c.to_string());
            }
        }

        // Load required characters (ASCII 32-126) = 95 chars
        for ch in 32..=126 {
            if let Ok(char_lines) = Self::read_character(&mut lines, height) {
                font.add_raw_char(
                    ch,
                    &char_lines.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                );
            } else {
                break;
            }
        }

        // Try to load one more character (often 127 or extended chars)
        if let Ok(char_lines) = Self::read_character(&mut lines, height) {
            font.add_raw_char(
                127,
                &char_lines.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            );
        }

        // Load additional tagged characters if any remain
        while let Ok(_char_lines) = Self::read_character(&mut lines, height) {
            // Tagged characters would need special handling - skip for now
        }

        Ok(font)
    }

    fn read_character<'a, I>(lines: &mut I, height: usize) -> Result<Vec<String>>
    where
        I: Iterator<Item = &'a str>,
    {
        let mut char_lines = Vec::new();

        for _ in 0..height {
            let line = lines
                .next()
                .ok_or_else(|| FontError::Parse("incomplete character".into()))?;

            // Remove trailing @ markers (single @ for line end, @@ for character end)
            let trimmed = if line.ends_with("@@") {
                char_lines.push(line[..line.len() - 2].to_string());
                break;
            } else if line.ends_with('@') {
                line[..line.len() - 1].to_string()
            } else {
                return Err(FontError::Parse("character line missing @ marker".into()));
            };

            char_lines.push(trimmed);
        }

        Ok(char_lines)
    }

    pub fn add_raw_char(&mut self, ch: u8, raw_lines: &[&str]) {
        // Build parts with proper NewLine separators & compute width/height in one pass.
        let mut parts = Vec::new();
        let mut max_width = 0usize;
        for (row, line) in raw_lines.iter().enumerate() {
            if row > 0 {
                parts.push(GlyphPart::NewLine);
            }
            max_width = max_width.max(line.len());
            for ch in line.chars() {
                // Convert hard blank character to HardBlank GlyphPart
                if ch == self.hard_blank {
                    parts.push(GlyphPart::HardBlank);
                } else {
                    parts.push(GlyphPart::Char(ch));
                }
            }
        }
        let glyph = Glyph {
            width: max_width,
            height: raw_lines.len(),
            parts,
        };
        self.glyphs.insert(ch as char, glyph);
    }

    pub fn has_char(&self, ch: char) -> bool {
        self.glyphs.contains_key(&ch)
    }
}
