//! Serialization must reject fonts that cannot be encoded, rather than
//! silently truncating offsets and producing unreadable files.
use retrofont::{
    FontError, Glyph, GlyphPart,
    tdf::{TdfFont, TdfFontType},
};

fn color_glyph(width: usize, height: usize) -> Glyph {
    let mut parts = Vec::new();
    for row in 0..height {
        if row > 0 {
            parts.push(GlyphPart::NewLine);
        }
        for _ in 0..width {
            parts.push(GlyphPart::AnsiChar {
                ch: 'X',
                fg: 7,
                bg: 0,
                blink: false,
            });
        }
    }
    Glyph {
        width,
        height,
        parts,
    }
}

/// A full set of maximum-size color glyphs exceeds the u16 glyph block encoding.
/// This previously serialized "successfully" into a file that failed to load.
#[test]
fn oversized_glyph_block_is_rejected() {
    let mut font = TdfFont::new("big", TdfFontType::Color, 1);
    for ch in '!'..='~' {
        font.add_glyph(ch, color_glyph(30, 12));
    }

    match font.to_bytes() {
        Err(FontError::TdfGlyphBlockTooLarge { .. }) => {}
        Err(e) => panic!("unexpected error: {e}"),
        Ok(bytes) => panic!("expected rejection, got {} bytes", bytes.len()),
    }
}

/// Anything that does serialize must survive a round trip intact.
#[test]
fn largest_encodable_font_round_trips() {
    let mut font = TdfFont::new("fits", TdfFontType::Color, 1);
    for ch in '!'..='P' {
        font.add_glyph(ch, color_glyph(30, 12));
    }
    let expected = font.glyph_count();

    let bytes = font.to_bytes().expect("should serialize");
    let reloaded = TdfFont::load(&bytes).expect("should reload");
    let reloaded = &reloaded[0];

    assert_eq!(reloaded.glyph_count(), expected);
    for ch in '!'..='P' {
        let g = reloaded.glyph(ch).unwrap_or_else(|| panic!("missing {ch}"));
        assert_eq!((g.width, g.height), (30, 12), "corrupted glyph {ch}");
    }
}

#[test]
fn glyph_dimensions_beyond_a_byte_are_rejected() {
    let mut font = TdfFont::new("wide", TdfFontType::Block, 1);
    let mut glyph = Glyph::new(300, 1);
    glyph.parts = vec![GlyphPart::Char('X'); 300];
    font.add_glyph('A', glyph);

    assert!(matches!(
        font.to_bytes(),
        Err(FontError::TdfGlyphTooLarge { .. })
    ));
}

#[test]
fn negative_spacing_is_rejected() {
    let mut font = TdfFont::new("neg", TdfFontType::Block, -1);
    font.add_glyph('A', color_glyph(2, 1));

    assert!(matches!(
        font.to_bytes(),
        Err(FontError::TdfSpacingOutOfRange { .. })
    ));
}
