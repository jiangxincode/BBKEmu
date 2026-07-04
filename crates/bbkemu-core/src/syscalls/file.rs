//! File system call implementations

use crate::syscall::{SyscallCategory, SyscallContext, SyscallEntry, SyscallResult, SyscallTable};

/// Register file syscalls
pub fn register(table: &mut SyscallTable) {
    // Open file
    table.register(SyscallEntry {
        address: 0xE060,
        name: "file_open",
        category: SyscallCategory::File,
        handler: file_open,
    });

    // Read file
    table.register(SyscallEntry {
        address: 0xE063,
        name: "file_read",
        category: SyscallCategory::File,
        handler: file_read,
    });

    // Write file
    table.register(SyscallEntry {
        address: 0xE066,
        name: "file_write",
        category: SyscallCategory::File,
        handler: file_write,
    });

    // Close file
    table.register(SyscallEntry {
        address: 0xE069,
        name: "file_close",
        category: SyscallCategory::File,
        handler: file_close,
    });

    // Delete file
    table.register(SyscallEntry {
        address: 0xE06C,
        name: "file_delete",
        category: SyscallCategory::File,
        handler: file_delete,
    });
}

fn file_open(ctx: &mut SyscallContext) -> SyscallResult {
    let mode = ctx.cpu.a();
    let name_lo = ctx.cpu.x() as u16;
    let name_hi = ctx.cpu.y() as u16;
    let name_addr = name_lo | (name_hi << 8);

    // Read filename from memory
    let mut name = String::new();
    for i in 0..255 {
        let ch = ctx.memory.read(name_addr + i);
        if ch == 0 {
            break;
        }
        name.push(ch as char);
    }

    log::trace!("file_open: '{}' mode={}", name, mode);

    // TODO: Implement actual file operations
    // For now, return error
    SyscallResult::with_return(0xFF) // Error
}

fn file_read(ctx: &mut SyscallContext) -> SyscallResult {
    let handle = ctx.cpu.a();
    let buf_lo = ctx.cpu.x() as u16;
    let buf_hi = ctx.cpu.y() as u16;
    let buf_addr = buf_lo | (buf_hi << 8);

    log::trace!("file_read: handle={} buf=0x{:04X}", handle, buf_addr);

    // TODO: Implement actual file read
    SyscallResult::with_return(0) // 0 bytes read
}

fn file_write(ctx: &mut SyscallContext) -> SyscallResult {
    let handle = ctx.cpu.a();
    let buf_lo = ctx.cpu.x() as u16;
    let buf_hi = ctx.cpu.y() as u16;
    let buf_addr = buf_lo | (buf_hi << 8);

    log::trace!("file_write: handle={} buf=0x{:04X}", handle, buf_addr);

    // TODO: Implement actual file write
    SyscallResult::with_return(0) // 0 bytes written
}

fn file_close(ctx: &mut SyscallContext) -> SyscallResult {
    let handle = ctx.cpu.a();
    log::trace!("file_close: handle={}", handle);

    // TODO: Implement actual file close
    SyscallResult::handled()
}

fn file_delete(ctx: &mut SyscallContext) -> SyscallResult {
    let name_lo = ctx.cpu.x() as u16;
    let name_hi = ctx.cpu.y() as u16;
    let name_addr = name_lo | (name_hi << 8);

    // Read filename from memory
    let mut name = String::new();
    for i in 0..255 {
        let ch = ctx.memory.read(name_addr + i);
        if ch == 0 {
            break;
        }
        name.push(ch as char);
    }

    log::trace!("file_delete: '{}'", name);

    // TODO: Implement actual file delete
    SyscallResult::with_return(0xFF) // Error
}
