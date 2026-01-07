//! Common vCPU operations independent of platform
//!
//! This module provides unified abstractions for vCPU operations that are
//! consistent across all hardware virtualization platforms (KVM, HVF, WHPX, VZ).
//!
//! # Architecture
//!
//! The [`VcpuOps`] trait defines a platform-agnostic interface for vCPU operations,
//! allowing different backends to implement the same logical operations while
//! handling platform-specific details internally.
//!
//! # Example
//!
//! ```rust,ignore
//! use vm_accel::vcpu_common::{VcpuOps, reg_convert};
//! use vm_core::GuestRegs;
//!
//! // Get vCPU from platform backend
//! let vcpu: Box<dyn VcpuOps> = backend.create_vcpu(0)?;
//!
//! // Read registers (works on all platforms)
//! let regs = vcpu.get_regs()?;
//! println!("PC = {:#x}", regs.pc);
//!
//! // Modify registers
//! let mut new_regs = regs;
//! new_regs.gpr[0] = 42;
//! vcpu.set_regs(&new_regs)?;
//!
//! // Run vCPU
//! let exit = vcpu.run()?;
//! ```

use vm_core::{GuestRegs, VmError};

/// Result type for vCPU operations
pub type VcpuResult<T> = Result<T, VmError>;

/// vCPU exit reason - describes why the vCPU exited
///
/// This represents the different reasons a vCPU might stop execution and return
/// control to the hypervisor. The specific reasons vary by platform, but this
/// unified enum captures the common categories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VcpuExit {
    /// IO instruction (in/out/ins/outs)
    Io {
        port: u16,
        direction: IoDirection,
        size: u8,
    },
    /// Memory-mapped IO
    Mmio {
        addr: u64,
        size: u8,
        is_write: bool,
    },
    /// System call (syscall/sysenter)
    SystemCall,
    /// Debug exception (breakpoint, single-step)
    Debug,
    /// Halt instruction executed
    Halted,
    /// External interrupt (hardware interrupt)
    Interrupt,
    /// Failed to get vCPU exit reason
    Unknown,
}

/// IO direction (read or write)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoDirection {
    In,
    Out,
}

/// Common vCPU operations trait
///
/// This trait provides a unified interface for vCPU operations across all
/// virtualization platforms (KVM, HVF, WHPX, VZ). Each platform backend
/// implements this trait, allowing the rest of the VM code to work with
/// any backend without modification.
///
/// # Thread Safety
///
/// Implementations of this trait are **not** required to be thread-safe.
/// vCPU instances should typically be used from a single thread, with
/// synchronization handled at a higher level if needed.
///
/// # Example Implementation
///
/// ```rust,ignore
/// use vm_accel::vcpu_common::VcpuOps;
///
/// pub struct MyVcpu {
///     fd: PlatformVcpuFd,
///     id: u32,
/// }
///
/// impl VcpuOps for MyVcpu {
///     fn get_id(&self) -> u32 {
///         self.id
///     }
///
///     fn run(&mut self) -> Result<VcpuExit, VmError> {
///         // Platform-specific run logic
///     }
///
///     // ... other methods
/// }
/// ```
pub trait VcpuOps: Send {
    /// Get the vCPU ID
    ///
    /// Returns the unique identifier for this vCPU within the VM.
    /// vCPU IDs are typically 0-based sequential integers.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let vcpu_id = vcpu.get_id();
    /// println!("vCPU{}", vcpu_id);
    /// ```
    fn get_id(&self) -> u32;

    /// Run the vCPU until it exits
    ///
    /// Executes guest code on this vCPU until an exit condition is encountered.
    /// The exit reason is returned, indicating why the vCPU stopped.
    ///
    /// # Blocking Behavior
    ///
    /// This call typically blocks until the vCPU exits. If you need non-blocking
    /// behavior, consider running the vCPU in a separate thread.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The vCPU is in an invalid state
    /// - The underlying platform call fails
    /// - The VM has been destroyed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// loop {
    ///     match vcpu.run()? {
    ///         VcpuExit::Io { port, .. } => {
    ///             // Handle IO instruction
    ///             handle_io(port);
    ///         }
    ///         VcpuExit::Halted => {
    ///             // Guest halted - stop running
    ///             break;
    ///         }
    ///         _ => {}
    ///     }
    /// }
    /// ```
    fn run(&mut self) -> VcpuResult<VcpuExit>;

    /// Get general-purpose registers
    ///
    /// Reads the current architectural register state from the vCPU.
    /// The returned [`GuestRegs`] structure contains:
    /// - `pc`: Program counter
    /// - `sp`: Stack pointer
    /// - `fp`: Frame pointer
    /// - `gpr`: Array of general-purpose registers
    ///
    /// # Register Mapping
    ///
    /// The exact mapping of architectural registers to the `gpr` array
    /// is architecture-specific:
    ///
    /// - **x86_64**: RAX, RCX, RDX, RBX, RSP, RBP, RSI, RDI, R8-R15
    /// - **aarch64**: X0-X30
    /// - **riscv64**: X1-X31
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let regs = vcpu.get_regs()?;
    /// println!("PC = {:#x}, SP = {:#x}", regs.pc, regs.sp);
    /// println!("R0 = {}", regs.gpr[0]);
    /// ```
    fn get_regs(&self) -> VcpuResult<GuestRegs>;

    /// Set general-purpose registers
    ///
    /// Updates the architectural register state of the vCPU.
    /// The vCPU will use the new register state on the next call to [`run()`](VcpuOps::run).
    ///
    /// # Partial Updates
    ///
    /// All fields in the [`GuestRegs`] structure are updated atomically.
    /// If you need to update only specific registers, read the current state
    /// first with [`get_regs()`](VcpuOps::get_regs), modify the desired fields,
    /// then call this method.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut regs = vcpu.get_regs()?;
    /// regs.gpr[0] = 42;  // Set first argument register
    /// regs.pc = entry_point;
    /// vcpu.set_regs(&regs)?;
    /// ```
    fn set_regs(&mut self, regs: &GuestRegs) -> VcpuResult<()>;

    /// Get floating-point/SIMD registers
    ///
    /// Reads the floating-point and SIMD register state.
    /// This includes:
    /// - x86_64: XMM, YMM, ZMM registers, MXCSR
    /// - aarch64: V registers, FPSR, FPCR
    ///
    /// # Platform Support
    ///
    /// Not all platforms support floating-point state access. Returns
    /// `VmError::NotSupported` if unavailable.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// match vcpu.get_fpu_regs() {
    ///     Ok(fpu) => println!("XMM0 = {:?}", fpu.xmm[0]),
    ///     Err(e) => println!("FPU state not available: {}", e),
    /// }
    /// ```
    fn get_fpu_regs(&self) -> VcpuResult<FpuRegs>;

    /// Set floating-point/SIMD registers
    ///
    /// Updates the floating-point and SIMD register state.
    ///
    /// # Platform Support
    ///
    /// Not all platforms support floating-point state updates. Returns
    /// `VmError::NotSupported` if unavailable.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut fpu = vcpu.get_fpu_regs()?;
    /// fpu.xmm[0] = [1.0f64, 2.0, 3.0, 4.0];
    /// vcpu.set_fpu_regs(&fpu)?;
    /// ```
    fn set_fpu_regs(&mut self, regs: &FpuRegs) -> VcpuResult<()>;
}

/// Floating-point and SIMD register state
///
/// This structure represents the floating-point and SIMD registers common
/// across architectures. The exact mapping is platform-specific.
///
/// # x86_64
///
/// - `xmm`: XMM0-XMM15 (128-bit each)
/// - `mxcsr`: SSE control/status register
///
/// # aarch64
///
/// - `xmm`: V0-V31 (128-bit each, mapped from ARM registers)
/// - `mxcsr`: FPCR (floating-point control register)
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FpuRegs {
    /// SIMD registers (XMM on x86_64, V registers on aarch64)
    pub xmm: [[u8; 16]; 32],
    /// Control/status register (MXCSR on x86_64, FPCR on aarch64)
    pub mxcsr: u32,
}

// ============================================================================
// Register Conversion Utilities
// ============================================================================

/// Register conversion utilities
///
/// This module provides traits and utilities for converting between
/// platform-specific register formats and the unified [`GuestRegs`] format.
///
/// # Example
///
/// ```rust,ignore
/// use vm_accel::vcpu_common::reg_convert::{ToGuestRegs, FromGuestRegs};
///
/// // Convert platform-specific format to GuestRegs
/// let kvm_regs = fd.get_regs()?;
/// let guest_regs = kvm_regs.to_guest_regs()?;
///
/// // Convert GuestRegs back to platform format
/// let kvm_regs = KvmRegs::from_guest_regs(&guest_regs)?;
/// ```
pub mod reg_convert {
    use super::*;

    /// Convert platform-specific register state to [`GuestRegs`]
    ///
    /// This trait should be implemented for platform-specific register structures,
    /// allowing them to be converted to the unified format.
    ///
    /// # Example Implementation
    ///
    /// ```rust,ignore
    /// use vm_accel::vcpu_common::reg_convert::ToGuestRegs;
    ///
    /// impl ToGuestRegs for kvm_regs {
    ///     fn to_guest_regs(&self) -> Result<GuestRegs, VmError> {
    ///         Ok(GuestRegs {
    ///             pc: self.rip,
    ///             sp: self.rsp,
    ///             fp: self.rbp,
    ///             gpr: [
    ///                 self.rax, self.rcx, self.rdx, self.rbx,
    ///                 self.rsp, self.rbp, self.rsi, self.rdi,
    ///                 // ... etc
    ///             ],
    ///         })
    ///     }
    /// }
    /// ```
    pub trait ToGuestRegs {
        /// Convert to [`GuestRegs`] format
        ///
        /// # Errors
        ///
        /// Returns an error if:
        /// - Register values are invalid (e.g., unaligned PC)
        /// - Required fields are missing
        fn to_guest_regs(&self) -> VcpuResult<GuestRegs>;
    }

    /// Convert [`GuestRegs`] to platform-specific register state
    ///
    /// This trait should be implemented for platform-specific register structures,
    /// allowing them to be created from the unified format.
    ///
    /// # Example Implementation
    ///
    /// ```rust,ignore
    /// use vm_accel::vcpu_common::reg_convert::FromGuestRegs;
    ///
    /// impl FromGuestRegs for kvm_regs {
    ///     fn from_guest_regs(regs: &GuestRegs) -> Result<Self, VmError>
    ///     where
    ///         Self: Sized
    ///     {
    ///         Ok(Self {
    ///             rip: regs.pc,
    ///             rsp: regs.sp,
    ///             rbp: regs.fp,
    ///             rax: regs.gpr[0],
    ///             rcx: regs.gpr[1],
    ///             // ... etc
    ///         })
    ///     }
    /// }
    /// ```
    pub trait FromGuestRegs
    where
        Self: Sized,
    {
        /// Create from [`GuestRegs`] format
        ///
        /// # Errors
        ///
        /// Returns an error if:
        /// - Register values are invalid for the platform
        /// - Required fields cannot be set
        fn from_guest_regs(regs: &GuestRegs) -> VcpuResult<Self>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vcpu_exit_debug() {
        let exit = VcpuExit::Debug;
        assert!(format!("{:?}", exit).contains("Debug"));
    }

    #[test]
    fn test_vcpu_exit_io() {
        let exit = VcpuExit::Io {
            port: 0x80,
            direction: IoDirection::Out,
            size: 1,
        };
        match exit {
            VcpuExit::Io { port, .. } => assert_eq!(port, 0x80),
            _ => panic!("Expected Io exit"),
        }
    }

    #[test]
    fn test_fpu_regs_default() {
        let fpu = FpuRegs::default();
        assert_eq!(fpu.xmm[0], [0u8; 16]);
        assert_eq!(fpu.mxcsr, 0);
    }

    #[test]
    fn test_fpu_regs_clone() {
        let mut fpu = FpuRegs::default();
        fpu.xmm[0] = [1u8; 16];
        fpu.mxcsr = 0x1F80;

        let fpu2 = fpu.clone();
        assert_eq!(fpu, fpu2);
    }

    #[test]
    fn test_io_direction() {
        assert_eq!(IoDirection::In, IoDirection::In);
        assert_ne!(IoDirection::In, IoDirection::Out);
    }
}
