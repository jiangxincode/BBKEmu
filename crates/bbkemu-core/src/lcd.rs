//! LCD display emulation (159x96 monochrome)

pub const LCD_WIDTH: usize = 159;
pub const LCD_HEIGHT: usize = 96;
pub const FRAMEBUFFER_SIZE: usize = LCD_WIDTH * LCD_HEIGHT;

/// LCD color theme
pub struct LcdTheme {
    /// Background color (RGB565)
    pub bg: u16,
    /// Foreground color (RGB565)
    pub fg: u16,
    /// Ghosting effect intensity (0 = off, higher = more ghosting)
    pub ghosting: u8,
}

impl LcdTheme {
    pub const GREY: Self = Self { bg: 0xD6DA, fg: 0x0000, ghosting: 20 };
    pub const GREEN: Self = Self { bg: 0x96E1, fg: 0x0882, ghosting: 20 };
    pub const BLUE: Self = Self { bg: 0x3EDD, fg: 0x09A8, ghosting: 20 };
    pub const YELLOW: Self = Self { bg: 0xF72C, fg: 0x2920, ghosting: 20 };
}

/// LCD framebuffer
pub struct Lcd {
    /// Pixel buffer (true = foreground, false = background)
    pixels: [bool; FRAMEBUFFER_SIZE],
    /// Ghosting effect accumulator
    ghosting: [i8; FRAMEBUFFER_SIZE],
    /// Cursor X position
    cursor_x: u8,
    /// Cursor Y position
    cursor_y: u8,
    /// Whether the display has been updated since last render
    dirty: bool,
}

impl Lcd {
    pub fn new() -> Self {
        Self {
            pixels: [false; FRAMEBUFFER_SIZE],
            ghosting: [0; FRAMEBUFFER_SIZE],
            cursor_x: 0,
            cursor_y: 0,
            dirty: true,
        }
    }

    /// Clear the entire display
    pub fn clear(&mut self) {
        self.pixels.fill(false);
        self.dirty = true;
    }

    /// Set a single pixel
    pub fn set_pixel(&mut self, x: u8, y: u8, color: bool) {
        if (x as usize) < LCD_WIDTH && (y as usize) < LCD_HEIGHT {
            self.pixels[y as usize * LCD_WIDTH + x as usize] = color;
            self.dirty = true;
        }
    }

    /// Get a single pixel
    pub fn get_pixel(&self, x: u8, y: u8) -> bool {
        if (x as usize) < LCD_WIDTH && (y as usize) < LCD_HEIGHT {
            self.pixels[y as usize * LCD_WIDTH + x as usize]
        } else {
            false
        }
    }

    /// Set cursor position
    pub fn set_cursor(&mut self, x: u8, y: u8) {
        self.cursor_x = x;
        self.cursor_y = y;
    }

    /// Draw a character at cursor position and advance cursor
    pub fn draw_char(&mut self, ch: u8, font: &FontData) {
        if let Some(glyph) = font.get_glyph(ch) {
            for row in 0..font.height {
                for col in 0..font.width {
                    if glyph.get_pixel(col, row) {
                        self.set_pixel(
                            self.cursor_x + col,
                            self.cursor_y + row,
                            true,
                        );
                    }
                }
            }
            self.cursor_x += font.width;
        }
    }

    /// Draw a string at cursor position
    pub fn draw_string(&mut self, s: &[u8], font: &FontData) {
        for &ch in s {
            self.draw_char(ch, font);
        }
    }

    /// Fill a rectangle
    pub fn fill_rect(&mut self, x: u8, y: u8, w: u8, h: u8, color: bool) {
        for dy in 0..h {
            for dx in 0..w {
                self.set_pixel(x + dx, y + dy, color);
            }
        }
    }

    /// Scroll the display up by N lines
    pub fn scroll_up(&mut self, lines: u8) {
        let offset = lines as usize * LCD_WIDTH;
        if offset < FRAMEBUFFER_SIZE {
            self.pixels.copy_within(offset.., 0);
            self.pixels[FRAMEBUFFER_SIZE - offset..].fill(false);
        }
        self.dirty = true;
    }

    /// Render to RGB565 buffer for frontend display
    pub fn render(&self, buf: &mut [u16], theme: &LcdTheme) {
        for i in 0..FRAMEBUFFER_SIZE {
            let color = if self.pixels[i] { theme.fg } else { theme.bg };
            buf[i] = color;
        }
    }

    /// Render with ghosting effect
    pub fn render_with_ghosting(&mut self, buf: &mut [u16], theme: &LcdTheme) {
        let bg_r = ((theme.bg >> 11) & 0x1F) as i16;
        let bg_g = ((theme.bg >> 6) & 0x1F) as i16;
        let bg_b = (theme.bg & 0x1F) as i16;
        let fg_r = ((theme.fg >> 11) & 0x1F) as i16;
        let fg_g = ((theme.fg >> 6) & 0x1F) as i16;
        let fg_b = (theme.fg & 0x1F) as i16;

        for i in 0..FRAMEBUFFER_SIZE {
            // Update ghosting accumulator
            if self.pixels[i] {
                if self.ghosting[i] < theme.ghosting as i8 {
                    self.ghosting[i] += 1;
                }
            } else {
                if self.ghosting[i] > 0 {
                    self.ghosting[i] -= 1;
                }
            }

            // Blend colors
            let a = self.ghosting[i] as f32 / theme.ghosting.max(1) as f32;
            let r = ((1.0 - a) * bg_r as f32 + a * fg_r as f32) as u16 & 0x1F;
            let g = ((1.0 - a) * bg_g as f32 + a * fg_g as f32) as u16 & 0x1F;
            let b = ((1.0 - a) * bg_b as f32 + a * fg_b as f32) as u16 & 0x1F;
            buf[i] = (r << 11) | (g << 6) | b;
        }
    }

    /// Check if display has been updated
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear dirty flag
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

/// Bitmap font data
pub struct FontData {
    pub width: u8,
    pub height: u8,
    /// Glyph data, each glyph is width*height bits packed into bytes
    glyphs: Vec<u8>,
}

impl FontData {
    pub fn new(width: u8, height: u8) -> Self {
        Self {
            width,
            height,
            glyphs: vec![0; 256 * ((width as usize * height as usize + 7) / 8)],
        }
    }

    /// Get glyph bitmap for a character
    pub fn get_glyph(&self, ch: u8) -> Option<GlyphView> {
        let glyph_size = (self.width as usize * self.height as usize + 7) / 8;
        let offset = ch as usize * glyph_size;
        if offset + glyph_size <= self.glyphs.len() {
            Some(GlyphView {
                data: &self.glyphs[offset..offset + glyph_size],
                width: self.width,
                height: self.height,
            })
        } else {
            None
        }
    }
}

/// View into a single glyph's bitmap data
pub struct GlyphView<'a> {
    data: &'a [u8],
    width: u8,
    height: u8,
}

impl<'a> GlyphView<'a> {
    /// Check if a pixel is set in the glyph
    pub fn get_pixel(&self, x: u8, y: u8) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let bit = y as usize * self.width as usize + x as usize;
        let byte = bit / 8;
        let bit = bit % 8;
        if byte < self.data.len() {
            (self.data[byte] >> (7 - bit)) & 1 != 0
        } else {
            false
        }
    }
}
