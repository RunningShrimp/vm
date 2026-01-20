//! # ISO Download Utility
//!
//! Download OS ISO images from official sources

use std::path::PathBuf;
use std::process::Command;

/// Supported OS distributions for ISO download
#[derive(Debug, Clone, Copy)]
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
    /// Get all supported distributions
    pub fn all() -> &'static [Distribution] {
        &[
            Distribution::Ubuntu,
            Distribution::Debian,
            Distribution::Arch,
            Distribution::Manjaro,
            Distribution::Fedora,
            Distribution::CentOS,
            Distribution::LinuxMint,
            Distribution::PopOS,
            Distribution::OpenSUSE,
        ]
    }

    /// Get distribution name
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

    /// Get download URL for latest version
    pub fn download_url(&self) -> &'static str {
        match self {
            Distribution::Ubuntu => {
                "https://releases.ubuntu.com/25.10/ubuntu-25.10-desktop-amd64.iso"
            }
            Distribution::Debian => {
                "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/debian-13.0.0-amd64-netinst.iso"
            }
            Distribution::Arch => {
                "https://geo.archlinux.org/iso/2025.01.01/archlinux-2025.01.01-x86_64.iso"
            }
            Distribution::Manjaro => {
                "https://download.manjaro.org/kde/24.0.0/manjaro-kde-24.0.0-240620-linux69.iso"
            }
            Distribution::Fedora => {
                "https://download.fedoraproject.org/pub/fedora/linux/releases/41/Workstation/x86_64/iso/Fedora-Workstation-Live-x86_64-41-1.4.iso"
            }
            Distribution::CentOS => {
                "https://mirror.stream.centos.org/9-stream/BaseOS/x86_64/iso/CentOS-Stream-9-latest-x86_64-boot.iso"
            }
            Distribution::Windows => {
                "https://go.microsoft.com/fwlink/?linkid=2247657" // Windows 11
            }
            Distribution::LinuxMint => {
                "https://linuxmint.com/iso/linuxmint-22.1-cinnamon-64bit.iso"
            }
            Distribution::PopOS => {
                "https://pop-iso.s3.amazonaws.com/pop-os_24.04.iso"
            }
            Distribution::OpenSUSE => {
                "https://download.opensuse.org/distribution/leap/15.6/iso/openSUSE-Leap-15.6-NET-x86_64-Media.iso"
            }
        }
    }

    /// Get expected minimum file size in bytes
    pub fn min_size_bytes(&self) -> u64 {
        match self {
            Distribution::Ubuntu => 3_000_000_000,     // 3GB
            Distribution::Debian => 400_000_000,       // 400MB
            Distribution::Arch => 900_000_000,         // 900MB
            Distribution::Manjaro => 2_500_000_000,    // 2.5GB
            Distribution::Fedora => 2_000_000_000,     // 2GB
            Distribution::CentOS => 700_000_000,       // 700MB
            Distribution::Windows => 5_000_000_000,    // 5GB
            Distribution::LinuxMint => 2_500_000_000,  // 2.5GB
            Distribution::PopOS => 2_500_000_000,      // 2.5GB
            Distribution::OpenSUSE => 500_000_000,     // 500MB
        }
    }
}

/// ISO Downloader
pub struct IsoDownloader {
    distribution: Distribution,
    output_path: PathBuf,
    show_progress: bool,
}

impl IsoDownloader {
    /// Create new ISO downloader
    pub fn new(distribution: Distribution, output_path: PathBuf) -> Self {
        Self {
            distribution,
            output_path,
            show_progress: true,
        }
    }

    /// Set progress display
    pub fn show_progress(mut self, show: bool) -> Self {
        self.show_progress = show;
        self
    }

    /// Download ISO
    pub async fn download(&self) -> Result<DownloadResult, String> {
        println!("===========================================");
        println!("    {} ISO Download", self.distribution.name());
        println!("===========================================");
        println!();

        let url = self.distribution.download_url();
        println!("Source: {}", url);
        println!("Destination: {}", self.output_path.display());
        println!();

        // Check if curl is available
        let curl_available = Command::new("curl")
            .arg("--version")
            .output()
            .is_ok();

        // Check if wget is available
        let wget_available = Command::new("wget")
            .arg("--version")
            .output()
            .is_ok();

        if curl_available {
            self.download_with_curl(url).await
        } else if wget_available {
            self.download_with_wget(url).await
        } else {
            Err("Neither curl nor wget is available. Please install one of them.".to_string())
        }
    }

    async fn download_with_curl(&self, url: &str) -> Result<DownloadResult, String> {
        use std::time::Instant;

        println!("Using curl for download...");
        let start = Instant::now();

        let mut args = vec![
            "-L",            // Follow redirects
            "-o",            // Output file
            self.output_path.to_str().unwrap(),
            url,
        ];

        if self.show_progress {
            args.insert(1, "#");
            args.insert(1, "--progress-bar");
        } else {
            args.insert(1, "-s");
            args.insert(1, "-S");
        }

        let output = Command::new("curl")
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to execute curl: {}", e))?;

        let duration = start.elapsed();

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Download failed: {}", stderr));
        }

        let metadata = std::fs::metadata(&self.output_path)
            .map_err(|e| format!("Failed to get file metadata: {}", e))?;

        let size_bytes = metadata.len();
        let size_mb = size_bytes / 1024 / 1024;

        if size_bytes < self.distribution.min_size_bytes() {
            return Err(format!(
                "Downloaded file is too small ({} MB). Expected at least {} MB",
                size_mb,
                self.distribution.min_size_bytes() / 1024 / 1024
            ));
        }

        println!();
        println!("✓ Download completed successfully!");
        println!("  Size: {} MB", size_mb);
        println!("  Duration: {:.2}s", duration.as_secs_f64());

        Ok(DownloadResult {
            path: self.output_path.clone(),
            size_bytes,
            duration_secs: duration.as_secs_f64(),
        })
    }

    async fn download_with_wget(&self, url: &str) -> Result<DownloadResult, String> {
        use std::time::Instant;

        println!("Using wget for download...");
        let start = Instant::now();

        let mut args = vec![
            "-O",            // Output file
            self.output_path.to_str().unwrap(),
            url,
        ];

        if !self.show_progress {
            args.insert(1, "-q");
        }

        let output = Command::new("wget")
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to execute wget: {}", e))?;

        let duration = start.elapsed();

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Download failed: {}", stderr));
        }

        let metadata = std::fs::metadata(&self.output_path)
            .map_err(|e| format!("Failed to get file metadata: {}", e))?;

        let size_bytes = metadata.len();
        let size_mb = size_bytes / 1024 / 1024;

        if size_bytes < self.distribution.min_size_bytes() {
            return Err(format!(
                "Downloaded file is too small ({} MB)",
                size_mb
            ));
        }

        println!();
        println!("✓ Download completed successfully!");
        println!("  Size: {} MB", size_mb);
        println!("  Duration: {:.2}s", duration.as_secs_f64());

        Ok(DownloadResult {
            path: self.output_path.clone(),
            size_bytes,
            duration_secs: duration.as_secs_f64(),
        })
    }
}

/// Download result
#[derive(Debug, Clone)]
pub struct DownloadResult {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub duration_secs: f64,
}

impl DownloadResult {
    pub fn size_mb(&self) -> u64 {
        self.size_bytes / 1024 / 1024
    }
}

/// List all available distributions
pub fn list_distributions() {
    println!("Available distributions for ISO download:");
    println!();

    for distro in Distribution::all() {
        println!("  {}", distro.name());
        println!("    URL: {}", distro.download_url());
        println!("    Min size: {} MB", distro.min_size_bytes() / 1024 / 1024);
        println!();
    }
}
