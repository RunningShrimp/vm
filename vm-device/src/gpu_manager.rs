///! 统一的 GPU 管理器
///!
///! 支持 GPU Passthrough、Mediated Device Passthrough 和 WGPU，
///! 提供优先级降级和用户选择功能

pub mod passthrough;
pub mod mdev;
pub mod wgpu_backend;

pub use passthrough::{GpuPassthrough, GpuInfo, GpuVendor};
pub use mdev::{GpuMdev, MdevType, MdevConfig};
pub use wgpu_backend::{WgpuBackend, GpuStats};
use crate::gpu_virt::GpuBackend as GpuBackendTrait;

/// GPU 模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuMode {
    /// GPU 直通（最高优先级）
    Passthrough,
    /// Mediated Device 直通（中等优先级）
    Mdev,
    /// WGPU 虚拟化（最低优先级）
    Wgpu,
}

impl GpuMode {
    /// 获取优先级（数值越小优先级越高）
    pub fn priority(&self) -> u8 {
        match self {
            GpuMode::Passthrough => 1,
            GpuMode::Mdev => 2,
            GpuMode::Wgpu => 3,
        }
    }

    /// 获取模式名称
    pub fn name(&self) -> &str {
        match self {
            GpuMode::Passthrough => "GPU Passthrough",
            GpuMode::Mdev => "Mediated Device Passthrough",
            GpuMode::Wgpu => "WGPU Virtualization",
        }
    }

    /// 获取模式描述
    pub fn description(&self) -> &str {
        match self {
            GpuMode::Passthrough => "Direct GPU passthrough with full hardware access",
            GpuMode::Mdev => "Mediated device passthrough (Intel GVT-g, NVIDIA vGPU, etc.)",
            GpuMode::Wgpu => "Software-based GPU virtualization using WGPU",
        }
    }
}

/// GPU 后端实例
pub enum GpuBackend {
    Passthrough(GpuPassthrough),
    Mdev(GpuMdev),
    Wgpu(WgpuBackend),
}

impl GpuBackend {
    /// 获取模式
    pub fn mode(&self) -> GpuMode {
        match self {
            GpuBackend::Passthrough(_) => GpuMode::Passthrough,
            GpuBackend::Mdev(_) => GpuMode::Mdev,
            GpuBackend::Wgpu(_) => GpuMode::Wgpu,
        }
    }

    /// 获取名称
    pub fn name(&self) -> String {
        match self {
            GpuBackend::Passthrough(pt) => {
                format!("{:?} GPU at {}", pt.get_info().vendor, pt.get_info().pci_address.to_string())
            }
            GpuBackend::Mdev(mdev) => {
                format!("mdev {:?}", mdev.get_type())
            }
            GpuBackend::Wgpu(_) => {
                "WGPU Virtual GPU".to_string()
            }
        }
    }
}

/// 统一的 GPU 管理器
pub struct UnifiedGpuManager {
    /// 可用的后端列表
    available_backends: Vec<GpuBackend>,
    /// 当前选中的后端
    selected_backend: Option<usize>,
    /// 用户偏好模式
    preferred_mode: Option<GpuMode>,
    /// 是否启用自动降级
    auto_fallback: bool,
}

impl UnifiedGpuManager {
    /// 创建新的 GPU 管理器
    pub fn new() -> Self {
        Self {
            available_backends: Vec::new(),
            selected_backend: None,
            preferred_mode: None,
            auto_fallback: true,
        }
    }

    /// 设置用户偏好模式
    pub fn set_preferred_mode(&mut self, mode: GpuMode) {
        self.preferred_mode = Some(mode);
    }

    /// 设置是否启用自动降级
    pub fn set_auto_fallback(&mut self, enabled: bool) {
        self.auto_fallback = enabled;
    }

    /// 扫描所有可用的 GPU 后端
    pub fn scan_backends(&mut self) -> Result<(), String> {
        self.available_backends.clear();

        // 1. 扫描 GPU Passthrough
        let passthrough_gpus = passthrough::scan_available_gpus();
        for gpu_info in passthrough_gpus {
            // 尝试创建 passthrough 实例
            if let Ok(info) = self.create_pci_device_info(&gpu_info) {
                if let Ok(pt) = GpuPassthrough::new(gpu_info.pci_address, info) {
                    if pt.is_available() {
                        self.available_backends.push(GpuBackend::Passthrough(pt));
                        log::info!("Found GPU Passthrough: {:?} at {}", 
                            gpu_info.vendor, gpu_info.pci_address.to_string());
                    }
                }
            }
        }

        // 2. 扫描 Mediated Device
        let mdev_gpus = mdev::scan_mdev_capable_gpus();
        for (address, configs) in mdev_gpus {
            for config in configs {
                if config.available_instances > 0 {
                    let mdev = GpuMdev::new(address, config.mdev_type, config.type_id.clone());
                    if mdev.is_available() {
                        self.available_backends.push(GpuBackend::Mdev(mdev));
                        log::info!("Found mdev GPU: {} at {}", config.name, address.to_string());
                    }
                }
            }
        }

        // 3. WGPU 总是可用（作为后备）
        let wgpu = WgpuBackend::new();
        self.available_backends.push(GpuBackend::Wgpu(wgpu));
        log::info!("WGPU backend available");

        Ok(())
    }

    /// 创建 PCI 设备信息（辅助函数）
    fn create_pci_device_info(&self, gpu_info: &GpuInfo) -> Result<vm_passthrough::PciDeviceInfo, String> {
        // 从 GpuInfo 创建 PciDeviceInfo
        let vendor_id = match gpu_info.vendor {
            GpuVendor::Nvidia => 0x10DE,
            GpuVendor::Amd => 0x1002,
            GpuVendor::Intel => 0x8086,
            GpuVendor::Other => 0x0000,
        };

        Ok(vm_passthrough::PciDeviceInfo {
            address: gpu_info.pci_address,
            vendor_id,
            device_id: 0, // 需要从实际设备读取
            class_code: 0x030000, // VGA compatible controller
            subsystem_vendor_id: 0,
            subsystem_device_id: 0,
            name: gpu_info.model.clone(),
        })
    }

    /// 自动选择最佳后端
    pub fn auto_select(&mut self) -> Result<(), String> {
        if self.available_backends.is_empty() {
            return Err("No GPU backends available".to_string());
        }

        // 如果用户指定了偏好模式，优先选择该模式
        if let Some(preferred) = self.preferred_mode {
            for (i, backend) in self.available_backends.iter().enumerate() {
                if backend.mode() == preferred {
                    self.selected_backend = Some(i);
                    log::info!("Selected preferred GPU mode: {}", preferred.name());
                    return Ok(());
                }
            }

            // 如果偏好模式不可用且启用了自动降级
            if !self.auto_fallback {
                return Err(format!("Preferred GPU mode {} not available", preferred.name()));
            }

            log::warn!("Preferred GPU mode {} not available, falling back", preferred.name());
        }

        // 按优先级选择（Passthrough > Mdev > WGPU）
        let mut best_backend = 0;
        let mut best_priority = u8::MAX;

        for (i, backend) in self.available_backends.iter().enumerate() {
            let priority = backend.mode().priority();
            if priority < best_priority {
                best_priority = priority;
                best_backend = i;
            }
        }

        self.selected_backend = Some(best_backend);
        let selected = &self.available_backends[best_backend];
        log::info!("Auto-selected GPU backend: {} ({})", selected.name(), selected.mode().name());

        Ok(())
    }

    /// 手动选择后端
    pub fn select_by_mode(&mut self, mode: GpuMode) -> Result<(), String> {
        for (i, backend) in self.available_backends.iter().enumerate() {
            if backend.mode() == mode {
                self.selected_backend = Some(i);
                log::info!("Manually selected GPU mode: {}", mode.name());
                return Ok(());
            }
        }

        Err(format!("GPU mode {} not available", mode.name()))
    }

    /// 手动选择后端（通过索引）
    pub fn select_by_index(&mut self, index: usize) -> Result<(), String> {
        if index >= self.available_backends.len() {
            return Err(format!("Invalid backend index: {}", index));
        }

        self.selected_backend = Some(index);
        let selected = &self.available_backends[index];
        log::info!("Manually selected GPU backend: {}", selected.name());

        Ok(())
    }

    /// 获取当前选中的后端
    pub fn get_selected_backend(&self) -> Option<&GpuBackend> {
        self.selected_backend.map(|i| &self.available_backends[i])
    }

    /// 获取当前选中的后端（可变引用）
    pub fn get_selected_backend_mut(&mut self) -> Option<&mut GpuBackend> {
        self.selected_backend.map(|i| &mut self.available_backends[i])
    }

    /// 获取所有可用的后端
    pub fn get_available_backends(&self) -> &[GpuBackend] {
        &self.available_backends
    }

    /// 初始化选中的后端
    pub fn initialize_selected(&mut self) -> Result<(), String> {
        if let Some(backend) = self.get_selected_backend_mut() {
            match backend {
                GpuBackend::Passthrough(pt) => {
                    pt.prepare().map_err(|e| e.to_string())?;
                    log::info!("Initialized GPU Passthrough");
                }
                GpuBackend::Mdev(mdev) => {
                    mdev.create().map_err(|e| e.to_string())?;
                    log::info!("Initialized mdev GPU");
                }
                GpuBackend::Wgpu(wgpu) => {
                    GpuBackendTrait::init(wgpu).map_err(|e| e.to_string())?;
                    log::info!("Initialized WGPU backend");
                }
            }
            Ok(())
        } else {
            Err("No backend selected".to_string())
        }
    }

    /// 打印可用的后端列表
    pub fn print_available_backends(&self) {
        println!("\n=== Available GPU Backends ===");
        for (i, backend) in self.available_backends.iter().enumerate() {
            let selected = if Some(i) == self.selected_backend { " [SELECTED]" } else { "" };
            println!("  {}. {} - {}{}", 
                i, 
                backend.mode().name(), 
                backend.name(),
                selected
            );
        }

        if let Some(preferred) = self.preferred_mode {
            println!("\nPreferred mode: {}", preferred.name());
        }
        println!("Auto fallback: {}", if self.auto_fallback { "enabled" } else { "disabled" });
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> Option<String> {
        if let Some(backend) = self.get_selected_backend() {
            match backend {
                GpuBackend::Passthrough(pt) => {
                    let info = pt.get_info();
                    Some(format!(
                        "GPU Passthrough: {:?} {}\nVRAM: {} MB\nDriver: {}",
                        info.vendor,
                        info.model,
                        info.vram_size / 1024 / 1024,
                        info.driver
                    ))
                }
                GpuBackend::Mdev(mdev) => {
                    Some(format!(
                        "mdev GPU: {:?}\nUUID: {}",
                        mdev.get_type(),
                        mdev.get_uuid().unwrap_or("not created")
                    ))
                }
                GpuBackend::Wgpu(wgpu) => {
                    let stats = GpuBackendTrait::get_stats(wgpu);
                    Some(format!(
                        "WGPU Virtual GPU\nCommand buffers: {}\nRender passes: {}\nCompute passes: {}\nTextures: {}\nBuffers: {}\nMemory: {} MB",
                        stats.command_buffer_count,
                        stats.render_pass_count,
                        stats.compute_pass_count,
                        stats.texture_count,
                        stats.buffer_count,
                        stats.total_memory_allocated / 1024 / 1024
                    ))
                }
            }
        } else {
            None
        }
    }
}

impl Default for UnifiedGpuManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_manager() {
        let mut manager = UnifiedGpuManager::new();
        
        // 扫描后端
        manager.scan_backends().expect("Failed to scan backends");
        
        // 打印可用后端
        manager.print_available_backends();
        
        // 自动选择
        manager.auto_select().expect("Failed to auto-select");
        
        // 打印统计信息
        if let Some(stats) = manager.get_stats() {
            println!("\n=== Selected Backend Stats ===");
            println!("{}", stats);
        }
    }

    #[test]
    fn test_manual_selection() {
        let mut manager = UnifiedGpuManager::new();
        manager.scan_backends().expect("Failed to scan backends");
        
        // 尝试选择 WGPU（总是可用）
        manager.select_by_mode(GpuMode::Wgpu).expect("Failed to select WGPU");
        
        assert_eq!(manager.get_selected_backend().unwrap().mode(), GpuMode::Wgpu);
    }

    #[test]
    fn test_preferred_mode_fallback() {
        let mut manager = UnifiedGpuManager::new();
        
        // 设置偏好为 Passthrough，但启用自动降级
        manager.set_preferred_mode(GpuMode::Passthrough);
        manager.set_auto_fallback(true);
        
        manager.scan_backends().expect("Failed to scan backends");
        manager.auto_select().expect("Failed to auto-select");
        
        // 应该选择了某个后端（可能降级到 WGPU）
        assert!(manager.get_selected_backend().is_some());
    }
}
