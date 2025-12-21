//! 最小设备集集成示例
//!
//! 演示如何将最小设备集集成到OS引导流程中

use crate::minimal_device_set::{MinimalDeviceSetConfig, MinimalDeviceSetManager};
use vm_core::{MMU, VmError};

// 注意：vm-boot 不是 vm-device 的依赖，所以这里使用简化的接口
// 在实际使用中，应该在上层模块中集成引导和设备初始化

/// 初始化最小设备集并注册到MMU
///
/// 这个函数演示了如何：
/// 1. 初始化最小设备集
/// 2. 注册设备到MMU
/// 3. 返回设备管理器供后续使用
pub async fn initialize_devices(memory: &mut dyn MMU) -> Result<MinimalDeviceSetManager, VmError> {
    // 1. 创建并初始化最小设备集
    let device_config = MinimalDeviceSetConfig::default();
    let mut device_manager = MinimalDeviceSetManager::new(device_config);
    device_manager.initialize().await?;

    // 2. 注册设备到MMU
    device_manager.register_to_mmu(memory)?;

    log::info!("最小设备集初始化完成");

    Ok(device_manager)
}

/// 设备中断处理示例
///
/// 演示如何在执行循环中处理设备中断
pub fn handle_device_interrupts(
    device_manager: &MinimalDeviceSetManager,
    context: usize,
) -> Result<(), VmError> {
    // 更新定时器（检查是否有定时器中断）
    device_manager.update_timer();

    // 通过PLIC处理中断
    if let Ok(Some(irq)) = device_manager.handle_interrupt_via_plic(context) {
        log::debug!("处理了中断: IRQ {}", irq);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initialize_devices() {
        use vm_mem::SoftMmu;

        // 创建内存管理器
        let mut memory = SoftMmu::new(64 * 1024 * 1024, false);

        // 测试设备初始化
        let result = initialize_devices(&mut memory).await;

        assert!(result.is_ok());
        let device_manager = result.unwrap();

        // 验证设备已创建
        assert!(device_manager.console().is_some());
        assert!(device_manager.timer().is_some());
        assert!(device_manager.plic().is_some());
    }
}
