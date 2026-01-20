//! # CentOS/RHEL Installation Command
//!
//! CLI command to install CentOS or RHEL from ISO with virtual disk

use std::path::{Path, PathBuf};
use vm_core::{ExecMode, GuestArch, VmConfig};
use vm_service::VmService;

/// CentOS/RHEL Installer Command
pub struct CentosInstallCommand {
    /// ISO path
    iso_path: PathBuf,
    /// Disk path (optional, auto-generated if not provided)
    disk_path: Option<PathBuf>,
    /// Disk size in GB (default: 25)
    disk_size_gb: u64,
    /// Memory size in MB (default: 4096)
    memory_mb: usize,
    /// Number of VCPUs (default: 1)
    vcpus: usize,
}

impl CentosInstallCommand {
    /// Create new CentOS install command
    pub fn new<P: AsRef<Path>>(iso_path: P) -> Self {
        Self {
            iso_path: iso_path.as_ref().to_path_buf(),
            disk_path: None,
            disk_size_gb: 25, // CentOS/RHEL needs moderate space
            memory_mb: 4096,  // 4GB for installation
            vcpus: 1,
        }
    }

    /// Set disk path
    pub fn disk_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.disk_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Set disk size in GB
    pub fn disk_size_gb(mut self, size_gb: u64) -> Self {
        self.disk_size_gb = size_gb;
        self
    }

    /// Set memory size in MB
    pub fn memory_mb(mut self, memory_mb: usize) -> Self {
        self.memory_mb = memory_mb;
        self
    }

    /// Set number of VCPUs
    pub fn vcpus(mut self, vcpus: usize) -> Self {
        self.vcpus = vcpus;
        self
    }

    /// Run CentOS/RHEL installation
    pub async fn run(&self) -> Result<InstallResult, String> {
        println!("===========================================");
        println!("    CentOS/RHEL Installation");
        println!("===========================================");
        println!();

        // Step 1: Validate ISO exists
        println!("Step 1: Validating ISO...");
        if !self.iso_path.exists() {
            return Err(format!("ISO file not found: {}", self.iso_path.display()));
        }
        println!("✓ ISO found: {}", self.iso_path.display());
        println!();

        // Step 2: Determine disk path
        let disk_path = if let Some(ref path) = self.disk_path {
            path.clone()
        } else {
            // Auto-generate disk path next to ISO
            let iso_stem = self
                .iso_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let mut disk_path = self
                .iso_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
            disk_path.push(format!("{}_disk.img", iso_stem));
            disk_path
        };

        // Step 3: Create VM configuration
        println!("Step 2: Creating VM configuration...");
        println!("  Architecture: x86_64");
        println!("  Memory: {} MB", self.memory_mb);
        println!("  VCPUs: {}", self.vcpus);
        println!("  Disk: {} ({} GB)", disk_path.display(), self.disk_size_gb);
        println!();

        let memory_size = self.memory_mb * 1024 * 1024;
        let config = VmConfig {
            guest_arch: GuestArch::X86_64,
            vcpu_count: self.vcpus,
            memory_size,
            exec_mode: ExecMode::Interpreter,
            ..Default::default()
        };

        // Step 4: Create VM service
        println!("Step 3: Initializing VM service...");
        let mut service = VmService::new(config, None)
            .await
            .map_err(|e| format!("Failed to create VM service: {}", e))?;
        println!("✓ VM service initialized");
        println!();

        // Step 5: Create virtual disk
        println!("Step 4: Creating virtual disk...");
        match service.create_disk(disk_path.to_str().unwrap(), self.disk_size_gb) {
            Ok(disk_info) => {
                println!("✓ Disk created successfully");
                println!("  Size: {:.2} GB", disk_info.size_gb());
                println!("  Sectors: {}", disk_info.sector_count);
            }
            Err(e) => match service.get_disk_info(disk_path.to_str().unwrap()) {
                Ok(info) => {
                    println!("✓ Using existing disk");
                    println!("  Size: {:.2} GB", info.size_gb());
                }
                Err(_) => {
                    return Err(format!("Failed to create disk: {}", e));
                }
            },
        }
        println!();

        // Step 6: Attach ISO
        println!("Step 5: Attaching ISO image...");
        let iso_info = service
            .attach_iso(self.iso_path.to_str().unwrap())
            .map_err(|e| format!("Failed to attach ISO: {}", e))?;
        println!("✓ ISO attached successfully");
        println!("  Size: {} MB", iso_info.size_mb);
        println!();

        // Step 7: Initialize AHCI SATA controller
        println!("Step 6: Initializing AHCI SATA controller...");
        service
            .init_ahci_controller()
            .map_err(|e| format!("Failed to initialize AHCI: {}", e))?;
        println!("✓ AHCI controller initialized");
        println!();

        // Step 8: Attach disk to AHCI
        println!("Step 7: Attaching disk to AHCI...");
        service
            .attach_disk_to_ahci(0, disk_path.to_str().unwrap())
            .map_err(|e| format!("Failed to attach disk to AHCI: {}", e))?;
        println!("✓ Disk attached to AHCI port 0");
        println!();

        // Step 9: Initialize ATAPI CD-ROM
        println!("Step 8: Initializing ATAPI CD-ROM...");
        service
            .init_atapi_cdrom(self.iso_path.to_str().unwrap())
            .map_err(|e| format!("Failed to initialize ATAPI CD-ROM: {}", e))?;
        println!("✓ ATAPI CD-ROM initialized");
        println!();

        // Step 10: Display storage device status
        println!("Step 9: Storage device status...");
        let storage_info = service.get_storage_devices_info();
        println!("  {}", storage_info.ahci_controller);
        println!("  {}", storage_info.atapi_cdrom);
        println!();

        // Step 11: Extract kernel from ISO
        println!("Step 10: Extracting kernel from ISO...");
        let kernel_path = "/tmp/centos_iso_extracted/centos_vmlinuz";

        if !Path::new(kernel_path).exists() {
            println!("  Extracting kernel files...");
            self.extract_kernel()?;
        } else {
            println!("  Using cached kernel");
        }
        println!("✓ Kernel ready: {}", kernel_path);
        println!();

        // Step 12: Load kernel
        println!("Step 11: Loading kernel...");
        let setup_load_addr = 0x10000u64;
        let pm_load_addr = 0x100000u64;

        let kernel_data =
            std::fs::read(kernel_path).map_err(|e| format!("Failed to read kernel file: {}", e))?;

        println!("  Kernel size: {} bytes", kernel_data.len());
        println!("  Loading as bzImage (split setup/protected mode)...");

        let entry_point = service
            .load_bzimage_kernel(&kernel_data, setup_load_addr, pm_load_addr)
            .map_err(|e| format!("Failed to load bzImage kernel: {}", e))?;

        println!("✓ bzImage loaded properly");
        println!("  Setup code: {:#010X}", setup_load_addr);
        println!("  Protected mode: {:#010X}", pm_load_addr);
        println!("  Entry point: {:#010X}", entry_point);
        println!();

        // Step 13: Boot sequence
        println!("Step 12: Starting boot sequence...");
        println!("===========================================");
        println!("  Boot Information");
        println!("===========================================");
        println!("  Architecture: x86_64");
        println!("  Boot mode: Real → Protected → Long");
        println!("  VGA display: 80x25 text mode @ 0xB8000");
        println!("  AHCI Controller: Enabled (SATA disk I/O)");
        println!("  ATAPI CD-ROM: Enabled (ISO access)");
        println!("  ISO: {} MB", iso_info.size_mb);
        println!("  Disk: {} GB @ AHCI port 0", self.disk_size_gb);
        println!("===========================================");
        println!();

        println!("Starting CentOS/RHEL boot sequence...");

        match service.boot_x86_kernel() {
            Ok(result) => {
                println!();
                println!("✓ Boot sequence completed");
                println!("  Result: {:?}", result);

                println!();
                println!("===========================================");
                println!("  VGA Display Output");
                println!("===========================================");

                match service.get_vga_display() {
                    Ok(vga_output) => {
                        println!("{}", vga_output);
                    }
                    Err(e) => {
                        println!("VGA display not available: {}", e);
                    }
                }

                println!("===========================================");
                println!("  Installation Status");
                println!("===========================================");
                println!("✓ All storage devices initialized:");
                println!("  - AHCI SATA controller (disk I/O)");
                println!("  - ATAPI CD-ROM (ISO access)");
                println!("  - VGA display (installer interface)");
                println!();
                println!("The CentOS/RHEL installer interface should be visible");
                println!("above in the VGA Display Output section.");
                println!();
                println!("CentOS/RHEL Installation Notes:");
                println!("  - CentOS/RHEL requires 25GB minimum disk space");
                println!("  - 4GB RAM recommended for smooth installation");
                println!("  - Boot files loaded from ISO");
                println!();
                println!("If VGA display is empty:");
                println!("  - The boot process may take longer");
                println!("  - Check the logs for more details");
                println!("===========================================");

                Ok(InstallResult {
                    disk_path: disk_path.to_string_lossy().to_string(),
                    disk_size_gb: self.disk_size_gb,
                    iso_size_mb: iso_info.size_mb,
                    kernel_loaded: true,
                    boot_complete: true,
                    instructions_executed: 0,
                    final_mode: format!("{:?}", result),
                    vga_output_saved: true,
                })
            }
            Err(e) => Err(format!("Boot failed: {}", e)),
        }
    }

    /// Extract kernel from CentOS/RHEL ISO
    fn extract_kernel(&self) -> Result<(), String> {
        use std::process::Command;

        let extract_dir = "/tmp/centos_iso_extracted";
        std::fs::create_dir_all(extract_dir)
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        println!("  Attempting to extract kernel using 7z...");

        let seven_z_check = Command::new("7z").arg("-h").output();
        let use_7z = seven_z_check.is_ok();

        if use_7z {
            let kernel_dst = "/tmp/centos_iso_extracted/centos_vmlinuz";

            let list_output = Command::new("7z")
                .args(&["l", self.iso_path.to_str().unwrap()])
                .output()
                .map_err(|e| format!("Failed to list ISO contents: {}", e))?;

            let list_str = String::from_utf8_lossy(&list_output.stdout);

            let kernel_path_in_iso = if list_str.contains("isolinux/vmlinuz") {
                "isolinux/vmlinuz"
            } else if list_str.contains("images/pxeboot/vmlinuz") {
                "images/pxeboot/vmlinuz"
            } else {
                return Err(
                    "Kernel not found in ISO. Expected isolinux/vmlinuz or images/pxeboot/vmlinuz"
                        .to_string(),
                );
            };

            println!("  Found kernel at: {}", kernel_path_in_iso);

            let extract_output = Command::new("7z")
                .args(&[
                    "e",
                    "-y",
                    self.iso_path.to_str().unwrap(),
                    "-o/tmp/centos_iso_extracted",
                    kernel_path_in_iso,
                ])
                .output()
                .map_err(|e| format!("Failed to extract kernel: {}", e))?;

            if !extract_output.status.success() {
                return Err(format!(
                    "Failed to extract kernel: {:?}",
                    String::from_utf8_lossy(&extract_output.stderr)
                ));
            }

            let extracted_name = kernel_path_in_iso.split('/').last().unwrap();
            let extracted_path = format!("/tmp/centos_iso_extracted/{}", extracted_name);

            if Path::new(&extracted_path).exists() && extracted_name != "centos_vmlinuz" {
                std::fs::rename(&extracted_path, kernel_dst)
                    .map_err(|e| format!("Failed to rename kernel: {}", e))?;
            }
        } else {
            println!("  7z not available, using hdiutil...");

            println!("  Mounting ISO...");
            let mount_output = Command::new("hdiutil")
                .args(&[
                    "attach",
                    "-readonly",
                    "-mountpoint",
                    "/tmp/centos_iso_mounted",
                    self.iso_path.to_str().unwrap(),
                ])
                .output()
                .map_err(|e| format!("Failed to mount ISO: {}", e))?;

            if !mount_output.status.success() {
                return Err(format!(
                    "Failed to mount ISO: {:?}",
                    String::from_utf8_lossy(&mount_output.stderr)
                ));
            }

            println!("  Extracting kernel files...");
            let kernel_src = "/tmp/centos_iso_mounted/isolinux/vmlinuz";
            let kernel_dst = "/tmp/centos_iso_extracted/centos_vmlinuz";

            let copy_result = Command::new("cp")
                .args(&[kernel_src, kernel_dst])
                .output()
                .map_err(|e| format!("Failed to copy kernel: {}", e))?;

            if !copy_result.status.success() {
                let alt_path = "/tmp/centos_iso_mounted/images/pxeboot/vmlinuz";
                if Path::new(alt_path).exists() {
                    Command::new("cp")
                        .args(&[alt_path, kernel_dst])
                        .output()
                        .map_err(|e| format!("Failed to copy kernel (alt): {}", e))?;
                } else {
                    let _ = Command::new("hdiutil")
                        .args(&["detach", "/tmp/centos_iso_mounted"])
                        .output();

                    return Err(
                        "Kernel not found in ISO. Tried isolinux/vmlinuz, images/pxeboot/vmlinuz"
                            .to_string(),
                    );
                }
            }

            println!("  Unmounting ISO...");
            let _ = Command::new("hdiutil")
                .args(&["detach", "/tmp/centos_iso_mounted"])
                .output();
        }

        let kernel_dst = "/tmp/centos_iso_extracted/centos_vmlinuz";
        if Path::new(kernel_dst).exists() {
            let metadata = std::fs::metadata(kernel_dst)
                .map_err(|e| format!("Failed to verify kernel: {}", e))?;
            println!("  Kernel size: {} MB", metadata.len() / 1024 / 1024);
        }

        Ok(())
    }
}

/// Installation result
#[derive(Debug, Clone)]
pub struct InstallResult {
    pub disk_path: String,
    pub disk_size_gb: u64,
    pub iso_size_mb: u64,
    pub kernel_loaded: bool,
    pub boot_complete: bool,
    pub instructions_executed: usize,
    pub final_mode: String,
    pub vga_output_saved: bool,
}
