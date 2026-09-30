use pretty_assertions::assert_eq;
use retrofont::{
    Font, FontError, Justify, Layout, MissingGlyph, RenderOptions, TextOptions,
    figlet::{FigletFont, layout},
    test_support::MemoryBufferTarget,
};

fn render(font: &Font, text: &str, options: &TextOptions) -> Vec<String> {
    let mut target = MemoryBufferTarget::new();
    font.render_str(&mut target, text, options).unwrap();
    target
        .lines
        .iter()
        .map(|l| {
            l.iter()
                .map(|c| c.ch)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

fn doom() -> Font {
    let bytes = include_bytes!("figlet/doom.flf");
    Font::Figlet(Box::new(FigletFont::load(bytes).unwrap()))
}

/// Single-row test font with the given layout bits.
fn font(full_layout: u32, glyphs: &[(u8, &str)]) -> Font {
    let mut fig = FigletFont::new("test");
    for (ch, row) in glyphs {
        fig.add_raw_char(*ch, &[row]);
    }
    fig.set_layout(full_layout);
    Font::Figlet(Box::new(fig))
}

fn pair(full_layout: u32, left: &str, right: &str) -> String {
    let font = font(full_layout, &[(b'L', left), (b'R', right)]);
    render(&font, "LR", &TextOptions::default()).remove(0)
}

// Expected output generated with FIGlet (via pyfiglet) using the same font file.
#[test]
fn doom_smushing_matches_figlet() {
    let expected = [
        r" _   _      _ _",
        r"| | | |    | | |",
        r"| |_| | ___| | | ___",
        r"|  _  |/ _ \ | |/ _ \",
        r"| | | |  __/ | | (_) |",
        r"\_| |_/\___|_|_|\___/",
        "",
        "",
    ];
    assert_eq!(render(&doom(), "Hello", &TextOptions::default()), expected);
}

#[test]
fn doom_layout_is_derived_from_old_layout() {
    let Font::Figlet(fig) = doom() else {
        unreachable!()
    };
    // old_layout 15 => rules 1-4 plus smushing.
    assert_eq!(fig.layout(), layout::SMUSHING | 15);
}

#[test]
fn full_width_places_glyphs_side_by_side() {
    let options = TextOptions {
        layout: Layout::FullWidth,
        ..TextOptions::default()
    };
    let rows = render(&doom(), "Hi", &options);
    assert_eq!(rows[1], r"| | | |(_)");
    assert_eq!(rows[5], r"\_| |_/|_|");
}

#[test]
fn smushing_rules() {
    let s = layout::SMUSHING;
    assert_eq!(pair(s | layout::EQUAL, "a|", "|b"), "a|b");
    assert_eq!(pair(s | layout::UNDERSCORE, "a_", "/b"), "a/b");
    assert_eq!(pair(s | layout::UNDERSCORE, "a(", "_b"), "a(b");
    assert_eq!(pair(s | layout::HIERARCHY, "a|", "/b"), "a/b");
    assert_eq!(pair(s | layout::HIERARCHY, "a<", "{b"), "a<b");
    assert_eq!(pair(s | layout::OPPOSITE_PAIR, "a[", "]b"), "a|b");
    assert_eq!(pair(s | layout::OPPOSITE_PAIR, "a)", "(b"), "a|b");
    assert_eq!(pair(s | layout::BIG_X, "a/", "\\b"), "a|b");
    assert_eq!(pair(s | layout::BIG_X, "a\\", "/b"), "aYb");
    assert_eq!(pair(s | layout::BIG_X, "a>", "<b"), "aXb");
    assert_eq!(pair(s | layout::HARDBLANK, "a$", "$b"), "a b");
}

#[test]
fn smushing_without_matching_rule_only_kerns() {
    let s = layout::SMUSHING;
    assert_eq!(pair(s | layout::EQUAL, "ax ", " yb"), "axyb");
    assert_eq!(pair(s | layout::EQUAL, "a$", "$b"), "a  b");
}

#[test]
fn universal_smushing_prefers_right_but_not_hardblanks() {
    assert_eq!(pair(layout::SMUSHING, "ax", "yb"), "ayb");
    assert_eq!(pair(layout::SMUSHING, "ax", "$b"), "axb");
}

#[test]
fn narrow_glyphs_are_never_smushed() {
    assert_eq!(pair(layout::SMUSHING | layout::EQUAL, "|", "|"), "||");
}

#[test]
fn kerning_and_full_width() {
    assert_eq!(pair(layout::KERNING, "a ", " b"), "ab");
    assert_eq!(pair(layout::KERNING, "a|", "|b"), "a||b");
    assert_eq!(pair(0, "a ", " b"), "a  b");
}

#[test]
fn layout_override() {
    let font = font(0, &[(b'L', "a|"), (b'R', "|b")]);
    let smush = TextOptions {
        layout: Layout::Smushing,
        ..TextOptions::default()
    };
    // No rules in the font: universal smushing.
    assert_eq!(render(&font, "LR", &smush), ["a|b"]);
    let kern = TextOptions {
        layout: Layout::Kerning,
        ..TextOptions::default()
    };
    assert_eq!(render(&font, "LR", &kern), ["a||b"]);
}

#[test]
fn newlines_stack_lines_and_empty_lines_keep_font_height() {
    let doom = doom();
    let single = render(&doom, "Hi", &TextOptions::default());
    assert_eq!(single.len(), 8);

    let double = render(&doom, "Hi\r\nHi", &TextOptions::default());
    assert_eq!(double[..8], single[..]);
    assert_eq!(double[8..], single[..]);

    let gap = render(&doom, "Hi\n\nHi", &TextOptions::default());
    assert_eq!(gap.len(), 24);
    assert!(gap[8..16].iter().all(String::is_empty));
}

// Expected output generated with FIGlet (via pyfiglet, width 31).
#[test]
fn wraps_at_word_boundaries_like_figlet() {
    let options = TextOptions {
        max_width: Some(30),
        ..TextOptions::default()
    };
    let expected = [
        r" _   _ _",
        r"| | | (_)",
        r"| |_| |_   _   _  ___  _   _",
        r"|  _  | | | | | |/ _ \| | | |",
        r"| | | | | | |_| | (_) | |_| |",
        r"\_| |_/_|  \__, |\___/ \__,_|",
        r"            __/ |",
        r"           |___/",
        r" _   _",
        r"| | | |",
        r"| |_| |__   ___ _ __ ___",
        r"| __| '_ \ / _ \ '__/ _ \",
        r"| |_| | | |  __/ | |  __/",
        r" \__|_| |_|\___|_|  \___|",
        "",
        "",
    ];
    assert_eq!(render(&doom(), "Hi you there", &options), expected);
}

#[test]
fn wraps_mid_word_when_a_word_does_not_fit() {
    let font = font(0, &[(b'a', "aa")]);
    let options = TextOptions {
        max_width: Some(5),
        ..TextOptions::default()
    };
    assert_eq!(render(&font, "aaaaa", &options), ["aaaa", "aaaa", "aa"]);
}

#[test]
fn a_glyph_wider_than_max_width_still_renders() {
    let font = font(0, &[(b'a', "aaaaaa")]);
    let options = TextOptions {
        max_width: Some(3),
        ..TextOptions::default()
    };
    assert_eq!(render(&font, "aa", &options), ["aaaaaa", "aaaaaa"]);
}

#[test]
fn justification() {
    let font = font(0, &[(b'a', "aa"), (b'b', "bb")]);
    let with = |justify, max_width| TextOptions {
        justify,
        max_width,
        ..TextOptions::default()
    };
    assert_eq!(
        render(&font, "ab\na", &with(Justify::Left, None)),
        ["aabb", "aa"]
    );
    assert_eq!(
        render(&font, "ab\na", &with(Justify::Center, None)),
        ["aabb", " aa"]
    );
    assert_eq!(
        render(&font, "ab\na", &with(Justify::Right, None)),
        ["aabb", "  aa"]
    );
    assert_eq!(
        render(&font, "ab", &with(Justify::Center, Some(10))),
        ["   aabb"]
    );
    assert_eq!(
        render(&font, "ab", &with(Justify::Right, Some(10))),
        ["      aabb"]
    );
}

#[test]
fn padding_and_gaps_are_transparent() {
    let font = font(0, &[(b'a', "aa")]);
    let options = TextOptions {
        justify: Justify::Right,
        max_width: Some(4),
        ..TextOptions::default()
    };
    let mut target = SkipCounter::default();
    font.render_str(&mut target, "a", &options).unwrap();
    assert_eq!((target.skipped, target.drawn), (2, 2));
}

#[derive(Default)]
struct SkipCounter {
    skipped: usize,
    drawn: usize,
}

impl retrofont::FontTarget for SkipCounter {
    type Error = std::fmt::Error;
    fn draw(&mut self, _cell: retrofont::Cell) -> Result<(), Self::Error> {
        self.drawn += 1;
        Ok(())
    }
    fn next_line(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn skip(&mut self) -> Result<(), Self::Error> {
        self.skipped += 1;
        Ok(())
    }
}

#[test]
fn missing_glyphs() {
    let font = font(0, &[(b'a', "aa"), (b'b', "bb")]);
    let mut target = MemoryBufferTarget::new();
    let err = font
        .render_str(&mut target, "a?b", &TextOptions::default())
        .unwrap_err();
    assert!(matches!(err, FontError::UnknownChar('?')));

    let skip = TextOptions {
        missing: MissingGlyph::Skip,
        ..TextOptions::default()
    };
    assert_eq!(render(&font, "a?b", &skip), ["aabb"]);
    // Case fallback still applies.
    assert_eq!(render(&font, "AB", &skip), ["aabb"]);
}

#[test]
fn measure_reports_layout_size() {
    let doom = doom();
    assert_eq!(
        doom.measure("Hello", &TextOptions::default()).unwrap(),
        (22, 8)
    );
    let wrapped = TextOptions {
        max_width: Some(30),
        ..TextOptions::default()
    };
    assert_eq!(doom.measure("Hi you there", &wrapped).unwrap(), (29, 16));
}

#[test]
fn layout_survives_serialization() {
    let mut fig = FigletFont::new("roundtrip");
    fig.add_raw_char(b'A', &["A"]);
    for bits in [0, layout::KERNING, layout::SMUSHING, layout::SMUSHING | 21] {
        fig.set_layout(bits);
        let loaded = FigletFont::load(&fig.to_bytes().unwrap()).unwrap();
        assert_eq!(loaded.layout(), bits);
    }
}

#[test]
fn old_layout_without_full_layout() {
    let mut fig = FigletFont::new("old");
    fig.add_raw_char(b'A', &["A"]);
    let bytes = fig.to_bytes().unwrap();
    let body = &bytes[bytes.iter().position(|&b| b == b'\n').unwrap()..];
    for (old, expected) in [
        ("-1", 0),
        ("0", layout::KERNING),
        ("5", layout::SMUSHING | 5),
        ("63", layout::SMUSHING | 31),
    ] {
        let mut data = format!("flf2a$ 1 1 80 {old} 0").into_bytes();
        data.extend_from_slice(body);
        assert_eq!(FigletFont::load(&data).unwrap().layout(), expected, "{old}");
    }
}

#[test]
fn tdf_text_matches_individual_glyphs() {
    let bytes = include_bytes!("tdf/CODERX.TDF");
    let font = Font::load(bytes).unwrap().remove(0);
    let options = TextOptions::default();

    let mut single = MemoryBufferTarget::new();
    font.render_glyph(&mut single, 'a', &RenderOptions::default())
        .unwrap();
    let single: Vec<String> = single
        .lines
        .iter()
        .map(|l| {
            l.iter()
                .map(|c| c.ch)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect();
    assert_eq!(render(&font, "a", &options), single);

    let (aw, ah) = font.glyph_size('a').unwrap();
    let (bw, bh) = font.glyph_size('b').unwrap();
    assert_eq!(font.measure("ab", &options).unwrap(), (aw + bw, ah.max(bh)));
}

#[test]
fn skipped_characters_use_the_missing_glyph_like_figlet() {
    let mut fig = FigletFont::new("zero");
    fig.add_raw_char(b'a', &["aa"]);
    fig.add_char('\0', &["?"]);
    let font = Font::Figlet(Box::new(fig));
    let skip = TextOptions {
        missing: MissingGlyph::Skip,
        ..TextOptions::default()
    };
    assert_eq!(render(&font, "a\u{1}a", &skip), ["aa?aa"]);
}

#[test]
fn skipped_characters_block_smushing_like_figlet() {
    let font = font(layout::SMUSHING, &[(b'L', "a|"), (b'R', "|b")]);
    let skip = TextOptions {
        missing: MissingGlyph::Skip,
        ..TextOptions::default()
    };
    assert_eq!(render(&font, "LR", &skip), ["a|b"]);
    assert_eq!(render(&font, "L?R", &skip), ["a||b"]);
}

#[test]
fn case_fallback_ignores_multi_character_mappings() {
    // 'ß' uppercases to "SS"; it must not fall back to 'S'.
    let font = font(0, &[(b'S', "S")]);
    let mut target = MemoryBufferTarget::new();
    let err = font
        .render_str(&mut target, "ß", &TextOptions::default())
        .unwrap_err();
    assert!(matches!(err, FontError::UnknownChar('ß')));
}
