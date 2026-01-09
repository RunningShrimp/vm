//! # VESA BIOS Extensions (VBE) for Graphics Mode Support
//!
//! Implements VESA VBE 2.0+ for high-resolution graphics modes.
//! This enables framebuffer graphics for the Ubuntu graphical installer.

use vm_core::{GuestAddr, MMU, VmError, VmResult};

/// VESA BIOS Extensions signature
pub const VBE_SIGNATURE: &[u8; 4] = b"VESA";

/// VBE 2.0+ Controller Information
#[repr(C)]
#[derive(Debug, Clone)]
pub struct VbeControllerInfo {
    pub signature: [u8; 4],          // "VESA"
    pub version: u16,                // VBE version
    pub oem_string: [u8; 128],       // OEM string
    pub capabilities: u32,           // Capabilities flags
    pub video_mode_ptr: u32,         // Pointer to supported modes
    pub total_memory: u16,           // Total memory in 64KB units
    pub oem_software_rev: u16,       // OEM software revision
    pub oem_vendor_name: [u8; 128],  // OEM vendor name
    pub oem_product_name: [u8; 128], // OEM product name
    pub oem_product_rev: u16,        // OEM product revision
    pub reserved: [u8; 222],         // Reserved
    pub oem_data: [u8; 256],         // OEM data
}

/// VBE Mode Information
#[repr(C)]
#[derive(Debug, Clone)]
pub struct VbeModeInfo {
    pub attributes: u16,            // Mode attributes
    pub win_a_attributes: u8,       // Window A attributes
    pub win_b_attributes: u8,       // Window B attributes
    pub win_granularity: u16,       // Window granularity
    pub win_size: u16,              // Window size
    pub win_a_segment: u16,         // Window A segment
    pub win_b_segment: u16,         // Window B segment
    pub win_func_ptr: u32,          // Window function pointer
    pub bytes_per_scanline: u16,    // Bytes per scanline
    pub x_resolution: u16,          // Horizontal resolution
    pub y_resolution: u16,          // Vertical resolution
    pub x_char_size: u8,            // Character cell width
    pub y_char_size: u8,            // Character cell height
    pub number_of_planes: u8,       // Number of memory planes
    pub bits_per_pixel: u8,         // Bits per pixel
    pub number_of_banks: u8,        // Number of banks
    pub memory_model: u8,           // Memory model type
    pub bank_size: u8,              // Bank size in KB
    pub number_of_image_pages: u8,  // Number of images
    pub reserved0: u8,              // Reserved
    pub red_mask_size: u8,          // Red mask size
    pub red_field_position: u8,     // Red field position
    pub green_mask_size: u8,        // Green mask size
    pub green_field_position: u8,   // Green field position
    pub blue_mask_size: u8,         // Blue mask size
    pub blue_field_position: u8,    // Blue field position
    pub rsvd_mask_size: u8,         // Reserved mask size
    pub rsvd_field_position: u8,    // Reserved field position
    pub direct_color_mode_info: u8, // Direct color mode info
    pub phys_base_ptr: u32,         // Physical address for LFB
    pub offscreen_mem_offset: u32,  // Offscreen memory offset
    pub offscreen_mem_size: u16,    // Offscreen memory size in KB
    pub reserved1: [u8; 206],       // Reserved
}

/// VBE Memory Models
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum VbeMemoryModel {
    TextMode = 0,
    CGAGraphics = 1,
    HerculesGraphics = 2,
    Planar = 3,
    PackedPixel = 4,
    DirectColor = 5,
    YUV = 6,
}

/// VBE Mode Attributes
pub const VBE_MODE_ATTR_SUPPORTED: u16 = 0x0001;
pub const VBE_MODE_ATTR_OPTIMAL: u16 = 0x0002;
pub const VBE_MODE_ATTR_VGA_COMPATIBLE: u16 = 0x0004;
pub const VBE_MODE_ATTR_LINEAR_FRAMEBUFFER: u16 = 0x0080;
pub const VBE_MODE_ATTR_GRAPHICS_MODE: u16 = 0x0010;

/// Common VESA Graphics Modes
pub const VBE_MODE_640X480X8BIT: u16 = 0x101;
pub const VBE_MODE_800X600X8BIT: u16 = 0x103;
pub const VBE_MODE_1024X768X8BIT: u16 = 0x105;
pub const VBE_MODE_1280X1024X8BIT: u16 = 0x107;
pub const VBE_MODE_1920X1080X24BIT: u16 = 0x11B;

/// VESA BIOS Extensions handler
pub struct VbeHandler {
    /// Current graphics mode
    current_mode: Option<VbeModeInfo>,
    /// Framebuffer address
    framebuffer_addr: u64,
    /// Framebuffer size
    framebuffer_size: usize,
}

impl VbeHandler {
    /// Create new VBE handler
    pub fn new() -> Self {
        Self {
            current_mode: None,
            framebuffer_addr: 0xE0000000, // Default LFB address
            framebuffer_size: 0,
        }
    }

    /// Get VBE controller information
    pub fn get_controller_info(&self) -> VbeControllerInfo {
        VbeControllerInfo {
            signature: *b"VESA",
            version: 0x0200, // VBE 2.0
            oem_string: {
                let mut buf = [0u8; 128];
                let oem = b"RustVM VBE 2.0\0";
                buf[..oem.len()].copy_from_slice(oem);
                buf
            },
            capabilities: 0x00000020, // Supports LFB
            video_mode_ptr: 0,
            total_memory: 256, // 16MB in 64KB units
            oem_software_rev: 0x0100,
            oem_vendor_name: {
                let mut buf = [0u8; 128];
                let vendor = b"RustVM\0";
                buf[..vendor.len()].copy_from_slice(vendor);
                buf
            },
            oem_product_name: {
                let mut buf = [0u8; 128];
                let product = b"VBE Emulator\0";
                buf[..product.len()].copy_from_slice(product);
                buf
            },
            oem_product_rev: 0x0100,
            reserved: [0; 222],
            oem_data: [0; 256],
        }
    }

    /// Get mode information for a specific VBE mode
    pub fn get_mode_info(&self, mode: u16) -> VbeModeInfo {
        match mode {
            VBE_MODE_1920X1080X24BIT => {
                VbeModeInfo {
                    attributes: VBE_MODE_ATTR_SUPPORTED
                        | VBE_MODE_ATTR_OPTIMAL
                        | VBE_MODE_ATTR_LINEAR_FRAMEBUFFER
                        | VBE_MODE_ATTR_GRAPHICS_MODE,
                    win_a_attributes: 0,
                    win_b_attributes: 0,
                    win_granularity: 0,
                    win_size: 0,
                    win_a_segment: 0,
                    win_b_segment: 0,
                    win_func_ptr: 0,
                    bytes_per_scanline: 1920 * 4, // 1920 pixels * 4 bytes (32-bit)
                    x_resolution: 1920,
                    y_resolution: 1080,
                    x_char_size: 8,
                    y_char_size: 16,
                    number_of_planes: 1,
                    bits_per_pixel: 32,
                    number_of_banks: 1,
                    memory_model: VbeMemoryModel::DirectColor as u8,
                    bank_size: 0,
                    number_of_image_pages: 1,
                    reserved0: 0,
                    red_mask_size: 8,
                    red_field_position: 16,
                    green_mask_size: 8,
                    green_field_position: 8,
                    blue_mask_size: 8,
                    blue_field_position: 0,
                    rsvd_mask_size: 8,
                    rsvd_field_position: 24,
                    direct_color_mode_info: 0,
                    phys_base_ptr: 0xE0000000,
                    offscreen_mem_offset: 0,
                    offscreen_mem_size: 0,
                    reserved1: [0; 206],
                }
            }
            VBE_MODE_1024X768X8BIT => {
                VbeModeInfo {
                    attributes: VBE_MODE_ATTR_SUPPORTED
                        | VBE_MODE_ATTR_OPTIMAL
                        | VBE_MODE_ATTR_LINEAR_FRAMEBUFFER
                        | VBE_MODE_ATTR_GRAPHICS_MODE,
                    win_a_attributes: 0,
                    win_b_attributes: 0,
                    win_granularity: 0,
                    win_size: 0,
                    win_a_segment: 0,
                    win_b_segment: 0,
                    win_func_ptr: 0,
                    bytes_per_scanline: 1024, // 1024 pixels * 1 byte (8-bit)
                    x_resolution: 1024,
                    y_resolution: 768,
                    x_char_size: 8,
                    y_char_size: 16,
                    number_of_planes: 1,
                    bits_per_pixel: 8,
                    number_of_banks: 1,
                    memory_model: VbeMemoryModel::DirectColor as u8,
                    bank_size: 0,
                    number_of_image_pages: 1,
                    reserved0: 0,
                    red_mask_size: 0,
                    red_field_position: 0,
                    green_mask_size: 0,
                    green_field_position: 0,
                    blue_mask_size: 0,
                    blue_field_position: 0,
                    rsvd_mask_size: 0,
                    rsvd_field_position: 0,
                    direct_color_mode_info: 0,
                    phys_base_ptr: 0xE0000000,
                    offscreen_mem_offset: 0,
                    offscreen_mem_size: 0,
                    reserved1: [0; 206],
                }
            }
            _ => {
                // Default to 1024x768x8
                self.get_mode_info(VBE_MODE_1024X768X8BIT)
            }
        }
    }

    /// Set VBE graphics mode
    pub fn set_mode(&mut self, mode: u16, mmu: &mut dyn MMU) -> VmResult<()> {
        log::info!("Setting VBE mode: {:#06X}", mode);

        let mode_info = self.get_mode_info(mode);

        // Calculate framebuffer size
        let fb_size = mode_info.x_resolution as usize
            * mode_info.y_resolution as usize
            * ((mode_info.bits_per_pixel as usize) / 8);

        let x_res = mode_info.x_resolution;
        let y_res = mode_info.y_resolution;
        let bpp = mode_info.bits_per_pixel;
        let fb_base = mode_info.phys_base_ptr;

        self.framebuffer_size = fb_size;
        self.current_mode = Some(mode_info);

        log::info!("VBE Mode set: {}x{}x{}bpp", x_res, y_res, bpp);
        log::info!(
            "Framebuffer: {}x{} @ {:#010X}, size: {} bytes",
            x_res,
            y_res,
            fb_base,
            fb_size
        );

        // Initialize framebuffer with black
        let fb_addr = GuestAddr(fb_base as u64);
        for i in 0..fb_size {
            mmu.write(GuestAddr(fb_addr.0 + i as u64), 0, 1)?;
        }

        Ok(())
    }

    /// Write pixel to framebuffer
    pub fn write_pixel(&self, mmu: &mut dyn MMU, x: u16, y: u16, color: u32) -> VmResult<()> {
        if let Some(ref mode) = self.current_mode {
            if x >= mode.x_resolution || y >= mode.y_resolution {
                return Ok(());
            }

            let offset = (y as usize * mode.bytes_per_scanline as usize
                + x as usize * (mode.bits_per_pixel as usize / 8)) as u64;

            let addr = GuestAddr(self.framebuffer_addr + offset);

            // Write color in RGB format (little-endian)
            match mode.bits_per_pixel {
                32 => {
                    // RGBA/BGRX format
                    mmu.write(addr, (color & 0xFF) as u64, 1)?;
                    mmu.write(GuestAddr(addr.0 + 1), ((color >> 8) & 0xFF) as u64, 1)?;
                    mmu.write(GuestAddr(addr.0 + 2), ((color >> 16) & 0xFF) as u64, 1)?;
                    mmu.write(GuestAddr(addr.0 + 3), ((color >> 24) & 0xFF) as u64, 1)?;
                }
                24 => {
                    // RGB format
                    mmu.write(addr, (color & 0xFF) as u64, 1)?;
                    mmu.write(GuestAddr(addr.0 + 1), ((color >> 8) & 0xFF) as u64, 1)?;
                    mmu.write(GuestAddr(addr.0 + 2), ((color >> 16) & 0xFF) as u64, 1)?;
                }
                8 => {
                    // Palette index
                    mmu.write(addr, (color & 0xFF) as u64, 1)?;
                }
                _ => {
                    return Err(VmError::Core(vm_core::CoreError::Internal {
                        message: format!("Unsupported bpp: {}", mode.bits_per_pixel),
                        module: "VBE".to_string(),
                    }));
                }
            }
        }
        Ok(())
    }

    /// Get current framebuffer address
    pub fn framebuffer_address(&self) -> u64 {
        self.framebuffer_addr
    }

    /// Get current mode info
    pub fn current_mode(&self) -> Option<&VbeModeInfo> {
        self.current_mode.as_ref()
    }
}

impl Default for VbeHandler {
    fn default() -> Self {
        Self::new()
    }
}
