//! Hypervisor.framework (HVF) acceleration backend for macOS
//!
//! This module provides hardware-assisted virtualization on macOS systems
//! using Apple's Hypervisor.framework.

use super::Accel;
use std::collections::HashMap;

/// Error type for HVF operations
#[derive(Debug, Clone)]
pub enum HvfError {
    /// HVF not available
    NotAvailable,
    /// Failed to create VM
    CreateVmFailed(i32),
    /// Failed to create vCPU
    CreateVcpuFailed(i32),
    /// Failed to map memory
    MapMemoryFailed(i32),
    /// Failed to run vCPU
    RunFailed(i32),
    /// Unsupported operation
    Unsupported(String),
}

impl std::fmt::Display for HvfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HvfError::NotAvailable => write!(f, "Hypervisor.framework not available"),
            HvfError::CreateVmFailed(e) => write!(f, "Failed to create VM: error {}", e),
            HvfError::CreateVcpuFailed(e) => write!(f, "Failed to create vCPU: error {}", e),
            HvfError::MapMemoryFailed(e) => write!(f, "Failed to map memory: error {}", e),
            HvfError::RunFailed(e) => write!(f, "Failed to run vCPU: error {}", e),
            HvfError::Unsupported(s) => write!(f, "Unsupported: {}", s),
        }
    }
}

impl std::error::Error for HvfError {}

/// HVF exit reasons
#[derive(Debug, Clone)]
pub enum HvfExitReason {
    Unknown,
    Exception,
    VirtualTimer,
    WFI,  // Wait For Interrupt (ARM)
    WFE,  // Wait For Event (ARM)
    HVC,  // Hypervisor Call
    SMC,  // Secure Monitor Call
    Shutdown,
}

/// Memory region tracking
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub guest_phys_addr: u64,
    pub size: u64,
    pub host_addr: u64,
    pub flags: u64,
}

/// HVF vCPU representation
pub struct HvfVcpu {
    #[cfg(target_os = "macos")]
    handle: u64,  // hv_vcpu_t
    id: u32,
}

/// HVF accelerator implementation
pub struct AccelHvf {
    initialized: bool,
    vcpus: Vec<HvfVcpu>,
    memory_regions: Vec<MemoryRegion>,
}

// HVF Constants (from Hypervisor.framework)
#[cfg(target_os = "macos")]
mod hvf_sys {
    // Memory flags
    pub const HV_MEMORY_READ: u64 = 1 << 0;
    pub const HV_MEMORY_WRITE: u64 = 1 << 1;
    pub const HV_MEMORY_EXEC: u64 = 1 << 2;
    
    // Exit reasons (ARM64)
    pub const HV_EXIT_REASON_CANCELED: u32 = 0;
    pub const HV_EXIT_REASON_EXCEPTION: u32 = 1;
    pub const HV_EXIT_REASON_VTIMER_ACTIVATED: u32 = 2;
    pub const HV_EXIT_REASON_UNKNOWN: u32 = 3;
    
    // Success code
    pub const HV_SUCCESS: i32 = 0;
    
    #[link(name = "Hypervisor", kind = "framework")]
    extern "C" {
        // VM management
        pub fn hv_vm_create(config: *mut std::ffi::c_void) -> i32;
        pub fn hv_vm_destroy() -> i32;
        
        // Memory management
        pub fn hv_vm_map(addr: *mut std::ffi::c_void, ipa: u64, size: usize, flags: u64) -> i32;
        pub fn hv_vm_unmap(ipa: u64, size: usize) -> i32;
        
        // vCPU management (ARM64)
        #[cfg(target_arch = "aarch64")]
        pub fn hv_vcpu_create(vcpu: *mut u64, exit: *mut *mut std::ffi::c_void, config: *mut std::ffi::c_void) -> i32;
        #[cfg(target_arch = "aarch64")]
        pub fn hv_vcpu_destroy(vcpu: u64) -> i32;
        #[cfg(target_arch = "aarch64")]
        pub fn hv_vcpu_run(vcpu: u64) -> i32;
        #[cfg(target_arch = "aarch64")]
        pub fn hv_vcpu_get_reg(vcpu: u64, reg: u32, value: *mut u64) -> i32;
        #[cfg(target_arch = "aarch64")]
        pub fn hv_vcpu_set_reg(vcpu: u64, reg: u32, value: u64) -> i32;
        #[cfg(target_arch = "aarch64")]
        pub fn hv_vcpu_get_sys_reg(vcpu: u64, reg: u16, value: *mut u64) -> i32;
        #[cfg(target_arch = "aarch64")]
        pub fn hv_vcpu_set_sys_reg(vcpu: u64, reg: u16, value: u64) -> i32;
    }
}

impl AccelHvf {
    pub fn new() -> Self {
        Self {
            initialized: false,
            vcpus: Vec::new(),
            memory_regions: Vec::new(),
        }
    }
    
    /// Check if HVF is available on this system
    pub fn is_available() -> bool {
        #[cfg(target_os = "macos")]
        {
            // Check for Hypervisor entitlement
            // On Apple Silicon, HVF requires the com.apple.security.hypervisor entitlement
            // For now, we try to create a VM and see if it succeeds
            unsafe {
                let result = hvf_sys::hv_vm_create(std::ptr::null_mut());
                if result == hvf_sys::HV_SUCCESS {
                    let _ = hvf_sys::hv_vm_destroy();
                    return true;
                }
            }
            false
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
    
    /// Map memory from host to guest
    #[cfg(target_os = "macos")]
    pub fn map_memory_region(&mut self, host_addr: *mut u8, guest_pa: u64, size: usize, read: bool, write: bool, exec: bool) -> Result<(), HvfError> {
        if !self.initialized {
            return Err(HvfError::NotAvailable);
        }
        
        let mut flags: u64 = 0;
        if read { flags |= hvf_sys::HV_MEMORY_READ; }
        if write { flags |= hvf_sys::HV_MEMORY_WRITE; }
        if exec { flags |= hvf_sys::HV_MEMORY_EXEC; }
        
        let result = unsafe {
            hvf_sys::hv_vm_map(host_addr as *mut _, guest_pa, size, flags)
        };
        
        if result != hvf_sys::HV_SUCCESS {
            return Err(HvfError::MapMemoryFailed(result));
        }
        
        self.memory_regions.push(MemoryRegion {
            guest_phys_addr: guest_pa,
            size: size as u64,
            host_addr: host_addr as u64,
            flags,
        });
        
        Ok(())
    }
    
    /// Create a vCPU
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    pub fn create_vcpu_internal(&mut self, id: u32) -> Result<(), HvfError> {
        if !self.initialized {
            return Err(HvfError::NotAvailable);
        }
        
        let mut vcpu_handle: u64 = 0;
        let mut exit_info: *mut std::ffi::c_void = std::ptr::null_mut();
        
        let result = unsafe {
            hvf_sys::hv_vcpu_create(&mut vcpu_handle, &mut exit_info, std::ptr::null_mut())
        };
        
        if result != hvf_sys::HV_SUCCESS {
            return Err(HvfError::CreateVcpuFailed(result));
        }
        
        self.vcpus.push(HvfVcpu {
            handle: vcpu_handle,
            id,
        });
        
        Ok(())
    }
    
    /// Run a vCPU
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    pub fn run_vcpu_internal(&mut self, vcpu_id: usize) -> Result<HvfExitReason, HvfError> {
        let vcpu = self.vcpus.get(vcpu_id)
            .ok_or(HvfError::Unsupported("Invalid vCPU ID".into()))?;
        
        let result = unsafe { hvf_sys::hv_vcpu_run(vcpu.handle) };
        
        if result != hvf_sys::HV_SUCCESS {
            return Err(HvfError::RunFailed(result));
        }
        
        // TODO: Parse exit info to determine actual exit reason
        Ok(HvfExitReason::Unknown)
    }
    
    /// Get general-purpose register (ARM64)
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    pub fn get_reg(&self, vcpu_id: usize, reg: u32) -> Result<u64, HvfError> {
        let vcpu = self.vcpus.get(vcpu_id)
            .ok_or(HvfError::Unsupported("Invalid vCPU ID".into()))?;
        
        let mut value: u64 = 0;
        let result = unsafe { hvf_sys::hv_vcpu_get_reg(vcpu.handle, reg, &mut value) };
        
        if result != hvf_sys::HV_SUCCESS {
            return Err(HvfError::RunFailed(result));
        }
        
        Ok(value)
    }
    
    /// Set general-purpose register (ARM64)
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    pub fn set_reg(&self, vcpu_id: usize, reg: u32, value: u64) -> Result<(), HvfError> {
        let vcpu = self.vcpus.get(vcpu_id)
            .ok_or(HvfError::Unsupported("Invalid vCPU ID".into()))?;
        
        let result = unsafe { hvf_sys::hv_vcpu_set_reg(vcpu.handle, reg, value) };
        
        if result != hvf_sys::HV_SUCCESS {
            return Err(HvfError::RunFailed(result));
        }
        
        Ok(())
    }
}

impl Default for AccelHvf {
    fn default() -> Self {
        Self::new()
    }
}

impl Accel for AccelHvf {
    #[cfg(target_os = "macos")]
    fn init(&mut self) -> bool {
        let result = unsafe { hvf_sys::hv_vm_create(std::ptr::null_mut()) };
        
        if result != hvf_sys::HV_SUCCESS {
            log::error!("Failed to create HVF VM: error {}", result);
            return false;
        }
        
        log::info!("Hypervisor.framework VM created successfully");
        self.initialized = true;
        true
    }
    
    #[cfg(not(target_os = "macos"))]
    fn init(&mut self) -> bool {
        log::warn!("Hypervisor.framework not available on this platform");
        false
    }
    
    #[cfg(target_os = "macos")]
    fn map_memory(&mut self, guest_pa: u64, size: u64) -> bool {
        if !self.initialized {
            return false;
        }
        
        // Allocate host memory
        let host_addr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                size as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        
        if host_addr == libc::MAP_FAILED {
            log::error!("Failed to allocate host memory for guest");
            return false;
        }
        
        match self.map_memory_region(host_addr as *mut u8, guest_pa, size as usize, true, true, true) {
            Ok(_) => {
                log::info!("Mapped {} bytes at guest PA {:#x}", size, guest_pa);
                true
            }
            Err(e) => {
                log::error!("Failed to map memory: {}", e);
                unsafe { libc::munmap(host_addr, size as usize); }
                false
            }
        }
    }
    
    #[cfg(not(target_os = "macos"))]
    fn map_memory(&mut self, _guest_pa: u64, _size: u64) -> bool {
        false
    }
    
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn create_vcpu(&mut self, id: u32) -> bool {
        match self.create_vcpu_internal(id) {
            Ok(_) => {
                log::info!("Created vCPU {}", id);
                true
            }
            Err(e) => {
                log::error!("Failed to create vCPU {}: {}", id, e);
                false
            }
        }
    }
    
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    fn create_vcpu(&mut self, _id: u32) -> bool {
        log::warn!("HVF vCPU creation not supported on this architecture");
        false
    }
    
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn run(&mut self) -> bool {
        if self.vcpus.is_empty() {
            return false;
        }
        
        match self.run_vcpu_internal(0) {
            Ok(exit) => {
                log::debug!("vCPU exit: {:?}", exit);
                !matches!(exit, HvfExitReason::Shutdown)
            }
            Err(e) => {
                log::error!("vCPU run failed: {}", e);
                false
            }
        }
    }
    
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    fn run(&mut self) -> bool {
        false
    }
    
    fn inject_interrupt(&mut self, _vector: u32) -> bool {
        // ARM64 interrupt injection via HVF is more complex
        // and requires setting specific system registers
        log::warn!("Interrupt injection not yet implemented for HVF");
        false
    }
}

impl Drop for AccelHvf {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        {
            // Destroy vCPUs first
            #[cfg(target_arch = "aarch64")]
            for vcpu in &self.vcpus {
                unsafe { hvf_sys::hv_vcpu_destroy(vcpu.handle); }
            }
            
            // Unmap memory regions
            for region in &self.memory_regions {
                unsafe {
                    hvf_sys::hv_vm_unmap(region.guest_phys_addr, region.size as usize);
                    libc::munmap(region.host_addr as *mut _, region.size as usize);
                }
            }
            
            // Destroy VM
            if self.initialized {
                unsafe { hvf_sys::hv_vm_destroy(); }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hvf_availability() {
        let available = AccelHvf::is_available();
        println!("HVF available: {}", available);
    }
    
    #[test]
    #[cfg(target_os = "macos")]
    fn test_hvf_init() {
        if !AccelHvf::is_available() {
            println!("Skipping HVF init test - HVF not available");
            return;
        }
        
        let mut accel = AccelHvf::new();
        let result = accel.init();
        println!("HVF init result: {}", result);
    }
}
