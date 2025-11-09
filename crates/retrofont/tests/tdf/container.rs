use retrofont::tdf::{TdfFont, TdfFontType};

const TEST_FONT: &[u8] = include_bytes!("CODERX.TDF");

#[test]
fn test_load_bundle() {
    let fonts = TdfFont::from_bytes(TEST_FONT).unwrap();
    assert_eq!(6, fonts.len());
    for f in &fonts {
        assert_eq!(f.font_type(), TdfFontType::Color);
    }
    assert_eq!(fonts[0].name, "Coder Blue");
    assert_eq!(fonts[1].name, "Coder Green");
    assert_eq!(fonts[2].name, "Coder Margen");
    assert_eq!(fonts[3].name, "Coder Purple");
    assert_eq!(fonts[4].name, "Coder Red");
    assert_eq!(fonts[5].name, "Coder Silver");
}

#[test]
fn test_save_and_reload_bundle() {
    let fonts = TdfFont::from_bytes(TEST_FONT).unwrap();
    let bundle = TdfFont::create_bundle(&fonts).unwrap();
    let parsed = TdfFont::from_bytes(&bundle).unwrap();
    assert_eq!(parsed.len(), 6);
    for (a, b) in fonts.iter().zip(parsed.iter()) {
        assert_eq!(a.name, b.name);
    }
}
