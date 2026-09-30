# retrofont

A Rust library for parsing, rendering, and converting retro ASCII/ANSI art fonts, supporting both FIGlet and TheDraw (TDF) formats with full Unicode support.

## Features

- 🎨 **Multiple Font Formats**: Parse and render both FIGlet (.flf) and TheDraw (.tdf) fonts
- 🔄 **Format Conversion**: Convert FIGlet fonts to TDF with compatibility checking
- 🌍 **Unicode Support**: Automatic CP437 to Unicode conversion with proper character mapping
- 🎭 **Rendering Modes**: Display mode for final output, Edit mode for font development
- 📐 **Text Layout**: FIGlet-compatible kerning and smushing, multi-line text, word wrapping and justification
- 📦 **Bundle Support**: Handle TDF files containing multiple fonts
- 🗜️ **Archive Support**: Load FIGlet fonts from ZIP files
- 🎨 **Color Support**: Full 16-color DOS palette with blink attribute
- 🔧 **Outline Styles**: 19 different outline rendering styles

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
retrofont = "0.2"
```

## Quick Start

```rust,no_run
use retrofont::{test_support::MemoryBufferTarget, Font, TextOptions};

fn main() -> retrofont::Result<()> {
    // Load a font (auto-detects format)
    let data = std::fs::read("fonts/doom.flf")?;
    let fonts = Font::load(&data)?;
    let font = &fonts[0]; // FIGlet returns one font, TDF can have multiple

    // Create a rendering target
    let mut target = MemoryBufferTarget::new();

    // Render a whole string; FIGlet fonts are kerned/smushed like `figlet` does
    font.render_str(&mut target, "HELLO", &TextOptions::default())?;

    // Inspect the result
    for line in &target.lines {
        let text: String = line.iter().map(|cell| cell.ch).collect();
        println!("{text}");
    }
    Ok(())
}
```

## Implementing Custom Render Targets

Create your own output format by implementing the `FontTarget` trait. Only
`draw` and `next_line` are required.

```rust
use retrofont::{Cell, FontTarget};
use std::fmt;

struct HtmlTarget {
    html: String,
}

impl FontTarget for HtmlTarget {
    type Error = fmt::Error;

    fn draw(&mut self, cell: Cell) -> Result<(), Self::Error> {
        // Escape HTML characters
        match cell.ch {
            '<' => self.html.push_str("&lt;"),
            '>' => self.html.push_str("&gt;"),
            '&' => self.html.push_str("&amp;"),
            c => self.html.push(c),
        }
        Ok(())
    }

    fn next_line(&mut self) -> Result<(), Self::Error> {
        self.html.push_str("<br>\n");
        Ok(())
    }

    // Optional: transparent cells advance without drawing.
    fn skip(&mut self) -> Result<(), Self::Error> {
        self.html.push(' ');
        Ok(())
    }
}
```

## Format Conversion

Convert FIGlet fonts to TheDraw format:

```rust,no_run
use retrofont::{
    convert::{can_convert_figlet_to_tdf, figlet_to_tdf},
    figlet::FigletFont,
    tdf::TdfFontType,
};

fn convert_font() -> retrofont::Result<()> {
    // Load FIGlet font
    let data = std::fs::read("input.flf")?;
    let figlet = FigletFont::load(&data)?;

    // Check compatibility
    if can_convert_figlet_to_tdf(&figlet, TdfFontType::Block) {
        // Convert to TDF
        let tdf = figlet_to_tdf(&figlet, TdfFontType::Block)?;

        // Serialize to bytes
        let tdf_bytes = tdf.to_bytes()?;
        std::fs::write("output.tdf", tdf_bytes)?;
    }
    Ok(())
}
```

## Working with TDF Bundles

```rust,no_run
use retrofont::{tdf::TdfFont, test_support::MemoryBufferTarget, Font, RenderOptions};

fn handle_bundle() -> retrofont::Result<()> {
    // Load a TDF bundle (multiple fonts)
    let data = std::fs::read("bundle.tdf")?;
    let fonts = Font::load(&data)?;

    // Iterate through fonts
    for (i, font) in fonts.iter().enumerate() {
        println!("Font {}: {}", i, font.name());

        // Check character availability
        if font.has_char('A') {
            // Render specific character
            let mut target = MemoryBufferTarget::new();
            font.render_glyph(&mut target, 'A', &RenderOptions::default())?;
        }
    }

    // Write a new bundle containing every TDF font that was loaded
    let tdf_fonts: Vec<TdfFont> = fonts
        .iter()
        .filter_map(|f| match f {
            Font::Tdf(tdf) => Some((**tdf).clone()),
            _ => None,
        })
        .collect();
    let bundle = TdfFont::serialize_bundle(&tdf_fonts)?;
    std::fs::write("new_bundle.tdf", bundle)?;

    Ok(())
}
```

## Render Options

Control rendering behavior with `RenderOptions`:

```rust
use retrofont::{RenderMode, RenderOptions};

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

## Text Layout

`Font::render_str` lays out whole strings. `'\n'` starts a new line of glyphs,
and `TextOptions` controls how glyphs are joined and lines are arranged:

```rust
use retrofont::{Font, Justify, Layout, MissingGlyph, TextOptions, figlet::FigletFont};

let mut fig = FigletFont::new("demo");
fig.add_raw_char(b'H', &["H  H", "HHHH", "H  H"]);
fig.add_raw_char(b'I', &["III", " I ", "III"]);
let font = Font::Figlet(Box::new(fig));

let options = TextOptions {
    layout: Layout::Kerning,      // FontDefault, FullWidth, Kerning or Smushing
    justify: Justify::Center,     // Left, Center or Right
    max_width: Some(40),          // wrap at spaces (mid-word if a word is too long)
    missing: MissingGlyph::Skip,  // or Error (default)
    ..TextOptions::default()
};
let (width, height) = font.measure("HI\nIH", &options)?;
assert_eq!((width, height), (7, 6));
# Ok::<(), retrofont::FontError>(())
```

`Layout::FontDefault` uses the layout from the FIGlet header (`old_layout` /
`full_layout`), including all six horizontal smushing rules; TDF fonts are laid
out at full width. Use `FigletFont::layout` / `set_layout` with the bits in
`figlet::layout` to inspect or change a font's layout. Transparent cells
(padding, gaps) are emitted through `FontTarget::skip`.

`Font::render_glyph` is still available to render a single glyph.

## Stream-based Loading

Load fonts from any `Read` source:

```rust
use retrofont::Font;
use std::io::Cursor;

fn load_from_memory(data: Vec<u8>) -> retrofont::Result<Vec<Font>> {
    Font::read(Cursor::new(data))
}
```

For zero-copy loading of an owned buffer, use `Font::load_owned` or
`Font::load_arc`, which let TDF glyphs decode lazily from the original bytes.

## Font Types

### FigletFont

- Text-based ASCII art fonts
- Supports hard blanks (non-breaking spaces)
- Horizontal kerning and smushing as specified by the font header
- ZIP archive support for compressed fonts
- Character range: ASCII printable, the German letters (ÄÖÜäöüß) and code-tagged
  characters anywhere in Unicode (`FigletFont::add_char` accepts any `char`)
- UTF-8 fonts, with a Latin-1 fallback for older fonts
- Glyphs are read like figlet reads them: any end-mark character, blank glyphs are
  defined with zero width, and `MissingGlyph::Skip` renders the font's code-0
  glyph if it has one

### TdfFont

- Binary format from TheDraw
- Three font types:
  - **Block**: Simple character-based
  - **Color**: Includes foreground/background colors and blink
  - **Outline**: Uses placeholders for box-drawing characters
- Character range: ASCII printable (! through ~)
- Bundle support (multiple fonts per file)

## Cell Attributes

Each rendered cell carries the character plus its DOS attributes:

```rust
use retrofont::Cell;

let cell = Cell::new('A', Some(7), Some(0), false);
assert_eq!(cell.ch, 'A'); // Unicode character
assert_eq!(cell.fg, Some(7)); // Foreground color (0-15)
assert_eq!(cell.bg, Some(0)); // Background color (0-15)
assert!(!cell.blink); // Blink attribute
```

## Error Handling

The library uses a `Result<T>` type alias with `FontError`:

```rust
use retrofont::{Font, Result};

fn load_font(path: &str) -> Result<Vec<Font>> {
    let data = std::fs::read(path)?; // IO errors auto-convert via From
    Font::load(&data)
}
```

## Feature Flags

```toml
[dependencies]
retrofont = { version = "0.2", default-features = false, features = ["tdf"] }
```

Available features:

- `tdf`: TheDraw font support (default)
- `figlet`: FIGlet font support (default)
- `convert`: Font conversion utilities (default, implies `tdf` and `figlet`)
- `color`: Color rendering support
- `serde`: `Serialize`/`Deserialize` support for glyph and render types
- `native`: Full ZIP codec set including C-backed zstd and AES decryption (default, implies `pure-rust`)
- `pure-rust`: Pure Rust ZIP decompression (deflate, deflate64, bzip2, lzma, xz, ppmd); no C toolchain required

### WebAssembly / pure Rust builds

Disable default features to drop the C-backed `native` codecs, e.g. for `wasm32-unknown-unknown`:

```toml
[dependencies]
retrofont = { version = "0.2", default-features = false, features = ["pure-rust", "convert"] }
```

## Performance Considerations

- Glyphs are held in fixed-size tables indexed by character code, avoiding hashing
- Parsed glyphs decode lazily on first access and are cached per font
- `load_arc` shares the source buffer, so bundles decode without copying glyph data
- Stream-based loading available via `Font::read`

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
