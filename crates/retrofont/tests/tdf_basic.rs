use retrofont::{
    tdf::{FontType, TdfFont},
    test_support::BufferTarget,
    Font, Glyph, GlyphPart, RenderMode,
};

fn lines(buf: &BufferTarget) -> Vec<String> {
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
    font.add_glyph(b'A', glyph.clone());
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
    let line0: String = target.lines[0].iter().map(|c| c.ch).collect();
    assert_eq!(line0, "AB");
    let line1: String = target.lines[1].iter().map(|c| c.ch).collect();
    assert_eq!(line1, "CD");
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
                blink: false,
            },
            GlyphPart::NewLine,
            GlyphPart::EndMarker,
            GlyphPart::Colored {
                ch: 'B',
                fg: 0x2,
                bg: 0xF,
                blink: false,
            },
        ],
    };
    font.add_glyph(b'Z', glyph.clone());
    let bytes = font.as_tdf_bytes().unwrap();
    let parsed = TdfFont::from_bytes(&bytes).unwrap();
    let mut target = BufferTarget::new();
    Font::Tdf(parsed[0].clone())
        .render_char(&mut target, 'Z', RenderMode::Edit)
        .unwrap();
    // Expect only 'A' then newline then 'B' because '&' suppressed in display and edit currently for Color
    let rendered_line0: String = target.lines[0].iter().map(|c| c.ch).collect();
    let rendered_line1: String = target.lines[1].iter().map(|c| c.ch).collect();
    assert_eq!(rendered_line0, "A");
    assert_eq!(rendered_line1, "&B");
}

#[test]
fn tdf_render_block_multiline() {
    let mut font = TdfFont::new("BLK", FontType::Block, 0);
    font.add_glyph(
        b'X',
        Glyph {
            width: 2,
            height: 2,
            parts: vec![
                GlyphPart::Char('X'),
                GlyphPart::Char('Y'),
                GlyphPart::NewLine,
                GlyphPart::Char('Z'),
                GlyphPart::Char('W'),
            ],
        },
    );
    let mut target = BufferTarget::new();
    Font::Tdf(font.clone())
        .render_char(&mut target, 'X', RenderMode::Display)
        .unwrap();
    assert_eq!(lines(&target), vec!["XY", "ZW"]);
}

#[test]
fn tdf_ampersand_hidden_display_visible_edit() {
    let mut font = TdfFont::new("AMP", FontType::Block, 0);
    font.add_glyph(
        b'A',
        Glyph {
            width: 3,
            height: 1,
            parts: vec![
                GlyphPart::Char('A'),
                GlyphPart::Char('B'),
                GlyphPart::EndMarker,
            ],
        },
    );
    let mut d = BufferTarget::new();
    Font::Tdf(font.clone())
        .render_char(&mut d, 'A', RenderMode::Display)
        .unwrap();
    assert_eq!(lines(&d), vec!["AB"]);
    let mut e = BufferTarget::new();
    Font::Tdf(font)
        .render_char(&mut e, 'A', RenderMode::Edit)
        .unwrap();
    assert_eq!(lines(&e), vec!["AB&"]);
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
                blink: false,
            }],
        },
    );
    let bundle = TdfFont::create_bundle(&[f1, f2]).unwrap();

    // Write bundle for manual inspection testing
    std::fs::write("test_bundle.tdf", &bundle).ok();

    let parsed = TdfFont::from_bytes(&bundle).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].name, "ONE");
    assert_eq!(parsed[1].name, "TWO");
}

#[test]
fn tdf_render_color_attribute_nibbles() {
    let mut font = TdfFont::new("COLR", FontType::Color, 0);
    font.add_glyph(
        b'C',
        Glyph {
            width: 3,
            height: 1,
            parts: vec![
                GlyphPart::Colored {
                    ch: 'A',
                    fg: 0xA,
                    bg: 0xB,
                    blink: false,
                },
                GlyphPart::Colored {
                    ch: ' ',
                    fg: 0x0,
                    bg: 0x1,
                    blink: false,
                },
                GlyphPart::Colored {
                    ch: 'B',
                    fg: 0x2,
                    bg: 0xC,
                    blink: false,
                },
            ],
        },
    );
    let mut target = BufferTarget::new();
    Font::Tdf(font.clone())
        .render_char(&mut target, 'C', RenderMode::Display)
        .unwrap();
    let line = lines(&target)[0].clone();
    assert_eq!(line, "A B");
    let cells = &target.lines[0];
    assert_eq!(cells[0].fg, Some(0xA & 0x0F));
    assert_eq!(cells[0].bg, Some(0xB & 0x0F));
}

#[test]
fn tdf_edit_mode_preserves_markers() {
    let mut font = TdfFont::new("EDIT", FontType::Outline, 0);
    font.add_glyph(
        b'E',
        Glyph {
            width: 3,
            height: 1,
            parts: vec![
                GlyphPart::FillMarker,
                GlyphPart::OutlineHole,
                GlyphPart::EndMarker,
            ],
        },
    );
    let mut target = BufferTarget::new();
    Font::Tdf(font.clone())
        .render_char(&mut target, 'E', RenderMode::Edit)
        .unwrap();
    // Edit mode shows special markers: '@' for FillMarker, 'O' for OutlineHole, '&' for EndMarker
    assert_eq!(lines(&target)[0], "@O&");
}

#[test]
fn tdf_outline_uses_unicode_box_chars() {
    // Outline style 0: 'A'->0xC4 (CP437 horizontal line), 'B'->0xC4, 'C'->0xB3 (vertical line)
    // After Unicode mapping we expect: ─, ─, │ (U+2500, U+2500, U+2502)
    let mut font = TdfFont::new("UNI", FontType::Outline, 0);
    font.add_glyph(
        b'U',
        Glyph {
            width: 3,
            height: 1,
            parts: vec![
                GlyphPart::OutlinePlaceholder(b'A'),
                GlyphPart::OutlinePlaceholder(b'B'),
                GlyphPart::OutlinePlaceholder(b'C'),
            ],
        },
    );
    let mut target = BufferTarget::new();
    Font::Tdf(font)
        .render_char(&mut target, 'U', RenderMode::Display)
        .unwrap();
    assert_eq!(lines(&target)[0], "──│");
}
