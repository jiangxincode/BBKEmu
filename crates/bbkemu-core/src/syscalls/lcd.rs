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
    // TODO: Use font data to draw character
    // For now, just log the character
    log::trace!("lcd_char: 0x{:02X} ('{}')", ch, ch as char);
    SyscallResult::handled()
}

fn lcd_string(ctx: &mut SyscallContext) -> SyscallResult {
    let addr_lo = ctx.cpu.x() as u16;
    let addr_hi = ctx.cpu.y() as u16;
    let addr = addr_lo | (addr_hi << 8);

    // Read string from memory and draw
    let mut offset = 0;
    loop {
        let ch = ctx.memory.read(addr + offset);
        if ch == 0 {
            break;
        }
        // TODO: Draw character
        log::trace!("lcd_string: char 0x{:02X} at offset {}", ch, offset);
        offset += 1;
        if offset > 255 {
            break; // Safety limit
        }
    }

    SyscallResult::handled()
}

fn lcd_cursor(ctx: &mut SyscallContext) -> SyscallResult {
    let x = ctx.cpu.x();
    let y = ctx.cpu.y();
    ctx.lcd.set_cursor(x, y);
    SyscallResult::handled()
}

fn lcd_rect(ctx: &mut SyscallContext) -> SyscallResult {
    let x = ctx.cpu.x();
    let y = ctx.cpu.y();
    let w = ctx.cpu.a();

    // Read height from memory or stack
    // TODO: Determine correct parameter passing
    let h = 10; // Placeholder

    ctx.lcd.fill_rect(x, y, w, h, true);
    SyscallResult::handled()
}

fn lcd_line(ctx: &mut SyscallContext) -> SyscallResult {
    let x1 = ctx.cpu.x();
    let y1 = ctx.cpu.y();
    let x2 = ctx.cpu.a();
    // TODO: Read y2 from memory
    let y2 = 0; // Placeholder

    // Draw line using Bresenham's algorithm
    let dx = (x2 as i16 - x1 as i16).abs();
    let dy = (y2 as i16 - y1 as i16).abs();
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut err = dx - dy;
    let mut x = x1 as i16;
    let mut y = y1 as i16;

    loop {
        ctx.lcd.set_pixel(x as u8, y as u8, true);
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
