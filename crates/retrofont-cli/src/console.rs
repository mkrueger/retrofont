use retrofont::{Cell, Font, FontTarget, RenderMode, Result};

pub struct ConsoleRenderer {
    lines: Vec<Vec<Cell>>,
    cur: usize,
}

impl ConsoleRenderer {
    pub fn new() -> Self {
        Self {
            lines: vec![Vec::new()],
            cur: 0,
        }
    }

    pub fn into_ansi_string(self) -> String {
        let mut out = String::new();
        for (li, line) in self.lines.iter().enumerate() {
            if li > 0 {
                out.push('\n');
            }
            for cell in line {
                match (cell.fg, cell.bg) {
                    (None, None) => {
                        out.push(cell.ch);
                    }
                    (Some(fg), Some(bg)) => {
                        out.push_str(&format!("\x1B[38;5;{}m\x1B[48;5;{}m{}", fg, bg, cell.ch));
                    }
                    (Some(fg), None) => {
                        out.push_str(&format!("\x1B[38;5;{}m{}", fg, cell.ch));
                    }
                    (None, Some(bg)) => {
                        out.push_str(&format!("\x1B[48;5;{}m{}", bg, cell.ch));
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
        if self.cur >= self.lines.len() {
            self.lines.push(Vec::new());
        }
        self.lines[self.cur].push(cell);
        Ok(())
    }
    fn next_line(&mut self) -> std::result::Result<(), Self::Error> {
        self.cur += 1;
        if self.cur >= self.lines.len() {
            self.lines.push(Vec::new());
        }
        Ok(())
    }
}

/// Convenience: render text into an ANSI colored String.
pub fn render_to_ansi<F: Font>(font: &F, text: &str, style: RenderMode) -> Result<String> {
    let mut renderer = ConsoleRenderer::new();
    font.render_str(&mut renderer, text, style)?;
    Ok(renderer.into_ansi_string())
}
