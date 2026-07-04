//! File system call implementations

use crate::syscall::{SyscallCategory, SyscallContext, SyscallEntry, SyscallResult, SyscallTable};

pub fn register(table: &mut SyscallTable) {
    // TODO: Implement file operations
    table.register(SyscallEntry {
        address: 0xE030, // TBD
        name: "file_open",
        category: SyscallCategory::File,
        handler: file_open,
    });
}

fn file_open(_ctx: &mut SyscallContext) -> SyscallResult {
    log::warn!("file_open: not implemented");
    SyscallResult::with_return(0xFF) // Error
}
