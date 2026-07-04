# BBKEmu

BBK (步步高) A-series electronic dictionary game emulator using **system call
interception (HLE)** instead of hardware register emulation.

## What is BBKEmu?

BBKEmu runs games from BBK electronic dictionaries (like the A4980 and A4988)
without requiring ROM files from physical hardware. It intercepts system calls
made by games to the operating system and reimplements them in Rust.

### Key Features

- **No ROM files required** - Runs games without needing 8.BIN or E.BIN dumps
- **Multiple models** - Supports BBK A4980 and A4988 (more planned)
- **Dual frontend** - Standalone application and libretro core for RetroArch
- **Rust implementation** - Clean, safe, and maintainable codebase
- **Developer tools** - Debugger, syscall logger, memory viewer

## Architecture

```
BBKEmu/
├── crates/
│   ├── bbkemu-core/        # Platform-independent emulator engine
│   ├── bbkemu/             # Standalone frontend
│   └── bbkemu-libretro/    # libretro core
└── docs/
    ├── DESIGN.md           # Architecture document
    ├── SYSCALLS.md         # System call reference
    └── GAM_FORMAT.md       # GAM file format spec
```

## Building

```bash
# Build everything
cargo build --release

# Build standalone only
cargo build --release -p bbkemu

# Build libretro core only
cargo build --release -p bbkemu-libretro
```

## Usage

### Standalone

```bash
# Run a game
cargo run --release -p bbkemu -- game.gam

# With options
cargo run --release -p bbkemu -- game.gam --scale 4 --model 4980 --debug
```

### RetroArch

1. Build the libretro core: `cargo build --release -p bbkemu-libretro`
2. Copy `target/release/bbkemu_libretro.dll` (or `.so` on Linux) to RetroArch's
   cores directory
3. Load the core and select your `.gam` file

## How It Works

BBK games are compiled 6502 machine code. When they need to draw on screen,
read keyboard input, or play audio, they call operating system routines via JSR
instructions to fixed addresses in the OS ROM.

BBKEmu intercepts these JSR calls and handles them directly in Rust, without
needing the actual OS ROM:

```
Game → JSR to OS address → BBKEmu intercepts → Rust implementation
```

## Status

⚠️ **Early development** - The project structure and architecture are in place,
but many system calls are not yet implemented. Game compatibility is limited.

### Current Status

- [x] Project structure (3 crates)
- [x] 6502 CPU (basic instruction set)
- [x] Memory bus with bank switching
- [x] GAM file loader
- [x] System call framework
- [x] LCD framebuffer
- [x] Input handling
- [x] Audio system (basic)
- [ ] Complete syscall implementations
- [ ] Integration with mos6502 crate for accuracy
- [ ] Window rendering (standalone)
- [ ] Audio output
- [ ] Save states
- [ ] Debugging tools
- [ ] Multi-model support

## Documentation

- [Design Document](docs/DESIGN.md) - Architecture and implementation plan
- [System Calls](docs/SYSCALLS.md) - System call reference
- [GAM Format](docs/GAM_FORMAT.md) - File format specification

## License

BSD-3-Clause
