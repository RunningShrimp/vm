//! # BIOS Interrupt Handlers
//!
//! Minimal BIOS interrupt implementation for x86 real-mode compatibility.
//! Implements INT 10h (video), INT 15h (system), and INT 16h (keyboard).

use vm_core::{GuestAddr, MMU, VmResult};
use super::realmode::RealModeRegs;
use super::vga::VgaDisplay;

/// BIOS interrupt handler
pub struct BiosInt {
    /// VGA display for INT 10h
    vga: VgaDisplay,
    /// Memory size in KB (for INT 15h)
    memory_size_kb: u16,
}

impl BiosInt {
    /// Create new BIOS interrupt handler
    pub fn new() -> Self {
        Self {
            vga: VgaDisplay::new(),
            memory_size_kb: 3 * 1024, // 3GB for x86_64
        }
    }

    /// Handle INT 10h - Video Services
    fn int10_video(&mut self, regs: &mut RealModeRegs, _mmu: &mut dyn MMU) -> VmResult<()> {
        let ah = ((regs.eax >> 8) & 0xFF) as u8;

        match ah {
            0x00 => {
                // Set video mode
                let al = (regs.eax & 0xFF) as u8;
                log::debug!("INT 10h: AH=00h (Set video mode), AL={:02X}", al);
                // For now, just clear display
                self.vga.clear();
            }
            0x01 => {
                // Set cursor shape
                let ch = ((regs.ecx >> 8) & 0xFF) as u8;
                let cl = (regs.ecx & 0xFF) as u8;
                log::debug!("INT 10h: AH=01h (Set cursor shape), CH={:02X}, CL={:02X}", ch, cl);
                // Cursor shape not implemented for text mode
            }
            0x02 => {
                // Set cursor position
                let bh = ((regs.ebx >> 8) & 0xFF) as u8; // Page number
                let dh = ((regs.edx >> 8) & 0xFF) as u8; // Row
                let dl = (regs.edx & 0xFF) as u8;       // Column
                log::debug!("INT 10h: AH=02h (Set cursor position), BH={:02X}, DH={:02X}, DL={:02X}",
                          bh, dh, dl);
                // Cursor position tracking not fully implemented
            }
            0x03 => {
                // Get cursor position
                let bh = ((regs.ebx >> 8) & 0xFF) as u8; // Page number
                log::debug!("INT 10h: AH=03h (Get cursor position), BH={:02X}", bh);
                // Return cursor at 0,0
                regs.edx = 0;
                regs.ecx = 0x0607; // Start line, end line
            }
            0x05 => {
                // Set active display page
                let al = (regs.eax & 0xFF) as u8;
                log::debug!("INT 10h: AH=05h (Set active display page), AL={:02X}", al);
                // Page switching not implemented
            }
            0x06 => {
                // Scroll up window
                let al = (regs.eax & 0xFF) as u8; // Lines to scroll (0 = clear)
                let ch = ((regs.ecx >> 8) & 0xFF) as u8; // Upper row
                let cl = (regs.ecx & 0xFF) as u8;       // Left col
                let dh = ((regs.edx >> 8) & 0xFF) as u8; // Lower row
                let dl = (regs.edx & 0xFF) as u8;       // Right col
                let bh = ((regs.ebx >> 8) & 0xFF) as u8; // Attribute

                if al == 0 {
                    // Clear window
                    log::debug!("INT 10h: AH=06h (Clear window), ({},{} to {},{}), attr={:02X}",
                              ch, cl, dh, dl, bh);
                    self.vga.clear();
                } else {
                    log::debug!("INT 10h: AH=06h (Scroll up), lines={}, window=({},{},{},{}), attr={:02X}",
                              al, ch, cl, dh, dl, bh);
                    // Scroll the display
                    for _ in 0..al {
                        self.vga.scroll_up();
                    }
                }
            }
            0x07 => {
                // Scroll down window
                let al = (regs.eax & 0xFF) as u8; // Lines to scroll
                let ch = ((regs.ecx >> 8) & 0xFF) as u8;
                let cl = (regs.ecx & 0xFF) as u8;
                let dh = ((regs.edx >> 8) & 0xFF) as u8;
                let dl = (regs.edx & 0xFF) as u8;
                let bh = ((regs.ebx >> 8) & 0xFF) as u8;
                log::debug!("INT 10h: AH=07h (Scroll down), lines={}, window=({},{},{},{}), attr={:02X}",
                          al, ch, cl, dh, dl, bh);
                // Scroll down not fully implemented
            }
            0x08 => {
                // Read character and attribute at cursor
                let bh = ((regs.ebx >> 8) & 0xFF) as u8; // Page number
                log::debug!("INT 10h: AH=08h (Read char/attr), BH={:02X}", bh);
                // Return space with default attribute
                regs.eax = 0x0720; // Attribute=0x07, Char=' '
            }
            0x09 => {
                // Write character and attribute
                let al = (regs.eax & 0xFF) as u8; // Character
                let bl = ((regs.ebx >> 8) & 0xFF) as u8; // Page/attribute
                let cx = regs.ecx as u16; // Count
                log::debug!("INT 10h: AH=09h (Write char/attr), AL={:02X}, BL={:02X}, CX={:04X}",
                          al, bl, cx);
                // Write character multiple times
                for _ in 0..cx {
                    self.vga.write_char(al as char);
                }
            }
            0x0A => {
                // Write character only
                let al = (regs.eax & 0xFF) as u8; // Character
                let cx = regs.ecx as u16; // Count
                log::debug!("INT 10h: AH=0Ah (Write char only), AL={:02X}, CX={:04X}", al, cx);
                // Write character multiple times
                for _ in 0..cx {
                    self.vga.write_char(al as char);
                }
            }
            0x0E => {
                // Write character in teletype mode
                let al = (regs.eax & 0xFF) as u8; // Character
                let bl = ((regs.ebx >> 8) & 0xFF) as u8; // Page/color
                log::debug!("INT 10h: AH=0Eh (Teletype output), AL={:02X} ('{}'), BL={:02X}",
                          al, if al >= 32 && al <= 126 { al as char } else { '?' }, bl);
                // Write character to display
                self.vga.write_char(al as char);
            }
            0x0F => {
                // Get current video mode
                log::debug!("INT 10h: AH=0Fh (Get video mode)");
                // Return text mode, 80 columns, 25 rows
                regs.eax = (regs.eax & 0xFFFF0000) | 0x0003; // Mode = 0x03 (80x25 text)
                regs.ebx = (regs.ebx & 0xFFFF0000) | 0x0000; // Page = 0
            }
            _ => {
                log::warn!("INT 10h: Unknown function AH={:02X}", ah);
            }
        }

        Ok(())
    }

    /// Handle INT 15h - System Services
    fn int15_system(&mut self, regs: &mut RealModeRegs, _mmu: &mut dyn MMU) -> VmResult<()> {
        let ah = ((regs.eax >> 8) & 0xFF) as u8;

        match ah {
            0x24 => {
                // A20 gate support
                log::debug!("INT 15h: AH=24h (A20 gate support)");
                regs.ecx = 0x0001; // A20 gate supported
                regs.eax &= 0xFF00FFFF; // Clear error byte
            }
            0x88 => {
                // Get extended memory size (above 1MB)
                log::debug!("INT 15h: AH=88h (Get extended memory)");
                // Return memory size in KB (above 1MB)
                let mem_kb = self.memory_size_kb.saturating_sub(1024); // Subtract first 1MB
                regs.eax = (regs.eax & 0xFFFF0000) | (mem_kb as u32);
                regs.eflags &= !0x0001; // Clear carry flag (success)
            }
            0xE8 => {
                // Query memory map (E820)
                let al = (regs.eax & 0xFF) as u8;
                if al == 0x01 {
                    // Check for E820 support
                    log::debug!("INT 15h: AH=E8h, AL=01h (Check E820 support)");
                    regs.eax = (regs.eax & 0xFFFFFF00) | 0x20; // E820 version
                    regs.ebx = 0; // Continuation value
                    regs.ecx = 20; // Buffer size
                    regs.edx = 0x534D4150; // 'SMAP' signature
                    regs.eflags &= !0x0001; // Clear carry (success)
                } else if al == 0x00 {
                    // Get memory map entry
                    log::debug!("INT 15h: AH=E8h, AL=00h (Get memory map entry)");
                    // For simplicity, return one entry for all memory
                    let edi = (regs.edi & 0xFFFF) as u16; // ES:DI buffer

                    // Memory map entry structure (20 bytes)
                    let base = 0u64;
                    let length = (self.memory_size_kb as u64) * 1024;
                    let entry_type = 1u32; // Usable RAM

                    // Write to buffer (simplified - would need ES:DI addressing)
                    log::debug!("E820 entry: base={:#x}, length={:#x}, type={}",
                              base, length, entry_type);

                    regs.ecx = 20; // Entry size
                    regs.ebx = 0; // No more entries
                    regs.edx = 0x534D4150; // 'SMAP'
                    regs.eflags &= !0x0001; // Clear carry (success)
                }
            }
            0xC0 => {
                // Get configuration
                log::debug!("INT 15h: AH=C0h (Get configuration)");
                // Return system configuration table
                regs.es = 0;
                regs.ebx = 0; // No configuration table
                regs.eax &= 0xFF00FFFF; // Clear error byte
                regs.eflags &= !0x0001; // Clear carry (success)
            }
            _ => {
                log::warn!("INT 15h: Unknown function AH={:02X}", ah);
                regs.eflags |= 0x0001; // Set carry (error)
            }
        }

        Ok(())
    }

    /// Handle INT 16h - Keyboard Services
    fn int16_keyboard(&mut self, regs: &mut RealModeRegs, _mmu: &mut dyn MMU) -> VmResult<()> {
        let ah = ((regs.eax >> 8) & 0xFF) as u8;

        match ah {
            0x00 => {
                // Get keystroke
                log::debug!("INT 16h: AH=00h (Get keystroke)");
                // No keyboard input available - return empty
                regs.eflags |= 0x0040; // Set zero flag
            }
            0x01 => {
                // Check keystroke
                log::debug!("INT 16h: AH=01h (Check keystroke)");
                // No keystroke available
                regs.eflags |= 0x0040; // Set zero flag (none available)
            }
            0x02 => {
                // Get shift flags
                log::debug!("INT 16h: AH=02h (Get shift flags)");
                regs.eax = (regs.eax & 0xFFFFFF00) | 0x00; // No modifiers pressed
            }
            _ => {
                log::warn!("INT 16h: Unknown function AH={:02X}", ah);
            }
        }

        Ok(())
    }

    /// Handle BIOS interrupt
    pub fn handle_int(&mut self, int_num: u8, regs: &mut RealModeRegs, mmu: &mut dyn MMU) -> VmResult<bool> {
        match int_num {
            0x10 => {
                self.int10_video(regs, mmu)?;
                Ok(true) // Handled
            }
            0x15 => {
                self.int15_system(regs, mmu)?;
                Ok(true) // Handled
            }
            0x16 => {
                self.int16_keyboard(regs, mmu)?;
                Ok(true) // Handled
            }
            0x17 => {
                // INT 17h - Parallel Port Services
                let ah = ((regs.eax >> 8) & 0xFF) as u8;
                log::debug!("INT 17h: Parallel port, AH={:02X}", ah);
                // For now, just return success (no printer)
                regs.eax = (regs.eax & 0xFF00) | 0x00; // AH=0 (no error)
                Ok(true) // Handled
            }
            0x2A => {
                // INT 2Ah - Keyboard Services
                let ah = ((regs.eax >> 8) & 0xFF) as u8;
                log::debug!("INT 2Ah: Keyboard, AH={:02X}", ah);
                // Return no keyboard data available
                regs.eax = ((0x01 as u32) << 8) | ((regs.eax) & 0xFF); // AL=unchanged, AH=1 (keyboard flag)
                regs.eflags |= 0x0001; // Set carry (error)
                Ok(true) // Handled
            }
            _ => {
                log::debug!("BIOS interrupt {:02X} not implemented", int_num);
                Ok(false) // Not handled
            }
        }
    }

    /// Sync VGA display to MMU
    pub fn sync_vga(&mut self, mmu: &mut dyn MMU) -> VmResult<()> {
        self.vga.sync_to_mmu(mmu)
    }

    /// Get VGA display content as string (for debugging)
    pub fn vga_to_string(&self) -> String {
        self.vga.to_string()
    }
}

impl Default for BiosInt {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bios_create() {
        let bios = BiosInt::new();
        assert_eq!(bios.memory_size_kb, 3 * 1024);
    }

    #[test]
    fn test_bios_vga_write() {
        let mut bios = BiosInt::new();
        bios.vga.write_str("Test");
        let content = bios.vga.to_string();
        assert!(content.contains("Test"));
    }
}
