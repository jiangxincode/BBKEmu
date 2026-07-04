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

    log::info!("BBKEmu starting...");

    // Select model
    let bbk_model: &'static BbkModel = match cli.model.as_str() {
        "4980" => &model::MODEL_4980,
        "4988" => &model::MODEL_4988,
        _ => {
            log::warn!("Unknown model '{}', using A4980", cli.model);
            &model::MODEL_4980
        }
    };

    log::info!("Creating emulator...");
    let mut emu = Emulator::new(bbk_model);
    log::info!("Emulator created");

    // Load optional ROMs
    let mut has_rom = false;
    if let Some(path) = &cli.rom8 {
        let data = fs::read(path)?;
        emu.load_rom_8(&data);
        has_rom = true;
    } else {
        // Try to load font ROM from default location
        let default_paths = [
            "tmp/gam4980/retroarch/system/gam4980/8.BIN",
            "system/gam4980/8.BIN",
            "8.BIN",
        ];
        for path in &default_paths {
            if let Ok(data) = fs::read(path) {
                log::info!("Loading font ROM from {}", path);
                emu.load_rom_8(&data);
                has_rom = true;
                break;
            }
        }
    }
    if let Some(path) = &cli.rome {
        let data = fs::read(path)?;
        emu.load_rom_e(&data);
        has_rom = true;
    }

    // Run OS initialization if ROM is loaded
    // Skip for now - bank table is set up manually in load_gam
    // if has_rom {
    //     emu.run_os_init();
    // }

    // Load game
    let game_data = fs::read(&cli.game)?;
    emu.load_gam(&game_data)?;

    log::info!("Starting BBKEmu with model {}", bbk_model.name);
    log::info!("Game: {}", cli.game.display());
    log::info!("Scale: {}x", cli.scale);

    // Run emulation
    // In a real implementation, this would be a window event loop
    // For now, just run a fixed number of frames
    let target_fps = 60;
    let frame_duration = std::time::Duration::from_micros(1_000_000 / target_fps);

    log::info!("Running emulation at {} FPS...", target_fps);

    for frame in 0..600 {
        let start = std::time::Instant::now();

        // Run one frame
        emu.run_frame();

        // Handle input (placeholder)
        // In real implementation, this would read from window events

        // Render (placeholder)
        // In real implementation, this would render to window

        // Frame timing
        let elapsed = start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }

        // Log progress every 60 frames
        if frame % 60 == 0 {
            log::info!("Frame {}: {} cycles", frame, emu.cpu.cycles());
        }
    }

    log::info!("Emulation complete. Frames: {}", emu.frame_count());

    Ok(())
}
