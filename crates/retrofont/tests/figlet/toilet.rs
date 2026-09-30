use pretty_assertions::assert_eq;
use retrofont::{
    Font, FontError, GlyphPart,
    figlet::{FigletFont, FigletFormat},
};
use std::io::Write;

/// A font file with the given signature (and hard blank), where every ASCII glyph shows
/// its own character; `glyph` can override individual glyphs.
fn build(signature: &str, glyph: impl Fn(char) -> Option<Vec<u8>>) -> Vec<u8> {
    let mut out = format!("{signature} 1 1 10 -1 0\n").into_bytes();
    for c in ' '..='~' {
        let r = if c == '@' { '*' } else { c };
        out.extend(glyph(c).unwrap_or_else(|| format!("{r}@@\n").into_bytes()));
    }
    out
}

fn ansi_char(ch: char, fg: u8, bg: u8, blink: bool) -> GlyphPart {
    GlyphPart::AnsiChar { ch, fg, bg, blink }
}

#[test]
fn toilet_fonts_load_as_utf8() {
    let bytes = build("tlf2a\x7f", |c| {
        (c == 'A').then(|| "█▄\x7f@@\n".as_bytes().to_vec())
    });
    let font = FigletFont::load(&bytes).unwrap();
    assert_eq!(font.format(), FigletFormat::Tlf);
    assert_eq!(font.hard_blank, '\x7f');
    let glyph = font.glyph('A').unwrap();
    assert_eq!(glyph.width, 3);
    assert_eq!(
        glyph.parts,
        [
            GlyphPart::Char('█'),
            GlyphPart::Char('▄'),
            GlyphPart::HardBlank
        ]
    );

    let fonts = Font::load(&bytes).unwrap();
    assert!(matches!(&fonts[..], [Font::Figlet(f)] if f.format() == FigletFormat::Tlf));
    assert_eq!(fonts[0].default_extension(), "tlf");
}

#[test]
fn toilet_fonts_must_be_utf8() {
    let bytes = build("tlf2a$", |c| (c == 'A').then(|| b"\xe9@@\n".to_vec()));
    assert!(matches!(FigletFont::load(&bytes), Err(FontError::Utf8(_))));
}

#[test]
fn toilet_glyphs_carry_ansi_colours() {
    let bytes = build("tlf2a$", |c| {
        let row: &str = match c {
            // Bold red on blue, then a plain character after a reset.
            'A' => "\x1b[0;1;31;44m#\x1b[0mx@@",
            // Negative image swaps colours; blink is kept.
            'B' => "\x1b[5;7;32;43mx\x1b[0m@@",
            // Bright colours; bold with the default colour becomes white on black.
            'C' => "\x1b[96;105mc\x1b[0;1md\x1b[m@@",
            // A coloured end mark and unrelated escape sequences are not content.
            'D' => "\x1b[2Kd\x1b[31m@@\x1b[0m  ",
            _ => return None,
        };
        Some(format!("{row}\n").into_bytes())
    });
    let font = FigletFont::load(&bytes).unwrap();
    let parts = |c| font.glyph(c).unwrap().parts.clone();
    assert_eq!(
        parts('A'),
        [ansi_char('#', 12, 1, false), GlyphPart::Char('x')]
    );
    assert_eq!(parts('B'), [ansi_char('x', 6, 2, true)]);
    assert_eq!(
        parts('C'),
        [ansi_char('c', 11, 13, false), ansi_char('d', 15, 0, false)]
    );
    assert_eq!(parts('D'), [GlyphPart::Char('d')]);
    assert_eq!(font.glyph('C').unwrap().width, 2);
}

#[test]
fn figlet_fonts_keep_escape_characters_as_text() {
    let bytes = build("flf2a$", |c| (c == 'A').then(|| b"\x1b[1mA@@\n".to_vec()));
    let font = FigletFont::load(&bytes).unwrap();
    assert_eq!(font.glyph('A').unwrap().width, 5);
}

#[test]
fn colours_round_trip_through_toilet_files() {
    // The last visible character is '@', so the writer must pick another end mark.
    let bytes = build("tlf2a$", |c| {
        (c == 'A').then(|| b"\x1b[5mz\x1b[0m\x1b[0;1;31;44m@\x1b[0m#\n".to_vec())
    });
    let font = FigletFont::load(&bytes).unwrap();
    let parts = font.glyph('A').unwrap().parts.clone();
    assert_eq!(
        parts,
        [ansi_char('z', 7, 0, true), ansi_char('@', 12, 1, false)]
    );

    let written = font.to_bytes().unwrap();
    assert!(written.starts_with(b"tlf2a"));
    assert_eq!(
        FigletFont::load(&written)
            .unwrap()
            .glyph('A')
            .unwrap()
            .parts,
        parts
    );

    // FIGlet files can't store colours, only the characters.
    let mut flf = font;
    flf.set_format(FigletFormat::Flf);
    let plain = FigletFont::load(&flf.to_bytes().unwrap()).unwrap();
    assert_eq!(plain.format(), FigletFormat::Flf);
    assert_eq!(
        plain.glyph('A').unwrap().parts,
        [GlyphPart::Char('z'), GlyphPart::Char('@')]
    );
}

#[test]
fn zipped_fonts_are_found_by_content() {
    // TOIlet's own zipped fonts name their only entry "-".
    let tlf = build("tlf2a$", |_| None);
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("README", stored).unwrap();
    zip.write_all(b"not a font").unwrap();
    zip.start_file("-", stored).unwrap();
    zip.write_all(&tlf).unwrap();
    let bytes = zip.finish().unwrap().into_inner();

    let font = FigletFont::load(&bytes).unwrap();
    assert_eq!(font.format(), FigletFormat::Tlf);
    assert!(font.has_char('A'));
    assert!(matches!(
        &Font::load(&bytes).unwrap()[..],
        [Font::Figlet(_)]
    ));
}

#[test]
fn zip_without_font_is_rejected() {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("font.flf", stored).unwrap();
    zip.write_all(b"not a font").unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    assert!(matches!(FigletFont::load(&bytes), Err(FontError::ZipNoFlf)));
}
