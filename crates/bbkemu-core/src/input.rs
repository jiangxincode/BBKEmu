//! Keyboard input handling

use std::collections::VecDeque;

/// BBK key codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BbkKey {
    OnOff = 0x00,
    HomeMenu = 0x01,
    EcSj = 0x02,
    EcSw = 0x03,
    Ce = 0x04,
    Dlg = 0x05,
    Download = 0x06,
    Spk = 0x07,
    Key1 = 0x08,
    Key2 = 0x09,
    Key3 = 0x0A,
    Key4 = 0x0B,
    Key5 = 0x0C,
    Key6 = 0x0D,
    Key7 = 0x0E,
    Key8 = 0x0F,
    Q = 0x10,
    W = 0x11,
    E = 0x12,
    R = 0x13,
    T = 0x14,
    Y = 0x15,
    U = 0x16,
    I = 0x17,
    A = 0x18,
    S = 0x19,
    D = 0x1A,
    F = 0x1B,
    G = 0x1C,
    H = 0x1D,
    J = 0x1E,
    K = 0x1F,
    Input = 0x20,
    Z = 0x21,
    X = 0x22,
    C = 0x23,
    V = 0x24,
    B = 0x25,
    N = 0x26,
    M = 0x27,
    Zy = 0x28,
    Help = 0x29,
    Search = 0x2A,
    Insert = 0x2B,
    Modify = 0x2C,
    Del = 0x2D,
    Exit = 0x2E,
    Enter = 0x2F,
    Key9 = 0x30,
    Key0 = 0x31,
    O = 0x32,
    P = 0x33,
    L = 0x34,
    Up = 0x35,
    Space = 0x36,
    Left = 0x37,
    Down = 0x38,
    Right = 0x39,
    PgUp = 0x3A,
    PgDn = 0x3B,
}

impl BbkKey {
    /// Convert from raw key code
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0x00..=0x07 => Some(unsafe { std::mem::transmute::<u8, BbkKey>(code) }),
            0x08..=0x0F => Some(unsafe { std::mem::transmute::<u8, BbkKey>(code) }),
            0x10..=0x17 => Some(unsafe { std::mem::transmute::<u8, BbkKey>(code) }),
            0x18..=0x1F => Some(unsafe { std::mem::transmute::<u8, BbkKey>(code) }),
            0x20..=0x27 => Some(unsafe { std::mem::transmute::<u8, BbkKey>(code) }),
            0x28..=0x2F => Some(unsafe { std::mem::transmute::<u8, BbkKey>(code) }),
            0x30..=0x3B => Some(unsafe { std::mem::transmute::<u8, BbkKey>(code) }),
            _ => None,
        }
    }

    /// Get key name for display
    pub fn name(&self) -> &'static str {
        match self {
            Self::OnOff => "ON/OFF",
            Self::HomeMenu => "MENU",
            Self::EcSj => "SJ",
            Self::EcSw => "SW",
            Self::Ce => "CE",
            Self::Dlg => "DLG",
            Self::Download => "DL",
            Self::Spk => "SPK",
            Self::Key1 => "1",
            Self::Key2 => "2",
            Self::Key3 => "3",
            Self::Key4 => "4",
            Self::Key5 => "5",
            Self::Key6 => "6",
            Self::Key7 => "7",
            Self::Key8 => "8",
            Self::Key9 => "9",
            Self::Key0 => "0",
            Self::Q => "Q",
            Self::W => "W",
            Self::E => "E",
            Self::R => "R",
            Self::T => "T",
            Self::Y => "Y",
            Self::U => "U",
            Self::I => "I",
            Self::O => "O",
            Self::P => "P",
            Self::A => "A",
            Self::S => "S",
            Self::D => "D",
            Self::F => "F",
            Self::G => "G",
            Self::H => "H",
            Self::J => "J",
            Self::K => "K",
            Self::L => "L",
            Self::Z => "Z",
            Self::X => "X",
            Self::C => "C",
            Self::V => "V",
            Self::B => "B",
            Self::N => "N",
            Self::M => "M",
            Self::Input => "INPUT",
            Self::Zy => "ZY",
            Self::Help => "HELP",
            Self::Search => "SEARCH",
            Self::Insert => "INSERT",
            Self::Modify => "MODIFY",
            Self::Del => "DEL",
            Self::Exit => "EXIT",
            Self::Enter => "ENTER",
            Self::Up => "UP",
            Self::Down => "DOWN",
            Self::Left => "LEFT",
            Self::Right => "RIGHT",
            Self::PgUp => "PGUP",
            Self::PgDn => "PGDN",
            Self::Space => "SPACE",
        }
    }
}

/// Input state manager
pub struct Input {
    /// Key buffer (FIFO)
    key_buffer: VecDeque<u8>,
    /// Current key code (for register emulation)
    key_code: u8,
    /// Whether a key interrupt is pending
    interrupt_pending: bool,
    /// Minimum interval between repeated key presses (ms)
    min_repeat_interval: u64,
    /// Last key press time
    last_press_time: u64,
    /// Last key pressed
    last_key: u8,
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

impl Input {
    pub fn new() -> Self {
        Self {
            key_buffer: VecDeque::with_capacity(16),
            key_code: 0,
            interrupt_pending: false,
            min_repeat_interval: 0,
            last_press_time: 0,
            last_key: 0,
        }
    }

    /// Handle key down event from frontend
    pub fn key_down(&mut self, key: BbkKey, timestamp_ms: u64) {
        let code = key as u8;

        // Rate limiting
        if code == self.last_key
            && timestamp_ms.saturating_sub(self.last_press_time) < self.min_repeat_interval
        {
            return;
        }

        self.last_key = code;
        self.last_press_time = timestamp_ms;

        // Set key code register
        self.key_code = code | 0x80;
        self.interrupt_pending = true;

        // Add to key buffer
        if self.key_buffer.len() < 16 {
            self.key_buffer.push_back(code & 0x3F);
        }
    }

    /// Handle key up event
    pub fn key_up(&mut self) {
        self.key_code = 0;
    }

    /// Check if a key interrupt is pending
    pub fn has_interrupt(&self) -> bool {
        self.interrupt_pending
    }

    /// Clear key interrupt
    pub fn clear_interrupt(&mut self) {
        self.interrupt_pending = false;
    }

    /// Get key code (for register read at 0x24E)
    pub fn key_code(&self) -> u8 {
        self.key_code
    }

    /// Get key from buffer (blocking syscall)
    pub fn get_key(&mut self) -> u8 {
        self.key_buffer.pop_front().unwrap_or(0)
    }

    /// Check if key is available (non-blocking syscall)
    pub fn key_hit(&self) -> bool {
        !self.key_buffer.is_empty()
    }

    /// Clear key buffer
    pub fn clear_buffer(&mut self) {
        self.key_buffer.clear();
    }

    /// Set minimum repeat interval
    pub fn set_min_repeat_interval(&mut self, ms: u64) {
        self.min_repeat_interval = ms;
    }
}
