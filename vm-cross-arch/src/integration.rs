//! 集成模块
//!
//! 提供与VM服务集成的便捷接口

use super::{AutoExecutor, CrossArchConfig, HostArch, VmConfigExt, create_auto_vm_config};
use vm_core::{ExecMode, GuestArch, MMU, VmConfig, VmError};
use vm_ir::IRBlock;
use vm_mem::SoftMmu;

/// 跨架构VM构建器
///
/// 提供便捷的API来创建和配置跨架构VM
pub struct CrossArchVmBuilder {
    guest_arch: GuestArch,
    memory_size: Option<usize>,
    exec_mode: Option<ExecMode>,
}

impl CrossArchVmBuilder {
    /// 创建新的构建器
    pub fn new(guest_arch: GuestArch) -> Self {
        Self {
            guest_arch,
            memory_size: None,
            exec_mode: None,
        }
    }

    /// 设置内存大小
    pub fn memory_size(mut self, size: usize) -> Self {
        self.memory_size = Some(size);
        self
    }

    /// 设置执行模式
    pub fn exec_mode(mut self, mode: ExecMode) -> Self {
        self.exec_mode = Some(mode);
        self
    }

    /// 构建VM配置和执行器
    pub fn build(self) -> Result<CrossArchVm, VmError> {
        // 创建VM配置
        let (mut vm_config, cross_config) =
            super::create_auto_vm_config(self.guest_arch, self.memory_size)?;

        // 应用执行模式（如果指定）
        if let Some(mode) = self.exec_mode {
            vm_config.exec_mode = mode;
        }

        // 创建执行器
        let executor = AutoExecutor::auto_create(self.guest_arch, Some(vm_config.exec_mode))?;

        // 创建MMU
        let mmu = SoftMmu::new(vm_config.memory_size, false);

        Ok(CrossArchVm {
            config: vm_config,
            cross_config,
            executor,
            mmu,
        })
    }
}

/// 跨架构VM实例
pub struct CrossArchVm {
    config: VmConfig,
    cross_config: CrossArchConfig,
    executor: AutoExecutor,
    mmu: SoftMmu,
}

impl CrossArchVm {
    /// 获取VM配置
    pub fn config(&self) -> &VmConfig {
        &self.config
    }

    /// 获取跨架构配置
    pub fn cross_config(&self) -> &CrossArchConfig {
        &self.cross_config
    }

    /// 获取MMU（可变引用）
    pub fn mmu_mut(&mut self) -> &mut SoftMmu {
        &mut self.mmu
    }

    /// 执行代码
    pub fn execute(&mut self, pc: u64) -> Result<vm_core::ExecResult, VmError> {
        self.executor.execute_block(&mut self.mmu, pc)
    }

    /// 加载代码到内存
    pub fn load_code(&mut self, addr: u64, code: &[u8]) -> Result<(), VmError> {
        for (i, byte) in code.iter().enumerate() {
            self.mmu.write(addr + i as u64, *byte as u64, 1)?;
        }
        Ok(())
    }

    /// 获取执行引擎（用于访问寄存器等）
    pub fn engine_mut(&mut self) -> &mut dyn vm_core::ExecutionEngine<IRBlock> {
        self.executor.engine_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_arch_vm_builder() {
        let vm = CrossArchVmBuilder::new(GuestArch::X86_64)
            .memory_size(128 * 1024 * 1024)
            .build();

        assert!(vm.is_ok());
        let vm = vm.unwrap();
        assert_eq!(vm.config().guest_arch, GuestArch::X86_64);
        assert!(vm.cross_config().is_supported());
    }
}
