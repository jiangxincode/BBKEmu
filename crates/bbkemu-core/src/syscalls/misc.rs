//! Miscellaneous system call implementations

use crate::syscall::{SyscallCategory, SyscallContext, SyscallEntry, SyscallResult, SyscallTable};

/// Register misc syscalls
pub fn register(table: &mut SyscallTable) {
    // System initialization
    table.register(SyscallEntry {
        address: 0xE070,
        name: "sys_init",
        category: SyscallCategory::System,
        handler: sys_init,
    });

    // Power off
    table.register(SyscallEntry {
        address: 0xE073,
        name: "power_off",
        category: SyscallCategory::System,
        handler: power_off,
    });

    // Get system info
    table.register(SyscallEntry {
        address: 0xE076,
        name: "sys_info",
        category: SyscallCategory::System,
        handler: sys_info,
    });

    // Random number
    table.register(SyscallEntry {
        address: 0xE079,
        name: "random",
        category: SyscallCategory::System,
        handler: random,
    });

    // BRK handler (game exit)
    table.register(SyscallEntry {
        address: 0x0260,
        name: "brk_exit",
        category: SyscallCategory::System,
        handler: brk_exit,
    });
}

fn sys_init(ctx: &mut SyscallContext) -> SyscallResult {
    // Initialize system
    ctx.lcd.clear();
    ctx.input.clear_buffer();
    ctx.audio.stop();

    // Set system control register
    ctx.memory.ram[0x200] = 0x00;

    // Set INCR register (auto-increment for all channels)
    ctx.memory.ram[0x207] = 0x0F;

    log::info!("System initialized");
    SyscallResult::handled()
}

fn power_off(_ctx: &mut SyscallContext) -> SyscallResult {
    log::info!("Power off requested");
    // In a real implementation, this would shut down the emulator
    SyscallResult::handled()
}

fn sys_info(ctx: &mut SyscallContext) -> SyscallResult {
    // Return system info in registers
    // A = model (0 = 4980, 1 = 4988)
    // X = firmware version low
    // Y = firmware version high
    SyscallResult::with_return(0) // 4980
}

fn random(ctx: &mut SyscallContext) -> SyscallResult {
    // Simple pseudo-random number generator
    let seed = ctx.memory.ram[0x2000] as u16;
    let seed = seed.wrapping_mul(25173).wrapping_add(13849);
    ctx.memory.ram[0x2000] = (seed >> 8) as u8;
    SyscallResult::with_return((seed & 0xFF) as u8)
}

fn brk_exit(_ctx: &mut SyscallContext) -> SyscallResult {
    log::info!("Game exited via BRK");
    // Signal emulator to stop
    // In a real implementation, this would set a flag
    SyscallResult::handled()
}
