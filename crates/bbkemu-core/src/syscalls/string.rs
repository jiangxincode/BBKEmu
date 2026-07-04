//! String system call implementations

use crate::syscall::{SyscallCategory, SyscallContext, SyscallEntry, SyscallResult, SyscallTable};

/// Register string syscalls
pub fn register(table: &mut SyscallTable) {
    // String length
    table.register(SyscallEntry {
        address: 0xE050,
        name: "strlen",
        category: SyscallCategory::String,
        handler: strlen,
    });

    // String copy
    table.register(SyscallEntry {
        address: 0xE053,
        name: "strcpy",
        category: SyscallCategory::String,
        handler: strcpy,
    });

    // String compare
    table.register(SyscallEntry {
        address: 0xE056,
        name: "strcmp",
        category: SyscallCategory::String,
        handler: strcmp,
    });

    // String concatenate
    table.register(SyscallEntry {
        address: 0xE059,
        name: "strcat",
        category: SyscallCategory::String,
        handler: strcat,
    });

    // Memory copy
    table.register(SyscallEntry {
        address: 0xE05C,
        name: "memcpy",
        category: SyscallCategory::String,
        handler: memcpy,
    });

    // Memory set
    table.register(SyscallEntry {
        address: 0xE05F,
        name: "memset",
        category: SyscallCategory::String,
        handler: memset,
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
    let dst_lo = ctx.cpu.x() as u16;
    let dst_hi = ctx.cpu.y() as u16;
    let dst = dst_lo | (dst_hi << 8);

    let src_lo = ctx.memory.read(0x20) as u16;
    let src_hi = ctx.memory.read(0x21) as u16;
    let src = src_lo | (src_hi << 8);

    let mut offset = 0u16;
    loop {
        let ch = ctx.memory.read(src + offset);
        ctx.memory.write(dst + offset, ch);
        if ch == 0 {
            break;
        }
        offset += 1;
        if offset > 255 {
            break; // Safety limit
        }
    }

    SyscallResult::handled()
}

fn strcmp(ctx: &mut SyscallContext) -> SyscallResult {
    let str1_lo = ctx.cpu.x() as u16;
    let str1_hi = ctx.cpu.y() as u16;
    let str1 = str1_lo | (str1_hi << 8);

    let str2_lo = ctx.memory.read(0x20) as u16;
    let str2_hi = ctx.memory.read(0x21) as u16;
    let str2 = str2_lo | (str2_hi << 8);

    let mut offset = 0u16;
    loop {
        let ch1 = ctx.memory.read(str1 + offset);
        let ch2 = ctx.memory.read(str2 + offset);

        if ch1 != ch2 {
            return SyscallResult::with_return(if ch1 > ch2 { 1 } else { 0xFF });
        }
        if ch1 == 0 {
            return SyscallResult::with_return(0);
        }

        offset += 1;
        if offset > 255 {
            break;
        }
    }

    SyscallResult::with_return(0)
}

fn strcat(ctx: &mut SyscallContext) -> SyscallResult {
    let dst_lo = ctx.cpu.x() as u16;
    let dst_hi = ctx.cpu.y() as u16;
    let dst = dst_lo | (dst_hi << 8);

    let src_lo = ctx.memory.read(0x20) as u16;
    let src_hi = ctx.memory.read(0x21) as u16;
    let src = src_lo | (src_hi << 8);

    // Find end of destination string
    let mut dst_len = 0u16;
    while ctx.memory.read(dst + dst_len) != 0 {
        dst_len += 1;
        if dst_len > 255 {
            break;
        }
    }

    // Copy source to end of destination
    let mut offset = 0u16;
    loop {
        let ch = ctx.memory.read(src + offset);
        ctx.memory.write(dst + dst_len + offset, ch);
        if ch == 0 {
            break;
        }
        offset += 1;
        if dst_len + offset > 255 {
            break;
        }
    }

    SyscallResult::handled()
}

fn memcpy(ctx: &mut SyscallContext) -> SyscallResult {
    let dst_lo = ctx.cpu.x() as u16;
    let dst_hi = ctx.cpu.y() as u16;
    let dst = dst_lo | (dst_hi << 8);

    let src_lo = ctx.memory.read(0x20) as u16;
    let src_hi = ctx.memory.read(0x21) as u16;
    let src = src_lo | (src_hi << 8);

    let len = ctx.cpu.a() as u16;

    for i in 0..len {
        let byte = ctx.memory.read(src + i);
        ctx.memory.write(dst + i, byte);
    }

    SyscallResult::handled()
}

fn memset(ctx: &mut SyscallContext) -> SyscallResult {
    let dst_lo = ctx.cpu.x() as u16;
    let dst_hi = ctx.cpu.y() as u16;
    let dst = dst_lo | (dst_hi << 8);

    let value = ctx.cpu.a();
    let len = ctx.memory.read(0x20) as u16;

    for i in 0..len {
        ctx.memory.write(dst + i, value);
    }

    SyscallResult::handled()
}
