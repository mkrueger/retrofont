//! FIGlet font placeholder.
use crate::{
    error::{FontError, Result},
    glyph::{Glyph, GlyphPart},
};
use std::io::{Cursor, Read};
use std::ops::Range;
use std::sync::{Arc, OnceLock};
use std::{fs, path::Path};
use zip::ZipArchive;

#[derive(Clone)]
pub struct FigletFont {
    pub name: String,
    pub header: String,
    pub comments: Vec<String>,
    pub hard_blank: char,
    // Programmatic/converted glyphs live here.
    glyphs_overlay: [Option<Glyph>; 256],
    // Parsed glyphs are decoded on-demand.
    lazy: Option<LazyFigletSource>,
}

#[derive(Clone)]
struct LazyFigletSource {
    bytes: Arc<[u8]>,
    hard_blank: char,
    // One entry per glyph line, in parse order.
    glyph_lines: Vec<Range<usize>>,
    // For each byte code (0..=255): start index into `glyph_lines` or u32::MAX.
    glyph_line_start: [u32; 256],
    // Number of lines for this glyph.
    glyph_line_len: [u8; 256],
    // Cached decoded glyphs.
    cache: Arc<[OnceLock<Glyph>; 256]>,
    // Precomputed spacing hint (average max line width).
    avg_width: Option<usize>,
}

impl FigletFont {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            header: String::new(),
            comments: Vec::new(),
            hard_blank: '$',
            glyphs_overlay: std::array::from_fn(|_| None),
            lazy: None,
        }
    }

    /// Safe access to a glyph by byte code (0-255).
    pub fn glyph(&self, ch: char) -> Option<&Glyph> {
        let code = ch as u32;
        if code > u8::MAX as u32 {
            return None;
        }
        let idx = code as usize;

        if let Some(g) = self.glyphs_overlay[idx].as_ref() {
            return Some(g);
        }
        let lazy = self.lazy.as_ref()?;
        if lazy.glyph_line_start[idx] == u32::MAX {
            return None;
        }
        Some(lazy.cache[idx].get_or_init(|| decode_glyph(lazy, idx)))
    }

    /// Iterate over all defined FIGlet glyphs as (char, &Glyph).
    pub fn iter_glyphs(&self) -> impl Iterator<Item = (char, &Glyph)> {
        (0u16..=255).filter_map(move |i| {
            let ch = (i as u8) as char;
            self.glyph(ch).map(|g| (ch, g))
        })
    }

    pub fn load_file(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)?;
        Self::load(&bytes)
    }

    pub fn glyph_count(&self) -> usize {
        let mut count = 0usize;
        for i in 0..256 {
            if self.glyphs_overlay[i].is_some() {
                count += 1;
                continue;
            }
            if let Some(lazy) = &self.lazy {
                if lazy.glyph_line_start[i] != u32::MAX {
                    count += 1;
                }
            }
        }
        count
    }

    /// Calculate the average width of defined glyphs (excluding space if undefined).
    /// Returns None if no glyphs are defined.
    pub(crate) fn spacing(&self) -> Option<usize> {
        // Prefer the precomputed hint for parsed fonts.
        if let Some(lazy) = &self.lazy {
            if lazy.avg_width.is_some() {
                return lazy.avg_width;
            }
        }
        // Fallback: compute from overlay glyphs.
        let mut total = 0usize;
        let mut count = 0usize;
        for g in self.glyphs_overlay.iter().flatten() {
            total += g.width;
            count += 1;
        }
        if count == 0 {
            None
        } else {
            Some(total / count)
        }
    }

    pub fn load(bytes: &[u8]) -> Result<Self> {
        Self::load_arc(Arc::<[u8]>::from(bytes.to_vec()))
    }

    pub fn load_arc(bytes: Arc<[u8]>) -> Result<Self> {
        let data = bytes.as_ref();
        // Detect gzip signature (1F 8B) and decompress via zip crate fallback if possible.
        if bytes.len() >= 2 && bytes[0] == 0x1F && bytes[1] == 0x8B {
            // The 'zip' crate doesn't natively handle bare .gz streams.
            // For now return error to avoid pulling second decompression crate.
            return Err(FontError::FigletGzipNotSupported);
        }
        // If file looks like a ZIP (PK\x03\x04) attempt to locate a .flf inside.
        if data.len() >= 4 && &data[0..4] == b"PK\x03\x04" {
            let mut archive = ZipArchive::new(Cursor::new(data))
                .map_err(|e| FontError::Zip(format!("open error: {e}")))?;
            let mut found = None;
            for i in 0..archive.len() {
                let mut file = archive
                    .by_index(i)
                    .map_err(|e| FontError::Zip(format!("entry error: {e}")))?;
                if file.name().ends_with(".flf") {
                    let mut buf = Vec::new();
                    file.read_to_end(&mut buf)
                        .map_err(|e| FontError::Zip(format!("read error: {e}")))?;
                    found = Some(buf);
                    break;
                }
            }
            if let Some(content) = found {
                return FigletFont::parse_bytes(Arc::<[u8]>::from(content));
            }
            return Err(FontError::ZipNoFlf);
        }
        FigletFont::parse_bytes(bytes)
    }

    fn parse_bytes(bytes: Arc<[u8]>) -> Result<Self> {
        // Validate UTF-8 once; we will slice by newline boundaries thereafter.
        let _ = std::str::from_utf8(bytes.as_ref())?;

        let line_ranges = compute_line_ranges(bytes.as_ref());
        if line_ranges.is_empty() {
            return Err(FontError::FigletMissingHeader);
        }

        let mut line_idx = 0usize;
        let header_range = line_ranges[line_idx].clone();
        let header_line = std::str::from_utf8(&bytes[header_range.clone()])?;
        line_idx += 1;
        if !header_line.starts_with("flf2a") {
            return Err(FontError::FigletInvalidSignature);
        }

        // Extract hard blank character (the character immediately after "flf2a")
        let hard_blank = header_line.chars().nth(5).unwrap_or('$');

        let header_parts: Vec<&str> = header_line.split_whitespace().collect();
        if header_parts.len() < 6 {
            return Err(FontError::FigletIncompleteHeader);
        }

        // Extract header parameters
        let height: usize = header_parts
            .get(1)
            .and_then(|s| s.parse().ok())
            .ok_or(FontError::FigletMissingHeight)?;
        let comment_count: usize = header_parts
            .get(5)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let mut font = FigletFont::new("figlet");
        font.header = header_line.to_string();
        font.hard_blank = hard_blank;

        // Read comment lines
        for _ in 0..comment_count {
            if line_idx >= line_ranges.len() {
                break;
            }
            let r = line_ranges[line_idx].clone();
            line_idx += 1;
            let c = std::str::from_utf8(&bytes[r]).unwrap_or("");
            font.comments.push(c.to_string());
        }

        // Parse glyphs lazily: record slices per glyph.
        let mut glyph_lines: Vec<Range<usize>> = Vec::new();
        let mut glyph_line_start: [u32; 256] = [u32::MAX; 256];
        let mut glyph_line_len: [u8; 256] = [0u8; 256];
        let mut sum_width = 0usize;
        let mut count = 0usize;

        // Load required characters (ASCII 32-126) = 95 chars
        for ch in 32u8..=126u8 {
            match read_character_ranges(&line_ranges, &mut line_idx, height, bytes.as_ref()) {
                Ok(ranges) => {
                    let start = glyph_lines.len();
                    let mut max_w = 0usize;
                    for r in &ranges {
                        max_w = max_w.max(r.end.saturating_sub(r.start));
                    }
                    glyph_lines.extend(ranges);
                    glyph_line_start[ch as usize] = start as u32;
                    glyph_line_len[ch as usize] = (glyph_lines.len() - start) as u8;
                    sum_width += max_w;
                    count += 1;
                }
                Err(_) => break,
            }
        }

        // Try to load one more character (often 127 or extended chars)
        if let Ok(ranges) =
            read_character_ranges(&line_ranges, &mut line_idx, height, bytes.as_ref())
        {
            let start = glyph_lines.len();
            let mut max_w = 0usize;
            for r in &ranges {
                max_w = max_w.max(r.end.saturating_sub(r.start));
            }
            glyph_lines.extend(ranges);
            glyph_line_start[127] = start as u32;
            glyph_line_len[127] = (glyph_lines.len() - start) as u8;
            sum_width += max_w;
            count += 1;
        }

        // Load additional tagged characters if any remain (skip)
        while read_character_ranges(&line_ranges, &mut line_idx, height, bytes.as_ref()).is_ok() {
            // Tagged characters would need special handling - skip for now
        }

        let cache: Arc<[OnceLock<Glyph>; 256]> = Arc::new(std::array::from_fn(|_| OnceLock::new()));
        let avg_width = if count == 0 {
            None
        } else {
            Some(sum_width / count)
        };
        font.lazy = Some(LazyFigletSource {
            bytes,
            hard_blank,
            glyph_lines,
            glyph_line_start,
            glyph_line_len,
            cache,
            avg_width,
        });

        Ok(font)
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
        self.glyphs_overlay[ch as usize] = Some(glyph);
    }

    pub fn has_char(&self, ch: char) -> bool {
        let code = ch as u32;
        if code > u8::MAX as u32 {
            return false;
        }
        let idx = code as usize;
        if self.glyphs_overlay[idx].is_some() {
            return true;
        }
        self.lazy
            .as_ref()
            .is_some_and(|lazy| lazy.glyph_line_start[idx] != u32::MAX)
    }

    /// Serialize this FIGlet font to bytes in .flf format.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();

        // Determine max height from all glyphs
        let max_height = self.compute_max_height();

        // Write header line
        // Format: flf2a<hardblank> height baseline maxlen smush comment_count
        let comment_count = self.comments.len();
        let header = format!(
            "flf2a{} {} {} {} -1 {}\n",
            self.hard_blank, max_height, max_height, 80, comment_count
        );
        out.extend(header.as_bytes());

        // Write comment lines
        for comment in &self.comments {
            out.extend(comment.as_bytes());
            out.push(b'\n');
        }

        // Write glyphs for ASCII 32-126 (required characters)
        for ch in 32u8..=126u8 {
            self.write_glyph_lines(&mut out, ch as char, max_height);
        }

        // Write glyph for ASCII 127 if present
        if self.has_char(127 as char) {
            self.write_glyph_lines(&mut out, 127 as char, max_height);
        }

        Ok(out)
    }

    fn compute_max_height(&self) -> usize {
        let mut max_h = 1usize;
        for ch in 32u8..=127u8 {
            if let Some(g) = self.glyph(ch as char) {
                max_h = max_h.max(g.height);
            }
        }
        max_h
    }

    fn write_glyph_lines(&self, out: &mut Vec<u8>, ch: char, max_height: usize) {
        if let Some(glyph) = self.glyph(ch) {
            // Build lines from glyph parts
            let mut lines: Vec<String> = Vec::new();
            let mut current_line = String::new();

            for part in &glyph.parts {
                match part {
                    GlyphPart::NewLine => {
                        lines.push(current_line);
                        current_line = String::new();
                    }
                    GlyphPart::HardBlank => {
                        current_line.push(self.hard_blank);
                    }
                    GlyphPart::Char(c) => {
                        current_line.push(*c);
                    }
                    _ => {
                        // For other part types, use space as fallback
                        current_line.push(' ');
                    }
                }
            }
            // Don't forget the last line if not empty
            if !current_line.is_empty() || lines.is_empty() {
                lines.push(current_line);
            }

            // Pad to max_height if needed
            while lines.len() < max_height {
                lines.push(String::new());
            }

            // Write lines with @ markers
            for (i, line) in lines.iter().enumerate() {
                out.extend(line.as_bytes());
                if i == lines.len() - 1 {
                    out.extend(b"@@\n"); // Last line gets @@
                } else {
                    out.extend(b"@\n");
                }
            }
        } else {
            // Write empty glyph placeholder
            for i in 0..max_height {
                if i == max_height - 1 {
                    out.extend(b"@@\n");
                } else {
                    out.extend(b"@\n");
                }
            }
        }
    }
}

fn compute_line_ranges(bytes: &[u8]) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            let mut end = i;
            if end > start && bytes[end - 1] == b'\r' {
                end -= 1;
            }
            out.push(start..end);
            start = i + 1;
        }
    }
    if start <= bytes.len() {
        let mut end = bytes.len();
        if end > start && bytes[end - 1] == b'\r' {
            end -= 1;
        }
        if start != end {
            out.push(start..end);
        }
    }
    out
}

fn read_character_ranges(
    lines: &[Range<usize>],
    line_idx: &mut usize,
    height: usize,
    bytes: &[u8],
) -> Result<Vec<Range<usize>>> {
    let mut out = Vec::with_capacity(height);
    for _ in 0..height {
        let r = lines
            .get(*line_idx)
            .ok_or(FontError::FigletIncompleteChar)?
            .clone();
        *line_idx += 1;
        let line = &bytes[r.clone()];
        if line.ends_with(b"@@") {
            out.push(r.start..(r.end - 2));
            break;
        }
        if line.ends_with(b"@") {
            out.push(r.start..(r.end - 1));
            continue;
        }
        return Err(FontError::FigletMissingMarker);
    }
    Ok(out)
}

fn decode_glyph(lazy: &LazyFigletSource, idx: usize) -> Glyph {
    let start = lazy.glyph_line_start[idx] as usize;
    let len = lazy.glyph_line_len[idx] as usize;
    let mut parts = Vec::new();
    let mut max_width = 0usize;

    for row in 0..len {
        if row > 0 {
            parts.push(GlyphPart::NewLine);
        }
        let r = &lazy.glyph_lines[start + row];
        let s = unsafe { std::str::from_utf8_unchecked(&lazy.bytes[r.clone()]) };
        let mut line_width = 0usize;
        for ch in s.chars() {
            if ch == lazy.hard_blank {
                parts.push(GlyphPart::HardBlank);
            } else {
                parts.push(GlyphPart::Char(ch));
            }
            line_width += 1;
        }
        max_width = max_width.max(line_width);
    }

    Glyph {
        width: max_width,
        height: len,
        parts,
    }
}
