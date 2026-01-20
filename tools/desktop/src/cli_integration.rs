//! CLI Integration Module
//!
//! Bridges the Tauri GUI with the CLI installation commands

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Supported OS distributions
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Distribution {
    Ubuntu,
    Debian,
    Arch,
    Manjaro,
    Fedora,
    CentOS,
    Windows,
    LinuxMint,
    PopOS,
    OpenSUSE,
}

impl Distribution {
    pub fn name(&self) -> &str {
        match self {
            Distribution::Ubuntu => "Ubuntu",
            Distribution::Debian => "Debian",
            Distribution::Arch => "Arch Linux",
            Distribution::Manjaro => "Manjaro",
            Distribution::Fedora => "Fedora",
            Distribution::CentOS => "CentOS",
            Distribution::Windows => "Windows",
            Distribution::LinuxMint => "Linux Mint",
            Distribution::PopOS => "Pop!_OS",
            Distribution::OpenSUSE => "openSUSE",
        }
    }

    pub fn all() -> &'static [Distribution] {
        &[
            Distribution::Ubuntu,
            Distribution::Debian,
            Distribution::Arch,
            Distribution::Manjaro,
            Distribution::Fedora,
            Distribution::CentOS,
            Distribution::Windows,
            Distribution::LinuxMint,
            Distribution::PopOS,
            Distribution::OpenSUSE,
        ]
    }
}

/// Installation configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstallConfig {
    pub distribution: Distribution,
    pub iso_path: String,
    pub disk_path: Option<String>,
    pub disk_size_gb: u64,
    pub memory_mb: usize,
    pub vcpus: usize,
}

impl Default for InstallConfig {
    fn default() -> Self {
        Self {
            distribution: Distribution::Ubuntu,
            iso_path: String::new(),
            disk_path: None,
            disk_size_gb: 30,
            memory_mb: 4096,
            vcpus: 1,
        }
    }
}

impl InstallConfig {
    /// Convert to internal config that uses PathBuf
    pub fn to_internal(&self) -> InternalInstallConfig {
        InternalInstallConfig {
            distribution: self.distribution,
            iso_path: PathBuf::from(&self.iso_path),
            disk_path: self.disk_path.as_ref().map(PathBuf::from),
            disk_size_gb: self.disk_size_gb,
            memory_mb: self.memory_mb,
            vcpus: self.vcpus,
        }
    }
}

/// Internal installation config that uses PathBuf
#[derive(Clone)]
pub struct InternalInstallConfig {
    pub distribution: Distribution,
    pub iso_path: PathBuf,
    pub disk_path: Option<PathBuf>,
    pub disk_size_gb: u64,
    pub memory_mb: usize,
    pub vcpus: usize,
}

/// Installation progress and status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstallProgress {
    pub step: String,
    pub progress_percent: u8,
    pub current_message: String,
    pub is_complete: bool,
    pub has_error: bool,
    pub error_message: Option<String>,
}

impl Default for InstallProgress {
    fn default() -> Self {
        Self {
            step: "Initializing".to_string(),
            progress_percent: 0,
            current_message: "Preparing installation...".to_string(),
            is_complete: false,
            has_error: false,
            error_message: None,
        }
    }
}

/// Installation result
#[derive(Debug, Clone)]
pub struct InstallResult {
    pub success: bool,
    pub disk_path: String,
    pub disk_size_gb: u64,
    pub iso_size_mb: u64,
    pub kernel_loaded: bool,
    pub boot_complete: bool,
    pub final_mode: String,
}

/// Active installation tracker
pub struct ActiveInstallation {
    pub config: InternalInstallConfig,
    pub progress: Arc<Mutex<InstallProgress>>,
    pub is_running: Arc<Mutex<bool>>,
}

/// CLI Integration Service
pub struct CliIntegrationService {
    active_installations: Arc<Mutex<Vec<ActiveInstallation>>>,
}

impl CliIntegrationService {
    pub fn new() -> Self {
        Self {
            active_installations: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get all supported distributions
    pub fn list_distributions(&self) -> Vec<DistroInfo> {
        Distribution::all()
            .iter()
            .map(|d| DistroInfo {
                name: d.name().to_string(),
                id: format!("{:?}", d),
            })
            .collect()
    }

    /// Get default configuration for a distribution
    pub fn get_default_config(&self, distribution: Distribution) -> InstallConfig {
        match distribution {
            Distribution::Ubuntu => InstallConfig {
                distribution: Distribution::Ubuntu,
                iso_path: String::new(),
                disk_path: None,
                disk_size_gb: 30,
                memory_mb: 4096,
                vcpus: 1,
            },
            Distribution::Debian => InstallConfig {
                distribution: Distribution::Debian,
                iso_path: String::new(),
                disk_path: None,
                disk_size_gb: 20,
                memory_mb: 3072,
                vcpus: 1,
            },
            Distribution::Arch => InstallConfig {
                distribution: Distribution::Arch,
                iso_path: String::new(),
                disk_path: None,
                disk_size_gb: 20,
                memory_mb: 2048,
                vcpus: 1,
            },
            Distribution::Manjaro => InstallConfig {
                distribution: Distribution::Manjaro,
                iso_path: String::new(),
                disk_path: None,
                disk_size_gb: 30,
                memory_mb: 4096,
                vcpus: 1,
            },
            Distribution::Windows => InstallConfig {
                distribution: Distribution::Windows,
                iso_path: String::new(),
                disk_path: None,
                disk_size_gb: 50,
                memory_mb: 8192,
                vcpus: 2,
            },
            _ => InstallConfig::default(),
        }
    }

    /// Start an OS installation
    pub async fn start_installation(
        &self,
        config: InstallConfig,
    ) -> Result<String, String> {
        let internal_config = config.to_internal();
        let install_id = format!("install_{}", uuid::Uuid::new_v4());

        let progress = Arc::new(Mutex::new(InstallProgress::default()));
        let is_running = Arc::new(Mutex::new(true));

        let installation = ActiveInstallation {
            config: internal_config.clone(),
            progress: progress.clone(),
            is_running: is_running.clone(),
        };

        {
            let mut installations = self.active_installations.lock().await;
            installations.push(installation);
        }

        // Start the installation in the background
        let install_id_clone = install_id.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::run_installation(internal_config, progress, is_running).await {
                eprintln!("Installation failed: {}", e);
            }
        });

        Ok(install_id_clone)
    }

    /// Get installation progress
    pub async fn get_installation_progress(
        &self,
        install_id: &str,
    ) -> Result<InstallProgress, String> {
        let installations = self.active_installations.lock().await;

        // Find the installation by ID (simplified - in reality, you'd track IDs)
        if !installations.is_empty() {
            let installation = installations.last().ok_or("No active installation")?;
            let progress = installation.progress.lock().await;
            return Ok(progress.clone());
        }

        Err("Installation not found".to_string())
    }

    /// Cancel an installation
    pub async fn cancel_installation(&self, install_id: &str) -> Result<(), String> {
        let mut installations = self.active_installations.lock().await;

        // Find and cancel the installation (simplified)
        if !installations.is_empty() {
            let installation = installations.last().ok_or("No active installation")?;
            *installation.is_running.lock().await = false;
            installations.pop();
            return Ok(());
        }

        Err("Installation not found".to_string())
    }

    /// Run the actual installation (this would call the CLI commands)
    async fn run_installation(
        config: InternalInstallConfig,
        progress: Arc<Mutex<InstallProgress>>,
        is_running: Arc<Mutex<bool>>,
    ) -> Result<InstallResult, String> {
        // Update progress
        {
            let mut p = progress.lock().await;
            p.step = "Validating ISO".to_string();
            p.progress_percent = 5;
            p.current_message = format!("Checking ISO: {}", config.iso_path.display());
        }

        // Simulate installation steps
        // In a real implementation, this would call the CLI installation commands

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        {
            let mut p = progress.lock().await;
            p.step = "Creating VM".to_string();
            p.progress_percent = 10;
            p.current_message = "Initializing VM service".to_string();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        {
            let mut p = progress.lock().await;
            p.step = "Creating Disk".to_string();
            p.progress_percent = 20;
            p.current_message = format!("Creating {} GB disk", config.disk_size_gb);
        }

        // Check if cancelled
        if !*is_running.lock().await {
            return Err("Installation cancelled".to_string());
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        {
            let mut p = progress.lock().await;
            p.step = "Loading Kernel".to_string();
            p.progress_percent = 50;
            p.current_message = "Extracting and loading kernel".to_string();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        {
            let mut p = progress.lock().await;
            p.step = "Booting".to_string();
            p.progress_percent = 80;
            p.current_message = "Starting boot sequence".to_string();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        {
            let mut p = progress.lock().await;
            p.step = "Complete".to_string();
            p.progress_percent = 100;
            p.current_message = "Installation complete".to_string();
            p.is_complete = true;
        }

        Ok(InstallResult {
            success: true,
            disk_path: config.disk_path.unwrap_or_else(|| PathBuf::from("./disk.img")).to_string_lossy().to_string(),
            disk_size_gb: config.disk_size_gb,
            iso_size_mb: 2500,
            kernel_loaded: true,
            boot_complete: true,
            final_mode: "GUI installer running".to_string(),
        })
    }
}

impl Default for CliIntegrationService {
    fn default() -> Self {
        Self::new()
    }
}

/// Distribution information for GUI
#[derive(Debug, Clone, serde::Serialize)]
pub struct DistroInfo {
    pub name: String,
    pub id: String,
}
