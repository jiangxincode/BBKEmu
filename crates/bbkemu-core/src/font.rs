//! Built-in 8x8 bitmap font for BBK emulator
//! Covers ASCII characters 0x20-0x7E

/// Built-in font data (8x8 pixels per character)
/// Each character is 8 bytes, one byte per row, MSB first
pub const FONT_DATA: &[u8; 768] = include_bytes!("font.bin");

/// Get font bitmap for a character
pub fn get_font_bitmap(ch: u8) -> [u8; 8] {
    let mut bitmap = [0u8; 8];

    // Map character to font index
    let index = match ch {
        0x20..=0x7E => (ch - 0x20) as usize, // ASCII printable
        _ => 0, // Default to space for unknown characters
    };

    let offset = index * 8;
    if offset + 8 <= FONT_DATA.len() {
        bitmap.copy_from_slice(&FONT_DATA[offset..offset + 8]);
    }

    bitmap
}
