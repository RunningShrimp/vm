//! OS级引导支持
//!
//! 提供操作系统引导所需的核心功能：
//! - 页表初始化
//! - 特权态设置
//! - 异常向量表设置
//! - 基本内存布局

use vm_core::{CoreError, GuestAddr, GuestPhysAddr, MMU, VmError};

/// OS引导上下文
#[derive(Debug, Clone)]
pub struct OsBootContext {
    /// 架构类型
    pub arch: Architecture,
    /// 页表根地址
    pub page_table_root: Option<GuestPhysAddr>,
    /// 异常向量表地址
    pub exception_vector_base: Option<GuestAddr>,
    /// 栈顶地址
    pub stack_top: Option<GuestAddr>,
    /// 堆起始地址
    pub heap_start: Option<GuestAddr>,
    /// 内核参数传递地址
    pub kernel_params_addr: Option<GuestAddr>,
}

/// 支持的架构
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    X86_64,
    Aarch64,
    Riscv64,
}

/// 页表配置
#[derive(Debug, Clone)]
pub struct PageTableConfig {
    /// 页大小
    pub page_size: u64,
    /// 页表级别数
    pub levels: u32,
    /// 虚拟地址位数
    pub va_bits: u32,
}

/// 特权态配置
#[derive(Debug, Clone)]
pub struct PrivilegeConfig {
    /// 初始特权级别
    pub initial_privilege: PrivilegeLevel,
    /// 是否启用分页
    pub paging_enabled: bool,
    /// 是否启用中断
    pub interrupts_enabled: bool,
}

/// 特权级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegeLevel {
    User,
    Supervisor,
    Machine,
}

/// OS引导管理器
pub struct OsBootManager {
    context: OsBootContext,
    page_table_config: PageTableConfig,
    /// 特权态配置（用于设置分页、中断等标志）
    #[allow(dead_code)] // 配置信息，用于未来扩展特权态设置
    privilege_config: PrivilegeConfig,
}

impl OsBootManager {
    /// 创建新的OS引导管理器
    pub fn new(arch: Architecture) -> Self {
        let (page_table_config, privilege_config) = match arch {
            Architecture::X86_64 => (
                PageTableConfig {
                    page_size: 4096,
                    levels: 4,
                    va_bits: 48,
                },
                PrivilegeConfig {
                    initial_privilege: PrivilegeLevel::Supervisor,
                    paging_enabled: true,
                    interrupts_enabled: false,
                },
            ),
            Architecture::Aarch64 => (
                PageTableConfig {
                    page_size: 4096,
                    levels: 4,
                    va_bits: 48,
                },
                PrivilegeConfig {
                    initial_privilege: PrivilegeLevel::Supervisor,
                    paging_enabled: true,
                    interrupts_enabled: false,
                },
            ),
            Architecture::Riscv64 => (
                PageTableConfig {
                    page_size: 4096,
                    levels: 3,
                    va_bits: 39,
                },
                PrivilegeConfig {
                    initial_privilege: PrivilegeLevel::Machine,
                    paging_enabled: false,
                    interrupts_enabled: false,
                },
            ),
        };

        Self {
            context: OsBootContext {
                arch,
                page_table_root: None,
                exception_vector_base: None,
                stack_top: None,
                heap_start: None,
                kernel_params_addr: None,
            },
            page_table_config,
            privilege_config,
        }
    }

    /// 设置页表根地址
    pub fn set_page_table_root(&mut self, addr: GuestPhysAddr) {
        self.context.page_table_root = Some(addr);
    }

    /// 设置异常向量表基地址
    pub fn set_exception_vector_base(&mut self, addr: GuestAddr) {
        self.context.exception_vector_base = Some(addr);
    }

    /// 设置栈顶地址
    pub fn set_stack_top(&mut self, addr: GuestAddr) {
        self.context.stack_top = Some(addr);
    }

    /// 设置堆起始地址
    pub fn set_heap_start(&mut self, addr: GuestAddr) {
        self.context.heap_start = Some(addr);
    }

    /// 设置内核参数地址
    pub fn set_kernel_params_addr(&mut self, addr: GuestAddr) {
        self.context.kernel_params_addr = Some(addr);
    }

    /// 初始化页表
    pub fn initialize_page_tables(&self, memory: &mut dyn MMU) -> Result<GuestPhysAddr, VmError> {
        match self.context.arch {
            Architecture::X86_64 => self.initialize_x86_64_page_tables(memory),
            Architecture::Aarch64 => self.initialize_aarch64_page_tables(memory),
            Architecture::Riscv64 => self.initialize_riscv64_page_tables(memory),
        }
    }

    /// 初始化x86_64页表
    fn initialize_x86_64_page_tables(
        &self,
        memory: &mut dyn MMU,
    ) -> Result<GuestPhysAddr, VmError> {
        let page_size = self.page_table_config.page_size;

        // 为页表分配物理内存（这里简化为直接分配连续内存）
        let pml4_addr = self.allocate_page_table_memory(memory, page_size * 10)?;

        // 初始化PML4表（第4级）
        self.initialize_page_table_level(memory, pml4_addr, 512)?;

        // 创建基本映射：身份映射前1GB内存
        self.create_identity_mapping(memory, pml4_addr, 0, 1024 * 1024 * 1024, page_size)?;

        Ok(pml4_addr)
    }

    /// 初始化AArch64页表
    fn initialize_aarch64_page_tables(
        &self,
        memory: &mut dyn MMU,
    ) -> Result<GuestPhysAddr, VmError> {
        let page_size = self.page_table_config.page_size;

        // 为页表分配物理内存
        let ttbr0_addr = self.allocate_page_table_memory(memory, page_size * 10)?;

        // 初始化页表
        self.initialize_page_table_level(memory, ttbr0_addr, 512)?;

        // 创建基本映射
        self.create_identity_mapping(memory, ttbr0_addr, 0, 1024 * 1024 * 1024, page_size)?;

        Ok(ttbr0_addr)
    }

    /// 初始化RISC-V64页表
    fn initialize_riscv64_page_tables(
        &self,
        memory: &mut dyn MMU,
    ) -> Result<GuestPhysAddr, VmError> {
        let page_size = self.page_table_config.page_size;

        // 为页表分配物理内存
        let satp_addr = self.allocate_page_table_memory(memory, page_size * 5)?;

        // 初始化页表
        self.initialize_page_table_level(memory, satp_addr, 512)?;

        // RISC-V默认不启用分页，但可以设置基础映射
        self.create_identity_mapping(memory, satp_addr, 0, 128 * 1024 * 1024, page_size)?;

        Ok(satp_addr)
    }

    /// 分配页表内存
    fn allocate_page_table_memory(
        &self,
        memory: &mut dyn MMU,
        size: u64,
    ) -> Result<GuestPhysAddr, VmError> {
        // 在低地址分配页表内存
        let base_addr = GuestPhysAddr(0x100000); // 1MB处开始分配

        // 清零内存
        for i in 0..size {
            memory.write(GuestAddr(base_addr.0 + i), 0, 1)?;
        }

        Ok(base_addr)
    }

    /// 初始化页表级别
    fn initialize_page_table_level(
        &self,
        memory: &mut dyn MMU,
        table_addr: GuestPhysAddr,
        entries: u64,
    ) -> Result<(), VmError> {
        // 清零整个页表
        for i in 0..entries {
            let entry_addr = table_addr.0 + i * 8; // 每个条目8字节
            memory.write(GuestAddr(entry_addr), 0, 8)?;
        }
        Ok(())
    }

    /// 创建身份映射
    fn create_identity_mapping(
        &self,
        memory: &mut dyn MMU,
        root_table: GuestPhysAddr,
        va_start: u64,
        va_end: u64,
        page_size: u64,
    ) -> Result<(), VmError> {
        let mut va = va_start;
        while va < va_end {
            self.map_page(memory, root_table, va, GuestPhysAddr(va), page_size)?;
            va += page_size;
        }
        Ok(())
    }

    /// 映射单个页面
    fn map_page(
        &self,
        memory: &mut dyn MMU,
        root_table: GuestPhysAddr,
        va: u64,
        pa: GuestPhysAddr,
        page_size: u64,
    ) -> Result<(), VmError> {
        // 验证页面大小参数，确保其为标准4KB或更大粒度
        if page_size < 4096 || (page_size & (page_size - 1)) != 0 {
            return Err(VmError::Core(CoreError::InvalidConfig {
                message: format!(
                    "page_size must be power of 2 and at least 4KB, got {}",
                    page_size
                ),
                field: "page_size".to_string(),
            }));
        }

        match self.context.arch {
            Architecture::X86_64 => {
                // x86_64 4级页表映射逻辑
                let pml4_index = (va >> 39) & 0x1FF;
                let pdp_index = (va >> 30) & 0x1FF;
                let pd_index = (va >> 21) & 0x1FF;
                let pt_index = (va >> 12) & 0x1FF;

                // 遍历PML4
                let pml4_entry_addr = GuestAddr(root_table.0 + pml4_index * 8);
                let pml4_entry = memory.read(pml4_entry_addr, 8).unwrap_or(0) | 0x3; // Present + Writable
                let pdp_table = GuestPhysAddr(pml4_entry & !0xFFF);

                // 遍历PDP
                let pdp_entry_addr = GuestAddr(pdp_table.0 + pdp_index * 8);
                let pdp_entry = memory.read(pdp_entry_addr, 8).unwrap_or(0) | 0x3;
                let pd_table = GuestPhysAddr(pdp_entry & !0xFFF);

                // 遍历PD
                let pd_entry_addr = GuestAddr(pd_table.0 + pd_index * 8);
                let pd_entry = memory.read(pd_entry_addr, 8).unwrap_or(0) | 0x3;
                let pt_table = GuestPhysAddr(pd_entry & !0xFFF);

                // 设置PT条目
                let pt_entry_addr = GuestAddr(pt_table.0 + pt_index * 8);
                let entry_value = pa.0 | 0x3; // Present + Writable
                memory.write(pt_entry_addr, entry_value, 8)?;

                // 回写上级表条目
                memory.write(pml4_entry_addr, pml4_entry, 8)?;
                memory.write(pdp_entry_addr, pdp_entry, 8)?;
                memory.write(pd_entry_addr, pd_entry, 8)?;
            }
            Architecture::Aarch64 => {
                // AArch64 页表映射逻辑
                // AArch64 使用4KB粒度的分层页表
                let l3_index = (va >> 12) & 0x1FF;
                let l2_index = (va >> 21) & 0x1FF;
                let l1_index = (va >> 30) & 0x1FF;
                let l0_index = (va >> 39) & 0x1FF;

                // 简化实现：直接在L3表中设置条目
                let entry_value = pa.0 | 0x3; // Present + Page
                let entry_addr = GuestAddr(
                    root_table.0
                        + ((l0_index * 512 + l1_index * 512 + l2_index * 512 + l3_index) * 8),
                );
                memory.write(entry_addr, entry_value, 8)?;
            }
            Architecture::Riscv64 => {
                // RISC-V Sv39 页表映射逻辑
                let vpn2 = (va >> 30) & 0x1FF;
                let vpn1 = (va >> 21) & 0x1FF;
                let vpn0 = (va >> 12) & 0x1FF;

                // 页表项值 = PPN | FLAGS
                // PPN = (pa >> 12), FLAGS = 0x1F for VRWXU + Valid
                let entry_value = (pa.0 >> 2) | 0x1F;

                // 遍历3级页表
                let l2_table = root_table;
                let l2_entry_addr = GuestAddr(l2_table.0 + vpn2 * 8);
                let l2_entry = memory.read(l2_entry_addr, 8).unwrap_or(0) | 0x1;
                let l1_table = GuestPhysAddr((l2_entry >> 10) << 12);

                let l1_entry_addr = GuestAddr(l1_table.0 + vpn1 * 8);
                let l1_entry = memory.read(l1_entry_addr, 8).unwrap_or(0) | 0x1;
                let l0_table = GuestPhysAddr((l1_entry >> 10) << 12);

                // 在L0 (叶表) 中设置条目
                let l0_entry_addr = GuestAddr(l0_table.0 + vpn0 * 8);
                memory.write(l0_entry_addr, entry_value, 8)?;

                // 回写上级表条目
                memory.write(l2_entry_addr, l2_entry, 8)?;
                memory.write(l1_entry_addr, l1_entry, 8)?;
            }
        }

        Ok(())
    }

    /// 设置特权态
    pub fn setup_privilege_state(
        &self,
        context: &mut vm_core::VcpuStateContainer,
    ) -> Result<(), VmError> {
        match self.context.arch {
            Architecture::X86_64 => self.setup_x86_64_privilege(context),
            Architecture::Aarch64 => self.setup_aarch64_privilege(context),
            Architecture::Riscv64 => self.setup_riscv64_privilege(context),
        }
    }

    /// 设置x86_64特权态
    fn setup_x86_64_privilege(
        &self,
        context: &mut vm_core::VcpuStateContainer,
    ) -> Result<(), VmError> {
        // 设置栈指针（RSP）
        if let Some(stack_top) = self.context.stack_top {
            context.state.regs.sp = stack_top.0;
            // x86_64 使用 RSP 作为栈指针，对应 gpr[4]
            context.state.regs.gpr[4] = stack_top.0;
        }

        // 设置帧指针（RBP）
        if let Some(stack_top) = self.context.stack_top {
            context.state.regs.fp = stack_top.0;
            // x86_64 使用 RBP 作为帧指针，对应 gpr[5]
            context.state.regs.gpr[5] = stack_top.0;
        }

        // 注意：CR3、CR0 等控制寄存器在当前架构中不直接暴露在 GuestRegs 中
        // 这些寄存器由 MMU 或硬件加速层管理
        // 页表根地址已通过 context.page_table_root 传递，由 MMU 使用

        tracing::debug!(
            "x86_64 privilege state setup: SP={:#x}, FP={:#x}",
            context.state.regs.sp,
            context.state.regs.fp
        );

        Ok(())
    }

    /// 设置AArch64特权态
    fn setup_aarch64_privilege(
        &self,
        context: &mut vm_core::VcpuStateContainer,
    ) -> Result<(), VmError> {
        // 设置栈指针（SP_EL1）
        if let Some(stack_top) = self.context.stack_top {
            context.state.regs.sp = stack_top.0;
            // AArch64 使用 X31 (SP) 作为栈指针，对应 gpr[31]
            context.state.regs.gpr[31] = stack_top.0;
        }

        // 注意：TTBR0_EL1、SCTLR_EL1 等系统寄存器在当前架构中不直接暴露在 GuestRegs 中
        // 这些寄存器由 MMU 或硬件加速层管理
        // 页表根地址已通过 context.page_table_root 传递，由 MMU 使用

        tracing::debug!(
            "AArch64 privilege state setup: SP={:#x}",
            context.state.regs.sp
        );

        Ok(())
    }

    /// 设置RISC-V64特权态
    fn setup_riscv64_privilege(
        &self,
        context: &mut vm_core::VcpuStateContainer,
    ) -> Result<(), VmError> {
        // 设置栈指针（SP/x2）
        if let Some(stack_top) = self.context.stack_top {
            context.state.regs.sp = stack_top.0;
            // RISC-V 使用 x2 作为栈指针，对应 gpr[2]
            context.state.regs.gpr[2] = stack_top.0;
        }

        // 注意：satp、mstatus 等控制寄存器在当前架构中不直接暴露在 GuestRegs 中
        // 这些寄存器由 MMU 或硬件加速层管理
        // 页表根地址已通过 context.page_table_root 传递，由 MMU 使用

        tracing::debug!(
            "RISC-V64 privilege state setup: SP={:#x}",
            context.state.regs.sp
        );

        Ok(())
    }

    /// 设置异常向量表
    pub fn setup_exception_vectors(&self, memory: &mut dyn MMU) -> Result<(), VmError> {
        let vector_base = self.context.exception_vector_base.unwrap_or(GuestAddr(0));

        match self.context.arch {
            Architecture::X86_64 => self.setup_x86_64_exception_vectors(memory, vector_base),
            Architecture::Aarch64 => self.setup_aarch64_exception_vectors(memory, vector_base),
            Architecture::Riscv64 => self.setup_riscv64_exception_vectors(memory, vector_base),
        }
    }

    /// 设置x86_64异常向量表
    fn setup_x86_64_exception_vectors(
        &self,
        memory: &mut dyn MMU,
        base: GuestAddr,
    ) -> Result<(), VmError> {
        // 设置基本的异常处理程序地址
        // 这里简化实现，实际应该设置具体的处理函数地址

        let handlers = [
            base.0,        // Divide by zero
            base.0 + 0x10, // Debug
            base.0 + 0x20, // NMI
            base.0 + 0x30, // Breakpoint
            base.0 + 0x40, // Overflow
                           // ... 其他异常
        ];

        for (i, &handler_addr) in handlers.iter().enumerate() {
            let vector_addr = base.0 + (i as u64 * 8);
            memory.write(GuestAddr(vector_addr), handler_addr, 8)?;
        }

        Ok(())
    }

    /// 设置AArch64异常向量表
    fn setup_aarch64_exception_vectors(
        &self,
        memory: &mut dyn MMU,
        base: GuestAddr,
    ) -> Result<(), VmError> {
        // AArch64异常向量表有特定的格式
        // 这里简化实现

        let vector_size = 0x80; // 每个向量128字节
        for i in 0..16 {
            let vector_addr = base.0 + (i as u64 * vector_size);
            // 写入基本的异常处理代码或跳转指令
            memory.write(GuestAddr(vector_addr), 0xD4000001, 4)?; // SVC #1 或其他指令
        }

        Ok(())
    }

    /// 设置RISC-V异常向量表
    fn setup_riscv64_exception_vectors(
        &self,
        memory: &mut dyn MMU,
        base: GuestAddr,
    ) -> Result<(), VmError> {
        // RISC-V使用mtvec寄存器指向向量表
        // 这里设置基本的跳转指令

        for i in 0..32 {
            let vector_addr = base.0 + (i as u64 * 4);
            // 写入基本的异常处理代码
            memory.write(GuestAddr(vector_addr), 0x00000073, 4)?; // ECALL 或其他指令
        }

        Ok(())
    }

    /// 获取引导上下文
    pub fn context(&self) -> &OsBootContext {
        &self.context
    }

    /// 加载内核镜像到内存
    ///
    /// 支持简单的二进制镜像加载（未来可扩展支持ELF/PE格式）
    pub fn load_kernel_image(
        &self,
        memory: &mut dyn MMU,
        image_data: &[u8],
        load_addr: GuestAddr,
    ) -> Result<GuestAddr, VmError> {
        tracing::info!(
            "Loading kernel image: {} bytes at {:#x}",
            image_data.len(),
            load_addr.0
        );

        // 将镜像数据写入内存
        for (offset, &byte) in image_data.iter().enumerate() {
            let addr = GuestAddr(load_addr.0 + offset as u64);
            memory.write(addr, byte as u64, 1)?;
        }

        tracing::info!("Kernel image loaded successfully");
        Ok(load_addr)
    }

    /// 执行完整的OS引导流程
    pub fn perform_os_boot(
        &mut self,
        memory: &mut dyn MMU,
        kernel_entry: GuestAddr,
    ) -> Result<OsBootResult, VmError> {
        tracing::info!(
            "Starting OS boot process for {:?} architecture, kernel entry: {:#x}",
            self.context.arch,
            kernel_entry.0
        );

        // 1. 初始化页表
        let page_table_root = self.initialize_page_tables(memory)?;
        self.context.page_table_root = Some(page_table_root);
        tracing::debug!("Page tables initialized at {:#x}", page_table_root.0);

        // 2. 设置异常向量表
        let vector_base = GuestAddr(0x1000); // 4KB处
        self.context.exception_vector_base = Some(vector_base);
        self.setup_exception_vectors(memory)?;
        tracing::debug!("Exception vectors setup at {:#x}", vector_base.0);

        // 3. 设置栈和堆
        let stack_top = GuestAddr(0x800000); // 8MB处
        let heap_start = GuestAddr(0x1000000); // 16MB处
        self.context.stack_top = Some(stack_top);
        self.context.heap_start = Some(heap_start);
        tracing::debug!(
            "Stack top: {:#x}, Heap start: {:#x}",
            stack_top.0,
            heap_start.0
        );

        // 4. 设置内核参数地址
        let params_addr = GuestAddr(0x2000); // 8KB处
        self.context.kernel_params_addr = Some(params_addr);
        tracing::debug!("Kernel params address: {:#x}", params_addr.0);

        // 5. 创建引导结果
        let result = OsBootResult {
            page_table_root,
            exception_vector_base: vector_base,
            stack_top,
            heap_start,
            kernel_entry,
            kernel_params_addr: params_addr,
        };

        tracing::info!("OS boot process completed successfully");
        Ok(result)
    }

    /// 完成引导并设置VCPU状态
    ///
    /// 在引导完成后，设置VCPU的初始状态，包括寄存器、程序计数器等
    pub fn finalize_boot(
        &self,
        context: &mut vm_core::VcpuStateContainer,
        kernel_entry: GuestAddr,
    ) -> Result<(), VmError> {
        // 1. 设置特权态（包括栈指针等）
        self.setup_privilege_state(context)?;

        // 2. 设置程序计数器到内核入口点
        context.state.pc = kernel_entry;
        context.state.regs.pc = kernel_entry.0;

        tracing::info!(
            "VCPU state finalized: PC={:#x}, SP={:#x}",
            kernel_entry.0,
            context.state.regs.sp
        );

        Ok(())
    }
}

/// OS引导结果
#[derive(Debug, Clone)]
pub struct OsBootResult {
    /// 页表根地址
    pub page_table_root: GuestPhysAddr,
    /// 异常向量表基地址
    pub exception_vector_base: GuestAddr,
    /// 栈顶地址
    pub stack_top: GuestAddr,
    /// 堆起始地址
    pub heap_start: GuestAddr,
    /// 内核入口点
    pub kernel_entry: GuestAddr,
    /// 内核参数地址
    pub kernel_params_addr: GuestAddr,
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_mem::SoftMmu;

    #[test]
    fn test_os_boot_manager_creation() {
        let manager = OsBootManager::new(Architecture::X86_64);
        assert_eq!(manager.context.arch, Architecture::X86_64);
        assert!(manager.context.page_table_root.is_none());
    }

    #[test]
    fn test_page_table_initialization() {
        let manager = OsBootManager::new(Architecture::X86_64);
        let mut memory = SoftMmu::new(64 * 1024 * 1024, false);

        let result = manager.initialize_page_tables(&mut memory);
        assert!(result.is_ok());

        let pt_root = result.unwrap();
        assert!(pt_root.0 > 0);
    }

    #[test]
    fn test_os_boot_process() {
        let mut manager = OsBootManager::new(Architecture::Riscv64);
        let mut memory = SoftMmu::new(64 * 1024 * 1024, false);
        let kernel_entry = GuestAddr(0x80000000);

        let result = manager.perform_os_boot(&mut memory, kernel_entry);
        assert!(result.is_ok());

        let boot_result = result.unwrap();
        assert_eq!(boot_result.kernel_entry, kernel_entry);
        assert!(boot_result.page_table_root.0 > 0);
    }
}
