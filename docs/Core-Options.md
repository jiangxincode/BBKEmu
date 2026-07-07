# Libretro Core Options

When running BBKEmu as a libretro core, these options are configurable
from RetroArch's *Quick Menu → Core Options*. Changes apply live without
reloading the game.

| Option | Values | Default | Effect |
|--------|--------|---------|--------|
| Swap LCD Width/Height | portrait, landscape | portrait | Swap display dimensions for landscape orientation |
| CPU Clock Rate | 0.25, 0.50, 0.75, 1.00, 1.50, 2.00, 3.00, 4.00, 8.00 | 1.00 | CPU speed multiplier |
| Timer Clock Rate | 0.25, 0.50, 0.75, 1.00, 1.50, 2.00, 3.00, 4.00, 8.00 | 1.00 | Timer speed multiplier |
| Key Repeat Interval | 0, 50, 100, 150, 200, 250, 300, 400, 500 | 0 | Minimum interval between repeated key presses (ms); 0 = no limit |

## Option Details

### Swap LCD Width/Height

Some BBK games are designed for landscape display. This option swaps the LCD
dimensions (159×96 becomes 96×159) for better gameplay on widescreen displays.

### CPU Clock Rate

Adjusts the CPU emulation speed. Useful for games that run too fast or too slow
on default settings. Lower values slow down the CPU, higher values speed it up.

### Timer Clock Rate

Adjusts the timer interrupt frequency independently from the CPU. Some games
use timers for animation timing, music tempo, or input repeat behavior.

### Key Repeat Interval

Sets the minimum time (in milliseconds) between repeated key presses when a
button is held down. This prevents overly rapid input in games that are
sensitive to key repeat speed.

- **0** — No limit (fastest repeat, default behavior)
- **50–500** — Increasingly slower repeat rates
