use retrofont::{
    tdf::{TdfFont, TdfFontType},
    test_support::MemoryBufferTarget,
    Font, Glyph, GlyphPart, RenderOptions,
};

fn lines(buf: &MemoryBufferTarget) -> Vec<String> {
    buf.lines
        .iter()
        .map(|l| l.iter().map(|c| c.ch).collect())
        .collect()
}

#[test]
fn tdf_round_trip_block_single_glyph() {
    let mut font = TdfFont::new("TEST", TdfFontType::Block, 0);
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
    font.add_glyph('A', glyph.clone());
    let bytes = font.to_bytes().expect("serialize");
    let parsed = TdfFont::load_bundle_bytes(&bytes).expect("parse");
    assert_eq!(parsed.len(), 1);
    let p = &parsed[0];
    assert_eq!(p.name, "TEST");
    assert_eq!(p.font_type(), TdfFontType::Block);
    // Validate via render
    let mut target = MemoryBufferTarget::new();
    Font::Tdf(p.clone())
        .render_glyph(&mut target, 'A', &RenderOptions::default())
        .unwrap();
    let line0: String = target.lines[0].iter().map(|c| c.ch).collect();
    assert_eq!(line0, "AB");
    let line1: String = target.lines[1].iter().map(|c| c.ch).collect();
    assert_eq!(line1, "CD");
}

#[test]
fn tdf_round_trip_color_attributes() {
    let mut font = TdfFont::new("COLOR", TdfFontType::Color, 0);
    let glyph = Glyph {
        width: 3,
        height: 2,
        parts: vec![
            GlyphPart::AnsiChar {
                ch: 'A',
                fg: 0x1,
                bg: 0xE,
                blink: false,
            },
            GlyphPart::NewLine,
            GlyphPart::EndMarker,
            GlyphPart::AnsiChar {
                ch: 'B',
                fg: 0x2,
                bg: 0xF,
                blink: false,
            },
        ],
    };
    font.add_glyph('Z', glyph.clone());
    let bytes = font.to_bytes().unwrap();
    let parsed = TdfFont::load_bundle_bytes(&bytes).unwrap();
    let mut target = MemoryBufferTarget::new();
    Font::Tdf(parsed[0].clone())
        .render_glyph(&mut target, 'Z', &RenderOptions::edit())
        .unwrap();
    // Expect only 'A' then newline then 'B' because '&' suppressed in display and edit currently for Color
    let rendered_line0: String = target.lines[0].iter().map(|c| c.ch).collect();
    let rendered_line1: String = target.lines[1].iter().map(|c| c.ch).collect();
    assert_eq!(rendered_line0, "A");
    assert_eq!(rendered_line1, "&B");
}

#[test]
fn tdf_render_block_multiline() {
    let mut font = TdfFont::new("BLK", TdfFontType::Block, 0);
    font.add_glyph(
        'X',
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
    let mut target = MemoryBufferTarget::new();
    Font::Tdf(font.clone())
        .render_glyph(&mut target, 'X', &RenderOptions::default())
        .unwrap();
    assert_eq!(lines(&target), vec!["XY", "ZW"]);
}

#[test]
fn tdf_ampersand_hidden_display_visible_edit() {
    let mut font = TdfFont::new("AMP", TdfFontType::Block, 0);
    font.add_glyph(
        'A',
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
    let mut d = MemoryBufferTarget::new();
    Font::Tdf(font.clone())
        .render_glyph(&mut d, 'A', &RenderOptions::default())
        .unwrap();
    assert_eq!(lines(&d), vec!["AB"]);
    let mut e = MemoryBufferTarget::new();
    Font::Tdf(font)
        .render_glyph(&mut e, 'A', &RenderOptions::edit())
        .unwrap();
    assert_eq!(lines(&e), vec!["AB&"]);
}

#[test]
fn tdf_bundle_multiple_fonts() {
    let mut f1 = TdfFont::new("ONE", TdfFontType::Block, 0);
    f1.add_glyph(
        'A',
        Glyph {
            width: 1,
            height: 1,
            parts: vec![GlyphPart::Char('A')],
        },
    );
    let mut f2 = TdfFont::new("TWO", TdfFontType::Color, 0);
    f2.add_glyph(
        'B',
        Glyph {
            width: 1,
            height: 1,
            parts: vec![GlyphPart::AnsiChar {
                ch: 'B',
                fg: 0x1,
                bg: 0xF,
                blink: false,
            }],
        },
    );
    let bundle = TdfFont::serialize_bundle(&[f1, f2]).unwrap();

    // Write bundle for manual inspection testing
    std::fs::write("test_bundle.tdf", &bundle).ok();

    let parsed = TdfFont::load_bundle_bytes(&bundle).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].name, "ONE");
    assert_eq!(parsed[1].name, "TWO");
}

#[test]
fn tdf_render_color_attribute_nibbles() {
    let mut font = TdfFont::new("COLR", TdfFontType::Color, 0);
    font.add_glyph(
        'C',
        Glyph {
            width: 3,
            height: 1,
            parts: vec![
                GlyphPart::AnsiChar {
                    ch: 'A',
                    fg: 0xA,
                    bg: 0xB,
                    blink: false,
                },
                GlyphPart::AnsiChar {
                    ch: ' ',
                    fg: 0x0,
                    bg: 0x1,
                    blink: false,
                },
                GlyphPart::AnsiChar {
                    ch: 'B',
                    fg: 0x2,
                    bg: 0xC,
                    blink: false,
                },
            ],
        },
    );
    let mut target = MemoryBufferTarget::new();
    Font::Tdf(font.clone())
        .render_glyph(&mut target, 'C', &RenderOptions::default())
        .unwrap();
    let line = lines(&target)[0].clone();
    assert_eq!(line, "A B");
    let cells = &target.lines[0];
    assert_eq!(cells[0].fg, Some(0xA & 0x0F));
    assert_eq!(cells[0].bg, Some(0xB & 0x0F));
}

#[test]
fn tdf_edit_mode_preserves_markers() {
    let mut font = TdfFont::new("EDIT", TdfFontType::Outline, 0);
    font.add_glyph(
        'E',
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
    let mut target = MemoryBufferTarget::new();
    Font::Tdf(font.clone())
        .render_glyph(&mut target, 'E', &RenderOptions::edit())
        .unwrap();
    // Edit mode shows special markers: '@' for FillMarker, 'O' for OutlineHole, '&' for EndMarker
    assert_eq!(lines(&target)[0], "@O&");
}

#[test]
fn tdf_outline_uses_unicode_box_chars() {
    // Outline style 0: 'A'->0xC4 (CP437 horizontal line), 'B'->0xC4, 'C'->0xB3 (vertical line)
    // After Unicode mapping we expect: ─, ─, │ (U+2500, U+2500, U+2502)
    let mut font = TdfFont::new("UNI", TdfFontType::Outline, 0);
    font.add_glyph(
        'U',
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
    let mut target = MemoryBufferTarget::new();
    Font::Tdf(font)
        .render_glyph(&mut target, 'U', &RenderOptions::default())
        .unwrap();
    assert_eq!(lines(&target)[0], "──│");
}
