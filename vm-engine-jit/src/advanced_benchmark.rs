//! 高级性能基准测试
//!
//! 本模块实现了高级性能基准测试功能，用于全面评估JIT引擎的性能表现，
//! 包括编译速度、执行速度、内存使用、缓存效率等多个维度。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use vm_core::{GuestAddr, VmError, MMU};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};
use crate::core::{JITEngine, JITConfig};
use crate::performance_benchmark::PerformanceBenchmarker;
use serde::{Serialize, Deserialize};

/// 基准测试配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// 测试名称
    pub name: String,
    /// 测试描述
    pub description: String,
    /// 测试迭代次数
    pub iterations: usize,
    /// 预热迭代次数
    pub warmup_iterations: usize,
    /// 测试超时时间
    pub timeout: Duration,
    /// 是否启用内存分析
    pub enable_memory_analysis: bool,
    /// 是否启用缓存分析
    pub enable_cache_analysis: bool,
    /// 是否启用并发测试
    pub enable_concurrent_tests: bool,
    /// 并发线程数
    pub concurrent_threads: usize,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            description: "默认基准测试".to_string(),
            iterations: 1000,
            warmup_iterations: 100,
            timeout: Duration::from_secs(60),
            enable_memory_analysis: true,
            enable_cache_analysis: true,
            enable_concurrent_tests: false,
            concurrent_threads: 4,
        }
    }
}

/// 基准测试结果
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// 测试名称
    pub name: String,
    /// 测试开始时间
    pub start_time: Instant,
    /// 测试结束时间
    pub end_time: Instant,
    /// 总执行时间
    pub total_duration: Duration,
    /// 平均执行时间
    pub avg_execution_time: Duration,
    /// 最小执行时间
    pub min_execution_time: Duration,
    /// 最大执行时间
    pub max_execution_time: Duration,
    /// 标准差
    pub std_deviation: Duration,
    /// 中位数
    pub median: Duration,
    /// 第95百分位数
    pub p95: Duration,
    /// 第99百分位数
    pub p99: Duration,
    /// 每秒操作数
    pub ops_per_second: f64,
    /// 内存使用统计
    pub memory_stats: Option<MemoryStats>,
    /// 缓存统计
    pub cache_stats: Option<CacheStats>,
    /// 编译统计
    pub compilation_stats: CompilationStats,
    /// 错误信息
    pub errors: Vec<String>,
}

/// 内存使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// 初始内存使用量 (字节)
    pub initial_memory: u64,
    /// 峰值内存使用量 (字节)
    pub peak_memory: u64,
    /// 平均内存使用量 (字节)
    pub avg_memory: u64,
    /// 内存分配次数
    pub allocation_count: u64,
    /// 内存释放次数
    pub deallocation_count: u64,
    /// 内存碎片率
    pub fragmentation_ratio: f64,
}

/// 缓存统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// 缓存命中次数
    pub hits: u64,
    /// 缓存未命中次数
    pub misses: u64,
    /// 命中率
    pub hit_rate: f64,
    /// 缓存大小 (字节)
    pub cache_size: u64,
    /// 缓存条目数
    pub entry_count: u64,
    /// 驱逐次数
    pub evictions: u64,
}

/// 编译统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationStats {
    /// 编译次数
    pub compilation_count: u64,
    /// 总编译时间
    pub total_compilation_time: Duration,
    /// 平均编译时间
    pub avg_compilation_time: Duration,
    /// 成功编译次数
    pub successful_compilations: u64,
    /// 失败编译次数
    pub failed_compilations: u64,
    /// 代码生成统计
    pub codegen_stats: CodegenStats,
}

/// 代码生成统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodegenStats {
    /// 生成的机器指令数
    pub machine_instructions: u64,
    /// 优化后的指令数
    pub optimized_instructions: u64,
    /// 原始指令数
    pub original_instructions: u64,
    /// 优化率
    pub optimization_ratio: f64,
    /// 代码大小 (字节)
    pub code_size: u64,
}

/// 测试用例生成器
pub struct BenchmarkTestCaseGenerator;

impl BenchmarkTestCaseGenerator {
    /// 生成简单算术测试用例
    pub fn generate_arithmetic_test(pc: GuestAddr, complexity: usize) -> IRBlock {
        let mut builder = IRBuilder::new(pc);
        
        // 生成一系列算术操作
        for i in 0..complexity {
            let dst = (i % 16) as u32;
            let src1 = ((i + 1) % 16) as u32;
            let src2 = ((i + 2) % 16) as u32;
            
            match i % 4 {
                0 => builder.push(IROp::Add { dst, src1, src2 }),
                1 => builder.push(IROp::Sub { dst, src1, src2 }),
                2 => builder.push(IROp::Mul { dst, src1, src2 }),
                _ => builder.push(IROp::Div { dst, src1, src2, signed: false }),
            }
        }
        
        builder.set_term(Terminator::Ret);
        builder.build()
    }
    
    /// 生成内存访问测试用例
    pub fn generate_memory_test(pc: GuestAddr, access_count: usize) -> IRBlock {
        let mut builder = IRBuilder::new(pc);
        
        // 生成一系列内存访问操作
        for i in 0..access_count {
            let dst = (i % 16) as u32;
            let base = 0; // 使用寄存器0作为基址
            let offset = (i * 64) as i64;
            
            match i % 3 {
                0 => builder.push(IROp::Load { dst, base, offset, size: 8, flags: vm_ir::MemFlags::default() }),
                1 => builder.push(IROp::Store { src: dst, base, offset, size: 8, flags: vm_ir::MemFlags::default() }),
                _ => {
                    // 先加载再存储
                    builder.push(IROp::Load { dst, base, offset, size: 8, flags: vm_ir::MemFlags::default() });
                    builder.push(IROp::Store { src: dst, base, offset: offset + 8, size: 8, flags: vm_ir::MemFlags::default() });
                }
            }
        }
        
        builder.set_term(Terminator::Ret);
        builder.build()
    }
    
    /// 生成控制流测试用例
    pub fn generate_control_flow_test(pc: GuestAddr, branch_count: usize) -> IRBlock {
        let mut builder = IRBuilder::new(pc);
        
        // 生成一系列条件分支
        for i in 0..branch_count {
            let dst = (i % 16) as u32;
            let src1 = ((i + 1) % 16) as u32;
            let src2 = ((i + 2) % 16) as u32;
            
            // 算术操作
            builder.push(IROp::Add { dst, src1, src2 });
            
            // 无条件跳转（模拟控制流）
            let target_pc = pc + ((i + 1) * 10) as u64;
            // 注意：IROp中没有Jmp，暂时跳过
            // builder.push(IROp::Jmp { target: target_pc });
        }
        
        builder.set_term(Terminator::Ret);
        builder.build()
    }
    
    /// 生成SIMD友好测试用例
    pub fn generate_simd_test(pc: GuestAddr, vector_count: usize) -> IRBlock {
        let mut builder = IRBuilder::new(pc);
        
        // 生成一系列适合SIMD的向量操作
        for i in 0..vector_count {
            let dst = (i % 16) as u32;
            let src1 = ((i + 1) % 16) as u32;
            let src2 = ((i + 2) % 16) as u32;
            
            // 连续的相同操作，适合SIMD向量化
            for _ in 0..4 {
                builder.push(IROp::Add { dst, src1, src2 });
            }
        }
        
        builder.set_term(Terminator::Ret);
        builder.build()
    }
    
    /// 生成混合测试用例
    pub fn generate_mixed_test(pc: GuestAddr, size: usize) -> IRBlock {
        let mut builder = IRBuilder::new(pc);
        
        for i in 0..size {
            let dst = (i % 16) as u32;
            let src1 = ((i + 1) % 16) as u32;
            let src2 = ((i + 2) % 16) as u32;
            
            match i % 5 {
                0 => builder.push(IROp::MovImm { dst, imm: (i * 10) as u64 }),
                1 => builder.push(IROp::Add { dst, src1, src2 }),
                2 => {
                    let base = 0; // 使用寄存器0作为基址
                    let offset = (i * 64) as i64;
                    builder.push(IROp::Load { dst, base, offset, size: 8, flags: vm_ir::MemFlags::default() });
                }
                3 => {
                    let base = 0; // 使用寄存器0作为基址
                    let offset = (i * 64) as i64;
                    builder.push(IROp::Store { src: dst, base, offset, size: 8, flags: vm_ir::MemFlags::default() });
                }
                _ => builder.push(IROp::Mul { dst, src1, src2 }),
            }
        }
        
        builder.set_term(Terminator::Ret);
        builder.build()
    }
}

/// 高级性能基准测试器
pub struct AdvancedPerformanceBenchmarker {
    /// JIT引擎
    jit_engine: Arc<JITEngine>,
    /// 测试结果
    results: Arc<Mutex<HashMap<String, BenchmarkResult>>>,
    /// 全局统计
    global_stats: Arc<Mutex<GlobalBenchmarkStats>>,
}

/// 全局基准测试统计
#[derive(Debug, Default)]
pub struct GlobalBenchmarkStats {
    /// 总测试数
    pub total_tests: u64,
    /// 总执行时间
    pub total_execution_time: Duration,
    /// 总编译时间
    pub total_compilation_time: Duration,
    /// 总内存使用量
    pub total_memory_usage: u64,
    /// 测试覆盖的PC地址
    pub covered_pcs: HashSet<GuestAddr>,
}

impl AdvancedPerformanceBenchmarker {
    /// 创建新的高级性能基准测试器
    pub fn new(jit_engine: Arc<JITEngine>) -> Self {
        Self {
            jit_engine,
            results: Arc::new(Mutex::new(HashMap::new())),
            global_stats: Arc::new(Mutex::new(GlobalBenchmarkStats::default())),
        }
    }

    /// Helper method to acquire results lock with error handling
    fn lock_results(&self) -> Result<std::sync::MutexGuard<HashMap<String, BenchmarkResult>>, String> {
        self.results.lock().map_err(|e| format!("Failed to acquire results lock: {}", e))
    }

    /// Helper method to acquire global stats lock with error handling
    fn lock_global_stats(&self) -> Result<std::sync::MutexGuard<GlobalBenchmarkStats>, String> {
        self.global_stats.lock().map_err(|e| format!("Failed to acquire global stats lock: {}", e))
    }
    
    /// 运行基准测试
    pub fn run_benchmark(&self, config: &BenchmarkConfig, test_case: IRBlock) -> BenchmarkResult {
        log::info!("开始运行基准测试: {}", config.name);
        
        let start_time = Instant::now();
        let mut execution_times = Vec::with_capacity(config.iterations);
        let mut errors = Vec::new();
        
        // 预热阶段
        for _ in 0..config.warmup_iterations {
            if let Err(e) = self.execute_test_case(&test_case) {
                log::warn!("预热阶段执行失败: {}", e);
            }
        }
        
        // 重置性能统计
        // 注意：这里需要实际的实现，暂时跳过
        
        // 正式测试阶段
        for i in 0..config.iterations {
            let iteration_start = Instant::now();
            
            match self.execute_test_case(&test_case) {
                Ok(_) => {
                    let execution_time = iteration_start.elapsed();
                    execution_times.push(execution_time);
                }
                Err(e) => {
                    errors.push(format!("迭代 {} 失败: {}", i, e));
                }
            }
            
            // 检查超时
            if start_time.elapsed() > config.timeout {
                errors.push(format!("测试超时，已完成 {} 次迭代", i));
                break;
            }
        }
        
        let end_time = Instant::now();
        let total_duration = end_time - start_time;
        
        // 计算统计信息
        let (avg_time, min_time, max_time, std_dev, median, p95, p99, ops_per_sec) = 
            if !execution_times.is_empty() {
                Self::calculate_time_stats(&execution_times)
            } else {
                (Duration::ZERO, Duration::ZERO, Duration::ZERO, Duration::ZERO, Duration::ZERO, Duration::ZERO, Duration::ZERO, 0.0)
            };
        
        // 收集内存统计
        let memory_stats = if config.enable_memory_analysis {
            Some(self.collect_memory_stats())
        } else {
            None
        };
        
        // 收集缓存统计
        let cache_stats = if config.enable_cache_analysis {
            Some(self.collect_cache_stats())
        } else {
            None
        };
        
        // 收集编译统计
        let compilation_stats = self.collect_compilation_stats();
        
        // 创建结果
        let result = BenchmarkResult {
            name: config.name.clone(),
            start_time,
            end_time,
            total_duration,
            avg_execution_time: avg_time,
            min_execution_time: min_time,
            max_execution_time: max_time,
            std_deviation: std_dev,
            median,
            p95,
            p99,
            ops_per_second: ops_per_sec,
            memory_stats,
            cache_stats,
            compilation_stats,
            errors,
        };
        
        // 保存结果
        {
            match self.lock_results() {
                Ok(mut results) => {
                    results.insert(config.name.clone(), result.clone());
                }
                Err(e) => {
                    log::error!("Failed to save results: {}", e);
                }
            }
        }
        
        // 更新全局统计
        self.update_global_stats(&result);
        
        log::info!("基准测试完成: {}, 平均执行时间: {:?}", config.name, avg_time);
        
        result
    }
    
    /// 执行测试用例
    fn execute_test_case(&self, test_case: &IRBlock) -> Result<(), VmError> {
        // 注意：由于JITEngine需要可变引用，而这里是不可变引用，
        // 我们需要使用Arc<Mutex<JITEngine>>或者修改接口
        // 为了简化，这里先返回Ok，实际实现需要处理执行逻辑
        
        Ok(())
    }
    
    /// 计算时间统计
    fn calculate_time_stats(times: &[Duration]) -> (Duration, Duration, Duration, Duration, Duration, Duration, Duration, f64) {
        if times.is_empty() {
            return (Duration::ZERO, Duration::ZERO, Duration::ZERO, Duration::ZERO, Duration::ZERO, Duration::ZERO, Duration::ZERO, 0.0);
        }

        let total: Duration = times.iter().sum();
        let avg = total / times.len() as u32;

        let min = times.iter().min().copied().unwrap_or(Duration::ZERO);
        let max = times.iter().max().copied().unwrap_or(Duration::ZERO);
        
        // 计算标准差
        let variance = times.iter()
            .map(|&time| {
                let diff = time.as_nanos() as f64 - avg.as_nanos() as f64;
                diff * diff
            })
            .sum::<f64>() / times.len() as f64;
        let std_dev = Duration::from_nanos(variance.sqrt() as u64);
        
        // 计算中位数
        let mut sorted_times = times.to_vec();
        sorted_times.sort();
        let median = if sorted_times.len() % 2 == 0 {
            let mid1 = sorted_times[sorted_times.len() / 2 - 1];
            let mid2 = sorted_times[sorted_times.len() / 2];
            (mid1 + mid2) / 2
        } else {
            sorted_times[sorted_times.len() / 2]
        };
        
        // 计算百分位数
        let p95_idx = (sorted_times.len() as f64 * 0.95) as usize;
        let p99_idx = (sorted_times.len() as f64 * 0.99) as usize;
        let p95 = sorted_times[p95_idx.min(sorted_times.len() - 1)];
        let p99 = sorted_times[p99_idx.min(sorted_times.len() - 1)];
        
        // 计算每秒操作数
        let ops_per_sec = if avg.as_nanos() > 0 {
            1_000_000_000.0 / avg.as_nanos() as f64
        } else {
            0.0
        };
        
        (avg, min, max, std_dev, median, p95, p99, ops_per_sec)
    }
    
    /// 收集内存统计
    fn collect_memory_stats(&self) -> MemoryStats {
        // 注意：这里需要实际的实现，暂时返回默认值
        MemoryStats {
            initial_memory: 0,
            peak_memory: 0,
            avg_memory: 0,
            allocation_count: 0,
            deallocation_count: 0,
            fragmentation_ratio: 0.0,
        }
    }
    
    /// 收集缓存统计
    fn collect_cache_stats(&self) -> CacheStats {
        // 注意：这里需要实际的实现，暂时返回默认值
        CacheStats {
            hits: 0,
            misses: 0,
            hit_rate: 0.0,
            cache_size: 0,
            entry_count: 0,
            evictions: 0,
        }
    }
    
    /// 收集编译统计
    fn collect_compilation_stats(&self) -> CompilationStats {
        // 注意：这里需要实际的实现，暂时返回默认值
        CompilationStats {
            compilation_count: 0,
            total_compilation_time: Duration::ZERO,
            avg_compilation_time: Duration::ZERO,
            successful_compilations: 0,
            failed_compilations: 0,
            codegen_stats: CodegenStats {
                machine_instructions: 0,
                optimized_instructions: 0,
                original_instructions: 0,
                optimization_ratio: 0.0,
                code_size: 0,
            },
        }
    }
    
    /// 更新全局统计
    fn update_global_stats(&self, result: &BenchmarkResult) {
        match self.lock_global_stats() {
            Ok(mut global_stats) => {
                global_stats.total_tests += 1;
                global_stats.total_execution_time += result.total_duration;
                global_stats.total_compilation_time += result.compilation_stats.total_compilation_time;

                if let Some(ref memory_stats) = result.memory_stats {
                    global_stats.total_memory_usage += memory_stats.peak_memory;
                }
            }
            Err(e) => {
                log::error!("Failed to update global stats: {}", e);
            }
        }
    }
    
    /// 获取测试结果
    pub fn get_result(&self, name: &str) -> Option<BenchmarkResult> {
        match self.lock_results() {
            Ok(results) => results.get(name).cloned(),
            Err(e) => {
                log::error!("Failed to get result: {}", e);
                None
            }
        }
    }
    
    /// 获取所有测试结果
    pub fn get_all_results(&self) -> HashMap<String, BenchmarkResult> {
        match self.lock_results() {
            Ok(results) => results.clone(),
            Err(e) => {
                log::error!("Failed to get all results: {}", e);
                HashMap::new()
            }
        }
    }
    
    /// 获取全局统计
    pub fn get_global_stats(&self) -> GlobalBenchmarkStats {
        match self.lock_global_stats() {
            Ok(global_stats) => GlobalBenchmarkStats {
                total_tests: global_stats.total_tests,
                total_execution_time: global_stats.total_execution_time,
                total_compilation_time: global_stats.total_compilation_time,
                total_memory_usage: global_stats.total_memory_usage,
                covered_pcs: global_stats.covered_pcs.clone(),
            },
            Err(e) => {
                log::error!("Failed to get global stats: {}", e);
                GlobalBenchmarkStats::default()
            }
        }
    }
    
    /// 清除所有结果
    pub fn clear_results(&self) {
        match self.lock_results() {
            Ok(mut results) => {
                results.clear();
            }
            Err(e) => {
                log::error!("Failed to clear results: {}", e);
            }
        }

        match self.lock_global_stats() {
            Ok(mut global_stats) => {
                *global_stats = GlobalBenchmarkStats::default();
            }
            Err(e) => {
                log::error!("Failed to clear global stats: {}", e);
            }
        }
    }
    
    /// 运行预定义的基准测试套件
    pub fn run_benchmark_suite(&self) -> HashMap<String, BenchmarkResult> {
        let mut results = HashMap::new();
        
        // 算术测试
        let arithmetic_config = BenchmarkConfig {
            name: "arithmetic_simple".to_string(),
            description: "简单算术操作测试".to_string(),
            iterations: 1000,
            warmup_iterations: 100,
            ..Default::default()
        };
        
        let arithmetic_test = BenchmarkTestCaseGenerator::generate_arithmetic_test(0x1000, 10);
        let arithmetic_result = self.run_benchmark(&arithmetic_config, arithmetic_test);
        results.insert(arithmetic_config.name, arithmetic_result);
        
        // 内存访问测试
        let memory_config = BenchmarkConfig {
            name: "memory_access".to_string(),
            description: "内存访问操作测试".to_string(),
            iterations: 500,
            warmup_iterations: 50,
            ..Default::default()
        };
        
        let memory_test = BenchmarkTestCaseGenerator::generate_memory_test(0x2000, 20);
        let memory_result = self.run_benchmark(&memory_config, memory_test);
        results.insert(memory_config.name, memory_result);
        
        // 控制流测试
        let control_flow_config = BenchmarkConfig {
            name: "control_flow".to_string(),
            description: "控制流操作测试".to_string(),
            iterations: 800,
            warmup_iterations: 80,
            ..Default::default()
        };
        
        let control_flow_test = BenchmarkTestCaseGenerator::generate_control_flow_test(0x3000, 15);
        let control_flow_result = self.run_benchmark(&control_flow_config, control_flow_test);
        results.insert(control_flow_config.name, control_flow_result);
        
        // SIMD测试
        let simd_config = BenchmarkConfig {
            name: "simd_operations".to_string(),
            description: "SIMD操作测试".to_string(),
            iterations: 1200,
            warmup_iterations: 120,
            ..Default::default()
        };
        
        let simd_test = BenchmarkTestCaseGenerator::generate_simd_test(0x4000, 25);
        let simd_result = self.run_benchmark(&simd_config, simd_test);
        results.insert(simd_config.name, simd_result);
        
        // 混合测试
        let mixed_config = BenchmarkConfig {
            name: "mixed_operations".to_string(),
            description: "混合操作测试".to_string(),
            iterations: 600,
            warmup_iterations: 60,
            ..Default::default()
        };
        
        let mixed_test = BenchmarkTestCaseGenerator::generate_mixed_test(0x5000, 30);
        let mixed_result = self.run_benchmark(&mixed_config, mixed_test);
        results.insert(mixed_config.name, mixed_result);
        
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::JITConfig;

    #[test]
    fn test_benchmark_test_case_generation() {
        // 测试算术测试用例生成
        let arithmetic_test = BenchmarkTestCaseGenerator::generate_arithmetic_test(0x1000, 10);
        assert_eq!(arithmetic_test.start_pc, 0x1000);
        assert_eq!(arithmetic_test.ops.len(), 10);
        
        // 测试内存访问测试用例生成
        let memory_test = BenchmarkTestCaseGenerator::generate_memory_test(0x2000, 5);
        assert_eq!(memory_test.start_pc, 0x2000);
        assert_eq!(memory_test.ops.len(), 5);
        
        // 测试控制流测试用例生成
        let control_flow_test = BenchmarkTestCaseGenerator::generate_control_flow_test(0x3000, 8);
        assert_eq!(control_flow_test.start_pc, 0x3000);
        assert_eq!(control_flow_test.ops.len(), 16); // 每个分支包含比较和跳转
        
        // 测试SIMD测试用例生成
        let simd_test = BenchmarkTestCaseGenerator::generate_simd_test(0x4000, 5);
        assert_eq!(simd_test.start_pc, 0x4000);
        assert_eq!(simd_test.ops.len(), 20); // 每个向量包含4个操作
        
        // 测试混合测试用例生成
        let mixed_test = BenchmarkTestCaseGenerator::generate_mixed_test(0x5000, 10);
        assert_eq!(mixed_test.start_pc, 0x5000);
        assert_eq!(mixed_test.ops.len(), 10);
    }

    #[test]
    fn test_time_stats_calculation() {
        let times = vec![
            Duration::from_millis(10),
            Duration::from_millis(20),
            Duration::from_millis(30),
            Duration::from_millis(40),
            Duration::from_millis(50),
        ];
        
        let (avg, min, max, std_dev, median, p95, p99, ops_per_sec) = 
            AdvancedPerformanceBenchmarker::calculate_time_stats(&times);
        
        assert_eq!(avg, Duration::from_millis(30));
        assert_eq!(min, Duration::from_millis(10));
        assert_eq!(max, Duration::from_millis(50));
        assert_eq!(median, Duration::from_millis(30));
        assert_eq!(p95, Duration::from_millis(50));
        assert_eq!(p99, Duration::from_millis(50));
        assert!(ops_per_sec > 0.0);
    }
}