//! BBK OS system call implementations
//! These are the low-level OS functions accessed via JSR to 0xD000-0xEFFF addresses

use crate::syscall::{SyscallCategory, SyscallContext, SyscallEntry, SyscallResult, SyscallTable};

/// Register BBK OS syscalls
pub fn register(table: &mut SyscallTable) {
    // LCD drawing functions
    table.register(SyscallEntry {
        address: 0xD300,
        name: "lcd_clear_area",
        category: SyscallCategory::Lcd,
        handler: lcd_clear_area,
    });

    table.register(SyscallEntry {
        address: 0xD320,
        name: "lcd_hline",
        category: SyscallCategory::Lcd,
        handler: lcd_hline,
    });

    table.register(SyscallEntry {
        address: 0xD340,
        name: "lcd_draw_block",
        category: SyscallCategory::Lcd,
        handler: lcd_draw_block,
    });

    table.register(SyscallEntry {
        address: 0xD360,
        name: "lcd_vline",
        category: SyscallCategory::Lcd,
        handler: lcd_vline,
    });

    table.register(SyscallEntry {
        address: 0xD380,
        name: "lcd_fill_rect",
        category: SyscallCategory::Lcd,
        handler: lcd_fill_rect,
    });

    // Keyboard functions
    table.register(SyscallEntry {
        address: 0xD3A0,
        name: "os_key_get",
        category: SyscallCategory::Keyboard,
        handler: os_key_get,
    });

    table.register(SyscallEntry {
        address: 0xD3C0,
        name: "os_key_hit",
        category: SyscallCategory::Keyboard,
        handler: os_key_hit,
    });

    // Audio functions
    table.register(SyscallEntry {
        address: 0xD400,
        name: "os_beep",
        category: SyscallCategory::Audio,
        handler: os_beep,
    });

    // Timer functions
    table.register(SyscallEntry {
        address: 0xD420,
        name: "os_delay",
        category: SyscallCategory::Timer,
        handler: os_delay,
    });

    // Drawing helper functions
    table.register(SyscallEntry {
        address: 0xD596,
        name: "draw_offset_8",
        category: SyscallCategory::Lcd,
        handler: draw_offset_8,
    });

    table.register(SyscallEntry {
        address: 0xD5A6,
        name: "draw_offset_20",
        category: SyscallCategory::Lcd,
        handler: draw_offset_20,
    });

    table.register(SyscallEntry {
        address: 0xD5B6,
        name: "draw_offset_18",
        category: SyscallCategory::Lcd,
        handler: draw_offset_18,
    });

    // Bitmap operations
    table.register(SyscallEntry {
        address: 0xD2CA,
        name: "bitmap_and",
        category: SyscallCategory::Lcd,
        handler: bitmap_and,
    });

    // Compare function
    table.register(SyscallEntry {
        address: 0xD362,
        name: "compare16",
        category: SyscallCategory::System,
        handler: compare16,
    });

    // Multiply function
    table.register(SyscallEntry {
        address: 0xD1A2,
        name: "multiply16",
        category: SyscallCategory::System,
        handler: multiply16,
    });

    // Stack operations
    table.register(SyscallEntry {
        address: 0xDAAA,
        name: "push_a",
        category: SyscallCategory::System,
        handler: push_a,
    });

    table.register(SyscallEntry {
        address: 0xDACA,
        name: "set_cursor_stack",
        category: SyscallCategory::Lcd,
        handler: set_cursor_stack,
    });

    table.register(SyscallEntry {
        address: 0xDAE6,
        name: "copy_16",
        category: SyscallCategory::System,
        handler: copy_16,
    });

    table.register(SyscallEntry {
        address: 0xDBE1,
        name: "test_not_zero",
        category: SyscallCategory::System,
        handler: test_not_zero,
    });

    table.register(SyscallEntry {
        address: 0xDCEF,
        name: "push_4_bytes",
        category: SyscallCategory::System,
        handler: push_4_bytes,
    });

    // Complex drawing function
    table.register(SyscallEntry {
        address: 0xD6AF,
        name: "draw_complex",
        category: SyscallCategory::Lcd,
        handler: draw_complex,
    });

    // Function pointer call
    table.register(SyscallEntry {
        address: 0xD572,
        name: "call_ptr",
        category: SyscallCategory::System,
        handler: call_ptr,
    });
}

fn lcd_clear_area(ctx: &mut SyscallContext) -> SyscallResult {
    // Clear LCD framebuffer
    for i in 0x0400..0x1000 {
        ctx.memory.write(i, 0);
    }
    SyscallResult::handled()
}

fn lcd_hline(ctx: &mut SyscallContext) -> SyscallResult {
    let y = ctx.cpu.a();
    let x1 = ctx.cpu.x();
    let x2 = ctx.cpu.y();

    // Draw horizontal line at y from x1 to x2
    for x in x1..=x2 {
        ctx.lcd.set_pixel(x, y, true);
    }
    SyscallResult::handled()
}

fn lcd_draw_block(ctx: &mut SyscallContext) -> SyscallResult {
    let src = ctx.cpu.memory().read16(0x20);
    let dst = ctx.cpu.memory().read16(0x26);

    // Copy data from src to dst
    for i in 0..32 {
        let byte = ctx.cpu.memory().read(src + i);
        if byte == 0 {
            break;
        }
        if dst + i >= 0x0400 && dst + i < 0x1000 {
            ctx.cpu.memory_mut().write(dst + i, byte);
        }
    }
    SyscallResult::handled()
}

fn lcd_vline(ctx: &mut SyscallContext) -> SyscallResult {
    let x = ctx.cpu.a();
    let y1 = ctx.cpu.x();
    let y2 = ctx.cpu.y();

    // Draw vertical line at x from y1 to y2
    for y in y1..=y2 {
        ctx.lcd.set_pixel(x, y, true);
    }
    SyscallResult::handled()
}

fn lcd_fill_rect(ctx: &mut SyscallContext) -> SyscallResult {
    let x = ctx.cpu.memory().read(0x20);
    let y = ctx.cpu.memory().read(0x21);
    let w = ctx.cpu.memory().read(0x22);
    let h = ctx.cpu.memory().read(0x23);

    ctx.lcd.fill_rect(x, y, w, h, true);
    SyscallResult::handled()
}

fn os_key_get(ctx: &mut SyscallContext) -> SyscallResult {
    let key = ctx.input.get_key();
    SyscallResult::with_return(key)
}

fn os_key_hit(ctx: &mut SyscallContext) -> SyscallResult {
    let has_key = ctx.input.key_hit();
    SyscallResult::with_return(if has_key { 1 } else { 0 })
}

fn os_beep(ctx: &mut SyscallContext) -> SyscallResult {
    let freq = ctx.cpu.x() as u16 | (ctx.cpu.y() as u16) << 8;
    let duration = ctx.cpu.a() as u16;
    if freq > 0 {
        ctx.audio.play_tone(freq, duration * 10);
    }
    SyscallResult::handled()
}

fn os_delay(ctx: &mut SyscallContext) -> SyscallResult {
    let ms = ctx.cpu.a() as u32;
    SyscallResult {
        handled: true,
        return_value: None,
        cycles: ms * 4000,
    }
}

fn draw_offset_8(ctx: &mut SyscallContext) -> SyscallResult {
    let addr = ctx.cpu.memory().read16(0x2A);
    let result = addr.wrapping_add(8);
    ctx.cpu.memory_mut().write(0x20, result as u8);
    ctx.cpu.memory_mut().write(0x21, (result >> 8) as u8);
    SyscallResult::handled()
}

fn draw_offset_20(ctx: &mut SyscallContext) -> SyscallResult {
    let addr = ctx.cpu.memory().read16(0x2A);
    let result = addr.wrapping_add(0x20);
    ctx.cpu.memory_mut().write(0x23, result as u8);
    ctx.cpu.memory_mut().write(0x24, (result >> 8) as u8);
    SyscallResult::handled()
}

fn draw_offset_18(ctx: &mut SyscallContext) -> SyscallResult {
    let addr = ctx.cpu.memory().read16(0x2A);
    let result = addr.wrapping_add(0x18);
    ctx.cpu.memory_mut().write(0x23, result as u8);
    ctx.cpu.memory_mut().write(0x24, (result >> 8) as u8);
    SyscallResult::handled()
}

fn bitmap_and(ctx: &mut SyscallContext) -> SyscallResult {
    let src1 = ctx.cpu.memory().read16(0x20);
    let src2 = ctx.cpu.memory().read16(0x23);
    let dst = ctx.cpu.memory().read16(0x2A);
    for i in 0..4 {
        let b1 = ctx.cpu.memory().read(src1 + i);
        let b2 = ctx.cpu.memory().read(src2 + i);
        ctx.cpu.memory_mut().write(dst + 8 + i, b1 & b2);
    }
    // Update [0x20:0x21]
    let addr = ctx.cpu.memory().read16(0x2A);
    let result = addr.wrapping_add(8);
    ctx.cpu.memory_mut().write(0x20, result as u8);
    ctx.cpu.memory_mut().write(0x21, (result >> 8) as u8);
    SyscallResult::handled()
}

fn compare16(ctx: &mut SyscallContext) -> SyscallResult {
    let val1 = ctx.cpu.memory().read16(0x20);
    let val2 = ctx.cpu.memory().read16(0x23);
    SyscallResult::with_return(if val1 == val2 { 0 } else { 1 })
}

fn multiply16(ctx: &mut SyscallContext) -> SyscallResult {
    let a = ctx.cpu.memory().read16(0x20);
    let b = ctx.cpu.memory().read16(0x23);
    let result = (a as u32).wrapping_mul(b as u32);
    ctx.cpu.memory_mut().write(0x26, result as u8);
    ctx.cpu.memory_mut().write(0x27, (result >> 8) as u8);
    SyscallResult::handled()
}

fn push_a(ctx: &mut SyscallContext) -> SyscallResult {
    let stack = ctx.cpu.memory().read16(0x28).wrapping_sub(1);
    let value = ctx.cpu.a();
    ctx.cpu.memory_mut().write(0x28, stack as u8);
    ctx.cpu.memory_mut().write(0x29, (stack >> 8) as u8);
    ctx.cpu.memory_mut().write(stack, value);
    SyscallResult::handled()
}

fn set_cursor_stack(ctx: &mut SyscallContext) -> SyscallResult {
    let stack = ctx.cpu.memory().read16(0x28).wrapping_sub(2);
    let lo = ctx.cpu.memory().read(0x20);
    let hi = ctx.cpu.memory().read(0x21);
    ctx.cpu.memory_mut().write(0x28, stack as u8);
    ctx.cpu.memory_mut().write(0x29, (stack >> 8) as u8);
    ctx.cpu.memory_mut().write(stack, lo);
    ctx.cpu.memory_mut().write(stack.wrapping_add(1), hi);
    SyscallResult::handled()
}

fn copy_16(ctx: &mut SyscallContext) -> SyscallResult {
    let src = ctx.cpu.memory().read16(0x23);
    let dst = ctx.cpu.memory().read16(0x20);
    for i in 0..2 {
        let byte = ctx.cpu.memory().read(src + i);
        ctx.cpu.memory_mut().write(dst + i, byte);
    }
    SyscallResult::handled()
}

fn test_not_zero(ctx: &mut SyscallContext) -> SyscallResult {
    let val = ctx.cpu.memory().read16(0x23);
    SyscallResult::with_return(if val == 0 { 0 } else { 1 })
}

fn push_4_bytes(ctx: &mut SyscallContext) -> SyscallResult {
    let addr = ctx.cpu.memory().read16(0x20);
    let sp = ctx.cpu.sp();
    for i in 0..4 {
        let byte = ctx.cpu.memory().read(addr + i);
        ctx.cpu.memory_mut().write(0x100 + sp.wrapping_sub(i as u8) as u16, byte);
    }
    ctx.cpu.set_sp(sp.wrapping_sub(4));
    SyscallResult::handled()
}

fn draw_complex(ctx: &mut SyscallContext) -> SyscallResult {
    // Complex drawing function - just return success for HLE
    SyscallResult::handled()
}

fn call_ptr(ctx: &mut SyscallContext) -> SyscallResult {
    // Function pointer call - not fully implemented for HLE
    SyscallResult::handled()
}
