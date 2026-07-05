//! Debugger for development and ROM hacking

use std::collections::HashSet;

/// Watchpoint type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchKind {
    Read,
    Write,
    ReadWrite,
}

/// Watchpoint
pub struct Watchpoint {
    pub address: u16,
    pub kind: WatchKind,
}

/// CPU trace entry
pub struct TraceEntry {
    pub cycle: u64,
    pub pc: u16,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub status: u8,
    pub instruction: String,
}

/// Syscall log entry
pub struct SyscallLogEntry {
    pub cycle: u64,
    pub address: u16,
    pub name: String,
}

/// Debugger
pub struct Debugger {
    /// Breakpoint addresses
    breakpoints: HashSet<u16>,
    /// Watchpoints
    watchpoints: Vec<Watchpoint>,
    /// Whether we're in single-step mode
    stepping: bool,
    /// Trace log buffer
    trace_log: Vec<TraceEntry>,
    /// Max trace log size
    max_trace: usize,
    /// Syscall log
    syscall_log: Vec<SyscallLogEntry>,
    /// Whether syscall logging is enabled
    pub syscall_logging: bool,
}

impl Default for Debugger {
    fn default() -> Self {
        Self::new()
    }
}

impl Debugger {
    pub fn new() -> Self {
        Self {
            breakpoints: HashSet::new(),
            watchpoints: Vec::new(),
            stepping: false,
            trace_log: Vec::new(),
            max_trace: 10000,
            syscall_log: Vec::new(),
            syscall_logging: false,
        }
    }

    /// Add a breakpoint
    pub fn add_breakpoint(&mut self, addr: u16) {
        self.breakpoints.insert(addr);
        log::info!("Breakpoint added at 0x{:04X}", addr);
    }

    /// Remove a breakpoint
    pub fn remove_breakpoint(&mut self, addr: u16) {
        self.breakpoints.remove(&addr);
        log::info!("Breakpoint removed at 0x{:04X}", addr);
    }

    /// Check if address has a breakpoint
    pub fn has_breakpoint(&self, addr: u16) -> bool {
        self.breakpoints.contains(&addr)
    }

    /// Add a watchpoint
    pub fn add_watchpoint(&mut self, addr: u16, kind: WatchKind) {
        self.watchpoints.push(Watchpoint {
            address: addr,
            kind,
        });
        log::info!("Watchpoint added at 0x{:04X} ({:?})", addr, kind);
    }

    /// Set single-step mode
    pub fn set_stepping(&mut self, stepping: bool) {
        self.stepping = stepping;
    }

    /// Check if in single-step mode
    pub fn is_stepping(&self) -> bool {
        self.stepping
    }

    /// Add a trace entry
    pub fn add_trace(&mut self, entry: TraceEntry) {
        if self.trace_log.len() >= self.max_trace {
            self.trace_log.remove(0);
        }
        self.trace_log.push(entry);
    }

    /// Get trace log
    pub fn trace_log(&self) -> &[TraceEntry] {
        &self.trace_log
    }

    /// Log a syscall
    pub fn log_syscall(&mut self, cycle: u64, address: u16, name: &str) {
        if self.syscall_logging {
            self.syscall_log.push(SyscallLogEntry {
                cycle,
                address,
                name: name.to_string(),
            });
        }
    }

    /// Get syscall log
    pub fn syscall_log(&self) -> &[SyscallLogEntry] {
        &self.syscall_log
    }

    /// Clear all logs
    pub fn clear_logs(&mut self) {
        self.trace_log.clear();
        self.syscall_log.clear();
    }

    /// Get all breakpoints
    pub fn breakpoints(&self) -> &HashSet<u16> {
        &self.breakpoints
    }
}
