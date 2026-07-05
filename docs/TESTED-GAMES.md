# Tested Games

This document tracks game compatibility testing with BBKEmu.

## Test Results

| Game | Status | Model | Notes |
|------|--------|-------|-------|
| 伏魔记 (FMJ) | ⚠️ Partial | A4980 | Basic loading works |
| 三国霸业 (Baye) | ⚠️ Partial | A4980 | Basic loading works |
| 金庸群侠传 | ⚠️ Partial | A4980 | Basic loading works |
| 魔塔 | ⚠️ Partial | A4980 | Basic loading works |
| 侠客行 | ⚠️ Partial | A4980 | Basic loading works |

## Status Legend

| Status | Description |
|--------|-------------|
| ✅ Playable | Game runs correctly, all features work |
| ⚠️ Partial | Game loads and runs, some features may not work |
| ❌ Broken | Game fails to load or crashes |
| � Untested | Not yet tested |

## Testing Notes

- All games require ROM files (8.BIN and E.BIN) from a physical BBK dictionary
- Games are tested with the A4980 model unless otherwise noted
- Audio and save functionality are still being implemented

## Game Resources

Game resources can be downloaded from [Baidu Netdisk](https://pan.baidu.com/s/1CuJe-RKXG_E-LhdI5ldg?pwd=aloy).

## Adding New Games

To test a new game:

1. Place the .gam file in the working directory
2. Ensure ROM files are available
3. Run: `bbkemu game.gam -8 8.BIN -e E.BIN --debug`
4. Check the log output for errors
5. Update this document with the results
