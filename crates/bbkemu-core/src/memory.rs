//! Memory bus with bank switching

/// Physical memory map constants
pub const RAM_SIZE: usize = 0x8000;         // 32 KiB
pub const FLASH_SIZE: usize = 0x200000;     // 2 MiB
pub const ROM8_SIZE: usize = 0x200000;      // 2 MiB (font ROM)
pub const ROME_SIZE: usize = 0x200000;      // 2 MiB (OS ROM)

/// Hardware register addresses (page 0)
pub mod registers {
    pub const DATA1: u16 = 0x00;
    pub const DATA2: u16 = 0x01;
    pub const DATA3: u16 = 0x02;
    pub const DATA4: u16 = 0x03;
    pub const ISR: u16 = 0x04;
    pub const TISR: u16 = 0x05;
    pub const BK_SEL: u16 = 0x0C;
    pub const BK_ADRL: u16 = 0x0D;
    pub const BK_ADRH: u16 = 0x0E;
    pub const SYSCON: u16 = 0x200;
    pub const INCR: u16 = 0x207;
    pub const ADDR1L: u16 = 0x208;
    pub const PB: u16 = 0x21B;
    pub const STCON: u16 = 0x226;
    pub const ST1LD: u16 = 0x227;
    pub const ST2LD: u16 = 0x228;
    pub const ST3LD: u16 = 0x229;
    pub const ST4LD: u16 = 0x22A;
    pub const MTCT: u16 = 0x22B;
    pub const STCTCON: u16 = 0x22E;
    pub const CTLD: u16 = 0x22F;
    pub const RTCSEC: u16 = 0x234;
    pub const RTCMIN: u16 = 0x235;
    pub const RTCHR: u16 = 0x236;
    pub const RTCDAYL: u16 = 0x237;
    pub const RTCDAYH: u16 = 0x238;
    pub const IER: u16 = 0x23A;
    pub const TIER: u16 = 0x23B;
    pub const AUDCON: u16 = 0x23F;
    pub const KEYCODE: u16 = 0x24E;
    pub const MACCTL: u16 = 0x260;
    pub const KEY_BUFF_TOP: u16 = 0x2003;
    pub const KEY_BUFF_BOTTOM: u16 = 0x2004;
    pub const KEY_BUFFER: u16 = 0x2008;
}

/// Bank switch controller
pub struct BankSwitch {
    /// Physical address per bank (4 KiB pages)
    banks: [u32; 16],
    /// Currently selected bank register
    selected: u8,
}

impl BankSwitch {
    pub fn new() -> Self {
        Self {
            banks: [0; 16],
            selected: 0,
        }
    }

    /// Translate a 16-bit address to physical address via bank mapping
    pub fn translate(&self, addr: u16) -> u32 {
        let bank = (addr >> 12) as usize;
        (self.banks[bank] << 12) | (addr & 0x0FFF) as u32
    }

    /// Set a bank mapping
    pub fn set(&mut self, bank: u8, physical: u32) {
        if (bank as usize) < 16 {
            self.banks[bank as usize] = physical;
        }
    }

    /// Get selected bank register
    pub fn selected(&self) -> u8 {
        self.selected
    }

    /// Set selected bank register
    pub fn set_selected(&mut self, val: u8) {
        self.selected = val & 0x0F;
    }

    /// Read bank address register (low)
    pub fn read_adrl(&self) -> u8 {
        (self.banks[self.selected as usize] & 0xFF) as u8
    }

    /// Read bank address register (high)
    pub fn read_adrh(&self) -> u8 {
        ((self.banks[self.selected as usize] >> 8) & 0x0F) as u8
    }

    /// Write bank address register (low)
    pub fn write_adrl(&mut self, val: u8) {
        let sel = self.selected as usize;
        self.banks[sel] = (self.banks[sel] & 0xFF00) | val as u32;
    }

    /// Write bank address register (high)
    pub fn write_adrh(&mut self, val: u8) {
        let sel = self.selected as usize;
        self.banks[sel] = (self.banks[sel] & 0x00FF) | ((val as u32 & 0x0F) << 8);
    }
}

/// Main memory bus
pub struct Memory {
    /// RAM (32 KiB)
    pub ram: [u8; RAM_SIZE],
    /// Flash (2 MiB)
    pub flash: [u8; FLASH_SIZE],
    /// Font ROM (2 MiB, optional - loaded from 8.BIN)
    pub rom_8: Option<[u8; ROM8_SIZE]>,
    /// OS ROM (2 MiB, optional - loaded from E.BIN, not needed for HLE)
    pub rom_e: Option<[u8; ROME_SIZE]>,
    /// Bank switch controller
    pub bank_switch: BankSwitch,
    /// Flash command state
    flash_cmd: u8,
    flash_cycles: u8,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            ram: [0; RAM_SIZE],
            flash: [0xFF; FLASH_SIZE],
            rom_8: None,
            rom_e: None,
            bank_switch: BankSwitch::new(),
            flash_cmd: 0,
            flash_cycles: 0,
        }
    }

    /// Initialize memory with default bank mappings
    pub fn init(&mut self) {
        self.ram[registers::INCR as usize] = 0x0F;
    }

    /// Load font ROM (8.BIN) - optional
    pub fn load_rom_8(&mut self, data: &[u8]) {
        let mut rom = [0u8; ROM8_SIZE];
        let len = data.len().min(ROM8_SIZE);
        rom[..len].copy_from_slice(&data[..len]);
        self.rom_8 = Some(rom);
    }

    /// Load OS ROM (E.BIN) - optional for HLE mode
    pub fn load_rom_e(&mut self, data: &[u8]) {
        let mut rom = [0u8; ROME_SIZE];
        let len = data.len().min(ROME_SIZE);
        rom[..len].copy_from_slice(&data[..len]);
        self.rom_e = Some(rom);
    }

    /// Read a byte from memory
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // Page 0 - hardware registers
            0x00..=0xFF => self.read_page0(addr),
            // RAM
            0x100..=0x7FFF => self.ram[addr as usize],
            // High addresses accessed via bank switching
            _ => {
                let paddr = self.bank_switch.translate(addr);
                self.read_physical(paddr)
            }
        }
    }

    /// Write a byte to memory
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x00..=0xFF => self.write_page0(addr, val),
            0x100..=0x7FFF => {
                self.ram[addr as usize] = val;
                // Special handling for certain addresses
                if addr == registers::PB {
                    self.ram[addr as usize] = 0; // Disable audio
                }
                if addr == 0x2028 {
                    self.ram[addr as usize] = 0xFF; // Prevent auto power off
                }
            }
            _ => {
                let paddr = self.bank_switch.translate(addr);
                self.write_physical(paddr, val);
            }
        }
    }

    /// Read a 16-bit value (little-endian)
    pub fn read16(&self, addr: u16) -> u16 {
        self.read(addr) as u16 | (self.read(addr.wrapping_add(1)) as u16) << 8
    }

    fn read_page0(&self, addr: u16) -> u8 {
        match addr {
            registers::DATA1..=registers::DATA4 => {
                // Direct memory access via address registers
                // TODO: implement direct_read
                self.ram[addr as usize]
            }
            registers::BK_SEL => self.bank_switch.selected(),
            registers::BK_ADRL => self.bank_switch.read_adrl(),
            registers::BK_ADRH => self.bank_switch.read_adrh(),
            _ => self.ram[addr as usize],
        }
    }

    fn write_page0(&mut self, addr: u16, val: u8) {
        match addr {
            registers::DATA1..=registers::DATA4 => {
                // TODO: implement direct_write
            }
            registers::ISR => {
                self.ram[addr as usize] &= val;
            }
            registers::TISR => {
                self.ram[addr as usize] &= val;
            }
            registers::BK_SEL => {
                self.bank_switch.set_selected(val);
            }
            registers::BK_ADRL => {
                self.bank_switch.write_adrl(val);
            }
            registers::BK_ADRH => {
                self.bank_switch.write_adrh(val);
            }
            _ => {
                self.ram[addr as usize] = val;
            }
        }
    }

    fn read_physical(&self, addr: u32) -> u8 {
        if addr < 0x8000 {
            self.ram[addr as usize]
        } else if addr >= 0x200000 && addr < 0x400000 {
            self.read_flash(addr - 0x200000)
        } else if addr >= 0x800000 && addr < 0xA00000 {
            if let Some(ref rom) = self.rom_8 {
                rom[(addr - 0x800000) as usize]
            } else {
                0x00
            }
        } else if addr >= 0xE00000 && addr < 0x1000000 {
            if let Some(ref rom) = self.rom_e {
                rom[(addr - 0xE00000) as usize]
            } else {
                0x00
            }
        } else {
            0x00
        }
    }

    fn write_physical(&mut self, addr: u32, val: u8) {
        if addr < 0x8000 {
            self.ram[addr as usize] = val;
        } else if addr >= 0x200000 && addr < 0x400000 {
            self.write_flash(addr - 0x200000, val);
        }
        // ROM areas are read-only
    }

    fn read_flash(&self, addr: u32) -> u8 {
        if self.flash_cmd == 0 || self.flash_cmd == 1 {
            // Rotate last 32KiB to the front for save
            let addr = (addr + 0x8000) % FLASH_SIZE as u32;
            self.flash[addr as usize]
        } else {
            // Software ID or CFI mode
            0x00
        }
    }

    fn write_flash(&mut self, addr: u32, val: u8) {
        // Simplified flash write - full implementation TBD
        if self.flash_cmd == 1 {
            let addr = (addr + 0x8000) % FLASH_SIZE as u32;
            self.flash[addr as usize] = val;
            self.flash_cmd = 0;
            self.flash_cycles = 0;
        }
        // TODO: Implement full flash command sequence
    }

    /// Get RAM slice for save states
    pub fn ram(&self) -> &[u8; RAM_SIZE] {
        &self.ram
    }

    /// Get mutable RAM slice
    pub fn ram_mut(&mut self) -> &mut [u8; RAM_SIZE] {
        &mut self.ram
    }
}
