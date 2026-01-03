//! Configuration Management
//!
//! Handles reading, writing, and validating VM configurations.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json;

use crate::ipc::VmConfig;

const CONFIG_DIR: &str = ".vm/configs";

/// Configuration manager for VM settings
pub struct ConfigManager {
    config_dir: PathBuf,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new(base_dir: Option<PathBuf>) -> Result<Self, String> {
        let config_dir = match base_dir {
            Some(dir) => dir.join(CONFIG_DIR),
            None => {
                let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
                home.join(CONFIG_DIR)
            }
        };

        // Create config directory if it doesn't exist
        fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;

        Ok(Self { config_dir })
    }

    /// Load all VM configurations
    pub fn load_all_configs(&self) -> Result<Vec<VmConfig>, String> {
        let entries = fs::read_dir(&self.config_dir).map_err(|e| e.to_string())?;
        let mut configs = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "json") {
                match self.load_config_from_path(&path) {
                    Ok(config) => configs.push(config),
                    Err(e) => {
                        eprintln!("Failed to load config {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(configs)
    }

    /// Load a specific configuration by VM ID
    pub fn load_config(&self, vm_id: &str) -> Result<VmConfig, String> {
        let path = self.config_dir.join(format!("{}.json", vm_id));
        self.load_config_from_path(&path)
    }

    /// Load configuration from a file path
    fn load_config_from_path(&self, path: &Path) -> Result<VmConfig, String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    }

    /// Save a VM configuration
    pub fn save_config(&self, config: &VmConfig) -> Result<(), String> {
        let path = self.config_dir.join(format!("{}.json", config.id));
        let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
        fs::write(path, content).map_err(|e| e.to_string())
    }

    /// Delete a VM configuration file
    pub fn delete_config(&self, vm_id: &str) -> Result<(), String> {
        let path = self.config_dir.join(format!("{}.json", vm_id));
        fs::remove_file(path).map_err(|e| e.to_string())
    }

    /// Validate VM configuration
    pub fn validate_config(config: &VmConfig) -> Result<(), String> {
        if config.name.is_empty() {
            return Err("VM name cannot be empty".to_string());
        }

        if config.cpu_count == 0 {
            return Err("CPU count must be at least 1".to_string());
        }

        if config.cpu_count > 128 {
            return Err("CPU count exceeds maximum (128)".to_string());
        }

        if config.memory_mb < 256 {
            return Err("Memory must be at least 256 MB".to_string());
        }

        if config.memory_mb > 1_048_576 {
            return Err("Memory exceeds maximum (1 TB)".to_string());
        }

        if config.disk_gb == 0 {
            return Err("Disk size must be at least 1 GB".to_string());
        }

        if config.disk_gb > 10_000 {
            return Err("Disk size exceeds maximum (10 TB)".to_string());
        }

        Ok(())
    }

    /// Create a default configuration template
    pub fn create_template(name: &str, os_type: crate::ipc::OsType) -> VmConfig {
        use uuid::Uuid;

        use crate::ipc::{DisplayMode, OsType};

        let (cpu, memory, disk, display_mode) = match os_type {
            OsType::Ubuntu | OsType::Debian => {
                (2, 4096, 50, DisplayMode::GUI) // Desktop variants with GUI
            }
            OsType::CentOS => (2, 2048, 30, DisplayMode::GUI),
            OsType::Windows => (4, 8192, 100, DisplayMode::GUI),
            OsType::Other => (2, 2048, 30, DisplayMode::Terminal),
        };

        VmConfig {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            cpu_count: cpu,
            memory_mb: memory,
            disk_gb: disk,
            display_mode,
            os_type,
        }
    }
}

pub mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            std::env::var("USERPROFILE").ok().map(PathBuf::from)
        }

        #[cfg(not(target_os = "windows"))]
        {
            std::env::var("HOME").ok().map(PathBuf::from)
        }
    }
}
