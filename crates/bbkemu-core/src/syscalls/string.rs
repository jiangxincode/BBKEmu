//! String system call implementations

use crate::syscall::{SyscallCategory, SyscallContext, SyscallEntry, SyscallResult, SyscallTable};

pub fn register(table: &mut SyscallTable) {
    table.register(SyscallEntry {
        address: 0xE050, // TBD
        name: "strlen",
        category: SyscallCategory::String,
        handler: strlen,
    });

    table.register(SyscallEntry {
        address: 0xE053, // TBD
        name: "strcpy",
        category: SyscallCategory::String,
        handler: strcpy,
    });
}

fn strlen(ctx: &mut SyscallContext) -> SyscallResult {
    let addr_lo = ctx.cpu.x() as u16;
    let addr_hi = ctx.cpu.y() as u16;
    let addr = addr_lo | (addr_hi << 8);
    let mut len = 0u8;
    loop {
        let b = ctx.memory.read(addr.wrapping_add(len as u16));
        if b == 0 {
            break;
        }
        len = len.wrapping_add(1);
        if len == 0 {
            break; // Overflow
        }
    }
    SyscallResult::with_return(len)
}

fn strcpy(ctx: &mut SyscallContext) -> SyscallResult {
    log::trace!("strcpy: not fully implemented");
    SyscallResult::handled()
}
