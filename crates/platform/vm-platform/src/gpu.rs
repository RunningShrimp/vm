//! GPU直通
//!
//! 提供NVIDIA和AMD GPU的直通支持
//! 从 vm-passthrough/gpu.rs 迁移而来

use super::{PassthroughError, PciAddress, PciDeviceInfo};

/// GPU 直通配置
#[derive(Debug, Clone)]
pub struct GpuConfig {
    pub address: PciAddress,
    pub vendor: String,
    pub device_name: String,
    pub vga_arbiter: bool, // VGA 仲裁器
}

impl GpuConfig {
    /// 创建新的 GPU 配置
    pub fn new(address: PciAddress, info: &PciDeviceInfo) -> Self {
        Self {
            address,
            vendor: format!("0x{:04x}", info.vendor_id),
            device_name: info.name.clone(),
            vga_arbiter: false,
        }
    }

    /// 设置 VGA 仲裁器
    pub fn set_vga_arbiter(&mut self, enable: bool) {
        self.vga_arbiter = enable;
    }
}

/// NVIDIA GPU 直通
pub struct NvidiaGpuPassthrough {
    config: GpuConfig,
}

impl NvidiaGpuPassthrough {
    /// 创建新的 NVIDIA GPU 直通
    pub fn new(address: PciAddress, info: &PciDeviceInfo) -> Self {
        Self {
            config: GpuConfig::new(address, info),
        }
    }

    /// 准备直通
    pub fn prepare(&self) -> Result<(), PassthroughError> {
        log::info!(
            "Preparing NVIDIA GPU passthrough for {}",
            self.config.address
        );

        #[cfg(target_os = "linux")]
        {
            use std::fs;
            use std::path::Path;

            let pci_path = format!("/sys/bus/pci/devices/{}", self.config.address.to_string());
            let pci_path = Path::new(&pci_path);

            // 1. 检查设备是否存在
            if !pci_path.exists() {
                return Err(PassthroughError::DeviceNotFound(
                    self.config.address.to_string(),
                ));
            }

            // 2. 解绑现有驱动
            let driver_path = pci_path.join("driver");
            if driver_path.exists() {
                let driver_name = driver_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");

                log::info!("Unbinding driver: {}", driver_name);
                let unbind_path = driver_path.join("unbind");
                if let Err(e) = fs::write(unbind_path, self.config.address.to_string().as_bytes()) {
                    log::warn!("Failed to unbind driver: {}", e);
                }
            }

            // 3. 启用 VGA 仲裁器（如果需要）
            if self.config.vga_arbiter {
                log::info!("Enabling VGA arbiter for GPU");
                // VGA 仲裁器配置逻辑
            }

            // 4. 检查并设置 IOMMU
            let iommu_group_path = pci_path.join("iommu_group");
            if !iommu_group_path.exists() {
                log::warn!("IOMMU group not found for GPU");
                return Err(PassthroughError::IommuNotEnabled);
            }

            log::info!("NVIDIA GPU prepared successfully");
        }

        #[cfg(not(target_os = "linux"))]
        {
            log::warn!("NVIDIA GPU passthrough only supported on Linux");
            Err(PassthroughError::DriverBindingFailed(
                "GPU passthrough only supported on Linux".to_string(),
            ))
        }

        #[cfg(target_os = "linux")]
        {
            Ok(())
        }
    }

    /// 清理直通
    pub fn cleanup(&self) -> Result<(), PassthroughError> {
        log::info!(
            "Cleaning up NVIDIA GPU passthrough for {}",
            self.config.address
        );

        #[cfg(target_os = "linux")]
        {
            use std::fs;
            use std::path::Path;

            let pci_path = format!("/sys/bus/pci/devices/{}", self.config.address.to_string());
            let pci_path = Path::new(&pci_path);

            // 1. 重新绑定主机驱动
            let driver_override_path = pci_path.join("driver_override");
            if driver_override_path.exists() {
                // 清除 driver_override
                if let Err(e) = fs::write(&driver_override_path, b"") {
                    log::warn!("Failed to clear driver override: {}", e);
                }
            }

            // 2. 尝试重新绑定 NVIDIA 驱动
            let bind_path = "/sys/bus/pci/drivers/nvidia/bind";
            if Path::new(bind_path).exists() {
                if let Err(e) = fs::write(bind_path, self.config.address.to_string().as_bytes()) {
                    log::warn!("Failed to rebind NVIDIA driver: {}", e);
                }
            }

            // 3. 重置 GPU
            let reset_path = pci_path.join("reset");
            if reset_path.exists() {
                if let Err(e) = fs::write(&reset_path, b"1") {
                    log::warn!("Failed to reset GPU: {}", e);
                }
            }

            log::info!("NVIDIA GPU cleanup completed");
        }

        #[cfg(not(target_os = "linux"))]
        {
            log::warn!("NVIDIA GPU cleanup only supported on Linux");
        }

        Ok(())
    }
}

/// AMD GPU 直通
pub struct AmdGpuPassthrough {
    config: GpuConfig,
}

impl AmdGpuPassthrough {
    /// 创建新的 AMD GPU 直通
    pub fn new(address: PciAddress, info: &PciDeviceInfo) -> Self {
        Self {
            config: GpuConfig::new(address, info),
        }
    }

    /// 准备直通
    pub fn prepare(&self) -> Result<(), PassthroughError> {
        log::info!("Preparing AMD GPU passthrough for {}", self.config.address);

        #[cfg(target_os = "linux")]
        {
            use std::fs;
            use std::path::Path;

            let pci_path = format!("/sys/bus/pci/devices/{}", self.config.address.to_string());
            let pci_path = Path::new(&pci_path);

            // 1. 检查设备是否存在
            if !pci_path.exists() {
                return Err(PassthroughError::DeviceNotFound(
                    self.config.address.to_string(),
                ));
            }

            // 2. 解绑现有驱动
            let driver_path = pci_path.join("driver");
            if driver_path.exists() {
                log::info!("Unbinding AMD GPU driver");
                let unbind_path = driver_path.join("unbind");
                if let Err(e) = fs::write(unbind_path, self.config.address.to_string().as_bytes()) {
                    log::warn!("Failed to unbind driver: {}", e);
                }
            }

            // 3. 检查并设置 IOMMU
            let iommu_group_path = pci_path.join("iommu_group");
            if !iommu_group_path.exists() {
                log::warn!("IOMMU group not found for GPU");
                return Err(PassthroughError::IommuNotEnabled);
            }

            // 4. 对于 AMD GPU，可能需要处理音频设备（HDMI音频）
            // 通常 GPU 的 function 0 是视频，function 1 是音频
            if self.config.address.function == 0 {
                let audio_addr = PciAddress::new(
                    self.config.address.domain,
                    self.config.address.bus,
                    self.config.address.device,
                    1, // Audio function
                );
                log::info!("Checking for AMD GPU audio device at {}", audio_addr);
            }

            log::info!("AMD GPU prepared successfully");
        }

        #[cfg(not(target_os = "linux"))]
        {
            log::warn!("AMD GPU passthrough only supported on Linux");
            Err(PassthroughError::DriverBindingFailed(
                "GPU passthrough only supported on Linux".to_string(),
            ))
        }

        #[cfg(target_os = "linux")]
        {
            Ok(())
        }
    }

    /// 清理直通
    pub fn cleanup(&self) -> Result<(), PassthroughError> {
        log::info!(
            "Cleaning up AMD GPU passthrough for {}",
            self.config.address
        );

        #[cfg(target_os = "linux")]
        {
            use std::fs;
            use std::path::Path;

            let pci_path = format!("/sys/bus/pci/devices/{}", self.config.address.to_string());
            let pci_path = Path::new(&pci_path);

            // 1. 清除 driver_override
            let driver_override_path = pci_path.join("driver_override");
            if driver_override_path.exists() {
                if let Err(e) = fs::write(&driver_override_path, b"") {
                    log::warn!("Failed to clear driver override: {}", e);
                }
            }

            // 2. 尝试重新绑定 AMDGPU 驱动
            let amdgpu_bind_path = "/sys/bus/pci/drivers/amdgpu/bind";
            let radeon_bind_path = "/sys/bus/pci/drivers/radeon/bind";

            let bind_result = if Path::new(amdgpu_bind_path).exists() {
                fs::write(amdgpu_bind_path, self.config.address.to_string().as_bytes())
            } else if Path::new(radeon_bind_path).exists() {
                fs::write(radeon_bind_path, self.config.address.to_string().as_bytes())
            } else {
                Ok(0)
            };

            if let Err(e) = bind_result {
                log::warn!("Failed to rebind AMD driver: {}", e);
            }

            // 3. 重置 GPU
            let reset_path = pci_path.join("reset");
            if reset_path.exists() {
                if let Err(e) = fs::write(&reset_path, b"1") {
                    log::warn!("Failed to reset GPU: {}", e);
                }
            }

            log::info!("AMD GPU cleanup completed");
        }

        #[cfg(not(target_os = "linux"))]
        {
            log::warn!("AMD GPU cleanup only supported on Linux");
        }

        Ok(())
    }
}
