//! KVM (Kernel-based Virtual Machine) acceleration backend for Linux
//!
//! This module provides hardware-assisted virtualization on Linux systems
//! using the KVM hypervisor interface.

use super::{Accel, MemFlags as AccelMemFlags};
use std::path::Path;
use std::collections::HashMap;

#[cfg(feature = "kvm")]
use kvm_ioctls::{Kvm, VmFd, VcpuFd, VcpuExit};
#[cfg(feature = "kvm")]
use kvm_bindings::*;

/// Error type for KVM operations
#[derive(Debug, Clone)]
pub enum KvmError {
    /// KVM device not available
    NotAvailable,
    /// Failed to create VM
    CreateVmFailed(String),
    /// Failed to create vCPU
    CreateVcpuFailed(String),
    /// Failed to map memory
    MapMemoryFailed(String),
    /// Failed to run vCPU
    RunFailed(String),
    /// Unsupported operation
    Unsupported(String),
}

impl std::fmt::Display for KvmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KvmError::NotAvailable => write!(f, "KVM device not available"),
            KvmError::CreateVmFailed(s) => write!(f, "Failed to create VM: {}", s),
            KvmError::CreateVcpuFailed(s) => write!(f, "Failed to create vCPU: {}", s),
            KvmError::MapMemoryFailed(s) => write!(f, "Failed to map memory: {}", s),
            KvmError::RunFailed(s) => write!(f, "Failed to run vCPU: {}", s),
            KvmError::Unsupported(s) => write!(f, "Unsupported: {}", s),
        }
    }
}

impl std::error::Error for KvmError {}

/// Extended VM exit reasons
#[derive(Debug, Clone)]
pub enum VmExitReasonKvm {
    Unknown,
    Halt,
    Shutdown,
    IoIn { port: u16, size: u8 },
    IoOut { port: u16, data: Vec<u8> },
    MmioRead { addr: u64, size: u8 },
    MmioWrite { addr: u64, data: Vec<u8> },
    Hypercall,
    InternalError,
}

/// KVM vCPU state
#[cfg(feature = "kvm")]
pub struct KvmVcpu {
    fd: VcpuFd,
    id: u32,
}

#[cfg(not(feature = "kvm"))]
pub struct KvmVcpu {
    id: u32,
}

/// Memory region tracking
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub slot: u32,
    pub guest_phys_addr: u64,
    pub memory_size: u64,
    pub userspace_addr: u64,
    pub flags: u32,
}

/// KVM accelerator implementation
pub struct AccelKvm {
    #[cfg(feature = "kvm")]
    kvm: Option<Kvm>,
    #[cfg(feature = "kvm")]
    vm: Option<VmFd>,
    #[cfg(feature = "kvm")]
    vcpus: Vec<KvmVcpu>,
    
    memory_regions: HashMap<u32, MemoryRegion>,
    next_slot: u32,
    initialized: bool,
}

impl AccelKvm {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "kvm")]
            kvm: None,
            #[cfg(feature = "kvm")]
            vm: None,
            #[cfg(feature = "kvm")]
            vcpus: Vec::new(),
            memory_regions: HashMap::new(),
            next_slot: 0,
            initialized: false,
        }
    }
    
    /// Check if KVM is available on this system
    pub fn is_available() -> bool {
        Path::new("/dev/kvm").exists()
    }
    
    /// Get KVM API version
    #[cfg(feature = "kvm")]
    pub fn api_version(&self) -> Option<i32> {
        self.kvm.as_ref().map(|k| k.get_api_version())
    }
    
    /// Get supported vCPU count
    #[cfg(feature = "kvm")]
    pub fn max_vcpus(&self) -> Option<usize> {
        self.kvm.as_ref().map(|k| k.get_nr_vcpus())
    }
    
    /// Set user memory region
    #[cfg(feature = "kvm")]
    fn set_user_memory_region(&mut self, region: &MemoryRegion) -> Result<(), KvmError> {
        let vm = self.vm.as_ref().ok_or(KvmError::NotAvailable)?;
        
        let kvm_region = kvm_userspace_memory_region {
            slot: region.slot,
            flags: region.flags,
            guest_phys_addr: region.guest_phys_addr,
            memory_size: region.memory_size,
            userspace_addr: region.userspace_addr,
        };
        
        unsafe {
            vm.set_user_memory_region(kvm_region)
                .map_err(|e| KvmError::MapMemoryFailed(e.to_string()))?;
        }
        
        Ok(())
    }
    
    /// Get vCPU registers (x86_64)
    #[cfg(all(feature = "kvm", target_arch = "x86_64"))]
    pub fn get_regs(&self, vcpu_id: usize) -> Result<kvm_regs, KvmError> {
        let vcpu = self.vcpus.get(vcpu_id)
            .ok_or(KvmError::Unsupported("Invalid vCPU ID".into()))?;
        vcpu.fd.get_regs()
            .map_err(|e| KvmError::RunFailed(e.to_string()))
    }
    
    /// Set vCPU registers (x86_64)
    #[cfg(all(feature = "kvm", target_arch = "x86_64"))]
    pub fn set_regs(&self, vcpu_id: usize, regs: &kvm_regs) -> Result<(), KvmError> {
        let vcpu = self.vcpus.get(vcpu_id)
            .ok_or(KvmError::Unsupported("Invalid vCPU ID".into()))?;
        vcpu.fd.set_regs(regs)
            .map_err(|e| KvmError::RunFailed(e.to_string()))
    }
    
    /// Get special registers (x86_64)
    #[cfg(all(feature = "kvm", target_arch = "x86_64"))]
    pub fn get_sregs(&self, vcpu_id: usize) -> Result<kvm_sregs, KvmError> {
        let vcpu = self.vcpus.get(vcpu_id)
            .ok_or(KvmError::Unsupported("Invalid vCPU ID".into()))?;
        vcpu.fd.get_sregs()
            .map_err(|e| KvmError::RunFailed(e.to_string()))
    }
    
    /// Set special registers (x86_64)
    #[cfg(all(feature = "kvm", target_arch = "x86_64"))]
    pub fn set_sregs(&self, vcpu_id: usize, sregs: &kvm_sregs) -> Result<(), KvmError> {
        let vcpu = self.vcpus.get(vcpu_id)
            .ok_or(KvmError::Unsupported("Invalid vCPU ID".into()))?;
        vcpu.fd.set_sregs(sregs)
            .map_err(|e| KvmError::RunFailed(e.to_string()))
    }
    
    /// Run a single vCPU and return exit reason
    #[cfg(feature = "kvm")]
    pub fn run_vcpu(&mut self, vcpu_id: usize) -> Result<VmExitReasonKvm, KvmError> {
        let vcpu = self.vcpus.get_mut(vcpu_id)
            .ok_or(KvmError::Unsupported("Invalid vCPU ID".into()))?;
        
        match vcpu.fd.run() {
            Ok(exit) => {
                match exit {
                    VcpuExit::Hlt => Ok(VmExitReasonKvm::Halt),
                    VcpuExit::IoIn(port, data) => Ok(VmExitReasonKvm::IoIn { port, size: data.len() as u8 }),
                    VcpuExit::IoOut(port, data) => Ok(VmExitReasonKvm::IoOut { port, data: data.to_vec() }),
                    VcpuExit::MmioRead(addr, data) => Ok(VmExitReasonKvm::MmioRead { addr, size: data.len() as u8 }),
                    VcpuExit::MmioWrite(addr, data) => Ok(VmExitReasonKvm::MmioWrite { addr, data: data.to_vec() }),
                    VcpuExit::Shutdown => Ok(VmExitReasonKvm::Shutdown),
                    VcpuExit::InternalError => Ok(VmExitReasonKvm::InternalError),
                    _ => Ok(VmExitReasonKvm::Unknown),
                }
            }
            Err(e) => Err(KvmError::RunFailed(e.to_string())),
        }
    }
}

impl Default for AccelKvm {
    fn default() -> Self {
        Self::new()
    }
}

impl Accel for AccelKvm {
    #[cfg(feature = "kvm")]
    fn init(&mut self) -> bool {
        if !Self::is_available() {
            log::warn!("KVM not available on this system");
            return false;
        }
        
        match Kvm::new() {
            Ok(kvm) => {
                log::info!("KVM initialized, API version: {}", kvm.get_api_version());
                
                match kvm.create_vm() {
                    Ok(vm) => {
                        log::info!("KVM VM created successfully");
                        self.kvm = Some(kvm);
                        self.vm = Some(vm);
                        self.initialized = true;
                        true
                    }
                    Err(e) => {
                        log::error!("Failed to create KVM VM: {}", e);
                        false
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to initialize KVM: {}", e);
                false
            }
        }
    }
    
    #[cfg(not(feature = "kvm"))]
    fn init(&mut self) -> bool {
        log::warn!("KVM support not compiled in");
        false
    }
    
    #[cfg(feature = "kvm")]
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
        
        let region = MemoryRegion {
            slot: self.next_slot,
            guest_phys_addr: guest_pa,
            memory_size: size,
            userspace_addr: host_addr as u64,
            flags: 0,
        };
        
        match self.set_user_memory_region(&region) {
            Ok(_) => {
                self.memory_regions.insert(self.next_slot, region);
                self.next_slot += 1;
                log::info!("Mapped {} bytes at guest PA {:#x}", size, guest_pa);
                true
            }
            Err(e) => {
                log::error!("Failed to map memory: {}", e);
                // Clean up allocated memory
                unsafe { libc::munmap(host_addr, size as usize); }
                false
            }
        }
    }
    
    #[cfg(not(feature = "kvm"))]
    fn map_memory(&mut self, _guest_pa: u64, _size: u64) -> bool {
        false
    }
    
    #[cfg(feature = "kvm")]
    fn create_vcpu(&mut self, id: u32) -> bool {
        if !self.initialized {
            return false;
        }
        
        let vm = match &self.vm {
            Some(v) => v,
            None => return false,
        };
        
        match vm.create_vcpu(id as u64) {
            Ok(vcpu_fd) => {
                let vcpu = KvmVcpu {
                    fd: vcpu_fd,
                    id,
                };
                self.vcpus.push(vcpu);
                log::info!("Created vCPU {}", id);
                true
            }
            Err(e) => {
                log::error!("Failed to create vCPU {}: {}", id, e);
                false
            }
        }
    }
    
    #[cfg(not(feature = "kvm"))]
    fn create_vcpu(&mut self, _id: u32) -> bool {
        false
    }
    
    #[cfg(feature = "kvm")]
    fn run(&mut self) -> bool {
        if self.vcpus.is_empty() {
            return false;
        }
        
        match self.run_vcpu(0) {
            Ok(exit) => {
                log::debug!("vCPU exit: {:?}", exit);
                !matches!(exit, VmExitReasonKvm::Shutdown | VmExitReasonKvm::InternalError)
            }
            Err(e) => {
                log::error!("vCPU run failed: {}", e);
                false
            }
        }
    }
    
    #[cfg(not(feature = "kvm"))]
    fn run(&mut self) -> bool {
        false
    }
    
    #[cfg(feature = "kvm")]
    fn inject_interrupt(&mut self, vector: u32) -> bool {
        if self.vcpus.is_empty() {
            return false;
        }
        
        let irq = kvm_interrupt { irq: vector };
        match self.vcpus[0].fd.interrupt(irq) {
            Ok(_) => {
                log::debug!("Injected interrupt vector {}", vector);
                true
            }
            Err(e) => {
                log::error!("Failed to inject interrupt: {}", e);
                false
            }
        }
    }
    
    #[cfg(not(feature = "kvm"))]
    fn inject_interrupt(&mut self, _vector: u32) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_kvm_availability() {
        let available = AccelKvm::is_available();
        println!("KVM available: {}", available);
    }
    
    #[test]
    #[cfg(feature = "kvm")]
    fn test_kvm_init() {
        if !AccelKvm::is_available() {
            println!("Skipping KVM init test - KVM not available");
            return;
        }
        
        let mut accel = AccelKvm::new();
        let result = accel.init();
        println!("KVM init result: {}", result);
        
        if result {
            println!("KVM API version: {:?}", accel.api_version());
            println!("Max vCPUs: {:?}", accel.max_vcpus());
        }
    }
}
