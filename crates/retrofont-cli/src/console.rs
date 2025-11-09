use retrofont::{Cell, Font, FontTarget, RenderMode, Result};

/// DOS default palette (VGA text mode colors)
const DOS_PALETTE: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00), // 0: black
    (0x00, 0x00, 0xAA), // 1: blue
    (0x00, 0xAA, 0x00), // 2: green
    (0x00, 0xAA, 0xAA), // 3: cyan
    (0xAA, 0x00, 0x00), // 4: red
    (0xAA, 0x00, 0xAA), // 5: magenta
    (0xAA, 0x55, 0x00), // 6: brown
    (0xAA, 0xAA, 0xAA), // 7: light gray
    (0x55, 0x55, 0x55), // 8: dark gray
    (0x55, 0x55, 0xFF), // 9: light blue
    (0x55, 0xFF, 0x55), // 10: light green
    (0x55, 0xFF, 0xFF), // 11: light cyan
    (0xFF, 0x55, 0x55), // 12: light red
    (0xFF, 0x55, 0xFF), // 13: light magenta
    (0xFF, 0xFF, 0x55), // 14: yellow
    (0xFF, 0xFF, 0xFF), // 15: white
];

/// A rendering buffer that accumulates glyphs horizontally.
///
/// When rendering text like "Hello", each character's glyph is multi-line (e.g., ASCII art).
/// This renderer places glyphs side-by-side horizontally by maintaining an X position
/// that advances after each character, while next_line() moves to the next row within
/// the current glyph without changing X.
pub struct ConsoleRenderer {
    lines: Vec<Vec<Cell>>,
    cur_line: usize,
    cur_x: usize,
}

impl ConsoleRenderer {
    pub fn new() -> Self {
        Self {
            lines: vec![Vec::new()],
            cur_line: 0,
            cur_x: 0,
        }
    }

    /// Reset to the next character position (advances X, resets Y to 0)
    pub fn next_char(&mut self) {
        self.cur_x = self.lines.iter().map(|line| line.len()).max().unwrap_or(0);
        self.cur_line = 0;
    }

    pub fn into_ansi_string(self) -> String {
        let mut out = String::new();
        for (li, line) in self.lines.iter().enumerate() {
            if li > 0 {
                out.push('\n');
            }
            for cell in line {
                let ch = cell.ch;
                match (cell.fg, cell.bg) {
                    (None, None) => {
                        out.push(ch);
                    }
                    (Some(fg), Some(bg)) => {
                        let (fr, fg_g, fb) = DOS_PALETTE[fg as usize % 16];
                        let (br, bg_g, bb) = DOS_PALETTE[bg as usize % 16];
                        out.push_str(&format!(
                            "\x1B[38;2;{};{};{}m\x1B[48;2;{};{};{}m{}",
                            fr, fg_g, fb, br, bg_g, bb, ch
                        ));
                    }
                    (Some(fg), None) => {
                        let (r, g, b) = DOS_PALETTE[fg as usize % 16];
                        out.push_str(&format!("\x1B[38;2;{};{};{}m{}", r, g, b, ch));
                    }
                    (None, Some(bg)) => {
                        let (r, g, b) = DOS_PALETTE[bg as usize % 16];
                        out.push_str(&format!("\x1B[48;2;{};{};{}m{}", r, g, b, ch));
                    }
                }
            }
            out.push_str("\x1B[0m");
        }
        out
    }
}

impl FontTarget for ConsoleRenderer {
    type Error = std::fmt::Error;

    fn draw(&mut self, cell: Cell) -> std::result::Result<(), Self::Error> {
        // Ensure we have enough lines
        while self.cur_line >= self.lines.len() {
            self.lines.push(Vec::new());
        }

        // Extend the current line to cur_x if needed (fill with spaces)
        while self.lines[self.cur_line].len() < self.cur_x {
            self.lines[self.cur_line].push(Cell::new(' ', None, None, false));
        }
        // Add the cell at the current position
        self.lines[self.cur_line].push(cell);
        Ok(())
    }

    fn next_line(&mut self) -> std::result::Result<(), Self::Error> {
        // Move to the next line within the current glyph (preserves X position)
        self.cur_line += 1;
        Ok(())
    }
}

/// Convenience: render text into an ANSI colored String.
/// Characters are placed horizontally, with each glyph rendered side-by-side.
pub fn render_to_ansi(font: &Font, text: &str, style: RenderMode) -> Result<String> {
    let mut renderer = ConsoleRenderer::new();

    for ch in text.chars() {
        font.render_char(&mut renderer, ch, style)?;
        renderer.next_char();
    }

    Ok(renderer.into_ansi_string())
}
