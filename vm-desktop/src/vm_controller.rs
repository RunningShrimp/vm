//! VM Controller - Lifecycle Management
//!
//! Handles starting, stopping, pausing, and resuming virtual machines.
//! Manages the integration with vm-core and vm-service.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use log::info;
use tokio::task::JoinHandle;
use vm_core::{ExecMode, GuestArch as Arch, VmConfig as CoreVmConfig};
use vm_service::VmService;

use crate::ipc::{VmConfig, VmInstance, VmState};

/// Enhanced VM instance with actual service integration
pub struct EnhancedVmInstance {
    pub instance: VmInstance,
    pub service: Option<Arc<Mutex<VmService>>>,
    pub config: CoreVmConfig,
    pub kernel_path: Option<PathBuf>,
    pub start_pc: u64,
}

// Custom Clone implementation because VmService doesn't implement Clone
impl Clone for EnhancedVmInstance {
    fn clone(&self) -> Self {
        Self {
            instance: self.instance.clone(),
            service: None, // VmService doesn't implement Clone, so we set to None
            config: self.config.clone(),
            kernel_path: self.kernel_path.clone(),
            start_pc: self.start_pc,
        }
    }
}

/// Central controller for managing VM instances
pub struct VmController {
    vms: Arc<Mutex<HashMap<String, EnhancedVmInstance>>>,
    vm_tasks: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    vm_configs: Arc<Mutex<HashMap<String, CoreVmConfig>>>,
}

impl VmController {
    /// Create a new VM controller
    pub fn new() -> Self {
        Self {
            vms: Arc::new(Mutex::new(HashMap::new())),
            vm_tasks: Arc::new(Mutex::new(HashMap::new())),
            vm_configs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// List all registered VMs
    pub fn list_vms(&self) -> Result<Vec<VmInstance>, String> {
        let vms = self.vms.lock().map_err(|e| e.to_string())?;
        Ok(vms.values().map(|vm| vm.instance.clone()).collect())
    }

    /// Get a specific VM by ID
    pub fn get_vm(&self, id: &str) -> Result<Option<VmInstance>, String> {
        let vms = self.vms.lock().map_err(|e| e.to_string())?;
        Ok(vms.get(id).map(|vm| vm.instance.clone()))
    }

    /// Get enhanced VM instance with service
    pub fn get_enhanced_vm(&self, id: &str) -> Result<Option<EnhancedVmInstance>, String> {
        let vms = self.vms.lock().map_err(|e| e.to_string())?;
        Ok(vms.get(id).cloned())
    }

    /// Convert GUI config to core VM config
    fn gui_config_to_core(&self, config: &VmConfig) -> CoreVmConfig {
        CoreVmConfig {
            guest_arch: Arch::Riscv64, // Default to RISC-V 64
            vcpu_count: config.cpu_count as usize,
            memory_size: (config.memory_mb * 1024 * 1024) as usize,
            exec_mode: ExecMode::Interpreter, // Default to interpreter
            kernel_path: None,                // Will be set separately
            initrd_path: None,
        }
    }

    /// Create a new VM configuration
    pub fn create_vm(&self, config: VmConfig) -> Result<VmInstance, String> {
        let core_config = self.gui_config_to_core(&config);

        let vm = VmInstance {
            id: config.id.clone(),
            name: config.name.clone(),
            state: VmState::Stopped,
            cpu_count: config.cpu_count,
            memory_mb: config.memory_mb,
            disk_gb: config.disk_gb,
            display_mode: config.display_mode,
        };

        let enhanced_vm = EnhancedVmInstance {
            instance: vm.clone(),
            service: None,
            config: core_config.clone(),
            kernel_path: None,
            start_pc: 0x80000000, // Default start PC for RISC-V
        };

        let mut vms = self.vms.lock().map_err(|e| e.to_string())?;
        if vms.contains_key(&vm.id) {
            return Err("VM already exists".to_string());
        }

        let mut configs = self.vm_configs.lock().map_err(|e| e.to_string())?;
        configs.insert(vm.id.clone(), core_config);
        vms.insert(vm.id.clone(), enhanced_vm);

        info!("Created VM: {} ({})", vm.name, vm.id);
        Ok(vm)
    }

    /// Set kernel path for a VM
    pub fn set_kernel_path(&self, id: &str, path: PathBuf) -> Result<(), String> {
        let mut vms = self.vms.lock().map_err(|e| e.to_string())?;
        let vm = vms.get_mut(id).ok_or("VM not found")?;
        vm.kernel_path = Some(path);
        info!("Set kernel path for VM: {}", id);
        Ok(())
    }

    /// Set start PC for a VM
    pub fn set_start_pc(&self, id: &str, start_pc: u64) -> Result<(), String> {
        let mut vms = self.vms.lock().map_err(|e| e.to_string())?;
        let vm = vms.get_mut(id).ok_or("VM not found")?;
        vm.start_pc = start_pc;
        info!("Set start PC for VM {}: 0x{:x}", id, start_pc);
        Ok(())
    }

    /// Start a VM
    pub async fn start_vm(&self, id: &str) -> Result<(), String> {
        // 先获取VM信息，但不持有锁跨await点
        let (vm_config, kernel_path, start_pc) = {
            let vms = self.vms.lock().map_err(|e| e.to_string())?;
            let vm = vms.get(id).ok_or("VM not found")?;
            if vm.instance.state != VmState::Stopped {
                return Err(format!(
                    "VM is not in stopped state: {:?}",
                    vm.instance.state
                ));
            }

            // Check if kernel path is set
            if vm.kernel_path.is_none() {
                return Err("Kernel path not set. Use set_kernel_path first.".to_string());
            }

            (vm.config.clone(), vm.kernel_path.clone(), vm.start_pc)
        };

        info!("Starting VM: {}", id);

        // Create and initialize the VM service
        let mut vm_service = VmService::new(vm_config, None)
            .await
            .map_err(|e| format!("Failed to create VM service: {}", e))?;

        // Load kernel if path is set
        if let Some(ref kernel_path) = kernel_path {
            let kernel_path_str = kernel_path.to_string_lossy();
            vm_service
                .load_kernel(&kernel_path_str, start_pc)
                .map_err(|e| format!("Failed to load kernel: {}", e))?;
        }

        // 更新VM状态
        {
            let mut vms = self.vms.lock().map_err(|e| e.to_string())?;
            let vm = vms.get_mut(id).ok_or("VM not found")?;
            // Store the service in the VM instance
            vm.service = Some(Arc::new(Mutex::new(vm_service)));
            vm.instance.state = VmState::Running;
        }

        // Create a background task to monitor the VM
        let vm_id = id.to_string();
        let vm_tasks = self.vm_tasks.clone();
        let task = tokio::spawn(async move {
            // Simulate VM monitoring
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                // In a real implementation, this would check VM health
                // and handle any VM-specific background tasks

                // Check if the VM is still supposed to be running
                if let Ok(tasks) = vm_tasks.lock() {
                    if !tasks.contains_key(&vm_id) {
                        break; // VM was stopped
                    }
                } else {
                    // 如果无法获取锁，退出循环
                    break;
                }
            }
        });

        // Store the task handle
        {
            let mut tasks = self.vm_tasks.lock().map_err(|e| e.to_string())?;
            tasks.insert(id.to_string(), task);
        }

        info!("VM started successfully: {}", id);
        Ok(())
    }

    /// Stop a VM
    pub async fn stop_vm(&self, id: &str) -> Result<(), String> {
        // Request VM service to stop if it exists
        if let Some(service) = {
            let vms = self.vms.lock().map_err(|e| e.to_string())?;
            let vm = vms.get(id).ok_or("VM not found")?;

            vm.service.clone()
        } {
            service.lock().map_err(|e| e.to_string())?.request_stop();
            info!("Requested VM service to stop: {}", id);
        }

        // 更新VM状态
        {
            let mut vms = self.vms.lock().map_err(|e| e.to_string())?;
            let vm = vms.get_mut(id).ok_or("VM not found")?;
            vm.instance.state = VmState::Stopped;
            // Clear the service reference
            vm.service = None;
        }

        // Terminate the VM monitoring task
        if let Some(task) = {
            let mut tasks = self.vm_tasks.lock().map_err(|e| e.to_string())?;
            tasks.remove(id)
        } {
            task.abort();
        }

        info!("VM stopped: {}", id);
        Ok(())
    }

    /// Pause a VM
    pub async fn pause_vm(&self, id: &str) -> Result<(), String> {
        // Request VM service to pause if it exists
        if let Some(service) = {
            let vms = self.vms.lock().map_err(|e| e.to_string())?;
            let vm = vms.get(id).ok_or("VM not found")?;

            if vm.instance.state != VmState::Running {
                return Err("VM is not running".to_string());
            }

            vm.service.clone()
        } {
            service.lock().map_err(|e| e.to_string())?.request_pause();
            info!("Requested VM service to pause: {}", id);
        }

        // 更新VM状态
        {
            let mut vms = self.vms.lock().map_err(|e| e.to_string())?;
            let vm = vms.get_mut(id).ok_or("VM not found")?;
            vm.instance.state = VmState::Paused;
        }

        info!("VM paused: {}", id);
        Ok(())
    }

    /// Resume a paused VM
    pub async fn resume_vm(&self, id: &str) -> Result<(), String> {
        // Request VM service to resume if it exists
        if let Some(service) = {
            let vms = self.vms.lock().map_err(|e| e.to_string())?;
            let vm = vms.get(id).ok_or("VM not found")?;

            if vm.instance.state != VmState::Paused {
                return Err("VM is not paused".to_string());
            }

            vm.service.clone()
        } {
            service.lock().map_err(|e| e.to_string())?.request_resume();
            info!("Requested VM service to resume: {}", id);
        }

        // 更新VM状态
        {
            let mut vms = self.vms.lock().map_err(|e| e.to_string())?;
            let vm = vms.get_mut(id).ok_or("VM not found")?;
            vm.instance.state = VmState::Running;
        }

        info!("VM resumed: {}", id);
        Ok(())
    }

    /// Delete a VM configuration
    pub fn delete_vm(&self, id: &str) -> Result<(), String> {
        let mut vms = self.vms.lock().map_err(|e| e.to_string())?;

        let vm = vms.get(id).ok_or("VM not found")?;
        if vm.instance.state != VmState::Stopped {
            return Err("Cannot delete a running VM".to_string());
        }

        vms.remove(id);

        // Also remove from configs
        let mut configs = self.vm_configs.lock().map_err(|e| e.to_string())?;
        configs.remove(id);

        info!("VM deleted: {}", id);
        Ok(())
    }

    /// Update VM configuration
    pub fn update_vm_config(&self, config: VmConfig) -> Result<VmInstance, String> {
        let mut vms = self.vms.lock().map_err(|e| e.to_string())?;

        let vm = vms.get_mut(&config.id).ok_or("VM not found")?;
        if vm.instance.state != VmState::Stopped {
            return Err("Can only modify stopped VMs".to_string());
        }

        // Update instance properties
        vm.instance.name = config.name.clone();
        vm.instance.cpu_count = config.cpu_count;
        vm.instance.memory_mb = config.memory_mb;
        vm.instance.disk_gb = config.disk_gb;
        vm.instance.display_mode = config.display_mode.clone();

        // Update core config
        let core_config = self.gui_config_to_core(&config);
        vm.config = core_config.clone();

        // Update configs storage
        let mut configs = self.vm_configs.lock().map_err(|e| e.to_string())?;
        configs.insert(config.id.clone(), core_config);

        info!("VM configuration updated: {}", config.id);
        Ok(vm.instance.clone())
    }

    /// Create a snapshot of a running VM
    pub async fn create_snapshot(
        &self,
        id: &str,
        name: String,
        description: String,
    ) -> Result<String, String> {
        let mut vms = self.vms.lock().map_err(|e| e.to_string())?;

        let vm = vms.get_mut(id).ok_or("VM not found")?;
        if vm.instance.state != VmState::Running {
            return Err("Can only create snapshots of running VMs".to_string());
        }

        if let Some(ref mut service) = vm.service {
            let snapshot_id = service
                .lock()
                .map_err(|e| e.to_string())?
                .create_snapshot(name, description)
                .map_err(|e| format!("Failed to create snapshot: {}", e))?;

            info!("Snapshot created for VM {}: {}", id, snapshot_id);
            Ok(snapshot_id)
        } else {
            Err("VM service not available".to_string())
        }
    }

    /// Restore a snapshot
    pub async fn restore_snapshot(&self, id: &str, snapshot_id: &str) -> Result<(), String> {
        let mut vms = self.vms.lock().map_err(|e| e.to_string())?;

        let vm = vms.get_mut(id).ok_or("VM not found")?;

        if let Some(ref mut service) = vm.service {
            service
                .lock()
                .map_err(|e| e.to_string())?
                .restore_snapshot(snapshot_id)
                .map_err(|e| format!("Failed to restore snapshot: {}", e))?;

            info!("Snapshot restored for VM {}: {}", id, snapshot_id);
            Ok(())
        } else {
            Err("VM service not available".to_string())
        }
    }

    /// List snapshots for a VM
    pub async fn list_snapshots(&self, id: &str) -> Result<Vec<String>, String> {
        let vms = self.vms.lock().map_err(|e| e.to_string())?;

        let vm = vms.get(id).ok_or("VM not found")?;

        if let Some(ref service) = vm.service {
            let snapshots = service
                .lock()
                .map_err(|e| e.to_string())?
                .list_snapshots()
                .map_err(|e| format!("Failed to list snapshots: {}", e))?;
            Ok(snapshots)
        } else {
            Err("VM service not available".to_string())
        }
    }

    /// Get console output from a running VM
    pub fn get_console_output(&self, id: &str) -> Result<Vec<String>, String> {
        let vms = self.vms.lock().map_err(|e| e.to_string())?;

        let vm = vms.get(id).ok_or("VM not found")?;

        if vm.instance.state != VmState::Running {
            return Ok(vec![
                "[系统] 虚拟机未运行".to_string(),
                "[提示] 启动虚拟机以查看控制台输出".to_string(),
            ]);
        }

        // In a real implementation, this would fetch actual console output
        // For now, return simulated boot messages
        Ok(vec![
            "[启动] VM Manager v0.1.0".to_string(),
            "[内核] 检测到 CPU: RISC-V 64".to_string(),
            "[内核] 检测到内存: {} MB".replace("{}", &vm.instance.memory_mb.to_string()),
            "[内核] 初始化 MMU...".to_string(),
            "[内核] 初始化中断控制器...".to_string(),
            "[设备] 初始化 VirtIO 设备...".to_string(),
            "[设备]   - VirtIO block device: /dev/vda ({} GB)"
                .replace("{}", &vm.instance.disk_gb.to_string()),
            "[设备]   - VirtIO network device: eth0".to_string(),
            "[成功] 系统启动完成".to_string(),
            "[运行] 正在运行...".to_string(),
        ])
    }
}

impl Default for VmController {
    fn default() -> Self {
        Self::new()
    }
}
