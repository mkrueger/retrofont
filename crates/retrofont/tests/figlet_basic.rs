use retrofont::{figlet::FigletFont, test_support::BufferTarget, Font, RenderMode};

fn lines(buf: &BufferTarget) -> Vec<String> {
    buf.lines
        .iter()
        .map(|l| l.iter().map(|c| c.ch).collect())
        .collect()
}

#[test]
fn figlet_basic_render() {
    let mut font: FigletFont = FigletFont::new("FIG");
    font.add_raw_char(b'A', &["AA", "AA"]);
    let mut target = BufferTarget::new();
    font.render_char(&mut target, 'A', RenderMode::Display)
        .unwrap();
    assert_eq!(lines(&target), vec!["AA", "AA"]);
}

#[test]
fn figlet_newline_height() {
    let mut font = FigletFont::new("FIG2");
    font.add_raw_char(b'B', &["B", "B", "B"]);
    let mut target = BufferTarget::new();
    font.render_char(&mut target, 'B', RenderMode::Display)
        .unwrap();
    assert_eq!(lines(&target).len(), 3);
}

#[test]
fn figlet_edit_equals_display() {
    let mut font = FigletFont::new("FIG3");
    font.add_raw_char(b'C', &["C@", "CO"]);
    let mut d = BufferTarget::new();
    font.render_char(&mut d, 'C', RenderMode::Display).unwrap();
    let mut e = BufferTarget::new();
    font.render_char(&mut e, 'C', RenderMode::Edit).unwrap();
    assert_eq!(lines(&d), lines(&e));
}
