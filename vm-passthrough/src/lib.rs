//! Hardware Passthrough Module
//!
//! 实现 PCIe 设备直通，支持 GPU 和 NPU 等加速器

use std::collections::HashMap;

// GPU/NPU 加速器模块（通过 feature 控制）
#[cfg(feature = "cuda")]
pub mod cuda;
#[cfg(feature = "cuda")]
pub mod cuda_compiler;

#[cfg(feature = "rocm")]
pub mod rocm;
#[cfg(feature = "rocm")]
pub mod rocm_compiler;

#[cfg(feature = "npu")]
pub mod arm_npu;
#[cfg(feature = "npu")]
pub mod npu;

// 通用模块（始终可用）
pub mod gpu;
pub mod pcie;
pub mod sriov;

// 条件导出
#[cfg(feature = "cuda")]
pub use cuda::{CudaAccelerator, CudaDevicePtr, CudaMemcpyKind, CudaStream};
#[cfg(feature = "cuda")]
pub use cuda_compiler::{CompileOptions, CompiledKernel, CudaJITCompiler};

#[cfg(feature = "rocm")]
pub use rocm::{RocmAccelerator, RocmDevicePtr, RocmStream};
#[cfg(feature = "rocm")]
pub use rocm_compiler::{CompiledAmdGpuKernel, RocmCompileOptions, RocmJITCompiler};

#[cfg(feature = "npu")]
pub use arm_npu::{ArmNpuAccelerator, NpuCapabilities, NpuDevicePtr, NpuVendor};

pub use sriov::{QosConfig, SriovVfManager, VfConfig, VfId, VfMacConfig, VfState, VlanConfig};

/// PCIe 设备地址
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PciAddress {
    pub domain: u16,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

impl PciAddress {
    pub fn new(domain: u16, bus: u8, device: u8, function: u8) -> Self {
        Self {
            domain,
            bus,
            device,
            function,
        }
    }
}

impl std::fmt::Display for PciAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:04x}:{:02x}:{:02x}.{}",
            self.domain, self.bus, self.device, self.function
        )
    }
}

impl std::str::FromStr for PciAddress {
    type Err = PassthroughError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return Err(PassthroughError::InvalidAddress(s.to_string()));
        }

        let domain = u16::from_str_radix(parts[0], 16).map_err(|_| PassthroughError::ParseError)?;
        let bus = u8::from_str_radix(parts[1], 16).map_err(|_| PassthroughError::ParseError)?;

        let dev_func: Vec<&str> = parts[2].split('.').collect();
        if dev_func.len() != 2 {
            return Err(PassthroughError::InvalidAddress(s.to_string()));
        }

        let device =
            u8::from_str_radix(dev_func[0], 16).map_err(|_| PassthroughError::ParseError)?;
        let function =
            u8::from_str_radix(dev_func[1], 16).map_err(|_| PassthroughError::ParseError)?;

        Ok(Self {
            domain,
            bus,
            device,
            function,
        })
    }
}

/// PCIe 设备信息
#[derive(Debug, Clone)]
pub struct PciDeviceInfo {
    pub address: PciAddress,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u32,
    pub subsystem_vendor_id: u16,
    pub subsystem_device_id: u16,
    pub name: String,
}

/// 设备类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    Unknown,
    GpuNvidia,
    GpuAmd,
    GpuIntel,
    GpuMobile, // 移动端集成 GPU
    Npu,
    NetworkCard,
    StorageController,
}

/// 直通错误类型
#[derive(Debug, thiserror::Error)]
pub enum PassthroughError {
    #[error("Invalid PCI address: {0}")]
    InvalidAddress(String),
    #[error("Device not found: {0}")]
    DeviceNotFound(String),
    #[error("Device already in use")]
    DeviceInUse,
    #[error("Permission denied")]
    PermissionDenied,
    #[error("IOMMU not enabled")]
    IommuNotEnabled,
    #[error("Driver binding failed: {0}")]
    DriverBindingFailed(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Parse error")]
    ParseError,
    #[error("Lock poisoned: {0}")]
    LockPoisoned(String),
}

/// 硬件直通管理器
pub struct PassthroughManager {
    devices: HashMap<PciAddress, PciDeviceInfo>,
    attached_devices: HashMap<PciAddress, Box<dyn PassthroughDevice>>,
}

impl PassthroughManager {
    /// 创建新的直通管理器
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            attached_devices: HashMap::new(),
        }
    }

    /// 扫描系统中的 PCIe 设备
    pub fn scan_devices(&mut self) -> Result<(), PassthroughError> {
        #[cfg(target_os = "linux")]
        {
            self.scan_devices_linux()?;
        }

        #[cfg(target_os = "windows")]
        {
            self.scan_devices_windows()?;
        }

        Ok(())
    }

    /// Linux 下扫描 PCIe 设备
    #[cfg(target_os = "linux")]
    fn scan_devices_linux(&mut self) -> Result<(), PassthroughError> {
        use std::fs;
        use std::path::Path;

        let pci_path = Path::new("/sys/bus/pci/devices");
        if !pci_path.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(pci_path)? {
            let entry = entry?;
            let addr_str = entry.file_name().to_string_lossy().to_string();

            if let Ok(address) = PciAddress::from_str(&addr_str) {
                if let Ok(info) = self.read_device_info_linux(&entry.path(), address) {
                    self.devices.insert(address, info);
                }
            }
        }

        Ok(())
    }

    /// 读取 Linux 下的设备信息
    #[cfg(target_os = "linux")]
    fn read_device_info_linux(
        &self,
        path: &std::path::Path,
        address: PciAddress,
    ) -> Result<PciDeviceInfo, PassthroughError> {
        use std::fs;

        let read_hex = |file: &str| -> Result<u32, PassthroughError> {
            let content = fs::read_to_string(path.join(file))?;
            let trimmed = content.trim().trim_start_matches("0x");
            Ok(u32::from_str_radix(trimmed, 16).unwrap_or(0))
        };

        let vendor_id = read_hex("vendor")? as u16;
        let device_id = read_hex("device")? as u16;
        let class_code = read_hex("class")?;
        let subsystem_vendor = read_hex("subsystem_vendor").unwrap_or(0) as u16;
        let subsystem_device = read_hex("subsystem_device").unwrap_or(0) as u16;

        // 尝试读取设备名称
        let name = fs::read_to_string(path.join("label"))
            .or_else(|_| fs::read_to_string(path.join("uevent")))
            .unwrap_or_else(|_| format!("PCI Device {:04x}:{:04x}", vendor_id, device_id))
            .lines()
            .next()
            .unwrap_or("")
            .to_string();

        Ok(PciDeviceInfo {
            address,
            vendor_id,
            device_id,
            class_code,
            subsystem_vendor_id: subsystem_vendor,
            subsystem_device_id: subsystem_device,
            name,
        })
    }

    /// Windows 下扫描 PCIe 设备
    #[cfg(target_os = "windows")]
    fn scan_devices_windows(&mut self) -> Result<(), PassthroughError> {
        // Windows 实现需要使用 SetupAPI 或 WMI
        log::warn!("Windows PCIe device scanning not yet implemented");
        Ok(())
    }

    /// 获取所有设备
    pub fn get_devices(&self) -> &HashMap<PciAddress, PciDeviceInfo> {
        &self.devices
    }

    /// 根据类型筛选设备
    pub fn filter_by_type(&self, device_type: DeviceType) -> Vec<&PciDeviceInfo> {
        self.devices
            .values()
            .filter(|info| self.classify_device(info) == device_type)
            .collect()
    }

    /// 分类设备
    fn classify_device(&self, info: &PciDeviceInfo) -> DeviceType {
        // PCI 类代码: 0x03xxxx 表示显示控制器
        if (info.class_code >> 16) == 0x03 {
            match info.vendor_id {
                0x10DE => DeviceType::GpuNvidia, // NVIDIA
                0x1002 => DeviceType::GpuAmd,    // AMD
                0x8086 => DeviceType::GpuIntel,  // Intel
                _ => DeviceType::GpuMobile,
            }
        } else if (info.class_code >> 16) == 0x12 {
            // 0x12xxxx 可能是加速器/协处理器
            DeviceType::Npu
        } else if (info.class_code >> 16) == 0x02 {
            DeviceType::NetworkCard
        } else if (info.class_code >> 16) == 0x01 {
            DeviceType::StorageController
        } else {
            DeviceType::Unknown
        }
    }

    /// 附加设备到虚拟机
    pub fn attach_device(
        &mut self,
        address: PciAddress,
        device: Box<dyn PassthroughDevice>,
    ) -> Result<(), PassthroughError> {
        if self.attached_devices.contains_key(&address) {
            return Err(PassthroughError::DeviceInUse);
        }

        device.prepare_passthrough()?;
        self.attached_devices.insert(address, device);
        log::info!("Attached device {} to VM", address);
        Ok(())
    }

    /// 分离设备
    pub fn detach_device(&mut self, address: PciAddress) -> Result<(), PassthroughError> {
        if let Some(device) = self.attached_devices.remove(&address) {
            device.cleanup_passthrough()?;
            log::info!("Detached device {} from VM", address);
        }
        Ok(())
    }

    /// 打印设备列表
    pub fn print_devices(&self) {
        println!("\n=== PCIe Devices ===");
        for (addr, info) in &self.devices {
            let dev_type = self.classify_device(info);
            println!(
                "{} - {:04x}:{:04x} - {:?} - {}",
                addr, info.vendor_id, info.device_id, dev_type, info.name
            );
        }
    }
}

impl Default for PassthroughManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 直通设备 trait
pub trait PassthroughDevice: Send + Sync {
    /// 准备设备直通
    fn prepare_passthrough(&self) -> Result<(), PassthroughError>;

    /// 清理直通配置
    fn cleanup_passthrough(&self) -> Result<(), PassthroughError>;

    /// 获取设备信息
    fn get_info(&self) -> &PciDeviceInfo;
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_pci_address_parsing() {
        let addr = PciAddress::from_str("0000:01:00.0").expect("Failed to parse PCI address");
        assert_eq!(addr.domain, 0);
        assert_eq!(addr.bus, 1);
        assert_eq!(addr.device, 0);
        assert_eq!(addr.function, 0);
        assert_eq!(addr.to_string(), "0000:01:00.0");
    }

    #[test]
    fn test_passthrough_manager() {
        let mut manager = PassthroughManager::new();
        let _ = manager.scan_devices();
        manager.print_devices();
    }
}
