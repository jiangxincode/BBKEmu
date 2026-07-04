//! BBKEmu standalone frontend

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use bbkemu_core::{Emulator, BbkModel, model};

#[derive(Parser)]
#[command(name = "bbkemu")]
#[command(about = "BBK electronic dictionary game emulator")]
struct Cli {
    /// GAM file to load
    game: PathBuf,

    /// Font ROM file (8.BIN) - optional
    #[arg(short = '8', long)]
    rom8: Option<PathBuf>,

    /// OS ROM file (E.BIN) - optional, for LLE mode
    #[arg(short = 'e', long)]
    rome: Option<PathBuf>,

    /// Display scale factor
    #[arg(short, long, default_value = "4")]
    scale: u32,

    /// Start in fullscreen mode
    #[arg(short, long)]
    fullscreen: bool,

    /// BBK model (4980, 4988)
    #[arg(short, long, default_value = "4980")]
    model: String,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let log_level = if cli.debug {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };
    env_logger::Builder::new().filter_level(log_level).init();

    // Select model
    let bbk_model: &'static BbkModel = match cli.model.as_str() {
        "4980" => &model::MODEL_4980,
        "4988" => &model::MODEL_4988,
        _ => {
            log::warn!("Unknown model '{}', using A4980", cli.model);
            &model::MODEL_4980
        }
    };

    // Create emulator
    let mut emu = Emulator::new(bbk_model);

    // Load optional ROMs
    if let Some(path) = &cli.rom8 {
        let data = fs::read(path)?;
        emu.load_rom_8(&data);
    }
    if let Some(path) = &cli.rome {
        let data = fs::read(path)?;
        emu.load_rom_e(&data);
    }

    // Load game
    let game_data = fs::read(&cli.game)?;
    emu.load_gam(&game_data)?;

    log::info!("Starting BBKEmu with model {}", bbk_model.name);
    log::info!("Game: {}", cli.game.display());
    log::info!("Scale: {}x", cli.scale);

    // TODO: Initialize window (winit + softbuffer)
    // TODO: Initialize audio (cpal)
    // TODO: Main loop: run frame + render + handle input

    // Temporary: run a few frames for testing
    for _ in 0..60 {
        emu.run_frame();
    }

    log::info!("Emulation complete. Frames: {}", emu.frame_count());

    Ok(())
}
