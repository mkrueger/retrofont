use retrofont::{tdf::{TdfFont, TdfFontType}, Glyph, GlyphPart};

fn main() {
    // Test 1: Overlay-Glyph entfernen
    let mut font = TdfFont::new("TEST", TdfFontType::Block, 0);
    font.add_glyph('A', Glyph { width: 1, height: 1, parts: vec![GlyphPart::Char('A')] });
    font.add_glyph('B', Glyph { width: 1, height: 1, parts: vec![GlyphPart::Char('B')] });
    
    println!("Before remove: has_char('A')={}, has_char('B')={}, count={}", 
        font.has_char('A'), font.has_char('B'), font.glyph_count());
    
    let removed = font.remove_glyph('A');
    println!("After remove_glyph('A'): removed={}, has_char('A')={}, has_char('B')={}, count={}", 
        removed, font.has_char('A'), font.has_char('B'), font.glyph_count());
    
    // Test 2: Lazy-Glyph entfernen (aus geparster Datei)
    let bytes = font.to_bytes().unwrap();
    let mut parsed = TdfFont::load(&bytes).unwrap().into_iter().next().unwrap();
    
    println!("\nParsed font: has_char('B')={}, count={}", parsed.has_char('B'), parsed.glyph_count());
    
    let removed2 = parsed.remove_glyph('B');
    println!("After remove_glyph('B'): removed={}, has_char('B')={}, count={}", 
        removed2, parsed.has_char('B'), parsed.glyph_count());
}
