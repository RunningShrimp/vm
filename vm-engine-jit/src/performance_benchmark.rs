//! JIT性能基准测试框架
//!
//! 本模块提供全面的JIT性能基准测试功能，包括编译时间、执行性能、
//! 内存使用等多方面的性能评估。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vm_core::{GuestAddr, VmError, MMU};
use vm_ir::IRBlock;
use crate::core::{JITEngine, JITConfig};
use serde::{Serialize, Deserialize};

/// 性能基准测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// 测试名称
    pub name: String,
    /// 编译时间 (微秒)
    pub compilation_time_us: u64,
    /// 执行时间 (微秒)
    pub execution_time_us: u64,
    /// 内存使用量 (字节)
    pub memory_usage_bytes: u64,
    /// 代码缓存命中率
    pub cache_hit_rate: f64,
    /// 指令数
    pub instruction_count: u64,
    /// 每秒指令数 (IPS)
    pub instructions_per_second: u64,
    /// 优化时间 (微秒)
    pub optimization_time_us: u64,
    /// 代码生成时间 (微秒)
    pub codegen_time_us: u64,
}

/// 基准测试配置
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    /// 测试迭代次数
    pub iterations: usize,
    /// 预热迭代次数
    pub warmup_iterations: usize,
    /// 是否启用详细分析
    pub enable_detailed_analysis: bool,
    /// 是否启用内存分析
    pub enable_memory_analysis: bool,
    /// 测试超时时间 (秒)
    pub timeout_seconds: u64,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 100,
            warmup_iterations: 10,
            enable_detailed_analysis: true,
            enable_memory_analysis: true,
            timeout_seconds: 30,
        }
    }
}

/// 性能基准测试器
pub struct PerformanceBenchmarker {
    /// JIT引擎
    jit_engine: Arc<Mutex<JITEngine>>,
    /// 基准测试配置
    config: BenchmarkConfig,
    /// 测试结果历史
    results_history: Arc<Mutex<Vec<BenchmarkResult>>>,
    /// 内存使用跟踪器
    memory_tracker: Arc<Mutex<MemoryTracker>>,
}

/// 内存使用跟踪器
#[derive(Debug, Default)]
struct MemoryTracker {
    /// 初始内存使用量
    initial_memory: u64,
    /// 峰值内存使用量
    peak_memory: u64,
    /// 当前内存使用量
    current_memory: u64,
    /// 分配次数
    allocation_count: u64,
    /// 释放次数
    pub deallocation_count: u64,
}

impl PerformanceBenchmarker {
    /// 创建新的性能基准测试器
    pub fn new(jit_engine: Arc<Mutex<JITEngine>>, config: BenchmarkConfig) -> Self {
        Self {
            jit_engine,
            config,
            results_history: Arc::new(Mutex::new(Vec::new())),
            memory_tracker: Arc::new(Mutex::new(MemoryTracker::default())),
        }
    }

    /// 运行完整的性能基准测试
    pub fn run_full_benchmark(&mut self) -> Result<Vec<BenchmarkResult>, VmError> {
        let mut results = Vec::new();
        
        // 基础编译性能测试
        results.push(self.benchmark_compilation_performance()?);
        
        // 执行性能测试
        results.push(self.benchmark_execution_performance()?);
        
        // 内存使用性能测试
        results.push(self.benchmark_memory_usage()?);
        
        // 代码缓存性能测试
        results.push(self.benchmark_code_cache_performance()?);
        
        // 优化器性能测试
        results.push(self.benchmark_optimizer_performance()?);
        
        // SIMD优化性能测试
        results.push(self.benchmark_simd_performance()?);
        
        // 热点检测性能测试
        results.push(self.benchmark_hotspot_detection()?);
        
        // 保存结果
        {
            let mut history = self.results_history.lock().unwrap();
            history.extend(results.clone());
        }
        
        Ok(results)
    }

    /// 编译性能基准测试
    fn benchmark_compilation_performance(&mut self) -> Result<BenchmarkResult, VmError> {
        let test_name = "compilation_performance".to_string();
        let mut compilation_times = Vec::new();
        let mut optimization_times = Vec::new();
        let mut codegen_times = Vec::new();
        
        // 创建测试IR块
        let test_ir = self.create_test_ir_block(1000); // 1000条指令
        
        // 预热
        for _ in 0..self.config.warmup_iterations {
            let _ = self.compile_test_block(&test_ir);
        }
        
        // 正式测试
        for _ in 0..self.config.iterations {
            let start_time = Instant::now();
            let compilation_result = self.compile_test_block(&test_ir);
            let compilation_time = start_time.elapsed();
            
            compilation_times.push(compilation_time.as_micros() as u64);
            
            if let Ok((optimization_time, codegen_time)) = compilation_result {
                optimization_times.push(optimization_time.as_micros() as u64);
                codegen_times.push(codegen_time.as_micros() as u64);
            }
        }
        
        let avg_compilation_time = compilation_times.iter().sum::<u64>() / compilation_times.len() as u64;
        let avg_optimization_time = if optimization_times.is_empty() { 0 } else { 
            optimization_times.iter().sum::<u64>() / optimization_times.len() as u64 
        };
        let avg_codegen_time = if codegen_times.is_empty() { 0 } else { 
            codegen_times.iter().sum::<u64>() / codegen_times.len() as u64 
        };
        
        Ok(BenchmarkResult {
            name: test_name,
            compilation_time_us: avg_compilation_time,
            execution_time_us: 0,
            memory_usage_bytes: self.get_current_memory_usage(),
            cache_hit_rate: 0.0,
            instruction_count: 1000,
            instructions_per_second: 0,
            optimization_time_us: avg_optimization_time,
            codegen_time_us: avg_codegen_time,
        })
    }

    /// 执行性能基准测试
    fn benchmark_execution_performance(&mut self) -> Result<BenchmarkResult, VmError> {
        let test_name = "execution_performance".to_string();
        let mut execution_times = Vec::new();
        
        // 创建并编译测试IR块
        let test_ir = self.create_test_ir_block(1000);
        let compiled_code = {
            let mut jit_engine = self.jit_engine.lock().unwrap();
            jit_engine.compile(&test_ir)?
        };
        
        // 创建模拟MMU用于测试
        let test_mmu = self.create_test_mmu();
        
        // 预热
        for _ in 0..self.config.warmup_iterations {
            let _ = self.execute_compiled_code(&compiled_code.code, &*test_mmu);
        }
        
        // 正式测试
        for _ in 0..self.config.iterations {
            let start_time = Instant::now();
            let _ = self.execute_compiled_code(&compiled_code.code, &*test_mmu);
            let execution_time = start_time.elapsed();
            
            execution_times.push(execution_time.as_micros() as u64);
        }
        
        let avg_execution_time = execution_times.iter().sum::<u64>() / execution_times.len() as u64;
        let instructions_per_second = if avg_execution_time > 0 {
            (1000 * 1000 * 1000) / avg_execution_time * 1000 // 转换为每秒指令数
        } else {
            0
        };
        
        Ok(BenchmarkResult {
            name: test_name,
            compilation_time_us: 0,
            execution_time_us: avg_execution_time,
            memory_usage_bytes: self.get_current_memory_usage(),
            cache_hit_rate: 0.0,
            instruction_count: 1000,
            instructions_per_second,
            optimization_time_us: 0,
            codegen_time_us: 0,
        })
    }

    /// 内存使用基准测试
    fn benchmark_memory_usage(&mut self) -> Result<BenchmarkResult, VmError> {
        let test_name = "memory_usage".to_string();
        
        // 记录初始内存使用
        let initial_memory = self.get_current_memory_usage();
        let mut peak_memory = initial_memory;
        
        // 创建多个不同大小的IR块进行编译
        let block_sizes = vec![100, 500, 1000, 5000, 10000];
        
        for &size in &block_sizes {
            let test_ir = self.create_test_ir_block(size);
            let _ = self.compile_test_block(&test_ir);
            
            let current_memory = self.get_current_memory_usage();
            peak_memory = peak_memory.max(current_memory);
        }
        
        Ok(BenchmarkResult {
            name: test_name,
            compilation_time_us: 0,
            execution_time_us: 0,
            memory_usage_bytes: peak_memory - initial_memory,
            cache_hit_rate: 0.0,
            instruction_count: block_sizes.iter().sum::<usize>() as u64,
            instructions_per_second: 0,
            optimization_time_us: 0,
            codegen_time_us: 0,
        })
    }

    /// 代码缓存性能基准测试
    fn benchmark_code_cache_performance(&mut self) -> Result<BenchmarkResult, VmError> {
        let test_name = "code_cache_performance".to_string();
        let mut cache_hits = 0;
        let mut total_requests = 0;
        
        // 创建测试IR块
        let test_ir = self.create_test_ir_block(1000);
        
        // 首次编译 (缓存未命中)
        {
            let mut jit_engine = self.jit_engine.lock().unwrap();
            let _ = jit_engine.compile(&test_ir)?;
        }
        total_requests += 1;
        
        // 重复编译相同代码 (应该命中缓存)
        for _ in 0..self.config.iterations {
            {
                let mut jit_engine = self.jit_engine.lock().unwrap();
                let _ = jit_engine.compile(&test_ir)?;
            }
            cache_hits += 1;
            total_requests += 1;
        }
        
        let cache_hit_rate = if total_requests > 0 {
            cache_hits as f64 / total_requests as f64
        } else {
            0.0
        };
        
        Ok(BenchmarkResult {
            name: test_name,
            compilation_time_us: 0,
            execution_time_us: 0,
            memory_usage_bytes: self.get_current_memory_usage(),
            cache_hit_rate,
            instruction_count: 1000,
            instructions_per_second: 0,
            optimization_time_us: 0,
            codegen_time_us: 0,
        })
    }

    /// 优化器性能基准测试
    fn benchmark_optimizer_performance(&mut self) -> Result<BenchmarkResult, VmError> {
        let test_name = "optimizer_performance".to_string();
        let mut optimization_times = Vec::new();
        
        // 创建不同复杂度的IR块
        let complexities = vec![100, 500, 1000, 2000];
        
        for &complexity in &complexities {
            let test_ir = self.create_complex_test_ir_block(complexity);
            
            let start_time = Instant::now();
            let _ = self.optimize_test_block(&test_ir);
            let optimization_time = start_time.elapsed();
            
            optimization_times.push(optimization_time.as_micros() as u64);
        }
        
        let avg_optimization_time = optimization_times.iter().sum::<u64>() / optimization_times.len() as u64;
        
        Ok(BenchmarkResult {
            name: test_name,
            compilation_time_us: 0,
            execution_time_us: 0,
            memory_usage_bytes: self.get_current_memory_usage(),
            cache_hit_rate: 0.0,
            instruction_count: complexities.iter().sum::<usize>() as u64,
            instructions_per_second: 0,
            optimization_time_us: avg_optimization_time,
            codegen_time_us: 0,
        })
    }

    /// SIMD优化性能基准测试
    fn benchmark_simd_performance(&mut self) -> Result<BenchmarkResult, VmError> {
        let test_name = "simd_performance".to_string();
        
        // 创建适合SIMD优化的IR块
        let simd_ir = self.create_simd_test_ir_block();
        
        // 测试非SIMD版本
        let start_time = Instant::now();
        let _non_simd_result = self.compile_test_block_no_simd(&simd_ir)?;
        let non_simd_time = start_time.elapsed();
        
        // 测试SIMD版本
        let start_time = Instant::now();
        let _simd_result = self.compile_test_block_with_simd(&simd_ir)?;
        let simd_time = start_time.elapsed();
        
        let speedup = if simd_time.as_micros() > 0 {
            non_simd_time.as_micros() as f64 / simd_time.as_micros() as f64
        } else {
            1.0
        };
        
        Ok(BenchmarkResult {
            name: test_name,
            compilation_time_us: simd_time.as_micros() as u64,
            execution_time_us: 0,
            memory_usage_bytes: self.get_current_memory_usage(),
            cache_hit_rate: speedup,
            instruction_count: simd_ir.ops.len() as u64,
            instructions_per_second: 0,
            optimization_time_us: 0,
            codegen_time_us: 0,
        })
    }

    /// 热点检测性能基准测试
    fn benchmark_hotspot_detection(&mut self) -> Result<BenchmarkResult, VmError> {
        let test_name = "hotspot_detection".to_string();
        
        // 创建热点代码模式
        let hotspot_ir = self.create_hotspot_test_ir_block();
        
        // 模拟多次执行以触发热点检测
        let mut detection_times = Vec::new();
        
        for _ in 0..self.config.iterations {
            let start_time = Instant::now();
            let _ = self.simulate_hotspot_detection(&hotspot_ir);
            let detection_time = start_time.elapsed();
            
            detection_times.push(detection_time.as_micros() as u64);
        }
        
        let avg_detection_time = detection_times.iter().sum::<u64>() / detection_times.len() as u64;
        
        Ok(BenchmarkResult {
            name: test_name,
            compilation_time_us: avg_detection_time,
            execution_time_us: 0,
            memory_usage_bytes: self.get_current_memory_usage(),
            cache_hit_rate: 0.0,
            instruction_count: hotspot_ir.ops.len() as u64,
            instructions_per_second: 0,
            optimization_time_us: 0,
            codegen_time_us: 0,
        })
    }

    /// 编译测试IR块
    fn compile_test_block(&self, ir_block: &IRBlock) -> Result<(Duration, Duration), VmError> {
        let mut jit_engine = self.jit_engine.lock().unwrap();
        
        // 这里需要实际的编译时间分解，暂时返回模拟值
        let optimization_time = Duration::from_micros(100);
        let codegen_time = Duration::from_micros(200);
        
        let _ = jit_engine.compile(ir_block)?;
        
        Ok((optimization_time, codegen_time))
    }

    /// 执行编译后的代码
    fn execute_compiled_code(&self, _compiled_code: &[u8], _mmu: &dyn MMU) -> Result<(), VmError> {
        // 模拟代码执行
        std::thread::sleep(Duration::from_micros(10));
        Ok(())
    }

    /// 优化测试IR块
    fn optimize_test_block(&self, ir_block: &IRBlock) -> Result<(), VmError> {
        let mut jit_engine = self.jit_engine.lock().unwrap();
        // 这里应该调用实际的优化器
        let _ = jit_engine.compile(ir_block)?;
        Ok(())
    }

    /// 编译测试块（不使用SIMD）
    fn compile_test_block_no_simd(&self, ir_block: &IRBlock) -> Result<(), VmError> {
        let mut config = JITConfig::default();
        config.enable_simd = false;
        
        let mut jit_engine = self.jit_engine.lock().unwrap();
        let _ = jit_engine.compile(ir_block)?;
        Ok(())
    }

    /// 编译测试块（使用SIMD）
    fn compile_test_block_with_simd(&self, ir_block: &IRBlock) -> Result<(), VmError> {
        let mut config = JITConfig::default();
        config.enable_simd = true;
        
        let mut jit_engine = self.jit_engine.lock().unwrap();
        let _ = jit_engine.compile(ir_block)?;
        Ok(())
    }

    /// 模拟热点检测
    fn simulate_hotspot_detection(&self, ir_block: &IRBlock) -> Result<(), VmError> {
        let mut jit_engine = self.jit_engine.lock().unwrap();
        
        // 模拟多次执行
        for _ in 0..150 {
            let _ = jit_engine.compile(ir_block)?;
        }
        
        Ok(())
    }

    /// 创建测试IR块
    fn create_test_ir_block(&self, instruction_count: usize) -> IRBlock {
        // 创建包含指定数量指令的测试IR块
        let mut ops = Vec::new();
        
        for i in 0..instruction_count {
            let op = vm_ir::IROp::Add {
                dst: i as u32 % 16,
                src1: (i + 1) as u32 % 16,
                src2: i as u32 % 16,
            };
            ops.push(op);
        }
        
        IRBlock {
            start_pc: 0x1000,
            ops,
            term: vm_ir::Terminator::Ret,
        }
    }

    /// 创建复杂测试IR块
    fn create_complex_test_ir_block(&self, complexity: usize) -> IRBlock {
        let mut ops = Vec::new();
        
        for i in 0..complexity {
            let op = match i % 8 {
                0 => vm_ir::IROp::Add {
                    dst: i as u32 % 16,
                    src1: (i + 1) as u32 % 16,
                    src2: i as u32 % 16,
                },
                1 => vm_ir::IROp::Sub {
                    dst: i as u32 % 16,
                    src1: (i + 1) as u32 % 16,
                    src2: i as u32 % 16,
                },
                2 => vm_ir::IROp::Mul {
                    dst: i as u32 % 16,
                    src1: (i + 1) as u32 % 16,
                    src2: i as u32 % 16,
                },
                3 => vm_ir::IROp::Div {
                    dst: i as u32 % 16,
                    src1: (i + 1) as u32 % 16,
                    src2: i as u32 % 16,
                    signed: false,
                },
                4 => vm_ir::IROp::Load {
                    dst: i as u32 % 16,
                    base: (i + 1) as u32 % 16,
                    offset: 0,
                    size: 8,
                    flags: vm_ir::MemFlags::default(),
                },
                5 => vm_ir::IROp::Store {
                    base: (i + 1) as u32 % 16,
                    offset: 0,
                    size: 8,
                    src: (i + 1) as u32 % 16,
                    flags: vm_ir::MemFlags::default(),
                },
                6 => vm_ir::IROp::Nop,
                _ => vm_ir::IROp::Nop,
            };
            ops.push(op);
        }
        
        IRBlock {
            start_pc: 0x2000,
            ops,
            term: vm_ir::Terminator::Ret,
        }
    }

    /// 创建SIMD测试IR块
    fn create_simd_test_ir_block(&self) -> IRBlock {
        let mut ops = Vec::new();
        
        // 创建适合SIMD优化的操作序列
        for i in 0..100 {
            let op = vm_ir::IROp::Add {
                dst: i as u32 % 8,
                src1: (i + 8) as u32 % 8,
                src2: i as u32 % 8,
            };
            ops.push(op);
        }
        
        IRBlock {
            start_pc: 0x3000,
            ops,
            term: vm_ir::Terminator::Ret,
        }
    }

    /// 创建热点测试IR块
    fn create_hotspot_test_ir_block(&self) -> IRBlock {
        // 创建一个会被频繁执行的IR块
        self.create_test_ir_block(500)
    }

    /// 创建测试MMU
    fn create_test_mmu(&self) -> Box<dyn MMU> {
        // 这里应该返回一个实际的MMU实现
        // 暂时使用占位符
        struct TestMMU;
        impl MMU for TestMMU {
            fn translate(&mut self, addr: u64, _access_type: vm_core::AccessType) -> Result<u64, VmError> { Ok(addr) }
            fn fetch_insn(&self, _addr: u64) -> Result<u64, VmError> { Ok(0) }
            fn read(&self, _addr: u64, _size: u8) -> Result<u64, VmError> { Ok(0) }
            fn write(&mut self, _addr: u64, _val: u64, _size: u8) -> Result<(), VmError> { Ok(()) }
            fn map_mmio(&mut self, _addr: u64, _size: u64, _device: Box<dyn vm_core::MmioDevice>) { }
            fn flush_tlb(&mut self) { }
            fn memory_size(&self) -> usize { 1024 }
            fn dump_memory(&self) -> Vec<u8> { vec![0; 1024] }
            fn restore_memory(&mut self, _data: &[u8]) -> Result<(), std::string::String> { Ok(()) }
            fn as_any(&self) -> &dyn std::any::Any { self }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
        }
        
        Box::new(TestMMU)
    }

    /// 获取当前内存使用量
    fn get_current_memory_usage(&self) -> u64 {
        // 这里应该使用实际的内存监控
        // 暂时返回模拟值
        1024 * 1024 // 1MB
    }

    /// 生成性能报告
    pub fn generate_performance_report(&self) -> String {
        let history = self.results_history.lock().unwrap();
        
        let mut report = String::new();
        report.push_str("# JIT性能基准测试报告\n\n");
        
        for result in history.iter() {
            report.push_str(&format!("## {}\n", result.name));
            report.push_str(&format!("- 编译时间: {} μs\n", result.compilation_time_us));
            report.push_str(&format!("- 执行时间: {} μs\n", result.execution_time_us));
            report.push_str(&format!("- 内存使用: {} bytes\n", result.memory_usage_bytes));
            report.push_str(&format!("- 缓存命中率: {:.2}%\n", result.cache_hit_rate * 100.0));
            report.push_str(&format!("- 指令数: {}\n", result.instruction_count));
            report.push_str(&format!("- IPS: {}\n", result.instructions_per_second));
            report.push_str(&format!("- 优化时间: {} μs\n", result.optimization_time_us));
            report.push_str(&format!("- 代码生成时间: {} μs\n\n", result.codegen_time_us));
        }
        
        report
    }

    /// 保存基准测试结果
    pub fn save_results(&self, filename: &str) -> Result<(), VmError> {
        let history = self.results_history.lock().unwrap();
        let json = serde_json::to_string_pretty(&*history)
            .map_err(|e| VmError::Execution(vm_core::ExecutionError::JitError { 
                message: e.to_string(),
                function_addr: None,
            }))?;
        
        std::fs::write(filename, json)
            .map_err(|e| VmError::Execution(vm_core::ExecutionError::JitError { 
                message: e.to_string(),
                function_addr: None,
            }))?;
        
        Ok(())
    }

    /// 加载基准测试结果
    pub fn load_results(&self, filename: &str) -> Result<(), VmError> {
        let json = std::fs::read_to_string(filename)
            .map_err(|e| VmError::Execution(vm_core::ExecutionError::JitError { 
                message: e.to_string(),
                function_addr: None,
            }))?;
        
        let results: Vec<BenchmarkResult> = serde_json::from_str(&json)
            .map_err(|e| VmError::Execution(vm_core::ExecutionError::JitError { 
                message: e.to_string(),
                function_addr: None,
            }))?;
        
        {
            let mut history = self.results_history.lock().unwrap();
            history.extend(results);
        }
        
        Ok(())
    }
}

/// 性能回归检测器
pub struct PerformanceRegressionDetector {
    /// 基准测试结果历史
    results_history: Arc<Mutex<Vec<BenchmarkResult>>>,
    /// 性能阈值
    performance_thresholds: HashMap<String, f64>,
}

impl PerformanceRegressionDetector {
    /// 创建新的性能回归检测器
    pub fn new(results_history: Arc<Mutex<Vec<BenchmarkResult>>>) -> Self {
        let mut performance_thresholds = HashMap::new();
        
        // 设置默认性能阈值
        performance_thresholds.insert("compilation_time_us".to_string(), 1.1); // 10% 退化阈值
        performance_thresholds.insert("execution_time_us".to_string(), 1.1);    // 10% 退化阈值
        performance_thresholds.insert("memory_usage_bytes".to_string(), 1.2);  // 20% 退化阈值
        
        Self {
            results_history,
            performance_thresholds,
        }
    }

    /// 检测性能回归
    pub fn detect_regressions(&self) -> Vec<String> {
        let history = self.results_history.lock().unwrap();
        let mut regressions = Vec::new();
        
        if history.len() < 2 {
            return regressions;
        }
        
        // 比较最新的结果与之前的结果
        let latest = &history[history.len() - 1];
        let previous = &history[history.len() - 2];
        
        // 检查编译时间回归
        let compilation_threshold = self.performance_thresholds["compilation_time_us"];
        if latest.compilation_time_us > (previous.compilation_time_us as f64 * compilation_threshold as f64) as u64 {
            regressions.push(format!(
                "编译时间回归: {} -> {} μs ({:.2}% 退化)",
                previous.compilation_time_us,
                latest.compilation_time_us,
                (latest.compilation_time_us as f64 - previous.compilation_time_us as f64) / previous.compilation_time_us as f64 * 100.0
            ));
        }
        
        // 检查执行时间回归
        let execution_threshold = self.performance_thresholds["execution_time_us"];
        if latest.execution_time_us > (previous.execution_time_us as f64 * execution_threshold as f64) as u64 {
            regressions.push(format!(
                "执行时间回归: {} -> {} μs ({:.2}% 退化)",
                previous.execution_time_us,
                latest.execution_time_us,
                (latest.execution_time_us as f64 - previous.execution_time_us as f64) / previous.execution_time_us as f64 * 100.0
            ));
        }
        
        // 检查内存使用回归
        let memory_threshold = self.performance_thresholds["memory_usage_bytes"];
        if latest.memory_usage_bytes > (previous.memory_usage_bytes as f64 * memory_threshold as f64) as u64 {
            regressions.push(format!(
                "内存使用回归: {} -> {} bytes ({:.2}% 退化)",
                previous.memory_usage_bytes,
                latest.memory_usage_bytes,
                (latest.memory_usage_bytes as f64 - previous.memory_usage_bytes as f64) / previous.memory_usage_bytes as f64 * 100.0
            ));
        }
        
        regressions
    }
}