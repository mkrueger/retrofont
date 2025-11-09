//! TDF font support (placeholder implementation)
use crate::{
    error::{FontError, Result},
    glyph::{Glyph, GlyphPart, RenderMode},
    FontTarget,
};
use once_cell::sync::Lazy;
use std::collections::HashMap;

// Constants adapted from icy_engine TheDrawFont
const THE_DRAW_FONT_ID: &[u8; 18] = b"TheDraw FONTS file";
const CTRL_Z: u8 = 0x1A;
const FONT_INDICATOR: u32 = 0xFF00_AA55;
const FONT_NAME_LEN: usize = 12;
const FONT_NAME_LEN_MAX: usize = 16; // 12 + 4 nulls
const CHAR_TABLE_SIZE: usize = 94; // printable  !..~ range

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FontType {
    Outline,
    Block,
    Color,
}

#[derive(Debug)]
pub enum TdfParseError {
    FileTooShort,
    IdLengthMismatch(u8),
    IdMismatch,
    FontIndicatorMismatch,
    UnsupportedFontType(u8),
    GlyphOutsideFontDataSize(usize),
    NameTooLong(usize),
}

impl std::fmt::Display for TdfParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use TdfParseError::*;
        match self {
            FileTooShort => write!(f, "file too short"),
            IdLengthMismatch(l) => write!(f, "id length mismatch {l}"),
            IdMismatch => write!(f, "id mismatch"),
            FontIndicatorMismatch => write!(f, "font indicator mismatch"),
            UnsupportedFontType(t) => write!(f, "unsupported font type {t}"),
            GlyphOutsideFontDataSize(o) => write!(f, "glyph outside font data {o}"),
            NameTooLong(l) => write!(f, "name too long {l}"),
        }
    }
}

impl std::error::Error for TdfParseError {}

#[derive(Clone)]
pub struct TdfFont {
    pub name: String,
    pub font_type: FontType,
    spacing: i32,
    glyphs: Vec<Option<Glyph>>, // full 256 for convenience, but TDF maps subset
}

impl TdfFont {
    pub fn new(name: impl Into<String>, font_type: FontType, spacing: i32) -> Self {
        Self {
            name: name.into(),
            font_type,
            spacing,
            glyphs: vec![None; 256],
        }
    }

    pub fn add_glyph(&mut self, ch: u8, glyph: Glyph) {
        if (ch as usize) < 256 {
            self.glyphs[ch as usize] = Some(glyph);
        }
    }

    /// Returns the number of defined characters in this font.
    pub fn char_count(&self) -> usize {
        self.glyphs.iter().filter(|g| g.is_some()).count()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Vec<Self>> {
        // Parse one or multiple fonts from bundle
        if bytes.len() < 20 {
            return Err(FontError::Parse("tdf: file too short".into()));
        }
        let mut o = 0usize;
        let id_len = bytes[o] as usize;
        o += 1;
        if id_len != THE_DRAW_FONT_ID.len() + 1 {
            return Err(FontError::Parse(format!(
                "tdf: id length mismatch {}",
                id_len
            )));
        }
        if &bytes[o..o + 18] != THE_DRAW_FONT_ID {
            return Err(FontError::Parse("tdf: id mismatch".into()));
        }
        o += 18;
        if bytes[o] != CTRL_Z {
            return Err(FontError::Parse("tdf: missing ctrl-z".into()));
        }
        o += 1;
        let mut fonts = Vec::new();
        while o < bytes.len() {
            if bytes[o] == 0 {
                break;
            } // bundle terminator
            if o + 4 > bytes.len() {
                return Err(FontError::Parse("tdf: truncated indicator".into()));
            }
            let indicator = u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            if indicator != FONT_INDICATOR {
                return Err(FontError::Parse("tdf: font indicator mismatch".into()));
            }
            o += 4;
            if o >= bytes.len() {
                return Err(FontError::Parse("tdf: truncated name len".into()));
            }
            let orig_len = bytes[o] as usize;
            o += 1;
            let mut name_len = orig_len.min(FONT_NAME_LEN_MAX);
            if o + name_len > bytes.len() {
                return Err(FontError::Parse("tdf: truncated name".into()));
            }
            for i in 0..name_len {
                if bytes[o + i] == 0 {
                    name_len = i;
                    break;
                }
            }
            let name = String::from_utf8_lossy(&bytes[o..o + name_len]).to_string();
            o += FONT_NAME_LEN; // always skip full 12 bytes region
            o += 4; // magic bytes
            if o >= bytes.len() {
                return Err(FontError::Parse("tdf: truncated font type".into()));
            }
            let font_type = match bytes[o] {
                0 => FontType::Outline,
                1 => FontType::Block,
                2 => FontType::Color,
                other => return Err(FontError::Parse(format!("tdf: unsupported type {}", other))),
            };
            o += 1;
            if o >= bytes.len() {
                return Err(FontError::Parse("tdf: truncated spacing".into()));
            }
            let spacing = bytes[o] as i32;
            o += 1;
            if o + 2 > bytes.len() {
                return Err(FontError::Parse("tdf: truncated block size".into()));
            }
            let block_size = (bytes[o] as u16 | ((bytes[o + 1] as u16) << 8)) as usize;
            o += 2;
            if o + CHAR_TABLE_SIZE * 2 > bytes.len() {
                return Err(FontError::Parse("tdf: truncated char table".into()));
            }
            let mut lookup = Vec::with_capacity(CHAR_TABLE_SIZE);
            for _ in 0..CHAR_TABLE_SIZE {
                let off = bytes[o] as u16 | ((bytes[o + 1] as u16) << 8);
                o += 2;
                lookup.push(off);
            }
            if o + block_size > bytes.len() {
                return Err(FontError::Parse("tdf: block size beyond file".into()));
            }
            let base = o; // start of glyph block
            let mut font = TdfFont::new(name, font_type, spacing);
            for (i, char_offset) in lookup.iter().enumerate() {
                let mut glyph_offset = *char_offset as usize;
                if glyph_offset == 0xFFFF {
                    continue;
                }
                if glyph_offset >= block_size {
                    return Err(FontError::Parse(format!(
                        "tdf: glyph {} outside block",
                        glyph_offset
                    )));
                }
                glyph_offset += base;
                if glyph_offset + 2 > bytes.len() {
                    continue;
                }
                let width = bytes[glyph_offset] as usize;
                glyph_offset += 1;
                let height = bytes[glyph_offset] as usize;
                glyph_offset += 1;
                let mut parts = Vec::new();
                loop {
                    if glyph_offset >= bytes.len() {
                        break;
                    }
                    let ch = bytes[glyph_offset];
                    glyph_offset += 1;
                    if ch == 0 {
                        break;
                    }
                    if ch == 13 {
                        parts.push(GlyphPart::NewLine);
                        continue;
                    }
                    if ch == b'&' {
                        parts.push(GlyphPart::EndMarker);
                        continue;
                    }
                    match font_type {
                        FontType::Color => {
                            if glyph_offset >= bytes.len() {
                                break;
                            }
                            let attr = bytes[glyph_offset];
                            glyph_offset += 1;
                            let fg = (attr >> 4) & 0x0F;
                            let bg = attr & 0x0F;
                            if ch == 0xFF {
                                parts.push(GlyphPart::HardBlank);
                            } else {
                                let uc = crate::tdf::CP437_TO_UNICODE[ch as usize];
                                parts.push(GlyphPart::Colored { ch: uc, fg, bg });
                            }
                        }
                        FontType::Block => {
                            if ch == 0xFF {
                                parts.push(GlyphPart::HardBlank);
                            } else {
                                parts.push(GlyphPart::Char(
                                    crate::tdf::CP437_TO_UNICODE[ch as usize],
                                ));
                            }
                        }
                        FontType::Outline => {
                            if ch == b'@' {
                                parts.push(GlyphPart::FillMarker);
                            } else if ch == b'O' {
                                parts.push(GlyphPart::OutlineHole);
                            } else if ch >= b'A' && ch <= b'R' {
                                parts.push(GlyphPart::OutlinePlaceholder(ch));
                            } else if ch == b' ' {
                                parts.push(GlyphPart::Char(' '));
                            } else {
                                parts.push(GlyphPart::Char(
                                    crate::tdf::CP437_TO_UNICODE[ch as usize],
                                ));
                            }
                        }
                    }
                }
                let glyph = Glyph {
                    width,
                    height,
                    parts,
                };
                font.glyphs[b' ' as usize + 1 + i] = Some(glyph); // map printable range starting at space+1
            }
            o += block_size;
            fonts.push(font);
        }
        Ok(fonts)
    }

    /// Iterate over all defined glyphs, yielding (char, &Glyph).
    /// Skips empty slots. Only characters with code < 256 are considered.
    pub fn iter_glyphs(&self) -> impl Iterator<Item = (char, &Glyph)> {
        self.glyphs
            .iter()
            .enumerate()
            .filter_map(|(i, g)| g.as_ref().map(|glyph| (i as u8 as char, glyph)))
    }

    pub fn as_tdf_bytes(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        out.push(THE_DRAW_FONT_ID.len() as u8 + 1);
        out.extend(THE_DRAW_FONT_ID);
        out.push(CTRL_Z);
        self.append_font_data(&mut out)?;
        Ok(out)
    }

    pub fn create_bundle(fonts: &[TdfFont]) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        out.push(THE_DRAW_FONT_ID.len() as u8 + 1);
        out.extend(THE_DRAW_FONT_ID);
        out.push(CTRL_Z);
        for f in fonts {
            f.append_font_data(&mut out)?;
        }
        out.push(0); // terminator
        Ok(out)
    }

    fn append_font_data(&self, out: &mut Vec<u8>) -> Result<()> {
        out.extend(u32::to_le_bytes(FONT_INDICATOR));
        if self.name.len() > FONT_NAME_LEN {
            return Err(FontError::Parse(format!(
                "name too long {}",
                self.name.len()
            )));
        }
        out.push(FONT_NAME_LEN as u8);
        out.extend(self.name.as_bytes());
        out.extend(vec![0; FONT_NAME_LEN - self.name.len()]);
        out.extend([0, 0, 0, 0]);
        let type_byte = match self.font_type {
            FontType::Outline => 0,
            FontType::Block => 1,
            FontType::Color => 2,
        };
        out.push(type_byte);
        out.push(self.spacing as u8);
        // build lookup + glyph data
        let mut lookup = Vec::new();
        let mut glyph_block = Vec::new();
        for i in 0..CHAR_TABLE_SIZE {
            let code_index = b' ' as usize + 1 + i;
            if let Some(g) = &self.glyphs.get(code_index).and_then(|g| g.as_ref()) {
                lookup.extend(u16::to_le_bytes(glyph_block.len() as u16));
                glyph_block.push(g.width as u8);
                glyph_block.push(g.height as u8);
                for part in &g.parts {
                    match part {
                        GlyphPart::NewLine => glyph_block.push(13),
                        GlyphPart::EndMarker => glyph_block.push(b'&'),
                        GlyphPart::HardBlank => glyph_block.push(0xFF),
                        GlyphPart::FillMarker => glyph_block.push(b'@'),
                        GlyphPart::OutlineHole => glyph_block.push(b'O'),
                        GlyphPart::OutlinePlaceholder(b) => glyph_block.push(*b),
                        GlyphPart::Char(c) => {
                            let mapped = UNICODE_TO_CP437.get(c).copied().unwrap_or(b'?');
                            glyph_block.push(mapped);
                        }
                        GlyphPart::Colored { ch, fg, bg } => {
                            let mapped = UNICODE_TO_CP437.get(ch).copied().unwrap_or(b'?');
                            glyph_block.push(mapped);
                            glyph_block.push((*fg << 4) | (*bg & 0x0F));
                        }
                    }
                }
                glyph_block.push(0); // terminator
            } else {
                lookup.extend(u16::to_le_bytes(0xFFFF));
            }
        }
        out.extend(u16::to_le_bytes(glyph_block.len() as u16));
        out.extend(lookup);
        out.extend(glyph_block);
        Ok(())
    }
}

impl TdfFont {
    pub fn font_type(&self) -> FontType {
        self.font_type
    }
    pub fn has_char(&self, ch: char) -> bool {
        (ch as u32) < 256 && self.glyphs[ch as usize].is_some()
    }
    pub fn render_char<T: FontTarget>(
        &self,
        target: &mut T,
        ch: char,
        mode: RenderMode,
    ) -> Result<()> {
        let Some(g) = (ch as u32 <= 255)
            .then(|| self.glyphs[ch as usize].clone())
            .flatten()
        else {
            return Err(FontError::UnknownChar(ch));
        };
        g.render(target, mode)
    }
}

pub const CP437_TO_UNICODE: [char; 256] = [
    '\x00', '\u{263a}', '\u{263b}', '\u{2665}', '\u{2666}', '\u{2663}', '\u{2660}', '\u{2022}',
    '\x08', '\x09', '\x0A', '\u{2642}', '\u{2640}', '\x0D', '\u{266b}', '\u{263c}', '\u{25ba}',
    '\u{25c4}', '\u{2195}', '\u{203c}', '\u{00b6}', '\u{00a7}', '\u{25ac}', '\u{21a8}', '\u{2191}',
    '\u{2193}', '\x1A', '\x1B', '\u{221f}', '\u{2194}', '\u{25b2}', '\u{25bc}', '\u{0020}',
    '\u{0021}', '\u{0022}', '\u{0023}', '\u{0024}', '\u{0025}', '\u{0026}', '\u{0027}', '\u{0028}',
    '\u{0029}', '\u{002a}', '\u{002b}', '\u{002c}', '\u{002d}', '\u{002e}', '\u{002f}', '\u{0030}',
    '\u{0031}', '\u{0032}', '\u{0033}', '\u{0034}', '\u{0035}', '\u{0036}', '\u{0037}', '\u{0038}',
    '\u{0039}', '\u{003a}', '\u{003b}', '\u{003c}', '\u{003d}', '\u{003e}', '\u{003f}', '\u{0040}',
    '\u{0041}', '\u{0042}', '\u{0043}', '\u{0044}', '\u{0045}', '\u{0046}', '\u{0047}', '\u{0048}',
    '\u{0049}', '\u{004a}', '\u{004b}', '\u{004c}', '\u{004d}', '\u{004e}', '\u{004f}', '\u{0050}',
    '\u{0051}', '\u{0052}', '\u{0053}', '\u{0054}', '\u{0055}', '\u{0056}', '\u{0057}', '\u{0058}',
    '\u{0059}', '\u{005a}', '\u{005b}', '\u{005c}', '\u{005d}', '\u{005e}', '\u{005f}', '\u{0060}',
    '\u{0061}', '\u{0062}', '\u{0063}', '\u{0064}', '\u{0065}', '\u{0066}', '\u{0067}', '\u{0068}',
    '\u{0069}', '\u{006a}', '\u{006b}', '\u{006c}', '\u{006d}', '\u{006e}', '\u{006f}', '\u{0070}',
    '\u{0071}', '\u{0072}', '\u{0073}', '\u{0074}', '\u{0075}', '\u{0076}', '\u{0077}', '\u{0078}',
    '\u{0079}', '\u{007a}', '\u{007b}', '\u{007c}', '\u{007d}', '\u{007e}', '\u{007f}', '\u{00c7}',
    '\u{00fc}', '\u{00e9}', '\u{00e2}', '\u{00e4}', '\u{00e0}', '\u{00e5}', '\u{00e7}', '\u{00ea}',
    '\u{00eb}', '\u{00e8}', '\u{00ef}', '\u{00ee}', '\u{00ec}', '\u{00c4}', '\u{00c5}', '\u{00c9}',
    '\u{00e6}', '\u{00c6}', '\u{00f4}', '\u{00f6}', '\u{00f2}', '\u{00fb}', '\u{00f9}', '\u{00ff}',
    '\u{00d6}', '\u{00dc}', '\u{00a2}', '\u{00a3}', '\u{00a5}', '\u{20a7}', '\u{0192}', '\u{00e1}',
    '\u{00ed}', '\u{00f3}', '\u{00fa}', '\u{00f1}', '\u{00d1}', '\u{00aa}', '\u{00ba}', '\u{00bf}',
    '\u{2310}', '\u{00ac}', '\u{00bd}', '\u{00bc}', '\u{00a1}', '\u{00ab}', '\u{00bb}', '\u{2591}',
    '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{2561}', '\u{2562}', '\u{2556}', '\u{2555}',
    '\u{2563}', '\u{2551}', '\u{2557}', '\u{255d}', '\u{255c}', '\u{255b}', '\u{2510}', '\u{2514}',
    '\u{2534}', '\u{252c}', '\u{251c}', '\u{2500}', '\u{253c}', '\u{255e}', '\u{255f}', '\u{255a}',
    '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256c}', '\u{2567}', '\u{2568}',
    '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}', '\u{2552}', '\u{2553}', '\u{256b}', '\u{256a}',
    '\u{2518}', '\u{250c}', '\u{2588}', '\u{2584}', '\u{258c}', '\u{2590}', '\u{2580}', '\u{03b1}',
    '\u{00df}', '\u{0393}', '\u{03c0}', '\u{03a3}', '\u{03c3}', '\u{00b5}', '\u{03c4}', '\u{03a6}',
    '\u{0398}', '\u{03a9}', '\u{03b4}', '\u{221e}', '\u{03c6}', '\u{03b5}', '\u{2229}', '\u{2261}',
    '\u{00b1}', '\u{2265}', '\u{2264}', '\u{2320}', '\u{2321}', '\u{00f7}', '\u{2248}', '\u{00b0}',
    '\u{2219}', '\u{00b7}', '\u{221a}', '\u{207f}', '\u{00b2}', '\u{25a0}', '\u{00a0}',
];

// Reverse lookup for serialization: Unicode char -> CP437 byte.
// Built lazily to avoid startup cost in binaries not using serialization.
pub static UNICODE_TO_CP437: Lazy<HashMap<char, u8>> = Lazy::new(|| {
    let mut m = HashMap::with_capacity(256);
    for (i, c) in CP437_TO_UNICODE.iter().enumerate() {
        // Skip control NUL so we don't map general '\0' into output inadvertently.
        if *c != '\0' {
            m.insert(*c, i as u8);
        }
    }
    m
});
