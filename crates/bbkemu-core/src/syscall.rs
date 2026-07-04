//! System call dispatcher for HLE mode

use std::collections::HashMap;

use crate::cpu::CpuWrapper;
use crate::memory::Memory;
use crate::lcd::Lcd;
use crate::input::Input;
use crate::audio::Audio;

/// System call category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyscallCategory {
    Lcd,
    Keyboard,
    Audio,
    File,
    Timer,
    String,
    Math,
    System,
}

/// System call handler function type
pub type SyscallHandler = fn(&mut SyscallContext) -> SyscallResult;

/// Context passed to syscall handlers
pub struct SyscallContext<'a> {
    pub cpu: &'a mut CpuWrapper,
    pub memory: &'a mut Memory,
    pub lcd: &'a mut Lcd,
    pub input: &'a mut Input,
    pub audio: &'a mut Audio,
}

/// Result of a syscall execution
pub struct SyscallResult {
    /// Whether the syscall was handled
    pub handled: bool,
    /// Optional return value in A register
    pub return_value: Option<u8>,
    /// Cycles consumed
    pub cycles: u32,
}

impl SyscallResult {
    pub fn handled() -> Self {
        Self {
            handled: true,
            return_value: None,
            cycles: 0,
        }
    }

    pub fn with_return(val: u8) -> Self {
        Self {
            handled: true,
            return_value: Some(val),
            cycles: 0,
        }
    }

    pub fn not_handled() -> Self {
        Self {
            handled: false,
            return_value: None,
            cycles: 0,
        }
    }
}

/// System call entry
pub struct SyscallEntry {
    /// Entry point address
    pub address: u16,
    /// Human-readable name
    pub name: &'static str,
    /// Category
    pub category: SyscallCategory,
    /// Handler function
    pub handler: SyscallHandler,
}

/// System call table
pub struct SyscallTable {
    /// Syscalls indexed by address
    entries: HashMap<u16, SyscallEntry>,
}

impl SyscallTable {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Register a syscall
    pub fn register(&mut self, entry: SyscallEntry) {
        self.entries.insert(entry.address, entry);
    }

    /// Try to handle a JSR target as a syscall
    pub fn try_handle(&self, target: u16, ctx: &mut SyscallContext) -> SyscallResult {
        if let Some(entry) = self.entries.get(&target) {
            log::trace!("Syscall: {} (0x{:04X})", entry.name, target);
            (entry.handler)(ctx)
        } else {
            SyscallResult::not_handled()
        }
    }

    /// Check if an address is a known syscall
    pub fn is_syscall(&self, addr: u16) -> bool {
        self.entries.contains_key(&addr)
    }

    /// Get syscall info for an address
    pub fn get(&self, addr: u16) -> Option<&SyscallEntry> {
        self.entries.get(&addr)
    }

    /// Get all registered syscalls
    pub fn all(&self) -> impl Iterator<Item = &SyscallEntry> {
        self.entries.values()
    }
}
