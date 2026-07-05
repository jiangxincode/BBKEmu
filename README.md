# BBKEmu —— BBK Electronic Dictionary Game Emulator

<p align="center">
  <a href="https://github.com/jiangxincode/BBKEmu/actions/workflows/ci.yml"><img src="https://github.com/jiangxincode/BBKEmu/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/jiangxincode/BBKEmu/releases/latest"><img src="https://img.shields.io/github/v/release/jiangxincode/BBKEmu" alt="Release"></a>
  <a href="https://github.com/jiangxincode/BBKEmu/releases"><img src="https://img.shields.io/github/downloads/jiangxincode/BBKEmu/total" alt="Downloads"></a>
  <a href="https://sonarcloud.io/dashboard?id=jiangxincode_BBKEmu"><img src="https://sonarcloud.io/api/project_badges/measure?project=jiangxincode_BBKEmu&metric=alert_status" alt="Quality Gate Status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-BSD%203--Clause-blue.svg" alt="License: BSD 3-Clause"></a>
</p>

BBK (步步高) A-series electronic dictionary game emulator written in Rust.

## Features

- **Complete 6502 CPU emulation** — using the mos6502 crate for accurate instruction execution
- **Bank-switched memory** — full memory map with flash, RAM, and ROM support
- **LCD display** — 159×96 monochrome framebuffer with ghosting effects
- **Keyboard input** — complete BBK key matrix emulation
- **Audio system** — tone generation with configurable frequency and duration
- **GAM file support** — loads BBK game files with automatic header parsing
- **Multiple models** — supports BBK A4980 and A4988 dictionaries
- **RetroArch integration** — libretro core for use with RetroArch frontend
- **Developer tools** — debugger with breakpoints, watchpoints, and syscall logging
- **Save states** — serialization support for save/load functionality

## Requirements

BBKEmu requires ROM files from a physical BBK dictionary to run games:

- **8.BIN** — Font ROM (2 MiB)
- **E.BIN** — OS ROM (2 MiB)

These files are not distributed with the emulator and must be obtained separately.

## Usage

### Standalone Mode

Download the latest binary from the [Releases](https://github.com/jiangxincode/BBKEmu/releases) page.

```bash
# Basic usage with ROM files
bbkemu game.gam -8 8.BIN -e E.BIN

# With options
bbkemu game.gam -8 8.BIN -e E.BIN --scale 4 --model 4980 --debug --fullscreen

# If ROM files are in system/BBKEmu/<model>/, system/BBKEmu/, or current directory
bbkemu game.gam
```

**Command-line options:**

| Option | Description | Default |
|--------|-------------|---------|
| `<GAME>` | GAM file to load | (required) |
| `-8, --rom8 <FILE>` | Font ROM file (8.BIN) | none |
| `-e, --rome <FILE>` | OS ROM file (E.BIN) | none |
| `-s, --scale <N>` | Display scale factor (1-16) | 4 |
| `-f, --fullscreen` | Start in fullscreen mode | false |
| `-m, --model <MODEL>` | BBK model (4980, 4988) | 4980 |
| `-d, --debug` | Enable debug logging | false |
| `-o, --output <FILE>` | Output BMP file path | output.bmp |
| `--frames <N>` | Run headless for N frames and exit | none |

### RetroArch Mode

BBKEmu can be used as a libretro core with RetroArch.

**Install the core:**

1. Build the libretro core:
   ```bash
   cargo build --release -p bbkemu-libretro
   ```
2. Copy the core file to RetroArch's `cores/` directory:
   - Windows: `target/release/bbkemu_libretro.dll`
   - Linux: `target/release/libbbkemu_libretro.so`
   - macOS: `target/release/libbbkemu_libretro.dylib`
3. Place ROM files in the appropriate directory:
   - **Recommended:** `system/BBKEmu/<model>/` (e.g., `system/BBKEmu/A4980/`)
   - **Legacy:** `system/BBKEmu/` or `system/`

**Load the core in RetroArch:**

1. Open RetroArch
2. Select "Load Core" → "BBKEmu"
3. Select "Load Content" and choose a `.gam` game file

#### Supported Features

- ✅ Video output (RGB565 pixel format)
- ✅ Audio output (tone generation)
- ✅ Input handling (keyboard mapping)
- ✅ Game loading (.gam files)

## Building

Requires [Rust](https://www.rust-lang.org/tools/install) (stable).

### Standalone Mode (Default)

```bash
cargo build -p bbkemu --release
cargo run -p bbkemu --release -- game.gam -8 8.BIN -e E.BIN
```

The binary is produced at `target/release/bbkemu` (or `bbkemu.exe` on Windows).

### Libretro Core (for RetroArch)

```bash
cargo build -p bbkemu-libretro --release
```

Cargo names the cdylib after its lib target, so this produces `bbkemu.dll`
on Windows (`libbbkemu.so` on Linux, `libbbkemu.dylib` on macOS) under
`target/release/`. Rename it to `bbkemu_libretro.<ext>` before dropping it into
RetroArch's `cores/` directory.

### ROM Analyzer Tool

```bash
cargo build -p bbkemu-rom-analyzer --release
```

This tool analyzes BBK OS ROM files for emulation development and debugging.

## Testing

Run the unit tests:

```bash
cargo test --workspace
```

## Architecture

```
crates/
├── bbkemu-core/               # Platform-independent emulator engine (library)
│   └── src/
│       ├── lib.rs             # Crate root (module declarations)
│       ├── emulator.rs        # Main emulator orchestrator
│       ├── cpu.rs             # 6502 CPU wrapper (mos6502 crate)
│       ├── memory.rs          # Memory bus with bank switching
│       ├── lcd.rs             # LCD framebuffer (159×96 monochrome)
│       ├── input.rs           # Keyboard input handling
│       ├── audio.rs           # Audio tone generation
│       ├── font.rs            # Font rendering
│       ├── font_data.rs       # Built-in font bitmap data
│       ├── gam.rs             # GAM file loader and parser
│       ├── model.rs           # BBK model definitions (A4980, A4988)
│       ├── debug.rs           # Debugger with breakpoints and watchpoints
│       └── save.rs            # Save state serialization
├── bbkemu/                    # Standalone binary (-> bbkemu)
│   └── src/
│       └── main.rs            # Window loop and CLI frontend
├── bbkemu-libretro/           # libretro cdylib (-> bbkemu_libretro.{dll,so,dylib})
│   └── src/
│       └── lib.rs             # libretro API implementation
└── bbkemu-rom-analyzer/       # ROM analysis tool
    └── src/
        └── main.rs            # OS ROM analyzer
```

## How It Works

BBK games are compiled 6502 machine code running on the BBK dictionary hardware.
The emulator executes the actual OS ROM code (E.BIN) and font ROM (8.BIN) to
provide the runtime environment for games:

```
Game → JSR to OS address → Execute OS ROM code → Hardware emulation
```

The 6502 CPU executes instructions from the memory-mapped ROM, while the
emulator provides hardware register emulation for LCD, keyboard, audio, and
timers.

## Documentation

- [Architecture](docs/ARCHITECTURE.md) — Project structure and design
- [Memory Map](docs/MEMORY-MAP.md) — Memory layout, hardware registers, and keyboard matrix
- [System Calls](docs/SYSCALLS.md) — System call reference
- [GAM Format](docs/GAM_FORMAT.md) — File format specification
- [Tested Games](docs/TESTED-GAMES.md) — Game compatibility list

## Game Compatibility

Game resources can be downloaded from [Baidu Netdisk](https://pan.baidu.com/s/1CuNeJe-RKXG_E-LhdI5ldg?pwd=aloy).

| Game | Status | Notes |
|------|--------|-------|
| Various .gam files | ⚠️ Partial | Basic loading works; requires ROM files |

## Contribute

Contributions are welcome! Whether you're interested in fixing bugs, adding features, improving documentation, or testing game compatibility, we'd love your help.

### Development Setup

```bash
# Clone the repository
git clone https://github.com/jiangxincode/BBKEmu.git
cd BBKEmu

# Place ROM files
mkdir -p system/BBKEmu
cp /path/to/8.BIN system/BBKEmu/
cp /path/to/E.BIN system/BBKEmu/

# Build and run
cargo run --release -p bbkemu -- game.gam

# Run tests
cargo test --workspace

# Run with debug logging
RUST_LOG=bbkemu=debug cargo run --release -p bbkemu -- game.gam --debug
```

## Acknowledgments

- [mos6502](https://crates.io/crates/mos6502) — 6502 CPU emulation library

## License

This project is licensed under the [BSD 3-Clause License](LICENSE).
