use pretty_assertions::assert_eq;
use retrofont::{GlyphPart, figlet::FigletFont};

/// A FIGlet file with 2-row glyphs; `glyph` returns the raw lines for each required ASCII
/// character, `extra` is appended after them.
fn build(glyph: impl Fn(char) -> Vec<u8>, extra: &[u8]) -> Vec<u8> {
    let mut out = b"flf2a$ 2 2 10 -1 1\ncomment\n".to_vec();
    for c in ' '..='~' {
        out.extend(glyph(c));
    }
    out.extend(extra);
    out
}

/// Each glyph shows its own character on both rows (`@` would collide with the end mark).
fn simple(c: char) -> Vec<u8> {
    let r = if c == '@' { '*' } else { c };
    format!("{r}@\n{r}@@\n").into_bytes()
}

fn german(blank: char) -> Vec<u8> {
    let mut out = String::new();
    for c in ['Ä', 'Ö', 'Ü', 'ä', 'ö', 'ü', 'ß'] {
        if c == blank {
            out += "@\n@@\n";
        } else {
            out += &format!("{c}@\n{c}@@\n");
        }
    }
    out.into_bytes()
}

fn rows(font: &FigletFont, ch: char) -> Vec<String> {
    let glyph = font
        .glyph(ch)
        .unwrap_or_else(|| panic!("no glyph for {ch:?}"));
    let mut rows = vec![String::new()];
    for part in &glyph.parts {
        match part {
            GlyphPart::NewLine => rows.push(String::new()),
            GlyphPart::Char(c) => rows.last_mut().unwrap().push(*c),
            GlyphPart::HardBlank => rows.last_mut().unwrap().push('$'),
            other => panic!("unexpected part {other:?}"),
        }
    }
    rows
}

#[test]
fn any_character_can_be_the_end_mark() {
    let bytes = build(
        |c| match c {
            // Different end mark, trailing whitespace after it.
            'A' => b"A#  \nA##\n".to_vec(),
            // An empty first row whose end marks look like a final "@@".
            'B' => b"@@\nB\x7f\x7f\n".to_vec(),
            // Last row without a doubled end mark.
            'C' => b"C@\nC@\n".to_vec(),
            // Whitespace and CR after the end mark.
            'D' => b"D@@@\nD@\t\r\n".to_vec(),
            // Only the run of the last character is removed.
            'E' => b"E##@\nE@@\n".to_vec(),
            c => simple(c),
        },
        b"",
    );
    let font = FigletFont::load(&bytes).unwrap();
    assert_eq!(rows(&font, 'A'), ["A", "A"]);
    assert_eq!(rows(&font, 'B'), ["", "B"]);
    assert_eq!(rows(&font, 'C'), ["C", "C"]);
    assert_eq!(rows(&font, 'D'), ["D", "D"]);
    assert_eq!(rows(&font, 'E'), ["E##", "E"]);
    assert_eq!(rows(&font, 'F'), ["F", "F"]);
}

#[test]
fn files_that_are_not_utf8_are_read_as_latin1() {
    let mut bytes = build(
        |c| match c {
            'A' => b"\xe9\xa4@\n\xe9@@\n".to_vec(),
            c => simple(c),
        },
        b"\xc4@\n\xc4@@\n",
    );
    let comment_start = bytes.iter().position(|&b| b == b'\n').unwrap() + 1;
    bytes.splice(comment_start..comment_start + 7, b"caf\xe9".iter().copied());

    let font = FigletFont::load(&bytes).unwrap();
    assert_eq!(font.comments, ["café"]);
    assert_eq!(rows(&font, 'A'), ["é¤", "é"]);
    assert_eq!(font.glyph('A').unwrap().width, 2);
    assert_eq!(rows(&font, 'Ä'), ["Ä", "Ä"]);
}

#[test]
fn german_glyphs_follow_the_ascii_set() {
    let font = FigletFont::load(&build(simple, &german('ä'))).unwrap();
    for c in ['Ä', 'Ö', 'Ü', 'ö', 'ü', 'ß'] {
        assert_eq!(rows(&font, c), [c.to_string(), c.to_string()]);
    }
    // Blank placeholders stay defined with zero width, as in figlet.
    assert_eq!(font.glyph('ä').unwrap().width, 0);
    assert!(!font.has_char('\u{7f}'));
    assert_eq!(font.glyph_count(), 95 + 7);
}

#[test]
fn german_glyphs_are_optional() {
    let font = FigletFont::load(&build(simple, b"")).unwrap();
    assert!(!font.has_char('Ä'));
    assert_eq!(font.glyph_count(), 95);
}

#[test]
fn code_tagged_glyphs() {
    let mut extra = german('\0');
    extra.extend(
        b"256 decimal\nd@\nd@@\n\
          0x101 hex\nh@\nh@@\n\
          0402 octal\no@\no@@\n\
          -5 private use, skipped\np@\np@@\n\
          65 replaces A\nZ@\nZ@@\n\
          not a code tag\n\
          0x2500\nx@\nx@@\n",
    );
    let font = FigletFont::load(&build(simple, &extra)).unwrap();
    assert_eq!(rows(&font, '\u{100}'), ["d", "d"]);
    assert_eq!(rows(&font, '\u{101}'), ["h", "h"]);
    assert_eq!(rows(&font, '\u{102}'), ["o", "o"]);
    assert_eq!(rows(&font, 'A'), ["Z", "Z"]);
    // Loading stops at the first line that isn't a code tag, like figlet.
    assert!(!font.has_char('\u{2500}'));
    assert_eq!(font.glyph_count(), 95 + 7 + 3);
}

#[test]
fn truncated_code_tagged_glyph_is_ignored() {
    let mut extra = german('\0');
    extra.extend(b"0x100\nd@\n");
    let font = FigletFont::load(&build(simple, &extra)).unwrap();
    assert!(!font.has_char('\u{100}'));
}

#[test]
fn extended_glyphs_round_trip() {
    let mut font = FigletFont::new("roundtrip");
    font.add_char('A', &["A@", "x$"]);
    font.add_char('ä', &["ä", "ä"]);
    font.add_char('€', &["€", "€"]);
    font.add_char('\u{7f}', &["d", "d"]);

    let bytes = font.to_bytes().unwrap();
    let loaded = FigletFont::load(&bytes).unwrap();
    for c in ['A', 'ä', '€', '\u{7f}'] {
        assert_eq!(rows(&loaded, c), rows(&font, c), "{c:?}");
    }
    // Missing German glyphs are written as zero-width placeholders.
    assert_eq!(loaded.glyph('Ä').unwrap().width, 0);
    assert_eq!(loaded.glyph_count(), 95 + 7 + 2);
}

#[test]
fn ascii_only_fonts_are_written_without_extra_sections() {
    let mut font = FigletFont::new("ascii");
    font.add_char('A', &["A"]);
    let loaded = FigletFont::load(&font.to_bytes().unwrap()).unwrap();
    assert_eq!(loaded.glyph_count(), 95);
}
