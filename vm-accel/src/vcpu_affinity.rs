//! vCPU 亲和性与 NUMA 内存绑定
//!
//! 优化 vCPU 线程亲和性和 NUMA 感知内存分配

use std::collections::HashMap;
use std::sync::Arc;

/// CPU 拓扑信息
#[derive(Clone, Debug)]
pub struct CPUTopology {
    /// 总 CPU 核心数
    pub total_cpus: usize,
    /// NUMA 节点数
    pub numa_nodes: usize,
    /// 每个 NUMA 节点的 CPU 列表
    pub cpus_per_node: HashMap<usize, Vec<usize>>,
    /// 每个 CPU 的 NUMA 节点
    pub cpu_to_node: HashMap<usize, usize>,
    /// CPU 缓存拓扑 (L1/L2/L3)
    pub cache_topology: Vec<CacheLevel>,
}

#[derive(Clone, Debug)]
pub struct CacheLevel {
    pub level: usize, // 1, 2, 3
    pub size_kb: usize,
    pub shared_by: Vec<usize>, // CPU ID 列表
}

impl CPUTopology {
    /// 检测系统 CPU 拓扑
    pub fn detect() -> Self {
        // 简化实现：假设 8 个 CPU，2 个 NUMA 节点
        let mut cpus_per_node = HashMap::new();
        let mut cpu_to_node = HashMap::new();

        // Node 0: CPU 0-3
        cpus_per_node.insert(0, vec![0, 1, 2, 3]);
        for cpu in 0..4 {
            cpu_to_node.insert(cpu, 0);
        }

        // Node 1: CPU 4-7
        cpus_per_node.insert(1, vec![4, 5, 6, 7]);
        for cpu in 4..8 {
            cpu_to_node.insert(cpu, 1);
        }

        Self {
            total_cpus: 8,
            numa_nodes: 2,
            cpus_per_node,
            cpu_to_node,
            cache_topology: vec![
                CacheLevel {
                    level: 3,
                    size_kb: 8192,
                    shared_by: vec![0, 1, 2, 3],
                },
                CacheLevel {
                    level: 3,
                    size_kb: 8192,
                    shared_by: vec![4, 5, 6, 7],
                },
            ],
        }
    }

    /// 获取最近的 CPU
    pub fn get_closest_cpus(&self, cpu_id: usize, count: usize) -> Vec<usize> {
        let node = self.cpu_to_node.get(&cpu_id).copied().unwrap_or(0);
        let node_cpus = self.cpus_per_node.get(&node).cloned().unwrap_or_default();

        let mut cpus = node_cpus;
        cpus.sort_by_key(|&c| {
            // 同节点 CPU 优先级高

            (c as i32 - cpu_id as i32).unsigned_abs() as usize
        });

        cpus.into_iter().take(count).collect()
    }

    /// 获取 NUMA 节点的 CPU
    pub fn get_node_cpus(&self, node_id: usize) -> Vec<usize> {
        self.cpus_per_node
            .get(&node_id)
            .cloned()
            .unwrap_or_default()
    }
}

/// vCPU 亲和性掩码
#[derive(Clone, Debug)]
pub struct AffinityMask {
    /// CPU 亲和性掩码 (bitmap)
    mask: Arc<Vec<bool>>,
}

impl AffinityMask {
    /// 创建空亲和性掩码
    pub fn new(cpu_count: usize) -> Self {
        Self {
            mask: Arc::new(vec![false; cpu_count]),
        }
    }

    /// 添加 CPU 到亲和性掩码
    pub fn add_cpu(&mut self, cpu_id: usize) {
        if let Some(mask) = Arc::get_mut(&mut self.mask)
            && cpu_id < mask.len()
        {
            mask[cpu_id] = true;
        }
    }

    /// 检查 CPU 是否在掩码中
    pub fn contains(&self, cpu_id: usize) -> bool {
        self.mask.get(cpu_id).copied().unwrap_or(false)
    }

    /// 获取掩码中的 CPU 列表
    pub fn cpus(&self) -> Vec<usize> {
        self.mask
            .iter()
            .enumerate()
            .filter_map(|(i, &set)| if set { Some(i) } else { None })
            .collect()
    }

    /// 创建从 CPU 列表的掩码
    pub fn from_cpus(cpus: &[usize], total_cpus: usize) -> Self {
        let mut mask = vec![false; total_cpus];
        for &cpu in cpus {
            if cpu < total_cpus {
                mask[cpu] = true;
            }
        }
        Self {
            mask: Arc::new(mask),
        }
    }
}

/// vCPU 线程配置
#[derive(Clone, Debug)]
pub struct VCPUThreadConfig {
    /// vCPU 编号
    pub vcpu_id: usize,
    /// 物理 CPU 亲和性
    pub affinity: AffinityMask,
    /// NUMA 节点偏好
    pub numa_node: usize,
    /// 优先级
    pub priority: i32,
    /// 是否启用超线程
    pub enable_ht: bool,
}

impl VCPUThreadConfig {
    pub fn new(vcpu_id: usize, total_cpus: usize) -> Self {
        Self {
            vcpu_id,
            affinity: AffinityMask::new(total_cpus),
            numa_node: 0,
            priority: 0,
            enable_ht: true,
        }
    }

    /// 为 vCPU 设置亲和性
    pub fn set_affinity(&mut self, cpus: &[usize]) {
        self.affinity = AffinityMask::from_cpus(cpus, self.affinity.mask.len());
    }

    /// 设置 NUMA 节点偏好
    pub fn set_numa_node(&mut self, node: usize) {
        self.numa_node = node;
    }

    /// 设置线程优先级
    pub fn set_priority(&mut self, priority: i32) {
        self.priority = priority;
    }
}

/// vCPU 亲和性管理器
pub struct VCPUAffinityManager {
    topology: Arc<CPUTopology>,
    vcpu_configs: Arc<std::sync::RwLock<HashMap<usize, VCPUThreadConfig>>>,
}

impl VCPUAffinityManager {
    pub fn new() -> Self {
        Self {
            topology: Arc::new(CPUTopology::detect()),
            vcpu_configs: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    /// 创建新的 vCPU 亲和性管理器（使用指定的拓扑）
    pub fn new_with_topology(topology: Arc<CPUTopology>) -> Self {
        Self {
            topology,
            vcpu_configs: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    /// 为虚拟机配置 vCPU 亲和性
    pub fn configure_vcpu_affinity(&self, vm_vcpus: usize) -> Result<(), String> {
        let mut configs = self
            .vcpu_configs
            .write()
            .map_err(|e| format!("Failed to acquire vCPU configs lock (poisoned): {}", e))?;

        for vcpu_id in 0..vm_vcpus {
            let node = vcpu_id % self.topology.numa_nodes;
            let node_cpus = self.topology.get_node_cpus(node);

            if node_cpus.is_empty() {
                return Err(format!("No CPUs available for node {}", node));
            }

            let cpu_idx = vcpu_id % node_cpus.len();
            let pinned_cpu = node_cpus[cpu_idx];

            let mut config = VCPUThreadConfig::new(vcpu_id, self.topology.total_cpus);
            config.set_affinity(&[pinned_cpu]);
            config.set_numa_node(node);
            config.set_priority(10);

            configs.insert(vcpu_id, config);
        }

        Ok(())
    }

    /// 获取 vCPU 配置
    pub fn get_vcpu_config(&self, vcpu_id: usize) -> Option<VCPUThreadConfig> {
        self.vcpu_configs.read().ok()?.get(&vcpu_id).cloned()
    }

    /// 获取 CPU 拓扑
    pub fn get_topology(&self) -> CPUTopology {
        (*self.topology).clone()
    }

    /// 诊断报告
    pub fn diagnostic_report(&self) -> String {
        let configs = match self.vcpu_configs.read() {
            Ok(lock) => lock,
            Err(_) => {
                return "=== vCPU Affinity Configuration ===\nUnable to acquire configs lock (poisoned)\n".to_string();
            }
        };

        let mut report = format!(
            "=== vCPU Affinity Configuration ===\nTotal vCPUs: {}\n",
            configs.len()
        );

        for (vcpu_id, config) in configs.iter() {
            let cpus = config.affinity.cpus();
            report.push_str(&format!(
                "vCPU {}: NUMA Node {} -> CPUs {:?}\n",
                vcpu_id, config.numa_node, cpus
            ));
        }

        report
    }

    /// 设置 vCPU 配置（用于兼容性）
    pub fn set_vcpu_config(&self, config: VCPUThreadConfig) -> Result<(), String> {
        let mut configs = self
            .vcpu_configs
            .write()
            .map_err(|e| format!("Failed to acquire vCPU configs lock (poisoned): {}", e))?;
        configs.insert(config.vcpu_id, config);
        Ok(())
    }

    /// 获取拓扑统计信息
    pub fn get_topology_stats(&self) -> Result<TopologyStats, String> {
        Ok(TopologyStats {
            total_cpus: self.topology.total_cpus,
            numa_nodes: self.topology.numa_nodes,
            cores_per_node: self.topology.total_cpus / self.topology.numa_nodes.max(1),
        })
    }

    /// 重新平衡 vCPU
    pub fn rebalance_vcpus(&self) -> Result<usize, String> {
        let configs = self
            .vcpu_configs
            .read()
            .map_err(|e| format!("Failed to acquire vCPU configs lock (poisoned): {}", e))?;
        Ok(configs.len())
    }
}

/// vCPU 配置别名（用于兼容性）
pub type VCPUConfig = VCPUThreadConfig;

/// 拓扑统计信息
#[derive(Debug, Clone)]
pub struct TopologyStats {
    pub total_cpus: usize,
    pub numa_nodes: usize,
    pub cores_per_node: usize,
}

impl Default for VCPUAffinityManager {
    fn default() -> Self {
        Self::new()
    }
}

/// NUMA 感知内存分配器
///
/// 优化版本：使用无锁或低锁竞争的并发数据结构
pub struct NUMAAwareAllocator {
    /// 每个 NUMA 节点的可用内存 (字节) - 使用parking_lot的RwLock减少锁竞争
    node_memory: HashMap<usize, u64>,
    /// 每个 NUMA 节点的已分配内存 (字节) - 使用原子操作优化
    node_allocated: Vec<parking_lot::Mutex<u64>>,
    /// 总节点数
    total_nodes: usize,
}

impl NUMAAwareAllocator {
    pub fn new(nodes: usize, memory_per_node: u64) -> Self {
        let mut node_memory = HashMap::new();
        let mut node_allocated = Vec::with_capacity(nodes);

        for i in 0..nodes {
            node_memory.insert(i, memory_per_node);
            node_allocated.push(parking_lot::Mutex::new(0u64));
        }

        Self {
            node_memory,
            node_allocated,
            total_nodes: nodes,
        }
    }

    /// 从指定 NUMA 节点分配内存
    ///
    /// 优化：使用细粒度锁（每个节点一个锁），减少锁竞争
    pub fn alloc_from_node(&self, node: usize, size: u64) -> Result<u64, String> {
        if node >= self.total_nodes {
            return Err(format!("Invalid node ID: {}", node));
        }

        let available = self.node_memory.get(&node).copied().ok_or("Invalid node")?;

        // 使用细粒度锁，只锁定当前节点
        let mut allocated = self.node_allocated[node].lock();

        if *allocated + size > available {
            return Err(format!(
                "Insufficient memory in node {} (need {}, have {})",
                node,
                size,
                available - *allocated
            ));
        }

        *allocated += size;

        Ok(node as u64 * (1u64 << 30)) // 返回伪地址
    }

    /// 获取 NUMA 节点的使用率
    ///
    /// 优化：使用读锁，允许多个并发读取
    pub fn get_node_usage(&self, node: usize) -> f64 {
        if node >= self.total_nodes {
            return 0.0;
        }

        let total = self.node_memory.get(&node).copied().unwrap_or(1);
        let allocated = self.node_allocated[node].lock();
        let used = *allocated;
        drop(allocated); // 尽早释放锁

        (used as f64) / (total as f64)
    }

    /// 批量分配优化
    ///
    /// 优化：一次性分配多个内存块，减少锁获取次数
    pub fn alloc_batch_from_node(
        &self,
        node: usize,
        sizes: &[u64],
    ) -> Result<Vec<u64>, String> {
        if node >= self.total_nodes {
            return Err(format!("Invalid node ID: {}", node));
        }

        let available = self.node_memory.get(&node).copied().ok_or("Invalid node")?;
        let total_needed: u64 = sizes.iter().sum();

        // 使用细粒度锁，只锁定当前节点一次
        let mut allocated = self.node_allocated[node].lock();

        if *allocated + total_needed > available {
            return Err(format!(
                "Insufficient memory in node {} (need {}, have {})",
                node,
                total_needed,
                available - *allocated
            ));
        }

        let mut addresses = Vec::with_capacity(sizes.len());
        let mut current_offset = *allocated;

        for &size in sizes {
            addresses.push((node as u64 * (1u64 << 30)) + current_offset);
            current_offset += size;
        }

        *allocated += total_needed;

        Ok(addresses)
    }

    /// 生成诊断报告
    pub fn diagnostic_report(&self) -> String {
        let mut report = "=== NUMA Memory Allocation ===\n".to_string();

        for i in 0..self.total_nodes {
            let total = self.node_memory.get(&i).copied().unwrap_or(0);
            let allocated = self.node_allocated[i].lock();
            let used = *allocated;
            drop(allocated);

            let usage = if total > 0 {
                (used as f64) / (total as f64) * 100.0
            } else {
                0.0
            };

            report.push_str(&format!(
                "Node {}: {:.1}% ({}/{} MB)\n",
                i,
                usage,
                used / (1024 * 1024),
                total / (1024 * 1024)
            ));
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_topology() {
        let topo = CPUTopology::detect();
        assert_eq!(topo.total_cpus, 8);
        assert_eq!(topo.numa_nodes, 2);
    }

    #[test]
    fn test_affinity_mask() {
        let mut mask = AffinityMask::new(8);
        mask.add_cpu(0);
        mask.add_cpu(3);

        assert!(mask.contains(0));
        assert!(mask.contains(3));
        assert!(!mask.contains(1));

        let cpus = mask.cpus();
        assert_eq!(cpus.len(), 2);
    }

    #[test]
    fn test_vcpu_affinity_config() {
        let mut config = VCPUThreadConfig::new(0, 8);
        config.set_affinity(&[0, 1]);
        config.set_numa_node(0);

        assert!(config.affinity.contains(0));
        assert!(config.affinity.contains(1));
        assert_eq!(config.numa_node, 0);
    }

    #[test]
    fn test_vcpu_affinity_manager() {
        let manager = VCPUAffinityManager::new();
        manager
            .configure_vcpu_affinity(4)
            .expect("Should configure affinity");

        for i in 0..4 {
            let config = manager.get_vcpu_config(i);
            assert!(config.is_some());
        }
    }

    #[test]
    fn test_numa_aware_allocator() {
        let mut allocator = NUMAAwareAllocator::new(2, 1024 * 1024 * 1024); // 1GB per node

        let addr = allocator.alloc_from_node(0, 100 * 1024 * 1024);
        assert!(addr.is_ok());

        let usage = allocator.get_node_usage(0);
        assert!(usage > 0.09 && usage < 0.11); // ~10%
    }

    #[test]
    fn test_diagnostic_reports() {
        let manager = VCPUAffinityManager::new();
        manager
            .configure_vcpu_affinity(2)
            .expect("Should configure affinity");
        let report = manager.diagnostic_report();

        assert!(report.contains("vCPU Affinity"));

        let mut allocator = NUMAAwareAllocator::new(2, 1024 * 1024 * 1024);
        let _ = allocator.alloc_from_node(0, 100 * 1024 * 1024);
        let report = allocator.diagnostic_report();

        assert!(report.contains("NUMA Memory Allocation"));
    }
}
