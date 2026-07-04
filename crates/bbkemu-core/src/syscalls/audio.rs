//! Audio system call implementations

use crate::syscall::{SyscallCategory, SyscallContext, SyscallEntry, SyscallResult, SyscallTable};

/// Register audio syscalls
pub fn register(table: &mut SyscallTable) {
    // Play beep/tone
    table.register(SyscallEntry {
        address: 0xE030,
        name: "beep",
        category: SyscallCategory::Audio,
        handler: beep,
    });

    // Stop sound
    table.register(SyscallEntry {
        address: 0xE033,
        name: "sound_stop",
        category: SyscallCategory::Audio,
        handler: sound_stop,
    });

    // Play music
    table.register(SyscallEntry {
        address: 0xE036,
        name: "music_play",
        category: SyscallCategory::Audio,
        handler: music_play,
    });

    // Stop music
    table.register(SyscallEntry {
        address: 0xE039,
        name: "music_stop",
        category: SyscallCategory::Audio,
        handler: music_stop,
    });
}

fn beep(ctx: &mut SyscallContext) -> SyscallResult {
    let freq_lo = ctx.cpu.x() as u16;
    let freq_hi = ctx.cpu.y() as u16;
    let freq = freq_lo | (freq_hi << 8);
    let duration = ctx.cpu.a() as u16;

    if freq > 0 {
        ctx.audio.play_tone(freq, duration * 10);
    }

    SyscallResult::handled()
}

fn sound_stop(ctx: &mut SyscallContext) -> SyscallResult {
    ctx.audio.stop();
    SyscallResult::handled()
}

fn music_play(ctx: &mut SyscallContext) -> SyscallResult {
    // TODO: Implement music playback
    log::trace!("music_play: not implemented");
    SyscallResult::handled()
}

fn music_stop(ctx: &mut SyscallContext) -> SyscallResult {
    ctx.audio.stop();
    SyscallResult::handled()
}
