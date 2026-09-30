//! FIGlet (.flf) and TOIlet (.tlf) font parsing, rendering and serialization.
//!
//! TOIlet fonts use the FIGlet format with a `tlf2a` signature and are always UTF-8.
use crate::{
    error::{FontError, Result},
    glyph::{Glyph, GlyphPart},
};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};
use std::ops::Range;
use std::sync::{Arc, OnceLock};
use std::{fs, path::Path};
use zip::ZipArchive;

/// Bits of the FIGfont 2 `full_layout` header field that control horizontal layout.
///
/// Rules 1-6 (`EQUAL` ..= `HARDBLANK`) only take effect together with [`SMUSHING`].
/// Without [`KERNING`] or [`SMUSHING`] glyphs are laid out at full width.
pub mod layout {
    /// Rule 1: two identical characters smush into one.
    pub const EQUAL: u32 = 1;
    /// Rule 2: an underscore is replaced by `|/\[]{}()<>`.
    pub const UNDERSCORE: u32 = 2;
    /// Rule 3: the character of the "higher" class wins (`|`, `/\`, `[]`, `{}`, `()`, `<>`).
    pub const HIERARCHY: u32 = 4;
    /// Rule 4: opposing brackets (`[]`, `{}`, `()`) become `|`.
    pub const OPPOSITE_PAIR: u32 = 8;
    /// Rule 5: `/\` -> `|`, `\/` -> `Y`, `><` -> `X`.
    pub const BIG_X: u32 = 16;
    /// Rule 6: two hard blanks smush into one.
    pub const HARDBLANK: u32 = 32;
    /// Mask of all horizontal smushing rules.
    pub const RULES: u32 = 63;
    /// Horizontal fitting: glyphs move together until they touch.
    pub const KERNING: u32 = 64;
    /// Horizontal smushing: glyphs overlap by one column where the rules allow it.
    pub const SMUSHING: u32 = 128;
}

/// The file format a [`FigletFont`] was read from and is written as.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FigletFormat {
    /// FIGlet `.flf` (`flf2a` signature). UTF-8, or Latin-1 for older fonts.
    #[default]
    Flf,
    /// TOIlet `.tlf` (`tlf2a` signature). Always UTF-8.
    Tlf,
}

impl FigletFormat {
    fn signature(self) -> &'static str {
        match self {
            FigletFormat::Flf => "flf2a",
            FigletFormat::Tlf => "tlf2a",
        }
    }

    fn detect(bytes: &[u8]) -> Option<Self> {
        [FigletFormat::Flf, FigletFormat::Tlf]
            .into_iter()
            .find(|f| bytes.starts_with(f.signature().as_bytes()))
    }

    /// The usual file extension, without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            FigletFormat::Flf => "flf",
            FigletFormat::Tlf => "tlf",
        }
    }
}

/// The characters that follow the 95 required ASCII glyphs in a FIGfont, in file order.
const DEUTSCH: [char; 7] = ['Ä', 'Ö', 'Ü', 'ä', 'ö', 'ü', 'ß'];

#[derive(Clone)]
pub struct FigletFont {
    pub name: String,
    pub header: String,
    pub comments: Vec<String>,
    pub hard_blank: char,
    format: FigletFormat,
    // FIGfont 2 `full_layout` bits, see [`layout`].
    full_layout: u32,
    // 0 = left-to-right, 1 = right-to-left.
    print_direction: u8,
    // Programmatic/converted glyphs live here and take precedence over parsed ones.
    glyphs_overlay: BTreeMap<char, Glyph>,
    // Parsed glyphs are decoded on-demand.
    lazy: Option<LazyFigletSource>,
}

/// Text encoding of a font file. FIGlet predates UTF-8, so files that aren't
/// valid UTF-8 are read as Latin-1 like C figlet does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Encoding {
    Utf8,
    Latin1,
}

impl Encoding {
    fn detect(bytes: &[u8]) -> Self {
        if std::str::from_utf8(bytes).is_ok() {
            Encoding::Utf8
        } else {
            Encoding::Latin1
        }
    }

    fn decode(self, bytes: &[u8]) -> Cow<'_, str> {
        match self {
            // Lossless: the whole file was validated and lines split on ASCII bytes.
            Encoding::Utf8 => String::from_utf8_lossy(bytes),
            Encoding::Latin1 => Cow::Owned(bytes.iter().map(|&b| char::from(b)).collect()),
        }
    }
}

#[derive(Clone)]
struct LazyFigletSource {
    bytes: Arc<[u8]>,
    encoding: Encoding,
    // TOIlet glyphs may contain ANSI colour codes.
    ansi: bool,
    hard_blank: char,
    // One entry per glyph line with end marks removed, in parse order.
    glyph_lines: Vec<Range<usize>>,
    // Character -> index into `glyphs` and `cache`.
    index: BTreeMap<char, usize>,
    // Per glyph: its rows as a range into `glyph_lines`.
    glyphs: Vec<Range<usize>>,
    // Cached decoded glyphs.
    cache: Arc<[OnceLock<Glyph>]>,
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
            format: FigletFormat::Flf,
            full_layout: 0,
            print_direction: 0,
            glyphs_overlay: BTreeMap::new(),
            lazy: None,
        }
    }

    /// Horizontal layout bits (FIGfont 2 `full_layout`), see [`layout`].
    ///
    /// Fonts without a `full_layout` header field get it derived from `old_layout`.
    pub fn layout(&self) -> u32 {
        self.full_layout
    }

    /// Set the horizontal layout bits (see [`layout`]). Vertical layout bits are ignored.
    pub fn set_layout(&mut self, full_layout: u32) {
        self.full_layout = full_layout & (layout::RULES | layout::KERNING | layout::SMUSHING);
    }

    /// Whether this is a FIGlet or TOIlet font; decides the signature [`to_bytes`](Self::to_bytes) writes.
    pub fn format(&self) -> FigletFormat {
        self.format
    }

    pub fn set_format(&mut self, format: FigletFormat) {
        self.format = format;
    }

    /// Print direction from the font header: 0 = left-to-right, 1 = right-to-left.
    pub fn print_direction(&self) -> u8 {
        self.print_direction
    }

    /// The glyph for `ch`, if the font defines it.
    pub fn glyph(&self, ch: char) -> Option<&Glyph> {
        if let Some(g) = self.glyphs_overlay.get(&ch) {
            return Some(g);
        }
        let lazy = self.lazy.as_ref()?;
        let &idx = lazy.index.get(&ch)?;
        Some(lazy.cache[idx].get_or_init(|| decode_glyph(lazy, idx)))
    }

    /// Iterate over all defined FIGlet glyphs as (char, &Glyph), ordered by character.
    pub fn iter_glyphs(&self) -> impl Iterator<Item = (char, &Glyph)> {
        self.defined_chars()
            .into_iter()
            .filter_map(move |ch| self.glyph(ch).map(|g| (ch, g)))
    }

    fn defined_chars(&self) -> BTreeSet<char> {
        let mut chars: BTreeSet<char> = self.glyphs_overlay.keys().copied().collect();
        if let Some(lazy) = &self.lazy {
            chars.extend(lazy.index.keys().copied());
        }
        chars
    }

    pub fn load_file(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)?;
        Self::load(&bytes)
    }

    pub fn glyph_count(&self) -> usize {
        self.defined_chars().len()
    }

    /// Calculate the average width of defined glyphs (excluding space if undefined).
    /// Returns None if no glyphs are defined.
    pub(crate) fn spacing(&self) -> Option<usize> {
        // Prefer the precomputed hint for parsed fonts.
        if let Some(lazy) = &self.lazy
            && lazy.avg_width.is_some()
        {
            return lazy.avg_width;
        }
        // Fallback: compute from overlay glyphs.
        let mut total = 0usize;
        let mut count = 0usize;
        for g in self.glyphs_overlay.values() {
            total += g.width;
            count += 1;
        }
        total.checked_div(count)
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
        // ZIP archive: use the first entry that is a FIGlet or TOIlet font. Entry names
        // are not reliable; TOIlet's own zipped fonts store theirs as "-".
        if data.starts_with(b"PK\x03\x04") {
            let mut archive = ZipArchive::new(Cursor::new(data))
                .map_err(|e| FontError::Zip(format!("open error: {e}")))?;
            for i in 0..archive.len() {
                let mut file = archive
                    .by_index(i)
                    .map_err(|e| FontError::Zip(format!("entry error: {e}")))?;
                if file.is_dir() {
                    continue;
                }
                let mut buf = Vec::new();
                file.read_to_end(&mut buf)
                    .map_err(|e| FontError::Zip(format!("read error: {e}")))?;
                if FigletFormat::detect(&buf).is_some() {
                    return FigletFont::parse_bytes(Arc::<[u8]>::from(buf));
                }
            }
            return Err(FontError::ZipNoFlf);
        }
        FigletFont::parse_bytes(bytes)
    }

    fn parse_bytes(bytes: Arc<[u8]>) -> Result<Self> {
        let format =
            FigletFormat::detect(bytes.as_ref()).ok_or(FontError::FigletInvalidSignature)?;
        let encoding = match format {
            FigletFormat::Flf => Encoding::detect(bytes.as_ref()),
            FigletFormat::Tlf => {
                std::str::from_utf8(bytes.as_ref())?;
                Encoding::Utf8
            }
        };
        let line_ranges = compute_line_ranges(bytes.as_ref());
        if line_ranges.is_empty() {
            return Err(FontError::FigletMissingHeader);
        }

        let mut line_idx = 0usize;
        let header_range = line_ranges[line_idx].clone();
        let header_line = encoding.decode(&bytes[header_range]).into_owned();
        line_idx += 1;

        // The hard blank is the character right after the signature.
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
        if !(1..=u8::MAX as usize).contains(&height) {
            return Err(FontError::FigletHeightOutOfRange {
                height,
                max: u8::MAX as usize,
            });
        }
        let comment_count: usize = header_parts
            .get(5)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Layouts are optional hints; unreadable values fall back to full width.
        let old_layout: i32 = header_parts
            .get(4)
            .and_then(|s| s.parse().ok())
            .unwrap_or(-1);
        let print_direction: u8 = header_parts
            .get(6)
            .and_then(|s| s.parse().ok())
            .filter(|&d| d <= 1)
            .unwrap_or(0);
        let full_layout = match header_parts.get(7).and_then(|s| s.parse::<u32>().ok()) {
            Some(full) => full,
            None if old_layout == 0 => layout::KERNING,
            None if old_layout < 0 => 0,
            None => (old_layout as u32 & 31) | layout::SMUSHING,
        };

        let mut font = FigletFont::new("figlet");
        font.header = header_line.to_string();
        font.hard_blank = hard_blank;
        font.format = format;
        font.set_layout(full_layout);
        font.print_direction = print_direction;

        // Read comment lines
        for _ in 0..comment_count {
            if line_idx >= line_ranges.len() {
                return Err(FontError::FigletIncompleteComments);
            }
            let r = line_ranges[line_idx].clone();
            line_idx += 1;
            font.comments.push(encoding.decode(&bytes[r]).into_owned());
        }

        // Parse glyphs lazily: record the line slices of each glyph.
        let mut reader = GlyphReader {
            bytes: bytes.as_ref(),
            lines: &line_ranges,
            pos: line_idx,
            height,
            encoding,
            ansi: format == FigletFormat::Tlf,
        };
        let mut builder = LazyBuilder::default();

        for ch in ' '..='~' {
            let rows = reader.read_glyph().ok_or(FontError::FigletIncompleteChar)?;
            builder.add(ch, rows);
        }

        // Older fonts end after the required ASCII set.
        for ch in DEUTSCH {
            let Some(rows) = reader.read_glyph() else {
                break;
            };
            // Blank glyphs stay defined with zero width, as in figlet.
            builder.add(ch, rows);
        }

        // Code-tagged glyphs: a line starting with the character code, then the glyph.
        // Like figlet, stop at the first line that isn't a code tag.
        while let Some(code) = reader
            .next_line()
            .and_then(|tag| parse_code_tag(&encoding.decode(tag)))
        {
            let Some(rows) = reader.read_glyph() else {
                break;
            };
            // Negative codes are font-private and can't be typed.
            if let Some(ch) = u32::try_from(code).ok().and_then(char::from_u32) {
                builder.add(ch, rows);
            }
        }

        let cache: Arc<[OnceLock<Glyph>]> =
            builder.glyphs.iter().map(|_| OnceLock::new()).collect();
        font.lazy = Some(LazyFigletSource {
            bytes,
            encoding,
            ansi: format == FigletFormat::Tlf,
            hard_blank,
            glyph_lines: builder.glyph_lines,
            index: builder.index,
            glyphs: builder.glyphs,
            cache,
            avg_width: builder.sum_width.checked_div(builder.required),
        });

        Ok(font)
    }

    /// Add (or replace) the glyph for a Latin-1 character code.
    pub fn add_raw_char(&mut self, ch: u8, raw_lines: &[&str]) {
        self.add_char(char::from(ch), raw_lines);
    }

    /// Add (or replace) the glyph for `ch`, one string per row.
    /// Occurrences of [`hard_blank`](Self::hard_blank) become hard blanks.
    pub fn add_char(&mut self, ch: char, raw_lines: &[&str]) {
        let mut parts = Vec::new();
        let mut max_width = 0usize;
        for (row, line) in raw_lines.iter().enumerate() {
            if row > 0 {
                parts.push(GlyphPart::NewLine);
            }
            max_width = max_width.max(line.chars().count());
            for c in line.chars() {
                if c == self.hard_blank {
                    parts.push(GlyphPart::HardBlank);
                } else {
                    parts.push(GlyphPart::Char(c));
                }
            }
        }
        let glyph = Glyph {
            width: max_width,
            height: raw_lines.len(),
            parts,
        };
        self.glyphs_overlay.insert(ch, glyph);
    }

    pub fn has_char(&self, ch: char) -> bool {
        self.glyphs_overlay.contains_key(&ch)
            || self
                .lazy
                .as_ref()
                .is_some_and(|lazy| lazy.index.contains_key(&ch))
    }

    /// Serialize this font to bytes in the FIGlet or TOIlet format, see [`format`](Self::format).
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();

        // Determine max height from all glyphs
        let max_height = self.compute_max_height();

        // Write header line
        // Format: flf2a|tlf2a<hardblank> height baseline maxlen old_layout comment_count
        //         print_direction full_layout
        let comment_count = self.comments.len();
        let full_layout = self.full_layout;
        let old_layout = if full_layout & layout::SMUSHING != 0 && full_layout & layout::RULES != 0
        {
            (full_layout & layout::RULES) as i32
        } else if full_layout & (layout::SMUSHING | layout::KERNING) != 0 {
            0
        } else {
            -1
        };
        let header = format!(
            "{}{} {} {} {} {} {} {} {}\n",
            self.format.signature(),
            self.hard_blank,
            max_height,
            max_height,
            80,
            old_layout,
            comment_count,
            self.print_direction,
            full_layout
        );
        out.extend(header.as_bytes());

        // Write comment lines
        for comment in &self.comments {
            out.extend(comment.as_bytes());
            out.push(b'\n');
        }

        // Required ASCII glyphs.
        for ch in ' '..='~' {
            self.write_glyph_lines(&mut out, ch, max_height);
        }

        // The German glyphs are positional and must precede code-tagged glyphs;
        // missing ones are written as blank (zero-width) glyphs.
        let tagged: Vec<char> = self
            .defined_chars()
            .into_iter()
            .filter(|ch| !(' '..='~').contains(ch) && !DEUTSCH.contains(ch))
            .collect();
        if !tagged.is_empty() || DEUTSCH.iter().any(|&ch| self.has_char(ch)) {
            for ch in DEUTSCH {
                self.write_glyph_lines(&mut out, ch, max_height);
            }
        }

        // Hex tags are the form every FIGlet implementation understands.
        for ch in tagged {
            out.extend(format!("0x{:04X}\n", ch as u32).as_bytes());
            self.write_glyph_lines(&mut out, ch, max_height);
        }

        Ok(out)
    }

    fn compute_max_height(&self) -> usize {
        self.iter_glyphs()
            .map(|(_, g)| g.height)
            .max()
            .unwrap_or(1)
            .max(1)
    }

    fn write_glyph_lines(&self, out: &mut Vec<u8>, ch: char, max_height: usize) {
        if let Some(glyph) = self.glyph(ch) {
            // Each line with its last visible character.
            let mut lines: Vec<(String, Option<char>)> = Vec::new();
            let mut current = (String::new(), None);

            for part in &glyph.parts {
                let ch = match *part {
                    GlyphPart::NewLine => {
                        lines.push(std::mem::take(&mut current));
                        continue;
                    }
                    GlyphPart::HardBlank => self.hard_blank,
                    GlyphPart::Char(c) => c,
                    // Colours can only be stored in TOIlet fonts.
                    GlyphPart::AnsiChar { ch, fg, bg, blink } => {
                        if self.format == FigletFormat::Tlf {
                            current.0 += &sgr_sequence(fg, bg, blink);
                            current.0.push(ch);
                            current.0 += "\x1b[0m";
                            current.1 = Some(ch);
                            continue;
                        }
                        ch
                    }
                    _ => ' ',
                };
                current.0.push(ch);
                current.1 = Some(ch);
            }
            // Don't forget the last line if not empty
            if !current.0.is_empty() || lines.is_empty() {
                lines.push(current);
            }

            // Pad to max_height if needed
            while lines.len() < max_height {
                lines.push((String::new(), None));
            }

            // End marks are chosen per line; avoid one that the line itself ends with,
            // since readers strip the whole trailing run of the end mark.
            for (i, (line, last)) in lines.iter().enumerate() {
                let mark = if *last == Some('@') { '#' } else { '@' };
                out.extend(line.as_bytes());
                out.extend(
                    mark.to_string()
                        .repeat(if i == lines.len() - 1 { 2 } else { 1 })
                        .as_bytes(),
                );
                out.push(b'\n');
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

struct GlyphReader<'a> {
    bytes: &'a [u8],
    lines: &'a [Range<usize>],
    pos: usize,
    height: usize,
    encoding: Encoding,
    ansi: bool,
}

/// One glyph row: the content range (end marks removed) and its width in characters.
type Row = (Range<usize>, usize);

impl<'a> GlyphReader<'a> {
    fn next_line(&mut self) -> Option<&'a [u8]> {
        let r = self.lines.get(self.pos)?.clone();
        self.pos += 1;
        Some(&self.bytes[r])
    }

    /// Read the next `height` lines as a glyph; `None` if the file ends first.
    fn read_glyph(&mut self) -> Option<Vec<Row>> {
        let lines = self.lines.get(self.pos..self.pos + self.height)?;
        self.pos += self.height;
        Some(
            lines
                .iter()
                .map(|r| strip_end_marks(self.bytes, r.clone(), self.encoding, self.ansi))
                .collect(),
        )
    }
}

/// Remove trailing whitespace, then the run of the line's last character (its end mark),
/// like C figlet's `readfontchar`. With `ansi`, only visible characters count.
fn strip_end_marks(bytes: &[u8], line: Range<usize>, encoding: Encoding, ansi: bool) -> Row {
    let s = &bytes[line.clone()];
    let (end, width) = match encoding {
        Encoding::Latin1 => {
            let mut end = s.len();
            while end > 0 && is_line_space(char::from(s[end - 1])) {
                end -= 1;
            }
            if let Some(&mark) = s[..end].last() {
                while end > 0 && s[end - 1] == mark {
                    end -= 1;
                }
            }
            (end, end)
        }
        Encoding::Utf8 => {
            let mut cells = parse_line(std::str::from_utf8(s).unwrap_or_default(), ansi);
            while cells.last().is_some_and(|c| is_line_space(c.ch)) {
                cells.pop();
            }
            if let Some(mark) = cells.last().map(|c| c.ch) {
                while cells.last().is_some_and(|c| c.ch == mark) {
                    cells.pop();
                }
            }
            (cells.last().map_or(0, |c| c.bytes.end), cells.len())
        }
    };
    (line.start..line.start + end, width)
}

fn is_line_space(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\r' | '\n' | '\x0B' | '\x0C')
}

/// libcaca colour index (same order as the DOS palette) for each ANSI colour.
const ANSI_TO_DOS: [u8; 8] = [0, 4, 2, 6, 1, 5, 3, 7];

/// ANSI SGR state, interpreted like libcaca's importer, which TOIlet uses.
#[derive(Clone, Copy, Debug, Default)]
struct Sgr {
    fg: Option<u8>,
    bg: Option<u8>,
    bold: bool,
    blink: bool,
    negative: bool,
    concealed: bool,
}

impl Sgr {
    fn apply(&mut self, params: &str) {
        for code in params.split(';').map(|p| p.parse::<u32>().unwrap_or(0)) {
            match code {
                0 => *self = Sgr::default(),
                1 => self.bold = true,
                5 | 6 => self.blink = true,
                7 => self.negative = true,
                8 => self.concealed = true,
                22 => self.bold = false,
                25 => self.blink = false,
                27 => self.negative = false,
                28 => self.concealed = false,
                30..=37 => self.fg = Some(ANSI_TO_DOS[code as usize - 30]),
                39 => self.fg = None,
                40..=47 => self.bg = Some(ANSI_TO_DOS[code as usize - 40]),
                49 => self.bg = None,
                90..=97 => self.fg = Some(ANSI_TO_DOS[code as usize - 90] + 8),
                100..=107 => self.bg = Some(ANSI_TO_DOS[code as usize - 100] + 8),
                _ => {}
            }
        }
    }

    /// Glyph part for `ch` drawn with this state; default colours become light gray on black.
    fn part(self, ch: char) -> GlyphPart {
        if self.concealed {
            return GlyphPart::Char(ch);
        }
        let (mut fg, bg) = if self.negative {
            (self.bg, self.fg)
        } else {
            (self.fg, self.bg)
        };
        if self.bold {
            fg = Some(fg.map_or(15, |c| c | 8));
        }
        if fg.is_none() && bg.is_none() && !self.blink {
            return GlyphPart::Char(ch);
        }
        GlyphPart::AnsiChar {
            ch,
            fg: fg.unwrap_or(7),
            bg: bg.unwrap_or(0),
            blink: self.blink,
        }
    }
}

/// SGR sequence reproducing a colour cell (DOS palette indices).
fn sgr_sequence(fg: u8, bg: u8, blink: bool) -> String {
    // The ANSI <-> DOS colour mapping is its own inverse.
    let code = |c: u8, base: u8, bright: u8| {
        let c = c & 15;
        let ansi = ANSI_TO_DOS[usize::from(c & 7)];
        if c < 8 { base + ansi } else { bright + ansi }
    };
    format!(
        "\x1b[0;{};{}{}m",
        code(fg, 30, 90),
        code(bg, 40, 100),
        if blink { ";5" } else { "" }
    )
}

struct LineCell {
    bytes: Range<usize>,
    ch: char,
    sgr: Sgr,
}

/// The visible characters of a glyph line. With `ansi`, escape sequences are consumed
/// and SGR colour codes tracked; the state starts fresh on every line.
fn parse_line(text: &str, ansi: bool) -> Vec<LineCell> {
    let mut cells = Vec::new();
    let mut sgr = Sgr::default();
    let mut chars = text.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        if !(ansi && ch == '\x1b') {
            cells.push(LineCell {
                bytes: i..i + ch.len_utf8(),
                ch,
                sgr,
            });
            continue;
        }
        if chars.next_if(|&(_, c)| c == '[').is_none() {
            // Two-character escape sequence.
            chars.next();
            continue;
        }
        let start = chars.peek().map_or(text.len(), |&(j, _)| j);
        for (j, c) in chars.by_ref() {
            if ('\x40'..='\x7e').contains(&c) {
                if c == 'm' {
                    sgr.apply(&text[start..j]);
                }
                break;
            }
        }
    }
    cells
}

/// Parse a code tag like C's `%li`: decimal, `0x` hex or `0`-prefixed octal, optionally signed.
fn parse_code_tag(line: &str) -> Option<i64> {
    let token = line.split_whitespace().next()?;
    let (negative, digits) = match token.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, token.strip_prefix('+').unwrap_or(token)),
    };
    let (digits, radix) = if let Some(hex) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        (hex, 16)
    } else if digits.len() > 1 && digits.starts_with('0') {
        (&digits[1..], 8)
    } else {
        (digits, 10)
    };
    if !digits.chars().next()?.is_digit(radix) {
        return None;
    }
    let value = i64::from_str_radix(digits, radix).ok()?;
    Some(if negative { -value } else { value })
}

#[derive(Default)]
struct LazyBuilder {
    glyph_lines: Vec<Range<usize>>,
    index: BTreeMap<char, usize>,
    glyphs: Vec<Range<usize>>,
    sum_width: usize,
    // Number of required ASCII glyphs, which the spacing hint is based on.
    required: usize,
}

impl LazyBuilder {
    /// Later definitions of the same character replace earlier ones, as in C figlet.
    fn add(&mut self, ch: char, rows: Vec<Row>) {
        if (' '..='~').contains(&ch) {
            self.sum_width += rows.iter().map(|(_, w)| *w).max().unwrap_or(0);
            self.required += 1;
        }
        let start = self.glyph_lines.len();
        self.glyph_lines.extend(rows.into_iter().map(|(r, _)| r));
        self.index.insert(ch, self.glyphs.len());
        self.glyphs.push(start..self.glyph_lines.len());
    }
}

fn decode_glyph(lazy: &LazyFigletSource, idx: usize) -> Glyph {
    let rows = &lazy.glyph_lines[lazy.glyphs[idx].clone()];
    let mut parts = Vec::new();
    let mut max_width = 0usize;

    for (row, r) in rows.iter().enumerate() {
        if row > 0 {
            parts.push(GlyphPart::NewLine);
        }
        let line = lazy.encoding.decode(&lazy.bytes[r.clone()]);
        let cells = parse_line(&line, lazy.ansi);
        for cell in &cells {
            parts.push(if cell.ch == lazy.hard_blank {
                GlyphPart::HardBlank
            } else {
                cell.sgr.part(cell.ch)
            });
        }
        max_width = max_width.max(cells.len());
    }

    Glyph {
        width: max_width,
        height: rows.len(),
        parts,
    }
}
