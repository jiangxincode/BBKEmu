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

    /// Output BMP file path
    #[arg(short, long, default_value = "output.bmp")]
    output: PathBuf,

    /// Number of frames to run
    #[arg(long, default_value = "60")]
    frames: u64,
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
    if let Some(path) = &cli.rom8 {
        let data = fs::read(path)?;
        emu.load_rom_8(&data);
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
                break;
            }
        }
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

    // Run emulation
    let target_fps = 60u64;
    let frame_duration = std::time::Duration::from_micros(1_000_000 / target_fps);

    log::info!("Running {} frames at {} FPS...", cli.frames, target_fps);

    for frame in 0..cli.frames {
        let start = std::time::Instant::now();

        // Run one frame
        emu.run_frame();

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

    // Render final frame to BMP
    let lcd_buffer = emu.render_lcd_buffer();
    save_bmp(&cli.output, &lcd_buffer, cli.scale)?;

    log::info!("Output saved to {}", cli.output.display());

    Ok(())
}

fn save_bmp(path: &PathBuf, pixels: &[bool; 159 * 96], scale: u32) -> Result<()> {
    let width = 159 * scale;
    let height = 96 * scale;

    // BMP file format
    let file_size = 54 + (width * height * 3) as u32;
    let mut bmp = Vec::with_capacity(file_size as usize);

    // BMP header
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size.to_le_bytes());
    bmp.extend_from_slice(&[0, 0, 0, 0]); // Reserved
    bmp.extend_from_slice(&54u32.to_le_bytes()); // Offset to pixel data

    // DIB header
    bmp.extend_from_slice(&40u32.to_le_bytes()); // Header size
    bmp.extend_from_slice(&(width as i32).to_le_bytes());
    bmp.extend_from_slice(&(height as i32).to_le_bytes());
    bmp.extend_from_slice(&1u16.to_le_bytes()); // Planes
    bmp.extend_from_slice(&24u16.to_le_bytes()); // Bits per pixel
    bmp.extend_from_slice(&0u32.to_le_bytes()); // Compression
    bmp.extend_from_slice(&0u32.to_le_bytes()); // Image size
    bmp.extend_from_slice(&0i32.to_le_bytes()); // X pixels per meter
    bmp.extend_from_slice(&0i32.to_le_bytes()); // Y pixels per meter
    bmp.extend_from_slice(&0u32.to_le_bytes()); // Colors used
    bmp.extend_from_slice(&0u32.to_le_bytes()); // Important colors

    // Pixel data (BGR format, bottom-up)
    let bg_color = [0xDA, 0xD6, 0x00]; // Grey background (BGR)
    let fg_color = [0x00, 0x00, 0x00]; // Black foreground (BGR)

    for y in (0..96).rev() {
        for _dy in 0..scale {
            for x in 0..159 {
                let pixel = pixels[y * 159 + x];
                let color = if pixel { &fg_color } else { &bg_color };
                for _dx in 0..scale {
                    bmp.extend_from_slice(color);
                }
            }
            // Pad to 4-byte boundary
            let row_size = (width * 3) as usize;
            let padding = (4 - (row_size % 4)) % 4;
            bmp.extend_from_slice(&vec![0u8; padding]);
        }
    }

    fs::write(path, bmp)?;
    Ok(())
}
