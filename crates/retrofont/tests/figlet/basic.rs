use retrofont::{figlet::FigletFont, types::{FontGlyph, FontType, RenderMode}, MemoryBufferTarget, Font};

fn lines(buf: &MemoryBufferTarget) -> Vec<String> { buf.lines.iter().map(|l| l.iter().map(|c| c.ch).collect()).collect() }

#[test]
fn figlet_basic_render() {
    let mut font = FigletFont::new("FIG");
    font.add_raw_char(b'A', &["AA","AA"]);
    let mut target = MemoryBufferTarget::new();
    font.render_glyph(&mut target, 'A', RenderMode::Display, 7, 0).unwrap();
    assert_eq!(lines(&target), vec!["AA".to_string(), "AA".to_string()]);
}

#[test]
fn figlet_newline_parsing() {
    let mut font = FigletFont::new("FIG2");
    font.add_raw_char(b'B', &["B","B","B"]);
    let g = font.glyphs[b'B' as usize].as_ref().unwrap();
    assert!(g.data.contains(&b'\n'));
    assert_eq!(g.height, 3);
}

#[test]
fn figlet_render_edit_mode_same_as_display() {
    let mut font = FigletFont::new("FIG3");
    font.add_raw_char(b'C', &["C@","CO"]); // '@','O' have no special treatment in figlet currently
    let mut d = MemoryBufferTarget::new();
    let mut e = MemoryBufferTarget::new();
    font.render_glyph(&mut d, 'C', RenderMode::Display, 7, 0).unwrap();
    font.render_glyph(&mut e, 'C', RenderMode::Edit, 7, 0).unwrap();
    assert_eq!(lines(&d), lines(&e));
}