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
            // 0xD2F6: Generic far-call via descriptor
            // Parameters: [0x26:0x27]=pointer to descriptor (3 bytes: target_low, target_high, segment)
            // A register contains the first parameter for the target function
            0xD2F6 => {
                let a_val = self.cpu.a();
                let descriptor_addr =
                    self.cpu.memory().ram[0x26] as u16 | (self.cpu.memory().ram[0x27] as u16) << 8;

                // Try to get descriptor from hardcoded table first
                let descriptor = self.hle_descriptor(descriptor_addr);
                let (target, segment) = descriptor.unwrap_or_else(|| {
                    // Fallback: read from memory
                    let target = self.cpu.memory().read16(descriptor_addr);
                    let segment = self.cpu.memory().read(descriptor_addr.wrapping_add(2));
                    (target, segment)
                });

                log::info!(
                    "D2F6: descriptor_addr=0x{descriptor_addr:04X} target=0x{target:04X} segment=0x{segment:02X} A=0x{a_val:02X}"
                );

                // Dispatch based on segment and target
                // Note: We don't set PC here because D2F6 is a JSR target
                // The game will return to the instruction after JSR $D2F6
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
                            // strlen
                            0x624D => {
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
                            // lcd_draw_char
                            0x7770 => {
                                // Read cursor position from software stack
                                let stack_ptr = self.cpu.memory().read16(0x28);
                                let pos = self.cpu.memory().read16(stack_ptr);
                                let fb_addr = if pos >=0x0400 && pos <0x1000 {
                                    (pos -0x0400) as usize
                                } else {
                                    0
                                };
                                log::info!(
                                    "lcd_draw_char: ch=0x{:02X} pos=0x{:04X} fb=0x{:04X} sp=0x{:04X}",
                                    a_val, pos, fb_addr, stack_ptr
                                );
                                let font_bitmap = crate::font_data::get_font_bitmap(a_val);
                                for row in 0..8 {
                                    let offset = fb_addr + row * 20;
                                    if offset <0x0C00 {
                                        self.cpu.memory_mut().ram[0x0400 + offset] = font_bitmap[row];
                                    }
                                }
                            }
                            // volume_set
                            0x7716 => {
                                if a_val == 0 {
                                    self.cpu.memory_mut().ram[0x201B] &= 0xFB;
                                } else {
                                    self.cpu.memory_mut().ram[0x201B] |= 0x04;
                                }
                            }
                            // volume_get
                            0x772E => {
                                let muted = self.cpu.memory().ram[0x201B] & 0x04;
                                self.cpu.set_a(if muted != 0 { 1 } else { 0 });
                            }
                            // rtc_get_sec
                            0x759F => {
                                let sec = self.cpu.memory().ram[0x234] & 0x3F;
                                self.cpu.set_a(sec);
                            }
                            // lcd_enable
                            0x7CE5 => {
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
                        // Game data is at flash offset 0xD000
                        const GAME_DATA_FLASH: u32 = 0xD000;
                        match target {
                            // file_open - open game data file
                            // A = mode, [0x20:0x21] = filename pointer
                            0x5000 => {
                                // Return handle 0 (success)
                                self.cpu.set_a(0x00);
                            }
                            // file_read - read from game data
                            // A = handle, X/Y = buffer addr, [0x20:0x21] = count
                            // Returns bytes read in A
                            0x5093 => {
                                let handle = a_val;
                                let buf_addr = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                                let count = self.cpu.memory().read16(0x20);
                                // Read file position from RAM (stored at 0x2060-0x2063)
                                let pos_lo = self.cpu.memory().ram[0x2060];
                                let pos_hi = self.cpu.memory().ram[0x2061];
                                let file_pos = pos_lo as u32 | (pos_hi as u32) << 8;
                                // Read from flash
                                let flash_addr = GAME_DATA_FLASH + file_pos;
                                let mut bytes_read = 0u16;
                                for i in 0..count {
                                    let byte = self.cpu.memory().read_physical(flash_addr + i as u32);
                                    self.cpu.memory_mut().write(buf_addr + i, byte);
                                    bytes_read += 1;
                                }
                                // Advance file position
                                let new_pos = file_pos + bytes_read as u32;
                                self.cpu.memory_mut().ram[0x2060] = new_pos as u8;
                                self.cpu.memory_mut().ram[0x2061] = (new_pos >> 8) as u8;
                                self.cpu.set_a(bytes_read as u8);
                            }
                            // file_write
                            0x5D45 => {
                                self.cpu.set_a(0); //0 bytes written
                            }
                            // file_close
                            0x5FF3 => {
                                self.cpu.set_a(0); // Success
                            }
                            // file_delete
                            0x6111 => {
                                self.cpu.set_a(0xFF); // Error
                            }
                            // file_eof - check end of file
                            0x6A79 => {
                                let pos_lo = self.cpu.memory().ram[0x2060];
                                let pos_hi = self.cpu.memory().ram[0x2061];
                                let file_pos = pos_lo as u32 | (pos_hi as u32) << 8;
                                // Game data is up to 32KB
                                let game_size = 0x8000u32;
                                self.cpu.set_a(if file_pos >= game_size { 1 } else { 0 });
                            }
                            // file_tell - get file position
                            0x7093 => {
                                let pos_lo = self.cpu.memory().ram[0x2060];
                                let pos_hi = self.cpu.memory().ram[0x2061];
                                // Return position in X/Y
                                self.cpu.set_a(pos_lo);
                            }
                            // file_seek - seek in file
                            // A = handle, X/Y = offset, [0x20] = whence (0=SET, 1=CUR, 2=END)
                            0x7D92 => {
                                let offset = self.cpu.x() as u32 | (self.cpu.y() as u32) << 8;
                                let whence = self.cpu.memory().ram[0x20];
                                let pos_lo = self.cpu.memory().ram[0x2060];
                                let pos_hi = self.cpu.memory().ram[0x2061];
                                let cur_pos = pos_lo as u32 | (pos_hi as u32) << 8;
                                let new_pos = match whence {
                                    0 => offset, // SET
                                    1 => cur_pos.wrapping_add(offset), // CUR
                                    2 => 0x8000u32.wrapping_sub(offset), // END
                                    _ => offset,
                                };
                                self.cpu.memory_mut().ram[0x2060] = new_pos as u8;
                                self.cpu.memory_mut().ram[0x2061] = (new_pos >> 8) as u8;
                                self.cpu.set_a(0); // Success
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
                        log::debug!("Unknown segment 0x{segment:02X} in D2F6 descriptor");
                        self.cpu.set_a(0);
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

            // 0xD6AF: Complex drawing/positioning function
            0xD6AF => {
                // This is a complex function that deals with drawing coordinates
                // For HLE, just return success
                SyscallResult::handled()
            }

            // 0xE901: Jump to 0xF5BD - table/buffer init
            0xE901 => {
                SyscallResult::handled()
            }

            // 0xE904: Jump to 0xF68A - screen init
            0xE904 => {
                SyscallResult::handled()
            }

            // 0xE907: Jump to 0xF475 - drawing helper
            0xE907 => {
                SyscallResult::handled()
            }

            // 0xE90A: Jump to 0xF457 - clear helper
            0xE90A => {
                SyscallResult::handled()
            }

            // 0xD596: Add 8 to [0x2A:0x2B], store in [0x20:0x21]
            0xD596 => {
                let addr = self.cpu.memory().read16(0x2A);
                let result = addr.wrapping_add(8);
                self.cpu.memory_mut().write(0x20, result as u8);
                self.cpu.memory_mut().write(0x21, (result >> 8) as u8);
                SyscallResult::handled()
            }

            // 0xD5A6: Add 0x20 to [0x2A:0x2B], store in [0x23:0x24]
            0xD5A6 => {
                let addr = self.cpu.memory().read16(0x2A);
                let result = addr.wrapping_add(0x20);
                self.cpu.memory_mut().write(0x23, result as u8);
                self.cpu.memory_mut().write(0x24, (result >> 8) as u8);
                SyscallResult::handled()
            }

            // 0xD5B6: Add 0x18 to [0x2A:0x2B], store in [0x23:0x24]
            0xD5B6 => {
                let addr = self.cpu.memory().read16(0x2A);
                let result = addr.wrapping_add(0x18);
                self.cpu.memory_mut().write(0x23, result as u8);
                self.cpu.memory_mut().write(0x24, (result >> 8) as u8);
                SyscallResult::handled()
            }

            // 0xD2CA: Bitmap AND operation
            // Reads from [0x20:0x21] and [0x23:0x24], ANDs, stores at [0x2A:0x2B]+8
            0xD2CA => {
                let src1 = self.cpu.memory().read16(0x20);
                let src2 = self.cpu.memory().read16(0x23);
                let dst = self.cpu.memory().read16(0x2A);
                for i in 0..4 {
                    let b1 = self.cpu.memory().read(src1 + i);
                    let b2 = self.cpu.memory().read(src2 + i);
                    self.cpu.memory_mut().write(dst + 8 + i, b1 & b2);
                }
                // Call 0xD596 to update [0x20:0x21]
                let addr = self.cpu.memory().read16(0x2A);
                let result = addr.wrapping_add(8);
                self.cpu.memory_mut().write(0x20, result as u8);
                self.cpu.memory_mut().write(0x21, (result >> 8) as u8);
                SyscallResult::handled()
            }

            // 0xD362: Compare [0x20:0x21] with [0x23:0x24]
            0xD362 => {
                let val1 = self.cpu.memory().read16(0x20);
                let val2 = self.cpu.memory().read16(0x23);
                self.cpu.set_a(if val1 == val2 { 0 } else { 1 });
                SyscallResult::handled()
            }

            // 0xD1A2: Multiply [0x20:0x21] by [0x23:0x24], store in [0x26:0x27]
            0xD1A2 => {
                let a = self.cpu.memory().read16(0x20);
                let b = self.cpu.memory().read16(0x23);
                let result = (a as u32).wrapping_mul(b as u32);
                self.cpu.memory_mut().write(0x26, result as u8);
                self.cpu.memory_mut().write(0x27, (result >> 8) as u8);
                SyscallResult::handled()
            }

            // 0xD6AF: Drawing/positioning function
            0xD6AF => {
                // Complex function, just return for now
                SyscallResult::handled()
            }

            // 0xE8F8: Segment dispatch - bank switch setup
            0xE8F8 => {
                // Save current bank mapping and set up for segment call
                SyscallResult::handled()
            }

            // 0xE8FB: Segment dispatch - bank switch setup
            0xE8FB => {
                SyscallResult::handled()
            }

            // 0xE8FE: Segment dispatch - bank switch restore
            0xE8FE => {
                SyscallResult::handled()
            }

            // 0xD572: Function pointer call via stack
            // Decrements [0x26:0x27] by 1 and jumps to it
            0xD572 => {
                let addr = self.cpu.memory().read16(0x26);
                let target = addr.wrapping_sub(1);
                self.cpu.memory_mut().write(0x26, target as u8);
                self.cpu.memory_mut().write(0x27, (target >> 8) as u8);
                // Push return address and jump
                let pc = self.cpu.pc();
                let sp = self.cpu.sp();
                let ret = pc.wrapping_sub(1); // Return to instruction after JSR
                self.cpu.memory_mut().ram[0x100 | sp as usize] = (ret >> 8) as u8;
                self.cpu.memory_mut().ram[0x100 | sp.wrapping_sub(1) as usize] = ret as u8;
                self.cpu.set_sp(sp.wrapping_sub(2));
                self.cpu.set_pc(target);
                SyscallResult::handled()
            }

            // 0xDCEF: Push 4 bytes from [0x20:0x21] onto stack
            0xDCEF => {
                let addr = self.cpu.memory().read16(0x20);
                let sp = self.cpu.sp();
                for i in 0..4 {
                    let byte = self.cpu.memory().read(addr + i);
                    self.cpu.memory_mut().ram[0x100 | sp.wrapping_sub(i as u8) as usize] = byte;
                }
                self.cpu.set_sp(sp.wrapping_sub(4));
                SyscallResult::handled()
            }

            // 0xD340: Compare and set flags
            0xD340 => {
                let val1 = self.cpu.memory().read16(0x20);
                let val2 = self.cpu.memory().read16(0x23);
                self.cpu.set_a(if val1 == val2 { 0 } else { 1 });
                SyscallResult::handled()
            }

            // 0xDAE6: Copy [0x23:0x24] to [0x20:0x21]
            0xDAE6 => {
                let src = self.cpu.memory().read16(0x23);
                let dst = self.cpu.memory().read16(0x20);
                for i in 0..2 {
                    let byte = self.cpu.memory().read(src + i);
                    self.cpu.memory_mut().write(dst + i, byte);
                }
                SyscallResult::handled()
            }

            // 0xDBE1: Loop/counter function
            0xDBE1 => {
                let val = self.cpu.memory().read16(0x23);
                self.cpu.set_a(if val == 0 { 0 } else { 1 });
                SyscallResult::handled()
            }

            // 0xDC80: Set cursor position from stack
            0xDC80 => {
                SyscallResult::handled()
            }

            // 0xD85F: Drawing helper
            0xD85F => {
                SyscallResult::handled()
            }

            // 0xDD38: Stack operation
            0xDD38 => {
                SyscallResult::handled()
            }

            // 0xDCB1: Drawing helper
            0xDCB1 => {
                SyscallResult::handled()
            }

            // 0xD29D: Drawing helper
            0xD29D => {
                SyscallResult::handled()
            }

            // 0xDB19: Drawing helper
            0xDB19 => {
                SyscallResult::handled()
            }

            // 0xDCF7: Stack operation
            0xDCF7 => {
                SyscallResult::handled()
            }

            // 0xDC83: Drawing helper
            0xDC83 => {
                SyscallResult::handled()
            }

            // 0xD201: Drawing helper
            0xD201 => {
                SyscallResult::handled()
            }

            // 0xD0A8: Drawing helper
            0xD0A8 => {
                SyscallResult::handled()
            }

            // 0xDCAB: Drawing helper
            0xDCAB => {
                SyscallResult::handled()
            }

            // 0xDCF4: Stack operation
            0xDCF4 => {
                SyscallResult::handled()
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
                log::info!(
                    "OS call caller=0x{pc:04X} target=0x{target:04X} A={:02X} X={:02X} Y={:02X}",
                    self.cpu.a(),
                    self.cpu.x(),
                    self.cpu.y()
                );
            }

            // Check if target is in OS/system area (0xD000-0xFFFF)
            // or if it's a known syscall address
            // Intercept calls to OS area (0xD000+) or registered syscalls
            let hle_mode = self.cpu.memory().rom_e.is_none();
            // D2F6 is now handled as a generic syscall dispatcher
            // No need for special begin_hle_far_call handling
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
            return self.handle_rom_far_call(target, segment, caller);
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

    /// Handle ROM far-call by implementing semantic equivalents
    fn handle_rom_far_call(&mut self, target: u16, segment: u8, caller: u16) -> bool {
        // ROM function implementations
        match (segment, target) {
            // Segment 0x04: Utility/Helper functions
            (0x04, _) => self.handle_segment_04(target, caller),
            // Segment 0x06: String/Math functions
            (0x06, _) => self.handle_segment_06(target, caller),
            // Segment 0x07: LCD/Graphics functions
            (0x07, _) => self.handle_segment_07(target, caller),
            // Segment 0x08: File/System functions
            (0x08, _) => self.handle_segment_08(target, caller),
            // Segment 0x0D: Initialization functions
            (0x0D, _) => self.handle_segment_0D(target, caller),
            _ => {
                log::debug!(
                    "Unknown ROM far-call segment=0x{segment:02X} target=0x{target:04X}, skipping"
                );
                false
            }
        }
    }

    /// Handle Segment 0x04: Utility/Helper functions
    fn handle_segment_04(&mut self, target: u16, caller: u16) -> bool {
        match target {
            // Complex utility function - just return success for HLE
            0x5000 => {
                // This is a complex function that deals with stack manipulation
                // For HLE, just return with A=0
                self.cpu.set_a(0);
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            _ => {
                log::debug!("Unknown segment 0x04 function at 0x{target:04X}");
                // Return success for unknown functions
                self.cpu.set_a(0);
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
        }
    }

    /// Handle Segment 0x0D: Initialization functions
    fn handle_segment_0D(&mut self, target: u16, caller: u16) -> bool {
        match target {
            // Init function - initialize memory areas
            0x6911 => {
                // Clear some memory areas
                self.cpu.memory_mut().ram[0x216A] = 0;
                for i in 0..4 {
                    let offset = i * 0x0C;
                    self.cpu.memory_mut().ram[0x2122 + offset] = 0;
                }
                self.cpu.set_a(0);
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            _ => {
                log::debug!("Unknown segment 0x0D function at 0x{target:04X}");
                // Return success for unknown functions
                self.cpu.set_a(0);
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
        }
    }

    /// Handle Segment 0x06: String/Math functions
    fn handle_segment_06(&mut self, target: u16, caller: u16) -> bool {
        match target {
            // strlen: X/Y = string address, returns length in A
            0x624D => {
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
                self.cpu.set_a(len);
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // strcpy: X/Y = dst, [0x20:0x21] = src
            0x6280 => {
                let dst = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let src = self.cpu.memory().read16(0x20);
                let mut i = 0u16;
                loop {
                    let byte = self.cpu.memory().read(src + i);
                    self.cpu.memory_mut().write(dst + i, byte);
                    if byte == 0 {
                        break;
                    }
                    i = i.wrapping_add(1);
                }
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // strcmp: X/Y = str1, [0x20:0x21] = str2, returns result in A
            0x62B0 => {
                let str1 = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let str2 = self.cpu.memory().read16(0x20);
                let mut i = 0u16;
                loop {
                    let c1 = self.cpu.memory().read(str1 + i);
                    let c2 = self.cpu.memory().read(str2 + i);
                    if c1 != c2 {
                        self.cpu.set_a(if c1 < c2 { 0xFF } else { 0x01 });
                        self.cpu.set_pc(caller.wrapping_add(3));
                        return true;
                    }
                    if c1 == 0 {
                        self.cpu.set_a(0);
                        self.cpu.set_pc(caller.wrapping_add(3));
                        return true;
                    }
                    i = i.wrapping_add(1);
                }
            }
            // strcat: X/Y = dst, [0x20:0x21] = src
            0x62E0 => {
                let dst = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let src = self.cpu.memory().read16(0x20);
                // Find end of dst
                let mut dst_end = 0u16;
                loop {
                    if self.cpu.memory().read(dst + dst_end) == 0 {
                        break;
                    }
                    dst_end = dst_end.wrapping_add(1);
                }
                // Copy src to end of dst
                let mut i = 0u16;
                loop {
                    let byte = self.cpu.memory().read(src + i);
                    self.cpu.memory_mut().write(dst + dst_end + i, byte);
                    if byte == 0 {
                        break;
                    }
                    i = i.wrapping_add(1);
                }
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // memcpy: X/Y = dst, [0x20:0x21] = src, A = length
            0x6310 => {
                let dst = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let src = self.cpu.memory().read16(0x20);
                let len = self.cpu.a() as u16;
                for i in 0..len {
                    let byte = self.cpu.memory().read(src + i);
                    self.cpu.memory_mut().write(dst + i, byte);
                }
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // memset: X/Y = dst, A = value, [0x20] = length
            0x6340 => {
                let dst = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let value = self.cpu.a();
                let len = self.cpu.memory().read(0x20) as u16;
                for i in 0..len {
                    self.cpu.memory_mut().write(dst + i, value);
                }
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            _ => {
                log::debug!("Unknown segment 0x06 function at 0x{target:04X}");
                false
            }
        }
    }

    /// Handle Segment 0x07: LCD/Graphics functions
    fn handle_segment_07(&mut self, target: u16, caller: u16) -> bool {
        match target {
            // lcd_draw_char: Draw character at position
            // A = character, X = column, Y = row
            0x7770 => {
                let ch = self.cpu.a();
                let col = self.cpu.x();
                let row = self.cpu.y();

                // Calculate framebuffer address
                let fb_addr = (row as usize * 20) + (col as usize);

                // Get font bitmap
                let font_bitmap = crate::font_data::get_font_bitmap(ch);

                // Write font data to LCD framebuffer
                for font_row in 0..8 {
                    let offset = fb_addr + font_row * 160 / 8;
                    if offset < 0x0C00 {
                        self.cpu.memory_mut().ram[0x0400 + offset] = font_bitmap[font_row];
                    }
                }

                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // lcd_draw_string: Draw string at position
            // X/Y = string address, [0x20] = column, [0x21] = row
            0x77A0 => {
                let str_addr = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let col = self.cpu.memory().read(0x20);
                let row = self.cpu.memory().read(0x21);

                let mut offset = 0u16;
                loop {
                    let ch = self.cpu.memory().read(str_addr + offset);
                    if ch == 0 {
                        break;
                    }

                    // Calculate framebuffer address
                    let fb_addr = (row as usize * 20) + ((col as usize + offset as usize) % 20);

                    // Get font bitmap
                    let font_bitmap = crate::font_data::get_font_bitmap(ch);

                    // Write font data to LCD framebuffer
                    for font_row in 0..8 {
                        let fb_offset = fb_addr + font_row * 160 / 8;
                        if fb_offset < 0x0C00 {
                            self.cpu.memory_mut().ram[0x0400 + fb_offset] =
                                font_bitmap[font_row];
                        }
                    }

                    offset = offset.wrapping_add(1);
                }

                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // rtc_get_sec: Get RTC seconds (masked with 0x3F)
            0x759F => {
                let sec = self.cpu.memory().ram[0x234] & 0x3F;
                self.cpu.set_a(sec);
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // volume_set: Set volume/mute
            // A = 0 for mute, non-zero for unmute
            0x7716 => {
                let value = self.cpu.a();
                if value == 0 {
                    self.cpu.memory_mut().ram[0x201B] &= 0xFB; // Clear bit2
                } else {
                    self.cpu.memory_mut().ram[0x201B] |= 0x04; // Set bit2
                }
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // volume_get: Get volume/mute status
            // Returns1 in A if unmuted,0 if muted
            0x772E => {
                let muted = self.cpu.memory().ram[0x201B] & 0x04;
                self.cpu.set_a(if muted != 0 { 1 } else { 0 });
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // lcd_enable: LCD display enable/configure
            0x7CE5 => {
                // Check if LCD is active
                let lcd_active = self.cpu.memory().ram[0x2059];
                if lcd_active == 0 {
                    self.cpu.set_a(0);
                    self.cpu.set_pc(caller.wrapping_add(3));
                    return true;
                }

                // Configure LCD controller registers
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

                self.cpu.set_a(0);
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            _ => {
                log::debug!("Unknown segment 0x07 function at 0x{target:04X}");
                false
            }
        }
    }

    /// Handle Segment 0x08: File/System functions
    fn handle_segment_08(&mut self, target: u16, caller: u16) -> bool {
        match target {
            // file_open: Open file
            // X/Y = filename address, A = mode (0=read, 1=write)
            0x5000 => {
                let _filename_addr = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let _mode = self.cpu.a();

                // For HLE, we don't support file operations
                // Return error (0xFF)
                self.cpu.set_a(0xFF);
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // file_read: Read from file
            // A = file handle, X/Y = buffer address, [0x20:0x21] = count
            0x5093 => {
                let _handle = self.cpu.a();
                let _buffer_addr = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let _count = self.cpu.memory().read16(0x20);

                // Return 0 bytes read
                self.cpu.set_a(0);
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // file_write: Write to file
            // A = file handle, X/Y = buffer address, [0x20:0x21] = count
            0x5D45 => {
                let _handle = self.cpu.a();
                let _buffer_addr = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let _count = self.cpu.memory().read16(0x20);

                // Return 0 bytes written
                self.cpu.set_a(0);
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // file_close: Close file
            // A = file handle
            0x5FF3 => {
                let _handle = self.cpu.a();

                // Return success
                self.cpu.set_a(0);
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // file_delete: Delete file
            // X/Y = filename address
            0x6111 => {
                let _filename_addr = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;

                // Return error
                self.cpu.set_a(0xFF);
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // file_seek: Seek in file
            // A = file handle, X/Y = offset, [0x20] = whence
            0x7D92 => {
                let _handle = self.cpu.a();
                let _offset = self.cpu.x() as u16 | (self.cpu.y() as u16) << 8;
                let _whence = self.cpu.memory().read(0x20);

                // Return error
                self.cpu.set_a(0xFF);
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // file_tell: Get file position
            // A = file handle
            0x7093 => {
                let _handle = self.cpu.a();

                // Return 0
                self.cpu.set_a(0);
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            // file_eof: Check end of file
            // A = file handle
            0x6A79 => {
                let _handle = self.cpu.a();

                // Return EOF
                self.cpu.set_a(1);
                self.cpu.set_pc(caller.wrapping_add(3));
                true
            }
            _ => {
                log::debug!("Unknown segment 0x08 function at 0x{target:04X}");
                false
            }
        }
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
