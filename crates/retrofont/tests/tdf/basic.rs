use retrofont::{
    tdf::{FontType, TdfFont},
    test_support::BufferTarget,
    Font, Glyph, GlyphPart, RenderMode,
};

// Helper: collect rendered lines into Vec<String>
fn lines_to_strings(buf: &BufferTarget) -> Vec<String> {
    buf.lines
        .iter()
        .map(|l| l.iter().map(|c| c.ch).collect())
        .collect()
}

#[test]
fn tdf_round_trip_block_single_glyph() {
    let mut font = TdfFont::new("TEST", FontType::Block, 0);
    let glyph = Glyph {
        width: 4,
        height: 2,
        parts: vec![
            GlyphPart::Char('A'),
            GlyphPart::Char('B'),
            GlyphPart::NewLine,
            GlyphPart::Char('C'),
            GlyphPart::Char('D'),
        ],
    };
    font.add_glyph(b'A', glyph);
    let bytes = font.as_tdf_bytes().expect("serialize");
    let parsed = TdfFont::from_bytes(&bytes).expect("parse");
    assert_eq!(parsed.len(), 1);
    let p = &parsed[0];
    assert_eq!(p.name, "TEST");
    assert_eq!(p.font_type(), FontType::Block);
    // Validate via render
    let mut target = BufferTarget::new();
    Font::Tdf(p.clone())
        .render_char(&mut target, 'A', RenderMode::Display)
        .unwrap();
    let lines = lines_to_strings(&target);
    assert_eq!(lines, vec!["AB", "CD"]);
}

#[test]
fn tdf_round_trip_color_attributes() {
    let mut font = TdfFont::new("COLOR", FontType::Color, 0);
    let glyph = Glyph {
        width: 3,
        height: 2,
        parts: vec![
            GlyphPart::Colored {
                ch: 'A',
                fg: 0x1,
                bg: 0xE,
            },
            GlyphPart::NewLine,
            GlyphPart::EndMarker,
            GlyphPart::Colored {
                ch: 'B',
                fg: 0x2,
                bg: 0xF,
            },
        ],
    };
    font.add_glyph(b'Z', glyph);
    let bytes = font.as_tdf_bytes().unwrap();
    let parsed = TdfFont::from_bytes(&bytes).unwrap();
    assert_eq!(parsed.len(), 1);
    // Validate via render
    let mut target = BufferTarget::new();
    Font::Tdf(parsed[0].clone())
        .render_char(&mut target, 'Z', RenderMode::Display)
        .unwrap();
    assert!(lines_to_strings(&target).len() > 0);
}

#[test]
fn tdf_render_block_multiline() {
    let mut font = TdfFont::new("BLK", FontType::Block, 0);
    let glyph = Glyph {
        width: 2,
        height: 2,
        parts: vec![
            GlyphPart::Char('X'),
            GlyphPart::Char('Y'),
            GlyphPart::NewLine,
            GlyphPart::Char('Z'),
            GlyphPart::Char('W'),
        ],
    };
    font.add_glyph(b'X', glyph);
    let mut target = BufferTarget::new();
    Font::Tdf(font.clone())
        .render_char(&mut target, 'X', RenderMode::Display)
        .unwrap();
    let lines = lines_to_strings(&target);
    assert_eq!(lines, vec!["XY", "ZW"]);
}

#[test]
fn tdf_render_ampersand_hidden_in_display_visible_in_edit() {
    let mut font = TdfFont::new("AMP", FontType::Block, 0);
    let glyph = Glyph {
        width: 3,
        height: 1,
        parts: vec![
            GlyphPart::Char('A'),
            GlyphPart::Char('B'),
            GlyphPart::EndMarker,
        ],
    };
    font.add_glyph(b'A', glyph);
    // Display mode: & suppressed
    let mut d_target = BufferTarget::new();
    Font::Tdf(font.clone())
        .render_char(&mut d_target, 'A', RenderMode::Display)
        .unwrap();
    assert_eq!(lines_to_strings(&d_target), vec!["AB"]);
    // Edit mode: & present
    let mut e_target = BufferTarget::new();
    Font::Tdf(font)
        .render_char(&mut e_target, 'A', RenderMode::Edit)
        .unwrap();
    assert_eq!(lines_to_strings(&e_target), vec!["AB&"]);
}

#[test]
fn tdf_bundle_multiple_fonts() {
    let mut f1 = TdfFont::new("ONE", FontType::Block, 0);
    f1.add_glyph(
        b'A',
        Glyph {
            width: 1,
            height: 1,
            parts: vec![GlyphPart::Char('A')],
        },
    );
    let mut f2 = TdfFont::new("TWO", FontType::Color, 0);
    f2.add_glyph(
        b'B',
        Glyph {
            width: 1,
            height: 1,
            parts: vec![GlyphPart::Colored {
                ch: 'B',
                fg: 0x1,
                bg: 0xF,
            }],
        },
    );
    let bundle = TdfFont::create_bundle(&[f1, f2]).unwrap();
    let parsed = TdfFont::from_bytes(&bundle).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].name, "ONE");
    assert_eq!(parsed[1].name, "TWO");
    assert_eq!(parsed[0].char_count(), 1);
    assert_eq!(parsed[1].char_count(), 1);
}

#[test]
fn tdf_render_color_attribute_nibbles() {
    let mut font = TdfFont::new("COLR", FontType::Color, 0);
    let glyph = Glyph {
        width: 3,
        height: 1,
        parts: vec![
            GlyphPart::Colored {
                ch: 'A',
                fg: 0xA,
                bg: 0xB,
            },
            GlyphPart::Colored {
                ch: ' ',
                fg: 0x0,
                bg: 0x1,
            },
            GlyphPart::Colored {
                ch: 'B',
                fg: 0x2,
                bg: 0xC,
            },
        ],
    };
    font.add_glyph(b'C', glyph);
    let mut target = BufferTarget::new();
    font.render_char(&mut target, 'C', RenderMode::Display)
        .unwrap();
    let line = lines_to_strings(&target).pop().unwrap();
    assert_eq!(line, "A B");
    let cells = &target.lines[0];
    assert_eq!(cells[0].fg, Some(0xA));
    assert_eq!(cells[0].bg, Some(0xB));
}

#[test]
fn tdf_outline_markers() {
    let mut font = TdfFont::new("OUTL", FontType::Outline, 0);
    let glyph = Glyph {
        width: 5,
        height: 1,
        parts: vec![
            GlyphPart::Char(' '),
            GlyphPart::FillMarker,
            GlyphPart::OutlineHole,
            GlyphPart::OutlinePlaceholder(b'A'),
            GlyphPart::OutlinePlaceholder(b'B'),
        ],
    };
    font.add_glyph(b'A', glyph);
    let mut target = BufferTarget::new();
    font.render_char(&mut target, 'A', RenderMode::Display)
        .unwrap();
    let line = lines_to_strings(&target)[0].clone();
    // Leading space + FillMarker + OutlineHole + 2 placeholders = various chars
    // Leading space stays leading, FillMarker/@, OutlineHole/O, then transformed A/B
    assert!(line.len() >= 4); // At least the non-leading characters
}

#[test]
fn tdf_edit_mode_preserves_markers() {
    let mut font = TdfFont::new("EDITM", FontType::Outline, 0);
    let glyph = Glyph {
        width: 3,
        height: 1,
        parts: vec![
            GlyphPart::FillMarker,
            GlyphPart::OutlineHole,
            GlyphPart::EndMarker,
        ],
    };
    font.add_glyph(b'E', glyph);
    let mut target = BufferTarget::new();
    font.render_char(&mut target, 'E', RenderMode::Edit)
        .unwrap();
    let line = lines_to_strings(&target)[0].clone();
    assert_eq!(line, "@O&");
}
