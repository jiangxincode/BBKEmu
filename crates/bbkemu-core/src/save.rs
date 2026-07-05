//! Save state and SRAM management

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Save state data
#[derive(Serialize, Deserialize)]
pub struct SaveState {
    /// RAM contents (use Vec for serde compatibility)
    pub ram: Vec<u8>,
    /// Flash contents (save area only)
    pub flash: Vec<u8>,
    /// CPU registers
    pub cpu: CpuState,
    /// Bank switch state
    pub bank_switch: BankState,
    /// Model identifier
    pub bank_sys_d: u16,
}

/// CPU state for serialization
#[derive(Serialize, Deserialize)]
pub struct CpuState {
    pub pc: u16,
    pub sp: u8,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub status: u8,
    pub cycles: u64,
}

/// Bank switch state for serialization
#[derive(Serialize, Deserialize)]
pub struct BankState {
    pub banks: Vec<u32>,
    pub selected: u8,
}

impl SaveState {
    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        Ok(bincode::deserialize(data)?)
    }
}
