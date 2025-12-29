//! 统一加速管理器
//!
//! 提供完整的硬件加速集成，包括虚拟化后端、SMMU、NUMA优化和vCPU亲和性管理

#![cfg(feature = "smmu")]

use crate::smmu::SmmuManager;
use crate::vcpu_affinity::{CPUTopology, VCPUAffinityManager};
use std::sync::Arc;

use vm_core::VmError;
use vm_core::error::CoreError;

/// 加速管理器错误类型
pub type AccelManagerError = VmError;

/// 统一加速管理器
///
/// 管理所有硬件虚拟化加速功能，包括：
/// - 虚拟化后端选择 (KVM/HVF/WHPX)
/// - SMMU (IOMMU) 集成
/// - NUMA 优化
/// - vCPU 亲和性管理
///
/// # 示例
///
/// ```ignore
/// use vm_accel::accel::AccelerationManager;
///
/// // 创建加速管理器
/// let mut manager = AccelerationManager::new()?;
///
/// // 设置完整加速栈
/// manager.setup_full_acceleration()?;
///
/// // 创建 vCPU 并设置亲和性
/// manager.create_vcpu_with_affinity(0)?;
/// ```
pub struct AccelerationManager {
    /// CPU 拓扑信息
    topology: Arc<CPUTopology>,

    /// vCPU 亲和性管理器
    #[cfg(not(any(target_os = "windows", target_os = "ios")))]
    vcpu_affinity: Option<VCPUAffinityManager>,

    /// SMMU 管理器 (可选特性)
    smmu: Option<SmmuManager>,

    /// NUMA 优化器
    numa_enabled: bool,

    /// 是否已初始化
    initialized: bool,
}

impl AccelerationManager {
    /// 创建新的加速管理器
    ///
    /// 检测系统拓扑并准备加速组件。
    ///
    /// # 返回值
    ///
    /// 成功返回管理器实例，失败返回错误。
    ///
    /// # 示例
    ///
    /// ```
    /// use vm_accel::accel::AccelerationManager;
    ///
    /// let manager = AccelerationManager::new();
    /// ```
    pub fn new() -> Result<Self, AccelManagerError> {
        log::info!("Initializing Acceleration Manager");

        let topology = Arc::new(CPUTopology::detect());

        log::info!("Detected CPU topology:");
        log::info!("  Total CPUs: {}", topology.total_cpus);
        log::info!("  NUMA nodes: {}", topology.numa_nodes);

        Ok(Self {
            topology,
            #[cfg(not(any(target_os = "windows", target_os = "ios")))]
            vcpu_affinity: None,
            smmu: None,
            numa_enabled: false,
            initialized: false,
        })
    }

    /// 初始化 SMMU
    ///
    /// 创建并初始化 ARM SMMUv3 管理器（如果硬件支持）。
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`，失败返回错误。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// manager.init_smmu()?;
    /// ```
    pub fn init_smmu(&mut self) -> Result<(), AccelManagerError> {
        if !self.initialized {
            return Err(VmError::Core(CoreError::InvalidState {
                message: "Manager not initialized".to_string(),
                current: "not_initialized".to_string(),
                expected: "initialized".to_string(),
            }));
        }

        log::info!("Initializing SMMU");

        // 检查 SMMU 是否可用
        if !SmmuManager::is_available() {
            log::warn!("SMMU hardware not detected, SMMU will run in emulated mode");
        }

        let smmu = SmmuManager::new();
        smmu.init().map_err(|e| {
            VmError::Core(CoreError::Internal {
                message: format!("SMMU initialization failed: {:?}", e),
                module: "vm-accel::AccelerationManager".to_string(),
            })
        })?;

        self.smmu = Some(smmu);

        log::info!("SMMU initialized successfully");
        Ok(())
    }

    /// 启用 NUMA 优化
    ///
    /// 启用 NUMA 感知的内存分配和 vCPU 调度。
    ///
    /// # 参数
    ///
    /// * `numa_nodes` - NUMA 节点数量
    ///
    /// # 示例
    ///
    /// ```ignore
    /// manager.enable_numa(2)?;
    /// ```
    pub fn enable_numa(&mut self, numa_nodes: usize) -> Result<(), AccelManagerError> {
        if numa_nodes == 0 || numa_nodes > self.topology.numa_nodes {
            return Err(VmError::Core(CoreError::InvalidParameter {
                name: "numa_nodes".to_string(),
                value: numa_nodes.to_string(),
                message: format!(
                    "Invalid NUMA node count (must be 1..{}, got {})",
                    self.topology.numa_nodes, numa_nodes
                ),
            }));
        }

        self.numa_enabled = true;

        log::info!("NUMA optimization enabled with {} nodes", numa_nodes);
        Ok(())
    }

    /// 禁用 NUMA 优化
    pub fn disable_numa(&mut self) {
        self.numa_enabled = false;
        log::info!("NUMA optimization disabled");
    }

    /// 初始化 vCPU 亲和性管理器
    ///
    /// 创建 vCPU 亲和性管理器用于优化 vCPU 到物理 CPU 的映射。
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`，失败返回错误。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// manager.init_vcpu_affinity()?;
    /// ```
    #[cfg(not(any(target_os = "windows", target_os = "ios")))]
    pub fn init_vcpu_affinity(&mut self) -> Result<(), AccelManagerError> {
        log::info!("Initializing vCPU affinity manager");

        let affinity_manager = VCPUAffinityManager::new_with_topology(self.topology.clone());

        self.vcpu_affinity = Some(affinity_manager);

        log::info!("vCPU affinity manager initialized");
        Ok(())
    }

    /// 初始化 vCPU 亲和性 - 存根实现 (Windows/iOS)
    #[cfg(any(target_os = "windows", target_os = "ios"))]
    pub fn init_vcpu_affinity(&mut self) -> Result<(), AccelManagerError> {
        log::warn!("vCPU affinity not fully supported on this platform");
        Ok(())
    }

    /// 设置完整加速栈
    ///
    /// 一次性初始化所有可用的加速功能：
    /// - vCPU 亲和性管理
    /// - NUMA 优化
    /// - SMMU (如果可用)
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`，失败返回错误。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// manager.setup_full_acceleration()?;
    /// ```
    pub fn setup_full_acceleration(&mut self) -> Result<(), AccelManagerError> {
        log::info!("Setting up full acceleration stack");

        self.initialized = true;

        // 初始化 vCPU 亲和性
        #[cfg(not(any(target_os = "windows", target_os = "ios")))]
        {
            if let Err(e) = self.init_vcpu_affinity() {
                log::warn!("Failed to initialize vCPU affinity: {:?}", e);
            }
        }

        // 启用 NUMA (如果系统有多个节点)
        if self.topology.numa_nodes > 1 {
            log::info!("Multiple NUMA nodes detected, enabling NUMA optimization");
            self.enable_numa(self.topology.numa_nodes)?;
        }

        // 初始化 SMMU (如果可用)
        {
            if SmmuManager::is_available() {
                if let Err(e) = self.init_smmu() {
                    log::warn!("Failed to initialize SMMU: {:?}", e);
                }
            } else {
                log::info!("SMMU hardware not detected");
            }
        }

        log::info!("Full acceleration stack setup complete");
        Ok(())
    }

    /// 配置 vCPU 亲和性
    ///
    /// 为虚拟机的 vCPU 设置物理 CPU 亲和性和 NUMA 节点偏好。
    ///
    /// # 参数
    ///
    /// * `vcpu_count` - vCPU 数量
    ///
    /// # 返回值
    ///
    /// 成功返回 `Ok(())`，失败返回错误。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// manager.configure_vcpu_affinity(4)?;
    /// ```
    #[cfg(not(any(target_os = "windows", target_os = "ios")))]
    pub fn configure_vcpu_affinity(&self, vcpu_count: usize) -> Result<(), AccelManagerError> {
        let affinity_manager = self.vcpu_affinity.as_ref().ok_or_else(|| {
            VmError::Core(CoreError::InvalidState {
                message: "vCPU affinity manager not initialized".to_string(),
                current: "no_affinity".to_string(),
                expected: "affinity_initialized".to_string(),
            })
        })?;

        affinity_manager
            .configure_vcpu_affinity(vcpu_count)
            .map_err(|e| {
                VmError::Core(CoreError::Internal {
                    message: format!("Failed to configure vCPU affinity: {}", e),
                    module: "vm-accel::AccelerationManager".to_string(),
                })
            })?;

        log::info!("Configured affinity for {} vCPUs", vcpu_count);
        Ok(())
    }

    /// 配置 vCPU 亲和性 - 存根实现 (Windows/iOS)
    #[cfg(any(target_os = "windows", target_os = "ios"))]
    pub fn configure_vcpu_affinity(&self, vcpu_count: usize) -> Result<(), AccelManagerError> {
        log::info!(
            "Configured affinity for {} vCPUs (affinity not supported on this platform)",
            vcpu_count
        );
        Ok(())
    }

    /// 获取 CPU 拓扑信息
    ///
    /// 返回系统的 CPU 拓扑结构，包括 NUMA 节点和缓存信息。
    ///
    /// # 返回值
    ///
    /// 返回 CPU 拓扑的克隆。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let topology = manager.get_topology();
    /// println!("Total CPUs: {}", topology.total_cpus);
    /// ```
    pub fn get_topology(&self) -> CPUTopology {
        (*self.topology).clone()
    }

    /// 检查 NUMA 是否已启用
    pub fn is_numa_enabled(&self) -> bool {
        self.numa_enabled
    }

    /// 检查 SMMU 是否已初始化
    pub fn is_smmu_initialized(&self) -> bool {
        self.smmu
            .as_ref()
            .map(|s| s.is_initialized())
            .unwrap_or(false)
    }

    /// 获取 SMMU 管理器引用 (如果已初始化)
    pub fn get_smmu(&self) -> Option<&SmmuManager> {
        self.smmu.as_ref()
    }

    /// 生成诊断报告
    ///
    /// 生成包含所有加速组件状态的详细报告。
    ///
    /// # 返回值
    ///
    /// 返回格式化的诊断报告字符串。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let report = manager.diagnostic_report();
    /// println!("{}", report);
    /// ```
    pub fn diagnostic_report(&self) -> String {
        let mut report = "=== Acceleration Manager Diagnostic Report ===\n\n".to_string();

        // CPU 拓扑信息
        report.push_str("CPU Topology:\n");
        report.push_str(&format!("  Total CPUs: {}\n", self.topology.total_cpus));
        report.push_str(&format!("  NUMA Nodes: {}\n", self.topology.numa_nodes));

        for (node_id, cpus) in &self.topology.cpus_per_node {
            report.push_str(&format!("  Node {} CPUs: {:?}\n", node_id, cpus));
        }

        report.push('\n');

        // NUMA 状态
        report.push_str(&format!(
            "NUMA Optimization: {}\n",
            if self.numa_enabled {
                "Enabled"
            } else {
                "Disabled"
            }
        ));

        // SMMU 状态
        {
            report.push_str(&format!(
                "SMMU: {}\n",
                if self.is_smmu_initialized() {
                    "Initialized"
                } else {
                    "Not initialized"
                }
            ));

            if let Some(ref smmu) = self.smmu {
                report.push_str(&format!(
                    "  Attached devices: {}\n",
                    smmu.attached_device_count()
                ));

                let devices = smmu.list_attached_devices();
                if !devices.is_empty() {
                    report.push_str("  Device list:\n");
                    for device in devices {
                        report.push_str(&format!("    - {}\n", device));
                    }
                }
            }
        }

        // vCPU 亲和性状态
        #[cfg(not(any(target_os = "windows", target_os = "ios")))]
        {
            if let Some(ref affinity) = self.vcpu_affinity {
                report.push_str("\nvCPU Affinity Configuration:\n");
                report.push_str(&affinity.diagnostic_report());
            }
        }

        report.push_str("=============================\n");

        report
    }
}

impl Default for AccelerationManager {
    fn default() -> Self {
        Self::new().expect("Failed to create AccelerationManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acceleration_manager_creation() {
        let manager = AccelerationManager::new();
        assert!(manager.is_ok());

        let manager = manager.unwrap();
        assert!(!manager.is_numa_enabled());
        assert!(!manager.is_smmu_initialized());
    }

    #[test]
    fn test_numa_enable_disable() {
        let mut manager = AccelerationManager::new().unwrap();

        // Test invalid node counts
        assert!(manager.enable_numa(0).is_err());
        assert!(manager.enable_numa(999).is_err());

        // Test valid NUMA enablement
        let node_count = manager.topology.numa_nodes.max(1);
        assert!(manager.enable_numa(node_count).is_ok());
        assert!(manager.is_numa_enabled());

        // Test disable
        manager.disable_numa();
        assert!(!manager.is_numa_enabled());
    }

    #[test]
    fn test_vcpu_affinity_configuration() {
        let mut manager = AccelerationManager::new().unwrap();

        #[cfg(not(any(target_os = "windows", target_os = "ios")))]
        {
            // Initialize affinity first
            assert!(manager.init_vcpu_affinity().is_ok());

            // Configure vCPUs
            assert!(manager.configure_vcpu_affinity(4).is_ok());

            // Verify topology
            let topology = manager.get_topology();
            assert!(topology.total_cpus > 0);
        }

        #[cfg(any(target_os = "windows", target_os = "ios"))]
        {
            // Should succeed on Windows/iOS (stub implementation)
            assert!(manager.configure_vcpu_affinity(4).is_ok());
        }
    }

    #[test]
    fn test_full_acceleration_setup() {
        let mut manager = AccelerationManager::new().unwrap();

        assert!(manager.setup_full_acceleration().is_ok());
        assert!(manager.initialized);

        // NUMA may or may not be enabled depending on system
        let report = manager.diagnostic_report();
        assert!(report.contains("Acceleration Manager"));
        assert!(report.contains("CPU Topology"));
    }

    #[test]
    fn test_diagnostic_report() {
        let manager = AccelerationManager::new().unwrap();

        let report = manager.diagnostic_report();

        assert!(report.contains("Acceleration Manager"));
        assert!(report.contains("CPU Topology"));
        assert!(report.contains("Total CPUs"));
        assert!(report.contains("NUMA"));
    }

    #[test]
    fn test_topology_access() {
        let manager = AccelerationManager::new().unwrap();

        let topology = manager.get_topology();
        assert!(topology.total_cpus > 0);
        assert!(topology.numa_nodes > 0);
        assert!(!topology.cpu_to_node.is_empty());
    }

    #[test]
    fn test_smmu_initialization() {
        let mut manager = AccelerationManager::new().unwrap();

        // SMMU initialization may fail if hardware not available
        let result = manager.init_smmu();

        if SmmuManager::is_available() {
            // If hardware is available, init should succeed
            assert!(result.is_ok());
            assert!(manager.is_smmu_initialized());
            assert!(manager.get_smmu().is_some());
        } else {
            // If no hardware, may fail or warn
            log::warn!("SMMU initialization test skipped (hardware not available)");
        }
    }
}
