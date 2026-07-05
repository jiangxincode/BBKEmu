# GAM File Format Specification

## Status

**Field interpretations may be incomplete or incorrect.**

## Overview

GAM files are game packages for BBK A-series electronic dictionaries.
They contain6502 machine code and associated data.

File extensions: `.gam`, `.smf`, `.sgm`, `.ssl` (model-dependent)

## File Structure

```
Offset      Size    Description
───────────────────────────────────────────────
0x00-0x05   6       Game metadata / identifier
0x06-0x0F   10      Game info block (copied to flash header)
0x10-0x3F   48      Reserved / padding
0x40-0x41   2       Entry point address (u16 little-endian)
0x42-0x45   4       Data section offset (u32 little-endian)
0x46+       ...     Game code and data
```

## Header Fields

### Entry Point (0x40-0x41)

The 6502 address where execution begins after the OS initialization completes.
Typically in the range 0x5000-0xBFFF (banked flash area).

### Data Offset (0x42-0x45)

Offset to the game's data section (graphics, maps, scripts, etc.).
Used to calculate bank mapping:
```
data_bank = 0x20D + (data_offset >> 12)
```

### Game Info Block (0x06-0x0F)

10 bytes of game metadata. Copied directly into the flash header at
`flash[0x8012..0x801C]`. Exact format TBD (possibly game name, version,
save slot info).

## Memory Layout After Loading

When a GAM file is loaded, it's placed into the simulated flash memory:

```
Flash Address    Content
───────────────────────────────────────
0x208000-0x20800F  System header (16 bytes)
0x208010-0x20801F  Game header (16 bytes)
0x20D000+          Game code (from GAM file)
```

### System Header (16 bytes)

```
Offset  Value   Description
0x00    0xC0    Header marker
0x01    0x00    Reserved
0x02-0x0F        Zeros
```

### Game Header (16 bytes)

```
Offset  Value       Description
0x00    0xD0        Header marker
0x01    0x00        Reserved
0x02-0x0B           Game info (from GAM 0x06-0x0F)
0x0C    size_lo      Game size low byte
0x0D    size_mid     Game size mid byte
0x0E    size_hi      Game size high byte
0x0F    0x3D        Type marker
```

## Bank Mapping

After loading, banks 0x05-0x0C are mapped to the game:

```
Bank  Address     Purpose
0x05  0x20D000    Game code page 1
0x06  0x20E000    Game code page 2
0x07  0x20F000    Game code page 3
0x08  0x210000    Game code page 4
0x09  0x20D000+data   Game data page 1
0x0A  +0x1000     Game data page 2
0x0B  +0x2000     Game data page 3
0x0C  +0x3000     Game data page 4
```

## Save Data

Save data is stored in the last 32 KiB of flash (0x1F8000-0x1FFFFF).

Flash header bytes at 0x70F8-0x70FF (4980) or 0x80F8-0x80FF (4988) control
save behavior:
```
0x02 = data present
0x03 = save file marker
```

## Execution Flow

```
1. Load OS ROM → initialize system
2. CPU starts at 0x350 (OS init)
3. OS init sets _MTCT to 0xFE when done
4. Load GAM file into flash
5. Setup bank mappings
6. Push return address 0x0260 (BRK handler) onto stack
7. Jump to GAM entry point
8. Game runs, calling OS syscalls via JSR
9. When game exits → RTS to 0x0260 → BRK → halt
```

## File Size Limits

Maximum GAM file size: 1,920 KiB (0x1E0000 bytes).

## Related Tools

- **BBK_GAM_Modifier**: Qt-based GAM file editor
- **BBK_Resource_Extract**: Resource unpacker
- **BBK_Picture_Encode**: Image codec for GAM graphics

## Open Questions

- [ ] Exact meaning of bytes 0x00-0x05 (game metadata)
- [ ] Game info block format (0x06-0x0F)
- [ ] Whether different models use different header formats
- [ ] How multi-file games work (if any)
- [ ] Compression used for graphics data
