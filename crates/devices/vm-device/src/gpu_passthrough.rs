use vm_passthrough::pcie::VfioDevice;
use vm_passthrough::{PassthroughDevice, PassthroughError, PciAddress, PciDeviceInfo};

/// GPU 直通模式
pub struct GpuPassthrough {
    vfio_device: VfioDevice,
    gpu_info: GpuInfo,
}

/// GPU 信息
#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub vendor: GpuVendor,
    pub model: String,
    pub pci_address: PciAddress,
    pub vram_size: u64,
    pub driver: String,
}

/// GPU 厂商
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Other,
}

impl GpuPassthrough {
    /// 创建新的 GPU 直通实例
    pub fn new(address: PciAddress, info: PciDeviceInfo) -> Result<Self, PassthroughError> {
        let vfio_device = VfioDevice::new(address, info.clone())?;
        let gpu_info = Self::detect_gpu_info(&info)?;

        Ok(Self {
            vfio_device,
            gpu_info,
        })
    }

    /// 检测 GPU 信息
    fn detect_gpu_info(info: &PciDeviceInfo) -> Result<GpuInfo, PassthroughError> {
        let vendor = match info.vendor_id {
            0x10DE => GpuVendor::Nvidia,
            0x1002 => GpuVendor::Amd,
            0x8086 => GpuVendor::Intel,
            _ => GpuVendor::Other,
        };

        // 尝试读取 GPU 型号名称
        let model = Self::read_gpu_model(&info.address)
            .unwrap_or_else(|_| format!("{:04x}:{:04x}", info.vendor_id, info.device_id));

        // 尝试读取显存大小
        let vram_size = Self::read_vram_size(&info.address).unwrap_or(0);

        // 检测当前驱动
        let driver =
            Self::read_current_driver(&info.address).unwrap_or_else(|_| "unknown".to_string());

        Ok(GpuInfo {
            vendor,
            model,
            pci_address: info.address,
            vram_size,
            driver,
        })
    }

    /// 读取 GPU 型号名称
    #[cfg(target_os = "linux")]
    fn read_gpu_model(address: &PciAddress) -> Result<String, PassthroughError> {
        let device_path = format!("/sys/bus/pci/devices/{}", address.to_string());

        // 尝试从多个位置读取设备名称
        let label_path = PathBuf::from(&device_path).join("label");
        if label_path.exists() {
            if let Ok(name) = fs::read_to_string(&label_path) {
                return Ok(name.trim().to_string());
            }
        }

        // 尝试读取 modalias
        let modalias_path = PathBuf::from(&device_path).join("modalias");
        if modalias_path.exists() {
            if let Ok(modalias) = fs::read_to_string(&modalias_path) {
                return Ok(modalias.trim().to_string());
            }
        }

        Err(PassthroughError::DeviceNotFound(
            "GPU model not found".to_string(),
        ))
    }

    #[cfg(not(target_os = "linux"))]
    fn read_gpu_model(_address: &PciAddress) -> Result<String, PassthroughError> {
        Err(PassthroughError::DeviceNotFound(
            "Not supported on this platform".to_string(),
        ))
    }

    /// 读取显存大小
    #[cfg(target_os = "linux")]
    fn read_vram_size(address: &PciAddress) -> Result<u64, PassthroughError> {
        let device_path = format!("/sys/bus/pci/devices/{}", address.to_string());

        // 尝试读取 BAR0 的大小（通常是显存）
        let resource_path = PathBuf::from(&device_path).join("resource");
        if let Ok(content) = fs::read_to_string(&resource_path) {
            if let Some(first_line) = content.lines().next() {
                let parts: Vec<&str> = first_line.split_whitespace().collect();
                if parts.len() >= 3 {
                    if let Ok(start) = u64::from_str_radix(parts[0].trim_start_matches("0x"), 16) {
                        if let Ok(end) = u64::from_str_radix(parts[1].trim_start_matches("0x"), 16)
                        {
                            return Ok(end - start);
                        }
                    }
                }
            }
        }

        Ok(0)
    }

    #[cfg(not(target_os = "linux"))]
    fn read_vram_size(_address: &PciAddress) -> Result<u64, PassthroughError> {
        Ok(0)
    }

    /// 读取当前驱动
    #[cfg(target_os = "linux")]
    fn read_current_driver(address: &PciAddress) -> Result<String, PassthroughError> {
        let device_path = format!("/sys/bus/pci/devices/{}", address.to_string());
        let driver_link = PathBuf::from(&device_path).join("driver");

        if driver_link.exists() {
            if let Ok(target) = fs::read_link(&driver_link) {
                if let Some(driver_name) = target.file_name() {
                    return Ok(driver_name.to_string_lossy().to_string());
                }
            }
        }

        Err(PassthroughError::DeviceNotFound(
            "No driver bound".to_string(),
        ))
    }

    #[cfg(not(target_os = "linux"))]
    fn read_current_driver(_address: &PciAddress) -> Result<String, PassthroughError> {
        Err(PassthroughError::DeviceNotFound(
            "Not supported on this platform".to_string(),
        ))
    }

    /// 检查是否可用于直通
    pub fn is_available(&self) -> bool {
        // 检查 IOMMU 是否启用
        if !VfioDevice::check_iommu_enabled() {
            return false;
        }

        // 检查设备是否已经被其他驱动占用
        if self.gpu_info.driver != "vfio-pci" && self.gpu_info.driver != "unknown" {
            return false;
        }

        true
    }

    /// 准备直通
    pub fn prepare(&mut self) -> Result<(), PassthroughError> {
        // 绑定到 vfio-pci 驱动
        self.vfio_device.bind_vfio_driver()?;

        // 打开 VFIO 设备
        self.vfio_device.open_vfio()?;

        log::info!("GPU passthrough prepared: {:?}", self.gpu_info);
        Ok(())
    }

    /// 获取 GPU 信息
    pub fn get_info(&self) -> &GpuInfo {
        &self.gpu_info
    }

    /// 获取 VFIO 设备
    pub fn get_vfio_device(&self) -> &VfioDevice {
        &self.vfio_device
    }
}

impl PassthroughDevice for GpuPassthrough {
    fn prepare_passthrough(&self) -> Result<(), PassthroughError> {
        log::info!(
            "Preparing GPU passthrough for {:?}",
            self.gpu_info.pci_address
        );
        Ok(())
    }

    fn cleanup_passthrough(&self) -> Result<(), PassthroughError> {
        log::info!(
            "Cleaning up GPU passthrough for {:?}",
            self.gpu_info.pci_address
        );
        Ok(())
    }

    fn get_info(&self) -> &PciDeviceInfo {
        &self.vfio_device.info
    }
}

/// 扫描可用的 GPU 设备
pub fn scan_available_gpus() -> Vec<GpuInfo> {
    let gpus = Vec::new();

    #[cfg(target_os = "linux")]
    {
        let pci_path = Path::new("/sys/bus/pci/devices");
        if !pci_path.exists() {
            return gpus;
        }

        if let Ok(entries) = fs::read_dir(pci_path) {
            for entry in entries.flatten() {
                let addr_str = entry.file_name().to_string_lossy().to_string();

                if let Ok(address) = PciAddress::from_str(&addr_str) {
                    // 读取设备类别
                    let class_path = entry.path().join("class");
                    if let Ok(class_str) = fs::read_to_string(&class_path) {
                        let class_code =
                            u32::from_str_radix(class_str.trim().trim_start_matches("0x"), 16)
                                .unwrap_or(0);

                        // 检查是否是显示控制器 (0x03xxxx)
                        if (class_code >> 16) == 0x03 {
                            // 读取厂商和设备 ID
                            let vendor_path = entry.path().join("vendor");
                            let device_path = entry.path().join("device");

                            if let (Ok(vendor_str), Ok(device_str)) = (
                                fs::read_to_string(&vendor_path),
                                fs::read_to_string(&device_path),
                            ) {
                                let vendor_id = u16::from_str_radix(
                                    vendor_str.trim().trim_start_matches("0x"),
                                    16,
                                )
                                .unwrap_or(0);

                                let device_id = u16::from_str_radix(
                                    device_str.trim().trim_start_matches("0x"),
                                    16,
                                )
                                .unwrap_or(0);

                                let info = PciDeviceInfo {
                                    address,
                                    vendor_id,
                                    device_id,
                                    class_code,
                                    subsystem_vendor_id: 0,
                                    subsystem_device_id: 0,
                                    name: String::new(),
                                };

                                if let Ok(gpu_info) = GpuPassthrough::detect_gpu_info(&info) {
                                    gpus.push(gpu_info);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    gpus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_gpus() {
        let gpus = scan_available_gpus();
        println!("Found {} GPU(s):", gpus.len());
        for gpu in gpus {
            println!(
                "  - {:?} {} at {}",
                gpu.vendor,
                gpu.model,
                gpu.pci_address.to_string()
            );
            println!("    VRAM: {} MB", gpu.vram_size / 1024 / 1024);
            println!("    Driver: {}", gpu.driver);
        }
    }
}
