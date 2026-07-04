//! LCD system call implementations

use crate::syscall::{SyscallCategory, SyscallContext, SyscallEntry, SyscallResult, SyscallTable};

/// Register LCD syscalls
pub fn register(table: &mut SyscallTable) {
    // TODO: Replace TBD addresses with actual addresses from OS ROM reverse engineering

    table.register(SyscallEntry {
        address: 0xE000, // TBD
        name: "lcd_clear",
        category: SyscallCategory::Lcd,
        handler: lcd_clear,
    });

    table.register(SyscallEntry {
        address: 0xE003, // TBD
        name: "lcd_pixel",
        category: SyscallCategory::Lcd,
        handler: lcd_pixel,
    });

    table.register(SyscallEntry {
        address: 0xE006, // TBD
        name: "lcd_char",
        category: SyscallCategory::Lcd,
        handler: lcd_char,
    });

    table.register(SyscallEntry {
        address: 0xE009, // TBD
        name: "lcd_string",
        category: SyscallCategory::Lcd,
        handler: lcd_string,
    });

    table.register(SyscallEntry {
        address: 0xE00C, // TBD
        name: "lcd_cursor",
        category: SyscallCategory::Lcd,
        handler: lcd_cursor,
    });

    table.register(SyscallEntry {
        address: 0xE00F, // TBD
        name: "lcd_rect",
        category: SyscallCategory::Lcd,
        handler: lcd_rect,
    });
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
    // TODO: Use actual font data
    log::trace!("lcd_char: 0x{:02X} ('{}')", ch, ch as char);
    SyscallResult::handled()
}

fn lcd_string(ctx: &mut SyscallContext) -> SyscallResult {
    let addr_lo = ctx.cpu.x() as u16;
    let addr_hi = ctx.cpu.y() as u16;
    let addr = addr_lo | (addr_hi << 8);
    // TODO: Read string from memory and draw
    log::trace!("lcd_string: addr=0x{:04X}", addr);
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
    // TODO: Get height from memory or register
    log::trace!("lcd_rect: x={}, y={}, w={}", x, y, w);
    SyscallResult::handled()
}
