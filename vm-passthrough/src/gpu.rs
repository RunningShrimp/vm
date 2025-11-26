//! GPU 加速和直通支持
//!
//! 支持 NVIDIA、AMD 和移动端 GPU

use super::{PciAddress, PciDeviceInfo, PassthroughDevice, PassthroughError};
use std::collections::HashMap;

/// GPU 厂商
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Qualcomm,
    Mali,
    PowerVR,
    Unknown,
}

/// GPU 架构
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuArchitecture {
    // NVIDIA
    Maxwell,
    Pascal,
    Volta,
    Turing,
    Ampere,
    Ada,
    Hopper,
    Blackwell,
    // AMD
    GCN,
    RDNA,
    RDNA2,
    RDNA3,
    CDNA,
    CDNA2,
    CDNA3,
    // 移动端
    Adreno,
    Mali,
    PowerVR,
    Unknown,
}

/// GPU 信息
#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub pci_info: PciDeviceInfo,
    pub vendor: GpuVendor,
    pub architecture: GpuArchitecture,
    pub name: String,
    pub vram_size_mb: usize,
    pub compute_capability: Option<(u32, u32)>,  // NVIDIA CUDA compute capability
    pub supports_cuda: bool,
    pub supports_rocm: bool,
    pub supports_vulkan: bool,
    pub supports_opencl: bool,
}

/// NVIDIA GPU 设备
pub struct NvidiaGpu {
    pub info: GpuInfo,
    pub address: PciAddress,
    pub cuda_version: Option<String>,
    pub driver_version: Option<String>,
}

impl NvidiaGpu {
    /// 创建新的 NVIDIA GPU
    pub fn new(pci_info: PciDeviceInfo, address: PciAddress) -> Self {
        let architecture = Self::detect_architecture(pci_info.device_id);
        let compute_capability = Self::get_compute_capability(pci_info.device_id);
        
        let info = GpuInfo {
            pci_info: pci_info.clone(),
            vendor: GpuVendor::Nvidia,
            architecture,
            name: pci_info.name.clone(),
            vram_size_mb: 0,  // 需要通过 NVML 或 nvidia-smi 获取
            compute_capability,
            supports_cuda: true,
            supports_rocm: false,
            supports_vulkan: true,
            supports_opencl: true,
        };

        Self {
            info,
            address,
            cuda_version: None,
            driver_version: None,
        }
    }

    /// 检测 NVIDIA GPU 架构
    fn detect_architecture(device_id: u16) -> GpuArchitecture {
        match device_id {
            // Blackwell (B100, B200)
            0x2300..=0x23FF => GpuArchitecture::Blackwell,
            // Hopper (H100, H200)
            0x2330..=0x233F => GpuArchitecture::Hopper,
            // Ada Lovelace (RTX 40 series)
            0x2684..=0x2704 => GpuArchitecture::Ada,
            // Ampere (RTX 30 series, A100)
            0x2200..=0x2300 => GpuArchitecture::Ampere,
            // Turing (RTX 20 series)
            0x1E00..=0x1FFF => GpuArchitecture::Turing,
            // Volta (V100)
            0x1D00..=0x1DFF => GpuArchitecture::Volta,
            // Pascal (GTX 10 series)
            0x1B00..=0x1CFF => GpuArchitecture::Pascal,
            // Maxwell
            0x1300..=0x1400 => GpuArchitecture::Maxwell,
            _ => GpuArchitecture::Unknown,
        }
    }

    /// 获取 CUDA 计算能力
    fn get_compute_capability(device_id: u16) -> Option<(u32, u32)> {
        match device_id {
            0x2300..=0x23FF => Some((10, 0)),  // Blackwell: 10.0
            0x2330..=0x233F => Some((9, 0)),   // Hopper: 9.0
            0x2684..=0x2704 => Some((8, 9)),   // Ada: 8.9
            0x2200..=0x2300 => Some((8, 6)),   // Ampere: 8.6
            0x1E00..=0x1FFF => Some((7, 5)),   // Turing: 7.5
            0x1D00..=0x1DFF => Some((7, 0)),   // Volta: 7.0
            0x1B00..=0x1CFF => Some((6, 1)),   // Pascal: 6.1
            0x1300..=0x1400 => Some((5, 2)),   // Maxwell: 5.2
            _ => None,
        }
    }

    /// 检测 CUDA 版本
    #[cfg(target_os = "linux")]
    pub fn detect_cuda_version(&mut self) -> Result<(), PassthroughError> {
        use std::process::Command;

        // 尝试运行 nvcc --version
        if let Ok(output) = Command::new("nvcc").arg("--version").output() {
            if output.status.success() {
                let version_str = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = version_str.lines().find(|l| l.contains("release")) {
                    self.cuda_version = Some(line.to_string());
                }
            }
        }

        // 尝试运行 nvidia-smi
        if let Ok(output) = Command::new("nvidia-smi").arg("--query-gpu=driver_version").arg("--format=csv,noheader").output() {
            if output.status.success() {
                self.driver_version = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
        }

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn detect_cuda_version(&mut self) -> Result<(), PassthroughError> {
        Ok(())
    }

    /// 启用 GPU 直通模式
    pub fn enable_passthrough_mode(&self) -> Result<(), PassthroughError> {
        log::info!("Enabling NVIDIA GPU passthrough for {}", self.address.to_string());
        
        // 对于 NVIDIA GPU，需要隐藏虚拟化环境
        // 否则驱动会检测到并拒绝工作（Error 43）
        log::warn!("NVIDIA GPU passthrough requires hypervisor hiding");
        log::info!("  - Use 'kvm=off' or similar flags");
        log::info!("  - May need vendor_id spoofing");
        
        Ok(())
    }

    /// 获取 GPU 信息
    pub fn get_info(&self) -> &GpuInfo {
        &self.info
    }
}

impl PassthroughDevice for NvidiaGpu {
    fn prepare_passthrough(&self) -> Result<(), PassthroughError> {
        self.enable_passthrough_mode()
    }

    fn cleanup_passthrough(&self) -> Result<(), PassthroughError> {
        log::info!("Cleaning up NVIDIA GPU passthrough");
        Ok(())
    }

    fn get_info(&self) -> &PciDeviceInfo {
        &self.info.pci_info
    }
}

/// AMD GPU 设备
pub struct AmdGpu {
    pub info: GpuInfo,
    pub address: PciAddress,
    pub rocm_version: Option<String>,
}

impl AmdGpu {
    /// 创建新的 AMD GPU
    pub fn new(pci_info: PciDeviceInfo, address: PciAddress) -> Self {
        let architecture = Self::detect_architecture(pci_info.device_id);
        
        let info = GpuInfo {
            pci_info: pci_info.clone(),
            vendor: GpuVendor::Amd,
            architecture,
            name: pci_info.name.clone(),
            vram_size_mb: 0,
            compute_capability: None,
            supports_cuda: false,
            supports_rocm: true,
            supports_vulkan: true,
            supports_opencl: true,
        };

        Self {
            info,
            address,
            rocm_version: None,
        }
    }

    /// 检测 AMD GPU 架构
    fn detect_architecture(device_id: u16) -> GpuArchitecture {
        match device_id {
            // CDNA 3 (MI300)
            0x7480..=0x748F => GpuArchitecture::CDNA3,
            // CDNA 2 (MI200)
            0x7400..=0x740F => GpuArchitecture::CDNA2,
            // RDNA 3 (RX 7000 series)
            0x7410..=0x74FF => GpuArchitecture::RDNA3,
            // CDNA (MI100)
            0x7380..=0x738F => GpuArchitecture::CDNA,
            // RDNA (RX 5000 series)
            0x7310..=0x731F => GpuArchitecture::RDNA,
            // RDNA 2 (RX 6000 series)
            0x7300..=0x730F | 0x7320..=0x737F | 0x7390..=0x73FF => GpuArchitecture::RDNA2,
            // GCN
            _ => GpuArchitecture::GCN,
        }
    }

    /// 检测 ROCm 版本
    #[cfg(target_os = "linux")]
    pub fn detect_rocm_version(&mut self) -> Result<(), PassthroughError> {
        use std::process::Command;

        if let Ok(output) = Command::new("rocm-smi").arg("--version").output() {
            if output.status.success() {
                self.rocm_version = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
        }

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn detect_rocm_version(&mut self) -> Result<(), PassthroughError> {
        Ok(())
    }

    /// 启用 GPU 直通模式
    pub fn enable_passthrough_mode(&self) -> Result<(), PassthroughError> {
        log::info!("Enabling AMD GPU passthrough for {}", self.address.to_string());
        
        // AMD GPU 对虚拟化环境更友好
        log::info!("  - AMD GPU passthrough typically works without special configuration");
        log::info!("  - Ensure IOMMU is enabled and reset bug is handled");
        
        Ok(())
    }
}

impl PassthroughDevice for AmdGpu {
    fn prepare_passthrough(&self) -> Result<(), PassthroughError> {
        self.enable_passthrough_mode()
    }

    fn cleanup_passthrough(&self) -> Result<(), PassthroughError> {
        log::info!("Cleaning up AMD GPU passthrough");
        Ok(())
    }

    fn get_info(&self) -> &PciDeviceInfo {
        &self.info.pci_info
    }
}

/// GPU 管理器
pub struct GpuManager {
    gpus: HashMap<PciAddress, Box<dyn PassthroughDevice>>,
}

impl GpuManager {
    /// 创建新的 GPU 管理器
    pub fn new() -> Self {
        Self {
            gpus: HashMap::new(),
        }
    }

    /// 添加 NVIDIA GPU
    pub fn add_nvidia_gpu(&mut self, pci_info: PciDeviceInfo, address: PciAddress) {
        let gpu = NvidiaGpu::new(pci_info, address);
        self.gpus.insert(address, Box::new(gpu));
    }

    /// 添加 AMD GPU
    pub fn add_amd_gpu(&mut self, pci_info: PciDeviceInfo, address: PciAddress) {
        let gpu = AmdGpu::new(pci_info, address);
        self.gpus.insert(address, Box::new(gpu));
    }

    /// 获取所有 GPU
    pub fn get_gpus(&self) -> &HashMap<PciAddress, Box<dyn PassthroughDevice>> {
        &self.gpus
    }

    /// 打印 GPU 信息
    pub fn print_gpus(&self) {
        println!("\n=== GPU Devices ===");
        for (addr, _gpu) in &self.gpus {
            println!("GPU at {}", addr.to_string());
        }
    }
}

impl Default for GpuManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 移动端 GPU 设备
pub struct MobileGpu {
    pub info: GpuInfo,
    pub soc_vendor: SocVendor,
    pub gpu_name: String,
}

/// SoC 厂商
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocVendor {
    Qualcomm,
    HiSilicon,
    MediaTek,
    Samsung,
    Apple,
    Unknown,
}

impl MobileGpu {
    /// 创建新的移动端 GPU
    pub fn new(soc_vendor: SocVendor, gpu_name: String) -> Self {
        let (vendor, architecture) = match soc_vendor {
            SocVendor::Qualcomm => (GpuVendor::Qualcomm, GpuArchitecture::Adreno),
            SocVendor::HiSilicon | SocVendor::MediaTek => (GpuVendor::Mali, GpuArchitecture::Mali),
            SocVendor::Samsung => (GpuVendor::Mali, GpuArchitecture::Mali),
            SocVendor::Apple => (GpuVendor::Unknown, GpuArchitecture::Unknown),
            SocVendor::Unknown => (GpuVendor::Unknown, GpuArchitecture::Unknown),
        };

        let info = GpuInfo {
            pci_info: PciDeviceInfo {
                address: PciAddress::new(0, 0, 0, 0),
                vendor_id: 0,
                device_id: 0,
                class_code: 0,
                subsystem_vendor_id: 0,
                subsystem_device_id: 0,
                name: gpu_name.clone(),
            },
            vendor,
            architecture,
            name: gpu_name.clone(),
            vram_size_mb: 0,  // 移动端共享内存
            compute_capability: None,
            supports_cuda: false,
            supports_rocm: false,
            supports_vulkan: true,
            supports_opencl: true,
        };

        Self {
            info,
            soc_vendor,
            gpu_name,
        }
    }

    /// 检测移动端 GPU
    #[cfg(target_os = "android")]
    pub fn detect() -> Option<Self> {
        use std::process::Command;

        // 尝试通过 getprop 获取 SoC 信息
        if let Ok(output) = Command::new("getprop").arg("ro.hardware").output() {
            let hardware = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
            
            let soc_vendor = if hardware.contains("qcom") || hardware.contains("qualcomm") {
                SocVendor::Qualcomm
            } else if hardware.contains("kirin") || hardware.contains("hisi") {
                SocVendor::HiSilicon
            } else if hardware.contains("mt") || hardware.contains("mediatek") {
                SocVendor::MediaTek
            } else if hardware.contains("exynos") {
                SocVendor::Samsung
            } else {
                SocVendor::Unknown
            };

            let gpu_name = match soc_vendor {
                SocVendor::Qualcomm => "Adreno GPU".to_string(),
                SocVendor::HiSilicon => "Mali GPU".to_string(),
                SocVendor::MediaTek => "Mali GPU".to_string(),
                SocVendor::Samsung => "Mali GPU".to_string(),
                _ => "Unknown GPU".to_string(),
            };

            return Some(Self::new(soc_vendor, gpu_name));
        }

        None
    }

    #[cfg(not(target_os = "android"))]
    pub fn detect() -> Option<Self> {
        None
    }

    /// 启用 GPU 加速
    pub fn enable_acceleration(&self) -> Result<(), PassthroughError> {
        log::info!("Enabling mobile GPU acceleration: {}", self.gpu_name);
        
        match self.soc_vendor {
            SocVendor::Qualcomm => {
                log::info!("  - Using Adreno GPU via Vulkan/OpenCL");
                log::info!("  - Supports hardware-accelerated video decode/encode");
            }
            SocVendor::HiSilicon => {
                log::info!("  - Using Mali GPU via Vulkan/OpenCL");
                log::info!("  - GPU Turbo optimization available");
            }
            SocVendor::MediaTek => {
                log::info!("  - Using Mali/Immortalis GPU via Vulkan/OpenCL");
                log::info!("  - HyperEngine optimization available");
            }
            SocVendor::Samsung => {
                log::info!("  - Using Mali GPU via Vulkan/OpenCL");
            }
            _ => {}
        }
        
        Ok(())
    }

    /// 获取 Vulkan 支持信息
    pub fn get_vulkan_info(&self) -> VulkanInfo {
        let version = match self.soc_vendor {
            SocVendor::Qualcomm => "1.3",  // Adreno 最新支持 Vulkan 1.3
            SocVendor::HiSilicon | SocVendor::MediaTek | SocVendor::Samsung => "1.3",
            _ => "1.0",
        };

        VulkanInfo {
            supported: true,
            version: version.to_string(),
            extensions: vec![
                "VK_KHR_swapchain".to_string(),
                "VK_KHR_surface".to_string(),
            ],
        }
    }
}

/// Vulkan 信息
#[derive(Debug, Clone)]
pub struct VulkanInfo {
    pub supported: bool,
    pub version: String,
    pub extensions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nvidia_architecture_detection() {
        assert_eq!(NvidiaGpu::detect_architecture(0x2684), GpuArchitecture::Ada);
        assert_eq!(NvidiaGpu::detect_architecture(0x2204), GpuArchitecture::Ampere);
    }

    #[test]
    fn test_compute_capability() {
        assert_eq!(NvidiaGpu::get_compute_capability(0x2684), Some((8, 9)));
        assert_eq!(NvidiaGpu::get_compute_capability(0x2204), Some((8, 6)));
    }
}
