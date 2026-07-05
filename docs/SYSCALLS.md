# BBK System Call Reference

## Status

**Actual syscall addresses and calling conventions need verification against real OS ROM.**

## Calling Convention

The BBK 6502 OS uses the following conventions:

- **Arguments**: Passed in Accumulator (A), X register, Y register, or via
  zero-page memory locations (0x20-0x2F)
- **Return value**: In Accumulator (A) or zero-page memory
- **Stack**: Standard 6502 stack at 0x0100-0x01FF
- **JSR/RTS**: Standard 6502 subroutine call/return

## Syscall Table (4980)

### LCD Operations

| Address | Name | A | X | Y | Description |
|---------|------|---|---|---|-------------|
| TBD | lcd_init | - | - | - | Initialize LCD |
| TBD | lcd_clear | - | - | - | Clear screen |
| TBD | lcd_pixel | color | x | y | Draw pixel |
| TBD | lcd_hline | color | x | width | Draw horizontal line at cursor Y |
| TBD | lcd_vline | color | y | height | Draw vertical line at cursor X |
| TBD | lcd_rect | color | w | h | Fill rectangle at cursor |
| TBD | lcd_char | char | - | - | Draw char at cursor, advance |
| TBD | lcd_string | addr_lo | addr_hi | - | Draw string at cursor |
| TBD | lcd_cursor | x | y | - | Set cursor position |
| TBD | lcd_scroll | lines | - | - | Scroll screen up |
| TBD | lcd_refresh | - | - | - | Flush buffer to display |

### Keyboard Operations

| Address | Name | A | X | Y | Description |
|---------|------|---|---|---|-------------|
| TBD | key_get | - | - | - | Blocking key read → A |
| TBD | key_hit | - | - | - | Non-blocking → A (0=no key) |
| TBD | key_clear | - | - | - | Clear key buffer |

### Timer Operations

| Address | Name | A | X | Y | Description |
|---------|------|---|---|---|-------------|
| TBD | timer_set | channel | value | - | Set timer |
| TBD | timer_get | channel | - | - | Read timer → A |
| TBD | rtc_read | field | - | - | Read RTC field → A |
| TBD | rtc_write | field | value | - | Write RTC field |

### File/Flash Operations

| Address | Name | A | X | Y | Description |
|---------|------|---|---|---|-------------|
| TBD | file_open | mode | name_lo | name_hi | Open file → handle in A |
| TBD | file_read | handle | buf_lo | buf_hi | Read from file |
| TBD | file_write | handle | buf_lo | buf_hi | Write to file |
| TBD | file_close | handle | - | - | Close file |
| TBD | file_delete | name_lo | name_hi | - | Delete file |

### Memory Operations

| Address | Name | A | X | Y | Description |
|---------|------|---|---|---|-------------|
| TBD | memcpy | dst_lo | src_lo | count | Copy memory |
| TBD | memset | dst_lo | value | count | Fill memory |
| TBD | memset16 | dst_lo | value | count | Fill 16-bit words |

### String Operations

| Address | Name | A | X | Y | Description |
|---------|------|---|---|---|-------------|
| TBD | strlen | addr_lo | addr_hi | - | String length → A |
| TBD | strcpy | dst_lo | src_lo | - | Copy string |
| TBD | strcmp | str1_lo | str2_lo | - | Compare → A (0=equal) |
| TBD | gbk_decode | code_hi | code_lo | - | Decode GBK → glyph index |

### Audio Operations

| Address | Name | A | X | Y | Description |
|---------|------|---|---|---|-------------|
| TBD | beep | freq_lo | freq_hi | duration | Play tone |
| TBD | sound_stop | - | - | - | Stop all sound |

### System Operations

| Address | Name | A | X | Y | Description |
|---------|------|---|---|---|-------------|
| 0x0260 | brk_handler | - | - | - | Game exit (BRK) |
| TBD | halt | - | - | - | Halt CPU (power off) |
| TBD | idle | - | - | - | Enter low-power idle |

## Interrupt Vectors

`0x0300 + idx * 4`:

| Vector | Index | Name | Description |
|--------|-------|------|-------------|
| 0x030C | 0x03 | ST1 | Timer 1 |
| 0x0310 | 0x04 | ST2 | Timer 2 |
| 0x0314 | 0x05 | ST3 | Timer 3 |
| 0x0318 | 0x06 | ST4 | Timer 4 |
| 0x033C | 0x0F | GTL | Timer low overflow |
| 0x0340 | 0x10 | GTH | Timer high overflow |
| 0x0344 | 0x11 | MT | Main timer |
| 0x0348 | 0x12 | CT | Counter timer |
| 0x034C | 0x13 | ALM | Alarm |
| 0x0308 | 0x02 | PI | Keyboard interrupt |

## Model Differences

### 4980 vs 4988

the key difference is `bank_sys_d`:
- 4980: `0x0EA8`
- 4988: `0x0E88`

Key mapping differences:
- 4988 has SHIFT key (0x2D) instead of DEL
- 4988 has different function key layout

Syscall addresses may differ between models.
