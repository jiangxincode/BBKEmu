//! 6502 CPU wrapper
//!
//! For now, implements a basic 6502 CPU directly.
//! Later can be replaced with the mos6502 crate's Bus trait for accuracy.

use crate::memory::Memory;

/// 6502 CPU registers
#[derive(Clone, Copy)]
pub struct Registers {
    pub pc: u16,
    pub sp: u8,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub status: u8,
}

/// Wraps6502 CPU with BBK-specific functionality
pub struct CpuWrapper {
    pub registers: Registers,
    pub cycles: u64,
}

impl CpuWrapper {
    pub fn new() -> Self {
        Self {
            registers: Registers {
                pc: 0,
                sp: 0xFF,
                a: 0,
                x: 0,
                y: 0,
                status: 0x04, // Interrupt disable
            },
            cycles: 0,
        }
    }

    /// Get the program counter
    pub fn pc(&self) -> u16 {
        self.registers.pc
    }

    /// Set the program counter
    pub fn set_pc(&mut self, addr: u16) {
        self.registers.pc = addr;
    }

    /// Get the stack pointer
    pub fn sp(&self) -> u8 {
        self.registers.sp
    }

    /// Set the stack pointer
    pub fn set_sp(&mut self, val: u8) {
        self.registers.sp = val;
    }

    /// Get accumulator
    pub fn a(&self) -> u8 {
        self.registers.a
    }

    /// Set accumulator
    pub fn set_a(&mut self, val: u8) {
        self.registers.a = val;
    }

    /// Get X register
    pub fn x(&self) -> u8 {
        self.registers.x
    }

    /// Get Y register
    pub fn y(&self) -> u8 {
        self.registers.y
    }

    /// Get processor status
    pub fn status(&self) -> u8 {
        self.registers.status
    }

    /// Push a byte onto the stack
    pub fn push(&mut self, memory: &mut Memory, val: u8) {
        memory.write(0x100 | self.registers.sp as u16, val);
        self.registers.sp = self.registers.sp.wrapping_sub(1);
    }

    /// Push a 16-bit value onto the stack (high byte first)
    pub fn push16(&mut self, memory: &mut Memory, val: u16) {
        self.push(memory, (val >> 8) as u8);
        self.push(memory, (val & 0xFF) as u8);
    }

    /// Pop a byte from the stack
    pub fn pop(&mut self, memory: &Memory) -> u8 {
        self.registers.sp = self.registers.sp.wrapping_add(1);
        memory.read(0x100 | self.registers.sp as u16)
    }

    /// Pop a 16-bit value from the stack (low byte first)
    pub fn pop16(&mut self, memory: &Memory) -> u16 {
        let lo = self.pop(memory) as u16;
        let hi = self.pop(memory) as u16;
        (hi << 8) | lo
    }

    /// Execute a single instruction, returns cycles consumed
    pub fn step(&mut self, memory: &mut Memory) -> u32 {
        let opcode = memory.read(self.registers.pc);

        match opcode {
            // NOP
            0xEA => {
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // BRK
            0x00 => {
                self.registers.pc += 1;
                self.push16(memory, self.registers.pc);
                self.push(memory, self.registers.status | 0x10);
                self.registers.status |= 0x04; // Set interrupt disable
                let lo = memory.read(0xFFFE) as u16;
                let hi = memory.read(0xFFFF) as u16;
                self.registers.pc = (hi << 8) | lo;
                self.cycles += 7;
                7
            }
            // JSR absolute
            0x20 => {
                let lo = memory.read(self.registers.pc + 1) as u16;
                let hi = memory.read(self.registers.pc + 2) as u16;
                let target = (hi << 8) | lo;
                let return_addr = self.registers.pc + 2;
                self.push16(memory, return_addr);
                self.registers.pc = target;
                self.cycles += 6;
                6
            }
            // RTS
            0x60 => {
                let addr = self.pop16(memory);
                self.registers.pc = addr + 1;
                self.cycles += 6;
                6
            }
            // RTI
            0x40 => {
                self.registers.status = self.pop(memory);
                let addr = self.pop16(memory);
                self.registers.pc = addr;
                self.cycles += 6;
                6
            }
            // LDA immediate
            0xA9 => {
                self.registers.a = memory.read(self.registers.pc + 1);
                self.update_nz(self.registers.a);
                self.registers.pc += 2;
                self.cycles += 2;
                2
            }
            // LDX immediate
            0xA2 => {
                self.registers.x = memory.read(self.registers.pc + 1);
                self.update_nz(self.registers.x);
                self.registers.pc += 2;
                self.cycles += 2;
                2
            }
            // LDY immediate
            0xA0 => {
                self.registers.y = memory.read(self.registers.pc + 1);
                self.update_nz(self.registers.y);
                self.registers.pc += 2;
                self.cycles += 2;
                2
            }
            // STA absolute
            0x8D => {
                let lo = memory.read(self.registers.pc + 1) as u16;
                let hi = memory.read(self.registers.pc + 2) as u16;
                let addr = (hi << 8) | lo;
                memory.write(addr, self.registers.a);
                self.registers.pc += 3;
                self.cycles += 4;
                4
            }
            // STX absolute
            0x8E => {
                let lo = memory.read(self.registers.pc + 1) as u16;
                let hi = memory.read(self.registers.pc + 2) as u16;
                let addr = (hi << 8) | lo;
                memory.write(addr, self.registers.x);
                self.registers.pc += 3;
                self.cycles += 4;
                4
            }
            // STY absolute
            0x8C => {
                let lo = memory.read(self.registers.pc + 1) as u16;
                let hi = memory.read(self.registers.pc + 2) as u16;
                let addr = (hi << 8) | lo;
                memory.write(addr, self.registers.y);
                self.registers.pc += 3;
                self.cycles += 4;
                4
            }
            // TAX
            0xAA => {
                self.registers.x = self.registers.a;
                self.update_nz(self.registers.x);
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // TAY
            0xA8 => {
                self.registers.y = self.registers.a;
                self.update_nz(self.registers.y);
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // TXA
            0x8A => {
                self.registers.a = self.registers.x;
                self.update_nz(self.registers.a);
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // TYA
            0x98 => {
                self.registers.a = self.registers.y;
                self.update_nz(self.registers.a);
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // TXS
            0x9A => {
                self.registers.sp = self.registers.x;
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // TSX
            0xBA => {
                self.registers.x = self.registers.sp;
                self.update_nz(self.registers.x);
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // JMP absolute
            0x4C => {
                let lo = memory.read(self.registers.pc + 1) as u16;
                let hi = memory.read(self.registers.pc + 2) as u16;
                self.registers.pc = (hi << 8) | lo;
                self.cycles += 3;
                3
            }
            // JMP indirect
            0x6C => {
                let lo = memory.read(self.registers.pc + 1) as u16;
                let hi = memory.read(self.registers.pc + 2) as u16;
                let addr = (hi << 8) | lo;
                let target_lo = memory.read(addr) as u16;
                // 6502 bug: high byte wraps within page
                let target_hi = memory.read((addr & 0xFF00) | ((addr + 1) & 0x00FF)) as u16;
                self.registers.pc = (target_hi << 8) | target_lo;
                self.cycles += 5;
                5
            }
            // BNE relative
            0xD0 => {
                let offset = memory.read(self.registers.pc + 1) as i8;
                self.registers.pc += 2;
                if self.registers.status & 0x02 == 0 {
                    // Zero flag not set
                    let old_pc = self.registers.pc;
                    self.registers.pc = self.registers.pc.wrapping_add(offset as u16);
                    self.cycles += 3; // +1 for branch taken
                    3
                } else {
                    self.cycles += 2;
                    2
                }
            }
            // BEQ relative
            0xF0 => {
                let offset = memory.read(self.registers.pc + 1) as i8;
                self.registers.pc += 2;
                if self.registers.status & 0x02 != 0 {
                    // Zero flag set
                    self.registers.pc = self.registers.pc.wrapping_add(offset as u16);
                    self.cycles += 3;
                    3
                } else {
                    self.cycles += 2;
                    2
                }
            }
            // SEC
            0x38 => {
                self.registers.status |= 0x01;
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // CLC
            0x18 => {
                self.registers.status &= !0x01;
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // SED
            0xF8 => {
                self.registers.status |= 0x08;
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // CLD
            0xD8 => {
                self.registers.status &= !0x08;
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // SEI
            0x78 => {
                self.registers.status |= 0x04;
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // CLI
            0x58 => {
                self.registers.status &= !0x04;
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // PHA
            0x48 => {
                self.push(memory, self.registers.a);
                self.registers.pc += 1;
                self.cycles += 3;
                3
            }
            // PLA
            0x68 => {
                self.registers.a = self.pop(memory);
                self.update_nz(self.registers.a);
                self.registers.pc += 1;
                self.cycles += 4;
                4
            }
            // PHP
            0x08 => {
                self.push(memory, self.registers.status | 0x30);
                self.registers.pc += 1;
                self.cycles += 3;
                3
            }
            // PLP
            0x28 => {
                self.registers.status = (self.pop(memory) & 0xEF) | 0x20;
                self.registers.pc += 1;
                self.cycles += 4;
                4
            }
            // CMP immediate
            0xC9 => {
                let val = memory.read(self.registers.pc + 1);
                let result = self.registers.a.wrapping_sub(val);
                self.update_nz(result);
                if self.registers.a >= val {
                    self.registers.status |= 0x01; // Carry
                } else {
                    self.registers.status &= !0x01;
                }
                self.registers.pc += 2;
                self.cycles += 2;
                2
            }
            // CPX immediate
            0xE0 => {
                let val = memory.read(self.registers.pc + 1);
                let result = self.registers.x.wrapping_sub(val);
                self.update_nz(result);
                if self.registers.x >= val {
                    self.registers.status |= 0x01;
                } else {
                    self.registers.status &= !0x01;
                }
                self.registers.pc += 2;
                self.cycles += 2;
                2
            }
            // CPY immediate
            0xC0 => {
                let val = memory.read(self.registers.pc + 1);
                let result = self.registers.y.wrapping_sub(val);
                self.update_nz(result);
                if self.registers.y >= val {
                    self.registers.status |= 0x01;
                } else {
                    self.registers.status &= !0x01;
                }
                self.registers.pc += 2;
                self.cycles += 2;
                2
            }
            // INX
            0xE8 => {
                self.registers.x = self.registers.x.wrapping_add(1);
                self.update_nz(self.registers.x);
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // INY
            0xC8 => {
                self.registers.y = self.registers.y.wrapping_add(1);
                self.update_nz(self.registers.y);
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // DEX
            0xCA => {
                self.registers.x = self.registers.x.wrapping_sub(1);
                self.update_nz(self.registers.x);
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // DEY
            0x88 => {
                self.registers.y = self.registers.y.wrapping_sub(1);
                self.update_nz(self.registers.y);
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // INC absolute
            0xEE => {
                let lo = memory.read(self.registers.pc + 1) as u16;
                let hi = memory.read(self.registers.pc + 2) as u16;
                let addr = (hi << 8) | lo;
                let val = memory.read(addr).wrapping_add(1);
                memory.write(addr, val);
                self.update_nz(val);
                self.registers.pc += 3;
                self.cycles += 6;
                6
            }
            // DEC absolute
            0xCE => {
                let lo = memory.read(self.registers.pc + 1) as u16;
                let hi = memory.read(self.registers.pc + 2) as u16;
                let addr = (hi << 8) | lo;
                let val = memory.read(addr).wrapping_sub(1);
                memory.write(addr, val);
                self.update_nz(val);
                self.registers.pc += 3;
                self.cycles += 6;
                6
            }
            // AND immediate
            0x29 => {
                self.registers.a &= memory.read(self.registers.pc + 1);
                self.update_nz(self.registers.a);
                self.registers.pc += 2;
                self.cycles += 2;
                2
            }
            // ORA immediate
            0x09 => {
                self.registers.a |= memory.read(self.registers.pc + 1);
                self.update_nz(self.registers.a);
                self.registers.pc += 2;
                self.cycles += 2;
                2
            }
            // EOR immediate
            0x49 => {
                self.registers.a ^= memory.read(self.registers.pc + 1);
                self.update_nz(self.registers.a);
                self.registers.pc += 2;
                self.cycles += 2;
                2
            }
            // ADC immediate
            0x69 => {
                let val = memory.read(self.registers.pc + 1);
                let carry = if self.registers.status & 0x01 != 0 { 1 } else { 0 };
                let result = self.registers.a as u16 + val as u16 + carry;
                let overflow = (!(self.registers.a ^ val) & (self.registers.a ^ result as u8) & 0x80) != 0;
                self.registers.a = result as u8;
                if result > 0xFF {
                    self.registers.status |= 0x01;
                } else {
                    self.registers.status &= !0x01;
                }
                if overflow {
                    self.registers.status |= 0x40;
                } else {
                    self.registers.status &= !0x40;
                }
                self.update_nz(self.registers.a);
                self.registers.pc += 2;
                self.cycles += 2;
                2
            }
            // SBC immediate
            0xE9 => {
                let val = memory.read(self.registers.pc + 1);
                let carry = if self.registers.status & 0x01 != 0 { 0 } else { 1 };
                let result = self.registers.a as i16 - val as i16 - carry as i16;
                let overflow = ((self.registers.a ^ val) & (self.registers.a ^ result as u8) & 0x80) != 0;
                self.registers.a = result as u8;
                if result >= 0 {
                    self.registers.status |= 0x01;
                } else {
                    self.registers.status &= !0x01;
                }
                if overflow {
                    self.registers.status |= 0x40;
                } else {
                    self.registers.status &= !0x40;
                }
                self.update_nz(self.registers.a);
                self.registers.pc += 2;
                self.cycles += 2;
                2
            }
            // BIT absolute
            0x2C => {
                let lo = memory.read(self.registers.pc + 1) as u16;
                let hi = memory.read(self.registers.pc + 2) as u16;
                let addr = (hi << 8) | lo;
                let val = memory.read(addr);
                if val & 0x80 != 0 { self.registers.status |= 0x80; } else { self.registers.status &= !0x80; }
                if val & 0x40 != 0 { self.registers.status |= 0x40; } else { self.registers.status &= !0x40; }
                if self.registers.a & val == 0 { self.registers.status |= 0x02; } else { self.registers.status &= !0x02; }
                self.registers.pc += 3;
                self.cycles += 4;
                4
            }
            // BCS relative
            0xB0 => {
                let offset = memory.read(self.registers.pc + 1) as i8;
                self.registers.pc += 2;
                if self.registers.status & 0x01 != 0 {
                    self.registers.pc = self.registers.pc.wrapping_add(offset as u16);
                    self.cycles += 3;
                    3
                } else {
                    self.cycles += 2;
                    2
                }
            }
            // BCC relative
            0x90 => {
                let offset = memory.read(self.registers.pc + 1) as i8;
                self.registers.pc += 2;
                if self.registers.status & 0x01 == 0 {
                    self.registers.pc = self.registers.pc.wrapping_add(offset as u16);
                    self.cycles += 3;
                    3
                } else {
                    self.cycles += 2;
                    2
                }
            }
            // BMI relative
            0x30 => {
                let offset = memory.read(self.registers.pc + 1) as i8;
                self.registers.pc += 2;
                if self.registers.status & 0x80 != 0 {
                    self.registers.pc = self.registers.pc.wrapping_add(offset as u16);
                    self.cycles += 3;
                    3
                } else {
                    self.cycles += 2;
                    2
                }
            }
            // BPL relative
            0x10 => {
                let offset = memory.read(self.registers.pc + 1) as i8;
                self.registers.pc += 2;
                if self.registers.status & 0x80 == 0 {
                    self.registers.pc = self.registers.pc.wrapping_add(offset as u16);
                    self.cycles += 3;
                    3
                } else {
                    self.cycles += 2;
                    2
                }
            }
            // BVC relative
            0x50 => {
                let offset = memory.read(self.registers.pc + 1) as i8;
                self.registers.pc += 2;
                if self.registers.status & 0x40 == 0 {
                    self.registers.pc = self.registers.pc.wrapping_add(offset as u16);
                    self.cycles += 3;
                    3
                } else {
                    self.cycles += 2;
                    2
                }
            }
            // BVS relative
            0x70 => {
                let offset = memory.read(self.registers.pc + 1) as i8;
                self.registers.pc += 2;
                if self.registers.status & 0x40 != 0 {
                    self.registers.pc = self.registers.pc.wrapping_add(offset as u16);
                    self.cycles += 3;
                    3
                } else {
                    self.cycles += 2;
                    2
                }
            }
            // ASL accumulator
            0x0A => {
                let carry = self.registers.a & 0x80 != 0;
                self.registers.a <<= 1;
                if carry { self.registers.status |= 0x01; } else { self.registers.status &= !0x01; }
                self.update_nz(self.registers.a);
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // LSR accumulator
            0x4A => {
                let carry = self.registers.a & 0x01 != 0;
                self.registers.a >>= 1;
                if carry { self.registers.status |= 0x01; } else { self.registers.status &= !0x01; }
                self.update_nz(self.registers.a);
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // ROL accumulator
            0x2A => {
                let old_carry = if self.registers.status & 0x01 != 0 { 1 } else { 0 };
                let new_carry = self.registers.a & 0x80 != 0;
                self.registers.a = (self.registers.a << 1) | old_carry;
                if new_carry { self.registers.status |= 0x01; } else { self.registers.status &= !0x01; }
                self.update_nz(self.registers.a);
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // ROR accumulator
            0x6A => {
                let old_carry = if self.registers.status & 0x01 != 0 { 0x80 } else { 0 };
                let new_carry = self.registers.a & 0x01 != 0;
                self.registers.a = (self.registers.a >> 1) | old_carry;
                if new_carry { self.registers.status |= 0x01; } else { self.registers.status &= !0x01; }
                self.update_nz(self.registers.a);
                self.registers.pc += 1;
                self.cycles += 2;
                2
            }
            // LDA zero page
            0xA5 => {
                let addr = memory.read(self.registers.pc + 1) as u16;
                self.registers.a = memory.read(addr);
                self.update_nz(self.registers.a);
                self.registers.pc += 2;
                self.cycles += 3;
                3
            }
            // STA zero page
            0x85 => {
                let addr = memory.read(self.registers.pc + 1) as u16;
                memory.write(addr, self.registers.a);
                self.registers.pc += 2;
                self.cycles += 3;
                3
            }
            // LDA absolute
            0xAD => {
                let lo = memory.read(self.registers.pc + 1) as u16;
                let hi = memory.read(self.registers.pc + 2) as u16;
                let addr = (hi << 8) | lo;
                self.registers.a = memory.read(addr);
                self.update_nz(self.registers.a);
                self.registers.pc += 3;
                self.cycles += 4;
                4
            }
            // Default: unknown opcode, skip
            _ => {
                log::warn!("Unknown opcode: 0x{:02X} at 0x{:04X}", opcode, self.registers.pc);
                self.registers.pc += 1;
                self.cycles += 1;
                1
            }
        }
    }

    /// Update Negative and Zero flags based on a value
    fn update_nz(&mut self, val: u8) {
        if val == 0 {
            self.registers.status |= 0x02; // Zero
        } else {
            self.registers.status &= !0x02;
        }
        if val & 0x80 != 0 {
            self.registers.status |= 0x80; // Negative
        } else {
            self.registers.status &= !0x80;
        }
    }
}
