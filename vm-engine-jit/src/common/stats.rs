//! JIT引擎通用统计基础设施
//!
//! 本模块提供了统一的统计基础设施，用于减少重复代码并提高一致性。

use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize, Serializer, Deserializer};

/// JIT优化统计基础结构
#[derive(Debug, Clone, Default)]
pub struct OptimizationStats {
    pub blocks_optimized: u64,
    pub ops_vectorized: u64,
    pub simd_ops_generated: u64,
    pub fma_fusions: u64,
    pub masked_ops: u64,
    pub load_store_vectorized: u64,
    pub horizontal_ops_optimized: u64,
    pub compilation_time: Duration,
    pub optimization_time: Duration,
}

impl OptimizationStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn total_time(&self) -> Duration {
        self.compilation_time + self.optimization_time
    }

    pub fn ops_per_ms(&self) -> f64 {
        if self.optimization_time.as_millis() > 0 {
            self.ops_vectorized as f64 / self.optimization_time.as_millis() as f64
        } else {
            0.0
        }
    }

    pub fn speedup_estimate(&self) -> f64 {
        if self.ops_vectorized > 0 && self.simd_ops_generated > 0 {
            let vector_width = 4.0;
            self.ops_vectorized as f64 * (vector_width - 1.0) / self.ops_vectorized as f64
        } else {
            1.0
        }
    }
}

/// 通用统计特征
pub trait Stats: Clone + Default + Send + Sync {
    /// 重置统计信息
    fn reset(&mut self);
    
    /// 合并另一个统计对象
    fn merge(&mut self, other: &Self);
    
    /// 获取统计摘要
    fn summary(&self) -> String;
}

/// 执行统计基础结构
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    /// 总执行次数
    pub total_executions: u64,
    /// 成功执行次数
    pub successful_executions: u64,
    /// 失败执行次数
    pub failed_executions: u64,
    /// 总执行时间（纳秒）
    pub total_execution_time_ns: u64,
    /// 最小执行时间（纳秒）
    pub min_execution_time_ns: u64,
    /// 最大执行时间（纳秒）
    pub max_execution_time_ns: u64,
    /// 平均执行时间（纳秒）
    pub avg_execution_time_ns: u64,
    /// 最后更新时间
    pub last_update_time: Option<Instant>,
}

impl Default for ExecutionStats {
    fn default() -> Self {
        Self {
            total_executions: 0,
            successful_executions: 0,
            failed_executions: 0,
            total_execution_time_ns: 0,
            min_execution_time_ns: u64::MAX,
            max_execution_time_ns: 0,
            avg_execution_time_ns: 0,
            last_update_time: None,
        }
    }
}

impl ExecutionStats {
    /// 创建新的执行统计
    pub fn new() -> Self {
        Self::default()
    }
    
    /// 记录执行开始
    pub fn record_execution_start(&self) -> ExecutionTimer {
        ExecutionTimer::new()
    }
    
    /// 记录成功执行
    pub fn record_successful_execution(&mut self, execution_time: Duration) {
        self.total_executions += 1;
        self.successful_executions += 1;
        self.update_timing_stats(execution_time);
        self.last_update_time = Some(Instant::now());
    }
    
    /// 记录失败执行
    pub fn record_failed_execution(&mut self, execution_time: Duration) {
        self.total_executions += 1;
        self.failed_executions += 1;
        self.update_timing_stats(execution_time);
        self.last_update_time = Some(Instant::now());
    }
    
    /// 更新时间统计
    fn update_timing_stats(&mut self, execution_time: Duration) {
        let time_ns = execution_time.as_nanos() as u64;
        self.total_execution_time_ns += time_ns;
        
        if time_ns < self.min_execution_time_ns {
            self.min_execution_time_ns = time_ns;
        }
        
        if time_ns > self.max_execution_time_ns {
            self.max_execution_time_ns = time_ns;
        }
        
        if self.total_executions > 0 {
            self.avg_execution_time_ns = self.total_execution_time_ns / self.total_executions;
        }
    }
    
    /// 获取成功率
    pub fn success_rate(&self) -> f64 {
        if self.total_executions == 0 {
            0.0
        } else {
            self.successful_executions as f64 / self.total_executions as f64
        }
    }
    
    /// 获取失败率
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }
}

impl Stats for ExecutionStats {
    fn reset(&mut self) {
        *self = Self::default();
    }
    
    fn merge(&mut self, other: &Self) {
        self.total_executions += other.total_executions;
        self.successful_executions += other.successful_executions;
        self.failed_executions += other.failed_executions;
        self.total_execution_time_ns += other.total_execution_time_ns;
        
        if other.min_execution_time_ns < self.min_execution_time_ns {
            self.min_execution_time_ns = other.min_execution_time_ns;
        }
        
        if other.max_execution_time_ns > self.max_execution_time_ns {
            self.max_execution_time_ns = other.max_execution_time_ns;
        }
        
        if self.total_executions > 0 {
            self.avg_execution_time_ns = self.total_execution_time_ns / self.total_executions;
        }
        
        // 使用最新的更新时间
        if let Some(other_time) = other.last_update_time {
            if let Some(self_time) = self.last_update_time {
                if other_time > self_time {
                    self.last_update_time = Some(other_time);
                }
            } else {
                self.last_update_time = Some(other_time);
            }
        }
    }
    
    fn summary(&self) -> String {
        format!(
            "执行统计: 总次数={}, 成功率={:.2}%, 平均时间={}ns, 最小时间={}ns, 最大时间={}ns",
            self.total_executions,
            self.success_rate() * 100.0,
            self.avg_execution_time_ns,
            if self.min_execution_time_ns == u64::MAX { 0 } else { self.min_execution_time_ns },
            self.max_execution_time_ns
        )
    }
}

// 自定义序列化实现，处理Instant类型
impl Serialize for ExecutionStats {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        
        let mut state = serializer.serialize_struct("ExecutionStats", 8)?;
        state.serialize_field("total_executions", &self.total_executions)?;
        state.serialize_field("successful_executions", &self.successful_executions)?;
        state.serialize_field("failed_executions", &self.failed_executions)?;
        state.serialize_field("total_execution_time_ns", &self.total_execution_time_ns)?;
        state.serialize_field("min_execution_time_ns", &self.min_execution_time_ns)?;
        state.serialize_field("max_execution_time_ns", &self.max_execution_time_ns)?;
        state.serialize_field("avg_execution_time_ns", &self.avg_execution_time_ns)?;
        // 将Instant转换为时间戳进行序列化
        state.serialize_field("last_update_time_ns", &self.last_update_time.map(|i| i.elapsed().as_nanos() as u64))?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for ExecutionStats {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct ExecutionStatsData {
            total_executions: u64,
            successful_executions: u64,
            failed_executions: u64,
            total_execution_time_ns: u64,
            min_execution_time_ns: u64,
            max_execution_time_ns: u64,
            avg_execution_time_ns: u64,
            last_update_time_ns: Option<u64>,
        }
        
        let data = ExecutionStatsData::deserialize(deserializer)?;
        
        Ok(ExecutionStats {
            total_executions: data.total_executions,
            successful_executions: data.successful_executions,
            failed_executions: data.failed_executions,
            total_execution_time_ns: data.total_execution_time_ns,
            min_execution_time_ns: data.min_execution_time_ns,
            max_execution_time_ns: data.max_execution_time_ns,
            avg_execution_time_ns: data.avg_execution_time_ns,
            // 对于反序列化，我们使用当前时间减去保存的时间差来近似恢复Instant
            last_update_time: data.last_update_time_ns.map(|_| Instant::now()),
        })
    }
}

/// 执行计时器
pub struct ExecutionTimer {
    start_time: Instant,
}

impl ExecutionTimer {
    /// 创建新的执行计时器
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }
    
    /// 完成计时并返回执行时间
    pub fn finish(self) -> Duration {
        self.start_time.elapsed()
    }
}

/// 内存统计基础结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// 总内存使用（字节）
    pub total_memory_bytes: u64,
    /// 已分配内存（字节）
    pub allocated_memory_bytes: u64,
    /// 峰值内存使用（字节）
    pub peak_memory_bytes: u64,
    /// 内存分配次数
    pub allocation_count: u64,
    /// 内存释放次数
    pub deallocation_count: u64,
    /// 内存碎片率
    pub fragmentation_ratio: f64,
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            total_memory_bytes: 0,
            allocated_memory_bytes: 0,
            peak_memory_bytes: 0,
            allocation_count: 0,
            deallocation_count: 0,
            fragmentation_ratio: 0.0,
        }
    }
}

impl MemoryStats {
    /// 创建新的内存统计
    pub fn new() -> Self {
        Self::default()
    }
    
    /// 记录内存分配
    pub fn record_allocation(&mut self, size: u64) {
        self.allocated_memory_bytes += size;
        self.allocation_count += 1;
        
        if self.allocated_memory_bytes > self.peak_memory_bytes {
            self.peak_memory_bytes = self.allocated_memory_bytes;
        }
        
        // 简化的碎片率计算
        if self.total_memory_bytes > 0 {
            self.fragmentation_ratio = 1.0 - (self.allocated_memory_bytes as f64 / self.total_memory_bytes as f64);
        }
    }
    
    /// 记录内存释放
    pub fn record_deallocation(&mut self, size: u64) {
        self.allocated_memory_bytes = self.allocated_memory_bytes.saturating_sub(size);
        self.deallocation_count += 1;
        
        // 更新碎片率
        if self.total_memory_bytes > 0 {
            self.fragmentation_ratio = 1.0 - (self.allocated_memory_bytes as f64 / self.total_memory_bytes as f64);
        }
    }
    
    /// 设置总内存容量
    pub fn set_total_memory(&mut self, total_bytes: u64) {
        self.total_memory_bytes = total_bytes;
        
        // 重新计算碎片率
        if self.total_memory_bytes > 0 {
            self.fragmentation_ratio = 1.0 - (self.allocated_memory_bytes as f64 / self.total_memory_bytes as f64);
        }
    }
    
    /// 获取内存使用率
    pub fn utilization_ratio(&self) -> f64 {
        if self.total_memory_bytes == 0 {
            0.0
        } else {
            self.allocated_memory_bytes as f64 / self.total_memory_bytes as f64
        }
    }
}

impl Stats for MemoryStats {
    fn reset(&mut self) {
        *self = Self::default();
    }
    
    fn merge(&mut self, other: &Self) {
        self.total_memory_bytes += other.total_memory_bytes;
        self.allocated_memory_bytes += other.allocated_memory_bytes;
        self.peak_memory_bytes = self.peak_memory_bytes.max(other.peak_memory_bytes);
        self.allocation_count += other.allocation_count;
        self.deallocation_count += other.deallocation_count;
        
        // 使用加权平均计算碎片率
        let total_allocations = self.allocation_count + other.allocation_count;
        if total_allocations > 0 {
            self.fragmentation_ratio = (
                self.fragmentation_ratio * self.allocation_count as f64 +
                other.fragmentation_ratio * other.allocation_count as f64
            ) / total_allocations as f64;
        }
    }
    
    fn summary(&self) -> String {
        format!(
            "内存统计: 总容量={}MB, 已使用={}MB, 峰值={}MB, 使用率={:.2}%, 碎片率={:.2}%",
            self.total_memory_bytes / (1024 * 1024),
            self.allocated_memory_bytes / (1024 * 1024),
            self.peak_memory_bytes / (1024 * 1024),
            self.utilization_ratio() * 100.0,
            self.fragmentation_ratio * 100.0
        )
    }
}

/// 缓存统计基础结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// 缓存命中次数
    pub hits: u64,
    /// 缓存未命中次数
    pub misses: u64,
    /// 缓存大小（字节）
    pub size_bytes: u64,
    /// 缓存条目数
    pub entries: u64,
    /// 缓存清理次数
    pub evictions: u64,
    /// 缓存插入次数
    pub insertions: u64,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self {
            hits: 0,
            misses: 0,
            size_bytes: 0,
            entries: 0,
            evictions: 0,
            insertions: 0,
        }
    }
}

impl CacheStats {
    /// 创建新的缓存统计
    pub fn new() -> Self {
        Self::default()
    }
    
    /// 记录缓存命中
    pub fn record_hit(&mut self) {
        self.hits += 1;
    }
    
    /// 记录缓存未命中
    pub fn record_miss(&mut self) {
        self.misses += 1;
    }
    
    /// 记录缓存插入
    pub fn record_insertion(&mut self, size_bytes: u64) {
        self.insertions += 1;
        self.entries += 1;
        self.size_bytes += size_bytes;
    }
    
    /// 记录缓存清理
    pub fn record_eviction(&mut self, size_bytes: u64) {
        self.evictions += 1;
        self.entries = self.entries.saturating_sub(1);
        self.size_bytes = self.size_bytes.saturating_sub(size_bytes);
    }
    
    /// 获取命中率
    pub fn hit_rate(&self) -> f64 {
        let total_requests = self.hits + self.misses;
        if total_requests == 0 {
            0.0
        } else {
            self.hits as f64 / total_requests as f64
        }
    }
    
    /// 获取未命中率
    pub fn miss_rate(&self) -> f64 {
        1.0 - self.hit_rate()
    }
}

impl Stats for CacheStats {
    fn reset(&mut self) {
        *self = Self::default();
    }
    
    fn merge(&mut self, other: &Self) {
        self.hits += other.hits;
        self.misses += other.misses;
        self.size_bytes += other.size_bytes;
        self.entries += other.entries;
        self.evictions += other.evictions;
        self.insertions += other.insertions;
    }
    
    fn summary(&self) -> String {
        format!(
            "缓存统计: 命中率={:.2}%, 条目数={}, 大小={}MB, 清理次数={}",
            self.hit_rate() * 100.0,
            self.entries,
            self.size_bytes / (1024 * 1024),
            self.evictions
        )
    }
}

/// 性能统计集合
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStatsCollection {
    /// 执行统计
    pub execution: ExecutionStats,
    /// 内存统计
    pub memory: MemoryStats,
    /// 缓存统计
    pub cache: CacheStats,
    /// 自定义统计
    pub custom: HashMap<String, f64>,
}

impl Default for PerformanceStatsCollection {
    fn default() -> Self {
        Self {
            execution: ExecutionStats::default(),
            memory: MemoryStats::default(),
            cache: CacheStats::default(),
            custom: HashMap::new(),
        }
    }
}

impl PerformanceStatsCollection {
    /// 创建新的性能统计集合
    pub fn new() -> Self {
        Self::default()
    }
    
    /// 添加自定义统计
    pub fn add_custom_stat(&mut self, key: String, value: f64) {
        self.custom.insert(key, value);
    }
    
    /// 获取自定义统计
    pub fn get_custom_stat(&self, key: &str) -> Option<f64> {
        self.custom.get(key).copied()
    }
    
    /// 生成完整报告
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== 性能统计报告 ===\n");
        report.push_str(&format!("{}\n", self.execution.summary()));
        report.push_str(&format!("{}\n", self.memory.summary()));
        report.push_str(&format!("{}\n", self.cache.summary()));
        
        if !self.custom.is_empty() {
            report.push_str("自定义统计:\n");
            for (key, value) in &self.custom {
                report.push_str(&format!("  {}: {:.2}\n", key, value));
            }
        }
        
        report
    }
}

impl Stats for PerformanceStatsCollection {
    fn reset(&mut self) {
        self.execution.reset();
        self.memory.reset();
        self.cache.reset();
        self.custom.clear();
    }
    
    fn merge(&mut self, other: &Self) {
        self.execution.merge(&other.execution);
        self.memory.merge(&other.memory);
        self.cache.merge(&other.cache);
        
        // 合并自定义统计
        for (key, value) in &other.custom {
            let entry = self.custom.entry(key.clone()).or_insert(0.0);
            *entry += value;
        }
    }
    
    fn summary(&self) -> String {
        self.generate_report()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_execution_stats() {
        let mut stats = ExecutionStats::new();
        
        // 记录一些执行
        stats.record_successful_execution(Duration::from_millis(10));
        stats.record_successful_execution(Duration::from_millis(20));
        stats.record_failed_execution(Duration::from_millis(5));
        
        assert_eq!(stats.total_executions, 3);
        assert_eq!(stats.successful_executions, 2);
        assert_eq!(stats.failed_executions, 1);
        assert_eq!(stats.success_rate(), 2.0 / 3.0);
        assert_eq!(stats.failure_rate(), 1.0 / 3.0);
    }
    
    #[test]
    fn test_memory_stats() {
        let mut stats = MemoryStats::new();
        stats.set_total_memory(1000);
        
        stats.record_allocation(100);
        stats.record_allocation(200);
        stats.record_deallocation(50);
        
        assert_eq!(stats.allocated_memory_bytes, 250);
        assert_eq!(stats.peak_memory_bytes, 300);
        assert_eq!(stats.utilization_ratio(), 0.25);
    }
    
    #[test]
    fn test_cache_stats() {
        let mut stats = CacheStats::new();
        
        // 记录一些缓存操作
        for _ in 0..8 {
            stats.record_hit();
        }
        for _ in 0..2 {
            stats.record_miss();
        }
        
        assert_eq!(stats.hit_rate(), 0.8);
        assert_eq!(stats.miss_rate(), 0.2);
    }
    
    #[test]
    fn test_stats_merge() {
        let mut stats1 = ExecutionStats::new();
        let mut stats2 = ExecutionStats::new();
        
        stats1.record_successful_execution(Duration::from_millis(10));
        stats2.record_successful_execution(Duration::from_millis(20));
        
        stats1.merge(&stats2);
        
        assert_eq!(stats1.total_executions, 2);
        assert_eq!(stats1.successful_executions, 2);
    }
}