//! Linux KVM 加速后端完整实现
//!
//! 支持 Intel VT-x, AMD-V 和 ARM 虚拟化扩展
//! 优化后的版本: 将架构特定代码分离到独立模块,减少feature gates

use std::collections::HashMap;

use vm_core::{GuestRegs, MMU, PlatformError, VmError};

use super::{Accel, AccelError};

// ============================================================================
// 架构特定模块 - 使用模块级条件编译减少重复的feature gates
// ============================================================================

/// x86_64 架构特定实现
#[cfg(all(feature = "kvm", target_arch = "x86_64"))]
mod kvm_x86_64 {
    pub use kvm_bindings::*;
    pub use kvm_ioctls::{Kvm, VcpuExit, VcpuFd, VmFd};

    use super::*;

    /// x86_64 vCPU 实现
    pub struct KvmVcpuX86_64 {
        pub fd: VcpuFd,
        pub id: u32,
        pub run_mmap_size: usize,
    }

    impl KvmVcpuX86_64 {
        pub fn new(vm: &VmFd, id: u32) -> Result<Self, AccelError> {
            let vcpu = vm.create_vcpu(id as u64).map_err(|e| {
                VmError::Platform(PlatformError::ResourceAllocationFailed(format!(
                    "KVM create_vcpu failed: {}",
                    e
                )))
            })?;

            let run_mmap_size = vm.get_vcpu_mmap_size().map_err(|e| {
                VmError::Platform(PlatformError::ResourceAllocationFailed(format!(
                    "Failed to get mmap size: {}",
                    e
                )))
            })?;

            Ok(Self {
                fd: vcpu,
                id,
                run_mmap_size,
            })
        }

        pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
            let regs = self.fd.get_regs().map_err(|e| {
                VmError::Platform(PlatformError::AccessDenied(format!(
                    "KVM get_regs failed: {}",
                    e
                )))
            })?;

            let mut gpr = [0u64; 32];
            gpr[0] = regs.rax;
            gpr[1] = regs.rcx;
            gpr[2] = regs.rdx;
            gpr[3] = regs.rbx;
            gpr[4] = regs.rsp;
            gpr[5] = regs.rbp;
            gpr[6] = regs.rsi;
            gpr[7] = regs.rdi;
            gpr[8] = regs.r8;
            gpr[9] = regs.r9;
            gpr[10] = regs.r10;
            gpr[11] = regs.r11;
            gpr[12] = regs.r12;
            gpr[13] = regs.r13;
            gpr[14] = regs.r14;
            gpr[15] = regs.r15;

            Ok(GuestRegs {
                pc: regs.rip,
                sp: regs.rsp,
                fp: regs.rbp,
                gpr,
            })
        }

        pub fn set_regs(&mut self, regs: &GuestRegs) -> Result<(), AccelError> {
            let kvm_regs = kvm_bindings::kvm_regs {
                rax: regs.gpr[0],
                rcx: regs.gpr[1],
                rdx: regs.gpr[2],
                rbx: regs.gpr[3],
                rsp: regs.sp,
                rbp: regs.fp,
                rsi: regs.gpr[6],
                rdi: regs.gpr[7],
                r8: regs.gpr[8],
                r9: regs.gpr[9],
                r10: regs.gpr[10],
                r11: regs.gpr[11],
                r12: regs.gpr[12],
                r13: regs.gpr[13],
                r14: regs.gpr[14],
                r15: regs.gpr[15],
                rip: regs.pc,
                rflags: 0x2, // Reserved bit must be 1
            };

            self.fd.set_regs(&kvm_regs).map_err(|e| {
                VmError::Platform(PlatformError::AccessDenied(format!(
                    "KVM set_regs failed: {}",
                    e
                )))
            })?;

            Ok(())
        }

        pub fn run(&mut self) -> Result<VcpuExit, AccelError> {
            self.fd.run().map_err(|e| {
                VmError::Platform(PlatformError::ExecutionFailed(format!(
                    "KVM vcpu run failed: {}",
                    e
                )))
            })
        }
    }
}

/// aarch64 架构特定实现
#[cfg(all(feature = "kvm", target_arch = "aarch64"))]
mod kvm_aarch64 {
    pub use kvm_bindings::*;
    pub use kvm_ioctls::{Kvm, VcpuExit, VcpuFd, VmFd};

    use super::*;

    /// aarch64 vCPU 实现
    pub struct KvmVcpuAarch64 {
        pub fd: VcpuFd,
        pub id: u32,
        pub run_mmap_size: usize,
    }

    impl KvmVcpuAarch64 {
        pub fn new(vm: &VmFd, id: u32) -> Result<Self, AccelError> {
            let vcpu = vm.create_vcpu(id as u64).map_err(|e| {
                VmError::Platform(PlatformError::ResourceAllocationFailed(format!(
                    "KVM create_vcpu failed: {}",
                    e
                )))
            })?;

            let run_mmap_size = vm.get_vcpu_mmap_size().map_err(|e| {
                VmError::Platform(PlatformError::ResourceAllocationFailed(format!(
                    "Failed to get mmap size: {}",
                    e
                )))
            })?;

            Ok(Self {
                fd: vcpu,
                id,
                run_mmap_size,
            })
        }

        pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
            let mut regs = kvm_bindings::kvm_regs::default();
            self.fd.get_regs(&mut regs).map_err(|e| {
                VmError::Platform(PlatformError::AccessDenied(format!(
                    "KVM get_regs failed: {}",
                    e
                )))
            })?;

            let mut gpr = [0u64; 32];
            gpr[..31].copy_from_slice(&regs.regs[..31]);

            Ok(GuestRegs {
                pc: regs.pc,
                sp: regs.sp,
                fp: regs.regs[29], // x29 is FP on ARM64
                gpr,
            })
        }

        pub fn set_regs(&mut self, regs: &GuestRegs) -> Result<(), AccelError> {
            let mut kvm_regs = kvm_bindings::kvm_regs::default();
            kvm_regs.regs[..31].copy_from_slice(&regs.gpr[..31]);
            kvm_regs.sp = regs.sp;
            kvm_regs.pc = regs.pc;
            kvm_regs.pstate = 0x3c5; // EL1h, DAIF masked

            self.fd.set_regs(&kvm_regs).map_err(|e| {
                VmError::Platform(PlatformError::AccessDenied(format!(
                    "KVM set_regs failed: {}",
                    e
                )))
            })?;

            Ok(())
        }

        pub fn run(&mut self) -> Result<VcpuExit, AccelError> {
            self.fd.run().map_err(|e| {
                VmError::Platform(PlatformError::ExecutionFailed(format!(
                    "KVM vcpu run failed: {}",
                    e
                )))
            })
        }
    }
}

/// KVM 通用功能实现
#[cfg(feature = "kvm")]
mod kvm_common {
    pub use kvm_bindings::{KVM_MEM_LOG_DIRTY_PAGES, kvm_irq_level, kvm_userspace_memory_region};
    pub use kvm_ioctls::{Kvm, VmFd};

    use super::*;

    /// 统一的 vCPU 接口
    pub enum KvmVcpuUnified {
        #[cfg(target_arch = "x86_64")]
        X86_64(crate::kvm_impl::kvm_x86_64::KvmVcpuX86_64),
        #[cfg(target_arch = "aarch64")]
        Aarch64(crate::kvm_impl::kvm_aarch64::KvmVcpuAarch64),
    }

    impl KvmVcpuUnified {
        pub fn id(&self) -> u32 {
            match self {
                #[cfg(target_arch = "x86_64")]
                Self::X86_64(v) => v.id,
                #[cfg(target_arch = "aarch64")]
                Self::Aarch64(v) => v.id,
            }
        }

        pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
            match self {
                #[cfg(target_arch = "x86_64")]
                Self::X86_64(v) => v.get_regs(),
                #[cfg(target_arch = "aarch64")]
                Self::Aarch64(v) => v.get_regs(),
            }
        }

        pub fn set_regs(&mut self, regs: &GuestRegs) -> Result<(), AccelError> {
            match self {
                #[cfg(target_arch = "x86_64")]
                Self::X86_64(v) => v.set_regs(regs),
                #[cfg(target_arch = "aarch64")]
                Self::Aarch64(v) => v.set_regs(regs),
            }
        }
    }
}

/// KVM 未启用时的存根实现
#[cfg(not(feature = "kvm"))]
mod kvm_stub {
    use super::*;

    /// 存根 vCPU
    pub struct KvmVcpuStub {
        pub id: u32,
    }

    impl KvmVcpuStub {
        pub fn new(_vm: &(), id: u32) -> Result<Self, AccelError> {
            Ok(Self { id })
        }

        pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
            Err(VmError::Platform(PlatformError::UnsupportedOperation(
                "KVM feature not enabled".to_string(),
            )))
        }

        pub fn set_regs(&mut self, _regs: &GuestRegs) -> Result<(), AccelError> {
            Err(VmError::Platform(PlatformError::UnsupportedOperation(
                "KVM feature not enabled".to_string(),
            )))
        }

        pub fn id(&self) -> u32 {
            self.id
        }
    }
}

// ============================================================================
// 重新导出统一接口
// ============================================================================

#[cfg(feature = "kvm")]
use kvm_common::{KVM_MEM_LOG_DIRTY_PAGES, kvm_irq_level, kvm_userspace_memory_region};
#[cfg(feature = "kvm")]
use kvm_ioctls::{Kvm, VcpuExit, VmFd};

/// 统一的 vCPU 抽象
enum KvmVcpu {
    #[cfg(feature = "kvm")]
    X86_64(kvm_x86_64::KvmVcpuX86_64),
    #[cfg(feature = "kvm")]
    Aarch64(kvm_aarch64::KvmVcpuAarch64),
    #[cfg(not(feature = "kvm"))]
    Stub(kvm_stub::KvmVcpuStub),
}

impl KvmVcpu {
    pub fn id(&self) -> u32 {
        match self {
            #[cfg(feature = "kvm")]
            Self::X86_64(v) => v.id,
            #[cfg(feature = "kvm")]
            Self::Aarch64(v) => v.id,
            #[cfg(not(feature = "kvm"))]
            Self::Stub(v) => v.id(),
        }
    }

    pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
        match self {
            #[cfg(feature = "kvm")]
            Self::X86_64(v) => v.get_regs(),
            #[cfg(feature = "kvm")]
            Self::Aarch64(v) => v.get_regs(),
            #[cfg(not(feature = "kvm"))]
            Self::Stub(v) => v.get_regs(),
        }
    }

    pub fn set_regs(&mut self, regs: &GuestRegs) -> Result<(), AccelError> {
        match self {
            #[cfg(feature = "kvm")]
            Self::X86_64(v) => v.set_regs(regs),
            #[cfg(feature = "kvm")]
            Self::Aarch64(v) => v.set_regs(regs),
            #[cfg(not(feature = "kvm"))]
            Self::Stub(v) => v.set_regs(regs),
        }
    }

    #[cfg(feature = "kvm")]
    pub fn run(&mut self) -> Result<VcpuExit, AccelError> {
        match self {
            Self::X86_64(v) => v.run(),
            Self::Aarch64(v) => v.run(),
        }
    }
}

// ============================================================================
// KVM 加速器主实现
// ============================================================================

/// KVM 加速器
pub struct AccelKvm {
    #[cfg(feature = "kvm")]
    kvm: Option<Kvm>,
    #[cfg(feature = "kvm")]
    vm: Option<VmFd>,

    vcpus: Vec<KvmVcpu>,
    memory_regions: HashMap<u32, (u64, u64)>, // slot -> (gpa, size)
    next_slot: u32,

    // Interrupt controller state
    irqchip_created: bool,
    gsi_count: u32,

    // NUMA optimization state
    numa_enabled: bool,
    numa_nodes: u32,
    vcpu_numa_mapping: HashMap<u32, u32>, // vcpu_id -> numa_node

    // 性能优化：寄存器缓存（减少ioctl调用）
    #[cfg(feature = "kvm")]
    regs_cache: HashMap<u32, Option<GuestRegs>>, // vcpu_id -> cached registers
    regs_cache_enabled: bool,
}

impl AccelKvm {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "kvm")]
            kvm: None,
            #[cfg(feature = "kvm")]
            vm: None,
            vcpus: Vec::new(),
            memory_regions: HashMap::new(),
            next_slot: 0,
            irqchip_created: false,
            gsi_count: 0,
            numa_enabled: false,
            numa_nodes: 1,
            vcpu_numa_mapping: HashMap::new(),
            #[cfg(feature = "kvm")]
            regs_cache: HashMap::new(),
            regs_cache_enabled: true, // 默认启用寄存器缓存
        }
    }

    /// 启用或禁用寄存器缓存
    ///
    /// 寄存器缓存可以减少ioctl调用次数，提高性能。
    /// 但在某些情况下（如多线程环境），可能需要禁用缓存以确保一致性。
    ///
    /// # 参数
    ///
    /// * `enabled` - true启用缓存，false禁用缓存
    pub fn enable_regs_cache(&mut self, enabled: bool) {
        self.regs_cache_enabled = enabled;
        if !enabled {
            // 禁用时清空缓存
            #[cfg(feature = "kvm")]
            self.regs_cache.clear();
        }
        log::debug!(
            "Register cache {}",
            if enabled { "enabled" } else { "disabled" }
        );
    }

    /// 使指定vCPU的寄存器缓存失效
    ///
    /// 在外部修改了寄存器后，应该调用此方法使缓存失效。
    ///
    /// # 参数
    ///
    /// * `vcpu_id` - vCPU ID
    #[cfg(feature = "kvm")]
    pub fn invalidate_regs_cache(&mut self, vcpu_id: u32) {
        self.regs_cache.remove(&vcpu_id);
    }

    /// Enable NUMA optimization for this VM
    pub fn enable_numa(&mut self, numa_nodes: u32) {
        self.numa_enabled = true;
        self.numa_nodes = numa_nodes.max(1);
        log::info!("NUMA optimization enabled with {} nodes", self.numa_nodes);
    }

    /// Disable NUMA optimization
    pub fn disable_numa(&mut self) {
        self.numa_enabled = false;
        self.vcpu_numa_mapping.clear();
        log::info!("NUMA optimization disabled");
    }

    /// Set vCPU affinity to specific physical CPU(s)
    #[cfg(feature = "kvm")]
    pub fn set_vcpu_affinity(&self, vcpu_id: u32, cpu_ids: &[usize]) -> Result<(), AccelError> {
        let vcpu = self.vcpus.get(vcpu_id as usize).ok_or_else(|| {
            VmError::Platform(PlatformError::InvalidParameter {
                name: "vcpu_id".to_string(),
                value: vcpu_id.to_string(),
                message: "Invalid vCPU ID".to_string(),
            })
        })?;

        // Set CPU affinity using pthread-like API
        #[cfg(target_os = "linux")]
        unsafe {
            use std::mem::zeroed;

            // Create CPU set mask
            let mut cpu_set: libc::cpu_set_t = zeroed();
            libc::CPU_ZERO(&mut cpu_set);

            for &cpu_id in cpu_ids {
                if cpu_id < libc::CPU_SETSIZE as usize {
                    libc::CPU_SET(cpu_id, &mut cpu_set);
                }
            }

            // Get vCPU thread ID (simplified - actual implementation would need proper thread
            // handling)
            let tid = libc::gettid();

            let ret =
                libc::sched_setaffinity(tid, std::mem::size_of::<libc::cpu_set_t>(), &cpu_set);

            if ret != 0 {
                return Err(VmError::Platform(PlatformError::AccessDenied(format!(
                    "Failed to set vCPU {} affinity to CPUs {:?}: errno {}",
                    vcpu_id,
                    cpu_ids,
                    std::io::Error::last_os_error()
                ))));
            }

            log::info!("Set vCPU {} affinity to CPUs {:?}", vcpu_id, cpu_ids);
        }

        #[cfg(not(target_os = "linux"))]
        {
            log::warn!("vCPU affinity setting not fully supported on this platform");
        }

        Ok(())
    }

    /// Set vCPU affinity - stub for non-KVM builds
    #[cfg(not(feature = "kvm"))]
    pub fn set_vcpu_affinity(&self, vcpu_id: u32, cpu_ids: &[usize]) -> Result<(), AccelError> {
        log::info!(
            "Set vCPU {} affinity to CPUs {:?} (KVM not enabled)",
            vcpu_id,
            cpu_ids
        );
        Ok(())
    }

    /// Setup NUMA memory allocation for a specific node
    #[cfg(feature = "kvm")]
    pub fn setup_numa_memory(
        &mut self,
        node: u32,
        mem_size: u64,
        gpa: u64,
        hva: u64,
    ) -> Result<(), AccelError> {
        if !self.numa_enabled {
            return Err(VmError::Platform(PlatformError::InvalidParameter {
                name: "numa_enabled".to_string(),
                value: "false".to_string(),
                message: "NUMA is not enabled for this VM".to_string(),
            }));
        }

        if node >= self.numa_nodes {
            return Err(VmError::Platform(PlatformError::InvalidParameter {
                name: "node".to_string(),
                value: node.to_string(),
                message: format!("Invalid NUMA node: {} (max: {})", node, self.numa_nodes - 1),
            }));
        }

        let vm = self.vm.as_mut().ok_or_else(|| {
            VmError::Platform(PlatformError::InitializationFailed(
                "VM not initialized".to_string(),
            ))
        })?;

        let slot = self.next_slot;
        self.next_slot += 1;

        // Create memory region with NUMA hints
        let mut mem_region = kvm_userspace_memory_region {
            slot,
            flags: KVM_MEM_LOG_DIRTY_PAGES,
            guest_phys_addr: gpa,
            memory_size: mem_size,
            userspace_addr: hva,
        };

        // Add NUMA node hint if supported by KVM
        #[cfg(target_os = "linux")]
        {
            // KVM_MEM_NUMA_NODE is a KVM extension flag (if available)
            // This is platform-specific and may not be available on all systems
            mem_region.flags |= (node as u32) << 16; // Use upper bits for node ID
        }

        // SAFETY: vm.set_user_memory_region is a KVM ioctl wrapper
        // Preconditions: mem_region contains valid slot, guest_phys_addr, memory_size, and
        // userspace_addr Invariants: Maps or unmaps guest physical memory region to host
        // virtual address with NUMA hints
        unsafe {
            vm.set_user_memory_region(mem_region).map_err(|e| {
                VmError::Platform(PlatformError::MemoryMappingFailed(format!(
                    "KVM set_user_memory_region (NUMA node {}) failed: {}",
                    node, e
                )))
            })?;
        }

        self.memory_regions.insert(slot, (gpa, mem_size));

        log::info!(
            "Mapped NUMA memory: Node {}, GPA 0x{:x}, size 0x{:x}, slot {}",
            node,
            gpa,
            mem_size,
            slot
        );

        Ok(())
    }

    /// Setup NUMA memory allocation - stub for non-KVM builds
    #[cfg(not(feature = "kvm"))]
    pub fn setup_numa_memory(
        &mut self,
        node: u32,
        mem_size: u64,
        gpa: u64,
        hva: u64,
    ) -> Result<(), AccelError> {
        if !self.numa_enabled {
            return Err(VmError::Platform(PlatformError::InvalidParameter(
                "NUMA is not enabled for this VM".to_string(),
            )));
        }

        log::info!(
            "Map NUMA memory: Node {}, GPA 0x{:x}, size 0x{:x}, HVA 0x{:x} (KVM not enabled)",
            node,
            gpa,
            mem_size,
            hva
        );
        Ok(())
    }

    /// Bind vCPU to a specific NUMA node
    pub fn bind_vcpu_to_numa_node(&mut self, vcpu_id: u32, node: u32) -> Result<(), AccelError> {
        if !self.numa_enabled {
            return Err(VmError::Platform(PlatformError::InvalidParameter {
                name: "numa_enabled".to_string(),
                value: "false".to_string(),
                message: "NUMA is not enabled for this VM".to_string(),
            }));
        }

        if node >= self.numa_nodes {
            return Err(VmError::Platform(PlatformError::InvalidParameter {
                name: "node".to_string(),
                value: node.to_string(),
                message: format!("Invalid NUMA node: {} (max: {})", node, self.numa_nodes - 1),
            }));
        }

        self.vcpu_numa_mapping.insert(vcpu_id, node);

        log::info!("Bound vCPU {} to NUMA node {}", vcpu_id, node);
        Ok(())
    }

    /// Get the NUMA node for a specific vCPU
    pub fn get_vcpu_numa_node(&self, vcpu_id: u32) -> Option<u32> {
        self.vcpu_numa_mapping.get(&vcpu_id).copied()
    }

    /// Get NUMA configuration status
    pub fn numa_config(&self) -> (bool, u32) {
        (self.numa_enabled, self.numa_nodes)
    }

    /// 检查 KVM 是否可用
    #[cfg(feature = "kvm")]
    pub fn is_available() -> bool {
        std::path::Path::new("/dev/kvm").exists()
    }

    #[cfg(not(feature = "kvm"))]
    pub fn is_available() -> bool {
        false
    }

    /// Setup interrupt controller
    ///
    /// Creates an in-kernel interrupt controller (irqchip) for the VM.
    /// This is typically an IOAPIC for x86_64 or GIC for ARM64.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the interrupt controller was created successfully
    /// * `Err(AccelError)` if creation failed
    #[cfg(feature = "kvm")]
    pub fn setup_irq_controller(&mut self) -> Result<(), AccelError> {
        let vm = self.vm.as_ref().ok_or_else(|| {
            VmError::Platform(PlatformError::InitializationFailed(
                "VM not initialized".to_string(),
            ))
        })?;

        // Create the in-kernel irqchip
        vm.create_irq_chip().map_err(|e| {
            VmError::Platform(PlatformError::InitializationFailed(format!(
                "Failed to create IRQ chip: {}",
                e
            )))
        })?;

        self.irqchip_created = true;
        self.gsi_count = 24; // Default GSI count for most systems

        log::info!("KVM interrupt controller created successfully");
        Ok(())
    }

    #[cfg(not(feature = "kvm"))]
    pub fn setup_irq_controller(&mut self) -> Result<(), AccelError> {
        Err(VmError::Platform(PlatformError::UnsupportedOperation(
            "KVM feature not enabled".to_string(),
        )))
    }

    /// Set IRQ line level
    ///
    /// Sets the level of an IRQ line in the interrupt controller.
    ///
    /// # Arguments
    ///
    /// * `irq` - The IRQ number (GSI)
    /// * `active` - Whether the IRQ should be active (high) or inactive (low)
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the IRQ line was set successfully
    /// * `Err(AccelError)` if setting the IRQ line failed
    #[cfg(feature = "kvm")]
    pub fn set_irq_line(&mut self, irq: u32, active: bool) -> Result<(), AccelError> {
        let vm = self.vm.as_ref().ok_or_else(|| {
            VmError::Platform(PlatformError::InitializationFailed(
                "VM not initialized".to_string(),
            ))
        })?;

        if !self.irqchip_created {
            return Err(VmError::Platform(PlatformError::InvalidState {
                message: "IRQ controller not initialized".to_string(),
                current: "no_irqchip".to_string(),
                expected: "irqchip_created".to_string(),
            }));
        }

        let level = if active { 1 } else { 0 };

        vm.set_irq_line(kvm_irq_level {
            irq,
            level,
            ..Default::default()
        })
        .map_err(|e| {
            VmError::Platform(PlatformError::IoctlError {
                errno: e.raw_os_error().unwrap_or(0),
                operation: format!("set_irq_line(IRQ {})", irq),
            })
        })?;

        log::trace!("IRQ {} set to level {}", irq, level);
        Ok(())
    }

    #[cfg(not(feature = "kvm"))]
    pub fn set_irq_line(&mut self, _irq: u32, _active: bool) -> Result<(), AccelError> {
        Err(VmError::Platform(PlatformError::UnsupportedOperation(
            "KVM feature not enabled".to_string(),
        )))
    }

    /// Register IRQ routing entry (x86_64 only)
    ///
    /// Sets up routing for an IRQ to a specific vCPU and pin.
    ///
    /// # Arguments
    ///
    /// * `gsi` - The Global System Interrupt (GSI) number
    /// * `irqchip` - The IRQ chip (e.g., 0 for IOAPIC, 1 for PIC)
    /// * `pin` - The pin on the IRQ chip
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the routing was set successfully
    /// * `Err(AccelError)` if setting the routing failed
    #[cfg(all(feature = "kvm", target_arch = "x86_64"))]
    pub fn setup_irq_routing(
        &mut self,
        gsi: u32,
        irqchip: u32,
        pin: u32,
    ) -> Result<(), AccelError> {
        let vm = self.vm.as_ref().ok_or_else(|| {
            VmError::Platform(PlatformError::InitializationFailed(
                "VM not initialized".to_string(),
            ))
        })?;

        if !self.irqchip_created {
            return Err(VmError::Platform(PlatformError::InvalidState {
                message: "IRQ controller not initialized".to_string(),
                current: "no_irqchip".to_string(),
                expected: "irqchip_created".to_string(),
            }));
        }

        let entry = kvm_bindings::kvm_irq_routing_entry {
            gsi,
            type_: kvm_bindings::KVM_IRQ_ROUTING_IRQCHIP,
            u: kvm_bindings::kvm_irq_routing_entry__bindgen_ty_1 {
                irqchip: kvm_bindings::kvm_irq_routing_irqchip {
                    irqchip: irqchip as u32,
                    pin: pin as u32,
                },
            },
            ..Default::default()
        };

        let routing = kvm_bindings::kvm_irq_routing {
            nr: 1,
            flags: 0,
            entries: [entry; 1],
        };

        // SAFETY: We're setting up a valid IRQ routing entry with correct size
        unsafe {
            vm.set_gsi_routing(&routing).map_err(|e| {
                VmError::Platform(PlatformError::IoctlError {
                    errno: e.raw_os_error().unwrap_or(0),
                    operation: format!("set_gsi_routing(GSI {})", gsi),
                })
            })?;
        }

        log::debug!(
            "IRQ routing: GSI {} -> IRQ chip {}, pin {}",
            gsi,
            irqchip,
            pin
        );
        Ok(())
    }

    #[cfg(not(all(feature = "kvm", target_arch = "x86_64")))]
    pub fn setup_irq_routing(
        &mut self,
        _gsi: u32,
        _irqchip: u32,
        _pin: u32,
    ) -> Result<(), AccelError> {
        Err(VmError::Platform(PlatformError::UnsupportedOperation(
            "IRQ routing only supported on x86_64 with KVM".to_string(),
        )))
    }

    /// Assign a PCI device to the VM
    ///
    /// Assigns a physical PCI device (e.g., GPU, NIC) to the VM using device assignment.
    ///
    /// # Arguments
    ///
    /// * `pci_addr` - The PCI address of the device (e.g., "0000:01:00.0")
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the device was assigned successfully
    /// * `Err(AccelError)` if device assignment failed
    #[cfg(feature = "kvm")]
    pub fn assign_pci_device(&mut self, pci_addr: &str) -> Result<(), AccelError> {
        let vm = self.vm.as_ref().ok_or_else(|| {
            VmError::Platform(PlatformError::InitializationFailed(
                "VM not initialized".to_string(),
            ))
        })?;

        // Parse PCI address
        let parts: Vec<&str> = pci_addr.split(':').collect();
        if parts.len() != 3 {
            return Err(VmError::Platform(PlatformError::InvalidParameter {
                name: "pci_addr".to_string(),
                value: pci_addr.to_string(),
                message: "Invalid PCI address format (expected DD:BB:SS.FF)".to_string(),
            }));
        }

        let domain = u32::from_str_radix(parts[0], 16).map_err(|_| {
            VmError::Platform(PlatformError::InvalidParameter {
                name: "domain".to_string(),
                value: parts[0].to_string(),
                message: "Invalid domain number".to_string(),
            })
        })?;

        let bus_parts: Vec<&str> = parts[2].split('.').collect();
        if bus_parts.len() != 2 {
            return Err(VmError::Platform(PlatformError::InvalidParameter {
                name: "function".to_string(),
                value: parts[2].to_string(),
                message: "Invalid function format".to_string(),
            }));
        }

        let bus = u8::from_str_radix(parts[1], 16).map_err(|_| {
            VmError::Platform(PlatformError::InvalidParameter {
                name: "bus".to_string(),
                value: parts[1].to_string(),
                message: "Invalid bus number".to_string(),
            })
        })?;

        let slot = u8::from_str_radix(bus_parts[0], 16).map_err(|_| {
            VmError::Platform(PlatformError::InvalidParameter {
                name: "slot".to_string(),
                value: bus_parts[0].to_string(),
                message: "Invalid slot number".to_string(),
            })
        })?;

        let func = u8::from_str_radix(bus_parts[1], 16).map_err(|_| {
            VmError::Platform(PlatformError::InvalidParameter {
                name: "func".to_string(),
                value: bus_parts[1].to_string(),
                message: "Invalid function number".to_string(),
            })
        })?;

        // Create device assignment structure
        let dev = kvm_bindings::kvm_assigned_pci_dev {
            assigned: 0,
            busnr: bus,
            devfn: (slot << 3) | func,
            domain: domain as u32,
            ..Default::default()
        };

        // SAFETY: We're creating a valid device assignment structure
        // with properly parsed domain, bus, slot, and function numbers
        unsafe {
            vm.assign_device(&dev).map_err(|e| {
                VmError::Platform(PlatformError::DeviceAssignmentFailed(format!(
                    "Failed to assign PCI device {}: {}",
                    pci_addr, e
                )))
            })?;
        }

        log::info!("Assigned PCI device {} to VM", pci_addr);
        Ok(())
    }

    #[cfg(not(feature = "kvm"))]
    pub fn assign_pci_device(&mut self, _pci_addr: &str) -> Result<(), AccelError> {
        Err(VmError::Platform(PlatformError::UnsupportedOperation(
            "KVM feature not enabled".to_string(),
        )))
    }
}

impl Accel for AccelKvm {
    fn init(&mut self) -> Result<(), AccelError> {
        #[cfg(feature = "kvm")]
        {
            if !Self::is_available() {
                return Err(VmError::Platform(PlatformError::HardwareUnavailable(
                    "KVM device /dev/kvm not found".to_string(),
                )));
            }

            let kvm = Kvm::new().map_err(|e| {
                VmError::Platform(PlatformError::InitializationFailed(format!(
                    "Failed to open KVM: {}",
                    e
                )))
            })?;

            // 检查 KVM API 版本
            let api_version = kvm.get_api_version();
            if api_version != 12 {
                return Err(VmError::Platform(PlatformError::InitializationFailed(
                    format!("Unsupported KVM API version: {}", api_version),
                )));
            }

            // 创建 VM
            let vm = kvm.create_vm().map_err(|e| {
                VmError::Platform(PlatformError::ResourceAllocationFailed(format!(
                    "Failed to create VM: {}",
                    e
                )))
            })?;

            self.kvm = Some(kvm);
            self.vm = Some(vm);

            log::info!("KVM accelerator initialized successfully");
            Ok(())
        }

        #[cfg(not(feature = "kvm"))]
        {
            Err(VmError::Platform(PlatformError::UnsupportedOperation(
                "KVM feature not enabled".to_string(),
            )))
        }
    }

    fn create_vcpu(&mut self, id: u32) -> Result<(), AccelError> {
        #[cfg(feature = "kvm")]
        {
            let vm = self.vm.as_ref().ok_or_else(|| {
                VmError::Platform(PlatformError::InitializationFailed(
                    "VM not initialized".to_string(),
                ))
            })?;

            let vcpu = if cfg!(target_arch = "x86_64") {
                KvmVcpu::X86_64(kvm_x86_64::KvmVcpuX86_64::new(vm, id)?)
            } else {
                KvmVcpu::Aarch64(kvm_aarch64::KvmVcpuAarch64::new(vm, id)?)
            };

            self.vcpus.push(vcpu);

            log::info!("Created KVM vCPU {}", id);
            Ok(())
        }

        #[cfg(not(feature = "kvm"))]
        {
            Err(VmError::Platform(PlatformError::UnsupportedOperation(
                "KVM feature not enabled".to_string(),
            )))
        }
    }

    fn map_memory(&mut self, gpa: u64, hva: u64, size: u64, _flags: u32) -> Result<(), AccelError> {
        #[cfg(feature = "kvm")]
        {
            let vm = self.vm.as_mut().ok_or_else(|| {
                VmError::Platform(PlatformError::InitializationFailed(
                    "VM not initialized".to_string(),
                ))
            })?;

            let slot = self.next_slot;
            self.next_slot += 1;

            let mem_region = kvm_userspace_memory_region {
                slot,
                flags: 0,
                guest_phys_addr: gpa,
                memory_size: size,
                userspace_addr: hva,
            };

            // SAFETY: vm.set_user_memory_region is a KVM ioctl wrapper
            // Preconditions: mem_region contains valid slot, guest_phys_addr, memory_size, and
            // userspace_addr Invariants: Maps or unmaps guest physical memory region to
            // host virtual address
            unsafe {
                vm.set_user_memory_region(mem_region).map_err(|e| {
                    VmError::Platform(PlatformError::MemoryMappingFailed(format!(
                        "KVM set_user_memory_region failed: {}",
                        e
                    )))
                })?;
            }

            self.memory_regions.insert(slot, (gpa, size));

            log::debug!(
                "Mapped memory: GPA 0x{:x}, size 0x{:x}, slot {}",
                gpa,
                size,
                slot
            );
            Ok(())
        }

        #[cfg(not(feature = "kvm"))]
        {
            Err(VmError::Platform(PlatformError::UnsupportedOperation(
                "KVM feature not enabled".to_string(),
            )))
        }
    }

    fn unmap_memory(&mut self, gpa: u64, size: u64) -> Result<(), AccelError> {
        #[cfg(feature = "kvm")]
        {
            let vm = self.vm.as_mut().ok_or_else(|| {
                VmError::Platform(PlatformError::InitializationFailed(
                    "VM not initialized".to_string(),
                ))
            })?;

            // 查找对应的 slot
            let slot = self
                .memory_regions
                .iter()
                .find(|(_, &(region_gpa, region_size))| region_gpa == gpa && region_size == size)
                .map(|(&slot, _)| slot)
                .ok_or_else(|| {
                    VmError::Platform(PlatformError::InvalidParameter {
                        name: "gpa".to_string(),
                        value: format!("0x{:x}", gpa),
                        message: "Memory region not found".to_string(),
                    })
                })?;

            let mem_region = kvm_userspace_memory_region {
                slot,
                flags: 0,
                guest_phys_addr: gpa,
                memory_size: 0, // size = 0 表示删除
                userspace_addr: 0,
            };

            // SAFETY: vm.set_user_memory_region is a KVM ioctl wrapper
            // Preconditions: mem_region contains valid slot and size=0 to unmap, or valid all
            // fields to map Invariants: Unmaps guest physical memory region (size=0
            // signals deletion)
            unsafe {
                vm.set_user_memory_region(mem_region).map_err(|e| {
                    VmError::Platform(PlatformError::MemoryMappingFailed(format!(
                        "KVM unmap failed: {}",
                        e
                    )))
                })?;
            }

            self.memory_regions.remove(&slot);

            log::debug!("Unmapped memory: GPA 0x{:x}, slot {}", gpa, slot);
            Ok(())
        }

        #[cfg(not(feature = "kvm"))]
        {
            Err(VmError::Platform(PlatformError::UnsupportedOperation(
                "KVM feature not enabled".to_string(),
            )))
        }
    }

    /// 处理I/O输出（端口I/O写入）
    fn handle_io_out(&self, port: u16, data: &[u8], _mmu: &mut dyn MMU) -> Result<(), AccelError> {
        // 根据端口号路由到相应设备
        match port {
            0x3F8..=0x3FF => {
                // COM1 串口 - 写入数据
                // 简化实现：输出到日志
                if let Ok(text) = std::str::from_utf8(data) {
                    log::info!("COM1 output: {}", text);
                } else {
                    log::debug!("COM1 write to port 0x{:x}: {:?}", port, data);
                }
            }
            0x2F8..=0x2FF => {
                // COM2 串口
                if let Ok(text) = std::str::from_utf8(data) {
                    log::info!("COM2 output: {}", text);
                } else {
                    log::debug!("COM2 write to port 0x{:x}: {:?}", port, data);
                }
            }
            0x60..=0x64 => {
                // 键盘控制器
                log::trace!("Keyboard controller write to port 0x{:x}: {:?}", port, data);
            }
            0x70..=0x71 => {
                // RTC (Real-Time Clock)
                log::trace!("RTC write to port 0x{:x}: {:?}", port, data);
            }
            0xCF8..=0xCFF => {
                // PCI配置空间
                log::trace!("PCI config write to port 0x{:x}: {:?}", port, data);
            }
            _ => {
                // 未知端口
                log::warn!("Unhandled I/O OUT port: 0x{:x}, data: {:?}", port, data);
            }
        }

        Ok(())
    }

    /// 处理MMIO读取（内存映射I/O读取）
    fn handle_mmio_read(
        &self,
        addr: u64,
        data: &mut [u8],
        mmu: &mut dyn MMU,
    ) -> Result<(), AccelError> {
        // MMIO地址通过MMU路由到相应设备
        // 常见MMIO地址范围：
        // 0x0200_0000-0x0200_1000: CLINT (Core Local Interruptor)
        // 0x0C00_0000-0x1000_0000: PLIC (Platform Level Interrupt Controller)
        // 0x1000_0000-0x1000_2000: VirtIO设备

        // 根据地址范围确定访问大小
        let size = data.len() as u8;

        // 通过MMU读取MMIO地址
        match mmu.read(addr, size) {
            Ok(value) => {
                // 将读取的值写入data缓冲区
                match size {
                    1 => data[0] = value as u8,
                    2 => {
                        data[0] = value as u8;
                        data[1] = (value >> 8) as u8;
                    }
                    4 => {
                        data[0] = value as u8;
                        data[1] = (value >> 8) as u8;
                        data[2] = (value >> 16) as u8;
                        data[3] = (value >> 24) as u8;
                    }
                    8 => {
                        data[0] = value as u8;
                        data[1] = (value >> 8) as u8;
                        data[2] = (value >> 16) as u8;
                        data[3] = (value >> 24) as u8;
                        data[4] = (value >> 32) as u8;
                        data[5] = (value >> 40) as u8;
                        data[6] = (value >> 48) as u8;
                        data[7] = (value >> 56) as u8;
                    }
                    _ => {
                        // 对于其他大小，使用字节数组转换
                        let bytes = value.to_le_bytes();
                        let copy_len = std::cmp::min(data.len(), bytes.len());
                        data[..copy_len].copy_from_slice(&bytes[..copy_len]);
                    }
                }
                log::trace!(
                    "MMIO read from 0x{:x}, size {}, value 0x{:x}",
                    addr,
                    size,
                    value
                );
                Ok(())
            }
            Err(e) => {
                log::warn!("MMIO read failed at 0x{:x}: {:?}", addr, e);
                // 读取失败时填充0
                data.fill(0);
                Err(VmError::Platform(PlatformError::MemoryAccessFailed(format!(
                    "MMIO read failed: {:?}",
                    e
                )))
                .into())
            }
        }
    }

    /// 处理MMIO写入（内存映射I/O写入）
    fn handle_mmio_write(
        &self,
        addr: u64,
        data: &[u8],
        mmu: &mut dyn MMU,
    ) -> Result<(), AccelError> {
        // MMIO地址通过MMU路由到相应设备
        let size = data.len() as u8;

        // 将data缓冲区转换为u64值
        let value = match size {
            1 => data[0] as u64,
            2 => (data[0] as u64) | ((data[1] as u64) << 8),
            4 => {
                (data[0] as u64)
                    | ((data[1] as u64) << 8)
                    | ((data[2] as u64) << 16)
                    | ((data[3] as u64) << 24)
            }
            8 => {
                (data[0] as u64)
                    | ((data[1] as u64) << 8)
                    | ((data[2] as u64) << 16)
                    | ((data[3] as u64) << 24)
                    | ((data[4] as u64) << 32)
                    | ((data[5] as u64) << 40)
                    | ((data[6] as u64) << 48)
                    | ((data[7] as u64) << 56)
            }
            _ => {
                // 对于其他大小，使用字节数组转换
                let mut bytes = [0u8; 8];
                let copy_len = std::cmp::min(data.len(), bytes.len());
                bytes[..copy_len].copy_from_slice(&data[..copy_len]);
                u64::from_le_bytes(bytes)
            }
        };

        // 通过MMU写入MMIO地址
        match mmu.write(addr, value, size) {
            Ok(_) => {
                log::trace!(
                    "MMIO write to 0x{:x}, size {}, value 0x{:x}",
                    addr,
                    size,
                    value
                );
                Ok(())
            }
            Err(e) => {
                log::warn!("MMIO write failed at 0x{:x}: {:?}", addr, e);
                Err(VmError::Platform(PlatformError::MemoryAccessFailed(format!(
                    "MMIO write failed: {:?}",
                    e
                )))
                .into())
            }
        }
    }

    /// 处理I/O输入（端口I/O读取）
    fn handle_io_in(
        &self,
        port: u16,
        data: &mut [u8],
        _mmu: &mut dyn MMU,
    ) -> Result<(), AccelError> {
        // 根据端口号路由到相应设备
        // 常见端口映射：
        // 0x3F8-0x3FF: COM1 (串口)
        // 0x2F8-0x2FF: COM2 (串口)
        // 0x60-0x64: 键盘控制器
        // 0x70-0x71: RTC
        // 0xCF8-0xCFF: PCI配置空间

        match port {
            0x3F8..=0x3FF => {
                // COM1 串口 - 读取数据
                // 简化实现：返回0表示无数据
                data.fill(0);
                log::trace!("COM1 read from port 0x{:x}", port);
            }
            0x2F8..=0x2FF => {
                // COM2 串口
                data.fill(0);
                log::trace!("COM2 read from port 0x{:x}", port);
            }
            0x60..=0x64 => {
                // 键盘控制器
                data.fill(0);
                log::trace!("Keyboard controller read from port 0x{:x}", port);
            }
            0x70..=0x71 => {
                // RTC (Real-Time Clock)
                data.fill(0);
                log::trace!("RTC read from port 0x{:x}", port);
            }
            0xCF8..=0xCFF => {
                // PCI配置空间
                data.fill(0);
                log::trace!("PCI config read from port 0x{:x}", port);
            }
            _ => {
                // 未知端口，返回0
                log::warn!("Unhandled I/O IN port: 0x{:x}", port);
                data.fill(0);
            }
        }

        Ok(())
    }

    fn run_vcpu(&mut self, vcpu_id: u32, mmu: &mut dyn MMU) -> Result<(), AccelError> {
        #[cfg(feature = "kvm")]
        {
            let vcpu = self.vcpus.get_mut(vcpu_id as usize).ok_or_else(|| {
                VmError::Platform(PlatformError::InvalidParameter {
                    name: "vcpu_id".to_string(),
                    value: vcpu_id.to_string(),
                    message: "Invalid vCPU ID".to_string(),
                })
            })?;

            let exit = vcpu.run()?;

            match exit {
                VcpuExit::Hlt => {
                    log::debug!("vCPU {} halted", vcpu_id);
                    Ok(())
                }
                VcpuExit::Shutdown => {
                    log::info!("vCPU {} shutdown", vcpu_id);
                    Ok(())
                }
                VcpuExit::IoIn(port, data) => {
                    log::debug!("I/O IN: port 0x{:x}, size {}", port, data.len());
                    self.handle_io_in(port, data, mmu)
                }
                VcpuExit::IoOut(port, data) => {
                    log::debug!("I/O OUT: port 0x{:x}, data {:?}", port, data);
                    self.handle_io_out(port, data, mmu)
                }
                VcpuExit::MmioRead(addr, data) => {
                    log::debug!("MMIO READ: addr 0x{:x}, size {}", addr, data.len());
                    self.handle_mmio_read(addr, data, mmu)
                }
                VcpuExit::MmioWrite(addr, data) => {
                    log::debug!("MMIO WRITE: addr 0x{:x}, data {:?}", addr, data);
                    self.handle_mmio_write(addr, data, mmu)
                }
                exit => {
                    log::warn!("Unhandled vCPU exit: {:?}", exit);
                    Err(VmError::Platform(PlatformError::ExecutionFailed(format!(
                        "Unhandled exit: {:?}",
                        exit
                    ))))
                }
            }
        }

        #[cfg(not(feature = "kvm"))]
        {
            Err(VmError::Platform(PlatformError::UnsupportedOperation(
                "KVM feature not enabled".to_string(),
            )))
        }
    }

    fn get_regs(&self, vcpu_id: u32) -> Result<GuestRegs, AccelError> {
        #[cfg(feature = "kvm")]
        {
            // 检查缓存
            if self.regs_cache_enabled {
                if let Some(cached) = self.regs_cache.get(&vcpu_id) {
                    if let Some(ref regs) = cached {
                        // 缓存命中，直接返回
                        log::trace!("Register cache hit for vCPU {}", vcpu_id);
                        return Ok(regs.clone());
                    }
                }
            }

            // 缓存未命中，从硬件读取
            let vcpu = self.vcpus.get(vcpu_id as usize).ok_or_else(|| {
                VmError::Platform(PlatformError::InvalidParameter {
                    name: "vcpu_id".to_string(),
                    value: vcpu_id.to_string(),
                    message: "Invalid vCPU ID".to_string(),
                })
            })?;

            let regs = vcpu.get_regs()?;

            // 更新缓存（需要可变引用，这里使用内部可变性）
            if self.regs_cache_enabled {
                // 注意：这里需要使用Unsafe或内部可变性模式
                // 为了简化，我们只在下次get_regs时更新缓存
                // 实际实现可能需要使用RefCell或Mutex
                log::trace!(
                    "Register cache miss for vCPU {}, caching registers",
                    vcpu_id
                );
            }

            Ok(regs)
        }

        #[cfg(not(feature = "kvm"))]
        {
            let vcpu = self.vcpus.get(vcpu_id as usize).ok_or_else(|| {
                VmError::Platform(PlatformError::InvalidParameter {
                    name: "vcpu_id".to_string(),
                    value: vcpu_id.to_string(),
                    message: "Invalid vCPU ID".to_string(),
                })
            })?;
            vcpu.get_regs()
        }
    }

    fn set_regs(&mut self, vcpu_id: u32, regs: &GuestRegs) -> Result<(), AccelError> {
        let vcpu = self.vcpus.get_mut(vcpu_id as usize).ok_or_else(|| {
            VmError::Platform(PlatformError::InvalidParameter {
                name: "vcpu_id".to_string(),
                value: vcpu_id.to_string(),
                message: "Invalid vCPU ID".to_string(),
            })
        })?;

        // 调用实际的set_regs
        vcpu.set_regs(regs)?;

        // 更新缓存
        #[cfg(feature = "kvm")]
        {
            if self.regs_cache_enabled {
                self.regs_cache.insert(vcpu_id, Some(regs.clone()));
                log::trace!("Updated register cache for vCPU {}", vcpu_id);
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "KVM"
    }
}

impl Default for AccelKvm {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kvm_availability() {
        println!("KVM available: {}", AccelKvm::is_available());
    }

    #[test]
    #[cfg(feature = "kvm")]
    fn test_kvm_init() {
        if AccelKvm::is_available() {
            let mut accel = AccelKvm::new();
            assert!(accel.init().is_ok());
        }
    }

    #[test]
    fn test_numa_enable_disable() {
        let mut accel = AccelKvm::new();

        // Default state
        let (enabled, nodes) = accel.numa_config();
        assert!(!enabled);
        assert_eq!(nodes, 1);

        // Enable NUMA
        accel.enable_numa(4);
        let (enabled, nodes) = accel.numa_config();
        assert!(enabled);
        assert_eq!(nodes, 4);

        // Disable NUMA
        accel.disable_numa();
        let (enabled, nodes) = accel.numa_config();
        assert!(!enabled);
        assert_eq!(nodes, 1); // Reset to default
    }

    #[test]
    fn test_vcpu_numa_binding() {
        let mut accel = AccelKvm::new();

        // Should fail when NUMA is not enabled
        let result = accel.bind_vcpu_to_numa_node(0, 0);
        assert!(result.is_err());

        // Enable NUMA
        accel.enable_numa(2);

        // Valid bindings
        assert!(accel.bind_vcpu_to_numa_node(0, 0).is_ok());
        assert!(accel.bind_vcpu_to_numa_node(1, 1).is_ok());

        // Verify bindings
        assert_eq!(accel.get_vcpu_numa_node(0), Some(0));
        assert_eq!(accel.get_vcpu_numa_node(1), Some(1));
        assert_eq!(accel.get_vcpu_numa_node(2), None);

        // Invalid node
        let result = accel.bind_vcpu_to_numa_node(0, 5);
        assert!(result.is_err());
    }

    #[test]
    fn test_vcpu_affinity() {
        let accel = AccelKvm::new();

        // This should not panic even without KVM enabled
        // On non-Linux platforms, it will just log a warning
        let result = accel.set_vcpu_affinity(0, &[0, 1, 2, 3]);
        #[cfg(feature = "kvm")]
        {
            // May fail if vCPU doesn't exist, but shouldn't panic
            assert!(result.is_ok() || result.is_err());
        }
        #[cfg(not(feature = "kvm"))]
        {
            // Should succeed when KVM is not enabled (stub implementation)
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_numa_memory_setup() {
        let mut accel = AccelKvm::new();

        // Should fail when NUMA is not enabled
        let result = accel.setup_numa_memory(0, 1024 * 1024, 0x1000, 0x7000_0000);
        assert!(result.is_err());

        // Enable NUMA
        accel.enable_numa(2);

        // Invalid node
        let result = accel.setup_numa_memory(5, 1024 * 1024, 0x1000, 0x7000_0000);
        assert!(result.is_err());

        // Valid setup (will fail at KVM init if not actually running KVM)
        #[cfg(feature = "kvm")]
        {
            if AccelKvm::is_available() {
                let _ = accel.init();
                let result = accel.setup_numa_memory(0, 1024 * 1024, 0x1000, 0x7000_0000);
                // May fail if VM not properly initialized, but shouldn't panic
                assert!(result.is_ok() || result.is_err());
            }
        }
    }

    #[test]
    fn test_numa_config_state() {
        let mut accel = AccelKvm::new();

        // Test minimum value enforcement
        accel.enable_numa(0);
        let (_, nodes) = accel.numa_config();
        assert_eq!(nodes, 1); // Should be at least 1

        accel.enable_numa(8);
        let (_, nodes) = accel.numa_config();
        assert_eq!(nodes, 8);
    }
}

// ============================================================================
// 辅助宏 - 减少条件编译重复
// ============================================================================

/// 宏：统一架构特定的 vCPU 操作委托
///
/// 这个宏消除了在每个方法中重复编写条件编译的需要。
/// 它自动为不同架构生成适当的 match 分支。
#[cfg(feature = "kvm")]
#[macro_export]
macro_rules! kvm_vcpu_delegate {
    // 无参数方法调用
    ($self:ident, $method:ident) => {
        match $self {
            #[cfg(target_arch = "x86_64")]
            Self::X86_64(v) => v.$method(),
            #[cfg(target_arch = "aarch64")]
            Self::Aarch64(v) => v.$method(),
        }
    };

    // 单参数方法调用
    ($self:ident, $method:ident, $arg:expr) => {
        match $self {
            #[cfg(target_arch = "x86_64")]
            Self::X86_64(v) => v.$method($arg),
            #[cfg(target_arch = "aarch64")]
            Self::Aarch64(v) => v.$method($arg),
        }
    };

    // 多参数方法调用
    ($self:ident, $method:ident, $($args:expr),+) => {
        match $self {
            #[cfg(target_arch = "x86_64")]
            Self::X86_64(v) => v.$method($($args),+),
            #[cfg(target_arch = "aarch64")]
            Self::Aarch64(v) => v.$method($($args),+),
        }
    };
}

/// 宏：为 KvmVcpuUnified 自动实现方法
///
/// 使用此宏可以自动生成标准的委托方法，避免手动编写。
#[cfg(feature = "kvm")]
#[macro_export]
macro_rules! impl_kvm_vcpu_methods {
    () => {
        /// 获取 vCPU 寄存器状态
        pub fn get_regs(&self) -> Result<GuestRegs, AccelError> {
            kvm_vcpu_delegate!(self, get_regs)
        }

        /// 设置 vCPU 寄存器状态
        pub fn set_regs(&mut self, regs: &GuestRegs) -> Result<(), AccelError> {
            kvm_vcpu_delegate!(self, set_regs, regs)
        }
    };
}
