//! Audio system call implementations

use crate::syscall::{SyscallCategory, SyscallContext, SyscallEntry, SyscallResult, SyscallTable};

pub fn register(table: &mut SyscallTable) {
    table.register(SyscallEntry {
        address: 0xE020, // TBD
        name: "beep",
        category: SyscallCategory::Audio,
        handler: beep,
    });

    table.register(SyscallEntry {
        address: 0xE023, // TBD
        name: "sound_stop",
        category: SyscallCategory::Audio,
        handler: sound_stop,
    });
}

fn beep(ctx: &mut SyscallContext) -> SyscallResult {
    let freq_lo = ctx.cpu.x() as u16;
    let freq_hi = ctx.cpu.y() as u16;
    let freq = freq_lo | (freq_hi << 8);
    let duration = ctx.cpu.a() as u16;
    ctx.audio.play_tone(freq, duration * 10); // Assume units of 10ms
    SyscallResult::handled()
}

fn sound_stop(ctx: &mut SyscallContext) -> SyscallResult {
    ctx.audio.stop();
    SyscallResult::handled()
}
