//! System call implementations for HLE mode

pub mod lcd;
pub mod keyboard;
pub mod audio;
pub mod file;
pub mod timer;
pub mod string;
pub mod misc;
pub mod bbkos;

use crate::syscall::SyscallTable;

/// Build the default syscall table
/// Addresses are based on common BBK OS patterns
pub fn build_syscall_table() -> SyscallTable {
    let mut table = SyscallTable::new();

    // Register LCD syscalls
    lcd::register(&mut table);

    // Register keyboard syscalls
    keyboard::register(&mut table);

    // Register audio syscalls
    audio::register(&mut table);

    // Register file syscalls
    file::register(&mut table);

    // Register timer syscalls
    timer::register(&mut table);

    // Register string syscalls
    string::register(&mut table);

    // Register misc syscalls
    misc::register(&mut table);

    // Register BBK OS syscalls
    bbkos::register(&mut table);

    table
}
