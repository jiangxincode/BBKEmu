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
use crate::syscall::{SyscallContext, SyscallTable};
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
        }
    }

    /// Create emulator with default model (A4980)
    pub fn default() -> Self {
        Self::new(&MODEL_4980)
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
            // LCD syscalls
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
                // lcd_char
                let ch = self.cpu.a();
                log::trace!("lcd_char: 0x{:02X}", ch);
                SyscallResult::handled()
            }
            0xE00C => {
                // lcd_string
                let addr = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                log::trace!("lcd_string: addr=0x{:04X}", addr);
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
                log::trace!("lcd_rect");
                SyscallResult::handled()
            }
            0xE015 => {
                // lcd_line
                log::trace!("lcd_line");
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

            // Keyboard syscalls
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

            // Audio syscalls
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

            // Timer syscalls
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

            // String syscalls
            0xE050 => {
                // strlen
                let addr = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let mut len = 0u8;
                loop {
                    if self.cpu.memory().read(addr + len as u16) == 0 {
                        break;
                    }
                    len = len.wrapping_add(1);
                    if len == 0 {
                        break;
                    }
                }
                SyscallResult::with_return(len)
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

            // System syscalls
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

            // BBK OS functions (4980 model)
            // 0xD2F6: Draw character to LCD
            // Parameters: A=character code, X=font/mode, [0x26:0x27]=pointer to position data
            // The position data at [0x26:0x27] contains the LCD framebuffer address
            0xD2F6 => {
                let ch = self.cpu.a();
                let _mode = self.cpu.x();
                let pos_addr =
                    self.cpu.memory().ram[0x26] as u16 | (self.cpu.memory().ram[0x27] as u16) << 8;

                // Read the position from the pointer
                let fb_addr = self.cpu.memory().read16(pos_addr) as usize;

                // Get font bitmap - use built-in font or font ROM
                let font_bitmap: [u8; 8] = if let Some(ref rom) = self.cpu.memory().rom_8 {
                    // Use font ROM if available
                    let font_offset = (ch as usize) * 8;
                    if font_offset + 8 <= rom.len() {
                        let mut data = [0u8; 8];
                        data.copy_from_slice(&rom[font_offset..font_offset + 8]);
                        data
                    } else {
                        crate::font_data::get_font_bitmap(ch)
                    }
                } else {
                    // Use built-in font
                    crate::font_data::get_font_bitmap(ch)
                };

                // Write font data to LCD framebuffer
                for row in 0..8 {
                    let fb_offset = fb_addr + row * 20; // 20 bytes per row
                    if fb_offset < 0x1000 - 0x0400 {
                        self.cpu.memory_mut().ram[0x0400 + fb_offset] = font_bitmap[row];
                    }
                }

                SyscallResult::handled()
            }

            // 0xDACA: Set cursor position
            // Parameters: A=value, [0x20:0x21]=position in LCD coordinates
            0xDACA => {
                let stack = self.cpu.memory().read16(0x28).wrapping_sub(2);
                let lo = self.cpu.memory().read(0x20);
                let hi = self.cpu.memory().read(0x21);
                self.cpu.memory_mut().write(0x28, stack as u8);
                self.cpu.memory_mut().write(0x29, (stack >> 8) as u8);
                self.cpu.memory_mut().write(stack, lo);
                self.cpu.memory_mut().write(stack.wrapping_add(1), hi);

                SyscallResult::handled()
            }

            // Push A onto the compiler-managed software stack.
            0xDAAA => {
                let stack = self.cpu.memory().read16(0x28).wrapping_sub(1);
                let value = self.cpu.a();
                self.cpu.memory_mut().write(0x28, stack as u8);
                self.cpu.memory_mut().write(0x29, (stack >> 8) as u8);
                self.cpu.memory_mut().write(stack, value);
                SyscallResult::handled()
            }

            // 0xD340: Draw string or block
            // Parameters: [0x20:0x21]=source address, [0x26:0x27]=dest address
            0xD340 => {
                let src =
                    self.cpu.memory().ram[0x20] as u16 | (self.cpu.memory().ram[0x21] as u16) << 8;
                let dst =
                    self.cpu.memory().ram[0x26] as u16 | (self.cpu.memory().ram[0x27] as u16) << 8;

                // Copy data from src to dst
                // This is used for copying screen regions or drawing blocks
                for i in 0..32 {
                    let byte = self.cpu.memory().read(src + i);
                    if byte == 0 {
                        break;
                    }
                    if dst + i >= 0x0400 && dst + i < 0x1000 {
                        self.cpu.memory_mut().ram[(dst + i) as usize] = byte;
                    }
                }

                SyscallResult::handled()
            }

            // 0xD300: Clear screen area
            0xD300 => {
                // Clear LCD framebuffer
                for i in 0x0400..0x1000 {
                    self.cpu.memory_mut().ram[i] = 0;
                }
                SyscallResult::handled()
            }

            // 0xD320: Draw horizontal line
            0xD320 => {
                let y = self.cpu.a();
                let x1 = self.cpu.x();
                let x2 = self.cpu.y();

                // Draw horizontal line at y from x1 to x2
                for x in x1..=x2 {
                    self.lcd.set_pixel(x, y, true);
                }

                SyscallResult::handled()
            }

            // 0xD360: Draw vertical line
            0xD360 => {
                let x = self.cpu.a();
                let y1 = self.cpu.x();
                let y2 = self.cpu.y();

                // Draw vertical line at x from y1 to y2
                for y in y1..=y2 {
                    self.lcd.set_pixel(x, y, true);
                }

                SyscallResult::handled()
            }

            // 0xD380: Fill rectangle
            0xD380 => {
                let x = self.cpu.memory().ram[0x20];
                let y = self.cpu.memory().ram[0x21];
                let w = self.cpu.memory().ram[0x22];
                let h = self.cpu.memory().ram[0x23];

                // Fill rectangle
                for dy in 0..h {
                    for dx in 0..w {
                        self.lcd.set_pixel(x + dx, y + dy, true);
                    }
                }

                SyscallResult::handled()
            }

            // 0xD3A0: Get key input
            0xD3A0 => {
                let key = self.input.get_key();
                SyscallResult::with_return(key)
            }

            // 0xD3C0: Check key hit
            0xD3C0 => {
                let has_key = self.input.key_hit();
                SyscallResult::with_return(if has_key { 1 } else { 0 })
            }

            // 0xD400: Play sound
            0xD400 => {
                let freq = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let duration = self.cpu.a() as u16;
                if freq > 0 {
                    self.audio.play_tone(freq, duration * 10);
                }
                SyscallResult::handled()
            }

            // 0xD420: Delay/wait
            0xD420 => {
                let ms = self.cpu.a() as u32;
                // Just consume some cycles
                SyscallResult {
                    handled: true,
                    return_value: None,
                    cycles: ms * 4000,
                }
            }

            _ => {
                // Unknown syscall - log it for debugging
                log::info!("Unknown syscall at 0x{:04X}", target);
                SyscallResult::not_handled()
            }
        }
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
            log::info!(
                "BRK at 0x{:04X} SP=0x{:02X}, game exiting",
                pc,
                self.cpu.sp()
            );
            self.running = false;
            return 1;
        }

        // Check for JSR instruction - potential syscall
        if opcode == 0x20 {
            let target = self.cpu.memory().read16(pc + 1);
            if target >= 0xD000 {
                let ram = &self.cpu.memory().ram;
                log::debug!(
                    "OS call caller=0x{pc:04X} target=0x{target:04X} physical=0x{:06X} A={:02X} X={:02X} Y={:02X} ZP20={:02X?}",
                    self.cpu.memory().bank_switch.translate(target),
                    self.cpu.a(),
                    self.cpu.x(),
                    self.cpu.y(),
                    &ram[0x20..0x30]
                );
            }

            // Check if target is in OS/system area (0xD000-0xFFFF)
            // or if it's a known syscall address
            // Intercept calls to OS area (0xD000+) or registered syscalls
            let hle_mode = self.cpu.memory().rom_e.is_none();
            if hle_mode && target == 0xD2F6 {
                if self.begin_hle_far_call(pc) {
                    return 6;
                }
            }
            if hle_mode && (target >= 0xD000 || self.syscalls.is_syscall(target)) {
                let result = self.handle_syscall(target);

                if result.handled {
                    self.cpu.set_pc(pc + 3);
                    if let Some(val) = result.return_value {
                        self.cpu.set_a(val);
                    }
                    return 3;
                } else {
                    // Unknown OS function - skip and return
                    self.cpu.set_pc(pc + 3);
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

    fn begin_hle_far_call(&mut self, caller: u16) -> bool {
        let descriptor = self.cpu.memory().read16(0x26);
        let Some((target, segment)) = self.hle_descriptor(descriptor) else {
            log::warn!("Unknown HLE far-call descriptor 0x{descriptor:04X}");
            return false;
        };

        // Segments below E0 address OS ROM groups and require semantic handlers.
        if segment < 0xE0 {
            log::debug!(
                "Skipping ROM-only HLE far call descriptor=0x{descriptor:04X} target=0x{target:04X} segment={segment:02X}"
            );
            return false;
        }

        let banks = std::array::from_fn(|index| self.cpu.memory().bank_switch.banks[5 + index]);
        let data_base = u32::from(self.cpu.memory().read(0x2029))
            | (u32::from(self.cpu.memory().read(0x202A)) << 8);
        let base = data_base + u32::from(segment - 0xE0) * 4;
        for index in 0..4 {
            self.cpu
                .memory_mut()
                .bank_switch
                .set(5 + index, base + u32::from(index));
        }

        let sp = self.cpu.sp();
        let trampoline = HLE_FAR_RETURN.wrapping_sub(1);
        self.cpu.memory_mut().ram[0x100 | sp as usize] = (trampoline >> 8) as u8;
        self.cpu.memory_mut().ram[0x100 | sp.wrapping_sub(1) as usize] = trampoline as u8;
        self.cpu.set_sp(sp.wrapping_sub(2));
        self.hle_far_calls.push(HleFarCall {
            return_pc: caller.wrapping_add(3),
            banks,
        });
        self.cpu.set_pc(target);
        true
    }

    fn hle_descriptor(&self, address: u16) -> Option<(u16, u8)> {
        const MODEL_4988: &[(u16, u16, u8)] = &[
            (0xE7B2, 0x7770, 0x07),
            (0xE8EC, 0x624D, 0x06),
            (0xE932, 0x5000, 0x08),
            (0xE935, 0x5093, 0x08),
            (0xE938, 0x5D45, 0x08),
            (0xE93B, 0x5FF3, 0x08),
            (0xE93E, 0x6111, 0x08),
            (0xE944, 0x7D92, 0x08),
            (0xE95C, 0x7093, 0x08),
            (0xE965, 0x6A79, 0x08),
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
