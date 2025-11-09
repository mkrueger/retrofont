# retrofont-cli

Command-line interface for the retrofont library - render and convert retro ASCII/ANSI art fonts (FIGlet and TheDraw formats).

![Sample rendering](assets/sample.png)

## Installation

```bash
cargo install retrofont-cli
```

## Usage

### Render Text

Render text using FIGlet or TDF fonts:

```bash
# FIGlet font
retrofont render --font fonts/doom.flf --text "Hello World"

# TDF font (TheDraw)
retrofont render --font fonts/block.tdf --text "Retro"

# With custom colors (DOS palette 0-15)
retrofont render --font fonts/color.tdf --text "Color" --fg 14 --bg 1

# Edit mode (shows construction markers)
retrofont render --font fonts/outline.tdf --text "Debug" --edit

# Outline font with specific style (0-18)
retrofont render --font fonts/outline.tdf --text "Style" --outline 5
```

### Convert Fonts

Convert between FIGlet and TDF formats:

```bash
# Convert FIGlet to TDF block font
retrofont convert --input font.flf --output font.tdf --type block

# Convert to color font (adds default DOS colors)
retrofont convert --input font.flf --output font.tdf --type color

# Convert to outline font
retrofont convert --input font.flf --output font.tdf --type outline
```

### Inspect Fonts

View font metadata and available characters:

```bash
# Show font information
retrofont inspect --font fonts/bundle.tdf

# Output:
# Font: ANSI Shadow
# Type: Block
# Characters: 94
# Spacing: 1
```

## Color Palette

The CLI uses the authentic DOS VGA 16-color palette:

| Index | Color         | RGB (hex) |
|-------|---------------|-----------|
| 0     | Black         | #000000   |
| 1     | Blue          | #0000AA   |
| 2     | Green         | #00AA00   |
| 3     | Cyan          | #00AAAA   |
| 4     | Red           | #AA0000   |
| 5     | Magenta       | #AA00AA   |
| 6     | Brown         | #AA5500   |
| 7     | Light Gray    | #AAAAAA   |
| 8     | Dark Gray     | #555555   |
| 9     | Light Blue    | #5555FF   |
| 10    | Light Green   | #55FF55   |
| 11    | Light Cyan    | #55FFFF   |
| 12    | Light Red     | #FF5555   |
| 13    | Light Magenta | #FF55FF   |
| 14    | Yellow        | #FFFF55   |
| 15    | White         | #FFFFFF   |

## Outline Styles

For outline fonts, 19 different rendering styles are available (0-18). Each style uses different box-drawing characters for rendering the font outlines.

## Supported Formats

- **FIGlet** (.flf): ASCII art fonts with hard blank support
- **FIGlet ZIP** (.flf as .zip): Compressed FIGlet fonts
- **TheDraw** (.tdf): DOS-era ANSI art fonts with color and outline support
- **TDF Bundles**: Multiple fonts in a single .tdf file

## Examples

```bash
# Render a banner
retrofont render --font fonts/banner.flf --text "WELCOME"

# Create colored output
retrofont render --font fonts/color.tdf --text "Rainbow" --fg 14 --bg 1

# Convert and render
retrofont convert --input ascii.flf --output ascii.tdf --type color
retrofont render --font ascii.tdf --text "Converted!" --fg 10

# Inspect a font bundle
retrofont inspect --font fonts/collection.tdf
```

## Terminal Requirements

- UTF-8 support for proper box-drawing characters
- 256-color or truecolor support for accurate color rendering
- Monospace font recommended for proper alignment

## License

This project is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## See Also

- [retrofont library](https://crates.io/crates/retrofont) - The core library
- [Repository](https://github.com/mkrueger/retrofont) - Source code and issues