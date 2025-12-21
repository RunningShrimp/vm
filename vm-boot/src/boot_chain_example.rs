//! OS引导链完整示例
//!
//! 演示如何使用 OsBootManager 完成完整的OS引导流程

use crate::{Architecture, OsBootManager, OsBootResult};
use vm_core::{GuestAddr, VcpuStateContainer, VmError};
use vm_mem::SoftMmu;

/// 执行完整的OS引导链
///
/// 这个函数演示了完整的引导流程：
/// 1. 创建引导管理器
/// 2. 加载内核镜像
/// 3. 初始化页表
/// 4. 设置异常向量表
/// 5. 设置特权态
/// 6. 跳转到内核入口点
pub fn boot_os_example(
    arch: Architecture,
    kernel_image: &[u8],
    kernel_entry: GuestAddr,
    memory_size: usize,
) -> Result<(OsBootResult, VcpuStateContainer), VmError> {
    // 1. 创建引导管理器
    let mut boot_manager = OsBootManager::new(arch);

    // 2. 创建内存管理器
    let mut memory = SoftMmu::new(memory_size, false);

    // 3. 加载内核镜像到内存
    // 根据架构选择加载地址
    let load_addr = match arch {
        Architecture::X86_64 => GuestAddr(0x100000), // 1MB (x86_64 传统加载地址)
        Architecture::Aarch64 => GuestAddr(0x80000), // 512KB (AArch64 典型加载地址)
        Architecture::Riscv64 => GuestAddr(0x80000000), // 2GB (RISC-V 典型加载地址)
    };

    boot_manager.load_kernel_image(&mut memory, kernel_image, load_addr)?;

    // 4. 执行引导流程
    let boot_result = boot_manager.perform_os_boot(&mut memory, kernel_entry)?;

    // 5. 创建VCPU状态并设置初始值
    let mut vcpu_state = VcpuStateContainer {
        vcpu_id: 0,
        state: vm_core::VmState::default(),
        running: false,
    };

    // 6. 完成引导并设置VCPU状态
    boot_manager.finalize_boot(&mut vcpu_state, kernel_entry)?;

    Ok((boot_result, vcpu_state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_os_example() {
        // 创建一个简单的测试内核镜像（只是一个占位）
        let kernel_image = vec![0x90; 1024]; // NOP 指令填充

        // 测试 x86_64 引导
        let result = boot_os_example(
            Architecture::X86_64,
            &kernel_image,
            GuestAddr(0x100000),
            64 * 1024 * 1024, // 64MB
        );

        assert!(result.is_ok());
        let (boot_result, vcpu_state) = result.unwrap();
        assert_eq!(boot_result.kernel_entry, GuestAddr(0x100000));
        assert_eq!(vcpu_state.state.pc, GuestAddr(0x100000));
        assert!(vcpu_state.state.regs.sp > 0);
    }
}
