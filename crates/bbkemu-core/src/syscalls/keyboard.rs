//! Keyboard system call implementations

use crate::syscall::{SyscallCategory, SyscallContext, SyscallEntry, SyscallResult, SyscallTable};

/// Register keyboard syscalls
pub fn register(table: &mut SyscallTable) {
    table.register(SyscallEntry {
        address: 0xE010, // TBD
        name: "key_get",
        category: SyscallCategory::Keyboard,
        handler: key_get,
    });

    table.register(SyscallEntry {
        address: 0xE013, // TBD
        name: "key_hit",
        category: SyscallCategory::Keyboard,
        handler: key_hit,
    });

    table.register(SyscallEntry {
        address: 0xE016, // TBD
        name: "key_clear",
        category: SyscallCategory::Keyboard,
        handler: key_clear,
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
