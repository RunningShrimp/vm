//! 最小设备集管理器 - Phase 2
//!
//! 提供 OS 引导和运行所需的最小设备集：
//! - console: VirtIO Console (virtio_console)
//! - timer: CLINT (Core Local Interruptor) for RISC-V
//! - block: VirtIO Block (virtio_block)
//! - net: VirtIO Network (vhost_net)
//!
//! 同时提供 IRQ 分发机制

use crate::block_service::BlockDeviceService;
use crate::clint::Clint;
use crate::plic::Plic;
use crate::vhost_net::VhostNet;
use crate::virtio::VirtioDevice;
use crate::virtio_console::VirtioConsole;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use vm_core::{GuestAddr, MMU, VmError};

/// 设备配置
#[derive(Debug, Clone)]
pub struct DeviceConfig {
    /// 设备基地址
    pub base_addr: GuestAddr,
    /// 设备大小
    pub size: u64,
    /// IRQ 号
    pub irq: Option<u32>,
}

/// 最小设备集配置
#[derive(Debug, Clone)]
pub struct MinimalDeviceSetConfig {
    /// Console 设备配置
    pub console: Option<DeviceConfig>,
    /// Timer (CLINT) 配置
    pub timer: Option<DeviceConfig>,
    /// Block 设备配置
    pub block: Option<DeviceConfig>,
    /// Network 设备配置
    pub net: Option<DeviceConfig>,
    /// PLIC (Platform Level Interrupt Controller) 配置
    pub plic: Option<DeviceConfig>,
    /// Hart 数量 (用于 CLINT)
    pub num_harts: usize,
    /// 时钟频率 (用于 CLINT)
    pub clock_freq: u64,
}

impl Default for MinimalDeviceSetConfig {
    fn default() -> Self {
        Self {
            console: Some(DeviceConfig {
                base_addr: GuestAddr(0x10001000),
                size: 0x1000,
                irq: Some(1),
            }),
            timer: Some(DeviceConfig {
                base_addr: GuestAddr(0x2000000),
                size: 0x10000,
                irq: Some(7), // Timer interrupt
            }),
            block: Some(DeviceConfig {
                base_addr: GuestAddr(0x10002000),
                size: 0x1000,
                irq: Some(2),
            }),
            net: Some(DeviceConfig {
                base_addr: GuestAddr(0x10003000),
                size: 0x1000,
                irq: Some(3),
            }),
            plic: Some(DeviceConfig {
                base_addr: GuestAddr(0xc000000),
                size: 0x4000000,
                irq: None,
            }),
            num_harts: 1,
            clock_freq: 10_000_000, // 10 MHz
        }
    }
}

/// 最小设备集管理器
pub struct MinimalDeviceSetManager {
    /// 配置
    config: MinimalDeviceSetConfig,
    /// Console 设备
    console: Option<Arc<Mutex<VirtioConsole>>>,
    /// Timer 设备 (CLINT)
    timer: Option<Arc<Mutex<Clint>>>,
    /// Block 设备服务
    block_service: Option<BlockDeviceService>,
    /// Network 设备
    net: Option<Arc<Mutex<VhostNet>>>,
    /// PLIC (中断控制器)
    plic: Option<Arc<Mutex<Plic>>>,
}

impl MinimalDeviceSetManager {
    /// 创建新的最小设备集管理器
    pub fn new(config: MinimalDeviceSetConfig) -> Self {
        Self {
            config,
            console: None,
            timer: None,
            block_service: None,
            net: None,
            plic: None,
        }
    }

    /// 初始化最小设备集
    pub async fn initialize(&mut self) -> Result<(), VmError> {
        log::info!("初始化最小设备集...");

        // 1. 初始化 Console 设备
        if let Some(console_config) = &self.config.console {
            let console = Arc::new(Mutex::new(VirtioConsole::new(1)));
            self.console = Some(console.clone());

            log::info!(
                "Console 设备已初始化: 地址=0x{:x}, 大小=0x{:x}, IRQ={:?}",
                console_config.base_addr.0,
                console_config.size,
                console_config.irq
            );
        }

        // 2. 初始化 Timer 设备 (CLINT)
        if let Some(timer_config) = &self.config.timer {
            let timer = Arc::new(Mutex::new(Clint::new(
                self.config.num_harts,
                self.config.clock_freq,
            )));
            self.timer = Some(timer.clone());

            log::info!(
                "Timer 设备已初始化: 地址=0x{:x}, 大小=0x{:x}, IRQ={:?}, harts={}, freq={}Hz",
                timer_config.base_addr.0,
                timer_config.size,
                timer_config.irq,
                self.config.num_harts,
                self.config.clock_freq
            );
        }

        // 3. 初始化 PLIC (中断控制器)
        if let Some(plic_config) = &self.config.plic {
            let plic = Arc::new(Mutex::new(Plic::new(2, 127))); // 2 contexts, 127 interrupts
            self.plic = Some(plic.clone());

            log::info!(
                "PLIC 已初始化: 地址=0x{:x}, 大小=0x{:x}",
                plic_config.base_addr.0,
                plic_config.size
            );
        }

        // 4. 初始化 Block 设备 (可选，需要文件路径)
        // 注意：Block 设备需要单独配置，因为需要文件路径
        // 这里只创建服务框架，实际设备由外部配置

        // 5. 初始化 Network 设备
        if let Some(net_config) = &self.config.net {
            let net = Arc::new(Mutex::new(VhostNet::new()));
            self.net = Some(net.clone());

            log::info!(
                "Network 设备已初始化: 地址=0x{:x}, 大小=0x{:x}, IRQ={:?}",
                net_config.base_addr.0,
                net_config.size,
                net_config.irq
            );
        }

        log::info!("最小设备集初始化完成");
        Ok(())
    }

    /// 添加 Block 设备
    pub async fn add_block_device(
        &mut self,
        file_path: &str,
        _device_config: DeviceConfig,
    ) -> Result<(), VmError> {
        let service = BlockDeviceService::open(file_path, false).await?;
        self.block_service = Some(service.clone());

        log::info!("Block 设备已添加: 文件={}", file_path);

        Ok(())
    }

    /// 注册所有设备到 MMU
    ///
    /// 将设备映射到MMIO地址空间，使Guest OS能够访问设备
    pub fn register_to_mmu(&self, mmu: &mut dyn MMU) -> Result<(), VmError> {
        log::info!("注册设备到 MMU...");

        // 1. 注册 Console 设备
        if let (Some(console_config), Some(console)) = (&self.config.console, &self.console) {
            let console_mmio = crate::virtio_console::VirtioConsoleMmio::from_arc(console.clone());
            mmu.map_mmio(
                console_config.base_addr,
                console_config.size,
                Box::new(console_mmio),
            );
            log::info!(
                "Console 设备已注册: 地址={:#x}, 大小={:#x}",
                console_config.base_addr.0,
                console_config.size
            );
        }

        // 2. 注册 Timer (CLINT) 设备
        if let (Some(timer_config), Some(timer)) = (&self.config.timer, &self.timer) {
            let timer_mmio = crate::clint::ClintMmio::new(timer.clone());
            mmu.map_mmio(
                timer_config.base_addr,
                timer_config.size,
                Box::new(timer_mmio),
            );
            log::info!(
                "Timer 设备已注册: 地址={:#x}, 大小={:#x}",
                timer_config.base_addr.0,
                timer_config.size
            );
        }

        // 3. 注册 PLIC (中断控制器)
        if let (Some(plic_config), Some(plic)) = (&self.config.plic, &self.plic) {
            let plic_mmio = crate::plic::PlicMmio::new(plic.clone());
            mmu.map_mmio(plic_config.base_addr, plic_config.size, Box::new(plic_mmio));
            log::info!(
                "PLIC 已注册: 地址={:#x}, 大小={:#x}",
                plic_config.base_addr.0,
                plic_config.size
            );
        }

        // 4. Block 和 Network 设备需要更复杂的注册逻辑
        // 它们通常通过 VirtIO 队列机制工作，需要额外的配置
        if self.config.block.is_some() {
            log::info!("Block 设备配置存在，但需要额外的 VirtIO 队列设置");
        }

        if self.config.net.is_some() {
            log::info!("Network 设备配置存在，但需要额外的 VirtIO 队列设置");
        }

        log::info!("设备注册完成");
        Ok(())
    }

    /// 处理中断
    ///
    /// 根据 IRQ 号路由到相应的设备，并触发设备的中断处理逻辑
    pub fn handle_interrupt(&self, irq: u32) -> Result<(), VmError> {
        log::debug!("处理中断: IRQ {}", irq);

        // 根据 IRQ 号路由到相应的设备
        match irq {
            1 => {
                // Console 中断
                if let Some(_console) = &self.console {
                    // VirtIO Console 中断通常表示有数据可用或队列状态变化
                    // 实际处理由 Guest OS 通过 MMIO 读取队列状态来完成
                    log::trace!("Console 中断已路由 (IRQ 1)");
                } else {
                    log::warn!("Console 中断触发但设备未初始化");
                }
            }
            2 => {
                // Block 设备中断
                if let Some(_block_service) = &self.block_service {
                    // VirtIO Block 中断表示 I/O 操作完成
                    log::trace!("Block 设备中断已路由 (IRQ 2)");
                } else {
                    log::warn!("Block 设备中断触发但设备未初始化");
                }
            }
            3 => {
                // Network 中断
                if let Some(_net) = &self.net {
                    // VirtIO Network 中断表示有数据包到达或发送完成
                    log::trace!("Network 中断已路由 (IRQ 3)");
                } else {
                    log::warn!("Network 中断触发但设备未初始化");
                }
            }
            7 => {
                // Timer 中断 (CLINT)
                if let Some(timer) = &self.timer {
                    // 清除定时器中断标志
                    // CLINT 的定时器中断通过读取/写入 mtimecmp 寄存器来清除
                    // 这里只是记录中断已处理
                    let _timer_lock = timer.lock();
                    log::trace!("Timer 中断已路由 (IRQ 7)");
                } else {
                    log::warn!("Timer 中断触发但设备未初始化");
                }
            }
            _ => {
                log::warn!("未知中断: IRQ {}", irq);
            }
        }

        Ok(())
    }

    /// 通过 PLIC 处理中断
    ///
    /// 如果配置了 PLIC，使用 PLIC 来路由中断；否则直接使用 IRQ 号
    pub fn handle_interrupt_via_plic(&self, context: usize) -> Result<Option<u32>, VmError> {
        if let Some(plic) = &self.plic {
            let mut plic_lock = plic.lock();

            // 检查是否有待处理的中断
            if plic_lock.has_interrupt(context) {
                // 声明中断（获取中断号）
                let interrupt_id = plic_lock.claim_mut(context);

                if interrupt_id > 0 {
                    log::debug!(
                        "PLIC 路由中断: context={}, interrupt_id={}",
                        context,
                        interrupt_id
                    );

                    // 根据中断 ID 路由到设备
                    // 中断 ID 映射：
                    // 1 = Console
                    // 2 = Block
                    // 3 = Network
                    // 7 = Timer
                    let irq = interrupt_id;
                    self.handle_interrupt(irq)?;

                    // 完成中断处理（通过写入 COMPLETE 寄存器）
                    // 注意：complete 是私有方法，需要通过 MMIO 写入来完成
                    // 这里我们直接调用内部方法（如果可用）或通过其他方式完成
                    // 简化实现：中断完成后由 Guest OS 通过 MMIO 写入完成
                    log::trace!("中断处理完成，等待 Guest OS 确认");

                    return Ok(Some(irq));
                }
            }
        }

        Ok(None)
    }

    /// 更新定时器
    pub fn update_timer(&self) {
        if let Some(timer) = &self.timer {
            let mut timer_lock = timer.lock();
            timer_lock.update_time();

            // 检查是否有定时器中断
            if timer_lock.has_timer_interrupt(0) {
                // 触发中断
                log::trace!("定时器中断已触发");
                let _ = self.handle_interrupt(7);
            }
        }
    }

    /// 获取控制台输出
    pub fn read_console(&self) -> Option<Vec<u8>> {
        // TODO: 实现控制台数据读取
        // VirtioConsole 需要更复杂的缓冲区管理
        None
    }

    /// 写入控制台输入
    pub fn write_console(&self, data: &[u8]) -> Result<(), VmError> {
        if let Some(console) = &self.console {
            let _console_lock = console.lock();
            // TODO: 实现控制台数据写入
            // VirtioConsole 需要更复杂的缓冲区管理
            // 使用下划线前缀表示暂时未使用，但保留用于未来实现
            let _ = data; // 暂时忽略
            Ok(())
        } else {
            Err(VmError::Device(vm_core::DeviceError::NotFound {
                device_type: "console".to_string(),
                identifier: "console".to_string(),
            }))
        }
    }

    /// 获取设备统计信息
    pub fn get_stats(&self) -> HashMap<String, String> {
        let mut stats = HashMap::new();

        if let Some(console) = &self.console {
            let console_lock = console.lock();
            stats.insert(
                "console_queues".to_string(),
                console_lock.num_queues().to_string(),
            );
        }

        if let Some(_timer) = &self.timer {
            stats.insert("timer_harts".to_string(), self.config.num_harts.to_string());
        }

        if let Some(_block_service) = &self.block_service {
            stats.insert("block_device".to_string(), "present".to_string());
        }

        if let Some(net) = &self.net {
            let net_lock = net.lock();
            stats.insert("net_queues".to_string(), net_lock.num_queues().to_string());
        }

        stats
    }

    /// 获取控制台设备引用
    pub fn console(&self) -> Option<Arc<Mutex<VirtioConsole>>> {
        self.console.as_ref().cloned()
    }

    /// 获取定时器设备引用
    pub fn timer(&self) -> Option<Arc<Mutex<Clint>>> {
        self.timer.as_ref().cloned()
    }

    /// 获取块设备服务引用
    pub fn block_service(&self) -> Option<&BlockDeviceService> {
        self.block_service.as_ref()
    }

    /// 获取网络设备引用
    pub fn net(&self) -> Option<Arc<Mutex<VhostNet>>> {
        self.net.as_ref().cloned()
    }

    /// 获取 PLIC 引用
    pub fn plic(&self) -> Option<Arc<Mutex<Plic>>> {
        self.plic.as_ref().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_minimal_device_set_initialization() {
        let config = MinimalDeviceSetConfig::default();
        let mut manager = MinimalDeviceSetManager::new(config);

        let result = manager.initialize().await;
        assert!(result.is_ok());

        // 验证设备已创建
        assert!(manager.console.is_some());
        assert!(manager.timer.is_some());
        assert!(manager.net.is_some());
        assert!(manager.plic.is_some());

        // 验证设备已创建
        assert!(manager.console.is_some());
        assert!(manager.timer.is_some());
        assert!(manager.net.is_some());
        assert!(manager.plic.is_some());
    }

    #[tokio::test]
    async fn test_console_operations() {
        let config = MinimalDeviceSetConfig::default();
        let mut manager = MinimalDeviceSetManager::new(config);

        manager.initialize().await.unwrap();

        // 测试写入控制台
        let test_data = b"Hello, World!";
        let result = manager.write_console(test_data);
        assert!(result.is_ok());

        // 测试读取控制台 (应该返回写入的数据)
        let output = manager.read_console();
        assert!(output.is_some());
        // 注意：实际的 virtio_console 实现可能有不同的行为
    }

    #[test]
    fn test_device_config() {
        let config = MinimalDeviceSetConfig::default();

        // 验证默认配置
        assert!(config.console.is_some());
        assert!(config.timer.is_some());
        assert!(config.block.is_some());
        assert!(config.net.is_some());
        assert!(config.plic.is_some());
        assert_eq!(config.num_harts, 1);
        assert_eq!(config.clock_freq, 10_000_000);
    }
}
