use anyhow::Result;
use clap::{Parser, Subcommand};
use retrofont::{
    convert::figlet_to_tdf,
    figlet::FigletFont,
    tdf::{TdfFont, TdfFontType},
    Font, RenderOptions,
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
        #[arg(short, long)]
        text: String,
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
    },
    /// Convert FIGlet (.flf) to TDF
    Convert {
        #[arg(short, long)]
        input: String,
        #[arg(short, long)]
        output: String,
        #[arg(long, default_value = "color")]
        ty: String,
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
            edit,
            outline,
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

            let bytes = fs::read(&font)?;
            let mut mode = if edit {
                RenderOptions::edit()
            } else {
                RenderOptions::default()
            };
            mode.outline_style = outline;
            // crude format detection
            let font_enum = if font.ends_with(".flf") {
                Font::Figlet(FigletFont::load(&bytes)?)
            } else {
                let mut fonts = TdfFont::load(&bytes)?;
                let first = fonts
                    .drain(..)
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("no font in file"))?;
                Font::Tdf(first)
            };
            let ansi = render_to_ansi(&font_enum, &text, &mode)?;
            println!("{ansi}");
        }

        Cmd::Convert { input, output, ty } => {
            let bytes = fs::read(&input)?;
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
