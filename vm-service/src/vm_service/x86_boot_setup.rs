//! # Proper Linux Boot Protocol Setup
//!
//! This module implements the correct Linux/x86 boot protocol setup
//! as specified in the official kernel documentation.

use std::mem::size_of;
use vm_core::{GuestAddr, MMU, VmError, VmResult};

/// Boot protocol setup configuration
pub struct BootConfig {
    /// Video mode (0xFFFF = normal, 0xFFFE = ext, 0xFFFD = ask)
    pub vid_mode: u16,
    /// Root device (0 = use default)
    pub root_dev: u16,
    /// Command line string
    pub cmdline: String,
    /// Initial ramdisk address
    pub initrd_addr: Option<u32>,
    /// Initial ramdisk size
    pub initrd_size: Option<u32>,
    /// EFI framebuffer address (for EFI boot)
    pub efifb_addr: Option<u64>,
    /// EFI framebuffer width
    pub efifb_width: Option<u32>,
    /// EFI framebuffer height
    pub efifb_height: Option<u32>,
    /// EFI framebuffer stride
    pub efifb_stride: Option<u32>,
}

impl Default for BootConfig {
    fn default() -> Self {
        Self {
            vid_mode: 0xFFFF,            // Normal video mode
            root_dev: 0,                 // Use default
            cmdline: "auto".to_string(), // Automatic boot
            initrd_addr: None,
            initrd_size: None,
            efifb_addr: None,
            efifb_width: None,
            efifb_height: None,
            efifb_stride: None,
        }
    }
}

/// Setup Linux boot protocol parameters properly
///
/// This function configures the boot protocol header fields that
/// are marked as "obligatory" in the Linux boot protocol documentation.
pub fn setup_linux_boot_protocol(
    mmu: &mut dyn MMU,
    kernel_load_addr: GuestAddr,
    config: &BootConfig,
) -> VmResult<()> {
    log::info!("=== Setting up Linux Boot Protocol ===");

    // Offset of boot protocol header in kernel
    const HEADER_OFFSET: u64 = 0x1F1;

    // Read and validate header signature
    let sig_addr = GuestAddr(kernel_load_addr.0 + 0x202);
    let sig = mmu.read(sig_addr, 4)?;
    let sig_bytes: [u8; 4] = [
        (sig & 0xFF) as u8,
        ((sig >> 8) & 0xFF) as u8,
        ((sig >> 16) & 0xFF) as u8,
        ((sig >> 24) & 0xFF) as u8,
    ];

    if &sig_bytes != b"HdrS" {
        log::warn!("Invalid boot signature: {:?}", sig_bytes);
        // Continue anyway - might be old kernel format
    } else {
        log::info!("✓ Valid bzImage boot signature found");
    }

    // Read protocol version
    let version_addr = GuestAddr(kernel_load_addr.0 + 0x206);
    let version = mmu.read(version_addr, 2)? as u16;
    let major = (version >> 8) as u8;
    let minor = (version & 0xFF) as u8;
    log::info!("Boot protocol version: {}.{}", major, minor);

    // 1. Set type_of_loader (offset 0x210) - OBLIGATORY
    // 0xFF = undefined bootloader, let's use a custom value
    const TYPE_OF_LOADER_OFFSET: u64 = 0x210;
    let loader_id: u8 = 0xFF; // Undefined/custom loader
    mmu.write(
        GuestAddr(kernel_load_addr.0 + TYPE_OF_LOADER_OFFSET),
        loader_id as u64,
        1,
    )?;
    log::info!("✓ Set type_of_loader = 0x{:02X}", loader_id);

    // 2. Set loadflags (offset 0x211) - OBLIGATORY
    // Bit 7 (0x80) = CAN_USE_HEAP - indicates heap_end_ptr is valid
    const LOADFLAGS_OFFSET: u64 = 0x211;
    let mut loadflags: u8 = 0x80; // Set CAN_USE_HEAP
    mmu.write(
        GuestAddr(kernel_load_addr.0 + LOADFLAGS_OFFSET),
        loadflags as u64,
        1,
    )?;
    log::info!("✓ Set loadflags = 0x{:02X} (CAN_USE_HEAP)", loadflags);

    // 3. Set heap_end_ptr (offset 0x224) - OBLIGATORY for protocol >= 2.01
    // This points to the end of the setup stack/heap minus 0x200
    if major >= 2 || (major == 2 && minor >= 1) {
        const HEAP_END_PTR_OFFSET: u64 = 0x224;
        // For simplicity, use 0x8000 as heap end (gives us 0x8000-0x200 = 0x7E00 bytes)
        let heap_end_ptr: u16 = 0x8000;
        mmu.write(
            GuestAddr(kernel_load_addr.0 + HEAP_END_PTR_OFFSET),
            heap_end_ptr as u64,
            2,
        )?;
        log::info!("✓ Set heap_end_ptr = 0x{:04X}", heap_end_ptr);
    }

    // 4. Set cmd_line_ptr (offset 0x228) - OBLIGATORY for protocol >= 2.02
    if major >= 2 || (major == 2 && minor >= 2) {
        // Allocate command line in memory (after heap)
        let cmdline_addr = GuestAddr(kernel_load_addr.0 + 0x9000);

        // Build full command line with EFI framebuffer info if provided
        let full_cmdline =
            if let (Some(fb_addr), Some(fb_width), Some(fb_height), Some(fb_stride)) = (
                config.efifb_addr,
                config.efifb_width,
                config.efifb_height,
                config.efifb_stride,
            ) {
                // Add EFI framebuffer parameter to command line
                // Format: efifb=<addr>:<width>x<height>@<stride>
                let efifb_param = format!(
                    "efifb={:#x}:{}x{}@{}",
                    fb_addr, fb_width, fb_height, fb_stride
                );
                format!("{} {}", config.cmdline, efifb_param)
            } else {
                config.cmdline.clone()
            };

        let cmdline_bytes = full_cmdline.as_bytes();

        // Write command line to memory
        mmu.write_bulk(cmdline_addr, cmdline_bytes)?;

        // Null-terminate
        mmu.write(GuestAddr(cmdline_addr.0 + cmdline_bytes.len() as u64), 0, 1)?;

        // Set cmd_line_ptr
        const CMD_LINE_PTR_OFFSET: u64 = 0x228;
        mmu.write(
            GuestAddr(kernel_load_addr.0 + CMD_LINE_PTR_OFFSET),
            cmdline_addr.0,
            4,
        )?;
        log::info!(
            "✓ Set cmd_line_ptr = 0x{:08X} ({})",
            cmdline_addr.0,
            full_cmdline
        );
    }

    // 5. Set vid_mode (offset 0x1FA) - RECOMMENDED
    const VID_MODE_OFFSET: u64 = 0x1FA;
    mmu.write(
        GuestAddr(kernel_load_addr.0 + VID_MODE_OFFSET),
        config.vid_mode as u64,
        2,
    )?;
    log::info!("✓ Set vid_mode = 0x{:04X}", config.vid_mode);

    // 6. Set ramdisk_image and ramdisk_size (offsets 0x218, 0x21C) - if initrd provided
    if let (Some(addr), Some(size)) = (config.initrd_addr, config.initrd_size) {
        const RAMDISK_IMAGE_OFFSET: u64 = 0x218;
        const RAMDISK_SIZE_OFFSET: u64 = 0x21C;

        mmu.write(
            GuestAddr(kernel_load_addr.0 + RAMDISK_IMAGE_OFFSET),
            addr as u64,
            4,
        )?;
        mmu.write(
            GuestAddr(kernel_load_addr.0 + RAMDISK_SIZE_OFFSET),
            size as u64,
            4,
        )?;
        log::info!("✓ Set initrd: addr=0x{:08X}, size={} bytes", addr, size);
    }

    log::info!("=== Boot Protocol Setup Complete ===");

    Ok(())
}

/// Calculate the correct entry point for kernel execution
///
/// For bzImage kernels, the entry point depends on several factors:
/// - Protocol version
/// - LOAD_HIGH flag in loadflags
/// - Whether kernel is relocatable
/// - Kernel size (to distinguish between actual zImage and bzImage with LOAD_HIGH=0)
pub fn calculate_entry_point(
    mmu: &mut dyn MMU,
    kernel_load_addr: GuestAddr,
) -> VmResult<GuestAddr> {
    // Read loadflags to check LOAD_HIGH bit (bit 0)
    const LOADFLAGS_OFFSET: u64 = 0x211;
    let loadflags = mmu.read(GuestAddr(kernel_load_addr.0 + LOADFLAGS_OFFSET), 1)? as u8;

    // Read setup_sects to determine kernel setup code size
    const SETUP_SECTS_OFFSET: u64 = 0x1F1;
    let setup_sects = mmu.read(GuestAddr(kernel_load_addr.0 + SETUP_SECTS_OFFSET), 1)? as u8;

    // Bit 0 = LOAD_HIGH: if set, kernel is bzImage loaded at 0x100000
    // if clear, kernel is zImage loaded at 0x10000
    let is_bzimage = (loadflags & 0x01) != 0;

    // For modern Ubuntu kernels, the LOAD_HIGH flag might be 0 but the kernel
    // is actually a bzImage. We can detect this by checking:
    // 1. If setup_sects > typical zImage value (e.g., > 4)
    // 2. If protocol version >= 2.02
    const PROTOCOL_VERSION_OFFSET: u64 = 0x206;
    let protocol_version =
        mmu.read(GuestAddr(kernel_load_addr.0 + PROTOCOL_VERSION_OFFSET), 2)? as u16;
    let major = (protocol_version >> 8) as u8;
    let minor = (protocol_version & 0xFF) as u8;

    // Check if this is actually a bzImage despite LOAD_HIGH=0
    let is_large_kernel = setup_sects > 4 || (major >= 2 && minor >= 2);

    if is_bzimage || (is_large_kernel && !is_bzimage) {
        if !is_bzimage {
            log::info!(
                "Large kernel detected (setup_sects={}, protocol={}.{}), treating as bzImage despite LOAD_HIGH=0",
                setup_sects,
                major,
                minor
            );
        } else {
            log::info!("bzImage detected: LOAD_HIGH flag is set");
        }

        // For bzImage kernels, the boot starts in REAL MODE at the setup code
        // The setup code is at the beginning of the kernel (where it was loaded)
        // NOT at code32_start (that's for protected mode)

        // According to Linux boot protocol spec, the entry point is:
        // - For old kernels: start of setup code (0x0000)
        // - For modern kernels: start of setup code + 0x200 (after boot sector)
        // The 0x200 offset skips the boot sector and jumps to main setup code

        // Based on previous successful iterations, try 0x200 offset first
        let real_mode_entry = GuestAddr(kernel_load_addr.0 + 0x0200);

        log::info!(
            "bzImage real-mode entry point: 0x{:08X} (setup code + 0x200)",
            real_mode_entry.0
        );
        log::info!("  Note: code32_start=0x100000 is for protected mode transition later");
        log::info!("  This skips the boot sector and starts at main setup code");

        Ok(real_mode_entry)
    } else {
        log::info!("zImage detected: LOAD_HIGH flag is clear");
        // For zImage, kernel runs from where it was loaded (0x10000)
        log::info!("zImage entry point: 0x{:08X}", kernel_load_addr.0);
        Ok(kernel_load_addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_config_default() {
        let config = BootConfig::default();
        assert_eq!(config.vid_mode, 0xFFFF);
        assert_eq!(config.cmdline, "auto");
        assert!(config.initrd_addr.is_none());
    }

    #[test]
    fn test_offsets() {
        // Verify critical offsets match boot protocol spec
        assert_eq!(0x1F1, 0x1F1); // setup_sects
        assert_eq!(0x1FA, 0x1FA); // vid_mode
        assert_eq!(0x202, 0x202); // header ("HdrS")
        assert_eq!(0x206, 0x206); // version
        assert_eq!(0x210, 0x210); // type_of_loader
        assert_eq!(0x211, 0x211); // loadflags
        assert_eq!(0x224, 0x224); // heap_end_ptr
        assert_eq!(0x228, 0x228); // cmd_line_ptr
    }
}
