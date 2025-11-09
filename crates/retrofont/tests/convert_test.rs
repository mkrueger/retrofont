use retrofont::{
    convert::{can_convert_figlet_to_tdf, figlet_to_tdf},
    figlet::FigletFont,
    tdf::TdfFontType,
};

#[test]
fn test_figlet_to_tdf_compatibility() {
    // Load a FIGlet font
    let bytes = include_bytes!("figlet/doom.flf");
    let fig = FigletFont::from_bytes(bytes).expect("Failed to load FIGlet font");

    // Check compatibility with Block type
    assert!(
        can_convert_figlet_to_tdf(&fig, TdfFontType::Block),
        "doom.flf should be compatible with Block type"
    );

    // Check compatibility with Color type
    assert!(
        can_convert_figlet_to_tdf(&fig, TdfFontType::Color),
        "doom.flf should be compatible with Color type"
    );

    // Check compatibility with Outline type
    assert!(
        can_convert_figlet_to_tdf(&fig, TdfFontType::Outline),
        "doom.flf should be compatible with Outline type"
    );
}

#[test]
fn test_figlet_to_tdf_conversion_block() {
    // Load a FIGlet font
    let bytes = include_bytes!("figlet/doom.flf");
    let fig = FigletFont::from_bytes(bytes).expect("Failed to load FIGlet font");

    // Convert to TDF Block type
    let tdf = figlet_to_tdf(&fig, TdfFontType::Block).expect("Conversion should succeed");

    // Check basic properties
    assert_eq!(tdf.name, fig.name);
    assert_eq!(tdf.font_type, TdfFontType::Block);

    // Check that characters in TDF range are converted
    assert!(tdf.has_char('A'), "Should have character 'A'");
    assert!(tdf.has_char('!'), "Should have character '!'");
    assert!(tdf.has_char('~'), "Should have character '~'");

    // Check that character count is reasonable
    assert!(
        tdf.glyph_count() > 0,
        "Should have converted some characters"
    );
}

#[test]
fn test_figlet_to_tdf_only_converts_printable_range() {
    // Load a FIGlet font
    let bytes = include_bytes!("figlet/doom.flf");
    let fig = FigletFont::from_bytes(bytes).expect("Failed to load FIGlet font");

    // Convert to TDF
    let tdf = figlet_to_tdf(&fig, TdfFontType::Block).expect("Conversion should succeed");

    // TDF should only have characters in the printable range (! to ~)
    // Characters outside this range should not be converted

    // Space (0x20) is before '!' (0x21), so it might not be in the TDF
    // Check a character definitely outside the range
    assert!(
        !tdf.has_char('\0'),
        "Should not have null character (outside TDF range)"
    );

    // Characters in the valid range should be checked
    let valid_chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()_+-=[]{}|;:',.<>?/~";
    for ch in valid_chars.chars() {
        // Only check if the original font has it
        if fig.has_char(ch) {
            assert!(
                tdf.has_char(ch),
                "Character '{}' should be converted if it exists in FIGlet",
                ch
            );
        }
    }
}

#[test]
fn test_figlet_to_tdf_roundtrip() {
    // Load a FIGlet font
    let bytes = include_bytes!("figlet/doom.flf");
    let fig = FigletFont::from_bytes(bytes).expect("Failed to load FIGlet font");

    // Convert to TDF
    let tdf = figlet_to_tdf(&fig, TdfFontType::Block).expect("Conversion should succeed");

    // Serialize to bytes
    let tdf_bytes = tdf.to_bytes().expect("Serialization should succeed");

    // Parse back
    let tdf_fonts =
        retrofont::tdf::TdfFont::load_bundle_bytes(&tdf_bytes).expect("Should parse TDF bytes");

    assert_eq!(tdf_fonts.len(), 1, "Should have one font in bundle");
    let tdf_parsed = &tdf_fonts[0];

    // Check properties match
    assert_eq!(tdf_parsed.name, tdf.name);
    assert_eq!(tdf_parsed.font_type, tdf.font_type);

    // Check character count matches
    assert_eq!(
        tdf_parsed.glyph_count(),
        tdf.glyph_count(),
        "Character count should match after roundtrip"
    );
}
