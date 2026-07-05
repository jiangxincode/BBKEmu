//! LCD system call implementations

use crate::syscall::{SyscallCategory, SyscallContext, SyscallEntry, SyscallResult, SyscallTable};

/// Register LCD syscalls
/// Addresses are in the OS ROM space (0xE000-0xEFFF)
pub fn register(table: &mut SyscallTable) {
    // LCD initialization
    table.register(SyscallEntry {
        address: 0xE000,
        name: "lcd_init",
        category: SyscallCategory::Lcd,
        handler: lcd_init,
    });

    // Clear screen
    table.register(SyscallEntry {
        address: 0xE003,
        name: "lcd_clear",
        category: SyscallCategory::Lcd,
        handler: lcd_clear,
    });

    // Draw pixel
    table.register(SyscallEntry {
        address: 0xE006,
        name: "lcd_pixel",
        category: SyscallCategory::Lcd,
        handler: lcd_pixel,
    });

    // Draw character
    table.register(SyscallEntry {
        address: 0xE009,
        name: "lcd_char",
        category: SyscallCategory::Lcd,
        handler: lcd_char,
    });

    // Draw string
    table.register(SyscallEntry {
        address: 0xE00C,
        name: "lcd_string",
        category: SyscallCategory::Lcd,
        handler: lcd_string,
    });

    // Set cursor position
    table.register(SyscallEntry {
        address: 0xE00F,
        name: "lcd_cursor",
        category: SyscallCategory::Lcd,
        handler: lcd_cursor,
    });

    // Draw rectangle
    table.register(SyscallEntry {
        address: 0xE012,
        name: "lcd_rect",
        category: SyscallCategory::Lcd,
        handler: lcd_rect,
    });

    // Draw line
    table.register(SyscallEntry {
        address: 0xE015,
        name: "lcd_line",
        category: SyscallCategory::Lcd,
        handler: lcd_line,
    });

    // Scroll screen
    table.register(SyscallEntry {
        address: 0xE018,
        name: "lcd_scroll",
        category: SyscallCategory::Lcd,
        handler: lcd_scroll,
    });

    // Refresh display
    table.register(SyscallEntry {
        address: 0xE01B,
        name: "lcd_refresh",
        category: SyscallCategory::Lcd,
        handler: lcd_refresh,
    });
}

fn lcd_init(ctx: &mut SyscallContext) -> SyscallResult {
    ctx.lcd.clear();
    SyscallResult::handled()
}

fn lcd_clear(ctx: &mut SyscallContext) -> SyscallResult {
    ctx.lcd.clear();
    SyscallResult::handled()
}

fn lcd_pixel(ctx: &mut SyscallContext) -> SyscallResult {
    let x = ctx.cpu.x();
    let y = ctx.cpu.y();
    let color = ctx.cpu.a() != 0;
    ctx.lcd.set_pixel(x, y, color);
    SyscallResult::handled()
}

fn lcd_char(ctx: &mut SyscallContext) -> SyscallResult {
    let ch = ctx.cpu.a();
    let x = ctx.lcd.cursor_x();
    let y = ctx.lcd.cursor_y();

    // Get font bitmap for the character
    let font_bitmap = crate::font_data::get_font_bitmap(ch);

    // Write font data to LCD framebuffer at cursor position
    for row in 0..8u8 {
        let byte = font_bitmap[row as usize];
        for bit in 0..8u8 {
            if byte & (1 << (7 - bit)) != 0 {
                ctx.lcd.set_pixel(x + bit, y + row, true);
            }
        }
    }

    // Advance cursor by character width (8 pixels)
    ctx.lcd.set_cursor(x + 8, y);

    SyscallResult::handled()
}

fn lcd_string(ctx: &mut SyscallContext) -> SyscallResult {
    let addr_lo = ctx.cpu.x() as u16;
    let addr_hi = ctx.cpu.y() as u16;
    let addr = addr_lo | (addr_hi << 8);

    let mut x = ctx.lcd.cursor_x();
    let y = ctx.lcd.cursor_y();

    // Read string from memory and draw each character
    let mut offset = 0u16;
    loop {
        let ch = ctx.memory.read(addr + offset);
        if ch == 0 {
            break;
        }

        // Get font bitmap for the character
        let font_bitmap = crate::font_data::get_font_bitmap(ch);

        // Write font data to LCD framebuffer
        for row in 0..8u8 {
            let byte = font_bitmap[row as usize];
            for bit in 0..8u8 {
                if byte & (1 << (7 - bit)) != 0 {
                    ctx.lcd.set_pixel(x + bit, y + row, true);
                }
            }
        }

        // Advance x position by character width (8 pixels)
        x = x.wrapping_add(8);

        offset += 1;
        if offset > 255 {
            break; // Safety limit
        }
    }

    // Update cursor position after the string
    ctx.lcd.set_cursor(x, y);

    SyscallResult::handled()
}

fn lcd_cursor(ctx: &mut SyscallContext) -> SyscallResult {
    let x = ctx.cpu.x();
    let y = ctx.cpu.y();
    ctx.lcd.set_cursor(x, y);
    SyscallResult::handled()
}

fn lcd_rect(ctx: &mut SyscallContext) -> SyscallResult {
    // Parameters: X=x, Y=y, A=width, height read from zero-page 0x20
    let x = ctx.cpu.x();
    let y = ctx.cpu.y();
    let w = ctx.cpu.a();
    let h = ctx.memory.read(0x20);

    ctx.lcd.fill_rect(x, y, w, h, true);
    SyscallResult::handled()
}

fn lcd_line(ctx: &mut SyscallContext) -> SyscallResult {
    // Parameters: X=x1, Y=y1, A=x2, y2 read from zero-page 0x20
    let x1 = ctx.cpu.x();
    let y1 = ctx.cpu.y();
    let x2 = ctx.cpu.a();
    let y2 = ctx.memory.read(0x20);

    // Draw line using Bresenham's algorithm
    let dx = (x2 as i16 - x1 as i16).abs();
    let dy = (y2 as i16 - y1 as i16).abs();
    let sx = if x1 < x2 { 1i16 } else { -1i16 };
    let sy = if y1 < y2 { 1i16 } else { -1i16 };
    let mut err = dx - dy;
    let mut x = x1 as i16;
    let mut y = y1 as i16;

    loop {
        if x >= 0 && x < 159 && y >= 0 && y < 96 {
            ctx.lcd.set_pixel(x as u8, y as u8, true);
        }
        if x == x2 as i16 && y == y2 as i16 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }

    SyscallResult::handled()
}

fn lcd_scroll(ctx: &mut SyscallContext) -> SyscallResult {
    let lines = ctx.cpu.a();
    ctx.lcd.scroll_up(lines);
    SyscallResult::handled()
}

fn lcd_refresh(ctx: &mut SyscallContext) -> SyscallResult {
    // LCD refresh is handled by the main loop
    // This syscall just signals that the display should be updated
    SyscallResult::handled()
}
