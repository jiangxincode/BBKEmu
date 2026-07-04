//! Miscellaneous system call implementations

use crate::syscall::{SyscallCategory, SyscallContext, SyscallEntry, SyscallResult, SyscallTable};

pub fn register(table: &mut SyscallTable) {
    table.register(SyscallEntry {
        address: 0xE060, // TBD
        name: "memcpy",
        category: SyscallCategory::System,
        handler: memcpy,
    });

    table.register(SyscallEntry {
        address: 0xE063, // TBD
        name: "memset",
        category: SyscallCategory::System,
        handler: memset,
    });

    // BRK handler - game exit
    table.register(SyscallEntry {
        address: 0x0260,
        name: "brk_exit",
        category: SyscallCategory::System,
        handler: brk_exit,
    });
}

fn memcpy(ctx: &mut SyscallContext) -> SyscallResult {
    // TODO: Implement memory copy
    log::trace!("memcpy: not fully implemented");
    SyscallResult::handled()
}

fn memset(ctx: &mut SyscallContext) -> SyscallResult {
    // TODO: Implement memory set
    log::trace!("memset: not fully implemented");
    SyscallResult::handled()
}

fn brk_exit(_ctx: &mut SyscallContext) -> SyscallResult {
    log::info!("Game exited via BRK");
    // TODO: Signal emulator to stop
    SyscallResult::handled()
}
