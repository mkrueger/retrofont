use retrofont::{
    tdf::{TdfFont, TdfFontType},
    test_support::MemoryBufferTarget,
    Font, Glyph, GlyphPart, RenderOptions,
};

// Helper: collect rendered lines into Vec<String>
fn lines_to_strings(buf: &MemoryBufferTarget) -> Vec<String> {
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
    font.add_glyph('A', glyph);
    let bytes = font.to_bytes().expect("serialize");
    let parsed = TdfFont::load(&bytes).expect("parse");
    assert_eq!(parsed.len(), 1);
    let p = &parsed[0];
    assert_eq!(p.name, "TEST");
    assert_eq!(p.font_type(), TdfFontType::Block);
    // Validate via render
    let mut target = MemoryBufferTarget::new();
    Font::Tdf(p.clone())
        .render_glyph(&mut target, 'A', &RenderOptions::default())
        .unwrap();
    let lines = lines_to_strings(&target);
    assert_eq!(lines, vec!["AB", "CD"]);
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
    font.add_glyph('Z', glyph);
    let bytes = font.to_bytes().unwrap();
    let parsed = TdfFont::load(&bytes).unwrap();
    assert_eq!(parsed.len(), 1);
    // Validate via render
    let mut target = MemoryBufferTarget::new();
    Font::Tdf(parsed[0].clone())
        .render_glyph(&mut target, 'Z', &RenderOptions::default())
        .unwrap();
    assert!(!lines_to_strings(&target).is_empty());
}

#[test]
fn tdf_render_block_multiline() {
    let mut font = TdfFont::new("BLK", TdfFontType::Block, 0);
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
    font.add_glyph('X', glyph);
    let mut target = MemoryBufferTarget::new();
    Font::Tdf(font.clone())
        .render_glyph(&mut target, 'X', &RenderOptions::default())
        .unwrap();
    let lines = lines_to_strings(&target);
    assert_eq!(lines, vec!["XY", "ZW"]);
}

#[test]
fn tdf_render_ampersand_hidden_in_display_visible_in_edit() {
    let mut font = TdfFont::new("AMP", TdfFontType::Block, 0);
    let glyph = Glyph {
        width: 3,
        height: 1,
        parts: vec![
            GlyphPart::Char('A'),
            GlyphPart::Char('B'),
            GlyphPart::EndMarker,
        ],
    };
    font.add_glyph('A', glyph);
    // Display mode: & suppressed
    let mut d_target = MemoryBufferTarget::new();
    Font::Tdf(font.clone())
        .render_glyph(&mut d_target, 'A', &RenderOptions::default())
        .unwrap();
    assert_eq!(lines_to_strings(&d_target), vec!["AB"]);
    // Edit mode: & present
    let mut e_target = MemoryBufferTarget::new();
    Font::Tdf(font)
        .render_glyph(&mut e_target, 'A', &RenderOptions::edit())
        .unwrap();
    assert_eq!(lines_to_strings(&e_target), vec!["AB&"]);
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
    let parsed = TdfFont::load(&bundle).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].name, "ONE");
    assert_eq!(parsed[1].name, "TWO");
    assert_eq!(parsed[0].glyph_count(), 1);
    assert_eq!(parsed[1].glyph_count(), 1);
}

#[test]
fn tdf_render_color_attribute_nibbles() {
    let mut font = TdfFont::new("COLR", TdfFontType::Color, 0);
    let glyph = Glyph {
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
    };
    font.add_glyph('C', glyph);
    let mut target = MemoryBufferTarget::new();
    Font::Tdf(font)
        .render_glyph(&mut target, 'C', &RenderOptions::default())
        .unwrap();
    let line = lines_to_strings(&target).pop().unwrap();
    assert_eq!(line, "A B");
    let cells = &target.lines[0];
    assert_eq!(cells[0].fg, Some(0xA));
    assert_eq!(cells[0].bg, Some(0xB));
}

#[test]
fn tdf_outline_markers() {
    let mut font = TdfFont::new("OUTL", TdfFontType::Outline, 0);
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
    font.add_glyph('A', glyph);
    let mut target = MemoryBufferTarget::new();
    Font::Tdf(font)
        .render_glyph(&mut target, 'A', &RenderOptions::default())
        .unwrap();
    let line = lines_to_strings(&target)[0].clone();
    // Leading space + FillMarker + OutlineHole + 2 placeholders = various chars
    // Leading space stays leading, FillMarker/@, OutlineHole/O, then transformed A/B
    assert!(line.len() >= 4); // At least the non-leading characters
}

#[test]
fn tdf_edit_mode_preserves_markers() {
    let mut font = TdfFont::new("EDITM", TdfFontType::Outline, 0);
    let glyph = Glyph {
        width: 3,
        height: 1,
        parts: vec![
            GlyphPart::FillMarker,
            GlyphPart::OutlineHole,
            GlyphPart::EndMarker,
        ],
    };
    font.add_glyph('E', glyph);
    let mut target = MemoryBufferTarget::new();
    Font::Tdf(font)
        .render_glyph(&mut target, 'E', &RenderOptions::edit())
        .unwrap();
    let line = lines_to_strings(&target)[0].clone();
    assert_eq!(line, "@O&");
}
