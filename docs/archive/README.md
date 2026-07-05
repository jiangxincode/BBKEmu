# Archive

This directory contains archived documentation from earlier development phases.

These documents are preserved for historical reference and are no longer
actively maintained.

## HLE Mode Documentation

- [hle-mode-limitations.md](hle-mode-limitations.md) — Analysis of why pure HLE mode (without OS ROM) is not feasible for BBK emulation

## Context

BBKEmu originally explored both HLE (High Level Emulation) and LLE (Low Level
Emulation) approaches. After extensive testing and analysis, the project settled
on LLE mode exclusively, as the BBK OS ROM's complexity (particularly the D2F6
far-call dispatcher) makes accurate HLE impractical.

See the main [README](../../README.md) for current documentation.
