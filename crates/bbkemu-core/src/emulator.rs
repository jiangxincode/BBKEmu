//! Main emulator orchestrator

use anyhow::Result;

use crate::cpu::CpuWrapper;
use crate::memory::Memory;
use crate::lcd::Lcd;
use crate::input::{Input, BbkKey};
use crate::audio::Audio;
use crate::syscall::{SyscallContext, SyscallTable};
use crate::syscalls;
use crate::gam::GamFile;
use crate::debug::Debugger;
use crate::model::{BbkModel, MODEL_4980};
use crate::save::SaveState;

/// Main emulator struct
pub struct Emulator {
    /// 6502 CPU
    pub cpu: CpuWrapper,
    /// Memory bus
    pub memory: Memory,
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
}

impl Emulator {
    /// Create a new emulator instance
    pub fn new(model: &'static BbkModel) -> Self {
        let audio = Audio::new(44100);
        let syscalls = syscalls::build_syscall_table();

        Self {
            cpu: CpuWrapper::new(),
            memory: Memory::new(),
            lcd: Lcd::new(),
            input: Input::new(),
            audio,
            syscalls,
            debug: Debugger::new(),
            model,
            running: false,
            frame_count: 0,
        }
    }

    /// Create emulator with default model (A4980)
    pub fn default() -> Self {
        Self::new(&MODEL_4980)
    }

    /// Load a GAM file
    pub fn load_gam(&mut self, data: &[u8]) -> Result<()> {
        let gam = GamFile::parse(data)?;
        log::info!("Loading game: {} (entry: 0x{:04X})", gam.name(), gam.entry_point);

        // Initialize memory
        self.memory.init();

        // Load game into flash at 0x20D000
        let flash_offset = 0xD000;
        let game_data = &gam.data;
        let end = (flash_offset + game_data.len()).min(self.memory.flash.len());
        self.memory.flash[flash_offset..end].copy_from_slice(&game_data[..end - flash_offset]);

        // Setup flash headers
        let sys_hdr = [
            0xC0u8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x2F,
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
        self.memory.flash[flash_base..flash_base + 16].copy_from_slice(&sys_hdr);
        self.memory.flash[flash_base + 16..flash_base + 32].copy_from_slice(&gam_hdr);

        // Setup bank mappings
        self.memory.bank_switch.set(0x5, 0x20D);
        self.memory.bank_switch.set(0x6, 0x20E);
        self.memory.bank_switch.set(0x7, 0x20F);
        self.memory.bank_switch.set(0x8, 0x210);

        let data_bank = 0x20D + (gam.data_offset >> 12);
        self.memory.bank_switch.set(0x9, data_bank);
        self.memory.bank_switch.set(0xA, data_bank + 1);
        self.memory.bank_switch.set(0xB, data_bank + 2);
        self.memory.bank_switch.set(0xC, data_bank + 3);

        // Setup save area
        let save_base = 0x7000; // 4980
        self.memory.flash[flash_base + save_base + 0xF8] = 0x02;
        self.memory.flash[flash_base + save_base + 0xF9] = 0x02;
        self.memory.flash[flash_base + save_base + 0xFA] = 0x02;
        self.memory.flash[flash_base + save_base + 0xFB] = 0x02;
        self.memory.flash[flash_base + save_base + 0xFC] = 0x02;
        self.memory.flash[flash_base + save_base + 0xFD] = 0x02;
        self.memory.flash[flash_base + save_base + 0xFE] = 0x03;
        self.memory.flash[flash_base + save_base + 0xFF] = 0x02;

        // Set system control
        self.memory.write(0x2029, 0x0D);
        self.memory.write(0x202A, 0x02);

        // Push return address (BRK handler at 0x0260)
        self.cpu.push16(&mut self.memory, 0x0260);

        // Set PC to game entry point
        self.cpu.set_pc(gam.entry_point);

        // Set initial CPU state
        self.cpu.set_sp(0xFF);

        self.running = true;
        log::info!("Game loaded, starting execution at 0x{:04X}", gam.entry_point);

        Ok(())
    }

    /// Load font ROM (8.BIN) - optional
    pub fn load_rom_8(&mut self, data: &[u8]) {
        self.memory.load_rom_8(data);
        log::info!("Font ROM loaded ({} bytes)", data.len());
    }

    /// Load OS ROM (E.BIN) - optional, for LLE fallback
    pub fn load_rom_e(&mut self, data: &[u8]) {
        self.memory.load_rom_e(data);
        log::info!("OS ROM loaded ({} bytes)", data.len());
    }

    /// Run one frame (~16.67ms at 60fps)
    pub fn run_frame(&mut self) {
        // BBK runs at ~4MHz, 60fps = ~66666 cycles per frame
        let cycles_per_frame = 66666u32;
        let mut cycles_run = 0u32;

        while cycles_run < cycles_per_frame && self.running {
            cycles_run += self.step();
        }

        self.frame_count += 1;
    }

    /// Execute a single CPU step
    fn step(&mut self) -> u32 {
        let pc = self.cpu.pc();

        // Check for breakpoint
        if self.debug.has_breakpoint(pc) {
            log::info!("Breakpoint hit at 0x{:04X}", pc);
            self.debug.set_stepping(true);
        }

        // Read the opcode
        let opcode = self.memory.read(pc);

        // Check for JSR instruction (0x20) - potential syscall
        if opcode == 0x20 {
            let target = self.memory.read16(pc + 1);
            if self.syscalls.is_syscall(target) {
                // Log syscall before creating context (to avoid borrow issues)
                let cycles = self.cpu.cycles;
                let name = self.syscalls.get(target).map_or("unknown", |e| e.name);
                self.debug.log_syscall(cycles, target, name);

                // Intercept the syscall
                let mut ctx = SyscallContext {
                    cpu: &mut self.cpu,
                    memory: &mut self.memory,
                    lcd: &mut self.lcd,
                    input: &mut self.input,
                    audio: &mut self.audio,
                };

                let result = self.syscalls.try_handle(target, &mut ctx);

                if result.handled {
                    // Skip the JSR instruction (3 bytes)
                    self.cpu.set_pc(pc + 3);
                    return 3; // Approximate cycles
                }
            }
        }

        // Check for BRK instruction (0x00)
        if opcode == 0x00 {
            // BRK - check if it's our exit handler
            log::info!("BRK at 0x{:04X}, game exiting", pc);
            self.running = false;
            return 1;
        }

        // Normal instruction execution
        // TODO: Integrate with mos6502 crate
        // For now, just advance PC
        self.cpu.set_pc(pc + 1);
        self.cpu.cycles += 1;

        1
    }

    /// Handle key down event
    pub fn key_down(&mut self, key: BbkKey) {
        self.input.key_down(key, self.frame_count * 16); // Approximate timestamp
    }

    /// Handle key up event
    pub fn key_up(&mut self) {
        self.input.key_up();
    }

    /// Get the LCD framebuffer
    pub fn get_framebuffer(&self) -> &[bool; 159 * 96] {
        // TODO: Return actual framebuffer
        static FB: [bool; 159 * 96] = [false; 159 * 96];
        &FB
    }

    /// Render LCD to RGB565 buffer
    pub fn render_lcd(&mut self, buf: &mut [u16], ghosting: bool) {
        use crate::lcd::LcdTheme;
        let theme = LcdTheme::GREY; // TODO: Make configurable
        if ghosting {
            self.lcd.render_with_ghosting(buf, &theme);
        } else {
            self.lcd.render(buf, &theme);
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
            ram: self.memory.ram().to_vec(),
            flash: self.memory.flash[..0x14000].to_vec(), // Save area only
            cpu: crate::save::CpuState {
                pc: self.cpu.pc(),
                sp: self.cpu.sp(),
                a: self.cpu.a(),
                x: self.cpu.x(),
                y: self.cpu.y(),
                status: self.cpu.status(),
                cycles: self.cpu.cycles,
            },
            bank_switch: crate::save::BankState {
                banks: vec![0; 16], // TODO: Get actual bank state
                selected: 0,
            },
            bank_sys_d: self.model.bank_sys_d,
        }
    }

    /// Load a save state
    pub fn load_save_state(&mut self, state: &SaveState) -> Result<()> {
        self.memory.ram_mut().copy_from_slice(&state.ram);
        let len = state.flash.len().min(self.memory.flash.len());
        self.memory.flash[..len].copy_from_slice(&state.flash[..len]);
        self.cpu.set_pc(state.cpu.pc);
        self.cpu.set_sp(state.cpu.sp);
        self.cpu.cycles = state.cpu.cycles;
        Ok(())
    }

    /// Get debugger
    pub fn debugger(&mut self) -> &mut Debugger {
        &mut self.debug
    }
}
