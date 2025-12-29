//! PCIe设备管理
//!
//! 提供PCIe设备地址管理、设备枚举、VFIO和IOMMU支持
//! 从 vm-passthrough/pcie.rs 迁移而来

use super::{PassthroughError, PciAddress, PciDeviceInfo};

/// IOMMU 组
#[derive(Debug, Clone)]
pub struct IommuGroup {
    pub id: u32,
    pub devices: Vec<PciAddress>,
}

/// VFIO 设备
pub struct VfioDevice {
    pub address: PciAddress,
    pub info: PciDeviceInfo,
    pub iommu_group: u32,
    pub vfio_fd: Option<i32>,
}

impl VfioDevice {
    /// 创建新的 VFIO 设备
    pub fn new(address: PciAddress, info: PciDeviceInfo) -> Result<Self, PassthroughError> {
        let iommu_group = Self::get_iommu_group(address)?;

        Ok(Self {
            address,
            info,
            iommu_group,
            vfio_fd: None,
        })
    }

    /// 获取设备的 IOMMU 组
    #[cfg(target_os = "linux")]
    fn get_iommu_group(address: PciAddress) -> Result<u32, PassthroughError> {
        use std::fs;
        use std::path::PathBuf;

        let device_path = format!("/sys/bus/pci/devices/{}", address.to_string());
        let iommu_link = PathBuf::from(device_path).join("iommu_group");

        if !iommu_link.exists() {
            return Err(PassthroughError::IommuNotEnabled);
        }

        let target = fs::read_link(iommu_link)?;
        let group_name = target
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| PassthroughError::InvalidAddress("Invalid IOMMU group".to_string()))?;

        group_name
            .parse::<u32>()
            .map_err(|_| PassthroughError::InvalidAddress("Invalid IOMMU group ID".to_string()))
    }

    #[cfg(not(target_os = "linux"))]
    fn get_iommu_group(_address: PciAddress) -> Result<u32, PassthroughError> {
        Err(PassthroughError::IommuNotEnabled)
    }

    /// 绑定到 VFIO 驱动
    #[cfg(target_os = "linux")]
    pub fn bind_vfio_driver(&self) -> Result<(), PassthroughError> {
        use std::fs;
        use std::io::Write;
        use std::path::PathBuf;

        let addr_str = self.address.to_string();
        let device_path = format!("/sys/bus/pci/devices/{}", addr_str);

        //1. 解绑当前驱动
        let driver_path = PathBuf::from(&device_path).join("driver");
        if driver_path.exists() {
            let unbind_path = driver_path.join("unbind");
            if let Ok(mut file) = fs::OpenOptions::new().write(true).open(unbind_path) {
                let _ = file.write_all(addr_str.as_bytes());
            }
        }

        //2. 绑定到 vfio-pci
        let new_id_path = "/sys/bus/pci/drivers/vfio-pci/new_id";
        if Path::new(new_id_path).exists() {
            let id_str = format!("{:04x} {:04x}", self.info.vendor_id, self.info.device_id);
            if let Ok(mut file) = fs::OpenOptions::new().write(true).open(new_id_path) {
                let _ = file.write_all(id_str.as_bytes());
            }
        }

        let bind_path = "/sys/bus/pci/drivers/vfio-pci/bind";
        if Path::new(bind_path).exists() {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .open(bind_path)
                .map_err(|e| PassthroughError::DriverBindingFailed(e.to_string()))?;
            file.write_all(addr_str.as_bytes())
                .map_err(|e| PassthroughError::DriverBindingFailed(e.to_string()))?;
        }

        log::info!("Bound device {} to vfio-pci driver", addr_str);
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn bind_vfio_driver(&self) -> Result<(), PassthroughError> {
        Err(PassthroughError::DriverBindingFailed(
            "VFIO only supported on Linux".to_string(),
        ))
    }

    /// 打开 VFIO 设备
    #[cfg(target_os = "linux")]
    pub fn open_vfio(&mut self) -> Result<(), PassthroughError> {
        use std::fs::OpenOptions;
        use std::os::unix::io::AsRawFd;

        let group_path = format!("/dev/vfio/{}", self.iommu_group);
        let group_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&group_path)
            .map_err(|_| PassthroughError::PermissionDenied)?;

        self.vfio_fd = Some(group_file.as_raw_fd());
        log::info!("Opened VFIO device for group {}", self.iommu_group);
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn open_vfio(&mut self) -> Result<(), PassthroughError> {
        Err(PassthroughError::DriverBindingFailed(
            "VFIO only supported on Linux".to_string(),
        ))
    }

    /// 检查 IOMMU 是否启用
    #[cfg(target_os = "linux")]
    pub fn check_iommu_enabled() -> bool {
        Path::new("/sys/kernel/iommu_groups").exists()
    }

    #[cfg(not(target_os = "linux"))]
    pub fn check_iommu_enabled() -> bool {
        false
    }
}

/// IOMMU 管理器
pub struct IommuManager {
    groups: Vec<IommuGroup>,
}

impl IommuManager {
    /// 创建新的 IOMMU 管理器
    pub fn new() -> Self {
        Self { groups: Vec::new() }
    }

    /// 扫描 IOMMU 组
    #[cfg(target_os = "linux")]
    pub fn scan_groups(&mut self) -> Result<(), PassthroughError> {
        use std::fs;
        use std::path::Path;

        let iommu_path = Path::new("/sys/kernel/iommu_groups");
        if !iommu_path.exists() {
            return Err(PassthroughError::IommuNotEnabled);
        }

        for entry in fs::read_dir(iommu_path)? {
            let entry = entry?;
            let group_id = entry
                .file_name()
                .to_string_lossy()
                .parse::<u32>()
                .unwrap_or(0);

            let devices_path = entry.path().join("devices");
            let mut devices = Vec::new();

            if devices_path.exists() {
                for dev_entry in fs::read_dir(devices_path)? {
                    let dev_entry = dev_entry?;
                    let addr_str = dev_entry.file_name().to_string_lossy().to_string();
                    if let Ok(addr) = PciAddress::from_str(&addr_str) {
                        devices.push(addr);
                    }
                }
            }

            self.groups.push(IommuGroup {
                id: group_id,
                devices,
            });
        }

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn scan_groups(&mut self) -> Result<(), PassthroughError> {
        Err(PassthroughError::IommuNotEnabled)
    }

    /// 获取所有 IOMMU 组
    pub fn get_groups(&self) -> &[IommuGroup] {
        &self.groups
    }

    /// 打印 IOMMU 组信息
    pub fn print_groups(&self) {
        println!("\n=== IOMMU Groups ===");
        for group in &self.groups {
            println!("Group {}: {} device(s)", group.id, group.devices.len());
            for addr in &group.devices {
                println!("  - {}", addr);
            }
        }
    }
}

impl Default for IommuManager {
    fn default() -> Self {
        Self::new()
    }
}
