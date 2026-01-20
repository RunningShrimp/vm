//! # VGA Text Mode Display
//!
//! Minimal VGA text mode emulation for displaying boot messages and installer UI.
//! Implements the standard 80x25 character display at physical address 0xB8000.

use vm_core::{GuestAddr, MMU, VmError, VmResult};

/// VGA text mode dimensions
pub const VGA_WIDTH: usize = 80;
pub const VGA_HEIGHT: usize = 25;
pub const VGA_SIZE: usize = VGA_WIDTH * VGA_HEIGHT;

/// VGA text mode buffer physical address
pub const VGA_BUFFER_ADDR: u64 = 0xB8000;

/// VGA character attribute bits
pub const VGA_ATTR_FG_MASK: u8 = 0x0F;
pub const VGA_ATTR_BG_MASK: u8 = 0x70;
pub const VGA_ATTR_BLINK: u8 = 0x80;

/// VGA foreground colors
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum VgaColor {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    LightMagenta = 13,
    Yellow = 14,
    White = 15,
}

/// VGA text mode character with attribute
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct VgaChar {
    /// ASCII character
    pub ascii: u8,
    /// Attribute byte (color, blinking, etc.)
    pub attr: u8,
}

impl VgaChar {
    /// Create new VGA character with default attribute
    pub fn new(ascii: char) -> Self {
        Self {
            ascii: ascii as u8,
            attr: Self::default_attr(),
        }
    }

    /// Create colored VGA character
    pub fn with_color(ascii: char, foreground: VgaColor, background: VgaColor) -> Self {
        Self {
            ascii: ascii as u8,
            attr: (background as u8) << 4 | (foreground as u8),
        }
    }

    /// Get default attribute (light gray on black)
    fn default_attr() -> u8 {
        0x0F // Light gray (15) on black (0)
    }

    /// Convert to u16 for storage
    pub fn to_u16(self) -> u16 {
        ((self.attr as u16) << 8) | (self.ascii as u16)
    }

    /// Create from u16
    pub fn from_u16(val: u16) -> Self {
        Self {
            ascii: (val & 0xFF) as u8,
            attr: ((val >> 8) & 0xFF) as u8,
        }
    }
}

/// VGA text mode display
pub struct VgaDisplay {
    /// Display buffer
    buffer: [VgaChar; VGA_SIZE],
    /// Cursor position
    cursor_x: usize,
    cursor_y: usize,
    /// Display is dirty (needs update)
    dirty: bool,
}

impl VgaDisplay {
    /// Create new VGA display
    pub fn new() -> Self {
        Self {
            buffer: [VgaChar::new(' '); VGA_SIZE],
            cursor_x: 0,
            cursor_y: 0,
            dirty: false,
        }
    }

    /// Clear display
    pub fn clear(&mut self) {
        self.buffer = [VgaChar::new(' '); VGA_SIZE];
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.dirty = true;
    }

    /// Write character at current cursor position
    pub fn write_char(&mut self, c: char) {
        match c {
            '\n' => {
                self.cursor_x = 0;
                self.cursor_y += 1;
                if self.cursor_y >= VGA_HEIGHT {
                    self.scroll_up();
                    self.cursor_y = VGA_HEIGHT - 1;
                }
            }
            '\r' => {
                self.cursor_x = 0;
            }
            '\t' => {
                // Tab to next 8-column boundary
                self.cursor_x = (self.cursor_x + 8) & !7;
                if self.cursor_x >= VGA_WIDTH {
                    self.cursor_x = 0;
                    self.cursor_y += 1;
                }
            }
            c if c.is_ascii() && c != '\x00' => {
                let idx = self.cursor_y * VGA_WIDTH + self.cursor_x;
                if idx < VGA_SIZE {
                    self.buffer[idx] = VgaChar::new(c);
                    self.cursor_x += 1;
                    if self.cursor_x >= VGA_WIDTH {
                        self.cursor_x = 0;
                        self.cursor_y += 1;
                        if self.cursor_y >= VGA_HEIGHT {
                            self.scroll_up();
                            self.cursor_y = VGA_HEIGHT - 1;
                        }
                    }
                }
                self.dirty = true;
            }
            _ => {
                // Ignore non-ASCII characters
            }
        }
    }

    /// Write string
    pub fn write_str(&mut self, s: &str) {
        for c in s.chars() {
            self.write_char(c);
        }
    }

    /// Scroll display up by one line
    pub fn scroll_up(&mut self) {
        // Move all rows up
        for y in 0..(VGA_HEIGHT - 1) {
            for x in 0..VGA_WIDTH {
                let src = (y + 1) * VGA_WIDTH + x;
                let dst = y * VGA_WIDTH + x;
                self.buffer[dst] = self.buffer[src];
            }
        }

        // Clear last row
        for x in 0..VGA_WIDTH {
            self.buffer[(VGA_HEIGHT - 1) * VGA_WIDTH + x] = VgaChar::new(' ');
        }

        self.dirty = true;
    }

    /// Sync display buffer to MMU
    pub fn sync_to_mmu(&mut self, mmu: &mut dyn MMU) -> VmResult<()> {
        if !self.dirty {
            return Ok(());
        }

        let base_addr = GuestAddr(VGA_BUFFER_ADDR);

        for (i, &vga_char) in self.buffer.iter().enumerate() {
            let addr = GuestAddr(base_addr.0 + (i * 2) as u64);
            let val = vga_char.to_u16();
            mmu.write(addr, val as u64, 2)?;
        }

        self.dirty = false;
        Ok(())
    }

    /// Read display content to string (for debugging)
    pub fn to_string(&self) -> String {
        let mut result = String::new();
        for y in 0..VGA_HEIGHT {
            for x in 0..VGA_WIDTH {
                let idx = y * VGA_WIDTH + x;
                let c = self.buffer[idx].ascii as char;
                if c == '\0' {
                    break;
                }
                result.push(c);
            }
            result.push('\n');
        }
        result
    }

    /// Check if display is dirty
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
}

impl Default for VgaDisplay {
    fn default() -> Self {
        Self::new()
    }
}

/// Global VGA display instance (simplified)
static mut VGA_DISPLAY: Option<VgaDisplay> = None;

/// Initialize VGA display
pub fn vga_init() {
    unsafe {
        VGA_DISPLAY = Some(VgaDisplay::new());
    }
    log::info!("VGA display initialized: {}x{}", VGA_WIDTH, VGA_HEIGHT);
}

/// Write string to VGA display
pub fn vga_write_str(s: &str) {
    unsafe {
        if let Some(ref mut display) = VGA_DISPLAY {
            display.write_str(s);
        }
    }
}

/// Sync VGA display to MMU
pub fn vga_sync(mmu: &mut dyn MMU) -> VmResult<()> {
    unsafe {
        if let Some(ref mut display) = VGA_DISPLAY {
            display.sync_to_mmu(mmu)?;
        }
    }
    Ok(())
}

/// Get VGA display content as string
pub fn vga_to_string() -> String {
    unsafe {
        if let Some(ref display) = VGA_DISPLAY {
            display.to_string()
        } else {
            String::new()
        }
    }
}

/// Read VGA buffer from MMU and format for display
pub fn vga_read_from_mmu(mmu: &dyn MMU) -> String {
    let base_addr = GuestAddr(VGA_BUFFER_ADDR);
    let mut result = String::new();

    result.push_str("╔");
    result.push_str(&"═".repeat(78));
    result.push_str("╗\n");

    for y in 0..VGA_HEIGHT {
        result.push_str("║");
        for x in 0..VGA_WIDTH {
            let offset = (y * VGA_WIDTH + x) * 2;
            let addr = GuestAddr(base_addr.0 + offset as u64);

            // 读取attribute和character
            let attr_val = mmu.read(addr, 1).unwrap_or(0);
            let ch_val = mmu.read(GuestAddr(addr.0 + 1), 1).unwrap_or(0);
            let attr = attr_val as u8;
            let ch = ch_val as u8;

            // 只显示可打印字符
            if ch >= 32 && ch <= 126 {
                result.push(ch as char);
            } else if ch == 0 {
                result.push(' ');
            } else {
                result.push('·');
            }
        }
        result.push_str("║\n");
    }

    result.push_str("╚");
    result.push_str(&"═".repeat(78));
    result.push_str("╝\n");

    result
}

/// Save VGA display to file
pub fn vga_save_to_file(mmu: &dyn MMU, path: &str) -> Result<(), String> {
    let content = vga_read_from_mmu(mmu);
    std::fs::write(path, content).map_err(|e| format!("Failed to save VGA display: {}", e))?;

    log::info!("VGA display saved to: {}", path);
    Ok(())
}

/// Print VGA display to console (for debugging)
pub fn vga_print(mmu: &dyn MMU) {
    let content = vga_read_from_mmu(mmu);
    log::info!("VGA Display:\n{}", content);
}

/// Get VGA display as formatted string with border
pub fn vga_format_border(mmu: &dyn MMU) -> String {
    vga_read_from_mmu(mmu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vga_char() {
        let c = VgaChar::new('A');
        assert_eq!(c.ascii, b'A');
        assert_eq!(c.to_u16() & 0xFF, b'A' as u16);

        let colored = VgaChar::with_color('B', VgaColor::LightGreen, VgaColor::Blue);
        assert_eq!(colored.ascii, b'B');
        assert_eq!(colored.attr, 0x2A); // Blue (2) << 4 | LightGreen (10)
    }

    #[test]
    fn test_vga_display() {
        let mut display = VgaDisplay::new();
        display.write_str("Hello\nWorld");
        assert!(display.is_dirty());

        let s = display.to_string();
        assert!(s.contains("Hello"));
        assert!(s.contains("World"));
    }
}
