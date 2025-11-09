use retrofont::{figlet::FigletFont, test_support::MemoryBufferTarget, Font, RenderOptions};

fn lines(buf: &MemoryBufferTarget) -> Vec<String> {
    buf.lines
        .iter()
        .map(|l| l.iter().map(|c| c.ch).collect())
        .collect()
}

#[test]
fn figlet_basic_render() {
    let mut font: FigletFont = FigletFont::new("FIG");
    font.add_raw_char(b'A', &["AA", "AA"]);
    let mut target = MemoryBufferTarget::new();
    Font::Figlet(font)
        .render_glyph(&mut target, 'A', &RenderOptions::default())
        .unwrap();
    assert_eq!(lines(&target), vec!["AA", "AA"]);
}

#[test]
fn figlet_newline_height() {
    let mut font = FigletFont::new("FIG2");
    font.add_raw_char(b'B', &["B", "B", "B"]);
    let mut target = MemoryBufferTarget::new();
    Font::Figlet(font)
        .render_glyph(&mut target, 'B', &RenderOptions::default())
        .unwrap();
    assert_eq!(lines(&target).len(), 3);
}

#[test]
fn figlet_edit_equals_display() {
    let mut font = FigletFont::new("FIG3");
    font.add_raw_char(b'C', &["C@", "CO"]);
    let mut d = MemoryBufferTarget::new();
    Font::Figlet(font.clone())
        .render_glyph(&mut d, 'C', &RenderOptions::default())
        .unwrap();
    let mut e = MemoryBufferTarget::new();
    Font::Figlet(font)
        .render_glyph(&mut e, 'C', &RenderOptions::edit())
        .unwrap();
    assert_eq!(lines(&d), lines(&e));
}
