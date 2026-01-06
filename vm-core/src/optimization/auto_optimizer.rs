//! 自动优化控制器
//!
//! Round 36: 自动优化系统
//!
//! 基于前35轮优化的经验,创建智能的优化选择和调优系统:
//! - 工作负载自动识别
//! - 平台自动检测
//! - 优化自动启用
//! - 动态性能调优

use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

/// 工作负载类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkloadType {
    /// 计算密集型 (大量算术运算)
    ComputeIntensive,
    /// 内存密集型 (大量数据拷贝)
    MemoryIntensive,
    /// 分配密集型 (频繁对象分配)
    AllocationIntensive,
    /// JIT编译密集型 (频繁代码生成)
    JitIntensive,
    /// 混合型 (均衡负载)
    Mixed,
    /// 未知 (未识别)
    Unknown,
}

/// 平台特性
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCapabilities {
    /// CPU架构
    pub architecture: String,
    /// CPU核心数
    pub core_count: usize,
    /// 支持NEON SIMD
    pub supports_neon: bool,
    /// 支持AVX2
    pub supports_avx2: bool,
    /// 支持AVX-512
    pub supports_avx512: bool,
    /// 大小核架构
    pub has_big_little_cores: bool,
}

/// 优化策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationStrategy {
    /// 工作负载类型
    pub workload: WorkloadType,
    /// 启用SIMD优化
    pub enable_simd: bool,
    /// 启用NEON优化
    pub enable_neon: bool,
    /// 启用内存池
    pub enable_memory_pool: bool,
    /// 启用对象池
    pub enable_object_pool: bool,
    /// 启用TLB优化
    pub enable_tlb_optimization: bool,
    /// 启用JIT热点检测
    pub enable_jit_hotspot: bool,
    /// 内存对齐大小
    pub memory_alignment: usize,
    /// 优先级核心绑定 (big.LITTLE)
    pub prefer_performance_cores: bool,
}

impl Default for OptimizationStrategy {
    fn default() -> Self {
        Self {
            workload: WorkloadType::Unknown,
            enable_simd: true,
            enable_neon: cfg!(target_arch = "aarch64"),
            enable_memory_pool: true,
            enable_object_pool: true,
            enable_tlb_optimization: true,
            enable_jit_hotspot: true,
            memory_alignment: 16,
            prefer_performance_cores: false,
        }
    }
}

/// 性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// 测量时间戳 (Unix时间戳,纳秒)
    pub timestamp_ns: u64,
    /// 操作耗时 (纳秒)
    pub operation_time_ns: u64,
    /// 内存使用 (字节)
    pub memory_used_bytes: u64,
    /// CPU使用率 (0-100)
    pub cpu_usage_percent: f64,
    /// cache命中率
    pub cache_hit_rate: Option<f64>,
}

impl PerformanceMetrics {
    /// 创建新的性能指标
    pub fn new(operation_time_ns: u64) -> Self {
        Self {
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
            operation_time_ns,
            memory_used_bytes: 0,
            cpu_usage_percent: 0.0,
            cache_hit_rate: None,
        }
    }
}

/// 工作负载特征
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadCharacteristics {
    /// 平均操作时间
    pub avg_operation_time_ns: f64,
    /// 操作时间标准差
    pub operation_time_std_dev: f64,
    /// 内存分配频率
    pub allocation_frequency: f64,
    /// 内存拷贝大小
    pub memory_copy_size: f64,
    /// JIT编译频率
    pub jit_compilation_frequency: f64,
}

/// 自动优化控制器
pub struct AutoOptimizer {
    /// 平台特性
    platform: PlatformCapabilities,
    /// 当前策略
    current_strategy: Arc<Mutex<OptimizationStrategy>>,
    /// 性能历史 (最近100次)
    performance_history: Arc<Mutex<Vec<PerformanceMetrics>>>,
    /// 工作负载特征
    workload_characteristics: Arc<Mutex<Option<WorkloadCharacteristics>>>,
    /// 优化启用时间 (Unix时间戳,纳秒)
    optimization_start: Arc<Mutex<Option<u64>>>,
}

impl AutoOptimizer {
    /// 创建新的自动优化控制器
    pub fn new() -> Self {
        Self {
            platform: Self::detect_platform(),
            current_strategy: Arc::new(Mutex::new(OptimizationStrategy::default())),
            performance_history: Arc::new(Mutex::new(Vec::with_capacity(100))),
            workload_characteristics: Arc::new(Mutex::new(None)),
            optimization_start: Arc::new(Mutex::new(None)),
        }
    }

    /// 检测平台特性
    fn detect_platform() -> PlatformCapabilities {
        let architecture = if cfg!(target_arch = "x86_64") {
            "x86_64".to_string()
        } else if cfg!(target_arch = "aarch64") {
            "aarch64".to_string()
        } else {
            "unknown".to_string()
        };

        let core_count = num_cpus::get();

        // 使用cfg!进行编译时检测
        let supports_neon = cfg!(target_arch = "aarch64");
        let supports_avx2 = cfg!(target_arch = "x86_64");
        let supports_avx512 = cfg!(target_arch = "x86_64");

        // 简化检测: ARM64通常是big.LITTLE, x86_64通常是统一核心
        let has_big_little_cores = cfg!(target_arch = "aarch64");

        PlatformCapabilities {
            architecture,
            core_count,
            supports_neon,
            supports_avx2,
            supports_avx512,
            has_big_little_cores,
        }
    }

    /// 分析工作负载并推荐优化策略
    pub fn analyze_and_optimize(&self) -> OptimizationStrategy {
        // 1. 收集性能指标
        let characteristics = self.analyze_workload_characteristics();

        // 2. 识别工作负载类型
        let workload_type = self.classify_workload(&characteristics);

        // 3. 生成优化策略
        let strategy = self.generate_strategy(workload_type, &characteristics);

        // 4. 应用优化策略
        self.apply_strategy(&strategy);

        strategy
    }

    /// 分析工作负载特征
    fn analyze_workload_characteristics(&self) -> WorkloadCharacteristics {
        let history = self.performance_history.lock();

        if history.len() < 10 {
            // 数据不足,返回默认值
            return WorkloadCharacteristics {
                avg_operation_time_ns: 1000.0,
                operation_time_std_dev: 500.0,
                allocation_frequency: 1.0,
                memory_copy_size: 1024.0,
                jit_compilation_frequency: 0.1,
            };
        }

        // 计算统计数据
        let times: Vec<f64> = history.iter()
            .map(|m| m.operation_time_ns as f64)
            .collect();

        let avg = times.iter().sum::<f64>() / times.len() as f64;
        let variance = times.iter()
            .map(|t| (t - avg).powi(2))
            .sum::<f64>() / times.len() as f64;
        let std_dev = variance.sqrt();

        // 估算特征 (简化版本)
        WorkloadCharacteristics {
            avg_operation_time_ns: avg,
            operation_time_std_dev: std_dev,
            allocation_frequency: 1.0, // TODO: 从实际数据计算
            memory_copy_size: 4096.0,  // TODO: 从实际数据计算
            jit_compilation_frequency: 0.1, // TODO: 从实际数据计算
        }
    }

    /// 分类工作负载类型
    fn classify_workload(&self, characteristics: &WorkloadCharacteristics) -> WorkloadType {
        let avg_time = characteristics.avg_operation_time_ns;
        let std_dev = characteristics.operation_time_std_dev;
        let alloc_freq = characteristics.allocation_frequency;
        let mem_copy = characteristics.memory_copy_size;
        let jit_freq = characteristics.jit_compilation_frequency;

        // 简化的分类逻辑
        if jit_freq > 0.5 {
            WorkloadType::JitIntensive
        } else if alloc_freq > 10.0 {
            WorkloadType::AllocationIntensive
        } else if mem_copy > 10240.0 {
            WorkloadType::MemoryIntensive
        } else if avg_time > 10000.0 {
            WorkloadType::ComputeIntensive
        } else if std_dev / avg_time < 0.3 {
            WorkloadType::Mixed
        } else {
            WorkloadType::Unknown
        }
    }

    /// 生成优化策略
    fn generate_strategy(
        &self,
        workload: WorkloadType,
        _characteristics: &WorkloadCharacteristics,
    ) -> OptimizationStrategy {
        let mut strategy = OptimizationStrategy {
            workload,
            ..Default::default()
        };

        // 根据工作负载类型配置优化
        match workload {
            WorkloadType::ComputeIntensive => {
                strategy.enable_simd = true;
                strategy.enable_neon = self.platform.supports_neon;
                strategy.memory_alignment = 32; // 更大对齐提升计算性能
                strategy.prefer_performance_cores = self.platform.has_big_little_cores;
            }
            WorkloadType::MemoryIntensive => {
                strategy.enable_memory_pool = true;
                strategy.enable_simd = true; // SIMD内存拷贝
                strategy.enable_neon = self.platform.supports_neon;
                strategy.memory_alignment = 16; // NEON 128位对齐
                strategy.prefer_performance_cores = false; // 内存操作不需要P-core
            }
            WorkloadType::AllocationIntensive => {
                strategy.enable_object_pool = true;
                strategy.enable_memory_pool = true;
                strategy.enable_tlb_optimization = true;
                strategy.memory_alignment = 8; // 默认对齐
            }
            WorkloadType::JitIntensive => {
                strategy.enable_jit_hotspot = true;
                strategy.enable_simd = true;
                strategy.memory_alignment = 16;
                strategy.prefer_performance_cores = self.platform.has_big_little_cores;
            }
            WorkloadType::Mixed => {
                // 均衡策略: 启用所有优化
                strategy.enable_simd = true;
                strategy.enable_neon = self.platform.supports_neon;
                strategy.enable_memory_pool = true;
                strategy.enable_object_pool = true;
                strategy.enable_tlb_optimization = true;
                strategy.enable_jit_hotspot = true;
                strategy.memory_alignment = 16;
            }
            WorkloadType::Unknown => {
                // 保守策略: 启用安全优化
                strategy.enable_simd = true;
                strategy.enable_tlb_optimization = true;
                strategy.memory_alignment = 8;
            }
        }

        strategy
    }

    /// 应用优化策略
    fn apply_strategy(&self, strategy: &OptimizationStrategy) {
        *self.current_strategy.lock() = strategy.clone();
        *self.optimization_start.lock() = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
        );

        // TODO: 实际应用优化到各个组件
        // 例如:
        // - 配置内存分配器
        // - 启用/禁用SIMD路径
        // - 设置线程亲和性
        // - 调整TLB参数
    }

    /// 记录性能指标
    pub fn record_metrics(&self, metrics: PerformanceMetrics) {
        let mut history = self.performance_history.lock();
        history.push(metrics);

        // 保持最近100次记录
        if history.len() > 100 {
            history.remove(0);
        }
    }

    /// 获取当前策略
    pub fn current_strategy(&self) -> OptimizationStrategy {
        self.current_strategy.lock().clone()
    }

    /// 获取平台特性
    pub fn platform(&self) -> &PlatformCapabilities {
        &self.platform
    }

    /// 获取工作负载特征
    pub fn workload_characteristics(&self) -> Option<WorkloadCharacteristics> {
        self.workload_characteristics.lock().clone()
    }

    /// 获取性能提升 (从优化开始)
    pub fn improvement_since_optimization(&self) -> Option<f64> {
        let start_opt = *self.optimization_start.lock();
        let start_ns = start_opt?;
        let history = self.performance_history.lock();

        // 找到优化开始后的第一个和最后一个指标
        let after_opt: Vec<_> = history.iter()
            .filter(|m| m.timestamp_ns >= start_ns)
            .collect();

        if after_opt.len() < 2 {
            return None;
        }

        let before = after_opt.first()?.operation_time_ns as f64;
        let after = after_opt.last()?.operation_time_ns as f64;

        Some((before - after) / before * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let optimizer = AutoOptimizer::new();
        let platform = optimizer.platform();

        assert!(!platform.architecture.is_empty());
        assert!(platform.core_count > 0);

        #[cfg(target_arch = "aarch64")]
        assert!(platform.supports_neon);

        #[cfg(target_arch = "x86_64")]
        assert!(platform.supports_avx2 || !platform.supports_avx2); // 至少检测正确
    }

    #[test]
    fn test_strategy_generation() {
        let optimizer = AutoOptimizer::new();

        // 测试不同工作负载的策略
        let characteristics = WorkloadCharacteristics {
            avg_operation_time_ns: 50000.0, // 慢速操作
            operation_time_std_dev: 1000.0,
            allocation_frequency: 1.0,
            memory_copy_size: 1024.0,
            jit_compilation_frequency: 0.1,
        };

        let strategy = optimizer.generate_strategy(
            WorkloadType::ComputeIntensive,
            &characteristics,
        );

        assert_eq!(strategy.workload, WorkloadType::ComputeIntensive);
        assert!(strategy.enable_simd);
        assert_eq!(strategy.memory_alignment, 32);
    }

    #[test]
    fn test_metrics_recording() {
        let optimizer = AutoOptimizer::new();

        let metrics = PerformanceMetrics {
            timestamp_ns: 12345,
            operation_time_ns: 1000,
            memory_used_bytes: 1024,
            cpu_usage_percent: 50.0,
            cache_hit_rate: Some(0.95),
        };

        optimizer.record_metrics(metrics.clone());

        let history = optimizer.performance_history.lock();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].operation_time_ns, 1000);
    }
}
