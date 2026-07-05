//! BBKEmu standalone frontend

use std::fs;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Parser;

use bbkemu_core::input::BbkKey;
use bbkemu_core::{model, BbkModel, Emulator};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

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
    #[arg(short, long, default_value = "4", value_parser = clap::value_parser!(u32).range(1..=16))]
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

    /// Run headless for this many frames and write --output
    #[arg(long)]
    frames: Option<u64>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let log_level = if cli.debug {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Warn)
        .filter_module("bbkemu", log_level)
        .filter_module("bbkemu_core", log_level)
        .init();

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
        let default_paths = ["system/BBKEmu/8.BIN", "8.BIN"];
        for path in &default_paths {
            if let Ok(data) = fs::read(path) {
                log::info!("Loading font ROM from {}", path);
                emu.load_rom_8(&data);
                break;
            }
        }
    }
    // Always load OS ROM - needed for D2F6 dispatch and proper initialization
    if let Some(path) = &cli.rome {
        let data = fs::read(path)?;
        emu.load_rom_e(&data);
    } else {
        let default_paths = ["system/BBKEmu/E.BIN", "E.BIN"];
        for path in &default_paths {
            if let Ok(data) = fs::read(path) {
                log::info!("Loading OS ROM from {}", path);
                emu.load_rom_e(&data);
                break;
            }
        }
    }

    // Load game
    let game_data = fs::read(&cli.game)?;
    emu.load_gam(&game_data)?;

    log::info!("Starting BBKEmu with model {}", bbk_model.name);
    log::info!("Game: {}", cli.game.display());
    log::info!("Scale: {}x", cli.scale);

    if let Some(frames) = cli.frames {
        run_headless(&mut emu, frames, &cli.output, cli.scale)?;
        return Ok(());
    }

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(emu, cli.scale, cli.fullscreen);
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn run_headless(emu: &mut Emulator, frames: u64, output: &PathBuf, scale: u32) -> Result<()> {
    for _ in 0..frames {
        emu.run_frame();
    }
    let lcd_buffer = emu.render_lcd_buffer();
    save_bmp(output, &lcd_buffer, scale)?;
    log::info!(
        "Emulation complete after {} frames at PC=0x{:04X} ({} cycles)",
        emu.frame_count(),
        emu.cpu.pc(),
        emu.cpu.cycles()
    );
    log::info!("Output saved to {}", output.display());
    Ok(())
}

struct App {
    emu: Emulator,
    scale: u32,
    fullscreen: bool,
    window: Option<Rc<Window>>,
    context: Option<Context<Rc<Window>>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    next_frame: Instant,
}

impl App {
    fn new(emu: Emulator, scale: u32, fullscreen: bool) -> Self {
        Self {
            emu,
            scale,
            fullscreen,
            window: None,
            context: None,
            surface: None,
            next_frame: Instant::now(),
        }
    }

    fn draw(&mut self) -> std::result::Result<(), String> {
        let window = self.window.as_ref().expect("window must exist");
        let surface = self.surface.as_mut().expect("surface must exist");
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);
        surface
            .resize(
                NonZeroU32::new(width).unwrap(),
                NonZeroU32::new(height).unwrap(),
            )
            .map_err(|error| error.to_string())?;

        let pixels = self.emu.render_lcd_buffer();
        let mut buffer = surface.buffer_mut().map_err(|error| error.to_string())?;
        for y in 0..height as usize {
            let source_y = y * 96 / height as usize;
            for x in 0..width as usize {
                let source_x = x * 159 / width as usize;
                buffer[y * width as usize + x] = if pixels[source_y * 159 + source_x] {
                    0x0014_1814
                } else {
                    0x00a8_b8a0
                };
            }
        }
        buffer.present().map_err(|error| error.to_string())?;
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let mut attributes = Window::default_attributes()
            .with_title("BBKEmu")
            .with_inner_size(LogicalSize::new(
                (159 * self.scale) as f64,
                (96 * self.scale) as f64,
            ))
            .with_min_inner_size(LogicalSize::new(159.0, 96.0));
        if self.fullscreen {
            attributes = attributes.with_fullscreen(Some(Fullscreen::Borderless(None)));
        }

        let window = match event_loop.create_window(attributes) {
            Ok(window) => Rc::new(window),
            Err(error) => {
                log::error!("Failed to create window: {error}");
                event_loop.exit();
                return;
            }
        };
        let context = match Context::new(window.clone()) {
            Ok(context) => context,
            Err(error) => {
                log::error!("Failed to create display context: {error}");
                event_loop.exit();
                return;
            }
        };
        let surface = match Surface::new(&context, window.clone()) {
            Ok(surface) => surface,
            Err(error) => {
                log::error!("Failed to create display surface: {error}");
                event_loop.exit();
                return;
            }
        };
        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
        self.next_frame = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.draw() {
                    log::error!("Failed to render frame: {error}");
                    event_loop.exit();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                if code == KeyCode::F12 && event.state == ElementState::Pressed {
                    let pixels = self.emu.render_lcd_buffer();
                    let path = PathBuf::from("bbkemu-screenshot.bmp");
                    match save_bmp(&path, &pixels, 1) {
                        Ok(()) => log::info!("Screenshot saved to {}", path.display()),
                        Err(error) => log::error!("Failed to save screenshot: {error}"),
                    }
                    return;
                }
                if code == KeyCode::Escape && event.state == ElementState::Pressed {
                    event_loop.exit();
                    return;
                }
                if let Some(key) = map_key(code) {
                    match event.state {
                        ElementState::Pressed => self.emu.key_down(key),
                        ElementState::Released => self.emu.key_up(),
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if now >= self.next_frame {
            self.emu.run_frame();
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            self.next_frame = now + Duration::from_micros(16_667);
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}

fn map_key(code: KeyCode) -> Option<BbkKey> {
    Some(match code {
        KeyCode::ArrowUp => BbkKey::Up,
        KeyCode::ArrowDown => BbkKey::Down,
        KeyCode::ArrowLeft => BbkKey::Left,
        KeyCode::ArrowRight => BbkKey::Right,
        KeyCode::Enter => BbkKey::Enter,
        KeyCode::Backspace => BbkKey::Exit,
        KeyCode::Delete => BbkKey::Del,
        KeyCode::Space => BbkKey::Space,
        KeyCode::PageUp => BbkKey::PgUp,
        KeyCode::PageDown => BbkKey::PgDn,
        KeyCode::Digit0 => BbkKey::Key0,
        KeyCode::Digit1 => BbkKey::Key1,
        KeyCode::Digit2 => BbkKey::Key2,
        KeyCode::Digit3 => BbkKey::Key3,
        KeyCode::Digit4 => BbkKey::Key4,
        KeyCode::Digit5 => BbkKey::Key5,
        KeyCode::Digit6 => BbkKey::Key6,
        KeyCode::Digit7 => BbkKey::Key7,
        KeyCode::Digit8 => BbkKey::Key8,
        KeyCode::Digit9 => BbkKey::Key9,
        KeyCode::KeyQ => BbkKey::Q,
        KeyCode::KeyW => BbkKey::W,
        KeyCode::KeyE => BbkKey::E,
        KeyCode::KeyR => BbkKey::R,
        KeyCode::KeyT => BbkKey::T,
        KeyCode::KeyY => BbkKey::Y,
        KeyCode::KeyU => BbkKey::U,
        KeyCode::KeyI => BbkKey::I,
        KeyCode::KeyO => BbkKey::O,
        KeyCode::KeyP => BbkKey::P,
        KeyCode::KeyA => BbkKey::A,
        KeyCode::KeyS => BbkKey::S,
        KeyCode::KeyD => BbkKey::D,
        KeyCode::KeyF => BbkKey::F,
        KeyCode::KeyG => BbkKey::G,
        KeyCode::KeyH => BbkKey::H,
        KeyCode::KeyJ => BbkKey::J,
        KeyCode::KeyK => BbkKey::K,
        KeyCode::KeyL => BbkKey::L,
        KeyCode::KeyZ => BbkKey::Z,
        KeyCode::KeyX => BbkKey::X,
        KeyCode::KeyC => BbkKey::C,
        KeyCode::KeyV => BbkKey::V,
        KeyCode::KeyB => BbkKey::B,
        KeyCode::KeyN => BbkKey::N,
        KeyCode::KeyM => BbkKey::M,
        _ => return None,
    })
}

fn save_bmp(path: &PathBuf, pixels: &[bool; 159 * 96], scale: u32) -> Result<()> {
    let width = 159 * scale;
    let height = 96 * scale;

    // BMP file format
    let row_size = width * 3;
    let row_stride = (row_size + 3) & !3;
    let file_size = 54 + row_stride * height;
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
            let padding = (row_stride - row_size) as usize;
            bmp.extend_from_slice(&vec![0u8; padding]);
        }
    }

    fs::write(path, bmp)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_navigation_and_action_keys() {
        assert_eq!(map_key(KeyCode::ArrowUp), Some(BbkKey::Up));
        assert_eq!(map_key(KeyCode::Enter), Some(BbkKey::Enter));
        assert_eq!(map_key(KeyCode::Backspace), Some(BbkKey::Exit));
    }

    #[test]
    fn leaves_unmapped_host_keys_available_to_frontend() {
        assert_eq!(map_key(KeyCode::F1), None);
        assert_eq!(map_key(KeyCode::Escape), None);
    }
}
