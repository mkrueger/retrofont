//! Text layout: composing glyphs into lines with FIGlet-style kerning and
//! smushing, multi-line text, word wrapping and justification.

use std::collections::VecDeque;
use std::rc::Rc;

use crate::{
    Cell, Font, FontError, FontTarget, Result,
    figlet::layout,
    glyph::{Glyph, RenderOptions, ResolvedPart},
};

/// Horizontal alignment of rendered lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Justify {
    #[default]
    Left,
    Center,
    Right,
}

/// How neighbouring glyphs are placed next to each other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Layout {
    /// Use the font's own layout (the FIGlet header; full width for TDF fonts).
    #[default]
    FontDefault,
    /// Every glyph keeps its full width.
    FullWidth,
    /// Glyphs are moved together until they touch.
    Kerning,
    /// Glyphs overlap by one column where the font's smushing rules allow it.
    /// Fonts without rules use universal smushing.
    Smushing,
}

/// What to do with characters the font has no glyph for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MissingGlyph {
    /// Fail with [`FontError::UnknownChar`].
    #[default]
    Error,
    /// Leave the character out.
    Skip,
}

/// Options for rendering a whole string with [`Font::render_str`].
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextOptions {
    pub render: RenderOptions,
    pub layout: Layout,
    pub justify: Justify,
    /// Maximum line width in cells. Longer lines wrap at the last space, or
    /// mid-word when a single word does not fit. `None` disables wrapping.
    pub max_width: Option<usize>,
    pub missing: MissingGlyph,
}

impl From<RenderOptions> for TextOptions {
    fn from(render: RenderOptions) -> Self {
        Self {
            render,
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Slot {
    /// Transparent cell.
    Empty,
    Cell(Cell),
    /// Never removed by kerning; `None` is drawn transparently.
    HardBlank(Option<Cell>),
}

impl Slot {
    fn is_blank(self) -> bool {
        match self {
            Slot::Empty => true,
            Slot::Cell(c) => c.ch == ' ' && c.bg.is_none(),
            Slot::HardBlank(_) => false,
        }
    }

    fn is_transparent(self) -> bool {
        matches!(self, Slot::Empty | Slot::HardBlank(None))
    }
}

struct GlyphRows {
    width: usize,
    rows: Vec<Vec<Slot>>,
}

impl GlyphRows {
    fn from_glyph(glyph: &Glyph, options: &RenderOptions) -> Self {
        let mut rows = vec![Vec::new()];
        for part in &glyph.parts {
            let slot = match part.resolve(options) {
                ResolvedPart::NewLine => {
                    rows.push(Vec::new());
                    continue;
                }
                ResolvedPart::Nothing => continue,
                ResolvedPart::Skip => Slot::Empty,
                ResolvedPart::Draw(cell) => Slot::Cell(cell),
                ResolvedPart::HardBlank(cell) => Slot::HardBlank(Some(cell)),
            };
            if let Some(row) = rows.last_mut() {
                row.push(slot);
            }
        }
        while rows.len() > glyph.height.max(1) && rows.last().is_some_and(Vec::is_empty) {
            rows.pop();
        }
        let width = rows.iter().map(Vec::len).max().unwrap_or(0);
        for row in &mut rows {
            row.resize(width, Slot::Empty);
        }
        Self { width, rows }
    }

    fn slot(&self, row: usize, col: usize) -> Slot {
        self.rows
            .get(row)
            .and_then(|r| r.get(col))
            .copied()
            .unwrap_or(Slot::Empty)
    }
}

/// One output line of composed glyphs. All rows are `width` cells wide.
#[derive(Clone, Default)]
struct Line {
    rows: Vec<Vec<Slot>>,
    width: usize,
    prev_glyph_width: usize,
}

impl Line {
    fn append(&mut self, glyph: &GlyphRows, mode: u32) {
        while self.rows.len() < glyph.rows.len() {
            self.rows.push(vec![Slot::Empty; self.width]);
        }
        let amount = self.smush_amount(glyph, mode);
        for (r, row) in self.rows.iter_mut().enumerate() {
            for k in 0..amount {
                // Overlap that would fall left of the line start is dropped (always blank).
                if self.width + k < amount {
                    continue;
                }
                let col = self.width + k - amount;
                let right = glyph.slot(r, k);
                row[col] = smush(row[col], right, mode, self.prev_glyph_width, glyph.width)
                    .unwrap_or(right);
            }
            row.extend((amount..glyph.width).map(|k| glyph.slot(r, k)));
        }
        self.width = self.width + glyph.width - amount;
        self.prev_glyph_width = glyph.width;
    }

    /// Number of columns the next glyph can be moved left (port of FIGlet's `smushamt`).
    fn smush_amount(&self, glyph: &GlyphRows, mode: u32) -> usize {
        if mode & (layout::SMUSHING | layout::KERNING) == 0 {
            return 0;
        }
        let mut max = glyph.width;
        for (r, line) in self.rows.iter().enumerate() {
            let (line_bound, left) = match line.iter().rposition(|s| !s.is_blank()) {
                Some(i) => (i, Some(line[i])),
                None => (0, None),
            };
            let (char_bound, right) = match glyph
                .rows
                .get(r)
                .and_then(|g| g.iter().position(|s| !s.is_blank()).map(|i| (i, g[i])))
            {
                Some((i, s)) => (i, Some(s)),
                None => (glyph.width, None),
            };
            let mut amount = char_bound as isize + line.len() as isize - 1 - line_bound as isize;
            match (left, right) {
                (None, _) => amount += 1,
                (Some(l), Some(r))
                    if smush(l, r, mode, self.prev_glyph_width, glyph.width).is_some() =>
                {
                    amount += 1
                }
                _ => {}
            }
            max = max.min(amount.max(0) as usize);
        }
        max
    }
}

/// Merge two overlapping cells (port of FIGlet's `smushem`). `None` means they can't be merged.
fn smush(
    left: Slot,
    right: Slot,
    mode: u32,
    left_width: usize,
    right_width: usize,
) -> Option<Slot> {
    if left.is_blank() {
        return Some(right);
    }
    if right.is_blank() {
        return Some(left);
    }
    if left_width < 2 || right_width < 2 || mode & layout::SMUSHING == 0 {
        return None;
    }
    let left_hb = matches!(left, Slot::HardBlank(_));
    let right_hb = matches!(right, Slot::HardBlank(_));
    if mode & layout::RULES == 0 {
        // Universal smushing: the right glyph wins, except over hard blanks.
        return Some(if right_hb { left } else { right });
    }
    if left_hb || right_hb {
        return (mode & layout::HARDBLANK != 0 && left_hb && right_hb).then_some(left);
    }
    let (Slot::Cell(lc), Slot::Cell(rc)) = (left, right) else {
        return None;
    };
    let (a, b) = (lc.ch, rc.ch);
    let with_char = |ch| Some(Slot::Cell(Cell { ch, ..rc }));

    if mode & layout::EQUAL != 0 && a == b {
        return Some(left);
    }
    if mode & layout::UNDERSCORE != 0 {
        const REPLACERS: &str = "|/\\[]{}()<>";
        if a == '_' && REPLACERS.contains(b) {
            return Some(right);
        }
        if b == '_' && REPLACERS.contains(a) {
            return Some(left);
        }
    }
    if mode & layout::HIERARCHY != 0 {
        const CLASSES: [&str; 6] = ["|", "/\\", "[]", "{}", "()", "<>"];
        let class = |c| CLASSES.iter().position(|s| s.contains(c));
        if let (Some(ca), Some(cb)) = (class(a), class(b))
            && ca != cb
        {
            return Some(if ca > cb { left } else { right });
        }
    }
    if mode & layout::OPPOSITE_PAIR != 0
        && matches!(
            (a, b),
            ('[', ']') | (']', '[') | ('{', '}') | ('}', '{') | ('(', ')') | (')', '(')
        )
    {
        return with_char('|');
    }
    if mode & layout::BIG_X != 0 {
        match (a, b) {
            ('/', '\\') => return with_char('|'),
            ('\\', '/') => return with_char('Y'),
            ('>', '<') => return with_char('X'),
            _ => {}
        }
    }
    None
}

fn layout_mode(font: &Font, layout_choice: Layout) -> u32 {
    let font_bits = match font {
        Font::Figlet(f) => f.layout(),
        Font::Tdf(_) => 0,
    };
    match layout_choice {
        Layout::FontDefault => font_bits,
        Layout::FullWidth => 0,
        Layout::Kerning => layout::KERNING,
        Layout::Smushing => (font_bits & layout::RULES) | layout::SMUSHING,
    }
}

fn glyph_rows(font: &Font, ch: char, options: &TextOptions) -> Result<Option<Rc<GlyphRows>>> {
    if let Some(glyph) = font.resolve_char(ch).and_then(|c| font.glyph(c)) {
        return Ok(Some(Rc::new(GlyphRows::from_glyph(glyph, &options.render))));
    }
    if ch == ' ' {
        // Hard blanks keep the gap between words from being kerned away.
        let width = font.spacing().unwrap_or(1);
        return Ok(Some(Rc::new(GlyphRows {
            width,
            rows: vec![vec![Slot::HardBlank(None); width]],
        })));
    }
    match options.missing {
        MissingGlyph::Error => Err(FontError::UnknownChar(ch)),
        MissingGlyph::Skip => Ok(None),
    }
}

fn compose(items: &[(char, Rc<GlyphRows>)], mode: u32) -> Line {
    let mut line = Line::default();
    for (_, glyph) in items {
        line.append(glyph, mode);
    }
    line
}

fn layout_paragraph(
    font: &Font,
    text: &str,
    options: &TextOptions,
    mode: u32,
    out: &mut Vec<Line>,
) -> Result<()> {
    let mut line = Line::default();
    let mut items: Vec<(char, Rc<GlyphRows>)> = Vec::new();
    let mut queue: VecDeque<char> = text.chars().collect();
    let mut wrapped = false;

    while let Some(ch) = queue.pop_front() {
        if wrapped && ch == ' ' && items.is_empty() {
            continue;
        }
        let Some(glyph) = glyph_rows(font, ch, options)? else {
            continue;
        };
        if let Some(max_width) = options.max_width
            && !items.is_empty()
        {
            let mut candidate = line.clone();
            candidate.append(&glyph, mode);
            if candidate.width <= max_width {
                line = candidate;
                items.push((ch, glyph));
                continue;
            }
            wrapped = true;
            if ch == ' ' {
                out.push(std::mem::take(&mut line));
                items.clear();
                continue;
            }
            // Break at the last space, carrying the partial word over to the next line.
            let head_end = items
                .iter()
                .rposition(|(c, _)| *c == ' ')
                .and_then(|space| items[..space].iter().rposition(|(c, _)| *c != ' '));
            if let Some(last) = head_end {
                let space = last
                    + 1
                    + items[last + 1..]
                        .iter()
                        .take_while(|(c, _)| *c == ' ')
                        .count();
                queue.push_front(ch);
                for (c, _) in items[space..].iter().rev() {
                    queue.push_front(*c);
                }
                out.push(compose(&items[..=last], mode));
                items.clear();
                line = Line::default();
                continue;
            }
            out.push(std::mem::take(&mut line));
            items.clear();
        }
        line.append(&glyph, mode);
        items.push((ch, glyph));
    }
    out.push(line);
    Ok(())
}

fn layout_text(font: &Font, text: &str, options: &TextOptions) -> Result<Vec<Line>> {
    let mode = layout_mode(font, options.layout);
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let paragraph = paragraph.strip_suffix('\r').unwrap_or(paragraph);
        layout_paragraph(font, paragraph, options, mode, &mut lines)?;
    }
    let empty_height = font.max_height();
    for line in &mut lines {
        if line.rows.is_empty() {
            line.rows = vec![Vec::new(); empty_height];
        }
    }
    Ok(lines)
}

impl Font {
    /// Render a whole string, laying glyphs out according to `options`.
    ///
    /// `'\n'` starts a new line of glyphs. Rows are emitted top to bottom and
    /// separated by [`FontTarget::next_line`]; transparent cells use
    /// [`FontTarget::skip`] and trailing transparent cells are omitted.
    ///
    /// ```
    /// use retrofont::{Font, TextOptions, figlet::FigletFont, test_support::MemoryBufferTarget};
    ///
    /// let mut fig = FigletFont::new("demo");
    /// fig.add_raw_char(b'H', &["H H", "HHH", "H H"]);
    /// fig.add_raw_char(b'I', &["III", " I ", "III"]);
    /// let font = Font::Figlet(Box::new(fig));
    ///
    /// let mut target = MemoryBufferTarget::new();
    /// font.render_str(&mut target, "HI\nIH", &TextOptions::default()).unwrap();
    /// assert_eq!(target.lines.len(), 6);
    /// ```
    pub fn render_str<T: FontTarget>(
        &self,
        target: &mut T,
        text: &str,
        options: &TextOptions,
    ) -> Result<()> {
        fn target_err<E: std::fmt::Display>(e: E) -> FontError {
            FontError::Target(e.to_string())
        }
        let lines = layout_text(self, text, options)?;
        let total_width = options
            .max_width
            .unwrap_or_else(|| lines.iter().map(|l| l.width).max().unwrap_or(0));
        let mut first_row = true;
        for line in &lines {
            let free = total_width.saturating_sub(line.width);
            let pad = match options.justify {
                Justify::Left => 0,
                // Rounds up like FIGlet does.
                Justify::Center => free.div_ceil(2),
                Justify::Right => free,
            };
            for row in &line.rows {
                if !first_row {
                    target.next_line().map_err(target_err)?;
                }
                first_row = false;
                let Some(end) = row.iter().rposition(|s| !s.is_transparent()) else {
                    continue;
                };
                for _ in 0..pad {
                    target.skip().map_err(target_err)?;
                }
                for slot in &row[..=end] {
                    match *slot {
                        Slot::Cell(cell) | Slot::HardBlank(Some(cell)) => {
                            target.draw(cell).map_err(target_err)?
                        }
                        Slot::Empty | Slot::HardBlank(None) => target.skip().map_err(target_err)?,
                    }
                }
            }
        }
        Ok(())
    }

    /// Size `(width, height)` in cells that [`Font::render_str`] would produce,
    /// before justification padding.
    pub fn measure(&self, text: &str, options: &TextOptions) -> Result<(usize, usize)> {
        let lines = layout_text(self, text, options)?;
        let width = lines.iter().map(|l| l.width).max().unwrap_or(0);
        let height = lines.iter().map(|l| l.rows.len()).sum();
        Ok((width, height))
    }
}
