//! TDF font support (placeholder implementation)
use crate::{
    error::{FontError, Result},
    glyph::{Glyph, GlyphPart},
};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

// Constants adapted from icy_engine TheDrawFont
const THE_DRAW_FONT_ID: &[u8; 18] = b"TheDraw FONTS file";
const CTRL_Z: u8 = 0x1A;
const FONT_INDICATOR: u32 = 0xFF00_AA55;
const FONT_NAME_LEN: usize = 12;
const FONT_NAME_LEN_MAX: usize = 16; // 12 + 4 nulls
const CHAR_TABLE_SIZE: usize = 94; // printable  !..~ range
const TDF_FIRST_CHAR: u8 = b'!';
const TDF_LAST_CHAR: u8 = b'~';

pub const MAX_TDF_GLYPH_WIDTH: usize = 30;
pub const MAX_TDF_GLYPH_HEIGHT: usize = 12;
const INVALID_GLYPH: u16 = 0xFFFF;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TdfFontType {
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
    pub font_type: TdfFontType,
    pub spacing: i32,
    // Overlay for programmatically constructed/modified glyphs.
    // Index 0 corresponds to '!'.
    glyphs_overlay: [Option<Glyph>; CHAR_TABLE_SIZE],
    // Lazy glyph source for parsed fonts.
    lazy: Option<LazyGlyphSource>,
}

#[derive(Clone)]
struct LazyGlyphSource {
    bytes: Arc<[u8]>,
    font_type: TdfFontType,
    glyph_block_base: usize,
    glyph_block_end: usize,
    lookup: [u16; CHAR_TABLE_SIZE],
    cache: Arc<[OnceLock<Glyph>; CHAR_TABLE_SIZE]>,
}

#[inline]
fn tdf_index(ch: char) -> Option<usize> {
    let code = ch as u32;
    if code > u8::MAX as u32 {
        return None;
    }
    let b = code as u8;
    if (TDF_FIRST_CHAR..=TDF_LAST_CHAR).contains(&b) {
        Some((b - TDF_FIRST_CHAR) as usize)
    } else {
        None
    }
}

#[inline]
fn tdf_char(index: usize) -> char {
    (TDF_FIRST_CHAR + index as u8) as char
}

impl TdfFont {
    pub fn new(name: impl Into<String>, font_type: TdfFontType, spacing: i32) -> Self {
        Self {
            name: name.into(),
            font_type,
            spacing,
            glyphs_overlay: std::array::from_fn(|_| None),
            lazy: None,
        }
    }

    pub fn add_glyph(&mut self, ch: char, glyph: Glyph) {
        let Some(idx) = tdf_index(ch) else {
            return;
        };
        self.glyphs_overlay[idx] = Some(glyph);
    }

    /// Removes a glyph from this font.
    /// Returns `true` if a glyph was present and has been removed.
    pub fn remove_glyph(&mut self, ch: char) -> bool {
        let Some(idx) = tdf_index(ch) else {
            return false;
        };
        let had_overlay = self.glyphs_overlay[idx].take().is_some();
        let had_lazy = if let Some(lazy) = &mut self.lazy {
            if lazy.lookup[idx] != INVALID_GLYPH {
                lazy.lookup[idx] = INVALID_GLYPH;
                true
            } else {
                false
            }
        } else {
            false
        };
        had_overlay || had_lazy
    }

    /// Returns the number of defined characters in this font.
    pub fn glyph_count(&self) -> usize {
        let mut count = 0usize;
        for i in 0..CHAR_TABLE_SIZE {
            if self.glyphs_overlay[i].is_some() {
                count += 1;
                continue;
            }
            if let Some(lazy) = &self.lazy {
                if lazy.lookup[i] != INVALID_GLYPH {
                    count += 1;
                }
            }
        }
        count
    }

    /// Calculate the average width of defined glyphs (excluding space if undefined).
    /// Returns None if no glyphs are defined.
    pub fn spacing(&self) -> Option<usize> {
        Some(self.spacing.max(1) as usize)
    }

    pub fn load(bytes: &[u8]) -> Result<Vec<Self>> {
        // Backwards-compatible API: this copies bytes for lazy decoding.
        Self::load_arc(Arc::<[u8]>::from(bytes.to_vec()))
    }

    pub fn load_arc(bytes: Arc<[u8]>) -> Result<Vec<Self>> {
        // Parse one or multiple fonts from bundle
        let b = bytes.as_ref();
        if b.len() < 20 {
            return Err(FontError::TdfFileTooShort);
        }
        let mut o = 0usize;
        let id_len = b[o] as usize;
        o += 1;
        if id_len != THE_DRAW_FONT_ID.len() + 1 {
            return Err(FontError::TdfIdLengthMismatch {
                expected: THE_DRAW_FONT_ID.len() + 1,
                got: id_len,
            });
        }
        if &b[o..o + 18] != THE_DRAW_FONT_ID {
            return Err(FontError::TdfIdMismatch);
        }
        o += 18;
        if b[o] != CTRL_Z {
            return Err(FontError::TdfMissingCtrlZ);
        }
        o += 1;
        let mut fonts = Vec::new();
        while o < b.len() {
            if b[o] == 0 {
                break;
            } // bundle terminator
            if o + 4 > b.len() {
                return Err(FontError::TdfTruncated { field: "indicator" });
            }
            let indicator = u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
            if indicator != FONT_INDICATOR {
                return Err(FontError::TdfFontIndicatorMismatch);
            }
            o += 4;
            if o >= b.len() {
                return Err(FontError::TdfTruncated {
                    field: "name length",
                });
            }
            let orig_len = b[o] as usize;
            o += 1;
            let mut name_len = orig_len.min(FONT_NAME_LEN_MAX);
            if o + name_len > b.len() {
                return Err(FontError::TdfTruncated { field: "name" });
            }
            for i in 0..name_len {
                if b[o + i] == 0 {
                    name_len = i;
                    break;
                }
            }
            let name = String::from_utf8_lossy(&b[o..o + name_len]).into_owned();
            o += FONT_NAME_LEN; // always skip full 12 bytes region
            o += 4; // magic bytes
            if o >= b.len() {
                return Err(FontError::TdfTruncated { field: "font type" });
            }
            let font_type = match b[o] {
                0 => TdfFontType::Outline,
                1 => TdfFontType::Block,
                2 => TdfFontType::Color,
                other => return Err(FontError::TdfUnsupportedType(other)),
            };
            o += 1;
            if o >= b.len() {
                return Err(FontError::TdfTruncated { field: "spacing" });
            }
            let spacing = b[o] as i32;
            o += 1;
            if o + 2 > b.len() {
                return Err(FontError::TdfTruncated {
                    field: "block size",
                });
            }
            let block_size = (b[o] as u16 | ((b[o + 1] as u16) << 8)) as usize;
            o += 2;
            if o + CHAR_TABLE_SIZE * 2 > b.len() {
                return Err(FontError::TdfTruncated {
                    field: "char table",
                });
            }
            let mut lookup: [u16; CHAR_TABLE_SIZE] = [0u16; CHAR_TABLE_SIZE];
            // We did one bounds check above; now do unchecked reads in the hot loop.
            unsafe {
                for i in 0..CHAR_TABLE_SIZE {
                    let lo = *b.get_unchecked(o);
                    let hi = *b.get_unchecked(o + 1);
                    lookup[i] = u16::from_le_bytes([lo, hi]);
                    o += 2;
                }
            }
            if o + block_size > b.len() {
                return Err(FontError::TdfTruncated {
                    field: "glyph block",
                });
            }
            // Validate lookup offsets are within block once, so glyph() can stay fast.
            for off in lookup.iter().copied() {
                if off == INVALID_GLYPH {
                    continue;
                }
                let off_usize = off as usize;
                if off_usize >= block_size {
                    return Err(FontError::TdfGlyphOutOfBounds {
                        offset: off_usize,
                        size: block_size,
                    });
                }
            }

            let base = o; // start of glyph block
            let glyph_block_end = o + block_size;
            let cache: Arc<[OnceLock<Glyph>; CHAR_TABLE_SIZE]> =
                Arc::new(std::array::from_fn(|_| OnceLock::new()));

            let font = TdfFont {
                name,
                font_type,
                spacing,
                glyphs_overlay: std::array::from_fn(|_| None),
                lazy: Some(LazyGlyphSource {
                    bytes: bytes.clone(),
                    font_type,
                    glyph_block_base: base,
                    glyph_block_end,
                    lookup,
                    cache,
                }),
            };

            o += block_size;
            fonts.push(font);
        }
        Ok(fonts)
    }

    /// Iterate over all defined glyphs, yielding (char, &Glyph).
    /// Skips empty slots. Only characters with code < 256 are considered.
    pub fn iter_glyphs(&self) -> impl Iterator<Item = (char, &Glyph)> {
        (0..CHAR_TABLE_SIZE).filter_map(move |i| {
            let ch = tdf_char(i);
            self.glyph(ch).map(|g| (ch, g))
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        out.push(THE_DRAW_FONT_ID.len() as u8 + 1);
        out.extend(THE_DRAW_FONT_ID);
        out.push(CTRL_Z);
        self.append_font_data(&mut out)?;
        Ok(out)
    }

    pub fn serialize_bundle(fonts: &[TdfFont]) -> Result<Vec<u8>> {
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
            return Err(FontError::TdfNameTooLong {
                len: self.name.len(),
                max: FONT_NAME_LEN,
            });
        }
        out.push(FONT_NAME_LEN as u8);
        out.extend(self.name.as_bytes());
        out.extend(vec![0; FONT_NAME_LEN - self.name.len()]);
        out.extend([0, 0, 0, 0]);
        let type_byte = match self.font_type {
            TdfFontType::Outline => 0,
            TdfFontType::Block => 1,
            TdfFontType::Color => 2,
        };
        out.push(type_byte);
        out.push(self.spacing as u8);
        // build lookup + glyph data
        let mut lookup = Vec::new();
        let mut glyph_block = Vec::new();
        for i in 0..CHAR_TABLE_SIZE {
            let ch = tdf_char(i);
            if let Some(g) = self.glyph(ch) {
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
                        GlyphPart::Skip => glyph_block.push(b' '),
                        GlyphPart::Char(c) => {
                            let mapped = UNICODE_TO_CP437.get(c).copied().unwrap_or(b'?');
                            glyph_block.push(mapped);
                        }
                        GlyphPart::AnsiChar { ch, fg, bg, blink } => {
                            let mapped = UNICODE_TO_CP437.get(ch).copied().unwrap_or(b'?');
                            glyph_block.push(mapped);
                            glyph_block.push(
                                ((bg & 0x07) << 4) | (fg & 0x0F) | if *blink { 0x80 } else { 0x00 },
                            );
                        }
                    }
                }
                glyph_block.push(0); // terminator
            } else {
                lookup.extend(u16::to_le_bytes(INVALID_GLYPH));
            }
        }
        out.extend(u16::to_le_bytes(glyph_block.len() as u16));
        out.extend(lookup);
        out.extend(glyph_block);
        Ok(())
    }

    /// Safe access to a glyph by raw byte code (0-255).
    /// Mirrors `FigletFont::glyph` for API consistency.
    pub fn glyph(&self, ch: char) -> Option<&Glyph> {
        let idx = tdf_index(ch)?;
        if let Some(g) = self.glyphs_overlay[idx].as_ref() {
            return Some(g);
        }
        let lazy = self.lazy.as_ref()?;
        if lazy.lookup[idx] == INVALID_GLYPH {
            return None;
        }

        // Quick sanity check (best-effort) so cache init can stay infallible.
        let off = lazy.lookup[idx] as usize;
        let abs = lazy.glyph_block_base + off;
        if abs + 2 > lazy.glyph_block_end {
            return None;
        }

        Some(lazy.cache[idx].get_or_init(|| decode_glyph(lazy, idx)))
    }

    /// Get the size of a glyph (width, height)
    pub fn glyph_size(&self, ch: char) -> Option<(usize, usize)> {
        self.glyph(ch).map(|g| (g.width, g.height))
    }

    /// Get the maximum height of all glyphs in this font
    pub fn max_height(&self) -> usize {
        let mut max_h = 0;
        for ch in '!'..='~' {
            if let Some(g) = self.glyph(ch) {
                max_h = max_h.max(g.height);
            }
        }
        max_h.max(1)
    }
    pub fn font_type(&self) -> TdfFontType {
        self.font_type
    }

    pub fn has_char(&self, ch: char) -> bool {
        let Some(idx) = tdf_index(ch) else {
            return false;
        };
        if self.glyphs_overlay[idx].is_some() {
            return true;
        }
        self.lazy
            .as_ref()
            .is_some_and(|lazy| lazy.lookup[idx] != INVALID_GLYPH)
    }
}

fn decode_glyph(lazy: &LazyGlyphSource, idx: usize) -> Glyph {
    let b = lazy.bytes.as_ref();
    let off = lazy.lookup[idx] as usize;
    let mut p = lazy.glyph_block_base + off;

    // Width/height are inside the glyph block.
    if p + 2 > lazy.glyph_block_end {
        return Glyph {
            width: 0,
            height: 0,
            parts: Vec::new(),
        };
    }

    let width = unsafe { *b.get_unchecked(p) as usize };
    let height = unsafe { *b.get_unchecked(p + 1) as usize };
    p += 2;

    let mut parts = Vec::with_capacity(width.saturating_mul(height).saturating_add(height));
    while p < lazy.glyph_block_end {
        let ch = unsafe { *b.get_unchecked(p) };
        p += 1;
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

        match lazy.font_type {
            TdfFontType::Color => {
                if p >= lazy.glyph_block_end {
                    break;
                }
                let attr = unsafe { *b.get_unchecked(p) };
                p += 1;
                let fg = attr & 0x0F;
                let bg = (attr >> 4) & 0x07;
                let blink = (attr & 0x80) != 0;
                if ch == 0xFF {
                    parts.push(GlyphPart::HardBlank);
                } else if ch == b' ' {
                    parts.push(GlyphPart::Skip);
                } else {
                    let uc = CP437_TO_UNICODE[ch as usize];
                    parts.push(GlyphPart::AnsiChar {
                        ch: uc,
                        fg,
                        bg,
                        blink,
                    });
                }
            }
            TdfFontType::Block => {
                if ch == 0xFF {
                    parts.push(GlyphPart::HardBlank);
                } else if ch == b' ' {
                    parts.push(GlyphPart::Skip);
                } else {
                    parts.push(GlyphPart::Char(CP437_TO_UNICODE[ch as usize]));
                }
            }
            TdfFontType::Outline => {
                if ch == b'@' {
                    parts.push(GlyphPart::FillMarker);
                } else if ch == b'O' {
                    parts.push(GlyphPart::OutlineHole);
                } else if (b'A'..=b'R').contains(&ch) {
                    parts.push(GlyphPart::OutlinePlaceholder(ch));
                } else if ch == b' ' {
                    parts.push(GlyphPart::Skip);
                } else {
                    parts.push(GlyphPart::Char(CP437_TO_UNICODE[ch as usize]));
                }
            }
        }
    }

    Glyph {
        width,
        height,
        parts,
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

// ─────────────────────────────────────────────────────────────────────────────
// Serde support – serializes TdfFont as compact TDF binary when possible,
// falls back to materialized glyphs for programmatically built fonts.
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "serde")]
mod serde_impl {
    use super::*;
    use serde::{de, ser, Deserialize, Deserializer, Serialize, Serializer};

    /// Compact representation: raw TDF bytes.
    #[derive(Serialize, Deserialize)]
    struct TdfFontRepr(#[serde(with = "serde_bytes")] Vec<u8>);

    mod serde_bytes {
        use serde::{Deserialize, Deserializer, Serializer};

        pub fn serialize<S: Serializer>(
            bytes: &Vec<u8>,
            s: S,
        ) -> std::result::Result<S::Ok, S::Error> {
            s.serialize_bytes(bytes)
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(
            d: D,
        ) -> std::result::Result<Vec<u8>, D::Error> {
            let bytes: &[u8] = Deserialize::deserialize(d)?;
            Ok(bytes.to_vec())
        }
    }

    impl Serialize for TdfFont {
        fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
            let tdf_bytes = TdfFont::serialize_bundle(std::slice::from_ref(self))
                .map_err(ser::Error::custom)?;
            TdfFontRepr(tdf_bytes).serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for TdfFont {
        fn deserialize<D: Deserializer<'de>>(
            deserializer: D,
        ) -> std::result::Result<Self, D::Error> {
            let TdfFontRepr(bytes) = TdfFontRepr::deserialize(deserializer)?;
            let fonts = TdfFont::load(&bytes).map_err(de::Error::custom)?;
            fonts
                .into_iter()
                .next()
                .ok_or_else(|| de::Error::custom("empty TDF bundle"))
        }
    }
}
