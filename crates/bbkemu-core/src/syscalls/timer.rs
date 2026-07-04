//! Timer system call implementations

use crate::syscall::{SyscallCategory, SyscallContext, SyscallEntry, SyscallResult, SyscallTable};

/// Register timer syscalls
pub fn register(table: &mut SyscallTable) {
    // Set timer
    table.register(SyscallEntry {
        address: 0xE040,
        name: "timer_set",
        category: SyscallCategory::Timer,
        handler: timer_set,
    });

    // Get timer value
    table.register(SyscallEntry {
        address: 0xE043,
        name: "timer_get",
        category: SyscallCategory::Timer,
        handler: timer_get,
    });

    // Read RTC
    table.register(SyscallEntry {
        address: 0xE046,
        name: "rtc_read",
        category: SyscallCategory::Timer,
        handler: rtc_read,
    });

    // Write RTC
    table.register(SyscallEntry {
        address: 0xE049,
        name: "rtc_write",
        category: SyscallCategory::Timer,
        handler: rtc_write,
    });

    // Delay
    table.register(SyscallEntry {
        address: 0xE04C,
        name: "delay",
        category: SyscallCategory::Timer,
        handler: delay,
    });
}

fn timer_set(ctx: &mut SyscallContext) -> SyscallResult {
    let channel = ctx.cpu.a() as usize;
    let value = ctx.cpu.x();

    if channel < 4 {
        // Set timer load value
        ctx.memory.ram[0x227 + channel] = value;
        // Enable timer
        ctx.memory.ram[0x226] |= 1 << channel;
    }

    SyscallResult::handled()
}

fn timer_get(ctx: &mut SyscallContext) -> SyscallResult {
    let channel = ctx.cpu.a() as usize;

    if channel < 4 {
        let value = ctx.memory.ram[0x227 + channel];
        SyscallResult::with_return(value)
    } else {
        SyscallResult::with_return(0)
    }
}

fn rtc_read(ctx: &mut SyscallContext) -> SyscallResult {
    let field = ctx.cpu.a() as usize;

    let value = match field {
        0 => ctx.memory.ram[0x234], // Seconds
        1 => ctx.memory.ram[0x235], // Minutes
        2 => ctx.memory.ram[0x236], // Hours
        3 => ctx.memory.ram[0x237], // Days low
        4 => ctx.memory.ram[0x238], // Days high
        _ => 0,
    };

    SyscallResult::with_return(value)
}

fn rtc_write(ctx: &mut SyscallContext) -> SyscallResult {
    let field = ctx.cpu.a() as usize;
    let value = ctx.cpu.x();

    match field {
        0 => ctx.memory.ram[0x234] = value, // Seconds
        1 => ctx.memory.ram[0x235] = value, // Minutes
        2 => ctx.memory.ram[0x236] = value, // Hours
        3 => ctx.memory.ram[0x237] = value, // Days low
        4 => ctx.memory.ram[0x238] = value, // Days high
        _ => {}
    }

    SyscallResult::handled()
}

fn delay(ctx: &mut SyscallContext) -> SyscallResult {
    let ms = ctx.cpu.a() as u32;
    // In a real implementation, this would sleep
    // For now, just consume some cycles
    let cycles = ms * 4000; // ~4MHz CPU
    SyscallResult {
        handled: true,
        return_value: None,
        cycles,
    }
}
