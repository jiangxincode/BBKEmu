//! Timer system call implementations

use crate::syscall::{SyscallCategory, SyscallContext, SyscallEntry, SyscallResult, SyscallTable};

pub fn register(table: &mut SyscallTable) {
    table.register(SyscallEntry {
        address: 0xE040, // TBD
        name: "timer_set",
        category: SyscallCategory::Timer,
        handler: timer_set,
    });

    table.register(SyscallEntry {
        address: 0xE043, // TBD
        name: "rtc_read",
        category: SyscallCategory::Timer,
        handler: rtc_read,
    });
}

fn timer_set(ctx: &mut SyscallContext) -> SyscallResult {
    let channel = ctx.cpu.a();
    let value = ctx.cpu.x();
    log::trace!("timer_set: channel={}, value={}", channel, value);
    // TODO: Implement timer
    SyscallResult::handled()
}

fn rtc_read(ctx: &mut SyscallContext) -> SyscallResult {
    let field = ctx.cpu.a();
    // TODO: Read RTC field
    log::trace!("rtc_read: field={}", field);
    SyscallResult::with_return(0)
}
