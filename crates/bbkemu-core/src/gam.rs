//! GAM file loader and parser

use anyhow::{bail, Result};

/// Maximum GAM file size (1920 KiB)
const MAX_GAM_SIZE: usize = 0x1E0000;

/// Parsed GAM file
pub struct GamFile {
    /// Entry point address
    pub entry_point: u16,
    /// Data section offset
    pub data_offset: u32,
    /// Game info block (bytes 0x06-0x0F)
    pub info: [u8; 10],
    /// Game code and data
    pub data: Vec<u8>,
}

impl GamFile {
    /// Parse a GAM file from raw bytes
    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() > MAX_GAM_SIZE {
            bail!("GAM file too large ({} > {})", raw.len(), MAX_GAM_SIZE);
        }

        if raw.len() < 0x46 {
            bail!("GAM file too small ({} < 0x46)", raw.len());
        }

        let entry_point = u16::from_le_bytes([raw[0x40], raw[0x41]]);
        let data_offset = u32::from_le_bytes([raw[0x42], raw[0x43], raw[0x44], raw[0x45]]);

        let mut info = [0u8; 10];
        info.copy_from_slice(&raw[0x06..0x10]);

        Ok(Self {
            entry_point,
            data_offset,
            info,
            data: raw.to_vec(),
        })
    }

    /// Get game name from info block (best effort)
    pub fn name(&self) -> String {
        // Try to extract a readable name from the info block
        self.info
            .iter()
            .filter(|&&b| b >= 0x20 && b < 0x7F)
            .map(|&b| b as char)
            .collect()
    }
}

/// System header for flash (16 bytes)
fn system_header() -> [u8; 16] {
    [
        0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x2F,
    ]
}

/// Game header for flash (16 bytes)
fn game_header(info: &[u8; 10], size: usize) -> [u8; 16] {
    let mut hdr = [0u8; 16];
    hdr[0] = 0xD0;
    hdr[1] = 0x00;
    hdr[2..12].copy_from_slice(info);
    hdr[12] = (size & 0xFF) as u8;
    hdr[13] = ((size >> 8) & 0xFF) as u8;
    hdr[14] = ((size >> 16) & 0xFF) as u8;
    hdr[15] = 0x3D;
    hdr
}
