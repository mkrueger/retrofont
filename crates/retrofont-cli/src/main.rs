use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use retrofont::{
    Font, Justify, Layout, RenderOptions, TextOptions,
    convert::figlet_to_tdf,
    figlet::FigletFont,
    tdf::{TdfFont, TdfFontType},
};
use std::fs;

use crate::console::render_to_ansi;
mod console;

fn validate_outline_style(s: &str) -> Result<usize, String> {
    let value: usize = s
        .parse()
        .map_err(|_| format!("'{}' is not a valid number", s))?;

    if value >= OUTLINE_STYLE_COUNT {
        Err(format!(
            "outline style {} is out of range (valid: 0..{})",
            value,
            OUTLINE_STYLE_COUNT - 1
        ))
    } else {
        Ok(value)
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum LayoutArg {
    /// The font's own layout
    Default,
    /// Every glyph at full width
    Full,
    /// Move glyphs together until they touch
    Kern,
    /// Overlap glyphs using the font's smushing rules
    Smush,
}

impl From<LayoutArg> for Layout {
    fn from(arg: LayoutArg) -> Self {
        match arg {
            LayoutArg::Default => Layout::FontDefault,
            LayoutArg::Full => Layout::FullWidth,
            LayoutArg::Kern => Layout::Kerning,
            LayoutArg::Smush => Layout::Smushing,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum JustifyArg {
    Left,
    Center,
    Right,
}

impl From<JustifyArg> for Justify {
    fn from(arg: JustifyArg) -> Self {
        match arg {
            JustifyArg::Left => Justify::Left,
            JustifyArg::Center => Justify::Center,
            JustifyArg::Right => Justify::Right,
        }
    }
}

#[derive(Parser)]
#[command(name = "retrofont", about = "Retro font toolkit CLI")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}
const OUTLINE_STYLE_COUNT: usize = 19;

#[derive(Subcommand)]
enum Cmd {
    /// Render text with a font
    Render {
        #[arg(short, long)]
        font: String,
        #[arg(
            short,
            long,
            help = "Text to render; newlines start a new line of glyphs"
        )]
        text: String,
        #[arg(
            long,
            value_enum,
            default_value = "default",
            help = "Horizontal glyph layout"
        )]
        layout: LayoutArg,
        #[arg(long, value_enum, default_value = "left")]
        justify: JustifyArg,
        #[arg(short, long, help = "Wrap lines wider than this many columns")]
        width: Option<usize>,
        #[arg(long, default_value = "7")]
        fg: u8,
        #[arg(long, default_value = "0")]
        bg: u8,
        #[arg(
            long,
            default_value = "0",
            help = "Outline style index (0..18). Only used for outline/convert modes.",
            value_parser = validate_outline_style
        )]
        outline: usize,
        #[arg(long)]
        edit: bool,
        #[arg(
            short,
            long,
            default_value = "1",
            help = "Font number in TDF bundle (1-based). Use 'inspect' to see available fonts."
        )]
        num: usize,
    },
    /// Convert FIGlet (.flf) to TDF
    Convert {
        #[arg(short, long)]
        input: String,
        #[arg(short, long)]
        output: String,
        #[arg(long = "type", alias = "ty", default_value = "color")]
        ty: String,
        #[arg(
            short,
            long,
            default_value = "1",
            help = "Font number in TDF bundle to convert (1-based). Use 'inspect' to see available fonts."
        )]
        num: usize,
    },
    /// Inspect font metadata
    Inspect {
        #[arg(short, long)]
        font: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Render {
            font,
            text,
            layout,
            justify,
            width,
            edit,
            outline,
            num,
            ..
        } => {
            // Extra defensive check (in case future changes bypass clap range)
            if outline >= OUTLINE_STYLE_COUNT {
                anyhow::bail!(
                    "Outline style {} out of range (valid: 0..={})",
                    outline,
                    OUTLINE_STYLE_COUNT - 1
                );
            }

            if num == 0 {
                anyhow::bail!("Font number must be 1 or greater (1-based index)");
            }

            let bytes = fs::read(&font)?;
            let mut mode = if edit {
                RenderOptions::edit()
            } else {
                RenderOptions::default()
            };
            mode.outline_style = outline;
            // crude format detection
            let font_enum = if font.ends_with(".flf") {
                if num > 1 {
                    anyhow::bail!("FIGlet files contain only one font, --num must be 1");
                }
                Font::Figlet(Box::new(FigletFont::load(&bytes)?))
            } else {
                let fonts = TdfFont::load(&bytes)?;
                let font_count = fonts.len();
                if font_count == 0 {
                    anyhow::bail!("No fonts found in TDF file");
                }
                if num > font_count {
                    anyhow::bail!(
                        "Font #{} does not exist. TDF bundle contains {} font(s). Use 'inspect' to list available fonts.",
                        num,
                        font_count
                    );
                }
                Font::Tdf(Box::new(fonts.into_iter().nth(num - 1).ok_or_else(
                    || anyhow::anyhow!("Font #{num} not found in TDF bundle"),
                )?))
            };
            let options = TextOptions {
                render: mode,
                layout: layout.into(),
                justify: justify.into(),
                max_width: width,
                ..TextOptions::default()
            };
            let ansi = render_to_ansi(&font_enum, &text, &options)?;
            println!("{ansi}");
        }

        Cmd::Convert {
            input,
            output,
            ty,
            num,
        } => {
            if num == 0 {
                anyhow::bail!("Font number must be 1 or greater (1-based index)");
            }

            let bytes = fs::read(&input)?;

            // Currently only FIGlet to TDF conversion is supported
            if !input.ends_with(".flf") {
                anyhow::bail!("Convert currently only supports FIGlet (.flf) input files");
            }

            if num > 1 {
                anyhow::bail!("FIGlet files contain only one font, --num must be 1");
            }

            let fig = FigletFont::load(&bytes)?;
            let target_type = match ty.to_lowercase().as_str() {
                "outline" => TdfFontType::Outline,
                "block" => TdfFontType::Block,
                "color" => TdfFontType::Color,
                _ => TdfFontType::Color,
            };
            let tdf = figlet_to_tdf(&fig, target_type)?;
            // placeholder serialization (real TDF writer TBD)
            match tdf.to_bytes() {
                Ok(bytes) => fs::write(&output, bytes)?,
                Err(e) => eprintln!("Failed to convert TDF font to bytes: {e}"),
            }
        }
        Cmd::Inspect { font } => {
            let bytes = fs::read(&font)?;
            if font.ends_with(".flf") {
                let f = FigletFont::load(&bytes)?;
                println!("FIGlet font: {}", f.name);
                println!("  Defined characters: {}", f.glyph_count());
            } else {
                let fonts = TdfFont::load(&bytes)?;
                let font_count = fonts.len();
                if font_count > 1 {
                    println!("TDF bundle: {} fonts", font_count);
                }
                for (idx, f) in fonts.iter().enumerate() {
                    if font_count > 1 {
                        println!("\nFont #{}: {} ({:?})", idx + 1, f.name, f.font_type());
                    } else {
                        println!("TDF font: {} ({:?})", f.name, f.font_type());
                    }
                    println!("  Defined characters: {}", f.glyph_count());
                }
            }
        }
    }
    Ok(())
}
