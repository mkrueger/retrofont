use retrofont::{
    tdf::{FontType, TdfFont},
    Glyph, GlyphPart,
};

#[test]
fn inspect_bundle_creation() {
    let mut f1 = TdfFont::new("Alpha", FontType::Block, 0);
    f1.add_glyph(
        b'A',
        Glyph {
            width: 1,
            height: 1,
            parts: vec![GlyphPart::Char('A')],
        },
    );
    f1.add_glyph(
        b'B',
        Glyph {
            width: 1,
            height: 1,
            parts: vec![GlyphPart::Char('B')],
        },
    );

    let mut f2 = TdfFont::new("Beta", FontType::Color, 0);
    f2.add_glyph(
        b'X',
        Glyph {
            width: 1,
            height: 1,
            parts: vec![GlyphPart::Colored {
                ch: 'X',
                fg: 0xF,
                bg: 0x0,
            }],
        },
    );
    f2.add_glyph(
        b'Y',
        Glyph {
            width: 1,
            height: 1,
            parts: vec![GlyphPart::Colored {
                ch: 'Y',
                fg: 0xF,
                bg: 0x0,
            }],
        },
    );
    f2.add_glyph(
        b'Z',
        Glyph {
            width: 1,
            height: 1,
            parts: vec![GlyphPart::Colored {
                ch: 'Z',
                fg: 0xF,
                bg: 0x0,
            }],
        },
    );

    let mut f3 = TdfFont::new("Gamma", FontType::Outline, 0);
    f3.add_glyph(
        b'1',
        Glyph {
            width: 1,
            height: 1,
            parts: vec![GlyphPart::OutlinePlaceholder(b'A')],
        },
    );

    let bundle = TdfFont::create_bundle(&[f1, f2, f3]).unwrap();
    std::fs::write("test_bundle.tdf", &bundle).unwrap();
    eprintln!(
        "Created test_bundle.tdf - run: cargo run --release -- inspect --font test_bundle.tdf"
    );
}
