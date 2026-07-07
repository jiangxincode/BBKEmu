# Memory Map and Hardware Registers

This document describes the BBK A-series memory architecture and hardware
registers emulated by BBKEmu.

## Memory Map

| Address Range | Size | Description |
|--------------|------|-------------|
| 0x0000-0x7FFF | 32 KiB | RAM |
| 0x8000-0xFFFF | 32 KiB | Bank-switched ROM/RAM window |
| 0x200000-0x3FFFFF | 2 MiB | Flash memory |
| 0x800000-0x9FFFFF | 2 MiB | Font ROM (8.BIN) |
| 0xE00000-0xFFFFFF | 2 MiB | OS ROM (E.BIN) |

### RAM Layout (0x0000-0x7FFF)

| Address | Description |
|---------|-------------|
| 0x0000-0x03FF | Zero page and stack |
| 0x0400-0x0FFF | LCD framebuffer (159×96 pixels) |
| 0x1000-0x7FFF | General purpose RAM |

### Bank Switching

The bank switch controller maps 16 × 4 KiB pages to physical addresses:

| Register | Address | Description |
|----------|---------|-------------|
| BANK_SEL | 0x0C | Bank select register (0-15) |
| BANK_ADR_L | 0x0D | Bank address low byte |
| BANK_ADR_H | 0x0E | Bank address high byte (4 bits) |

Default bank mappings:
- Bank 0: RAM page 0
- Bank 1-4: Flash pages 1-4
- Bank 5-8: Configurable (used for far calls)

## Hardware Registers

### Interrupt Controller

| Address | Name | Description |
|---------|------|-------------|
| 0x04 | ISR | Interrupt Status Register |
| 0x05 | TISR | Timer Interrupt Status Register |
| 0x23A | IER | Interrupt Enable Register |
| 0x23B | TIER | Timer Interrupt Enable Register |

#### ISR Bits (0x04)

| Bit | Name | Description |
|-----|------|-------------|
| 7 | PI | Keyboard interrupt |
| 6-2 | — | Reserved |
| 1 | CT | Counter interrupt |
| 0 | ALM | Alarm interrupt |

#### TISR Bits (0x05)

| Bit | Name | Description |
|-----|------|-------------|
| 3 | ST4 | Timer 4 status |
| 2 | ST3 | Timer 3 status |
| 1 | ST2 | Timer 2 status |
| 0 | ST1 | Timer 1 status |

### System Control

| Address | Name | Description |
|---------|------|-------------|
| 0x200 | SYSCON | System control register |
| 0x22B | MTCT | Main timer counter |

### Keyboard

| Address | Name | Description |
|---------|------|-------------|
| 0x24E | KEYCODE | Keyboard input register |

When a key is pressed, the corresponding keycode is written to this register.
Reading this register returns the last pressed key.

### LCD Framebuffer

The LCD is a 159×96 monochrome display. Each pixel is 1 bit, stored starting
at address 0x0400:

```
Address = 0x0400 + (y * 20) + (x / 8)
Bit = 7 - (x % 8)
```

Total size: 159 × 96 / 8 = 1,908 bytes (rounded to 20 × 96 = 1,920 bytes)

## OS ROM Addresses

The OS ROM (E.BIN) provides the operating system for BBK dictionaries. Games
call OS routines via JSR instructions to fixed addresses.

### Entry Points

| Address | Description |
|---------|-------------|
| 0xD000+ | OS dispatch table |
| 0xE000+ | System call implementations |

### System Call Categories

The OS ROM implements the following system call categories:

| Address Range | Category | Functions |
|--------------|----------|-----------|
| 0xE000-0xE01B | LCD | init, clear, pixel, char, string, cursor, rect, line, scroll, refresh |
| 0xE020-0xE029 | Keyboard | get, hit, clear, wait |
| 0xE030-0xE039 | Audio | beep, stop, music_play, music_stop |
| 0xE040-0xE04C | Timer | set, get, rtc_read, rtc_write, delay |
| 0xE050-0xE05F | String | strlen, strcpy, strcmp, strcat, memcpy, memset |
| 0xE060-0xE06C | File | open, read, write, close, delete |
| 0xE070-0xE079 | System | init, power_off, info, random |
| 0x0260 | BRK | Exit handler |

See [SYSCALLS.md](SYSCALLS.md) for detailed documentation.

## Interrupt Vectors

| Vector | Address | Description |
|--------|---------|-------------|
| RESET | 0xFFFC-0xFFFD | Reset vector |
| NMI | 0xFFFA-0xFFFB | Non-maskable interrupt |
| IRQ | 0xFFFE-0xFFFF | Interrupt request |
| BRK | 0xFFFE-0xFFFF | Break instruction |

## Timer System

The BBK hardware has multiple timers:

| Timer | Interrupt | Description |
|-------|-----------|-------------|
| Timer 1 | ST1 | General purpose timer |
| Timer 2 | ST2 | General purpose timer |
| Timer 3 | ST3 | General purpose timer |
| Timer 4 | ST4 | General purpose timer |
| Main Timer | MTCT | Main system timer (increments every 400 CPU cycles) |

## Audio

The audio system generates square wave tones:

- Frequency: Configurable (Hz)
- Duration: Configurable (ms)
- Output: Mono speaker

## Key Matrix

The BBK keyboard is organized as a matrix. Each key has a unique keycode:

| Code | Key | Code | Key | Code | Key | Code | Key |
|------|-----|------|-----|------|-----|------|-----|
| 0x00 | ON/OFF | 0x10 | Q | 0x20 | INPUT | 0x30 | 9 |
| 0x01 | HOME/MENU | 0x11 | W | 0x21 | Z | 0x31 | 0 |
| 0x02 | EC_SJ | 0x12 | E | 0x22 | X | 0x32 | O |
| 0x03 | EC_SW | 0x13 | R | 0x23 | C | 0x33 | P |
| 0x04 | CE | 0x14 | T | 0x24 | V | 0x34 | L |
| 0x05 | DLG | 0x15 | Y | 0x25 | B | 0x35 | UP |
| 0x06 | DOWNLOAD | 0x16 | U | 0x26 | N | 0x36 | SPACE |
| 0x07 | SPK | 0x17 | I | 0x27 | M | 0x37 | LEFT |
| 0x08 | 1 | 0x18 | A | 0x28 | ZY/SHIFT | 0x38 | DOWN |
| 0x09 | 2 | 0x19 | S | 0x29 | HELP | 0x39 | RIGHT |
| 0x0A | 3 | 0x1A | D | 0x2A | SEARCH | 0x3A | PGUP |
| 0x0B | 4 | 0x1B | F | 0x2B | INSERT | 0x3B | PGDN |
| 0x0C | 5 | 0x1C | G | 0x2C | MODIFY | | |
| 0x0D | 6 | 0x1D | H | 0x2D | DEL | | |
| 0x0E | 7 | 0x1E | J | 0x2E | EXIT | | |
| 0x0F | 8 | 0x1F | K | 0x2F | ENTER | | |

See [input.rs](../crates/bbkemu-core/src/input.rs) for the keycode enum definition.

## Detailed Register Map

### Page 0 (0x00-0xFF)

| Address | Name | Description |
|---------|------|-------------|
| 0x00-0x03 | DATA1-DATA4 | Direct memory access registers |
| 0x04 | ISR | Interrupt Status Register |
| 0x05 | TISR | Timer Interrupt Status Register |
| 0x0C | BK_SEL | Bank select register |
| 0x0D | BK_ADRL | Bank address low byte |
| 0x0E | BK_ADRH | Bank address high byte |

### Page 2 (0x200-0x2FF)

| Address | Name | Description |
|---------|------|-------------|
| 0x200 | SYSCON | System control |
| 0x207 | INCR | Auto-increment register |
| 0x208-0x213 | ADDR1-ADDR4 | Address registers (3 bytes each) |
| 0x21B | PB | Audio control |
| 0x226 | STCON | Timer control |
| 0x227-0x22A | ST1LD-ST4LD | Timer load values |
| 0x22B | MTCT | Main timer counter |
| 0x22E | STCTCON | Counter control |
| 0x22F | CTLD | Counter load |
| 0x230-0x238 | RTC | Real-time clock registers |
| 0x23A | IER | Interrupt Enable Register |
| 0x23B | TIER | Timer Interrupt Enable Register |
| 0x23F | AUDCON | Audio control |
| 0x24E | KEYCODE | Keyboard input |

### Page 2 (0x2000-0x2FFF)

| Address | Name | Description |
|---------|------|-------------|
| 0x2003 | KeyBuffTop | Key buffer top pointer |
| 0x2004 | KeyBuffBottom | Key buffer bottom pointer |
| 0x2008+ | KeyBuffer | Key buffer data |

## Interrupt Vectors

The interrupt vector table is located at 0x0300-0x03FF. Each vector is 4 bytes:

| Vector | Index | Description |
|--------|-------|-------------|
| PI | 0x02 | Keyboard interrupt |
| ST1 | 0x03 | Timer 1 interrupt |
| ST2 | 0x04 | Timer 2 interrupt |
| ST3 | 0x05 | Timer 3 interrupt |
| ST4 | 0x06 | Timer 4 interrupt |
| GTL | 0x0F | Timer low interrupt |
| GTH | 0x10 | Timer high interrupt |
| MT | 0x11 | Main timer interrupt |
| CT | 0x12 | Counter interrupt |
| ALM | 0x13 | Alarm interrupt |

### Interrupt Handling

When an interrupt occurs:
1. CPU pushes PC and status register to stack
2. CPU reads vector from 0x0300 + (index * 4)
3. CPU jumps to the address in the vector

## Timer Details

### Timer Control (STCON at 0x226)

| Bit | Description |
|-----|-------------|
| 0 | Timer 1 enable |
| 1 | Timer 2 enable |
| 2 | Timer 3 enable |
| 3 | Timer 4 enable |

### Timer Load Values

| Address | Timer |
|---------|-------|
| 0x227 | ST1LD (Timer 1 load) |
| 0x228 | ST2LD (Timer 2 load) |
| 0x229 | ST3LD (Timer 3 load) |
| 0x22A | ST4LD (Timer 4 load) |

### Main Timer (MTCT)

- Address: 0x22B
- Increments every 400 CPU cycles
- Triggers MT interrupt when overflow
- Used for OS initialization (wait for MTCT == 0xFE)
