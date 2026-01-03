//! NUMA 感知的 KVM 加速器
//!
//! 集成 NUMA 优化和 vCPU 亲和性管理的 KVM 加速器实现。
//!
//! ## 功能
//!
//! - NUMA 感知内存分配
//! - vCPU 到物理 CPU 的亲和性绑定
//! - 跨节点访问优化
//! - 负载均衡和性能监控
//!
//! ## 使用示例
//!
//! ```ignore
//! use vm_accel::kvm_numa::NUMAKvmAccelerator;
//!
//! let mut kvm = NUMAKvmAccelerator::new()?;
//! kvm.init()?;
//!
//! // 创建 vCPU 时自动应用 NUMA 和亲和性优化
//! kvm.create_vcpu(0)?;
//! ```

use std::sync::{Arc, Mutex};

use super::{Accel, AccelError};
use super::{
    kvm::{AccelKvm, KvmVcpu},
    numa_optimizer::{MemoryAllocationStrategy, NUMANodeStats, NUMAOptimizer},
    vcpu_affinity::{CPUTopology, VCPUAffinityManager, VCPUConfig},
};

/// NUMA 感知的 KVM 加速器
///
/// 在标准 KVM 加速器基础上添加：
/// - NUMA 感知内存分配
/// - vCPU 到物理 CPU 的智能绑定
/// - 跨节点访问统计和优化
///
/// # 标识
/// NUMA 优化的 KVM 加速器类
pub struct NUMAKvmAccelerator {
    /// 基础 KVM 加速器
    kvm: AccelKvm,
    /// NUMA 优化器（可选）
    numa_optimizer: Option<Arc<NUMAOptimizer>>,
    /// vCPU 亲和性管理器（可选）
    affinity_manager: Option<Arc<VCPUAffinityManager>>,
    /// CPU 拓扑信息
    topology: Option<Arc<CPUTopology>>,
    /// 是否启用 NUMA 优化
    numa_enabled: bool,
    /// 是否启用 vCPU 亲和性
    affinity_enabled: bool,
}

impl NUMAKvmAccelerator {
    /// 创建新的增强 KVM 加速器
    ///
    /// # 参数
    ///
    /// * `enable_numa` - 是否启用 NUMA 优化
    /// * `enable_affinity` - 是否启用 vCPU 亲和性
    ///
    /// # 返回值
    ///
    /// 返回增强 KVM 加速器实例
    ///
    /// # 示例
    ///
    /// ```
    /// use vm_accel::kvm_enhanced::NUMAKvmAccelerator;
    ///
    /// // 启用所有优化
    /// let kvm = NUMAKvmAccelerator::new(true, true)?;
    ///
    /// // 仅启用 NUMA 优化
    /// let kvm = NUMAKvmAccelerator::new(true, false)?;
    /// ```
    pub fn new(enable_numa: bool, enable_affinity: bool) -> Result<Self, AccelError> {
        log::info!(
            "Creating enhanced KVM accelerator (NUMA: {}, affinity: {})",
            enable_numa,
            enable_affinity
        );

        // 检测 CPU 拓扑
        let topology = if enable_numa || enable_affinity {
            Some(Arc::new(CPUTopology::detect()?))
        } else {
            None
        };

        // 创建 NUMA 优化器
        let numa_optimizer = if enable_numa {
            if let Some(ref topo) = topology {
                let affinity_mgr =
                    Arc::new(VCPUAffinityManager::new_with_topology(Arc::clone(topo)));
                Some(Arc::new(NUMAOptimizer::new(
                    Arc::clone(topo),
                    affinity_mgr,
                    1024 * 1024 * 1024, // 1GB per node
                )))
            } else {
                None
            }
        } else {
            None
        };

        // 创建 vCPU 亲和性管理器
        let affinity_manager = if enable_affinity {
            if let Some(ref topo) = topology {
                Some(Arc::new(VCPUAffinityManager::new_with_topology(
                    Arc::clone(topo),
                )))
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            kvm: AccelKvm::new(),
            numa_optimizer,
            affinity_manager,
            topology,
            numa_enabled: enable_numa,
            affinity_enabled: enable_affinity,
        })
    }

    /// 获取 NUMA 节点统计信息
    ///
    /// # 参数
    ///
    /// * `node_id` - NUMA 节点 ID
    ///
    /// # 返回值
    ///
    /// 返回节点统计信息，如果 NUMA 未启用则返回 None。
    pub fn get_numa_stats(&self, node_id: usize) -> Option<NUMANodeStats> {
        if let Some(ref optimizer) = self.numa_optimizer {
            optimizer.get_node_stats(node_id)
        } else {
            None
        }
    }

    /// 获取所有 NUMA 节点统计信息
    ///
    /// # 返回值
    ///
    /// 返回所有节点的统计信息向量
    pub fn get_all_numa_stats(&self) -> Vec<NUMANodeStats> {
        if let Some(ref optimizer) = self.numa_optimizer {
            optimizer.get_all_stats()
        } else {
            vec![]
        }
    }

    /// 获取 vCPU 亲和性配置
    ///
    /// # 参数
    ///
    /// * `vcpu_id` - vCPU ID
    ///
    /// # 返回值
    ///
    /// 返回 vCPU 配置，如果亲和性未启用则返回 None。
    pub fn get_vcpu_affinity(&self, vcpu_id: u32) -> Option<VCPUConfig> {
        if let Some(ref manager) = self.affinity_manager {
            manager.get_vcpu_config(vcpu_id)
        } else {
            None
        }
    }

    /// 设置内存分配策略
    ///
    /// # 参数
    ///
    /// * `strategy` - 内存分配策略
    pub fn set_memory_strategy(&mut self, strategy: MemoryAllocationStrategy) {
        if let Some(ref mut optimizer) = self.numa_optimizer {
            optimizer.set_strategy(strategy);
            log::info!("Memory allocation strategy set to {:?}", strategy);
        }
    }

    /// 更新 NUMA 统计信息
    ///
    /// 定期调用此方法以更新 NUMA 节点的统计信息
    pub fn update_numa_stats(&self) {
        if let Some(ref optimizer) = self.numa_optimizer {
            optimizer.update_stats();
            log::trace!("NUMA stats updated");
        }
    }

    /// 优化 vCPU 放置
    ///
    /// 根据当前系统负载重新优化 vCPU 到物理 CPU 的映射
    ///
    /// # 返回值
    ///
    /// 成功返回优化的 vCPU 数量，失败返回错误
    pub fn optimize_vcpu_placement(&self) -> Result<usize, String> {
        if !self.affinity_enabled {
            return Ok(0);
        }

        log::info!("Optimizing vCPU placement...");

        let affinity = self
            .affinity_manager
            .as_ref()
            .ok_or_else(|| "Affinity manager not initialized".to_string())?;

        let optimized = affinity
            .rebalance_vcpus()
            .map_err(|e| format!("Failed to rebalance vCPUs: {:?}", e))?;

        log::info!("Optimized {} vCPUs", optimized);
        Ok(optimized)
    }

    /// 获取性能指标
    ///
    /// # 返回值
    ///
    /// 返回性能指标字典
    pub fn get_performance_metrics(&self) -> std::collections::HashMap<String, f64> {
        let mut metrics = std::collections::HashMap::new();

        // 添加 NUMA 相关指标
        if let Some(ref optimizer) = self.numa_optimizer {
            let stats = optimizer.get_all_stats();
            for stat in &stats {
                let prefix = format!("numa_node_{}", stat.node_id);
                metrics.insert(format!("{}_cpu_usage", prefix), stat.cpu_usage);
                metrics.insert(format!("{}_memory_usage", prefix), stat.memory_usage);
                metrics.insert(
                    format!("{}_local_access_rate", prefix),
                    stat.local_access_rate(),
                );
                metrics.insert(
                    format!("{}_cross_node_accesses", prefix),
                    stat.cross_node_accesses as f64,
                );
            }
        }

        // 添加 vCPU 亲和性指标
        if let Some(ref affinity) = self.affinity_manager {
            if let Ok(topo_stats) = affinity.get_topology_stats() {
                metrics.insert("total_cpus", topo_stats.total_cpus as f64);
                metrics.insert("numa_nodes", topo_stats.numa_nodes as f64);
                metrics.insert("cores_per_node", topo_stats.cores_per_node as f64);
            }
        }

        metrics
    }

    /// 获取拓扑信息
    ///
    /// # 返回值
    ///
    /// 返回 CPU 拓扑信息，如果未检测则返回 None
    pub fn get_topology(&self) -> Option<&CPUTopology> {
        self.topology.as_ref().map(|arc| arc.as_ref())
    }

    /// 检查 NUMA 是否启用
    pub fn is_numa_enabled(&self) -> bool {
        self.numa_enabled
    }

    /// 检查 vCPU 亲和性是否启用
    pub fn is_affinity_enabled(&self) -> bool {
        self.affinity_enabled
    }
}

impl Default for NUMAKvmAccelerator {
    fn default() -> Self {
        Self::new(false, false).unwrap_or_else(|_| {
            // 如果创建失败，返回禁用所有优化的实例
            Self {
                kvm: AccelKvm::new(),
                numa_optimizer: None,
                affinity_manager: None,
                topology: None,
                numa_enabled: false,
                affinity_enabled: false,
            }
        })
    }
}

// 实现 Accel trait 以保持与标准接口兼容
impl Accel for NUMAKvmAccelerator {
    fn init(&mut self) -> Result<(), AccelError> {
        // 初始化基础 KVM
        self.kvm.init()?;

        log::info!(
            "Enhanced KVM accelerator initialized (NUMA: {}, affinity: {})",
            self.numa_enabled,
            self.affinity_enabled
        );

        Ok(())
    }

    fn create_vcpu(&mut self, id: u32) -> Result<(), AccelError> {
        // 创建基础 vCPU
        self.kvm.create_vcpu(id)?;

        // 应用 vCPU 亲和性（如果启用）
        if let Some(ref affinity) = self.affinity_manager {
            if let Some(ref topology) = self.topology {
                // 自动配置 vCPU 亲和性
                let config = VCPUConfig {
                    vcpu_id: id,
                    preferred_cores: None, // 让系统自动选择
                    node_id: None,         // 让系统自动选择
                    isol_cpus: false,
                };

                affinity.set_vcpu_config(config).map_err(|e| {
                    AccelError::CreateVcpuFailed(format!("Failed to set vCPU affinity: {:?}", e))
                })?;

                log::debug!("vCPU {} affinity configured automatically", id);
            }
        }

        Ok(())
    }

    fn map_memory(
        &mut self,
        guest_addr: u64,
        host_addr: u64,
        size: u64,
        flags: u32,
    ) -> Result<(), AccelError> {
        self.kvm.map_memory(guest_addr, host_addr, size, flags)
    }

    fn run_vcpu(&mut self, vcpu_id: u32, mmu: &mut dyn vm_core::MMU) -> Result<(), AccelError> {
        self.kvm.run_vcpu(vcpu_id, mmu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_kvm_creation() {
        let kvm = NUMAKvmAccelerator::new(false, false);
        assert!(kvm.is_ok());
        let kvm = kvm.expect("Should create KVM");
        assert!(!kvm.is_numa_enabled());
        assert!(!kvm.is_affinity_enabled());
    }

    #[test]
    fn test_enhanced_kvm_with_numa() {
        let kvm = NUMAKvmAccelerator::new(true, false);
        assert!(kvm.is_ok());
        let kvm = kvm.expect("Should create KVM with NUMA");
        assert!(kvm.is_numa_enabled());
        assert!(!kvm.is_affinity_enabled());
    }

    #[test]
    fn test_enhanced_kvm_with_affinity() {
        let kvm = NUMAKvmAccelerator::new(false, true);
        assert!(kvm.is_ok());
        let kvm = kvm.expect("Should create KVM with affinity");
        assert!(!kvm.is_numa_enabled());
        assert!(kvm.is_affinity_enabled());
    }

    #[test]
    fn test_enhanced_kvm_all_features() {
        let kvm = NUMAKvmAccelerator::new(true, true);
        assert!(kvm.is_ok());
        let kvm = kvm.expect("Should create KVM with all features");
        assert!(kvm.is_numa_enabled());
        assert!(kvm.is_affinity_enabled());
    }
}
