//! # EFI (Extensible Firmware Interface) Support
//!
//! Implements EFI firmware and Graphics Output Protocol for modern Linux kernel support.
//! This enables framebuffer graphics display for the Ubuntu graphical installer.

use std::sync::{Arc, Mutex};
use vm_core::{GuestAddr, MMU, VmError, VmResult};

/// EFI System Table signature
pub const EFI_SYSTEM_TABLE_SIGNATURE: u64 = 0x5453595320494249; // "IBI.2.0" + header

/// EFI Graphics Output Protocol GUID
pub const EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID: [u8; 16] = [
    0xde, 0xa9, 0x42, 0x90, 0xdc, 0x23, 0x13, 0x4f, 0x98, 0x61, 0x42, 0x98, 0xc5, 0x74, 0x88, 0x8d,
];

/// EFI framebuffer address (use high memory address)
pub const EFI_FRAMEBUFFER_ADDR: u64 = 0xF0000000;

/// EFI framebuffer size (1920x1080x4 bytes = 8.3MB, rounded to 16MB)
pub const EFI_FRAMEBUFFER_SIZE: u64 = 16 * 1024 * 1024;

/// Pixel format for EFI framebuffer
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum EfiPixelFormat {
    RedGreenBlue8BitPerColor = 0, // RGB 8-bit per color
    BlueGreenRed8BitPerColor = 1, // BGR 8-bit per color
    BitMask = 2,
    BgrOnly = 3,
    FormatMax = 4,
}

/// EFI Graphics Output Mode Information
#[repr(C)]
#[derive(Debug, Clone)]
pub struct EfiGraphicsOutputModeInfo {
    pub version: u32,
    pub horizontal_resolution: u32,
    pub vertical_resolution: u32,
    pub pixel_format: EfiPixelFormat,
    pub pixel_information: EfiPixelInformation,
    pub bytes_per_scanline: u32,
}

/// EFI Pixel Information (RGB masks)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct EfiPixelInformation {
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    pub reserved_mask: u32,
}

/// EFI Graphics Output Protocol Mode Structure
#[repr(C)]
#[derive(Debug)]
pub struct EfiGraphicsOutputProtocolMode {
    pub max_mode: u32,
    pub mode: u32,
    pub info: *mut EfiGraphicsOutputModeInfo,
    pub size_of_info: usize,
    pub frame_buffer_base: u64,
    pub frame_buffer_size: usize,
}

/// EFI Graphics Output Protocol
#[repr(C)]
pub struct EfiGraphicsOutputProtocol {
    pub query_mode: usize, // Function pointer
    pub set_mode: usize,   // Function pointer
    pub blt: usize,        // Function pointer (Block Transfer)
    pub mode: *mut EfiGraphicsOutputProtocolMode,
}

/// EFI Runtime Services
pub struct EfiRuntime {
    /// Framebuffer address
    framebuffer_addr: u64,
    /// Framebuffer size
    framebuffer_size: usize,
    /// Current graphics mode
    current_width: u32,
    current_height: u32,
    /// EFI system table address (for kernel handoff)
    system_table_addr: u64,
    /// Framebuffer data (for display output)
    framebuffer_data: Arc<Mutex<Vec<u8>>>,
}

impl EfiRuntime {
    /// Create new EFI runtime
    pub fn new() -> Self {
        Self {
            framebuffer_addr: EFI_FRAMEBUFFER_ADDR,
            framebuffer_size: EFI_FRAMEBUFFER_SIZE as usize,
            current_width: 1920,
            current_height: 1080,
            system_table_addr: 0,
            framebuffer_data: Arc::new(Mutex::new(vec![0u8; EFI_FRAMEBUFFER_SIZE as usize])),
        }
    }

    /// Initialize EFI runtime and map framebuffer
    pub fn initialize(&mut self, mmu: &mut dyn MMU) -> VmResult<()> {
        log::info!("=== Initializing EFI Runtime ===");
        log::info!(
            "Framebuffer: {:#010X} ({} MB)",
            self.framebuffer_addr,
            self.framebuffer_size / 1024 / 1024
        );
        log::info!("Resolution: {}x{}", self.current_width, self.current_height);

        // Map framebuffer memory
        let fb_addr = GuestAddr(self.framebuffer_addr);
        for i in 0..self.framebuffer_size {
            mmu.write(GuestAddr(fb_addr.0 + i as u64), 0, 1)?;
        }

        // Create EFI system table in memory
        self.system_table_addr = 0x10000; // Place in low memory
        self.setup_system_table(mmu)?;

        // Setup Graphics Output Protocol
        self.setup_graphics_protocol(mmu)?;

        log::info!("✓ EFI Runtime initialized");
        log::info!("  System Table: {:#010X}", self.system_table_addr);
        log::info!(
            "  Graphics Protocol: {:#010X}",
            self.framebuffer_addr + 0x1000
        );

        Ok(())
    }

    /// Setup EFI System Table
    fn setup_system_table(&self, mmu: &mut dyn MMU) -> VmResult<()> {
        let table_addr = GuestAddr(self.system_table_addr);

        // Write system table signature
        let sig = EFI_SYSTEM_TABLE_SIGNATURE.to_le_bytes();
        for (i, &byte) in sig.iter().enumerate() {
            mmu.write(GuestAddr(table_addr.0 + i as u64), byte as u64, 1)?;
        }

        // Set revision (2.70 = EFI 2.70)
        let revision_addr = table_addr.0 + 0x40;
        mmu.write(GuestAddr(revision_addr), 0x00020070, 4)?;

        log::info!("✓ EFI System Table created at {:#010X}", table_addr.0);
        Ok(())
    }

    /// Setup Graphics Output Protocol
    fn setup_graphics_protocol(&self, mmu: &mut dyn MMU) -> VmResult<()> {
        let proto_addr = GuestAddr(self.framebuffer_addr + 0x1000);

        // Create mode info structure
        let mode_info = EfiGraphicsOutputModeInfo {
            version: 1,
            horizontal_resolution: self.current_width,
            vertical_resolution: self.current_height,
            pixel_format: EfiPixelFormat::BlueGreenRed8BitPerColor,
            pixel_information: EfiPixelInformation {
                red_mask: 0x00FF0000,
                green_mask: 0x0000FF00,
                blue_mask: 0x000000FF,
                reserved_mask: 0xFF000000,
            },
            bytes_per_scanline: self.current_width * 4,
        };

        // Write mode info to memory (simplified)
        let info_addr = proto_addr.0 + 0x100;
        self.write_mode_info(mmu, GuestAddr(info_addr), &mode_info)?;

        // Setup mode structure
        let mode_addr = proto_addr.0 + 0x200;
        self.write_mode_structure(mmu, GuestAddr(mode_addr))?;

        log::info!("✓ EFI Graphics Output Protocol at {:#010X}", proto_addr.0);
        log::info!("  Mode: {}x{}x32", self.current_width, self.current_height);

        Ok(())
    }

    /// Write mode info structure to memory
    fn write_mode_info(
        &self,
        mmu: &mut dyn MMU,
        addr: GuestAddr,
        info: &EfiGraphicsOutputModeInfo,
    ) -> VmResult<()> {
        // Simplified: just write the structure
        let bytes = unsafe {
            std::slice::from_raw_parts(
                info as *const _ as *const u8,
                std::mem::size_of::<EfiGraphicsOutputModeInfo>(),
            )
        };

        for (i, &byte) in bytes.iter().enumerate() {
            mmu.write(GuestAddr(addr.0 + i as u64), byte as u64, 1)?;
        }

        Ok(())
    }

    /// Write mode structure to memory
    fn write_mode_structure(&self, mmu: &mut dyn MMU, addr: GuestAddr) -> VmResult<()> {
        let fb_base = self.framebuffer_addr;
        let fb_size = self.framebuffer_size;

        // Max mode (1 mode available)
        mmu.write(addr, 1, 4)?;

        // Current mode (0)
        mmu.write(GuestAddr(addr.0 + 4), 0, 4)?;

        // Info pointer
        mmu.write(GuestAddr(addr.0 + 8), addr.0 - 0x100, 8)?;

        // Framebuffer base
        mmu.write(GuestAddr(addr.0 + 24), fb_base, 8)?;

        // Framebuffer size
        mmu.write(GuestAddr(addr.0 + 32), fb_size as u64, 8)?;

        Ok(())
    }

    /// Get framebuffer address for kernel
    pub fn get_framebuffer_info(&self) -> (u64, u32, u32, u32) {
        (
            self.framebuffer_addr,
            self.current_width,
            self.current_height,
            32,
        )
    }

    /// Write pixel to framebuffer
    pub fn write_pixel(&self, x: u32, y: u32, color: u32) -> VmResult<()> {
        if x >= self.current_width || y >= self.current_height {
            return Ok(());
        }

        let offset = (y * self.current_width + x) as usize * 4;
        let mut data = self.framebuffer_data.lock().unwrap();

        data[offset] = (color & 0xFF) as u8;
        data[offset + 1] = ((color >> 8) & 0xFF) as u8;
        data[offset + 2] = ((color >> 16) & 0xFF) as u8;
        data[offset + 3] = ((color >> 24) & 0xFF) as u8;

        Ok(())
    }

    /// Get system table address (for kernel handoff)
    pub fn get_system_table_addr(&self) -> u64 {
        self.system_table_addr
    }
}

impl Default for EfiRuntime {
    fn default() -> Self {
        Self::new()
    }
}
