//! GPU执行器
//!
//! 提供高级GPU执行接口,包括内核缓存、错误处理和性能监控。

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use super::device::{GpuArg, GpuCompute, GpuDeviceManager, GpuKernel};
use super::error::{GpuError, GpuResult};

/// GPU执行配置
#[derive(Debug, Clone)]
pub struct GpuExecutionConfig {
    /// GPU内核源代码
    pub kernel_source: String,
    /// 内核名称
    pub kernel_name: String,
    /// 网格维度 (x, y, z)
    pub grid_dim: (u32, u32, u32),
    /// 块维度 (x, y, z)
    pub block_dim: (u32, u32, u32),
    /// 内核参数
    pub args: Vec<GpuArg>,
    /// 共享内存大小
    pub shared_memory_size: usize,
}

/// GPU执行器
///
/// 管理GPU设备执行,提供内核缓存、错误处理和性能监控。
pub struct GpuExecutor {
    /// GPU设备管理器
    device_manager: Arc<GpuDeviceManager>,
    /// 内核缓存 (kernel_name -> GpuKernel)
    kernel_cache: Arc<RwLock<HashMap<String, GpuKernel>>>,
    /// 性能统计
    stats: Arc<RwLock<GpuExecutorStats>>,
    /// 配置
    config: GpuExecutorConfig,
}

/// GPU执行器配置
#[derive(Debug, Clone)]
pub struct GpuExecutorConfig {
    /// 是否启用内核缓存
    pub enable_kernel_cache: bool,
    /// 内核缓存最大数量
    pub max_cache_size: usize,
    /// 是否启用性能监控
    pub enable_performance_monitoring: bool,
    /// 是否在GPU失败时回退到CPU
    pub enable_cpu_fallback: bool,
    /// GPU执行超时(秒)
    pub execution_timeout_secs: u64,
}

impl Default for GpuExecutorConfig {
    fn default() -> Self {
        Self {
            enable_kernel_cache: true,
            max_cache_size: 100,
            enable_performance_monitoring: true,
            enable_cpu_fallback: true,
            execution_timeout_secs: 30,
        }
    }
}

/// GPU执行器性能统计
#[derive(Debug, Clone, Default)]
pub struct GpuExecutorStats {
    /// 总执行次数
    pub total_executions: u64,
    /// GPU执行成功次数
    pub gpu_success_count: u64,
    /// GPU执行失败次数
    pub gpu_failure_count: u64,
    /// CPU回退次数
    pub cpu_fallback_count: u64,
    /// 内核编译次数
    pub kernel_compilation_count: u64,
    /// 缓存命中次数
    pub cache_hits: u64,
    /// 缓存未命中次数
    pub cache_misses: u64,
    /// 总GPU执行时间(纳秒)
    pub total_gpu_time_ns: u64,
    /// 总CPU回退时间(纳秒)
    pub total_cpu_time_ns: u64,
}

impl GpuExecutorStats {
    /// 计算GPU成功率
    pub fn gpu_success_rate(&self) -> f64 {
        if self.total_executions == 0 {
            0.0
        } else {
            (self.gpu_success_count as f64) / (self.total_executions as f64)
        }
    }

    /// 计算缓存命中率
    pub fn cache_hit_rate(&self) -> f64 {
        let total_cache_accesses = self.cache_hits + self.cache_misses;
        if total_cache_accesses == 0 {
            0.0
        } else {
            (self.cache_hits as f64) / (total_cache_accesses as f64)
        }
    }

    /// 计算平均GPU执行时间(微秒)
    pub fn avg_gpu_time_us(&self) -> f64 {
        if self.gpu_success_count == 0 {
            0.0
        } else {
            (self.total_gpu_time_ns as f64) / (self.gpu_success_count as f64) / 1000.0
        }
    }
}

/// GPU执行结果
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// 是否在GPU上执行
    pub executed_on_gpu: bool,
    /// 执行成功
    pub success: bool,
    /// 执行时间(纳秒)
    pub execution_time_ns: u64,
    /// 错误信息(如果失败)
    pub error: Option<String>,
}

impl GpuExecutor {
    /// 创建新的GPU执行器
    ///
    /// # Arguments
    ///
    /// * `config` - 执行器配置
    pub fn new(config: GpuExecutorConfig) -> Self {
        let device_manager = Arc::new(GpuDeviceManager::new());

        Self {
            device_manager,
            kernel_cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(GpuExecutorStats::default())),
            config,
        }
    }

    /// 使用默认配置创建GPU执行器
    pub fn with_default_config() -> Self {
        Self::new(GpuExecutorConfig::default())
    }

    /// 检查是否有可用的GPU设备
    pub fn has_gpu(&self) -> bool {
        self.device_manager.has_gpu()
    }

    /// 获取GPU设备信息
    pub fn device_info(&self) -> Option<super::device::GpuDeviceInfo> {
        self.device_manager
            .default_device()
            .map(|device| device.device_info())
    }

    /// 检查指令是否可以在GPU上执行
    ///
    /// # Arguments
    ///
    /// * `_instruction` - 指令字节码
    ///
    /// # Note
    /// 当前简化实现：如果有GPU就返回true
    /// 完整实现应该分析指令类型（矩阵运算、SIMD、并行计算等）
    pub fn can_execute_on_gpu(&self, _instruction: &[u8]) -> bool {
        // 简化实现：检查GPU可用性
        // 未来改进：实现指令启发式分析，判断是否适合GPU执行
        self.has_gpu()
    }

    /// 在GPU上执行指令
    ///
    /// # Arguments
    ///
    /// * `kernel_source` - GPU内核源代码
    /// * `kernel_name` - 内核名称
    /// * `grid_dim` - 网格维度 (x, y, z)
    /// * `block_dim` - 块维度 (x, y, z)
    /// * `args` - 内核参数
    /// * `shared_memory_size` - 共享内存大小
    pub fn execute_on_gpu(
        &self,
        kernel_source: &str,
        kernel_name: &str,
        grid_dim: (u32, u32, u32),
        block_dim: (u32, u32, u32),
        args: &[GpuArg],
        shared_memory_size: usize,
    ) -> GpuResult<ExecutionResult> {
        let start = Instant::now();

        // 检查GPU可用性
        let device = self
            .device_manager
            .default_device()
            .ok_or(GpuError::NoDeviceAvailable)?;

        // 更新统计
        {
            let mut stats = self.stats.write().unwrap();
            stats.total_executions += 1;
        }

        // 编译或获取缓存的内核
        let kernel = if self.config.enable_kernel_cache {
            self.get_or_compile_kernel(device, kernel_source, kernel_name)?
        } else {
            self.compile_kernel(device, kernel_source, kernel_name)?
        };

        // 执行内核
        let result = device.execute_kernel(&kernel, grid_dim, block_dim, args, shared_memory_size);

        let execution_time_ns = start.elapsed().as_nanos() as u64;

        match result {
            Ok(gpu_result) => {
                // 更新成功统计
                {
                    let mut stats = self.stats.write().unwrap();
                    stats.gpu_success_count += 1;
                    stats.total_gpu_time_ns += execution_time_ns;
                }

                Ok(ExecutionResult {
                    executed_on_gpu: true,
                    success: gpu_result.success,
                    execution_time_ns,
                    error: None,
                })
            }
            Err(e) => {
                // 更新失败统计
                {
                    let mut stats = self.stats.write().unwrap();
                    stats.gpu_failure_count += 1;
                }

                Ok(ExecutionResult {
                    executed_on_gpu: true,
                    success: false,
                    execution_time_ns,
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// 在GPU上执行指令,失败时回退到CPU
    ///
    /// # Arguments
    ///
    /// * `config` - 执行配置
    /// * `cpu_fallback` - CPU回退函数
    pub fn execute_with_fallback<F>(
        &self,
        config: &GpuExecutionConfig,
        cpu_fallback: F,
    ) -> ExecutionResult
    where
        F: FnOnce() -> Result<(), String>,
    {
        // 如果没有GPU或不启用CPU回退,直接执行CPU回退
        if !self.has_gpu() || !self.config.enable_cpu_fallback {
            let start = Instant::now();
            match cpu_fallback() {
                Ok(()) => {
                    let execution_time_ns = start.elapsed().as_nanos() as u64;
                    return ExecutionResult {
                        executed_on_gpu: false,
                        success: true,
                        execution_time_ns,
                        error: None,
                    };
                }
                Err(e) => {
                    return ExecutionResult {
                        executed_on_gpu: false,
                        success: false,
                        execution_time_ns: 0,
                        error: Some(e),
                    };
                }
            }
        }

        // 尝试在GPU上执行
        match self.execute_on_gpu(
            &config.kernel_source,
            &config.kernel_name,
            config.grid_dim,
            config.block_dim,
            &config.args,
            config.shared_memory_size,
        ) {
            Ok(result) if result.success => result,

            // GPU执行失败,回退到CPU
            Ok(result) => {
                log::warn!(
                    "GPU execution failed: {:?}, falling back to CPU",
                    result.error
                );

                let start = Instant::now();
                match cpu_fallback() {
                    Ok(()) => {
                        let execution_time_ns = start.elapsed().as_nanos() as u64;

                        // 更新CPU回退统计
                        {
                            let mut stats = self.stats.write().unwrap();
                            stats.cpu_fallback_count += 1;
                            stats.total_cpu_time_ns += execution_time_ns;
                        }

                        ExecutionResult {
                            executed_on_gpu: false,
                            success: true,
                            execution_time_ns,
                            error: None,
                        }
                    }
                    Err(e) => ExecutionResult {
                        executed_on_gpu: false,
                        success: false,
                        execution_time_ns: 0,
                        error: Some(e),
                    },
                }
            }

            // 严重错误(如设备不可用)
            Err(e) => {
                log::error!("GPU error: {}, falling back to CPU", e);

                let start = Instant::now();
                match cpu_fallback() {
                    Ok(()) => {
                        let execution_time_ns = start.elapsed().as_nanos() as u64;

                        // 更新CPU回退统计
                        {
                            let mut stats = self.stats.write().unwrap();
                            stats.cpu_fallback_count += 1;
                            stats.total_cpu_time_ns += execution_time_ns;
                        }

                        ExecutionResult {
                            executed_on_gpu: false,
                            success: true,
                            execution_time_ns,
                            error: None,
                        }
                    }
                    Err(e) => ExecutionResult {
                        executed_on_gpu: false,
                        success: false,
                        execution_time_ns: 0,
                        error: Some(e),
                    },
                }
            }
        }
    }

    /// 获取或编译内核(带缓存)
    fn get_or_compile_kernel(
        &self,
        device: &dyn GpuCompute,
        source: &str,
        kernel_name: &str,
    ) -> GpuResult<GpuKernel> {
        // 尝试从缓存获取
        if self.config.enable_kernel_cache {
            {
                let cache = self.kernel_cache.read().unwrap();
                if let Some(kernel) = cache.get(kernel_name) {
                    // 更新缓存命中统计
                    let mut stats = self.stats.write().unwrap();
                    stats.cache_hits += 1;
                    return Ok(kernel.clone());
                }
            }

            // 更新缓存未命中统计
            {
                let mut stats = self.stats.write().unwrap();
                stats.cache_misses += 1;
            }
        }

        // 编译内核
        let kernel = self.compile_kernel(device, source, kernel_name)?;

        // 添加到缓存
        if self.config.enable_kernel_cache {
            let mut cache = self.kernel_cache.write().unwrap();

            // 如果缓存已满,移除最旧的条目
            if cache.len() >= self.config.max_cache_size {
                // 简化的缓存淘汰策略：FIFO（先进先出）
                // 未来改进：实现LRU（最近最少使用）策略
                if let Some(key) = cache.keys().next().cloned() {
                    cache.remove(&key);
                }
            }

            cache.insert(kernel_name.to_string(), kernel.clone());
        }

        Ok(kernel)
    }

    /// 编译内核
    fn compile_kernel(
        &self,
        device: &dyn GpuCompute,
        source: &str,
        kernel_name: &str,
    ) -> GpuResult<GpuKernel> {
        // 更新编译统计
        {
            let mut stats = self.stats.write().unwrap();
            stats.kernel_compilation_count += 1;
        }

        device.compile_kernel(source, kernel_name)
    }

    /// 清空内核缓存
    pub fn clear_cache(&self) {
        let mut cache = self.kernel_cache.write().unwrap();
        cache.clear();
    }

    /// 获取性能统计
    pub fn stats(&self) -> GpuExecutorStats {
        let stats = self.stats.read().unwrap();
        stats.clone()
    }

    /// 重置性能统计
    pub fn reset_stats(&self) {
        let mut stats = self.stats.write().unwrap();
        *stats = GpuExecutorStats::default();
    }

    /// 打印性能统计
    pub fn print_stats(&self) {
        let stats = self.stats();
        println!("=== GPU Executor Statistics ===");
        println!("Total executions: {}", stats.total_executions);
        println!("GPU success count: {}", stats.gpu_success_count);
        println!("GPU failure count: {}", stats.gpu_failure_count);
        println!("CPU fallback count: {}", stats.cpu_fallback_count);
        println!(
            "Kernel compilation count: {}",
            stats.kernel_compilation_count
        );
        println!("Cache hits: {}", stats.cache_hits);
        println!("Cache misses: {}", stats.cache_misses);
        println!("Cache hit rate: {:.2}%", stats.cache_hit_rate() * 100.0);
        println!("GPU success rate: {:.2}%", stats.gpu_success_rate() * 100.0);
        println!("Avg GPU time: {:.2} μs", stats.avg_gpu_time_us());
    }
}

impl Default for GpuExecutor {
    fn default() -> Self {
        Self::with_default_config()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_creation() {
        let executor = GpuExecutor::default();
        // 执行器应该成功创建(即使没有GPU)
        assert!(executor.stats().total_executions == 0);
    }

    #[test]
    fn test_stats_default() {
        let stats = GpuExecutorStats::default();
        assert_eq!(stats.total_executions, 0);
        assert_eq!(stats.gpu_success_count, 0);
        assert_eq!(stats.gpu_failure_count, 0);
    }

    #[test]
    fn test_stats_calculations() {
        let mut stats = GpuExecutorStats::default();
        stats.total_executions = 100;
        stats.gpu_success_count = 80;
        stats.gpu_failure_count = 20;
        stats.cache_hits = 30;
        stats.cache_misses = 10;
        stats.total_gpu_time_ns = 1_000_000; // 1ms

        assert!((stats.gpu_success_rate() - 0.8).abs() < 0.01);
        assert!((stats.cache_hit_rate() - 0.75).abs() < 0.01);
        assert!((stats.avg_gpu_time_us() - 12.5).abs() < 0.1);
    }
}
