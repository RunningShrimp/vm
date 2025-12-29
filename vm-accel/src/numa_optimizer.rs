//! 高级 NUMA 感知优化器
//!
//! 提供全面的 NUMA 感知内存分配、vCPU 调度和数据局部性优化

use crate::vcpu_affinity::{CPUTopology, NUMAAwareAllocator, VCPUAffinityManager};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// NUMA 节点统计信息
#[derive(Debug, Clone)]
pub struct NUMANodeStats {
    /// 节点ID
    pub node_id: usize,
    /// CPU 使用率
    pub cpu_usage: f64,
    /// 内存使用率
    pub memory_usage: f64,
    /// 内存带宽使用率
    pub memory_bandwidth_usage: f64,
    /// 缓存未命中率
    pub cache_miss_rate: f64,
    /// 跨节点访问次数
    pub cross_node_accesses: u64,
    /// 本地访问次数
    pub local_accesses: u64,
}

impl NUMANodeStats {
    pub fn new(node_id: usize) -> Self {
        Self {
            node_id,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            memory_bandwidth_usage: 0.0,
            cache_miss_rate: 0.0,
            cross_node_accesses: 0,
            local_accesses: 0,
        }
    }

    /// 计算本地访问率
    pub fn local_access_rate(&self) -> f64 {
        let total = self.local_accesses + self.cross_node_accesses;
        if total == 0 {
            0.0
        } else {
            self.local_accesses as f64 / total as f64
        }
    }
}

/// 内存分配策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryAllocationStrategy {
    /// 本地优先：优先在请求节点的本地内存分配
    LocalFirst,
    /// 负载均衡：考虑各节点的负载均衡
    LoadBalanced,
    /// 带宽优化：优先选择内存带宽充足的节点
    BandwidthOptimized,
    /// 自动选择：基于运行时统计自动选择
    Adaptive,
}

/// NUMA 感知内存分配器
pub struct NUMAOptimizer {
    /// CPU 拓扑信息
    topology: Arc<CPUTopology>,
    /// vCPU 亲和性管理器
    affinity_manager: Arc<VCPUAffinityManager>,
    /// 基础内存分配器
    memory_allocator: Arc<RwLock<NUMAAwareAllocator>>,
    /// 节点统计信息
    node_stats: Arc<RwLock<HashMap<usize, NUMANodeStats>>>,
    /// 内存分配策略
    allocation_strategy: MemoryAllocationStrategy,
    /// 负载均衡阈值
    load_balance_threshold: f64,
    /// 统计更新间隔
    stats_update_interval: Duration,
    /// 上次统计更新时间
    last_stats_update: Arc<RwLock<Instant>>,
}

impl NUMAOptimizer {
    /// 创建新的 NUMA 优化器
    pub fn new(
        topology: Arc<CPUTopology>,
        affinity_manager: Arc<VCPUAffinityManager>,
        memory_per_node: u64,
    ) -> Self {
        let memory_allocator = Arc::new(RwLock::new(NUMAAwareAllocator::new(
            topology.numa_nodes,
            memory_per_node,
        )));

        let mut node_stats = HashMap::new();
        for node_id in 0..topology.numa_nodes {
            node_stats.insert(node_id, NUMANodeStats::new(node_id));
        }

        Self {
            topology,
            affinity_manager,
            memory_allocator,
            node_stats: Arc::new(RwLock::new(node_stats)),
            allocation_strategy: MemoryAllocationStrategy::Adaptive,
            load_balance_threshold: 0.8,
            stats_update_interval: Duration::from_millis(100),
            last_stats_update: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// 设置内存分配策略
    pub fn set_allocation_strategy(&mut self, strategy: MemoryAllocationStrategy) {
        self.allocation_strategy = strategy;
    }

    /// 设置内存分配策略（用于兼容性）
    pub fn set_strategy(&mut self, strategy: MemoryAllocationStrategy) {
        self.set_allocation_strategy(strategy);
    }

    /// 更新统计信息
    pub fn update_stats(&self) {
        self.update_stats_if_needed();
    }

    /// 获取所有节点统计信息
    pub fn get_all_stats(&self) -> Vec<NUMANodeStats> {
        match self.get_all_node_stats() {
            Ok(stats_map) => stats_map.into_values().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// NUMA 感知内存分配
    pub fn allocate_memory(
        &self,
        size: u64,
        preferred_node: Option<usize>,
    ) -> Result<(u64, usize), String> {
        self.update_stats_if_needed();

        let node_id = match self.allocation_strategy {
            MemoryAllocationStrategy::LocalFirst => preferred_node.unwrap_or(0),
            MemoryAllocationStrategy::LoadBalanced => self.select_load_balanced_node(size)?,
            MemoryAllocationStrategy::BandwidthOptimized => {
                self.select_bandwidth_optimized_node(size)?
            }
            MemoryAllocationStrategy::Adaptive => {
                self.select_adaptive_node(size, preferred_node)?
            }
        };

        let mut allocator = self
            .memory_allocator
            .write()
            .map_err(|e| format!("Failed to acquire memory allocator lock (poisoned): {}", e))?;
        let address = allocator.alloc_from_node(node_id, size)?;

        Ok((address, node_id))
    }

    /// 获取亲和性管理器的引用
    pub fn affinity_manager(&self) -> &Arc<VCPUAffinityManager> {
        &self.affinity_manager
    }

    /// 释放内存
    pub fn free_memory(&self, _node_id: usize, _size: u64) -> Result<(), String> {
        let mut _allocator = self
            .memory_allocator
            .write()
            .map_err(|e| format!("Failed to acquire memory allocator lock (poisoned): {}", e))?;
        // 简化实现：假设释放总是成功
        // 实际实现需要跟踪分配的内存块
        Ok(())
    }

    /// 选择负载均衡节点
    fn select_load_balanced_node(&self, _size: u64) -> Result<usize, String> {
        let stats = self
            .node_stats
            .read()
            .map_err(|e| format!("Failed to acquire node stats lock (poisoned): {}", e))?;
        let allocator = self
            .memory_allocator
            .read()
            .map_err(|e| format!("Failed to acquire memory allocator lock (poisoned): {}", e))?;

        let mut best_node = 0;
        let mut best_score = f64::INFINITY;

        for (&node_id, node_stat) in stats.iter() {
            let memory_usage = allocator.get_node_usage(node_id);

            // 计算综合负载分数
            let load_score = node_stat.cpu_usage * 0.4
                + memory_usage * 0.4
                + node_stat.memory_bandwidth_usage * 0.2;

            if load_score < best_score && memory_usage < self.load_balance_threshold {
                best_score = load_score;
                best_node = node_id;
            }
        }

        Ok(best_node)
    }

    /// 选择带宽优化节点
    fn select_bandwidth_optimized_node(&self, _size: u64) -> Result<usize, String> {
        let stats = self
            .node_stats
            .read()
            .map_err(|e| format!("Failed to acquire node stats lock (poisoned): {}", e))?;
        let allocator = self
            .memory_allocator
            .read()
            .map_err(|e| format!("Failed to acquire memory allocator lock (poisoned): {}", e))?;

        let mut best_node = 0;
        let mut best_bandwidth = 0.0;

        for (&node_id, node_stat) in stats.iter() {
            let memory_usage = allocator.get_node_usage(node_id);
            let available_bandwidth = 1.0 - node_stat.memory_bandwidth_usage;

            if available_bandwidth > best_bandwidth && memory_usage < self.load_balance_threshold {
                best_bandwidth = available_bandwidth;
                best_node = node_id;
            }
        }

        Ok(best_node)
    }

    /// 自适应节点选择（优化版：更智能的NUMA感知分配）
    ///
    /// 优化策略：
    /// 1. 优先选择本地节点（减少跨节点访问）
    /// 2. 考虑内存大小（大块内存优先选择带宽充足的节点）
    /// 3. 考虑访问模式（频繁访问的数据优先本地节点）
    /// 4. 动态调整策略（根据系统负载）
    fn select_adaptive_node(
        &self,
        size: u64,
        preferred_node: Option<usize>,
    ) -> Result<usize, String> {
        let stats = self
            .node_stats
            .read()
            .map_err(|e| format!("Failed to acquire node stats lock (poisoned): {}", e))?;
        let allocator = self
            .memory_allocator
            .read()
            .map_err(|e| format!("Failed to acquire memory allocator lock (poisoned): {}", e))?;

        // 策略1：如果有偏好节点，检查是否适合
        if let Some(node) = preferred_node
            && let Some(node_stat) = stats.get(&node)
        {
            let memory_usage = allocator.get_node_usage(node);
            let local_access_rate = node_stat.local_access_rate();
            let available_bandwidth = 1.0 - node_stat.memory_bandwidth_usage;

            // 综合评分：本地访问率 + 可用带宽 + 负载
            let score =
                local_access_rate * 0.5 + available_bandwidth * 0.3 + (1.0 - memory_usage) * 0.2;

            // 如果评分高且负载不高，优先选择
            if score > 0.6 && memory_usage < self.load_balance_threshold {
                return Ok(node);
            }
        }

        // 策略2：根据内存大小选择策略
        let large_allocation = size > 64 * 1024 * 1024; // 大于64MB

        if large_allocation {
            // 大块内存：优先选择带宽充足的节点
            self.select_bandwidth_optimized_node(size)
        } else {
            // 小块内存：优先选择本地节点或负载均衡
            // 尝试找到本地访问率高的节点
            let mut best_node = 0;
            let mut best_score = 0.0;

            for (&node_id, node_stat) in stats.iter() {
                let memory_usage = allocator.get_node_usage(node_id);
                let local_access_rate = node_stat.local_access_rate();
                let available_bandwidth = 1.0 - node_stat.memory_bandwidth_usage;

                // 综合评分：优先本地访问率
                let score = local_access_rate * 0.6
                    + available_bandwidth * 0.2
                    + (1.0 - memory_usage) * 0.2;

                if score > best_score && memory_usage < self.load_balance_threshold {
                    best_score = score;
                    best_node = node_id;
                }
            }

            if best_score > 0.5 {
                Ok(best_node)
            } else {
                // 如果所有节点评分都不高，使用负载均衡
                self.select_load_balanced_node(size)
            }
        }
    }

    /// 获取当前线程所在的NUMA节点（用于自动选择偏好节点）
    #[cfg(target_os = "linux")]
    pub fn get_current_node(&self) -> Option<usize> {
        unsafe {
            let cpu = libc::sched_getcpu();
            if cpu >= 0 {
                self.topology.cpu_to_node.get(&(cpu as usize)).copied()
            } else {
                None
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn get_current_node(&self) -> Option<usize> {
        None
    }

    /// NUMA感知内存分配（自动检测当前节点）
    pub fn allocate_memory_numa_aware(&self, size: u64) -> Result<(u64, usize), String> {
        // 自动检测当前线程所在的NUMA节点
        let preferred_node = self.get_current_node();
        self.allocate_memory(size, preferred_node)
    }

    /// 记录内存访问模式
    pub fn record_memory_access(
        &self,
        accessing_cpu: usize,
        target_node: usize,
        _size: usize,
    ) -> Result<(), String> {
        let accessing_node = self
            .topology
            .cpu_to_node
            .get(&accessing_cpu)
            .copied()
            .unwrap_or(0);

        let mut stats = self
            .node_stats
            .write()
            .map_err(|e| format!("Failed to acquire node stats lock (poisoned): {}", e))?;
        if let Some(node_stat) = stats.get_mut(&target_node) {
            if accessing_node == target_node {
                node_stat.local_accesses += 1;
            } else {
                node_stat.cross_node_accesses += 1;
            }
        }
        Ok(())
    }

    /// 更新节点统计信息
    pub fn update_node_stats(
        &self,
        node_id: usize,
        cpu_usage: f64,
        memory_bandwidth: f64,
        cache_miss_rate: f64,
    ) -> Result<(), String> {
        let mut stats = self
            .node_stats
            .write()
            .map_err(|e| format!("Failed to acquire node stats lock (poisoned): {}", e))?;
        if let Some(node_stat) = stats.get_mut(&node_id) {
            node_stat.cpu_usage = cpu_usage;
            node_stat.memory_bandwidth_usage = memory_bandwidth;
            node_stat.cache_miss_rate = cache_miss_rate;
        }
        Ok(())
    }

    /// 获取节点统计信息
    pub fn get_node_stats(&self, node_id: usize) -> Option<NUMANodeStats> {
        self.node_stats.read().ok()?.get(&node_id).cloned()
    }

    /// 获取所有节点统计信息
    pub fn get_all_node_stats(&self) -> Result<HashMap<usize, NUMANodeStats>, String> {
        let stats = self
            .node_stats
            .read()
            .map_err(|e| format!("Failed to acquire node stats lock (poisoned): {}", e))?;
        Ok(stats.clone())
    }

    /// 优化 vCPU 放置
    pub fn optimize_vcpu_placement(
        &self,
        vcpu_count: usize,
    ) -> Result<HashMap<usize, usize>, String> {
        let mut placement = HashMap::new();
        let _stats = self
            .node_stats
            .read()
            .map_err(|e| format!("Failed to acquire node stats lock (poisoned): {}", e))?;

        // 简单的轮询放置策略
        for vcpu_id in 0..vcpu_count {
            let node_id = vcpu_id % self.topology.numa_nodes;
            placement.insert(vcpu_id, node_id);
        }

        Ok(placement)
    }

    /// 重新平衡内存分配
    pub fn rebalance_memory(&self) -> Result<(), String> {
        // 简化实现：实际应该移动内存页到更合适的节点
        // 这里只是更新统计信息
        self.update_stats_if_needed();
        Ok(())
    }

    /// 生成优化建议
    pub fn generate_optimization_suggestions(&self) -> Result<Vec<String>, String> {
        let mut suggestions = Vec::new();
        let stats = self
            .node_stats
            .read()
            .map_err(|e| format!("Failed to acquire node stats lock (poisoned): {}", e))?;
        let allocator = self
            .memory_allocator
            .read()
            .map_err(|e| format!("Failed to acquire memory allocator lock (poisoned): {}", e))?;

        for (&node_id, node_stat) in stats.iter() {
            let memory_usage = allocator.get_node_usage(node_id);
            let local_access_rate = node_stat.local_access_rate();

            if memory_usage > 0.9 {
                suggestions.push(format!("Node {} memory usage is high ({:.1}%), consider adding more memory or rebalancing workloads", node_id, memory_usage * 100.0));
            }

            if local_access_rate < 0.5 {
                suggestions.push(format!("Node {} has low local access rate ({:.1}%), consider optimizing data placement", node_id, local_access_rate * 100.0));
            }

            if node_stat.cache_miss_rate > 0.1 {
                suggestions.push(format!(
                    "Node {} has high cache miss rate ({:.1}%), consider memory prefetching",
                    node_id,
                    node_stat.cache_miss_rate * 100.0
                ));
            }
        }

        Ok(suggestions)
    }

    /// 定期更新统计信息
    fn update_stats_if_needed(&self) {
        let now = Instant::now();
        let last_update = match self.last_stats_update.read() {
            Ok(lock) => *lock,
            Err(_) => return, // If lock is poisoned, skip update
        };

        if now.duration_since(last_update) >= self.stats_update_interval {
            // 模拟更新统计信息
            // 实际实现应该从系统获取真实的统计数据
            if let Ok(mut stats) = self.node_stats.write() {
                for node_stat in stats.values_mut() {
                    // 模拟一些统计数据变化
                    node_stat.cpu_usage = (node_stat.cpu_usage + 0.01).min(1.0);
                    node_stat.memory_bandwidth_usage =
                        (node_stat.memory_bandwidth_usage + 0.005).min(1.0);
                }
            }

            if let Ok(mut last_update) = self.last_stats_update.write() {
                *last_update = now;
            }
        }
    }

    /// 生成诊断报告
    pub fn diagnostic_report(&self) -> String {
        let mut report = "=== NUMA Optimizer Diagnostic Report ===\n\n".to_string();

        report.push_str(&format!(
            "Allocation Strategy: {:?}\n",
            self.allocation_strategy
        ));
        report.push_str(&format!(
            "Load Balance Threshold: {:.1}%\n\n",
            self.load_balance_threshold * 100.0
        ));

        let stats = match self.node_stats.read() {
            Ok(lock) => lock,
            Err(_) => {
                report.push_str("Unable to acquire node stats lock (poisoned)\n");
                return report;
            }
        };
        let allocator = match self.memory_allocator.read() {
            Ok(lock) => lock,
            Err(_) => {
                report.push_str("Unable to acquire memory allocator lock (poisoned)\n");
                return report;
            }
        };

        for node_id in 0..self.topology.numa_nodes {
            if let Some(node_stat) = stats.get(&node_id) {
                let memory_usage = allocator.get_node_usage(node_id);
                let local_access_rate = node_stat.local_access_rate();

                report.push_str(&format!("Node {}:\n", node_id));
                report.push_str(&format!(
                    "  CPU Usage: {:.1}%\n",
                    node_stat.cpu_usage * 100.0
                ));
                report.push_str(&format!("  Memory Usage: {:.1}%\n", memory_usage * 100.0));
                report.push_str(&format!(
                    "  Memory Bandwidth: {:.1}%\n",
                    node_stat.memory_bandwidth_usage * 100.0
                ));
                report.push_str(&format!(
                    "  Cache Miss Rate: {:.1}%\n",
                    node_stat.cache_miss_rate * 100.0
                ));
                report.push_str(&format!(
                    "  Local Access Rate: {:.1}%\n",
                    local_access_rate * 100.0
                ));
                report.push_str(&format!(
                    "  Cross-Node Accesses: {}\n",
                    node_stat.cross_node_accesses
                ));
                report.push_str(&format!(
                    "  Local Accesses: {}\n\n",
                    node_stat.local_accesses
                ));
            }
        }

        let suggestions = self.generate_optimization_suggestions().unwrap_or_default();
        if !suggestions.is_empty() {
            report.push_str("Optimization Suggestions:\n");
            for suggestion in suggestions {
                report.push_str(&format!("  - {}\n", suggestion));
            }
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vcpu_affinity::CPUTopology;

    #[test]
    fn test_numa_optimizer_creation() {
        let topology = Arc::new(CPUTopology::detect());
        let affinity_manager = Arc::new(VCPUAffinityManager::new());
        let optimizer = NUMAOptimizer::new(topology, affinity_manager, 1024 * 1024 * 1024);

        assert_eq!(
            optimizer.allocation_strategy,
            MemoryAllocationStrategy::Adaptive
        );
    }

    #[test]
    fn test_memory_allocation() {
        let topology = Arc::new(CPUTopology::detect());
        let affinity_manager = Arc::new(VCPUAffinityManager::new());
        let optimizer = NUMAOptimizer::new(topology, affinity_manager, 1024 * 1024 * 1024);

        let result = optimizer.allocate_memory(100 * 1024 * 1024, Some(0));
        assert!(result.is_ok());

        let (_address, node_id) = result.expect("Memory allocation should succeed");
        // Adaptive 策略可能选择不同的节点，取决于系统状态
        // 只验证返回了一个有效的节点ID
        assert!(node_id < 10); // 假设系统最多有10个NUMA节点
    }

    #[test]
    fn test_node_stats_tracking() {
        let topology = Arc::new(CPUTopology::detect());
        let affinity_manager = Arc::new(VCPUAffinityManager::new());
        let optimizer = NUMAOptimizer::new(topology, affinity_manager, 1024 * 1024 * 1024);

        optimizer
            .record_memory_access(0, 0, 4096)
            .expect("Local access should succeed"); // 本地访问
        optimizer
            .record_memory_access(0, 1, 4096)
            .expect("Cross-node access should succeed"); // 跨节点访问

        let stats = optimizer
            .get_node_stats(0)
            .expect("Should get stats for node 0");
        assert_eq!(stats.local_accesses, 1);
        assert_eq!(stats.cross_node_accesses, 0);

        let stats = optimizer
            .get_node_stats(1)
            .expect("Should get stats for node 1");
        assert_eq!(stats.local_accesses, 0);
        assert_eq!(stats.cross_node_accesses, 1);
    }

    #[test]
    fn test_optimization_suggestions() {
        let topology = Arc::new(CPUTopology::detect());
        let affinity_manager = Arc::new(VCPUAffinityManager::new());
        let optimizer = NUMAOptimizer::new(topology, affinity_manager, 1024 * 1024 * 1024);

        // 强制设置高负载状态来测试建议
        optimizer
            .update_node_stats(0, 0.95, 0.9, 0.15)
            .expect("Update should succeed");

        let suggestions = optimizer
            .generate_optimization_suggestions()
            .expect("Should generate suggestions");
        assert!(!suggestions.is_empty());
        assert!(
            suggestions
                .iter()
                .any(|s| s.contains("high") || s.contains("low") || s.contains("cache miss"))
        );
    }

    #[test]
    fn test_diagnostic_report() {
        let topology = Arc::new(CPUTopology::detect());
        let affinity_manager = Arc::new(VCPUAffinityManager::new());
        let optimizer = NUMAOptimizer::new(topology, affinity_manager, 1024 * 1024 * 1024);

        let report = optimizer.diagnostic_report();
        assert!(report.contains("NUMA Optimizer"));
        assert!(report.contains("Allocation Strategy"));
    }
}
