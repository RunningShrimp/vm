//! Platform abstraction layer for hardware virtualization backends
//!
//! This module provides a unified interface for working with different
//! hardware virtualization platforms (KVM, HVF, WHPX, VZ) through a
//! single [`PlatformBackend`] enum.
//!
//! # Architecture
//!
//! The [`PlatformBackend`] enum wraps platform-specific implementations,
//! providing a consistent API regardless of the underlying hypervisor.
//!
//! # Example
//!
//! ```rust,ignore
//! use vm_accel::platform::PlatformBackend;
//! use vm_accel::AccelKind;
//!
//! // Create backend for desired platform
//! let mut backend = PlatformBackend::new(AccelKind::Kvm)?;
//!
//! // Initialize the backend
//! backend.init()?;
//!
//! // Create vCPU (works the same for all platforms)
//! let vcpu = backend.create_vcpu(0)?;
//! ```

use vm_core::{GuestRegs, MMU, VmError};

use super::vcpu_common::{VcpuOps, VcpuResult};
use super::{Accel, AccelKind, AccelError};

// Import platform-specific implementations
#[cfg(target_os = "linux")]
use super::kvm_impl;

#[cfg(target_os = "macos")]
use super::hvf_impl;

#[cfg(target_os = "windows")]
use super::whpx_impl;

#[cfg(any(target_os = "ios", target_os = "tvos"))]
use super::vz_impl;

/// Unified platform backend
///
/// This enum wraps all supported virtualization platforms, providing a
/// unified interface for VM operations. Each variant contains the
/// platform-specific implementation.
///
/// # Variants
///
/// - `Kvm`: Linux KVM (Kernel-based Virtual Machine)
/// - `Hvf`: macOS Hypervisor.framework
/// - `Whpx`: Windows Hypervisor Platform
/// - `Vz`: iOS/tvOS Virtualization.framework
/// - `Fallback`: Software-only emulation (no hardware acceleration)
///
/// # Example
///
/// ```rust,ignore
/// use vm_accel::platform::PlatformBackend;
///
/// #[cfg(target_os = "linux")]
/// let backend = PlatformBackend::Kvm(kvm_impl::AccelKvm::new()?);
///
/// // Use the unified interface
/// backend.init()?;
/// backend.create_vcpu(0)?;
/// ```
pub enum PlatformBackend {
    /// Linux KVM backend
    #[cfg(target_os = "linux")]
    Kvm(kvm_impl::AccelKvm),

    /// macOS Hypervisor.framework backend
    #[cfg(target_os = "macos")]
    Hvf(hvf_impl::AccelHvf),

    /// Windows Hypervisor Platform backend
    #[cfg(target_os = "windows")]
    Whpx(whpx_impl::AccelWhpx),

    /// iOS/tvOS Virtualization.framework backend
    #[cfg(any(target_os = "ios", target_os = "tvos"))]
    Vz(vz_impl::AccelVz),

    /// Fallback backend (no hardware acceleration)
    Fallback(super::NoAccel),
}

impl PlatformBackend {
    /// Create a new platform backend
    ///
    /// Creates a backend instance for the specified acceleration kind.
    /// Returns an error if the requested backend type is not available
    /// on the current platform.
    ///
    /// # Arguments
    ///
    /// * `kind` - The type of acceleration backend to create
    ///
    /// # Errors
    ///
    /// Returns `VmError::NotSupported` if:
    /// - The requested backend is not available on this platform
    /// - The hardware does not support virtualization
    /// - Required permissions are missing
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use vm_accel::platform::PlatformBackend;
    /// use vm_accel::AccelKind;
    ///
    /// // Detect and create the best available backend
    /// let kind = AccelKind::detect_best();
    /// match kind {
    ///     AccelKind::Kvm => {
    ///         let backend = PlatformBackend::new(AccelKind::Kvm)?;
    ///         // ...
    ///     }
    ///     _ => println!("No acceleration available"),
    /// }
    /// ```
    pub fn new(kind: AccelKind) -> VcpuResult<Self> {
        match kind {
            #[cfg(target_os = "linux")]
            AccelKind::Kvm => {
                let kvm = kvm_impl::AccelKvm::new();
                Ok(PlatformBackend::Kvm(kvm))
            }

            #[cfg(target_os = "macos")]
            AccelKind::Hvf => {
                let hvf = hvf_impl::AccelHvf::new();
                Ok(PlatformBackend::Hvf(hvf))
            }

            #[cfg(target_os = "windows")]
            AccelKind::Whpx => {
                let whpx = whpx_impl::AccelWhpx::new();
                Ok(PlatformBackend::Whpx(whpx))
            }

            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            AccelKind::Hvf => {
                // iOS uses Hvf as the kind for VZ backend
                let vz = vz_impl::AccelVz::new();
                Ok(PlatformBackend::Vz(vz))
            }

            AccelKind::None => Ok(PlatformBackend::Fallback(super::NoAccel)),

            #[allow(unreachable_patterns)]
            _ => Err(VmError::Core(vm_core::CoreError::NotSupported {
                feature: format!("Acceleration backend: {:?}", kind),
                module: "vm-accel".to_string(),
            })),
        }
    }

    /// Create a vCPU with the specified ID
    ///
    /// Creates a new vCPU instance for this VM. The vCPU ID must be unique
    /// within the VM and is typically a sequential integer starting from 0.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique vCPU identifier
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The vCPU ID already exists
    /// - Maximum vCPU count is reached
    /// - Memory allocation fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Create vCPU 0
    /// let vcpu = backend.create_vcpu(0)?;
    ///
    /// // Create additional vCPUs for SMP guests
    /// let vcpu1 = backend.create_vcpu(1)?;
    /// let vcpu2 = backend.create_vcpu(2)?;
    /// ```
    pub fn create_vcpu(&mut self, id: u32) -> VcpuResult<Box<dyn VcpuOps>> {
        match self {
            #[cfg(target_os = "linux")]
            PlatformBackend::Kvm(backend) => {
                // Create KVM vCPU with VcpuOps implementation
                #[cfg(target_arch = "x86_64")]
                {
                    use kvm_impl::kvm_x86_64::KvmVcpuX86_64;
                    let vcpu = backend.create_vcpu(id).map_err(|e| match e {
                        AccelError::Vm(vm_err) => vm_err,
                        AccelError::Platform(plat_err) => VmError::Platform(plat_err),
                    })?;
                    Ok(Box::new(vcpu) as Box<dyn VcpuOps>)
                }

                #[cfg(target_arch = "aarch64")]
                {
                    use kvm_impl::kvm_aarch64::KvmVcpuAarch64;
                    let vcpu = backend.create_vcpu(id).map_err(|e| match e {
                        AccelError::Vm(vm_err) => vm_err,
                        AccelError::Platform(plat_err) => VmError::Platform(plat_err),
                    })?;
                    Ok(Box::new(vcpu) as Box<dyn VcpuOps>)
                }

                #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
                Err(VmError::Core(vm_core::CoreError::NotImplemented {
                    feature: format!("KVM VcpuOps for {}", std::env::consts::ARCH),
                    module: "vm-accel".to_string(),
                }))
            }

            #[cfg(target_os = "macos")]
            PlatformBackend::Hvf(backend) => {
                // Create HVF vCPU with VcpuOps implementation
                backend.create_vcpu_ops(id)
            }

            #[cfg(target_os = "windows")]
            PlatformBackend::Whpx(backend) => {
                // Create WHPX vCPU with VcpuOps implementation
                backend.create_vcpu_ops(id)
            }

            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            PlatformBackend::Vz(backend) => {
                // Create VZ vCPU with VcpuOps implementation
                backend.create_vcpu_ops(id)
            }

            PlatformBackend::Fallback(_) => Err(VmError::Core(vm_core::CoreError::NotSupported {
                feature: "vCPU creation without acceleration".to_string(),
                module: "vm-accel".to_string(),
            })),
        }
    }
}

// Implement Accel trait for PlatformBackend to maintain compatibility
impl Accel for PlatformBackend {
    fn init(&mut self) -> Result<(), AccelError> {
        match self {
            #[cfg(target_os = "linux")]
            PlatformBackend::Kvm(backend) => backend.init(),

            #[cfg(target_os = "macos")]
            PlatformBackend::Hvf(backend) => backend.init(),

            #[cfg(target_os = "windows")]
            PlatformBackend::Whpx(backend) => backend.init(),

            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            PlatformBackend::Vz(backend) => backend.init(),

            PlatformBackend::Fallback(backend) => backend.init(),
        }
    }

    fn create_vcpu(&mut self, id: u32) -> Result<(), AccelError> {
        match self {
            #[cfg(target_os = "linux")]
            PlatformBackend::Kvm(backend) => backend.create_vcpu(id),

            #[cfg(target_os = "macos")]
            PlatformBackend::Hvf(backend) => backend.create_vcpu(id),

            #[cfg(target_os = "windows")]
            PlatformBackend::Whpx(backend) => backend.create_vcpu(id),

            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            PlatformBackend::Vz(backend) => backend.create_vcpu(id),

            PlatformBackend::Fallback(backend) => backend.create_vcpu(id),
        }
    }

    fn map_memory(
        &mut self,
        gpa: u64,
        hva: u64,
        size: u64,
        flags: u32,
    ) -> Result<(), AccelError> {
        match self {
            #[cfg(target_os = "linux")]
            PlatformBackend::Kvm(backend) => backend.map_memory(gpa, hva, size, flags),

            #[cfg(target_os = "macos")]
            PlatformBackend::Hvf(backend) => backend.map_memory(gpa, hva, size, flags),

            #[cfg(target_os = "windows")]
            PlatformBackend::Whpx(backend) => backend.map_memory(gpa, hva, size, flags),

            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            PlatformBackend::Vz(backend) => backend.map_memory(gpa, hva, size, flags),

            PlatformBackend::Fallback(backend) => backend.map_memory(gpa, hva, size, flags),
        }
    }

    fn unmap_memory(&mut self, gpa: u64, size: u64) -> Result<(), AccelError> {
        match self {
            #[cfg(target_os = "linux")]
            PlatformBackend::Kvm(backend) => backend.unmap_memory(gpa, size),

            #[cfg(target_os = "macos")]
            PlatformBackend::Hvf(backend) => backend.unmap_memory(gpa, size),

            #[cfg(target_os = "windows")]
            PlatformBackend::Whpx(backend) => backend.unmap_memory(gpa, size),

            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            PlatformBackend::Vz(backend) => backend.unmap_memory(gpa, size),

            PlatformBackend::Fallback(backend) => backend.unmap_memory(gpa, size),
        }
    }

    fn run_vcpu(&mut self, vcpu_id: u32, mmu: &mut dyn MMU) -> Result<(), AccelError> {
        match self {
            #[cfg(target_os = "linux")]
            PlatformBackend::Kvm(backend) => backend.run_vcpu(vcpu_id, mmu),

            #[cfg(target_os = "macos")]
            PlatformBackend::Hvf(backend) => backend.run_vcpu(vcpu_id, mmu),

            #[cfg(target_os = "windows")]
            PlatformBackend::Whpx(backend) => backend.run_vcpu(vcpu_id, mmu),

            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            PlatformBackend::Vz(backend) => backend.run_vcpu(vcpu_id, mmu),

            PlatformBackend::Fallback(backend) => backend.run_vcpu(vcpu_id, mmu),
        }
    }

    fn get_regs(&self, vcpu_id: u32) -> Result<GuestRegs, AccelError> {
        match self {
            #[cfg(target_os = "linux")]
            PlatformBackend::Kvm(backend) => backend.get_regs(vcpu_id),

            #[cfg(target_os = "macos")]
            PlatformBackend::Hvf(backend) => backend.get_regs(vcpu_id),

            #[cfg(target_os = "windows")]
            PlatformBackend::Whpx(backend) => backend.get_regs(vcpu_id),

            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            PlatformBackend::Vz(backend) => backend.get_regs(vcpu_id),

            PlatformBackend::Fallback(backend) => backend.get_regs(vcpu_id),
        }
    }

    fn set_regs(&mut self, vcpu_id: u32, regs: &GuestRegs) -> Result<(), AccelError> {
        match self {
            #[cfg(target_os = "linux")]
            PlatformBackend::Kvm(backend) => backend.set_regs(vcpu_id, regs),

            #[cfg(target_os = "macos")]
            PlatformBackend::Hvf(backend) => backend.set_regs(vcpu_id, regs),

            #[cfg(target_os = "windows")]
            PlatformBackend::Whpx(backend) => backend.set_regs(vcpu_id, regs),

            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            PlatformBackend::Vz(backend) => backend.set_regs(vcpu_id, regs),

            PlatformBackend::Fallback(backend) => backend.set_regs(vcpu_id, regs),
        }
    }

    fn name(&self) -> &str {
        match self {
            #[cfg(target_os = "linux")]
            PlatformBackend::Kvm(_) => "KVM",
            #[cfg(target_os = "macos")]
            PlatformBackend::Hvf(_) => "HVF",
            #[cfg(target_os = "windows")]
            PlatformBackend::Whpx(_) => "WHPX",
            #[cfg(any(target_os = "ios", target_os = "tvos"))]
            PlatformBackend::Vz(_) => "VZ",
            PlatformBackend::Fallback(_) => "Fallback",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_backend_name() {
        #[cfg(target_os = "linux")]
        {
            let backend = PlatformBackend::Fallback(super::NoAccel);
            assert_eq!(backend.name(), "Fallback");
        }
    }
}
