use retrofont::figlet::FigletFont;
use std::path::Path;

#[test]
fn test_zipped_equals_plain() {
    let base_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let doom_plain = base_path.join("tests/figlet/doom.flf");
    let doom_zip = base_path.join("tests/figlet/doom_zipped.flf");
    let test_font = FigletFont::load_file(&doom_plain).unwrap();
    let zipped_font = FigletFont::load_file(&doom_zip).unwrap();
    assert_eq!(test_font.header, zipped_font.header);
    assert_eq!(test_font.glyph_count(), zipped_font.glyph_count());
    let doom_font_glyph_count = 96; // ASCII printable + space + extended
    assert_eq!(doom_font_glyph_count, test_font.glyph_count());
}
