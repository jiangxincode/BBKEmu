//! Main emulator orchestrator

use anyhow::Result;

use crate::audio::Audio;
use crate::cpu::CpuWrapper;
use crate::debug::Debugger;
use crate::gam::GamFile;
use crate::input::{BbkKey, Input};
use crate::lcd::Lcd;
use crate::memory::Memory;
use crate::model::{BbkModel, MODEL_4980};
use crate::save::SaveState;
use crate::syscall::SyscallTable;
use crate::syscalls;

/// Main emulator struct
pub struct Emulator {
    /// 6502 CPU (owns memory)
    pub cpu: CpuWrapper,
    /// LCD display
    pub lcd: Lcd,
    /// Input system
    pub input: Input,
    /// Audio system
    pub audio: Audio,
    /// System call table
    syscalls: SyscallTable,
    /// Debugger
    pub debug: Debugger,
    /// Current model
    model: &'static BbkModel,
    /// Whether the emulator is running
    running: bool,
    /// Frame counter
    frame_count: u64,
    /// CPU cycles accumulated toward the next 400-cycle timer tick.
    timer_cycle_remainder: u32,
    /// Return points for compiler-runtime far calls handled by HLE.
    hle_far_calls: Vec<HleFarCall>,
    /// Whether to use HLE syscall interception (intercept JSR to OS area)
    hle_syscalls: bool,
}

struct HleFarCall {
    return_pc: u16,
    banks: [u32; 4],
}

const HLE_FAR_RETURN: u16 = 0x02F0;

impl Emulator {
    /// Create a new emulator instance
    pub fn new(model: &'static BbkModel) -> Self {
        let audio = Audio::new(44100);
        let memory = Memory::new();
        let cpu = CpuWrapper::new(memory);
        let syscalls = syscalls::build_syscall_table();

        Self {
            cpu,
            lcd: Lcd::new(),
            input: Input::new(),
            audio,
            syscalls,
            debug: Debugger::new(),
            model,
            running: false,
            frame_count: 0,
            timer_cycle_remainder: 0,
            hle_far_calls: Vec::new(),
            hle_syscalls: false,
        }
    }

    /// Create emulator with default model (A4980)
    pub fn default() -> Self {
        Self::new(&MODEL_4980)
    }

    /// Enable HLE syscall interception
    pub fn set_hle_syscalls(&mut self, enabled: bool) {
        self.hle_syscalls = enabled;
    }

    /// Load a GAM file
    pub fn load_gam(&mut self, data: &[u8]) -> Result<()> {
        let gam = GamFile::parse(data)?;
        log::info!(
            "Loading game: {} (entry: 0x{:04X})",
            gam.name(),
            gam.entry_point
        );

        // Initialize memory
        self.cpu.memory_mut().init();
        let lle_mode = self.cpu.memory().rom_e.is_some();
        if lle_mode {
            self.cpu.reset();
            self.run_os_init();
        } else {
            // No OS ROM available - minimal init
            self.cpu.memory_mut().ram[0x200] = 0x00;
            self.cpu.memory_mut().ram[0x207] = 0x0F;
            self.cpu.memory_mut().ram[0x22B] = 0xFE;
            self.cpu.memory_mut().ram[0x28] = 0xD7;
            self.cpu.memory_mut().ram[0x29] = 0x17;
            self.hle_syscalls = true;
        }

        // Load game into flash at 0x20D000
        let flash_offset = 0xD000;
        let game_data = &gam.data;
        let end = (flash_offset + game_data.len()).min(self.cpu.memory().flash.len());
        self.cpu.memory_mut().flash[flash_offset..end]
            .copy_from_slice(&game_data[..end - flash_offset]);

        // Setup flash headers
        let sys_hdr = [
            0xC0u8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
            0x00, 0x2F,
        ];
        let mut gam_hdr = [0u8; 16];
        gam_hdr[0] = 0xD0;
        gam_hdr[1] = 0x00;
        gam_hdr[2..12].copy_from_slice(&gam.info);
        let size = game_data.len();
        gam_hdr[12] = (size & 0xFF) as u8;
        gam_hdr[13] = ((size >> 8) & 0xFF) as u8;
        gam_hdr[14] = ((size >> 16) & 0xFF) as u8;
        gam_hdr[15] = 0x3D;

        let flash_base = 0x8000;
        self.cpu.memory_mut().flash[flash_base..flash_base + 16].copy_from_slice(&sys_hdr);
        self.cpu.memory_mut().flash[flash_base + 16..flash_base + 32].copy_from_slice(&gam_hdr);

        // Setup bank mappings
        self.cpu.memory_mut().bank_switch.set(0x5, 0x20D);
        self.cpu.memory_mut().bank_switch.set(0x6, 0x20E);
        self.cpu.memory_mut().bank_switch.set(0x7, 0x20F);
        self.cpu.memory_mut().bank_switch.set(0x8, 0x210);

        let data_bank = 0x20D + (gam.data_offset >> 12);
        self.cpu.memory_mut().bank_switch.set(0x9, data_bank);
        self.cpu.memory_mut().bank_switch.set(0xA, data_bank + 1);
        self.cpu.memory_mut().bank_switch.set(0xB, data_bank + 2);
        self.cpu.memory_mut().bank_switch.set(0xC, data_bank + 3);

        // Setup OS ROM bank (bank 0xD -> OS ROM)
        // For 4980 model: 0x0EA8, for 4988: 0x0E88
        let os_bank = match self.model.bank_sys_d {
            0x0EA8 => 0x0EA8u32, // 4980
            0x0E88 => 0x0E88u32, // 4988
            _ => 0x0EA8u32,      // default to 4980
        };
        if !lle_mode {
            self.cpu.memory_mut().bank_switch.set(0xD, os_bank);
            self.cpu.memory_mut().bank_switch.set(0xE, os_bank + 1);
            self.cpu.memory_mut().bank_switch.set(0xF, os_bank + 2);
        }

        // Setup save area
        let save_base = 0x7000; // 4980
        self.cpu.memory_mut().flash[flash_base + save_base + 0xF8] = 0x02;
        self.cpu.memory_mut().flash[flash_base + save_base + 0xF9] = 0x02;
        self.cpu.memory_mut().flash[flash_base + save_base + 0xFA] = 0x02;
        self.cpu.memory_mut().flash[flash_base + save_base + 0xFB] = 0x02;
        self.cpu.memory_mut().flash[flash_base + save_base + 0xFC] = 0x02;
        self.cpu.memory_mut().flash[flash_base + save_base + 0xFD] = 0x02;
        self.cpu.memory_mut().flash[flash_base + save_base + 0xFE] = 0x03;
        self.cpu.memory_mut().flash[flash_base + save_base + 0xFF] = 0x02;

        // Set system control
        self.cpu.memory_mut().write(0x2029, 0x0D);
        self.cpu.memory_mut().write(0x202A, 0x02);

        if lle_mode {
            let sp = self.cpu.sp();
            self.cpu.memory_mut().ram[0x100 | sp as usize] = 0x02;
            self.cpu.memory_mut().ram[0x100 | sp.wrapping_sub(1) as usize] = 0x60;
            self.cpu.set_sp(sp.wrapping_sub(2));
        } else {
            self.cpu.reset();
        }
        self.cpu.set_pc(gam.entry_point);

        // Debug: check what's at the entry point
        let entry_opcode = self.cpu.memory().read(gam.entry_point);
        log::info!(
            "Entry point 0x{:04X} opcode: 0x{:02X}",
            gam.entry_point,
            entry_opcode
        );

        // Debug: check bank mapping
        let bank5 = self.cpu.memory().bank_switch.banks[5];
        log::info!("Bank 5 mapped to: 0x{:04X}", bank5);

        // Debug: check flash content at expected location
        let flash_offset = 0xD000 + (gam.entry_point & 0x0FFF) as usize;
        if flash_offset < self.cpu.memory().flash.len() {
            log::info!(
                "Flash[0x{:04X}] = 0x{:02X}",
                flash_offset,
                self.cpu.memory().flash[flash_offset]
            );
        }

        // Debug: check physical address translation
        let paddr = self.cpu.memory().bank_switch.translate(gam.entry_point);
        log::info!(
            "Physical address for 0x{:04X}: 0x{:08X}",
            gam.entry_point,
            paddr
        );

        // Debug: check read_physical
        let read_result = self.cpu.memory().read_physical(paddr);
        log::info!("read_physical(0x{:08X}) = 0x{:02X}", paddr, read_result);

        self.running = true;
        log::info!(
            "Game loaded, starting execution at 0x{:04X}",
            gam.entry_point
        );

        Ok(())
    }

    /// Load font ROM (8.BIN) - optional
    pub fn load_rom_8(&mut self, data: &[u8]) {
        self.cpu.memory_mut().load_rom_8(data);
        log::info!("Font ROM loaded ({} bytes)", data.len());
    }

    /// Load OS ROM (E.BIN) - optional, for LLE fallback
    pub fn load_rom_e(&mut self, data: &[u8]) {
        self.cpu.memory_mut().load_rom_e(data);
        log::info!("OS ROM loaded ({} bytes)", data.len());
    }

    /// Run OS initialization sequence
    /// This runs the OS code at 0x350 until _MTCT register becomes 0xFE
    pub fn run_os_init(&mut self) {
        log::info!("Running OS initialization...");

        // Set PC to OS entry point
        self.cpu.set_pc(0x350);

        // Run until _MTCT becomes 0xFE
        let mut max_cycles = 10_000_000; // Safety limit
        while max_cycles > 0 {
            let cycles = self.cpu.step();
            max_cycles -= cycles as i64;

            // Check if OS init is complete
            let mtct = self.cpu.memory().ram[0x22B]; // _MTCT register
            if mtct == 0xFE {
                log::info!("OS initialization complete");
                return;
            }
        }

        log::warn!("OS initialization timed out");
    }

    /// Handle a syscall directly
    fn handle_syscall(&mut self, target: u16) -> crate::syscall::SyscallResult {
        use crate::syscall::SyscallResult;

        match target {
            // LCD syscalls (0xE000-0xE01B)
            0xE000 => {
                // lcd_init
                self.lcd.clear();
                SyscallResult::handled()
            }
            0xE003 => {
                // lcd_clear
                self.lcd.clear();
                SyscallResult::handled()
            }
            0xE006 => {
                // lcd_pixel
                let x = self.cpu.x();
                let y = self.cpu.y();
                let color = self.cpu.a() != 0;
                self.lcd.set_pixel(x, y, color);
                SyscallResult::handled()
            }
            0xE009 => {
                // lcd_char - draw character at cursor position
                let ch = self.cpu.a();
                let x = self.lcd.cursor_x();
                let y = self.lcd.cursor_y();
                let font_bitmap = crate::font_data::get_font_bitmap(ch);
                for row in 0..8u8 {
                    let byte = font_bitmap[row as usize];
                    for bit in 0..8u8 {
                        if byte & (1 << (7 - bit)) != 0 {
                            self.lcd.set_pixel(x + bit, y + row, true);
                        }
                    }
                }
                self.lcd.set_cursor(x + 8, y);
                SyscallResult::handled()
            }
            0xE00C => {
                // lcd_string - draw string at cursor position
                let addr = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let mut x = self.lcd.cursor_x();
                let y = self.lcd.cursor_y();
                let mut offset = 0u16;
                loop {
                    let ch = self.cpu.memory().read(addr + offset);
                    if ch == 0 { break; }
                    let font_bitmap = crate::font_data::get_font_bitmap(ch);
                    for row in 0..8u8 {
                        let byte = font_bitmap[row as usize];
                        for bit in 0..8u8 {
                            if byte & (1 << (7 - bit)) != 0 {
                                self.lcd.set_pixel(x + bit, y + row, true);
                            }
                        }
                    }
                    x = x.wrapping_add(8);
                    offset += 1;
                    if offset > 255 { break; }
                }
                self.lcd.set_cursor(x, y);
                SyscallResult::handled()
            }
            0xE00F => {
                // lcd_cursor
                let x = self.cpu.x();
                let y = self.cpu.y();
                self.lcd.set_cursor(x, y);
                SyscallResult::handled()
            }
            0xE012 => {
                // lcd_rect
                let x = self.cpu.x();
                let y = self.cpu.y();
                let w = self.cpu.a();
                let h = self.cpu.memory().read(0x20);
                self.lcd.fill_rect(x, y, w, h, true);
                SyscallResult::handled()
            }
            0xE015 => {
                // lcd_line
                let x1 = self.cpu.x();
                let y1 = self.cpu.y();
                let x2 = self.cpu.a();
                let y2 = self.cpu.memory().read(0x20);
                let dx = (x2 as i16 - x1 as i16).abs();
                let dy = (y2 as i16 - y1 as i16).abs();
                let sx = if x1 < x2 { 1i16 } else { -1i16 };
                let sy = if y1 < y2 { 1i16 } else { -1i16 };
                let mut err = dx - dy;
                let mut x = x1 as i16;
                let mut y = y1 as i16;
                loop {
                    if x >= 0 && x < 159 && y >= 0 && y < 96 {
                        self.lcd.set_pixel(x as u8, y as u8, true);
                    }
                    if x == x2 as i16 && y == y2 as i16 { break; }
                    let e2 = 2 * err;
                    if e2 > -dy { err -= dy; x += sx; }
                    if e2 < dx { err += dx; y += sy; }
                }
                SyscallResult::handled()
            }
            0xE018 => {
                // lcd_scroll
                let lines = self.cpu.a();
                self.lcd.scroll_up(lines);
                SyscallResult::handled()
            }
            0xE01B => {
                // lcd_refresh
                SyscallResult::handled()
            }

            // Keyboard syscalls (0xE020-0xE029)
            0xE020 => {
                // key_get
                let key = self.input.get_key();
                SyscallResult::with_return(key)
            }
            0xE023 => {
                // key_hit
                let has_key = self.input.key_hit();
                SyscallResult::with_return(if has_key { 1 } else { 0 })
            }
            0xE026 => {
                // key_clear
                self.input.clear_buffer();
                SyscallResult::handled()
            }
            0xE029 => {
                // key_wait
                let key = self.input.get_key();
                SyscallResult::with_return(key)
            }

            // Audio syscalls (0xE030-0xE039)
            0xE030 => {
                // beep
                let freq = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let duration = self.cpu.a() as u16;
                if freq > 0 {
                    self.audio.play_tone(freq, duration * 10);
                }
                SyscallResult::handled()
            }
            0xE033 => {
                // sound_stop
                self.audio.stop();
                SyscallResult::handled()
            }
            0xE036 => {
                // music_play - not fully implemented
                SyscallResult::handled()
            }
            0xE039 => {
                // music_stop
                self.audio.stop();
                SyscallResult::handled()
            }

            // Timer syscalls (0xE040-0xE04C)
            0xE040 => {
                // timer_set
                let channel = self.cpu.a() as usize;
                let value = self.cpu.x();
                if channel < 4 {
                    self.cpu.memory_mut().ram[0x227 + channel] = value;
                    self.cpu.memory_mut().ram[0x226] |= 1 << channel;
                }
                SyscallResult::handled()
            }
            0xE043 => {
                // timer_get
                let channel = self.cpu.a() as usize;
                if channel < 4 {
                    SyscallResult::with_return(self.cpu.memory().ram[0x227 + channel])
                } else {
                    SyscallResult::with_return(0)
                }
            }
            0xE046 => {
                // rtc_read
                let field = self.cpu.a() as usize;
                let value = match field {
                    0 => self.cpu.memory().ram[0x234],
                    1 => self.cpu.memory().ram[0x235],
                    2 => self.cpu.memory().ram[0x236],
                    3 => self.cpu.memory().ram[0x237],
                    4 => self.cpu.memory().ram[0x238],
                    _ => 0,
                };
                SyscallResult::with_return(value)
            }
            0xE049 => {
                // rtc_write
                let field = self.cpu.a() as usize;
                let value = self.cpu.x();
                match field {
                    0 => self.cpu.memory_mut().ram[0x234] = value,
                    1 => self.cpu.memory_mut().ram[0x235] = value,
                    2 => self.cpu.memory_mut().ram[0x236] = value,
                    3 => self.cpu.memory_mut().ram[0x237] = value,
                    4 => self.cpu.memory_mut().ram[0x238] = value,
                    _ => {}
                }
                SyscallResult::handled()
            }
            0xE04C => {
                // delay
                let ms = self.cpu.a() as u32;
                SyscallResult { handled: true, return_value: None, cycles: ms * 4000 }
            }

            // String syscalls (0xE050-0xE05F)
            0xE050 => {
                // strlen
                let addr = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let mut len = 0u8;
                loop {
                    if self.cpu.memory().read(addr + len as u16) == 0 { break; }
                    len = len.wrapping_add(1);
                    if len == 0 { break; }
                }
                SyscallResult::with_return(len)
            }
            0xE053 => {
                // strcpy
                let dst = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let src = self.cpu.memory().read16(0x20);
                let mut i = 0u16;
                loop {
                    let byte = self.cpu.memory().read(src + i);
                    self.cpu.memory_mut().write(dst + i, byte);
                    if byte == 0 { break; }
                    i = i.wrapping_add(1);
                }
                SyscallResult::handled()
            }
            0xE056 => {
                // strcmp
                let str1 = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let str2 = self.cpu.memory().read16(0x20);
                let mut i = 0u16;
                loop {
                    let c1 = self.cpu.memory().read(str1 + i);
                    let c2 = self.cpu.memory().read(str2 + i);
                    if c1 != c2 {
                        return SyscallResult::with_return(if c1 < c2 { 0xFF } else { 0x01 });
                    }
                    if c1 == 0 { return SyscallResult::with_return(0); }
                    i = i.wrapping_add(1);
                }
            }
            0xE059 => {
                // strcat
                let dst = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let src = self.cpu.memory().read16(0x20);
                let mut dst_end = 0u16;
                loop {
                    if self.cpu.memory().read(dst + dst_end) == 0 { break; }
                    dst_end = dst_end.wrapping_add(1);
                }
                let mut i = 0u16;
                loop {
                    let byte = self.cpu.memory().read(src + i);
                    self.cpu.memory_mut().write(dst + dst_end + i, byte);
                    if byte == 0 { break; }
                    i = i.wrapping_add(1);
                }
                SyscallResult::handled()
            }
            0xE05C => {
                // memcpy
                let dst = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let src = self.cpu.memory().read16(0x20);
                let len = self.cpu.a() as u16;
                for i in 0..len {
                    let byte = self.cpu.memory().read(src + i);
                    self.cpu.memory_mut().write(dst + i, byte);
                }
                SyscallResult::handled()
            }
            0xE05F => {
                // memset
                let dst = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let value = self.cpu.a();
                let len = self.cpu.memory().read(0x20) as u16;
                for i in 0..len {
                    self.cpu.memory_mut().write(dst + i, value);
                }
                SyscallResult::handled()
            }

            // File syscalls (0xE060-0xE06C)
            0xE060 => {
                // file_open - return error for HLE
                SyscallResult::with_return(0xFF)
            }
            0xE063 => {
                // file_read - return 0 bytes read
                SyscallResult::with_return(0)
            }
            0xE066 => {
                // file_write - return 0 bytes written
                SyscallResult::with_return(0)
            }
            0xE069 => {
                // file_close - return success
                SyscallResult::handled()
            }
            0xE06C => {
                // file_delete - return success for HLE
                SyscallResult::with_return(0)
            }

            // System syscalls (0xE070-0xE079)
            0xE070 => {
                // sys_init
                self.lcd.clear();
                self.input.clear_buffer();
                self.audio.stop();
                SyscallResult::handled()
            }
            0xE073 => {
                // power_off
                log::info!("Power off requested");
                SyscallResult::handled()
            }
            0xE076 => {
                // sys_info - return model 4980
                SyscallResult::with_return(0)
            }
            0xE079 => {
                // random
                let seed = self.cpu.memory().ram[0x2000] as u16;
                let seed = seed.wrapping_mul(25173).wrapping_add(13849);
                self.cpu.memory_mut().ram[0x2000] = (seed >> 8) as u8;
                SyscallResult::with_return((seed & 0xFF) as u8)
            }
            0x0260 => {
                // brk_exit
                log::info!("Game exited via BRK");
                self.running = false;
                SyscallResult::handled()
            }

            // BBK OS functions (pure HLE mode - no OS ROM)
            0xD2F6 => {
                // D2F6 far-call dispatcher
                // Pop 2 bytes from software stack (matching OS ROM behavior)
                let stack = self.cpu.memory().read16(0x28).wrapping_add(2);
                self.cpu.memory_mut().write(0x28, stack as u8);
                self.cpu.memory_mut().write(0x29, (stack >> 8) as u8);
                self.handle_d2f6()
            }

            // E8F8 - bank switch setup (just return in HLE mode)
            0xE8F8 => SyscallResult::handled(),
            // E8FB - segment switch (just return in HLE mode)
            0xE8FB => SyscallResult::handled(),
            // E8FE - bank restore (just return in HLE mode)
            0xE8FE => SyscallResult::handled(),
            // D572 - function pointer call (just return in HLE mode)
            0xD572 => SyscallResult::handled(),

            // LCD drawing functions
            0xD300 => {
                // lcd_clear_area
                for i in 0x0400..0x1000 {
                    self.cpu.memory_mut().ram[i] = 0;
                }
                SyscallResult::handled()
            }
            0xD320 => {
                // lcd_hline
                let y = self.cpu.a();
                let x1 = self.cpu.x();
                let x2 = self.cpu.y();
                for x in x1..=x2 {
                    self.lcd.set_pixel(x, y, true);
                }
                SyscallResult::handled()
            }
            0xD340 => {
                // lcd_draw_block
                let src = self.cpu.memory().ram[0x20] as u16 | (self.cpu.memory().ram[0x21] as u16) << 8;
                let dst = self.cpu.memory().ram[0x26] as u16 | (self.cpu.memory().ram[0x27] as u16) << 8;
                for i in 0..32 {
                    let byte = self.cpu.memory().read(src + i);
                    if byte == 0 { break; }
                    if dst + i >= 0x0400 && dst + i < 0x1000 {
                        self.cpu.memory_mut().ram[(dst + i) as usize] = byte;
                    }
                }
                SyscallResult::handled()
            }
            0xD360 => {
                // lcd_vline
                let x = self.cpu.a();
                let y1 = self.cpu.x();
                let y2 = self.cpu.y();
                for y in y1..=y2 {
                    self.lcd.set_pixel(x, y, true);
                }
                SyscallResult::handled()
            }
            0xD380 => {
                // lcd_fill_rect
                let x = self.cpu.memory().ram[0x20];
                let y = self.cpu.memory().ram[0x21];
                let w = self.cpu.memory().ram[0x22];
                let h = self.cpu.memory().ram[0x23];
                self.lcd.fill_rect(x, y, w, h, true);
                SyscallResult::handled()
            }

            // Keyboard functions
            0xD3A0 => {
                // os_key_get
                let key = self.input.get_key();
                SyscallResult::with_return(key)
            }
            0xD3C0 => {
                // os_key_hit
                let has_key = self.input.key_hit();
                SyscallResult::with_return(if has_key { 1 } else { 0 })
            }

            // Audio functions
            0xD400 => {
                // os_beep
                let freq = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let duration = self.cpu.a() as u16;
                if freq > 0 {
                    self.audio.play_tone(freq, duration * 10);
                }
                SyscallResult::handled()
            }

            // Timer functions
            0xD420 => {
                // os_delay
                let ms = self.cpu.a() as u32;
                SyscallResult { handled: true, return_value: None, cycles: ms * 4000 }
            }

            // Drawing helper functions
            0xD596 => {
                // draw_offset_8
                let addr = self.cpu.memory().read16(0x2A);
                let result = addr.wrapping_add(8);
                self.cpu.memory_mut().write(0x20, result as u8);
                self.cpu.memory_mut().write(0x21, (result >> 8) as u8);
                SyscallResult::handled()
            }
            0xD5A6 => {
                // draw_offset_20
                let addr = self.cpu.memory().read16(0x2A);
                let result = addr.wrapping_add(0x20);
                self.cpu.memory_mut().write(0x23, result as u8);
                self.cpu.memory_mut().write(0x24, (result >> 8) as u8);
                SyscallResult::handled()
            }
            0xD5B6 => {
                // draw_offset_18
                let addr = self.cpu.memory().read16(0x2A);
                let result = addr.wrapping_add(0x18);
                self.cpu.memory_mut().write(0x23, result as u8);
                self.cpu.memory_mut().write(0x24, (result >> 8) as u8);
                SyscallResult::handled()
            }

            // Bitmap operations
            0xD2CA => {
                // bitmap_and
                let src1 = self.cpu.memory().read16(0x20);
                let src2 = self.cpu.memory().read16(0x23);
                let dst = self.cpu.memory().read16(0x2A);
                for i in 0..4 {
                    let b1 = self.cpu.memory().read(src1 + i);
                    let b2 = self.cpu.memory().read(src2 + i);
                    self.cpu.memory_mut().write(dst + 8 + i, b1 & b2);
                }
                let addr = self.cpu.memory().read16(0x2A);
                let result = addr.wrapping_add(8);
                self.cpu.memory_mut().write(0x20, result as u8);
                self.cpu.memory_mut().write(0x21, (result >> 8) as u8);
                SyscallResult::handled()
            }

            // Compare/multiply functions
            0xD362 => {
                // compare16
                let val1 = self.cpu.memory().read16(0x20);
                let val2 = self.cpu.memory().read16(0x23);
                SyscallResult::with_return(if val1 == val2 { 0 } else { 1 })
            }
            0xD1A2 => {
                // multiply16
                let a = self.cpu.memory().read16(0x20);
                let b = self.cpu.memory().read16(0x23);
                let result = (a as u32).wrapping_mul(b as u32);
                self.cpu.memory_mut().write(0x26, result as u8);
                self.cpu.memory_mut().write(0x27, (result >> 8) as u8);
                SyscallResult::handled()
            }

            // Stack operations
            0xDAAA => {
                // push_a
                let stack = self.cpu.memory().read16(0x28).wrapping_sub(1);
                let value = self.cpu.a();
                self.cpu.memory_mut().write(0x28, stack as u8);
                self.cpu.memory_mut().write(0x29, (stack >> 8) as u8);
                self.cpu.memory_mut().write(stack, value);
                SyscallResult::handled()
            }
            0xDACA => {
                // set_cursor_stack
                let stack = self.cpu.memory().read16(0x28).wrapping_sub(2);
                let lo = self.cpu.memory().read(0x20);
                let hi = self.cpu.memory().read(0x21);
                log::info!("DACA: stack=0x{:04X} [0x20:0x21]=0x{:02X}{:02X}", stack, hi, lo);
                self.cpu.memory_mut().write(0x28, stack as u8);
                self.cpu.memory_mut().write(0x29, (stack >> 8) as u8);
                self.cpu.memory_mut().write(stack, lo);
                self.cpu.memory_mut().write(stack.wrapping_add(1), hi);
                SyscallResult::handled()
            }
            0xDAE6 => {
                // copy_16
                let src = self.cpu.memory().read16(0x23);
                let dst = self.cpu.memory().read16(0x20);
                for i in 0..2 {
                    let byte = self.cpu.memory().read(src + i);
                    self.cpu.memory_mut().write(dst + i, byte);
                }
                SyscallResult::handled()
            }
            0xDBE1 => {
                // test_not_zero
                let val = self.cpu.memory().read16(0x23);
                SyscallResult::with_return(if val == 0 { 0 } else { 1 })
            }
            0xDCEF => {
                // push_4_bytes
                let addr = self.cpu.memory().read16(0x20);
                let sp = self.cpu.sp();
                for i in 0..4 {
                    let byte = self.cpu.memory().read(addr + i);
                    self.cpu.memory_mut().ram[0x100 | sp.wrapping_sub(i as u8) as usize] = byte;
                }
                self.cpu.set_sp(sp.wrapping_sub(4));
                SyscallResult::handled()
            }

            // Segment dispatch functions
            0xE8F8 => {
                // Segment dispatch - bank switch setup
                // Read segment from A register and setup bank mapping
                let segment = self.cpu.a();
                let data_base = self.cpu.memory().ram[0x2029] as u32 | (self.cpu.memory().ram[0x202A] as u32) << 8;
                let base = data_base + (segment as u32) * 4;
                for i in 0..4 {
                    self.cpu.memory_mut().bank_switch.set(5 + i, base + i as u32);
                }
                SyscallResult::handled()
            }
            0xE8FB => {
                // Segment dispatch - execute function in segment
                SyscallResult::handled()
            }
            0xE8FE => {
                // Segment dispatch - restore bank mapping
                SyscallResult::handled()
            }

            // Other helper functions
            0xE901 | 0xE904 | 0xE907 | 0xE90A => {
                SyscallResult::handled()
            }

            // Drawing helpers
            0xD6AF | 0xDC80 | 0xD85F | 0xDD38 | 0xDCB1 | 0xD29D | 0xDB19 |
            0xDCF7 | 0xDC83 | 0xD201 | 0xD0A8 | 0xDCAB | 0xDCF4 => {
                SyscallResult::handled()
            }

            // Function pointer call
            0xD572 => {
                let addr = self.cpu.memory().read16(0x26);
                let target = addr.wrapping_sub(1);
                self.cpu.memory_mut().write(0x26, target as u8);
                self.cpu.memory_mut().write(0x27, (target >> 8) as u8);
                let pc = self.cpu.pc();
                let sp = self.cpu.sp();
                let ret = pc.wrapping_sub(1);
                self.cpu.memory_mut().ram[0x100 | sp as usize] = (ret >> 8) as u8;
                self.cpu.memory_mut().ram[0x100 | sp.wrapping_sub(1) as usize] = ret as u8;
                self.cpu.set_sp(sp.wrapping_sub(2));
                self.cpu.set_pc(target);
                SyscallResult::handled()
            }

            _ => {
                // Unknown syscall - log it for debugging
                log::info!("Unknown syscall at 0x{:04X}", target);
                SyscallResult::not_handled()
            }
        }
    }

    /// Handle D2F6 far-call dispatcher
    fn handle_d2f6(&mut self) -> crate::syscall::SyscallResult {
        use crate::syscall::SyscallResult;

        let a_val = self.cpu.a();
        let descriptor_addr =
            self.cpu.memory().ram[0x26] as u16 | (self.cpu.memory().ram[0x27] as u16) << 8;

        // Try to get descriptor from hardcoded table first
        let descriptor = self.hle_descriptor(descriptor_addr);
        let (target, segment) = descriptor.unwrap_or_else(|| {
            let target = self.cpu.memory().read16(descriptor_addr);
            let segment = self.cpu.memory().read(descriptor_addr.wrapping_add(2));
            (target, segment)
        });

        log::debug!(
            "D2F6: desc=0x{descriptor_addr:04X} tgt=0x{target:04X} seg=0x{segment:02X} A=0x{a_val:02X}"
        );

        // For segments >= 0xE0, setup bank switching and execute
        if segment >= 0xE0 {
            let data_base = self.cpu.memory().ram[0x2029] as u32 | (self.cpu.memory().ram[0x202A] as u32) << 8;
            let base = data_base + ((segment - 0xE0) as u32) * 4;
            for i in 0..4 {
                self.cpu.memory_mut().bank_switch.set(5 + i, base + i as u32);
            }
        }

        // Dispatch based on segment and target
        match segment {
            0x04 => {
                // Utility functions - return success
                self.cpu.set_a(0);
            }
            0x05 => {
                // System functions - return success
                self.cpu.set_a(0);
            }
            0x06 => {
                // String/Math functions
                match target {
                    0x624D => {
                        // strlen
                        let addr = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                        let mut len = 0u8;
                        loop {
                            if self.cpu.memory().read(addr + len as u16) == 0 {
                                break;
                            }
                            len = len.wrapping_add(1);
                            if len == 0 { break; }
                        }
                        self.cpu.set_a(len);
                    }
                    _ => {
                        log::debug!("Unknown segment0x06 function 0x{target:04X} in D2F6");
                    }
                }
            }
            0x07 => {
                // LCD/Graphics functions
                match target {
                    0x7770 => {
                        // lcd_draw_char
                        let stack_ptr = self.cpu.memory().read16(0x28);
                        let pos = self.cpu.memory().read16(stack_ptr);
                        let fb_addr = if pos >= 0x0400 && pos < 0x1000 {
                            (pos - 0x0400) as usize
                        } else {
                            0
                        };
                        log::trace!(
                            "lcd_draw_char: ch=0x{:02X} pos=0x{:04X} fb=0x{:04X} sp=0x{:04X}",
                            a_val, pos, fb_addr, stack_ptr
                        );
                        // Try to read font from ROM first, fallback to built-in
                        let font_bitmap = if let Some(ref rom) = self.cpu.memory().rom_8 {
                            // Font ROM: each character is 8 bytes
                            let offset = (a_val as usize) * 8;
                            if offset + 8 <= rom.len() {
                                let mut bitmap = [0u8; 8];
                                bitmap.copy_from_slice(&rom[offset..offset+8]);
                                bitmap
                            } else {
                                crate::font_data::get_font_bitmap(a_val)
                            }
                        } else {
                            crate::font_data::get_font_bitmap(a_val)
                        };
                        for row in 0..8 {
                            let offset = fb_addr + row * 20;
                            if offset < 0x0C00 {
                                self.cpu.memory_mut().ram[0x0400 + offset] = font_bitmap[row];
                            }
                        }
                    }
                    0x7716 => {
                        // volume_set
                        if a_val == 0 {
                            self.cpu.memory_mut().ram[0x201B] &= 0xFB;
                        } else {
                            self.cpu.memory_mut().ram[0x201B] |= 0x04;
                        }
                    }
                    0x772E => {
                        // volume_get
                        let muted = self.cpu.memory().ram[0x201B] & 0x04;
                        self.cpu.set_a(if muted != 0 { 1 } else { 0 });
                    }
                    0x759F => {
                        // rtc_get_sec
                        let sec = self.cpu.memory().ram[0x234] & 0x3F;
                        self.cpu.set_a(sec);
                    }
                    0x7CE5 => {
                        // lcd_enable
                        let lcd_active = self.cpu.memory().ram[0x2059];
                        if lcd_active != 0 {
                            self.cpu.memory_mut().ram[0x021A] = 0xDF;
                            self.cpu.memory_mut().ram[0x0219] = 0x20;
                            self.cpu.memory_mut().ram[0x0218] = 0xFF;
                            self.cpu.memory_mut().ram[0x021D] = 0xEB;
                            self.cpu.memory_mut().ram[0x021C] = 0x14;
                            self.cpu.memory_mut().ram[0x021B] |= 0x3F;
                            self.cpu.memory_mut().ram[0x021B] &= 0xDF;
                            self.cpu.memory_mut().ram[0x021B] &= 0xF7;
                            self.cpu.memory_mut().ram[0x021B] |= 0x01;
                            self.cpu.memory_mut().ram[0x021B] |= 0x1F;
                            self.cpu.memory_mut().ram[0x021B] |= 0x02;
                        }
                        self.cpu.set_a(0);
                    }
                    _ => {
                        log::debug!("Unknown segment0x07 function 0x{target:04X} in D2F6");
                    }
                }
            }
            0x08 => {
                // File/System functions
                const GAME_DATA_FLASH: u32 = 0xD000;
                match target {
                    0x5000 => {
                        // file_open - open game data file
                        // A = mode, [0x20:0x21] = filename pointer
                        // Return file handle 0 and initialize file state
                        self.cpu.memory_mut().ram[0x2060] = 0x00; // file position low
                        self.cpu.memory_mut().ram[0x2061] = 0x00; // file position high
                        self.cpu.memory_mut().ram[0x2062] = 0x00; // file size low
                        self.cpu.memory_mut().ram[0x2063] = 0x80; // file size high (=32KB)
                        self.cpu.set_a(0x00); // handle 0
                    }
                    0x5093 => {
                        // file_read - read from game data file
                        // A = handle, X/Y = buffer address, [0x20:0x21] = count
                        // Returns bytes read in A (low byte), but also stores
                        // total bytes read at [0x20:0x21] for the caller.
                        let buf_addr = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                        let count = self.cpu.memory().read16(0x20);
                        let pos_lo = self.cpu.memory().ram[0x2060];
                        let pos_hi = self.cpu.memory().ram[0x2061];
                        let file_pos = pos_lo as u32 | (pos_hi as u32) << 8;
                        let flash_addr = GAME_DATA_FLASH + file_pos;
                        log::info!("file_read: buf=0x{:04X} count={} pos={} flash=0x{:05X}", buf_addr, count, file_pos, flash_addr);
                        let mut bytes_read = 0u16;
                        for i in 0..count {
                            let byte = self.cpu.memory().read_physical(flash_addr + i as u32);
                            self.cpu.memory_mut().write(buf_addr + i, byte);
                            bytes_read += 1;
                        }
                        let new_pos = file_pos + bytes_read as u32;
                        self.cpu.memory_mut().ram[0x2060] = new_pos as u8;
                        self.cpu.memory_mut().ram[0x2061] = (new_pos >> 8) as u8;
                        // Store bytes read in [0x20:0x21] for the caller
                        self.cpu.memory_mut().write(0x20, bytes_read as u8);
                        self.cpu.memory_mut().write(0x21, (bytes_read >> 8) as u8);
                        // Return low byte of bytes_read (matching OS behavior)
                        log::info!("file_read: bytes_read={} return={}", bytes_read, bytes_read as u8);
                        self.cpu.set_a(bytes_read as u8);
                    }
                    0x5D45 => {
                        // file_write
                        self.cpu.set_a(0);
                    }
                    0x5FF3 => {
                        // file_close
                        self.cpu.set_a(1); // non-zero = success
                        self.cpu.inner.registers.index_y = 1;
                    }
                    0x6111 => {
                        // file_delete - return 1 for success (game checks A != 0)
                        log::debug!("file_delete: returning A=1");
                        self.cpu.set_a(1);
                        self.cpu.inner.registers.index_y = 1;
                    }
                    0x6A79 => {
                        // file_eof
                        let pos_lo = self.cpu.memory().ram[0x2060];
                        let pos_hi = self.cpu.memory().ram[0x2061];
                        let file_pos = pos_lo as u32 | (pos_hi as u32) << 8;
                        let game_size = 0x8000u32;
                        self.cpu.set_a(if file_pos >= game_size { 1 } else { 0 });
                    }
                    0x7093 => {
                        // file_tell
                        let pos_lo = self.cpu.memory().ram[0x2060];
                        self.cpu.set_a(pos_lo);
                    }
                    0x7D92 => {
                        // file_seek
                        let offset = self.cpu.x() as u32 | (self.cpu.y() as u32) << 8;
                        let whence = self.cpu.memory().ram[0x20];
                        let pos_lo = self.cpu.memory().ram[0x2060];
                        let pos_hi = self.cpu.memory().ram[0x2061];
                        let cur_pos = pos_lo as u32 | (pos_hi as u32) << 8;
                        let new_pos = match whence {
                            0 => offset,
                            1 => cur_pos.wrapping_add(offset),
                            2 => 0x8000u32.wrapping_sub(offset),
                            _ => offset,
                        };
                        self.cpu.memory_mut().ram[0x2060] = new_pos as u8;
                        self.cpu.memory_mut().ram[0x2061] = (new_pos >> 8) as u8;
                        self.cpu.set_a(0);
                    }
                    _ => {
                        log::debug!("Unknown segment0x08 function 0x{target:04X} in D2F6");
                    }
                }
            }
            0x0D => {
                // Initialization functions
                match target {
                    0x6911 => {
                        self.cpu.memory_mut().ram[0x216A] = 0;
                        for i in 0..4 {
                            let offset = i * 0x0C;
                            self.cpu.memory_mut().ram[0x2122 + offset] = 0;
                        }
                        self.cpu.set_a(0);
                    }
                    _ => {
                        log::debug!("Unknown segment0x0D function 0x{target:04X} in D2F6");
                    }
                }
            }
            _ => {
                // Unknown segment - return success for HLE
                log::debug!("Unknown segment 0x{segment:02X} in D2F6 descriptor");
                self.cpu.set_a(0);
            }
        }

        SyscallResult::handled()
    }

    /// Run one frame (~16.67ms at 60fps)
    pub fn run_frame(&mut self) {
        // BBK runs at ~4MHz, 60fps = ~66666 cycles per frame
        let cycles_per_frame = 66666u32;
        let mut cycles_run = 0u32;

        while cycles_run < cycles_per_frame && self.running {
            let halted = self.cpu.memory().ram[0x200] & 0x08 != 0;
            let cycles = if halted { 400 } else { self.step() };
            cycles_run += cycles;
            self.timer_cycle_remainder += cycles;
            let ticks = self.timer_cycle_remainder / 400;
            if ticks > 0 {
                self.cpu.memory_mut().update_timers(ticks);
                self.timer_cycle_remainder %= 400;
            }
        }

        self.frame_count += 1;
    }

    /// Execute a single CPU step
    fn step(&mut self) -> u32 {
        let pc = self.cpu.pc();

        if pc == HLE_FAR_RETURN {
            if let Some(call) = self.hle_far_calls.pop() {
                for (index, bank) in call.banks.into_iter().enumerate() {
                    self.cpu.memory_mut().bank_switch.set(5 + index as u8, bank);
                }
                self.cpu.set_pc(call.return_pc);
                return 1;
            }
        }

        // Check for breakpoint
        if self.debug.has_breakpoint(pc) {
            log::info!("Breakpoint hit at 0x{:04X}", pc);
            self.debug.set_stepping(true);
        }

        // Check for BRK instruction (game exit)
        let opcode = self.cpu.memory().read(pc);
        if opcode == 0x00 {
            // Log the return address on stack for debugging
            let sp = self.cpu.sp();
            let ret_lo = self.cpu.memory().read(0x100 | (sp.wrapping_add(1) as u16));
            let ret_hi = self.cpu.memory().read(0x100 | (sp.wrapping_add(2) as u16));
            let ret_addr = ret_lo as u16 | (ret_hi as u16) << 8;
            log::info!(
                "BRK at 0x{:04X} SP=0x{:02X} ret=0x{:04X}, game exiting",
                pc, sp, ret_addr
            );
            self.running = false;
            return 1;
        }

        // Check for JSR instruction - potential syscall
        if opcode == 0x20 {
            let target = self.cpu.memory().read16(pc + 1);
            if target >= 0xD000 {
                let ram = &self.cpu.memory().ram;
                log::info!(
                    "OS call caller=0x{pc:04X} target=0x{target:04X} A={:02X} X={:02X} Y={:02X}",
                    self.cpu.a(),
                    self.cpu.x(),
                    self.cpu.y()
                );
            }

            // Log D2F6 calls for debugging
            if target == 0xD2F6 {
                let sp28 = self.cpu.memory().read16(0x28);
                log::info!("D2F6: [0x28]=0x{:04X} A=0x{:02X}", sp28, self.cpu.a());
            }

            // In HLE mode, only intercept syscalls in the SyscallTable.
            // Let D2F6 and other OS functions execute from OS ROM.
            if self.hle_syscalls && self.syscalls.is_syscall(target) {
                let result = self.handle_syscall(target);

                if result.handled {
                    self.cpu.set_pc(pc + 3);
                    if let Some(val) = result.return_value {
                        self.cpu.set_a(val);
                    }
                    return 3;
                }
            }
            // Other JSR calls execute normally
        }

        // Execute the instruction normally
        let cycles = self.cpu.step();

        // Handle interrupts
        self.handle_interrupts();

        cycles
    }


    fn hle_descriptor(&self, address: u16) -> Option<(u16, u8)> {
        const MODEL_4988: &[(u16, u16, u8)] = &[
            (0xE700, 0x5000, 0x04),
            (0xE78E, 0x67E2, 0x05),
            (0xE7B2, 0x7770, 0x07),
            (0xE7C4, 0x7716, 0x07),
            (0xE7C7, 0x772E, 0x07),
            (0xE7D9, 0x759F, 0x07),
            (0xE8EC, 0x624D, 0x06),
            (0xE932, 0x5000, 0x08),
            (0xE935, 0x5093, 0x08),
            (0xE938, 0x5D45, 0x08),
            (0xE93B, 0x5FF3, 0x08),
            (0xE93E, 0x6111, 0x08),
            (0xE947, 0x8515, 0x08),
            (0xE94A, 0x88DD, 0x08),
            (0xE944, 0x7D92, 0x08),
            (0xE95C, 0x7093, 0x08),
            (0xE965, 0x6A79, 0x08),
            (0xE9C2, 0x6911, 0x0D),
            (0xEA52, 0x7CE5, 0x07),
        ];
        if let Some((_, target, segment)) = MODEL_4988.iter().find(|entry| entry.0 == address) {
            return Some((*target, *segment));
        }

        let target = self.cpu.memory().read16(address);
        let segment = self.cpu.memory().read(address.wrapping_add(2));
        (target != 0).then_some((target, segment))
    }

    /// Handle pending interrupts
    fn handle_interrupts(&mut self) {
        let isr = self.cpu.memory().ram[0x04]; // ISR register
        let ier = self.cpu.memory().ram[0x23A]; // IER register
        let tisr = self.cpu.memory().ram[0x05]; // TISR register
        let tier = self.cpu.memory().ram[0x23B]; // TIER register
        let status = self.cpu.status();

        // Check if interrupts are disabled
        if status & 0x04 != 0 {
            return;
        }

        // Check for keyboard interrupt (PI)
        if (isr & 0x80) != 0 && (ier & 0x80) != 0 {
            self.cpu.memory_mut().ram[0x04] &= 0x7F; // Clear PI flag
            return;
        }

        // Check for timer interrupts
        if (tisr & 0x01) != 0 && (tier & 0x01) != 0 {
            let ram = &mut self.cpu.memory_mut().ram;
            ram[0x05] &= 0xFE;
            ram[0x2018] = ram[0x2018].wrapping_add(1);
            if ram[0x2018] >= ram[0x2019] {
                ram[0x201E] |= 0x01;
                ram[0x2018] = 0;
            }
            return;
        }
        if (tisr & 0x02) != 0 && (tier & 0x02) != 0 {
            self.trigger_interrupt(0x04); // ST2
            return;
        }
        if (tisr & 0x04) != 0 && (tier & 0x04) != 0 {
            self.trigger_interrupt(0x05); // ST3
            return;
        }
        if (tisr & 0x08) != 0 && (tier & 0x08) != 0 {
            self.trigger_interrupt(0x06); // ST4
            return;
        }

        // Check for alarm interrupt
        if (isr & 0x01) != 0 && (ier & 0x01) != 0 {
            self.trigger_interrupt(0x13); // ALM
            return;
        }

        // Check for counter interrupt
        if (isr & 0x02) != 0 && (ier & 0x02) != 0 {
            self.trigger_interrupt(0x12); // CT
            return;
        }
    }

    /// Trigger an interrupt
    fn trigger_interrupt(&mut self, vector_idx: u8) {
        // Push PC and status to stack
        let pc = self.cpu.pc();
        let status = self.cpu.status();
        let sp = self.cpu.sp();

        // Write to stack
        self.cpu.memory_mut().ram[0x100 | sp as usize] = (pc >> 8) as u8;
        self.cpu.memory_mut().ram[0x100 | sp.wrapping_sub(1) as usize] = (pc & 0xFF) as u8;
        self.cpu.memory_mut().ram[0x100 | sp.wrapping_sub(2) as usize] = status;
        self.cpu.set_sp(sp.wrapping_sub(3));

        // Each vector slot contains an executable ROM jump stub.
        self.cpu.set_pc(0x0300 + (vector_idx as u16) * 4);
    }

    /// Handle key down event
    pub fn key_down(&mut self, key: BbkKey) {
        let code = key as u8;
        let ram = &mut self.cpu.memory_mut().ram;
        ram[0x200] &= 0xF7;
        ram[0x24E] = code | 0x80;
        ram[0x04] |= 0x80;
        if ram[0x23A] & 0x80 != 0 {
            ram[0x2003] = 0;
            ram[0x2004] = 0x0F;
            ram[0x2017] = code & 0x3F;
            ram[0x24E] = 0;
        }
        self.input.key_down(key, self.frame_count * 16);
    }

    /// Handle key up event
    pub fn key_up(&mut self) {
        self.cpu.memory_mut().ram[0x24E] = 0;
        self.input.key_up();
    }

    /// Get the LCD framebuffer
    pub fn get_framebuffer(&self) -> &[bool; 159 * 96] {
        // TODO: Return actual framebuffer from RAM
        static FB: [bool; 159 * 96] = [false; 159 * 96];
        &FB
    }

    /// Render LCD to boolean buffer (true = foreground, false = background)
    pub fn render_lcd_buffer(&mut self) -> [bool; 159 * 96] {
        let mut pixels = [false; 159 * 96];

        self.cpu.memory_mut().ram[0x400] = self.cpu.memory().ram[0x1000];
        let ram = &self.cpu.memory().ram;
        let mut v = 0x400;

        for j in (-30i32..=65).rev() {
            let y = if j >= 0 { j } else { -j + 65 } as usize;
            for i in 1..20 {
                decode_lcd_byte(&mut pixels, y, i * 8, ram[v]);
                v += 1;
            }
            v += 13;
        }

        v = 0x413;
        for j in (-30i32..=64).rev() {
            let y = if j >= 0 { j } else { -j + 65 } as usize;
            decode_lcd_byte(&mut pixels, y, 0, ram[v]);
            v += 32;
        }
        decode_lcd_byte(&mut pixels, 65, 0, ram[0x0ff3]);

        pixels
    }

    /// Render LCD to RGB565 buffer
    pub fn render_lcd(&mut self, buf: &mut [u16], _ghosting: bool) {
        let pixels = self.render_lcd_buffer();

        // Render with theme
        use crate::lcd::LcdTheme;
        let theme = LcdTheme::GREY;

        for i in 0..159 * 96 {
            buf[i] = if pixels[i] { theme.fg } else { theme.bg };
        }
    }

    /// Get current model
    pub fn model(&self) -> &'static BbkModel {
        self.model
    }

    /// Check if emulator is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Stop the emulator
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Get frame count
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Create a save state
    pub fn save_state(&self) -> SaveState {
        SaveState {
            ram: self.cpu.memory().ram.to_vec(),
            flash: self.cpu.memory().flash[..0x14000].to_vec(),
            cpu: crate::save::CpuState {
                pc: self.cpu.pc(),
                sp: self.cpu.sp(),
                a: self.cpu.a(),
                x: self.cpu.x(),
                y: self.cpu.y(),
                status: self.cpu.status(),
                cycles: self.cpu.cycles(),
            },
            bank_switch: crate::save::BankState {
                banks: self.cpu.memory().bank_switch.banks.to_vec(),
                selected: self.cpu.memory().bank_switch.selected(),
            },
            bank_sys_d: self.model.bank_sys_d,
        }
    }

    /// Load a save state
    pub fn load_save_state(&mut self, state: &SaveState) -> Result<()> {
        self.cpu.memory_mut().ram.copy_from_slice(&state.ram);
        let len = state.flash.len().min(self.cpu.memory().flash.len());
        self.cpu.memory_mut().flash[..len].copy_from_slice(&state.flash[..len]);
        self.cpu.set_pc(state.cpu.pc);
        self.cpu.set_sp(state.cpu.sp);
        Ok(())
    }

    /// Get debugger
    pub fn debugger(&mut self) -> &mut Debugger {
        &mut self.debug
    }
}

fn decode_lcd_byte(pixels: &mut [bool; 159 * 96], y: usize, x: usize, byte: u8) {
    for bit in 0..8 {
        if x + bit < 159 {
            pixels[y * 159 + x + bit] = byte & (1 << (7 - bit)) != 0;
        }
    }
}

#[cfg(test)]
mod lcd_render_tests {
    use super::*;

    #[test]
    fn decodes_most_significant_bit_on_left() {
        let mut pixels = [false; 159 * 96];
        decode_lcd_byte(&mut pixels, 2, 8, 0b1000_0001);
        assert!(pixels[2 * 159 + 8]);
        assert!(pixels[2 * 159 + 15]);
        assert!(!pixels[2 * 159 + 9]);
    }

    #[test]
    fn clips_the_unused_160th_column() {
        let mut pixels = [false; 159 * 96];
        decode_lcd_byte(&mut pixels, 0, 152, 0xff);
        assert_eq!(pixels[..159].iter().filter(|pixel| **pixel).count(), 7);
    }
}
