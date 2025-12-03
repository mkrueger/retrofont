use retrofont::{figlet::FigletFont, Cell, Font, FontTarget, RenderMode};

/// Simple console renderer that places glyphs horizontally
struct SimpleRenderer {
    lines: Vec<Vec<Cell>>,
    cur_line: usize,
    cur_x: usize,
}

impl SimpleRenderer {
    fn new() -> Self {
        Self {
            lines: vec![Vec::new()],
            cur_line: 0,
            cur_x: 0,
        }
    }

    fn next_char(&mut self) {
        self.cur_x = self.lines.iter().map(|line| line.len()).max().unwrap_or(0);
        self.cur_line = 0;
    }

    fn print(&self) {
        for line in &self.lines {
            for cell in line {
                print!("{}", cell.ch);
            }
            println!();
        }
    }
}

impl FontTarget for SimpleRenderer {
    type Error = std::fmt::Error;

    fn draw(&mut self, cell: Cell) -> std::result::Result<(), Self::Error> {
        while self.cur_line >= self.lines.len() {
            self.lines.push(Vec::new());
        }
        while self.lines[self.cur_line].len() < self.cur_x {
            self.lines[self.cur_line].push(Cell::new(' ', None, None));
        }
        self.lines[self.cur_line].push(cell);
        Ok(())
    }

    fn next_line(&mut self) -> std::result::Result<(), Self::Error> {
        self.cur_line += 1;
        Ok(())
    }
}

fn main() {
    // Create a simple test font
    let mut font = FigletFont::new("Test");
    font.add_raw_char(b'H', &["HH  HH", "HH  HH", "HHHHHH", "HH  HH", "HH  HH"]);
    font.add_raw_char(b'i', &["  ii  ", "      ", "  ii  ", "  ii  ", "  ii  "]);

    let font_enum = Font::Figlet(font);
    let mut renderer = SimpleRenderer::new();

    // Render "Hi" horizontally
    println!("Rendering 'Hi' horizontally:");
    println!();

    for ch in "Hi".chars() {
        font_enum.render_glyph(&mut renderer, ch, RenderMode::Display).unwrap();
        renderer.next_char();
    }

    renderer.print();

    println!();
    println!("Notice how 'H' and 'i' are side-by-side, not stacked vertically!");
}
