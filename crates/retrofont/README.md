# retrofont

A Rust library for parsing, rendering, and converting retro ASCII/ANSI art fonts, supporting both FIGlet and TheDraw (TDF) formats with full Unicode support.

## Features

- 🎨 **Multiple Font Formats**: Parse and render both FIGlet (.flf) and TheDraw (.tdf) fonts
- 🔄 **Format Conversion**: Convert between FIGlet and TDF formats with compatibility checking
- 🌍 **Unicode Support**: Automatic CP437 to Unicode conversion with proper character mapping
- 🎭 **Rendering Modes**: Display mode for final output, Edit mode for font development
- 📦 **Bundle Support**: Handle TDF files containing multiple fonts
- 🗜️ **Archive Support**: Load FIGlet fonts from ZIP files
- 🎨 **Color Support**: Full 16-color DOS palette with blink attribute
- 🔧 **Outline Styles**: 19 different outline rendering styles

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
retrofont = "0.1.2"
```

## Quick Start

```rust
use retrofont::{Font, RenderOptions, test_support::BufferTarget};

fn main() -> retrofont::Result<()> {
    // Load a font (auto-detects format)
    let data = std::fs::read("fonts/doom.flf")?;
    let fonts = Font::from_bytes(&data)?;
    let font = &fonts[0];  // FIGlet returns one font, TDF can have multiple
    
    // Create a rendering target
    let mut target = BufferTarget::new();
    let options = RenderOptions::default();
    
    // Render text character by character
    for ch in "HELLO".chars() {
        font.render_char(&mut target, ch, &options)?;
        target.next_char();  // Advance to next character position
    }
    
    // Get the result
    println!("{}", target.to_string());
    Ok(())
}
```

## Implementing Custom Render Targets

Create your own output format by implementing the `FontTarget` trait:

```rust
use retrofont::{FontTarget, Cell};
use std::fmt;

struct HtmlTarget {
    html: String,
}

impl FontTarget for HtmlTarget {
    type Error = fmt::Error;
    
    fn draw(&mut self, cell: Cell) -> Result<(), Self::Error> {
        // Escape HTML characters
        let ch = match cell.ch {
            '<' => "&lt;",
            '>' => "&gt;",
            '&' => "&amp;",
            c => {
                self.html.push(c);
                return Ok(());
            }
        };
        self.html.push_str(ch);
        Ok(())
    }
    
    fn next_line(&mut self) -> Result<(), Self::Error> {
        self.html.push_str("<br>\n");
        Ok(())
    }
    
    fn next_char(&mut self) -> Result<(), Self::Error> {
        // Optional: Handle character spacing
        Ok(())
    }
}
```

## Format Conversion

Convert FIGlet fonts to TheDraw format:

```rust
use retrofont::{
    figlet::FigletFont,
    tdf::TdfFontType,
    convert::{convert_to_tdf, is_figlet_compatible_with_tdf}
};

fn convert_font() -> retrofont::Result<()> {
    // Load FIGlet font
    let data = std::fs::read("input.flf")?;
    let figlet = FigletFont::from_bytes(&data)?;
    
    // Check compatibility
    if is_figlet_compatible_with_tdf(&figlet, TdfFontType::Block) {
        // Convert to TDF
        let tdf = convert_to_tdf(&figlet, TdfFontType::Block)?;
        
        // Serialize to bytes
        let tdf_bytes = tdf.as_tdf_bytes()?;
        std::fs::write("output.tdf", tdf_bytes)?;
    }
    Ok(())
}
```

## Working with TDF Bundles

```rust
use retrofont::{Font, tdf::TdfFont};

fn handle_bundle() -> retrofont::Result<()> {
    // Load a TDF bundle (multiple fonts)
    let data = std::fs::read("bundle.tdf")?;
    let fonts = Font::from_bytes(&data)?;
    
    // Iterate through fonts
    for (i, font) in fonts.iter().enumerate() {
        println!("Font {}: {}", i, font.name());
        
        // Check character availability
        if font.has_char('A') {
            // Render specific character
            let mut target = BufferTarget::new();
            font.render_char(&mut target, 'A', &RenderOptions::default())?;
        }
    }
    
    // Create a new bundle
    if let Font::Tdf(tdf1) = &fonts[0] {
        if let Font::Tdf(tdf2) = &fonts[1] {
            let bundle = TdfFont::create_bundle(&[tdf1.clone(), tdf2.clone()])?;
            std::fs::write("new_bundle.tdf", bundle)?;
        }
    }
    
    Ok(())
}
```

## Render Options

Control rendering behavior with `RenderOptions`:

```rust
use retrofont::{RenderOptions, RenderMode};

// Default: Display mode
let opts = RenderOptions::default();

// Edit mode: shows font construction markers
let opts = RenderOptions::edit();

// Custom configuration
let opts = RenderOptions {
    render_mode: RenderMode::Display,
    outline_style: 5,  // Use outline style 5 (0-18 available)
};
```

## Stream-based Loading

Load fonts from any `Read` source:

```rust
use std::io::Cursor;
use retrofont::Font;

fn load_from_memory(data: Vec<u8>) -> retrofont::Result<Vec<Font>> {
    let cursor = Cursor::new(data);
    Font::from_reader(cursor)
}
```

## Font Types

### FigletFont
- Text-based ASCII art fonts
- Supports hard blanks (non-breaking spaces)
- ZIP archive support for compressed fonts
- Character range: ASCII printable + extended

### TdfFont
- Binary format from TheDraw
- Three font types:
  - **Block**: Simple character-based
  - **Color**: Includes foreground/background colors and blink
  - **Outline**: Uses placeholders for box-drawing characters
- Character range: ASCII printable (! through ~)
- Bundle support (multiple fonts per file)

## Cell Attributes

Each rendered cell contains:

```rust
pub struct Cell {
    pub ch: char,           // Unicode character
    pub fg: Option<u8>,     // Foreground color (0-15)
    pub bg: Option<u8>,     // Background color (0-15)
    pub blink: bool,        // Blink attribute
    pub bold: bool,         // Bold attribute (future use)
}
```

## Error Handling

The library uses a `Result<T>` type alias with `FontError`:

```rust
use retrofont::{Font, FontError, Result};

fn load_font(path: &str) -> Result<Vec<Font>> {
    match std::fs::read(path) {
        Ok(data) => Font::from_bytes(&data),
        Err(io_err) => Err(FontError::Parse(format!("Cannot read file: {}", io_err)))
    }
}
```

## Feature Flags

```toml
[dependencies]
retrofont = { version = "0.1.2", default-features = false, features = ["tdf"] }
```

Available features:
- `tdf`: TheDraw font support (default)
- `figlet`: FIGlet font support (default)
- `convert`: Font conversion utilities (default)
- `color`: Color rendering support (default)

## Performance Considerations

- Glyphs stored in `HashMap<char, Glyph>` for memory efficiency
- Stream-based loading available via `from_reader()`
- Pre-rendered glyphs cached per font
- Optimized for repeated rendering of the same characters

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

Contributions welcome! Please ensure:
- Tests pass: `cargo test`
- No clippy warnings: `cargo clippy`
- Formatted: `cargo fmt`

## See Also

- [CLI tool](https://crates.io/crates/retrofont-cli) - Command-line interface
- [Repository](https://github.com/mkrueger/retrofont) - Source code
- [FIGlet](http://www.figlet.org/) - FIGlet documentation
- [TheDraw](https://en.wikipedia.org/wiki/TheDraw) - TheDraw information