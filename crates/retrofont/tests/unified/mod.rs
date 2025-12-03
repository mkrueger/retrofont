//! Tests using the unified Font API

use retrofont::{test_support::MemoryBufferTarget, Font, RenderOptions};

const ZETRAX_TDF: &[u8] = include_bytes!("ZETRAX.TDF");

#[test]
fn test_load_zetrax_via_unified_api() {
    // Load using unified Font::load (auto-detects TDF format)
    let fonts = Font::load(ZETRAX_TDF).unwrap();

    // Should contain multiple fonts
    assert!(
        fonts.len() > 1,
        "Expected multiple fonts in ZETRAX.TDF, got {}",
        fonts.len()
    );

    // All fonts should have names accessible via unified API
    for font in &fonts {
        assert!(!font.name().is_empty());
    }
}

#[test]
fn test_render_via_unified_api() {
    let fonts = Font::load(ZETRAX_TDF).unwrap();
    let font = &fonts[0];

    // Use the unified render API
    let mut target = MemoryBufferTarget::new();
    let options = RenderOptions::default();

    // Render a character that should exist in most fonts
    if font.has_char('A') {
        font.render_glyph(&mut target, 'A', &options).unwrap();

        // Should have produced some output
        assert!(
            !target.lines.is_empty(),
            "Rendering 'A' should produce output"
        );
        assert!(!target.lines[0].is_empty(), "First line should have cells");
    }
}

#[test]
fn test_unified_api_spacing() {
    let fonts = Font::load(ZETRAX_TDF).unwrap();

    for font in &fonts {
        // spacing() should return Some value for TDF fonts
        let spacing = font.spacing();
        assert!(
            spacing.is_some(),
            "Font {} should have spacing",
            font.name()
        );
    }
}

#[test]
fn test_unified_api_case_fallback() {
    let fonts = Font::load(ZETRAX_TDF).unwrap();
    let font = &fonts[0];

    let mut target = MemoryBufferTarget::new();
    let options = RenderOptions::default();

    // TDF fonts typically only have uppercase letters
    // The unified API should fall back to uppercase when lowercase isn't found
    if font.has_char('A') && !font.has_char('a') {
        // Rendering 'a' should fall back to 'A'
        font.render_glyph(&mut target, 'a', &options).unwrap();
        assert!(
            !target.lines.is_empty(),
            "Case fallback should render 'a' as 'A'"
        );
    }
}
