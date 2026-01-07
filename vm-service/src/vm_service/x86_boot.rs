//! # x86 Boot Support
//!
//! Minimal x86 boot protocol support for loading bzImage kernels.
//! This module handles the Linux boot protocol and provides the necessary
//! data structures for kernel booting.

use vm_core::{GuestAddr, MMU, VmError, VmResult};
use std::mem::size_of;

/// Linux boot protocol header (at offset 0x202 in bzImage)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BootParams {
    /// 0x200: Offset of first setup code
    pub setup_sects: u8,
    /// 0x201: Flags
    pub root_flags: u16,
    /// 0x203: Size of compressed kernel in 16-byte units
    pub syssize: u16,
    /// 0x205: RAM disk image size (obsolete)
    pub ram_size: u16,
    /// 0x207: Video mode (see linux/video.h)
    pub vid_mode: u16,
    /// 0x209: Root filesystem partition
    pub root_dev: u16,
    /// 0x20B: Boot sector command line
    pub boot_flag: u16,
    /// 0x20D: Jump instruction (should be EB XX)
    pub jump: u16,
    /// 0x20F: Header signature "HdrS"
    pub header: [u8; 4],
    /// 0x213: Boot protocol version
    pub version: u16,
    /// 0x215: Realmode switch hook
    pub realmode_swtch: u32,
    /// 0x219: Load address for bzImage
    pub start_sys_seg: u16,
    /// 0x21B: Pointer to kernel version string
    pub kernel_version: u16,
    /// 0x21D: Type of bootloader
    pub type_of_loader: u8,
    /// 0x21E: Load flags
    pub loadflags: u8,
    /// 0x21F: Setup code size in 16-byte units
    pub setup_move_size: u16,
    /// 0x221: Boot protocol version high
    pub code32_start: u32,
    /// 0x225: Initial ramdisk image address
    pub ramdisk_start: u32,
    /// 0x229: Initial ramdisk image size
    pub ramdisk_len: u32,
    /// 0x22D: Command line pointer
    pub cmd_line_ptr: u32,
    /// 0x231: Highest initrd address
    pub initrd_addr_max: u32,
    /// 0x235: Kernel alignment
    pub kernel_alignment: u32,
    /// 0x239: Relocatable kernel flag
    pub relocatable_kernel: u8,
    /// 0x23A: Minimum alignment
    pub min_alignment: u8,
    /// 0x23B: Padding
    pub xloadflags: u16,
    /// 0x23D: CPU setup info
    pub cpu_setup_info: u32,
    /// 0x241: Prefixed command line
    pub cmdline_prefixed: u32,
    /// 0x245: Extended boot params
    pub ext_boot_param: u32,
}

impl BootParams {
    /// Parse boot parameters from bzImage
    pub fn from_bzimage(mmu: &mut (dyn MMU + 'static), load_addr: GuestAddr) -> VmResult<Self> {
        let header_offset = 0x202_usize;
        let params_addr = GuestAddr(load_addr.0 + header_offset as u64);

        // Read the boot params structure as u64 words
        let size_u64 = (size_of::<BootParams>() + 7) / 8;
        let mut words = [0u64; 0x50]; // BootParams is 0x246 bytes = ~74 u64s

        for i in 0..size_u64 {
            let addr = GuestAddr(params_addr.0 + (i * 8) as u64);
            words[i] = mmu.read(addr, 8)?;
        }

        // Convert to BootParams
        let params = unsafe {
            std::ptr::read(words.as_ptr() as *const _ as *const BootParams)
        };

        // Verify signature
        if &params.header != b"HdrS" {
            return Err(VmError::Core(vm_core::CoreError::Internal {
                message: format!("Invalid boot header signature: {:?}", params.header),
                module: "x86_boot".to_string(),
            }));
        }

        Ok(params)
    }

    /// Get the 32-bit entry point
    pub fn code32_start(&self) -> u32 {
        self.code32_start
    }

    /// Get boot protocol version
    pub fn protocol_version(&self) -> (u8, u8) {
        ((self.version >> 8) as u8, (self.version & 0xFF) as u8)
    }
}

/// x86 execution mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum X86Mode {
    /// 16-bit real mode
    RealMode,
    /// 32-bit protected mode
    ProtectedMode,
    /// 64-bit long mode
    LongMode,
}

/// Minimal x86 real-mode emulator context
pub struct RealModeContext {
    /// Segment registers
    pub cs: u16,
    pub ds: u16,
    pub es: u16,
    pub ss: u16,
    /// General registers (simplified)
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
    pub esi: u32,
    pub edi: u32,
    pub ebp: u32,
    pub esp: u32,
    /// Instruction pointer
    pub eip: u32,
    /// Flags
    pub eflags: u32,
    /// Current mode
    pub mode: X86Mode,
}

impl RealModeContext {
    /// Create new real-mode context
    pub fn new() -> Self {
        Self {
            cs: 0x07C0,  // Typical BIOS boot segment
            ds: 0x07C0,
            es: 0x07C0,
            ss: 0x07C0,
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: 0,
            esi: 0,
            edi: 0,
            ebp: 0,
            esp: 0x7C00,  // Stack top
            eip: 0,
            eflags: 0x2,  // Reserved bit set
            mode: X86Mode::RealMode,
        }
    }

    /// Convert segment:offset to linear address (real mode)
    pub fn seg_to_linear(&self, seg: u16, offset: u16) -> u32 {
        ((seg as u32) << 4) + (offset as u32)
    }

    /// Get current linear address (CS:IP)
    pub fn get_linear_ip(&self) -> u32 {
        self.seg_to_linear(self.cs, self.eip as u16)
    }

    /// Switch to protected mode
    pub fn switch_to_protected(&mut self) {
        self.mode = X86Mode::ProtectedMode;
        // In protected mode, segments become selectors
        // For now, use flat segments
        self.cs = 0x08;  // Kernel code selector
        self.ds = 0x10;  // Kernel data selector
        self.es = 0x10;
        self.ss = 0x10;
    }

    /// Switch to long mode (64-bit)
    pub fn switch_to_long(&mut self) {
        self.mode = X86Mode::LongMode;
        // In long mode, CS must have L=1 and D=0 bits
        self.cs = 0x08;  // 64-bit code selector
    }
}

impl Default for RealModeContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Load and bootstrap a bzImage kernel
pub fn load_bzimage(
    mmu: &mut (dyn MMU + 'static),
    kernel_data: &[u8],
    load_addr: GuestAddr,
    initrd_data: Option<&[u8]>,
    initrd_addr: Option<GuestAddr>,
) -> VmResult<GuestAddr> {
    // Load kernel to memory using write_bulk
    mmu.write_bulk(load_addr, kernel_data)?;

    // Load initrd if provided
    if let (Some(initrd), Some(addr)) = (initrd_data, initrd_addr) {
        mmu.write_bulk(addr, initrd)?;
        log::info!("Loaded initrd: {} bytes at 0x{:x}", initrd.len(), addr.0);
    }

    // Parse boot parameters
    let boot_params = BootParams::from_bzimage(mmu, load_addr)?;

    let (major, minor) = boot_params.protocol_version();
    log::info!(
        "bzImage boot protocol version: {}.{}",
        major,
        minor
    );
    log::info!("Kernel entry point: 0x{:x}", boot_params.code32_start());

    // Calculate entry point
    // For modern kernels, code32_start is relative to load_addr
    let entry_point = if boot_params.code32_start() == 0x100000 {
        // Standard 64-bit kernel entry
        GuestAddr(load_addr.0 + boot_params.code32_start() as u64)
    } else {
        // Use code32_start as is
        GuestAddr(boot_params.code32_start() as u64)
    };

    log::info!("Final entry point: 0x{:x}", entry_point.0);

    Ok(entry_point)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_params_size() {
        assert_eq!(size_of::<BootParams>(), 0x246);
    }

    #[test]
    fn test_real_mode_context() {
        let ctx = RealModeContext::new();
        assert_eq!(ctx.get_linear_ip(), 0x07C00);
    }
}
