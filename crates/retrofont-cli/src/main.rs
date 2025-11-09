use anyhow::Result;
use clap::{Parser, Subcommand};
use retrofont::{
    convert::figlet_to_tdf, figlet::FigletFont, tdf::TdfFont, Font, FontType, RenderMode,
};
use std::fs;

use crate::console::render_to_ansi;
mod console;

#[derive(Parser)]
#[command(name = "retrofont", about = "Retro font toolkit CLI")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

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
            font, text, edit, ..
        } => {
            let bytes = fs::read(&font)?;
            let mode = if edit {
                RenderMode::Edit
            } else {
                RenderMode::Display
            };
            // crude format detection
            let ansi = if font.ends_with(".flf") {
                let f = FigletFont::from_bytes(&bytes)?;
                render_to_ansi(&f, &text, mode)?
            } else {
                let fonts = TdfFont::from_bytes(&bytes)?; // bundle may contain multiple; take first
                let f = fonts
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("no font in file"))?;
                render_to_ansi(&f, &text, mode)?
            };
            println!("{ansi}");
        }
        Cmd::Convert { input, output, ty } => {
            let bytes = fs::read(&input)?;
            let fig = FigletFont::from_bytes(&bytes)?;
            let target_type = match ty.to_lowercase().as_str() {
                "outline" => FontType::Outline,
                "block" => FontType::Block,
                "color" => FontType::Color,
                _ => FontType::Color,
            };
            let _tdf = figlet_to_tdf(&fig, target_type)?;
            // placeholder serialization (real TDF writer TBD)
            fs::write(&output, b"TDF_PLACEHOLDER")?;
            eprintln!("Converted FIGlet -> TDF ({target_type:?}) -> {output}");
        }
        Cmd::Inspect { font } => {
            let bytes = fs::read(&font)?;
            if font.ends_with(".flf") {
                let f = FigletFont::from_bytes(&bytes)?;
                println!("FIGlet font: {}", f.name());
                println!("  Defined characters: {}", f.glyph_count());
            } else {
                let fonts = TdfFont::from_bytes(&bytes)?;
                let font_count = fonts.len();
                if font_count > 1 {
                    println!("TDF bundle: {} fonts", font_count);
                }
                for (idx, f) in fonts.iter().enumerate() {
                    if font_count > 1 {
                        println!("\nFont #{}: {} ({:?})", idx + 1, f.name(), f.font_type());
                    } else {
                        println!("TDF font: {} ({:?})", f.name(), f.font_type());
                    }
                    println!("  Defined characters: {}", f.char_count());
                }
            }
        }
    }
    Ok(())
}
