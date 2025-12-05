//! PCIe 设备直通实现
//!
//! 支持 VFIO (Virtual Function I/O) 和 IOMMU

use super::{PassthroughError, PciAddress, PciDeviceInfo};
use std::path::PathBuf;

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

        let addr_str = self.address.to_string();
        let device_path = format!("/sys/bus/pci/devices/{}", addr_str);

        // 1. 解绑当前驱动
        let driver_path = PathBuf::from(&device_path).join("driver");
        if driver_path.exists() {
            let unbind_path = driver_path.join("unbind");
            if let Ok(mut file) = fs::OpenOptions::new().write(true).open(unbind_path) {
                let _ = file.write_all(addr_str.as_bytes());
            }
        }

        // 2. 绑定到 vfio-pci
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
                println!("  - {}", addr.to_string());
            }
        }
    }
}

impl Default for IommuManager {
    fn default() -> Self {
        Self::new()
    }
}

/// PCIe 配置空间访问
pub struct PciConfigSpace {
    _address: PciAddress,
    config_path: PathBuf,
}

impl PciConfigSpace {
    pub fn new(address: PciAddress) -> Self {
        let config_path = PathBuf::from(format!(
            "/sys/bus/pci/devices/{}/config",
            address.to_string()
        ));
        Self {
            _address: address,
            config_path,
        }
    }

    /// 读取配置空间
    #[cfg(target_os = "linux")]
    pub fn read(&self, offset: usize, size: usize) -> Result<Vec<u8>, PassthroughError> {
        use std::fs;
        use std::io::{Read, Seek};

        let mut file = fs::File::open(&self.config_path)?;
        let mut buffer = vec![0u8; size];

        file.seek(std::io::SeekFrom::Start(offset as u64))?;
        file.read_exact(&mut buffer)?;

        Ok(buffer)
    }

    #[cfg(not(target_os = "linux"))]
    pub fn read(&self, _offset: usize, _size: usize) -> Result<Vec<u8>, PassthroughError> {
        Err(PassthroughError::IoError(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Not supported on this platform",
        )))
    }

    /// 写入配置空间
    #[cfg(target_os = "linux")]
    pub fn write(&self, offset: usize, data: &[u8]) -> Result<(), PassthroughError> {
        use std::fs;
        use std::io::{Seek, Write};

        let mut file = fs::OpenOptions::new().write(true).open(&self.config_path)?;

        file.seek(std::io::SeekFrom::Start(offset as u64))?;
        file.write_all(data)?;

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn write(&self, _offset: usize, _data: &[u8]) -> Result<(), PassthroughError> {
        Err(PassthroughError::IoError(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Not supported on this platform",
        )))
    }

    /// 读取 16 位值
    pub fn read_u16(&self, offset: usize) -> Result<u16, PassthroughError> {
        let data = self.read(offset, 2)?;
        Ok(u16::from_le_bytes([data[0], data[1]]))
    }

    /// 读取 32 位值
    pub fn read_u32(&self, offset: usize) -> Result<u32, PassthroughError> {
        let data = self.read(offset, 4)?;
        Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
    }

    /// 写入 16 位值
    pub fn write_u16(&self, offset: usize, value: u16) -> Result<(), PassthroughError> {
        self.write(offset, &value.to_le_bytes())
    }

    /// 写入 32 位值
    pub fn write_u32(&self, offset: usize, value: u32) -> Result<(), PassthroughError> {
        self.write(offset, &value.to_le_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iommu_check() {
        let enabled = VfioDevice::check_iommu_enabled();
        println!("IOMMU enabled: {}", enabled);
    }

    #[test]
    fn test_iommu_groups() {
        let mut manager = IommuManager::new();
        if let Ok(_) = manager.scan_groups() {
            manager.print_groups();
        }
    }
}
