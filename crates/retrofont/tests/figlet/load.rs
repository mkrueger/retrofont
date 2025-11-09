use retrofont::figlet::FigletFont;
use std::path::Path;

#[test]
fn test_zipped_equals_plain() {
    let base_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let doom_plain = base_path.join("tests/figlet/doom.flf");
    let doom_zip = base_path.join("tests/figlet/doom_zipped.flf");
    let font1 = FigletFont::load(&doom_plain).unwrap();
    let font2 = FigletFont::load(&doom_zip).unwrap();
    assert_eq!(font1.header(), font2.header());
    assert_eq!(font1.glyph_count(), font2.glyph_count());
    assert_eq!(96, font1.glyph_count());
}
