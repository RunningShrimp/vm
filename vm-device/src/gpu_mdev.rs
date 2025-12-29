use vm_passthrough::{PassthroughError, PciAddress};

/// Mediated Device 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdevType {
    IntelGvtG,  // Intel GVT-g
    NvidiaVgpu, // NVIDIA vGPU
    AmdMxgpu,   // AMD MxGPU
}

/// Mediated Device 配置
#[derive(Debug, Clone)]
pub struct MdevConfig {
    pub mdev_type: MdevType,
    pub type_id: String,
    pub name: String,
    pub description: String,
    pub instances: u32,
    pub available_instances: u32,
}

/// Mediated Device GPU
pub struct GpuMdev {
    parent_address: PciAddress,
    mdev_uuid: Option<String>,
    mdev_type: MdevType,
    type_id: String,
    created: bool,
}

impl GpuMdev {
    /// 创建新的 mdev GPU 实例
    pub fn new(parent_address: PciAddress, mdev_type: MdevType, type_id: String) -> Self {
        Self {
            parent_address,
            mdev_uuid: None,
            mdev_type,
            type_id,
            created: false,
        }
    }

    /// 获取父设备地址
    pub fn parent_address(&self) -> &PciAddress {
        &self.parent_address
    }

    /// 获取类型ID
    pub fn type_id(&self) -> &str {
        &self.type_id
    }

    /// 获取mdev UUID
    pub fn mdev_uuid(&self) -> Option<&String> {
        self.mdev_uuid.as_ref()
    }

    /// 检查 mdev 是否可用
    pub fn is_available(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            let mdev_path = self.get_mdev_supported_types_path();
            mdev_path.exists()
        }

        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    /// 获取 mdev 支持的类型路径
    #[cfg(target_os = "linux")]
    fn get_mdev_supported_types_path(&self) -> PathBuf {
        PathBuf::from(format!(
            "/sys/bus/pci/devices/{}/mdev_supported_types",
            self.parent_address.to_string()
        ))
    }

    /// 获取 mdev 设备路径
    #[cfg(target_os = "linux")]
    fn get_mdev_path(&self) -> PathBuf {
        PathBuf::from(format!(
            "/sys/bus/pci/devices/{}",
            self.parent_address.to_string()
        ))
    }

    /// 列出支持的 mdev 类型
    #[cfg(target_os = "linux")]
    pub fn list_supported_types(&self) -> Result<Vec<MdevConfig>, PassthroughError> {
        let types_path = self.get_mdev_supported_types_path();
        if !types_path.exists() {
            return Err(PassthroughError::DeviceNotFound(
                "mdev not supported on this device".to_string(),
            ));
        }

        let mut configs = Vec::new();

        for entry in fs::read_dir(&types_path)? {
            let entry = entry?;
            let type_id = entry.file_name().to_string_lossy().to_string();
            let type_path = entry.path();

            // 读取类型信息
            let name = fs::read_to_string(type_path.join("name"))
                .unwrap_or_else(|_| type_id.clone())
                .trim()
                .to_string();

            let description = fs::read_to_string(type_path.join("description"))
                .unwrap_or_else(|_| "No description".to_string())
                .trim()
                .to_string();

            let available_instances = fs::read_to_string(type_path.join("available_instances"))
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
                .unwrap_or(0);

            // 尝试读取设备数量
            let device_api = fs::read_to_string(type_path.join("device_api"))
                .unwrap_or_else(|_| "vfio-pci".to_string())
                .trim()
                .to_string();

            // 根据名称推断 mdev 类型
            let mdev_type = if name.contains("GVT") || name.contains("gvt") {
                MdevType::IntelGvtG
            } else if name.contains("GRID") || name.contains("vGPU") {
                MdevType::NvidiaVgpu
            } else if name.contains("MxGPU") {
                MdevType::AmdMxgpu
            } else {
                MdevType::IntelGvtG // 默认
            };

            configs.push(MdevConfig {
                mdev_type,
                type_id,
                name,
                description,
                instances: 0,
                available_instances,
            });
        }

        Ok(configs)
    }

    #[cfg(not(target_os = "linux"))]
    pub fn list_supported_types(&self) -> Result<Vec<MdevConfig>, PassthroughError> {
        Err(PassthroughError::DeviceNotFound(
            "mdev not supported on this platform".to_string(),
        ))
    }

    /// 创建 mdev 设备
    #[cfg(target_os = "linux")]
    pub fn create(&mut self) -> Result<(), PassthroughError> {
        if self.created {
            return Ok(());
        }

        // 生成 UUID
        let uuid = uuid::Uuid::new_v4().to_string();

        // 创建 mdev 设备
        let create_path = self
            .get_mdev_path()
            .join("mdev_supported_types")
            .join(&self.type_id)
            .join("create");

        if !create_path.exists() {
            return Err(PassthroughError::DeviceNotFound(format!(
                "mdev type {} not found",
                self.type_id
            )));
        }

        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(&create_path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    PassthroughError::PermissionDenied
                } else {
                    PassthroughError::IoError(e)
                }
            })?;

        file.write_all(uuid.as_bytes())
            .map_err(|e| PassthroughError::DriverBindingFailed(e.to_string()))?;

        self.mdev_uuid = Some(uuid.clone());
        self.created = true;

        log::info!("Created mdev device: {} (type: {})", uuid, self.type_id);
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn create(&mut self) -> Result<(), PassthroughError> {
        Err(PassthroughError::DeviceNotFound(
            "mdev not supported on this platform".to_string(),
        ))
    }

    /// 删除 mdev 设备
    #[cfg(target_os = "linux")]
    pub fn destroy(&mut self) -> Result<(), PassthroughError> {
        if !self.created {
            return Ok(());
        }

        if let Some(uuid) = &self.mdev_uuid {
            let remove_path = self.get_mdev_path().join(uuid).join("remove");

            if remove_path.exists() {
                let mut file = fs::OpenOptions::new().write(true).open(&remove_path)?;

                file.write_all(b"1")?;

                log::info!("Destroyed mdev device: {}", uuid);
            }

            self.mdev_uuid = None;
            self.created = false;
        }

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn destroy(&mut self) -> Result<(), PassthroughError> {
        Ok(())
    }

    /// 获取 mdev UUID
    pub fn get_uuid(&self) -> Option<&str> {
        self.mdev_uuid.as_deref()
    }

    /// 获取 mdev 类型
    pub fn get_type(&self) -> MdevType {
        self.mdev_type
    }

    /// 是否已创建
    pub fn is_created(&self) -> bool {
        self.created
    }
}

impl Drop for GpuMdev {
    fn drop(&mut self) {
        // 自动清理 mdev 设备
        let _ = self.destroy();
    }
}

/// 扫描支持 mdev 的 GPU
#[cfg(target_os = "linux")]
pub fn scan_mdev_capable_gpus() -> Vec<(PciAddress, Vec<MdevConfig>)> {
    let mut result = Vec::new();

    let pci_path = Path::new("/sys/bus/pci/devices");
    if !pci_path.exists() {
        return result;
    }

    if let Ok(entries) = fs::read_dir(pci_path) {
        for entry in entries.flatten() {
            let addr_str = entry.file_name().to_string_lossy().to_string();

            if let Ok(address) = PciAddress::from_str(&addr_str) {
                // 检查是否支持 mdev
                let mdev_path = entry.path().join("mdev_supported_types");
                if mdev_path.exists() {
                    let mdev = GpuMdev::new(address, MdevType::IntelGvtG, String::new());

                    if let Ok(configs) = mdev.list_supported_types() {
                        if !configs.is_empty() {
                            result.push((address, configs));
                        }
                    }
                }
            }
        }
    }

    result
}

#[cfg(not(target_os = "linux"))]
pub fn scan_mdev_capable_gpus() -> Vec<(PciAddress, Vec<MdevConfig>)> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_mdev_gpus() {
        let gpus = scan_mdev_capable_gpus();
        println!("Found {} mdev-capable GPU(s):", gpus.len());
        for (addr, configs) in gpus {
            println!("  GPU at {}:", addr.to_string());
            for config in configs {
                println!("    - {} ({})", config.name, config.type_id);
                println!("      Available instances: {}", config.available_instances);
                println!("      Description: {}", config.description);
            }
        }
    }
}
