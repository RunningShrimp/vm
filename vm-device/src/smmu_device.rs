//! SMMU 设备管理模块
//!
//! 本模块提供 ARM SMMU (System MMU) 设备的分配和管理功能，
//! 允许虚拟机设备通过 SMMU 进行安全的 DMA 操作。
//!
//! ## 功能
//!
//! - PCIe 设备 SMMU 分配
//! - Stream ID 管理
//! - DMA 地址转换
//! - 设备热插拔支持
//!
//! ## 使用示例
//!
//! ```ignore
//! use vm_device::smmu_device::SmmuDeviceManager;
//! use vm_accel::SmmuManager;
//!
//! let smmu_manager = SmmuManager::new();
//! smmu_manager.init()?;
//!
//! let device_manager = SmmuDeviceManager::new(smmu_manager);
//!
//! // 分配设备到 SMMU
//! device_manager.assign_device("0000:01:00.0", 0x1000, 0x10000)?;
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vm_accel::SmmuManager;
use vm_core::GuestAddr;

/// SMMU 设备分配信息
#[derive(Debug, Clone)]
pub struct SmmuDeviceInfo {
    /// PCIe BDF (Bus:Device.Function)
    pub bdf: String,
    /// 设备 ID (唯一标识符)
    pub device_id: String,
    /// 分配的 Stream ID
    pub stream_id: u16,
    /// DMA 地址范围
    pub dma_range: (GuestAddr, GuestAddr),
    /// 是否已启用
    pub enabled: bool,
}

impl SmmuDeviceInfo {
    /// 创建新的 SMMU 设备信息
    pub fn new(
        bdf: String,
        device_id: String,
        stream_id: u16,
        dma_range: (GuestAddr, GuestAddr),
    ) -> Self {
        Self {
            bdf,
            device_id,
            stream_id,
            dma_range,
            enabled: false,
        }
    }
}

/// SMMU 设备管理器
///
/// 管理虚拟机中所有需要 SMMU 保护的设备，负责设备分配、
/// Stream ID 管理和 DMA 地址空间配置。
///
/// # 标识
/// SMMU 设备管理类
#[derive(Clone)]
pub struct SmmuDeviceManager {
    /// SMMU 管理器引用
    smmu_manager: Arc<SmmuManager>,
    /// 设备映射 (device_id -> SmmuDeviceInfo)
    devices: Arc<Mutex<HashMap<String, SmmuDeviceInfo>>>,
    /// PCIe BDF 到 device_id 的映射
    bdf_map: Arc<Mutex<HashMap<String, String>>>,
}

impl SmmuDeviceManager {
    /// 创建新的 SMMU 设备管理器
    ///
    /// # 参数
    ///
    /// * `smmu_manager` - SMMU 管理器实例
    ///
    /// # 示例
    ///
    /// ```
    /// use vm_device::smmu_device::SmmuDeviceManager;
    /// use vm_accel::SmmuManager;
    ///
    /// let smmu = SmmuManager::new();
    /// let manager = SmmuDeviceManager::new(smmu);
    /// ```
    pub fn new(smmu_manager: Arc<SmmuManager>) -> Self {
        Self {
            smmu_manager,
            devices: Arc::new(Mutex::new(HashMap::new())),
            bdf_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 分配设备到 SMMU
    ///
    /// 为 PCIe 设备分配 SMMU Stream ID 并配置 DMA 地址空间。
    ///
    /// # 参数
    ///
    /// * `bdf` - PCIe BDF 标识符 (格式: "BBBB:DD:F.F")
    /// * `dma_start` - DMA 地址空间起始地址
    /// * `dma_size` - DMA 地址空间大小
    ///
    /// # 返回值
    ///
    /// 成功返回分配的 Stream ID，失败返回错误。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let stream_id = manager.assign_device("0000:01:00.0", 0x1000, 0x10000)?;
    /// ```
    pub fn assign_device(&self, bdf: &str, dma_start: u64, dma_size: u64) -> Result<u16, String> {
        // 检查设备是否已分配
        {
            let bdf_map = self
                .bdf_map
                .lock()
                .map_err(|e| format!("BDF map lock poisoned: {}", e))?;

            if let Some(device_id) = bdf_map.get(bdf) {
                let devices = self
                    .devices
                    .lock()
                    .map_err(|e| format!("Devices lock poisoned: {}", e))?;

                if let Some(info) = devices.get(device_id) {
                    log::info!(
                        "Device {} already assigned with stream ID {}",
                        bdf,
                        info.stream_id
                    );
                    return Ok(info.stream_id);
                }
            }
        }

        // 生成唯一的设备 ID
        let device_id = format!("pci-{}", bdf);

        // 计算 DMA 地址范围
        let dma_start = GuestAddr(dma_start);
        let dma_end = GuestAddr(dma_start.0 + dma_size);

        log::info!(
            "Assigning device {} to SMMU (DMA range: 0x{:x}-0x{:x})",
            bdf,
            dma_start.0,
            dma_end.0
        );

        // 通过 SMMU 管理器分配设备
        let stream_id = self
            .smmu_manager
            .attach_device(device_id.clone(), (dma_start, dma_end))
            .map_err(|e| format!("Failed to attach device to SMMU: {:?}", e))?;

        // 存储设备信息
        let device_info = SmmuDeviceInfo::new(
            bdf.to_string(),
            device_id.clone(),
            stream_id,
            (dma_start, dma_end),
        );

        {
            let mut devices = self
                .devices
                .lock()
                .map_err(|e| format!("Devices lock poisoned: {}", e))?;

            let mut bdf_map = self
                .bdf_map
                .lock()
                .map_err(|e| format!("BDF map lock poisoned: {}", e))?;

            devices.insert(device_id.clone(), device_info);
            bdf_map.insert(bdf.to_string(), device_id);
        }

        log::info!(
            "Device {} assigned to SMMU with stream ID {}",
            bdf,
            stream_id
        );
        Ok(stream_id)
    }

    /// 移除设备分配
    ///
    /// 从 SMMU 中移除设备并释放相关资源。
    ///
    /// # 参数
    ///
    /// * `bdf` - PCIe BDF 标识符
    ///
    /// # 示例
    ///
    /// ```ignore
    /// manager.unassign_device("0000:01:00.0")?;
    /// ```
    pub fn unassign_device(&self, bdf: &str) -> Result<(), String> {
        log::info!("Unassigning device {} from SMMU", bdf);

        // 查找设备 ID
        let device_id = {
            let bdf_map = self
                .bdf_map
                .lock()
                .map_err(|e| format!("BDF map lock poisoned: {}", e))?;

            bdf_map.get(bdf).cloned()
        };

        let device_id = device_id.ok_or_else(|| format!("Device {} not found in SMMU", bdf))?;

        // 从 SMMU 管理器中移除
        self.smmu_manager
            .detach_device(&device_id)
            .map_err(|e| format!("Failed to detach device: {:?}", e))?;

        // 从本地映射中移除
        {
            let mut devices = self
                .devices
                .lock()
                .map_err(|e| format!("Devices lock poisoned: {}", e))?;

            let mut bdf_map = self
                .bdf_map
                .lock()
                .map_err(|e| format!("BDF map lock poisoned: {}", e))?;

            devices.remove(&device_id);
            bdf_map.remove(bdf);
        }

        log::info!("Device {} unassigned from SMMU", bdf);
        Ok(())
    }

    /// 转换设备的 DMA 地址
    ///
    /// 将客户机物理地址转换为主机物理地址。
    ///
    /// # 参数
    ///
    /// * `bdf` - PCIe BDF 标识符
    /// * `guest_addr` - 客户机物理地址
    /// * `size` - 访问大小
    ///
    /// # 返回值
    ///
    /// 成功返回转换后的物理地址，失败返回错误。
    pub fn translate_dma_addr(
        &self,
        bdf: &str,
        guest_addr: GuestAddr,
        size: u64,
    ) -> Result<u64, String> {
        // 查找设备 ID
        let device_id = {
            let bdf_map = self
                .bdf_map
                .lock()
                .map_err(|e| format!("BDF map lock poisoned: {}", e))?;

            bdf_map.get(bdf).cloned()
        };

        let device_id = device_id.ok_or_else(|| format!("Device {} not found in SMMU", bdf))?;

        // 执行地址转换
        self.smmu_manager
            .translate_dma_addr(&device_id, guest_addr, size)
            .map_err(|e| format!("DMA translation failed: {:?}", e))
    }

    /// 获取设备信息
    ///
    /// # 参数
    ///
    /// * `bdf` - PCIe BDF 标识符
    ///
    /// # 返回值
    ///
    /// 返回设备信息克隆，如果设备不存在则返回 None。
    pub fn get_device_info(&self, bdf: &str) -> Option<SmmuDeviceInfo> {
        let bdf_map = self.bdf_map.lock().ok()?;
        let device_id = bdf_map.get(bdf).cloned()?;

        let devices = self.devices.lock().ok()?;
        devices.get(&device_id).cloned()
    }

    /// 列出所有分配的设备
    ///
    /// # 返回值
    ///
    /// 返回所有已分配设备的 BDF 列表。
    pub fn list_devices(&self) -> Vec<String> {
        self.bdf_map
            .lock()
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// 获取分配的设备数量
    pub fn device_count(&self) -> usize {
        self.devices
            .lock()
            .map(|devices| devices.len())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_manager_creation() {
        let smmu = Arc::new(SmmuManager::new());
        let manager = SmmuDeviceManager::new(smmu);
        assert_eq!(manager.device_count(), 0);
        assert!(manager.list_devices().is_empty());
    }

    #[test]
    fn test_device_info_creation() {
        let info = SmmuDeviceInfo::new(
            "0000:01:00.0".to_string(),
            "pci-0000:01:00.0".to_string(),
            0x100,
            (GuestAddr(0x1000), GuestAddr(0x10000)),
        );

        assert_eq!(info.bdf, "0000:01:00.0");
        assert_eq!(info.stream_id, 0x100);
        assert!(!info.enabled);
    }
}
