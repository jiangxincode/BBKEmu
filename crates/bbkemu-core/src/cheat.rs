//! Cheat engine for memory manipulation

use std::collections::HashMap;

/// Cheat code entry
#[derive(Debug, Clone)]
pub struct CheatCode {
    /// Address to write to (0x0000 - 0xFFFF for RAM, or physical address)
    pub address: u32,
    /// Value to write
    pub value: u8,
    /// Whether this cheat is enabled
    pub enabled: bool,
    /// Description of the cheat
    pub description: String,
}

/// Cheat engine that manages cheat codes
pub struct CheatEngine {
    /// Active cheat codes indexed by ID
    cheats: HashMap<u32, CheatCode>,
    /// Next cheat ID
    next_id: u32,
}

impl Default for CheatEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CheatEngine {
    /// Create a new cheat engine
    pub fn new() -> Self {
        Self {
            cheats: HashMap::new(),
            next_id: 0,
        }
    }

    /// Parse a cheat code string and add it
    /// Supported formats:
    /// - "AAAAAAVV" - Write value VV to address AAAAAA (6 hex digits address, 2 hex digits value)
    /// - "AAAA VVVV" - Write 16-bit value VVVV to address AAAA
    pub fn add_cheat(&mut self, code: &str) -> Option<u32> {
        let code = code.trim().replace([' ', '-'], "");

        // Try to parse as "AAAAAAVV" format (address: 6 hex, value: 2 hex)
        if code.len() == 8 {
            if let (Ok(addr), Ok(val)) = (
                u32::from_str_radix(&code[..6], 16),
                u8::from_str_radix(&code[6..8], 16),
            ) {
                let id = self.next_id;
                self.next_id += 1;
                self.cheats.insert(
                    id,
                    CheatCode {
                        address: addr,
                        value: val,
                        enabled: true,
                        description: format!("${:06X}={:02X}", addr, val),
                    },
                );
                return Some(id);
            }
        }

        // Try GameShark format "AAAAVVVV" (address: 4 hex, value: 4 hex)
        // This writes a 16-bit value (little-endian)
        if code.len() == 8 {
            if let (Ok(addr), Ok(val)) = (
                u16::from_str_radix(&code[..4], 16),
                u16::from_str_radix(&code[4..8], 16),
            ) {
                let id = self.next_id;
                self.next_id += 1;
                let lo = (val & 0xFF) as u8;
                let hi = ((val >> 8) & 0xFF) as u8;
                self.cheats.insert(
                    id,
                    CheatCode {
                        address: addr as u32,
                        value: lo,
                        enabled: true,
                        description: format!("${:04X}={:04X}", addr, val),
                    },
                );
                // Add high byte as separate cheat
                let id2 = self.next_id;
                self.next_id += 1;
                self.cheats.insert(
                    id2,
                    CheatCode {
                        address: (addr + 1) as u32,
                        value: hi,
                        enabled: true,
                        description: format!("${:04X}={:02X}", addr + 1, hi),
                    },
                );
                return Some(id);
            }
        }

        None
    }

    /// Remove a cheat by ID
    pub fn remove_cheat(&mut self, id: u32) -> bool {
        self.cheats.remove(&id).is_some()
    }

    /// Enable or disable a cheat
    pub fn set_cheat_enabled(&mut self, id: u32, enabled: bool) -> bool {
        if let Some(cheat) = self.cheats.get_mut(&id) {
            cheat.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Clear all cheats
    pub fn clear(&mut self) {
        self.cheats.clear();
    }

    /// Apply all enabled cheats to memory
    pub fn apply_cheats(&self, ram: &mut [u8], flash: &mut [u8]) {
        for cheat in self.cheats.values() {
            if !cheat.enabled {
                continue;
            }

            let addr = cheat.address as usize;

            // RAM addresses (0x0000 - 0x7FFF)
            if addr < ram.len() {
                ram[addr] = cheat.value;
            }
            // Flash addresses (0x200000 - 0x3FFFFF, mapped to flash array starting at 0)
            else if (0x200000..0x400000).contains(&addr) {
                let flash_addr = addr - 0x200000;
                if flash_addr < flash.len() {
                    flash[flash_addr] = cheat.value;
                }
            }
        }
    }

    /// Get all cheats
    pub fn cheats(&self) -> &HashMap<u32, CheatCode> {
        &self.cheats
    }

    /// Get a cheat by ID
    pub fn get_cheat(&self, id: u32) -> Option<&CheatCode> {
        self.cheats.get(&id)
    }

    /// Check if any cheats are active
    pub fn has_active_cheats(&self) -> bool {
        self.cheats.values().any(|c| c.enabled)
    }
}
