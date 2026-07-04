//! Keyboard system call implementations

use crate::syscall::{SyscallCategory, SyscallContext, SyscallEntry, SyscallResult, SyscallTable};

/// Register keyboard syscalls
pub fn register(table: &mut SyscallTable) {
    // Get key (blocking)
    table.register(SyscallEntry {
        address: 0xE020,
        name: "key_get",
        category: SyscallCategory::Keyboard,
        handler: key_get,
    });

    // Check key hit (non-blocking)
    table.register(SyscallEntry {
        address: 0xE023,
        name: "key_hit",
        category: SyscallCategory::Keyboard,
        handler: key_hit,
    });

    // Clear key buffer
    table.register(SyscallEntry {
        address: 0xE026,
        name: "key_clear",
        category: SyscallCategory::Keyboard,
        handler: key_clear,
    });

    // Wait for key
    table.register(SyscallEntry {
        address: 0xE029,
        name: "key_wait",
        category: SyscallCategory::Keyboard,
        handler: key_wait,
    });
}

fn key_get(ctx: &mut SyscallContext) -> SyscallResult {
    let key = ctx.input.get_key();
    SyscallResult::with_return(key)
}

fn key_hit(ctx: &mut SyscallContext) -> SyscallResult {
    let has_key = ctx.input.key_hit();
    SyscallResult::with_return(if has_key { 1 } else { 0 })
}

fn key_clear(ctx: &mut SyscallContext) -> SyscallResult {
    ctx.input.clear_buffer();
    SyscallResult::handled()
}

fn key_wait(ctx: &mut SyscallContext) -> SyscallResult {
    // Wait for a key press
    loop {
        let key = ctx.input.get_key();
        if key != 0 {
            return SyscallResult::with_return(key);
        }
        // In a real implementation, this would yield to the main loop
        // For now, just return 0
        return SyscallResult::with_return(0);
    }
}
