//! Memory bus with bank switching and hardware register handling

use mos6502::memory::Bus;

/// Physical memory map constants
pub const RAM_SIZE: usize = 0x8000;         // 32 KiB
pub const FLASH_SIZE: usize = 0x200000;     // 2 MiB
pub const ROM8_SIZE: usize = 0x200000;      // 2 MiB (font ROM)
pub const ROME_SIZE: usize = 0x200000;      // 2 MiB (OS ROM)

/// Hardware register addresses
pub mod registers {
    // Page 0 (0x00-0xFF)
    pub const DATA1: u16 = 0x00;
    pub const DATA2: u16 = 0x01;
    pub const DATA3: u16 = 0x02;
    pub const DATA4: u16 = 0x03;
    pub const ISR: u16 = 0x04;
    pub const TISR: u16 = 0x05;
    pub const BK_SEL: u16 = 0x0C;
    pub const BK_ADRL: u16 = 0x0D;
    pub const BK_ADRH: u16 = 0x0E;
    pub const IRCNT: u16 = 0x1B;

    // Page 2 (0x200-0x2FF)
    pub const SYSCON: u16 = 0x200;
    pub const INCR: u16 = 0x207;
    pub const ADDR1L: u16 = 0x208;
    pub const ADDR1M: u16 = 0x209;
    pub const ADDR1H: u16 = 0x20A;
    pub const ADDR2L: u16 = 0x20B;
    pub const ADDR2M: u16 = 0x20C;
    pub const ADDR2H: u16 = 0x20D;
    pub const ADDR3L: u16 = 0x20E;
    pub const ADDR3M: u16 = 0x20F;
    pub const ADDR3H: u16 = 0x210;
    pub const ADDR4L: u16 = 0x211;
    pub const ADDR4M: u16 = 0x212;
    pub const ADDR4H: u16 = 0x213;
    pub const PB: u16 = 0x21B;
    pub const STCON: u16 = 0x226;
    pub const ST1LD: u16 = 0x227;
    pub const ST2LD: u16 = 0x228;
    pub const ST3LD: u16 = 0x229;
    pub const ST4LD: u16 = 0x22A;
    pub const MTCT: u16 = 0x22B;
    pub const STCTCON: u16 = 0x22E;
    pub const CTLD: u16 = 0x22F;
    pub const ALMMIN: u16 = 0x230;
    pub const ALMHR: u16 = 0x231;
    pub const ALMDAYL: u16 = 0x232;
    pub const ALMDAYH: u16 = 0x233;
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

    // Page 2 high (0x2000-0x2FFF)
    pub const KEY_BUFF_TOP: u16 = 0x2003;
    pub const KEY_BUFF_BOTTOM: u16 = 0x2004;
    pub const KEY_BUFFER: u16 = 0x2008;
}

/// Bank switch controller
pub struct BankSwitch {
    /// Physical address per bank (4 KiB pages)
    pub banks: [u32; 16],
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

/// Flash command state
#[derive(Clone, Copy, PartialEq)]
enum FlashCmd {
    Normal,
    ByteProgram,
    SoftwareId,
    CfiQuery,
}

/// Main memory bus
pub struct Memory {
    /// RAM (32 KiB)
    pub ram: Vec<u8>,
    /// Flash (2 MiB)
    pub flash: Vec<u8>,
    /// Font ROM (2 MiB, optional - loaded from 8.BIN)
    pub rom_8: Option<Vec<u8>>,
    /// OS ROM (2 MiB, optional - loaded from E.BIN, not needed for HLE)
    pub rom_e: Option<Vec<u8>>,
    /// Bank switch controller
    pub bank_switch: BankSwitch,
    /// Flash command state
    flash_cmd: FlashCmd,
    flash_cycles: u8,
    /// Timer counters
    timer_counters: [u32; 5],
}

impl Memory {
    pub fn new() -> Self {
        Self {
            ram: vec![0; RAM_SIZE],
            flash: vec![0xFF; FLASH_SIZE],
            rom_8: None,
            rom_e: None,
            bank_switch: BankSwitch::new(),
            flash_cmd: FlashCmd::Normal,
            flash_cycles: 0,
            timer_counters: [0; 5],
        }
    }

    /// Initialize memory with default bank mappings
    pub fn init(&mut self) {
        // Set default INCR register (auto-increment for all 4 channels)
        self.ram[registers::INCR as usize] = 0x0F;

        // Initialize bank 0x03 to point to OS ROM vector area
        // This is where interrupt vectors are stored
        self.bank_switch.set(0x03, 0x0E); // Maps to 0xE000 area
    }

    /// Load font ROM (8.BIN) - optional
    pub fn load_rom_8(&mut self, data: &[u8]) {
        let mut rom = vec![0u8; ROM8_SIZE];
        let len = data.len().min(ROM8_SIZE);
        rom[..len].copy_from_slice(&data[..len]);
        self.rom_8 = Some(rom);
    }

    /// Load OS ROM (E.BIN) - optional for HLE mode
    pub fn load_rom_e(&mut self, data: &[u8]) {
        let mut rom = vec![0u8; ROME_SIZE];
        let len = data.len().min(ROME_SIZE);
        rom[..len].copy_from_slice(&data[..len]);
        self.rom_e = Some(rom);
    }

    /// Read a byte from memory
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // Page 0 - hardware registers
            0x0000..=0x00FF => self.read_page0(addr),
            // Pages 1-15: direct RAM (0x0100-0x0FFF)
            0x0100..=0x0FFF => self.ram[addr as usize],
            // Pages 16-255: bank-switched (0x1000-0xFFFF)
            _ => {
                let paddr = self.bank_switch.translate(addr);
                self.read_physical(paddr)
            }
        }
    }

    /// Write a byte to memory
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x00FF => self.write_page0(addr, val),
            0x0100..=0x0FFF => {
                self.write_ram(addr, val);
            }
            _ => {
                let paddr = self.bank_switch.translate(addr);
                self.write_physical(paddr, val);
            }
        }
    }

    /// Write to RAM with special handling
    fn write_ram(&mut self, addr: u16, val: u8) {
        self.ram[addr as usize] = val;

        // Disable audio channels when writing to PB register
        if addr == registers::PB {
            self.ram[addr as usize] = 0;
        }

        // Prevent auto power off
        if addr == 0x2028 {
            self.ram[addr as usize] = 0xFF;
        }
    }

    /// Read a 16-bit value (little-endian)
    pub fn read16(&self, addr: u16) -> u16 {
        self.read(addr) as u16 | (self.read(addr.wrapping_add(1)) as u16) << 8
    }

    /// Read from page 0 hardware registers
    fn read_page0(&self, addr: u16) -> u8 {
        match addr {
            // Direct memory access registers
            registers::DATA1..=registers::DATA4 => {
                let channel = (addr - registers::DATA1) as usize;
                self.direct_read(channel)
            }
            // Bank switch registers
            registers::BK_SEL => self.bank_switch.selected(),
            registers::BK_ADRL => self.bank_switch.read_adrl(),
            registers::BK_ADRH => self.bank_switch.read_adrh(),
            // Other registers - read from RAM
            _ => self.ram[addr as usize],
        }
    }

    /// Write to page 0 hardware registers
    fn write_page0(&mut self, addr: u16, val: u8) {
        match addr {
            // Direct memory access registers
            registers::DATA1..=registers::DATA4 => {
                let channel = (addr - registers::DATA1) as usize;
                self.direct_write(channel, val);
            }
            // Interrupt status register - write to clear
            registers::ISR => {
                self.ram[addr as usize] &= val;
            }
            // Timer interrupt status register - write to clear
            registers::TISR => {
                self.ram[addr as usize] &= val;
            }
            // Bank switch select
            registers::BK_SEL => {
                self.bank_switch.set_selected(val);
            }
            // Bank switch address low
            registers::BK_ADRL => {
                self.bank_switch.write_adrl(val);
            }
            // Bank switch address high
            registers::BK_ADRH => {
                self.bank_switch.write_adrh(val);
            }
            // Other registers - write to RAM
            _ => {
                self.ram[addr as usize] = val;
            }
        }
    }

    /// Direct memory read via address registers
    fn direct_read(&self, channel: usize) -> u8 {
        // Calculate address from ADDR registers
        let base = registers::ADDR1L as usize + channel * 3;
        let addr_l = self.ram[base] as u32;
        let addr_m = self.ram[base + 1] as u32;
        let addr_h = self.ram[base + 2] as u32;
        let paddr = addr_l | (addr_m << 8) | (addr_h << 16);

        // Auto-increment if enabled
        let incr = self.ram[registers::INCR as usize];
        if (incr & (1 << channel)) != 0 {
            // Note: auto-increment happens on read, but we can't mutate in read
            // This is handled in the CPU step
        }

        self.read_physical(paddr)
    }

    /// Direct memory write via address registers
    fn direct_write(&mut self, channel: usize, val: u8) {
        // Calculate address from ADDR registers
        let base = registers::ADDR1L as usize + channel * 3;
        let addr_l = self.ram[base] as u32;
        let addr_m = self.ram[base + 1] as u32;
        let addr_h = self.ram[base + 2] as u32;
        let paddr = addr_l | (addr_m << 8) | (addr_h << 16);

        // Auto-increment if enabled
        let incr = self.ram[registers::INCR as usize];
        if (incr & (1 << channel)) != 0 {
            self.increment_address(channel);
        }

        self.write_physical(paddr, val);
    }

    /// Increment address register for a channel
    fn increment_address(&mut self, channel: usize) {
        let base = registers::ADDR1L as usize + channel * 3;

        // Increment low byte
        self.ram[base] = self.ram[base].wrapping_add(1);
        if self.ram[base] == 0 {
            // Carry to middle byte
            self.ram[base + 1] = self.ram[base + 1].wrapping_add(1);
            if self.ram[base + 1] == 0 {
                // Carry to high byte
                self.ram[base + 2] = self.ram[base + 2].wrapping_add(1);
            }
        }
    }

    /// Read from physical address
    pub fn read_physical(&self, addr: u32) -> u8 {
        if addr < 0x8000 {
            // RAM
            self.ram[addr as usize]
        } else if addr >= 0x200000 && addr < 0x400000 {
            // Flash
            self.read_flash(addr - 0x200000)
        } else if addr >= 0x800000 && addr < 0xA00000 {
            // Font ROM (8.BIN)
            if let Some(ref rom) = self.rom_8 {
                rom[(addr - 0x800000) as usize]
            } else {
                0x00
            }
        } else if addr >= 0xE00000 && addr < 0x1000000 {
            // OS ROM (E.BIN)
            if let Some(ref rom) = self.rom_e {
                rom[(addr - 0xE00000) as usize]
            } else {
                0x00
            }
        } else {
            0x00
        }
    }

    /// Write to physical address
    fn write_physical(&mut self, addr: u32, val: u8) {
        if addr < 0x8000 {
            // RAM
            self.ram[addr as usize] = val;
        } else if addr >= 0x200000 && addr < 0x400000 {
            // Flash
            self.write_flash(addr - 0x200000, val);
        }
        // ROM areas are read-only
    }

    /// Read from flash
    fn read_flash(&self, addr: u32) -> u8 {
        match self.flash_cmd {
            FlashCmd::Normal | FlashCmd::ByteProgram => {
                // For save area (last 32KiB), rotate to front
                // Otherwise, read directly
                let actual_addr = if addr >= FLASH_SIZE as u32 - 0x8000 {
                    // Save area: rotate last 32KiB to front
                    (addr + 0x8000) % FLASH_SIZE as u32
                } else {
                    addr
                };
                self.flash[actual_addr as usize]
            }
            FlashCmd::SoftwareId => {
                // Return software ID
                match addr {
                    0x00 => 0x51, // Manufacturer ID
                    0x01 => 0x52, // Device ID
                    _ => 0x00,
                }
            }
            FlashCmd::CfiQuery => {
                // Return CFI query data
                match addr {
                    0x10 => 0x51, // 'Q'
                    0x11 => 0x52, // 'R'
                    0x12 => 0x59, // 'Y'
                    0x27 => 0x19, // 2^25 = 32MB
                    _ => 0x00,
                }
            }
        }
    }

    /// Write to flash with AMD command sequence
    fn write_flash(&mut self, addr: u32, val: u8) {
        match self.flash_cycles {
            0 => {
                // 1st Bus Write Cycle
                if addr == 0x5555 && val == 0xAA {
                    self.flash_cycles += 1;
                } else if val == 0xF0 {
                    // Software ID Exit / CFI Exit
                    self.flash_cmd = FlashCmd::Normal;
                }
            }
            1 | 4 => {
                // 2nd Bus Write Cycle / 5th Bus Write Cycle
                if addr == 0x2AAA && val == 0x55 {
                    self.flash_cycles += 1;
                }
            }
            2 => {
                // 3rd Bus Write Cycle
                if addr != 0x5555 {
                    return;
                }
                match val {
                    0xA0 => {
                        // Byte-Program
                        self.flash_cmd = FlashCmd::ByteProgram;
                        self.flash_cycles += 1;
                    }
                    0x80 => {
                        self.flash_cycles += 1;
                    }
                    0x90 => {
                        // Software ID Entry
                        self.flash_cmd = FlashCmd::SoftwareId;
                        self.flash_cycles = 0;
                    }
                    0x98 => {
                        // CFI Query Entry
                        self.flash_cmd = FlashCmd::CfiQuery;
                        self.flash_cycles = 0;
                    }
                    0xF0 => {
                        // Software ID Exit / CFI Exit
                        self.flash_cmd = FlashCmd::Normal;
                        self.flash_cycles = 0;
                    }
                    _ => {}
                }
            }
            3 => {
                // 4th Bus Write Cycle
                if self.flash_cmd == FlashCmd::ByteProgram {
                    self.flash_cmd = FlashCmd::Normal;
                    self.flash_cycles = 0;
                    // Rotate last 32KiB to the front for save
                    let addr = (addr + 0x8000) % FLASH_SIZE as u32;
                    self.flash[addr as usize] = val;
                } else if addr == 0x5555 && val == 0xAA {
                    self.flash_cycles += 1;
                }
            }
            5 => {
                // 6th Bus Write Cycle
                match val {
                    0x10 => {
                        // Chip-Erase
                        if addr == 0x5555 {
                            self.flash.fill(0xFF);
                        }
                    }
                    0x30 => {
                        // Sector-Erase
                        let addr = (addr + 0x8000) % FLASH_SIZE as u32;
                        let sector = addr & 0x1FF000;
                        for i in 0..0x1000 {
                            self.flash[(sector + i) as usize] = 0xFF;
                        }
                    }
                    0x50 => {
                        // Block-Erase
                        let addr = ((addr & 0x1F0000) + 0x8000) % FLASH_SIZE as u32;
                        for i in 0..0x8000 {
                            self.flash[(addr + i) as usize] = 0xFF;
                        }
                        let addr2 = (addr + 0x8000) % FLASH_SIZE as u32;
                        for i in 0..0x8000 {
                            self.flash[(addr2 + i) as usize] = 0xFF;
                        }
                    }
                    _ => {}
                }
                self.flash_cmd = FlashCmd::Normal;
                self.flash_cycles = 0;
            }
            _ => {
                self.flash_cycles = 0;
            }
        }
    }

    /// Update timers (call once per frame)
    pub fn update_timers(&mut self) {
        // Update main timer counter
        self.ram[registers::MTCT as usize] =
            self.ram[registers::MTCT as usize].wrapping_add(1);

        // Check for timer interrupts
        let stcon = self.ram[registers::STCON as usize];
        let tier = self.ram[registers::TIER as usize];

        for i in 0..4 {
            if (stcon & (1 << i)) != 0 {
                self.timer_counters[i] += 1;
                if self.timer_counters[i] >= 0x100 {
                    self.timer_counters[i] = self.ram[registers::ST1LD as usize + i] as u32;
                    if (tier & (1 << i)) != 0 {
                        self.ram[registers::TISR as usize] |= 1 << i;
                        self.ram[registers::SYSCON as usize] &= 0xF7;
                    }
                }
            }
        }
    }

    /// Get RAM slice for save states
    pub fn ram(&self) -> &[u8] {
        &self.ram
    }

    /// Get mutable RAM slice
    pub fn ram_mut(&mut self) -> &mut [u8] {
        &mut self.ram
    }
}

impl Bus for Memory {
    fn get_byte(&mut self, address: u16) -> u8 {
        self.read(address)
    }

    fn set_byte(&mut self, address: u16, value: u8) {
        self.write(address, value);
    }
}
