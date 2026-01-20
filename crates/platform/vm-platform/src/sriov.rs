//! SR-IOV虚拟化支持
//!
//! 提供单根I/O虚拟化支持，包括VF管理和MAC配置
//! 从 vm-passthrough/sriov.rs 迁移而来

use super::{PassthroughError, PciAddress, PciDeviceInfo};

/// SR-IOV VF 配置
#[derive(Debug, Clone)]
pub struct VfConfig {
    pub pf_address: PciAddress,    // 物理函数地址
    pub vf_id: u8,             // 虚拟函数ID
    pub mac_address: Option<String>,
    pub vlan_id: Option<u16>,
}

impl VfConfig {
    /// 创建新的 VF 配置
    pub fn new(pf_address: PciAddress, vf_id: u8) -> Self {
        Self {
            pf_address,
            vf_id,
            mac_address: None,
            vlan_id: None,
        }
    }

    /// 设置 MAC 地址
    pub fn set_mac(&mut self, mac: &str) {
        self.mac_address = Some(mac.to_string());
    }

    /// 设置 VLAN ID
    pub fn set_vlan(&mut self, vlan: u16) {
        self.vlan_id = Some(vlan);
    }
}

/// SR-IOV VF 管理器特征
pub trait SriovVfManager {
    /// 创建 VF
    fn create_vf(&mut self, pf_address: PciAddress, count: u8) -> Result<(), PassthroughError>;

    /// 删除 VF
    fn delete_vf(&mut self, pf_address: PciAddress, vf_id: u8) -> Result<(), PassthroughError>;

    /// 获取所有 VF 配置
    fn get_vfs(&self, pf_address: PciAddress) -> Result<Vec<VfConfig>, PassthroughError>;

    /// 配置 VF
    fn configure_vf(&mut self, vf: &VfConfig) -> Result<(), PassthroughError>;
}

/// SR-IOV QoS 配置
#[derive(Debug, Clone)]
pub struct QosConfig {
    pub max_tx_rate: Option<u32>,  // 最大发送速率（Mbps）
    pub max_rx_rate: Option<u32>,  // 最大接收速率（Mbps）
    pub min_tx_rate: Option<u32>,  // 最小发送速率（Mbps）
    pub min_rx_rate: Option<u32>,  // 最小接收速率（Mbps）
}

/// SR-IOV VF 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfState {
    Disabled,
    Enabled,
    InUse,
}

/// 简化版本的 SR-IOV VF 管理器
pub struct SimpleSriovVfManager {
    vfs: std::collections::HashMap<String, Vec<VfConfig>>,
}

impl SimpleSriovVfManager {
    /// 创建新的 SR-IOV VF 管理器
    pub fn new() -> Self {
        Self {
            vfs: std::collections::HashMap::new(),
        }
    }

    /// 扫描所有 SR-IOV 设备
    pub fn scan_devices(&mut self) -> Result<(), PassthroughError> {
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            use std::path::Path;

            log::info!("Scanning SR-IOV devices...");

            let pci_path = Path::new("/sys/bus/pci/devices");
            if !pci_path.exists() {
                log::warn!("PCI devices path not found");
                return Ok(());
            }

            // 遍历所有 PCI 设备
            for entry in fs::read_dir(pci_path)
                .map_err(|e| PassthroughError::IoError(e.to_string()))?
            {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                let device_path = entry.path();

                // 检查设备是否支持 SR-IOV（存在 sriov_numvfs 文件）
                let sriov_numvfs_path = device_path.join("sriov_numvfs");
                if !sriov_numvfs_path.exists() {
                    continue;
                }

                // 读取 SR-IOV 信息
                let addr_str = entry.file_name().to_string_lossy().to_string();
                let address = match PciAddress::from_str(&addr_str) {
                    Ok(addr) => addr,
                    Err(_) => continue,
                };

                // 读取 total VFs 数量
                let total_vfs = fs::read_to_string(device_path.join("sriov_totalvfs"))
                    .and_then(|s| Ok(s.trim().parse::<u8>().unwrap_or(0)))
                    .unwrap_or(0);

                // 读取当前 VFs 数量
                let current_vfs = fs::read_to_string(&sriov_numvfs_path)
                    .and_then(|s| Ok(s.trim().parse::<u8>().unwrap_or(0)))
                    .unwrap_or(0);

                log::info!(
                    "Found SR-IOV device {} - Total VFs: {}, Current VFs: {}",
                    address,
                    total_vfs,
                    current_vfs
                );

                // 初始化该 PF 的 VF 列表
                let mut vfs = Vec::new();
                for vf_id in 0..current_vfs {
                    vfs.push(VfConfig::new(address, vf_id));
                }

                self.vfs.insert(address.to_string(), vfs);
            }

            log::info!("SR-IOV device scan completed");
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            log::warn!("SR-IOV only supported on Linux");
        }

        Ok(())
    }

    /// 创建 VF
    pub fn create_vf(&mut self, pf_address: PciAddress, count: u8) -> Result<(), PassthroughError> {
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            use std::path::Path;

            log::info!("Creating {} VF(s) for {}", count, pf_address.to_string());

            let pci_path = format!(
                "/sys/bus/pci/devices/{}",
                pf_address.to_string()
            );
            let pci_path = Path::new(&pci_path);

            // 1. 检查设备是否存在
            if !pci_path.exists() {
                return Err(PassthroughError::DeviceNotFound(
                    pf_address.to_string(),
                ));
            }

            // 2. 检查是否支持 SR-IOV
            let sriov_numvfs_path = pci_path.join("sriov_numvfs");
            if !sriov_numvfs_path.exists() {
                return Err(PassthroughError::DriverBindingFailed(
                    "Device does not support SR-IOV".to_string(),
                ));
            }

            // 3. 读取最大 VF 数量
            let total_vfs_path = pci_path.join("sriov_totalvfs");
            let total_vfs: u8 = fs::read_to_string(&total_vfs_path)
                .map_err(|e| PassthroughError::IoError(e.to_string()))?
                .trim()
                .parse()
                .map_err(|_| PassthroughError::IoError("Invalid VF count".to_string()))?;

            if count > total_vfs {
                return Err(PassthroughError::DriverBindingFailed(
                    format!("Requested {} VFs but only {} available", count, total_vfs),
                ));
            }

            // 4. 创建 VFs（写入 sriov_numvfs 文件）
            fs::write(&sriov_numvfs_path, count.to_string().as_bytes())
                .map_err(|e| PassthroughError::IoError(format!("Failed to create VFs: {}", e)))?;

            log::info!("Successfully created {} VFs for {}", count, pf_address);

            // 5. 更新内部 VF 列表
            let mut vfs = Vec::new();
            for vf_id in 0..count {
                vfs.push(VfConfig::new(pf_address, vf_id));
            }
            self.vfs.insert(pf_address.to_string(), vfs);
        }

        #[cfg(not(target_os = "linux"))]
        {
            return Err(PassthroughError::DriverBindingFailed(
                "SR-IOV only supported on Linux".to_string(),
            ));
        }

        Ok(())
    }

    /// 删除 VF
    pub fn delete_vf(&mut self, pf_address: PciAddress, vf_id: u8) -> Result<(), PassthroughError> {
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            use std::path::Path;

            log::info!("Deleting VF {} for {}", vf_id, pf_address.to_string());

            let pci_path = format!(
                "/sys/bus/pci/devices/{}",
                pf_address.to_string()
            );
            let pci_path = Path::new(&pci_path);

            // 1. 检查设备是否存在
            if !pci_path.exists() {
                return Err(PassthroughError::DeviceNotFound(
                    pf_address.to_string(),
                ));
            }

            // 2. 检查是否支持 SR-IOV
            let sriov_numvfs_path = pci_path.join("sriov_numvfs");
            if !sriov_numvfs_path.exists() {
                return Err(PassthroughError::DriverBindingFailed(
                    "Device does not support SR-IOV".to_string(),
                ));
            }

            // 3. 读取当前 VF 数量
            let current_vfs: u8 = fs::read_to_string(&sriov_numvfs_path)
                .map_err(|e| PassthroughError::IoError(e.to_string()))?
                .trim()
                .parse()
                .unwrap_or(0);

            if vf_id >= current_vfs {
                return Err(PassthroughError::DriverBindingFailed(
                    format!("VF {} does not exist (current VFs: {})", vf_id, current_vfs),
                ));
            }

            // 4. 删除所有 VFs（通过写入 0 到 sriov_numvfs）
            // 注意：Linux SR-IOV 不支持删除单个 VF，只能删除所有 VFs
            fs::write(&sriov_numvfs_path, b"0")
                .map_err(|e| PassthroughError::IoError(format!("Failed to delete VFs: {}", e)))?;

            log::info!("Deleted all VFs for {}", pf_address);

            // 5. 清空内部 VF 列表
            self.vfs.remove(&pf_address.to_string());

            // 6. 如果需要，重新创建剩余的 VFs
            if vf_id > 0 {
                // 保留 vf_id 之前的 VFs
                let new_count = vf_id;
                fs::write(&sriov_numvfs_path, new_count.to_string().as_bytes())
                    .map_err(|e| PassthroughError::IoError(format!("Failed to recreate VFs: {}", e)))?;

                let mut vfs = Vec::new();
                for i in 0..new_count {
                    vfs.push(VfConfig::new(pf_address, i));
                }
                self.vfs.insert(pf_address.to_string(), vfs);

                log::info!("Recreated {} VFs for {}", new_count, pf_address);
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            log::warn!("SR-IOV VF deletion only supported on Linux");
        }

        Ok(())
    }

    /// 获取所有 VF 配置
    pub fn get_vfs(&self, pf_address: PciAddress) -> Result<Vec<VfConfig>, PassthroughError> {
        Ok(self.vfs
            .get(&pf_address.to_string())
            .cloned()
            .unwrap_or_default())
    }

    /// 配置 VF
    pub fn configure_vf(&mut self, vf: &VfConfig) -> Result<(), PassthroughError> {
        let pf_key = vf.pf_address.to_string();
        if let Some(vfs) = self.vfs.get_mut(&pf_key) {
            // 更新现有 VF
            if let Some(existing) = vfs.iter_mut().find(|v| v.vf_id == vf.vf_id) {
                existing.mac_address = vf.mac_address.clone();
                existing.vlan_id = vf.vlan_id;
            }
        } else {
            // 添加新 VF
            vfs.push(vf.clone());
        }
        } else {
            // 创建新 PF 列表
            self.vfs.insert(pf_key, vec![vf.clone()]);
        }

        log::info!("Configured VF {} for PF {}", vf.vf_id, pf_key);
        Ok(())
    }
}

impl Default for SimpleSriovVfManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SriovVfManager for SimpleSriovVfManager {
    fn create_vf(&mut self, pf_address: PciAddress, count: u8) -> Result<(), PassthroughError> {
        self.create_vf(pf_address, count)
    }

    fn delete_vf(&mut self, pf_address: PciAddress, vf_id: u8) -> Result<(), PassthroughError> {
        self.delete_vf(pf_address, vf_id)
    }

    fn get_vfs(&self, pf_address: PciAddress) -> Result<Vec<VfConfig>, PassthroughError> {
        self.get_vfs(pf_address)
    }

    fn configure_vf(&mut self, vf: &VfConfig) -> Result<(), PassthroughError> {
        self.configure_vf(vf)
    }
}
