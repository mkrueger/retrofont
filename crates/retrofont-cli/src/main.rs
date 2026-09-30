use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use retrofont::{
    Font, Justify, Layout, RenderOptions, TextOptions, convert::figlet_to_tdf,
    figlet::FigletFormat, tdf::TdfFontType,
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

/// Load font `num` (1-based) from a file, detecting the format from its content.
fn load_font(path: &str, num: usize) -> Result<Font> {
    if num == 0 {
        anyhow::bail!("Font number must be 1 or greater (1-based index)");
    }
    let fonts = Font::load(&fs::read(path)?)?;
    let count = fonts.len();
    fonts.into_iter().nth(num - 1).ok_or_else(|| {
        if count == 1 {
            anyhow::anyhow!("{path} contains a single font, --num must be 1")
        } else {
            anyhow::anyhow!(
                "Font #{num} does not exist. {path} contains {count} fonts. Use 'inspect' to list them."
            )
        }
    })
}

fn kind(font: &Font) -> String {
    match font {
        Font::Figlet(f) => match f.format() {
            FigletFormat::Flf => "FIGlet font".to_string(),
            FigletFormat::Tlf => "TOIlet font".to_string(),
        },
        Font::Tdf(f) => format!("TDF font ({:?})", f.font_type()),
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
    /// Convert a FIGlet (.flf) or TOIlet (.tlf) font to TDF
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

            let font_enum = load_font(&font, num)?;
            let mut mode = if edit {
                RenderOptions::edit()
            } else {
                RenderOptions::default()
            };
            mode.outline_style = outline;
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
            let Font::Figlet(fig) = load_font(&input, num)? else {
                anyhow::bail!("Convert only supports FIGlet (.flf) and TOIlet (.tlf) fonts");
            };
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
            let fonts = Font::load(&fs::read(&font)?)?;
            if fonts.len() > 1 {
                println!("TDF bundle: {} fonts", fonts.len());
            }
            for (idx, f) in fonts.iter().enumerate() {
                if fonts.len() > 1 {
                    println!("\nFont #{}: {} [{}]", idx + 1, f.name(), kind(f));
                } else {
                    println!("{}: {}", kind(f), f.name());
                }
                let count = match f {
                    Font::Figlet(f) => f.glyph_count(),
                    Font::Tdf(f) => f.glyph_count(),
                };
                println!("  Defined characters: {count}");
            }
        }
    }
    Ok(())
}
