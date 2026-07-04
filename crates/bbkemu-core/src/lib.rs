//! BBKEmu Core - Platform-independent BBK game emulator engine
//!
//! This crate implements a BBK A-series electronic dictionary game emulator
//! using system call interception (HLE) instead of hardware register emulation.

pub mod cpu;
pub mod memory;
pub mod syscall;
pub mod syscalls;
pub mod gam;
pub mod emulator;
pub mod lcd;
pub mod input;
pub mod audio;
pub mod save;
pub mod debug;
pub mod model;

pub use emulator::Emulator;
pub use model::BbkModel;
