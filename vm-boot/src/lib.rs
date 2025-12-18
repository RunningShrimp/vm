//! vm-boot: 虚拟机快速启动框架
//!
//! 支持多种启动方式：
//! - Direct Boot: 直接加载内核，跳过固件
//! - UEFI Boot: 使用 UEFI 固件引导
//! - BIOS Boot: 使用传统 BIOS 引导
//!
//! 同时提供运行时控制、快照和设备热插拔功能

use vm_core::{GuestAddr, PlatformError, VmError};

// 子模块
pub mod eltorito;
pub mod fast_boot;
pub mod gc_runtime;
pub mod hotplug;
pub mod incremental_snapshot;
pub mod iso9660;
pub mod runtime;
pub mod runtime_service;
pub mod snapshot;

// 重新导出常用类型
pub use gc_runtime::GcRuntime;
pub use hotplug::{DeviceInfo, DeviceType, HotplugEvent, HotplugManager};
pub use runtime::{RuntimeCommand, RuntimeController, RuntimeEvent, RuntimeState};
pub use snapshot::{SnapshotFileManager, SnapshotMetadata, VmSnapshot};
#[allow(deprecated)]
pub use snapshot::SnapshotManager; // 保留SnapshotManager以保持向后兼容

/// 启动错误类型别名
pub type BootError = VmError;

/// 从传统错误转换为统一错误
impl From<BootLegacyError> for VmError {
    fn from(err: BootLegacyError) -> Self {
        match err {
            BootLegacyError::KernelLoadFailed(msg) => {
                VmError::Core(vm_core::CoreError::InvalidState {
                    message: msg,
                    current: "Unknown".to_string(),
                    expected: "Valid kernel".to_string(),
                })
            }
            BootLegacyError::InitrdLoadFailed(msg) => {
                VmError::Platform(PlatformError::InitializationFailed(msg))
            }
            BootLegacyError::FirmwareLoadFailed(msg) => {
                VmError::Platform(PlatformError::InitializationFailed(msg))
            }
            BootLegacyError::InvalidConfig(msg) => {
                VmError::Platform(PlatformError::InitializationFailed(msg))
            }
            BootLegacyError::Io(e) => VmError::Platform(PlatformError::IoError(e.to_string())),
        }
    }
}

/// 传统的启动错误类型（保留用于向后兼容）
#[derive(Debug, thiserror::Error)]
pub enum BootLegacyError {
    #[error("Failed to load kernel: {0}")]
    KernelLoadFailed(String),
    #[error("Failed to load initrd: {0}")]
    InitrdLoadFailed(String),
    #[error("Failed to load firmware: {0}")]
    FirmwareLoadFailed(String),
    #[error("Invalid boot configuration: {0}")]
    InvalidConfig(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// 启动方式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootMethod {
    /// 直接启动（Linux Direct Boot）
    Direct,
    /// UEFI 固件启动
    Uefi,
    /// BIOS 固件启动
    Bios,
    /// ISO 镜像引导
    Iso,
}

/// 启动配置
#[derive(Debug, Clone)]
pub struct BootConfig {
    /// 启动方式
    pub method: BootMethod,
    /// 内核镜像路径
    pub kernel: Option<String>,
    /// 内核命令行参数
    pub cmdline: Option<String>,
    /// Initrd 镜像路径
    pub initrd: Option<String>,
    /// 固件路径（UEFI/BIOS）
    pub firmware: Option<String>,
    /// ISO 镜像路径
    pub iso: Option<String>,
    /// 内核加载地址
    pub kernel_load_addr: GuestAddr,
    /// Initrd 加载地址
    pub initrd_load_addr: GuestAddr,
}

impl Default for BootConfig {
    fn default() -> Self {
        Self {
            method: BootMethod::Direct,
            kernel: None,
            cmdline: None,
            initrd: None,
            firmware: None,
            iso: None,
            kernel_load_addr: GuestAddr(0x80000000), // 默认 RISC-V/ARM64 加载地址
            initrd_load_addr: GuestAddr(0x84000000),
        }
    }
}

impl BootConfig {
    /// 创建新的启动配置
    pub fn new(method: BootMethod) -> Self {
        Self {
            method,
            ..Default::default()
        }
    }

    /// 设置内核路径
    pub fn with_kernel(mut self, path: impl Into<String>) -> Self {
        self.kernel = Some(path.into());
        self
    }

    /// 设置内核命令行
    pub fn with_cmdline(mut self, cmdline: impl Into<String>) -> Self {
        self.cmdline = Some(cmdline.into());
        self
    }

    /// 设置 initrd 路径
    pub fn with_initrd(mut self, path: impl Into<String>) -> Self {
        self.initrd = Some(path.into());
        self
    }

    /// 设置固件路径
    pub fn with_firmware(mut self, path: impl Into<String>) -> Self {
        self.firmware = Some(path.into());
        self
    }

    /// 设置 ISO 镜像路径
    pub fn with_iso(mut self, path: impl Into<String>) -> Self {
        self.iso = Some(path.into());
        self
    }

    /// 设置内核加载地址
    pub fn with_kernel_addr(mut self, addr: GuestAddr) -> Self {
        self.kernel_load_addr = addr;
        self
    }

    /// 设置 initrd 加载地址
    pub fn with_initrd_addr(mut self, addr: GuestAddr) -> Self {
        self.initrd_load_addr = addr;
        self
    }

    /// 验证配置
    pub fn validate(&self) -> Result<(), BootError> {
        match self.method {
            BootMethod::Direct => {
                if self.kernel.is_none() {
                    return Err(VmError::Platform(PlatformError::InitializationFailed(
                        "Direct boot requires kernel path".to_string(),
                    )));
                }
            }
            BootMethod::Uefi | BootMethod::Bios => {
                if self.firmware.is_none() {
                    return Err(VmError::Platform(PlatformError::InitializationFailed(
                        format!("{:?} boot requires firmware path", self.method),
                    )));
                }
            }
            BootMethod::Iso => {
                if self.iso.is_none() {
                    return Err(VmError::Platform(PlatformError::InitializationFailed(
                        "ISO boot requires ISO path".to_string(),
                    )));
                }
            }
        }
        Ok(())
    }
}

/// 启动加载器
pub struct BootLoader {
    config: BootConfig,
}

impl BootLoader {
    /// 创建新的启动加载器
    pub fn new(config: BootConfig) -> Result<Self, BootError> {
        config.validate()?;
        Ok(Self { config })
    }

    /// 加载内核到内存
    pub fn load_kernel(&self, memory: &mut dyn vm_core::MMU) -> Result<GuestAddr, BootError> {
        let kernel_path = self.config.kernel.as_ref().ok_or_else(|| {
            VmError::Platform(PlatformError::InitializationFailed(
                "No kernel specified".to_string(),
            ))
        })?;

        let kernel_data = std::fs::read(kernel_path).map_err(|e| {
            VmError::Platform(PlatformError::InitializationFailed(format!(
                "Failed to read {}: {}",
                kernel_path, e
            )))
        })?;

        log::info!(
            "Loading kernel from {} ({} bytes)",
            kernel_path,
            kernel_data.len()
        );

        // 写入内存
        let load_addr = self.config.kernel_load_addr;
        memory.write_bulk(load_addr, &kernel_data).map_err(|e| {
            VmError::Platform(PlatformError::InitializationFailed(format!(
                "Memory write failed: {:?}",
                e
            )))
        })?;

        log::info!("Kernel loaded at 0x{:x}", load_addr);
        Ok(load_addr)
    }

    /// 加载 initrd 到内存
    pub fn load_initrd(
        &self,
        memory: &mut dyn vm_core::MMU,
    ) -> Result<Option<(GuestAddr, usize)>, BootError> {
        let Some(initrd_path) = &self.config.initrd else {
            return Ok(None);
        };

        let initrd_data = std::fs::read(initrd_path).map_err(|e| {
            VmError::Platform(PlatformError::InitializationFailed(format!(
                "Failed to read {}: {}",
                initrd_path, e
            )))
        })?;

        log::info!(
            "Loading initrd from {} ({} bytes)",
            initrd_path,
            initrd_data.len()
        );

        let load_addr = self.config.initrd_load_addr;
        for (i, &byte) in initrd_data.iter().enumerate() {
            memory
                .write(load_addr + i as u64, byte as u64, 1)
                .map_err(|e| {
                    VmError::Platform(PlatformError::InitializationFailed(format!(
                        "Memory write failed: {:?}",
                        e
                    )))
                })?;
        }

        log::info!("Initrd loaded at 0x{:x}", load_addr);
        Ok(Some((load_addr, initrd_data.len())))
    }

    /// 设置内核命令行
    pub fn setup_cmdline(
        &self,
        memory: &mut dyn vm_core::MMU,
        cmdline_addr: GuestAddr,
    ) -> Result<(), BootError> {
        let Some(cmdline) = &self.config.cmdline else {
            return Ok(());
        };

        log::info!("Setting up kernel command line: {}", cmdline);

        for (i, &byte) in cmdline.as_bytes().iter().enumerate() {
            memory
                .write(cmdline_addr + i as u64, byte as u64, 1)
                .map_err(|_| {
                    VmError::Core(vm_core::CoreError::InvalidState {
                        message: "Cmdline write failed".to_string(),
                        current: "Memory write error".to_string(),
                        expected: "Successful write".to_string(),
                    })
                })?;
        }

        // 添加 null 终止符
        memory
            .write(cmdline_addr + cmdline.len() as u64, 0, 1)
            .map_err(|_| {
                VmError::Core(vm_core::CoreError::InvalidState {
                    message: "Cmdline null terminator write failed".to_string(),
                    current: "Memory write error".to_string(),
                    expected: "Successful write".to_string(),
                })
            })?;

        Ok(())
    }

    /// 执行完整的启动流程
    pub fn boot(&self, memory: &mut dyn vm_core::MMU) -> Result<BootInfo, BootError> {
        match self.config.method {
            BootMethod::Direct => self.direct_boot(memory),
            BootMethod::Uefi => self.uefi_boot(memory),
            BootMethod::Bios => self.bios_boot(memory),
            BootMethod::Iso => self.iso_boot(memory),
        }
    }

    /// 直接启动（跳过固件）
    fn direct_boot(&self, memory: &mut dyn vm_core::MMU) -> Result<BootInfo, BootError> {
        log::info!("Starting Direct Boot");

        let kernel_addr = self.load_kernel(memory)?;
        let initrd_info = self.load_initrd(memory)?;

        // 命令行地址通常在 initrd 之后
        let cmdline_addr = if let Some((initrd_addr, initrd_size)) = initrd_info {
            initrd_addr + initrd_size as u64 + 0x1000 // 4KB 对齐
        } else {
            self.config.initrd_load_addr
        };

        self.setup_cmdline(memory, cmdline_addr)?;

        Ok(BootInfo {
            entry_point: kernel_addr,
            initrd_addr: initrd_info.map(|(addr, _)| addr),
            initrd_size: initrd_info.map(|(_, size)| size),
            cmdline_addr: Some(cmdline_addr),
        })
    }

    /// UEFI 启动
    fn uefi_boot(&self, memory: &mut dyn vm_core::MMU) -> Result<BootInfo, BootError> {
        log::info!("Starting UEFI Boot");

        let firmware_path = self.config.firmware.as_ref().ok_or_else(|| {
            VmError::Platform(PlatformError::InitializationFailed(
                "No firmware specified".to_string(),
            ))
        })?;

        let firmware_data = std::fs::read(firmware_path).map_err(|e| {
            VmError::Platform(PlatformError::InitializationFailed(format!(
                "Failed to read {}: {}",
                firmware_path, e
            )))
        })?;

        log::info!(
            "Loading UEFI firmware from {} ({} bytes)",
            firmware_path,
            firmware_data.len()
        );

        // UEFI 固件通常加载到高地址
        let firmware_addr = 0xFFFF_0000 - firmware_data.len() as u64;

        for (i, &byte) in firmware_data.iter().enumerate() {
            memory
                .write(vm_core::GuestAddr(firmware_addr + i as u64), byte as u64, 1)
                .map_err(|e| {
                    VmError::Platform(PlatformError::InitializationFailed(format!(
                        "Memory write failed: {:?}",
                        e
                    )))
                })?;
        }

        log::info!("UEFI firmware loaded at 0x{:x}", firmware_addr);

        Ok(BootInfo {
            entry_point: vm_core::GuestAddr(firmware_addr),
            initrd_addr: None,
            initrd_size: None,
            cmdline_addr: None,
        })
    }

    /// BIOS 启动
    fn bios_boot(&self, memory: &mut dyn vm_core::MMU) -> Result<BootInfo, BootError> {
        log::info!("Starting BIOS Boot");

        let firmware_path = self.config.firmware.as_ref().ok_or_else(|| {
            VmError::Platform(PlatformError::InitializationFailed(
                "No firmware specified".to_string(),
            ))
        })?;

        let firmware_data = std::fs::read(firmware_path).map_err(|e| {
            VmError::Platform(PlatformError::InitializationFailed(format!(
                "Failed to read {}: {}",
                firmware_path, e
            )))
        })?;

        log::info!(
            "Loading BIOS firmware from {} ({} bytes)",
            firmware_path,
            firmware_data.len()
        );

        // BIOS 固件加载到 0xF0000
        let firmware_addr = 0xF0000;

        for (i, &byte) in firmware_data.iter().enumerate() {
            memory
                .write(vm_core::GuestAddr(firmware_addr + i as u64), byte as u64, 1)
                .map_err(|e| {
                    VmError::Platform(PlatformError::InitializationFailed(format!(
                        "Memory write failed: {:?}",
                        e
                    )))
                })?;
        }

        log::info!("BIOS firmware loaded at 0x{:x}", firmware_addr);

        Ok(BootInfo {
            entry_point: vm_core::GuestAddr(0xFFF0), // BIOS 重置向量
            initrd_addr: None,
            initrd_size: None,
            cmdline_addr: None,
        })
    }

    /// ISO 引导
    fn iso_boot(&self, memory: &mut dyn vm_core::MMU) -> Result<BootInfo, BootError> {
        use std::fs::File;

        use crate::eltorito::ElTorito;

        log::info!("Starting ISO Boot");

        let iso_path = match self.config.iso.as_ref() {
            Some(path) => path,
            None => {
                return Err(VmError::Platform(PlatformError::InitializationFailed(
                    "No ISO specified".to_string(),
                )));
            }
        };

        let file = File::open(iso_path).map_err(|e| BootError::Io(e.to_string()))?;

        // 解析 El Torito 引导目录
        let mut eltorito = ElTorito::new(file).map_err(|_| {
            VmError::Platform(PlatformError::InitializationFailed(
                "Failed to parse El Torito".to_string(),
            ))
        })?;

        let catalog = eltorito.boot_catalog().ok_or_else(|| {
            VmError::Platform(PlatformError::InitializationFailed(
                "No boot catalog found".to_string(),
            ))
        })?;

        log::info!("Found El Torito boot catalog");
        log::info!("Platform ID: {}", catalog.validation_entry.platform_id);
        log::info!(
            "Boot media type: {:?}",
            catalog.initial_entry.boot_media_type
        );

        // 读取引导镜像
        let initial_entry = catalog.initial_entry.clone();
        let boot_image = eltorito.read_boot_image(&initial_entry).map_err(|e| {
            VmError::Platform(PlatformError::InitializationFailed(format!(
                "Failed to read boot image: {}",
                e
            )))
        })?;

        log::info!("Loaded boot image ({} bytes)", boot_image.len());

        // 将引导镜像加载到内存
        // 对于 BIOS 引导，通常加载到 0x7C00
        let boot_addr = 0x7C00u64;

        for (i, &byte) in boot_image.iter().enumerate() {
            memory
                .write(GuestAddr(boot_addr + i as u64), byte as u64, 1)
                .map_err(|e| {
                    VmError::Platform(PlatformError::InitializationFailed(format!(
                        "Memory write failed: {:?}",
                        e
                    )))
                })?;
        }

        log::info!("Boot image loaded at 0x{:x}", boot_addr);

        Ok(BootInfo {
            entry_point: GuestAddr(boot_addr),
            initrd_addr: None,
            initrd_size: None,
            cmdline_addr: None,
        })
    }
}

/// 启动信息
#[derive(Debug, Clone)]
pub struct BootInfo {
    /// 入口点地址
    pub entry_point: GuestAddr,
    /// Initrd 地址
    pub initrd_addr: Option<GuestAddr>,
    /// Initrd 大小
    pub initrd_size: Option<usize>,
    /// 命令行地址
    pub cmdline_addr: Option<GuestAddr>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_config_validation() {
        let config = BootConfig::new(BootMethod::Direct);
        assert!(config.validate().is_err());

        let config = config.with_kernel("/path/to/kernel");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_boot_method() {
        assert_eq!(BootMethod::Direct, BootMethod::Direct);
        assert_ne!(BootMethod::Direct, BootMethod::Uefi);
    }
}
