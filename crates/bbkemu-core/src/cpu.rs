//! 6502 CPU wrapper using mos6502 crate

use mos6502::cpu::CPU;
use mos6502::instruction::Nmos6502;

use crate::memory::Memory;

/// Wraps the mos6502 CPU with BBK-specific functionality
pub struct CpuWrapper {
    /// The underlying mos6502 CPU
    pub inner: CPU<Memory, Nmos6502>,
}

impl CpuWrapper {
    pub fn new(memory: Memory) -> Self {
        Self {
            inner: CPU::new(memory, Nmos6502),
        }
    }

    /// Get the program counter
    pub fn pc(&self) -> u16 {
        self.inner.registers.program_counter
    }

    /// Set the program counter
    pub fn set_pc(&mut self, addr: u16) {
        self.inner.registers.program_counter = addr;
    }

    /// Get the stack pointer
    pub fn sp(&self) -> u8 {
        self.inner.registers.stack_pointer.0
    }

    /// Set the stack pointer
    pub fn set_sp(&mut self, val: u8) {
        self.inner.registers.stack_pointer.0 = val;
    }

    /// Get accumulator
    pub fn a(&self) -> u8 {
        self.inner.registers.accumulator
    }

    /// Set accumulator
    pub fn set_a(&mut self, val: u8) {
        self.inner.registers.accumulator = val;
    }

    /// Get X register
    pub fn x(&self) -> u8 {
        self.inner.registers.index_x
    }

    /// Get Y register
    pub fn y(&self) -> u8 {
        self.inner.registers.index_y
    }

    /// Get processor status
    pub fn status(&self) -> u8 {
        self.inner.registers.status.bits()
    }

    /// Get total cycles
    pub fn cycles(&self) -> u64 {
        self.inner.cycles
    }

    /// Reset the CPU
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Execute a single instruction
    pub fn step(&mut self) -> u32 {
        let before = self.inner.cycles;

        // Fetch and decode the next instruction
        if let Some(decoded) = self.inner.fetch_next_and_decode() {
            self.inner.execute_instruction(decoded);
        }

        (self.inner.cycles - before) as u32
    }

    /// Access the memory bus
    pub fn memory(&self) -> &Memory {
        &self.inner.memory
    }

    /// Access the memory bus mutably
    pub fn memory_mut(&mut self) -> &mut Memory {
        &mut self.inner.memory
    }
}
