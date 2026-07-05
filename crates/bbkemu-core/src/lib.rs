//! BBKEmu Core - Platform-independent BBK game emulator engine
//!
//! This crate implements a BBK A-series electronic dictionary game emulator.

pub mod cpu;
pub mod memory;
pub mod gam;
pub mod emulator;
pub mod lcd;
pub mod input;
pub mod audio;
pub mod save;
pub mod debug;
pub mod model;
pub mod font_data;

pub use emulator::Emulator;
pub use model::BbkModel;
