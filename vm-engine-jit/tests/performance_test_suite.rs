//! JIT引擎性能测试套件
//!
//! 本模块提供全面的JIT引擎性能测试，包括单元测试、集成测试和压力测试。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vm_core::{GuestAddr, VmError, MMU, AccessType};
use vm_engine_jit::core::{JITEngine, JITConfig};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator, MemFlags};

/// 测试配置
#[derive(Debug, Clone)]
struct TestConfig {
    /// 测试迭代次数
    iterations: usize,
    /// 预热迭代次数
    warmup_iterations: usize,
    /// 超时时间（秒）
    timeout_seconds: u64,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            iterations: 100,
            warmup_iterations: 10,
            timeout_seconds: 30,
        }
    }
}

/// 性能测试结果
#[derive(Debug, Clone)]
struct TestResult {
    /// 测试名称
    name: String,
    /// 平均时间（微秒）
    avg_time_us: u64,
    /// 最小时间（微秒）
    min_time_us: u64,
    /// 最大时间（微秒）
    max_time_us: u64,
    /// 标准差
    stddev_us: f64,
    /// 是否通过
    passed: bool,
    /// 错误信息
    error_message: Option<String>,
}

/// JIT引擎性能测试套件
struct JITPerformanceTestSuite {
    /// JIT引擎
    jit_engine: Arc<Mutex<JITEngine>>,
    /// 测试配置
    config: TestConfig,
    /// 测试结果
    results: Vec<TestResult>,
}

impl JITPerformanceTestSuite {
    /// 创建新的测试套件
    fn new(config: TestConfig) -> Self {
        let jit_engine = Arc::new(Mutex::new(JITEngine::new(JITConfig::default())));
        Self {
            jit_engine,
            config,
            results: Vec::new(),
        }
    }

    /// 运行所有测试
    fn run_all_tests(&mut self) -> Result<(), VmError> {
        println!("开始JIT引擎性能测试...");
        
        // 基础编译性能测试
        self.test_basic_compilation_performance()?;
        
        // 复杂编译性能测试
        self.test_complex_compilation_performance()?;
        
        // SIMD优化性能测试
        self.test_simd_optimization_performance()?;
        
        // 内存使用性能测试
        self.test_memory_usage_performance()?;
        
        // 代码缓存性能测试
        self.test_code_cache_performance()?;
        
        // 并发编译性能测试
        self.test_concurrent_compilation_performance()?;
        
        // 热点检测性能测试
        self.test_hotspot_detection_performance()?;
        
        // 自适应阈值性能测试
        self.test_adaptive_threshold_performance()?;
        
        // 生成测试报告
        self.generate_test_report();
        
        Ok(())
    }

    /// 基础编译性能测试
    fn test_basic_compilation_performance(&mut self) -> Result<(), VmError> {
        let test_name = "基础编译性能测试";
        println!("运行: {}", test_name);
        
        let mut times = Vec::new();
        
        // 创建测试IR块
        let test_ir = self.create_basic_test_ir_block(0x1000, 1000);
        
        // 预热
        for _ in 0..self.config.warmup_iterations {
            let mut jit = self.jit_engine.lock().unwrap();
            let _ = jit.compile(&test_ir);
        }
        
        // 正式测试
        for _ in 0..self.config.iterations {
            let start_time = Instant::now();
            let mut jit = self.jit_engine.lock().unwrap();
            let _ = jit.compile(&test_ir);
            let elapsed = start_time.elapsed();
            times.push(elapsed.as_micros() as u64);
        }
        
        let result = self.calculate_test_result(test_name, times, Some(1000.0)); // 期望1ms以内
        self.results.push(result);
        
        Ok(())
    }

    /// 复杂编译性能测试
    fn test_complex_compilation_performance(&mut self) -> Result<(), VmError> {
        let test_name = "复杂编译性能测试";
        println!("运行: {}", test_name);
        
        let mut times = Vec::new();
        
        // 创建复杂测试IR块
        let test_ir = self.create_complex_test_ir_block(0x2000, 2000);
        
        // 预热
        for _ in 0..self.config.warmup_iterations {
            let mut jit = self.jit_engine.lock().unwrap();
            let _ = jit.compile(&test_ir);
        }
        
        // 正式测试
        for _ in 0..self.config.iterations {
            let start_time = Instant::now();
            let mut jit = self.jit_engine.lock().unwrap();
            let _ = jit.compile(&test_ir);
            let elapsed = start_time.elapsed();
            times.push(elapsed.as_micros() as u64);
        }
        
        let result = self.calculate_test_result(test_name, times, Some(5000.0)); // 期望5ms以内
        self.results.push(result);
        
        Ok(())
    }

    /// SIMD优化性能测试
    fn test_simd_optimization_performance(&mut self) -> Result<(), VmError> {
        let test_name = "SIMD优化性能测试";
        println!("运行: {}", test_name);
        
        // 测试非SIMD版本
        let mut no_simd_times = Vec::new();
        let no_simd_ir = self.create_simd_test_ir_block(0x3000, 1000);
        
        {
            let mut config = JITConfig::default();
            config.enable_simd = false;
            let mut jit = JITEngine::new(config);
            
            // 预热
            for _ in 0..self.config.warmup_iterations {
                let _ = jit.compile(&no_simd_ir);
            }
            
            // 正式测试
            for _ in 0..self.config.iterations {
                let start_time = Instant::now();
                let _ = jit.compile(&no_simd_ir);
                let elapsed = start_time.elapsed();
                no_simd_times.push(elapsed.as_micros() as u64);
            }
        }
        
        // 测试SIMD版本
        let mut simd_times = Vec::new();
        let simd_ir = self.create_simd_test_ir_block(0x3000, 1000);
        
        {
            let mut config = JITConfig::default();
            config.enable_simd = true;
            let mut jit = JITEngine::new(config);
            
            // 预热
            for _ in 0..self.config.warmup_iterations {
                let _ = jit.compile(&simd_ir);
            }
            
            // 正式测试
            for _ in 0..self.config.iterations {
                let start_time = Instant::now();
                let _ = jit.compile(&simd_ir);
                let elapsed = start_time.elapsed();
                simd_times.push(elapsed.as_micros() as u64);
            }
        }
        
        // 计算性能提升
        let avg_no_simd = no_simd_times.iter().sum::<u64>() as f64 / no_simd_times.len() as f64;
        let avg_simd = simd_times.iter().sum::<u64>() as f64 / simd_times.len() as f64;
        let speedup = avg_no_simd / avg_simd;
        
        let mut result = self.calculate_test_result(test_name, simd_times, Some(2000.0)); // 期望2ms以内
        result.error_message = Some(format!("SIMD性能提升: {:.2}x", speedup));
        self.results.push(result);
        
        Ok(())
    }

    /// 内存使用性能测试
    fn test_memory_usage_performance(&mut self) -> Result<(), VmError> {
        let test_name = "内存使用性能测试";
        println!("运行: {}", test_name);
        
        let mut times = Vec::new();
        
        // 创建多个不同大小的IR块
        let block_sizes = vec![100, 500, 1000, 2000, 5000];
        
        for &size in &block_sizes {
            let test_ir = self.create_basic_test_ir_block(0x4000 + size as u64, size);
            
            let start_time = Instant::now();
            let mut jit = self.jit_engine.lock().unwrap();
            let _ = jit.compile(&test_ir);
            let elapsed = start_time.elapsed();
            times.push(elapsed.as_micros() as u64);
        }
        
        let result = self.calculate_test_result(test_name, times, Some(10000.0)); // 期望10ms以内
        self.results.push(result);
        
        Ok(())
    }

    /// 代码缓存性能测试
    fn test_code_cache_performance(&mut self) -> Result<(), VmError> {
        let test_name = "代码缓存性能测试";
        println!("运行: {}", test_name);
        
        let mut times = Vec::new();
        let test_ir = self.create_basic_test_ir_block(0x5000, 1000);
        
        // 首次编译（缓存未命中）
        {
            let start_time = Instant::now();
            let mut jit = self.jit_engine.lock().unwrap();
            let _ = jit.compile(&test_ir);
            let elapsed = start_time.elapsed();
            times.push(elapsed.as_micros() as u64);
        }
        
        // 重复编译（缓存命中）
        for _ in 0..self.config.iterations {
            let start_time = Instant::now();
            let mut jit = self.jit_engine.lock().unwrap();
            let _ = jit.compile(&test_ir);
            let elapsed = start_time.elapsed();
            times.push(elapsed.as_micros() as u64);
        }
        
        let result = self.calculate_test_result(test_name, times, Some(100.0)); // 期望缓存命中时0.1ms以内
        self.results.push(result);
        
        Ok(())
    }

    /// 并发编译性能测试
    fn test_concurrent_compilation_performance(&mut self) -> Result<(), VmError> {
        let test_name = "并发编译性能测试";
        println!("运行: {}", test_name);
        
        let mut times = Vec::new();
        
        for thread_count in [1, 2, 4, 8].iter() {
            let start_time = Instant::now();
            
            // 预先创建所有测试IR块
            let mut test_blocks = Vec::new();
            for i in 0..*thread_count {
                let block = self.create_basic_test_ir_block(0x6000 + i as u64 * 0x1000, 1000);
                test_blocks.push(block);
            }
            
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let jit = Arc::new(Mutex::new(JITEngine::new(JITConfig::default())));
                let mut handles = Vec::new();
                
                for (i, test_ir) in test_blocks.into_iter().enumerate() {
                    let jit_clone = jit.clone();
                    let handle = tokio::spawn(async move {
                        let mut jit = jit_clone.lock().unwrap();
                        jit.compile(&test_ir).unwrap()
                    });
                    handles.push(handle);
                }
                
                for handle in handles {
                    handle.await.unwrap();
                }
            });
            
            let elapsed = start_time.elapsed();
            times.push(elapsed.as_micros() as u64);
        }
        
        let result = self.calculate_test_result(test_name, times, Some(5000.0)); // 期望5ms以内
        self.results.push(result);
        
        Ok(())
    }

    /// 热点检测性能测试
    fn test_hotspot_detection_performance(&mut self) -> Result<(), VmError> {
        let test_name = "热点检测性能测试";
        println!("运行: {}", test_name);
        
        let mut times = Vec::new();
        let test_ir = self.create_basic_test_ir_block(0x7000, 1000);
        
        // 测试热点检测开销
        for _ in 0..self.config.iterations {
            let start_time = Instant::now();
            
            // 模拟多次执行以触发热点检测
            for _ in 0..150 {
                let mut jit = self.jit_engine.lock().unwrap();
                let _ = jit.compile(&test_ir);
            }
            
            let elapsed = start_time.elapsed();
            times.push(elapsed.as_micros() as u64);
        }
        
        let result = self.calculate_test_result(test_name, times, Some(50000.0)); // 期望50ms以内
        self.results.push(result);
        
        Ok(())
    }

    /// 自适应阈值性能测试
    fn test_adaptive_threshold_performance(&mut self) -> Result<(), VmError> {
        let test_name = "自适应阈值性能测试";
        println!("运行: {}", test_name);
        
        let mut times = Vec::new();
        
        // 测试固定阈值
        {
            let mut config = JITConfig::default();
            config.enable_adaptive_compilation = false;
            let mut jit = JITEngine::new(config);
            let test_ir = self.create_basic_test_ir_block(0x8000, 1000);
            
            let start_time = Instant::now();
            let _ = jit.compile(&test_ir);
            let elapsed = start_time.elapsed();
            times.push(elapsed.as_micros() as u64);
        }
        
        // 测试自适应阈值
        {
            let mut config = JITConfig::default();
            config.enable_adaptive_compilation = true;
            let mut jit = JITEngine::new(config);
            let test_ir = self.create_basic_test_ir_block(0x8000, 1000);
            
            let start_time = Instant::now();
            let _ = jit.compile(&test_ir);
            let elapsed = start_time.elapsed();
            times.push(elapsed.as_micros() as u64);
        }
        
        let result = self.calculate_test_result(test_name, times, Some(2000.0)); // 期望2ms以内
        self.results.push(result);
        
        Ok(())
    }

    /// 计算测试结果
    fn calculate_test_result(&self, name: &str, times: Vec<u64>, expected_threshold: Option<f64>) -> TestResult {
        if times.is_empty() {
            return TestResult {
                name: name.to_string(),
                avg_time_us: 0,
                min_time_us: 0,
                max_time_us: 0,
                stddev_us: 0.0,
                passed: false,
                error_message: Some("没有测试数据".to_string()),
            };
        }

        let avg_time = times.iter().sum::<u64>() as f64 / times.len() as f64;
        let min_time = *times.iter().min().unwrap();
        let max_time = *times.iter().max().unwrap();
        
        // 计算标准差
        let variance = times.iter()
            .map(|&time| (time as f64 - avg_time).powi(2))
            .sum::<f64>() / times.len() as f64;
        let stddev = variance.sqrt();

        let passed = if let Some(threshold) = expected_threshold {
            avg_time <= threshold
        } else {
            true
        };

        TestResult {
            name: name.to_string(),
            avg_time_us: avg_time as u64,
            min_time_us: min_time,
            max_time_us: max_time,
            stddev_us: stddev,
            passed,
            error_message: if passed { None } else { Some("性能不达标".to_string()) },
        }
    }

    /// 生成测试报告
    fn generate_test_report(&self) {
        println!("\n=== JIT引擎性能测试报告 ===");
        
        let mut passed_count = 0;
        let mut total_count = 0;
        
        for result in &self.results {
            total_count += 1;
            if result.passed {
                passed_count += 1;
            }
            
            println!("\n测试: {}", result.name);
            println!("  平均时间: {} μs", result.avg_time_us);
            println!("  最小时间: {} μs", result.min_time_us);
            println!("  最大时间: {} μs", result.max_time_us);
            println!("  标准差: {:.2} μs", result.stddev_us);
            println!("  结果: {}", if result.passed { "通过" } else { "失败" });
            
            if let Some(ref error) = result.error_message {
                println!("  备注: {}", error);
            }
        }
        
        println!("\n=== 测试总结 ===");
        println!("通过: {}/{} ({:.1}%)", passed_count, total_count, passed_count as f64 / total_count as f64 * 100.0);
        
        if passed_count == total_count {
            println!("✅ 所有性能测试通过！");
        } else {
            println!("❌ 部分性能测试失败，需要优化");
        }
    }

    /// 创建基础测试IR块
    fn create_basic_test_ir_block(&self, addr: GuestAddr, instruction_count: usize) -> IRBlock {
        let mut builder = IRBuilder::new(addr);
        
        for i in 0..instruction_count {
            builder.push(IROp::MovImm { dst: (i % 16) as u32, imm: (i * 42) as u64 });
            builder.push(IROp::Add {
                dst: 0,
                src1: 0,
                src2: (i % 16) as u32,
            });
        }
        
        builder.set_term(Terminator::Jmp { target: addr + (instruction_count * 16) as u64 });
        builder.build()
    }

    /// 创建复杂测试IR块
    fn create_complex_test_ir_block(&self, addr: GuestAddr, complexity: usize) -> IRBlock {
        let mut builder = IRBuilder::new(addr);
        
        for i in 0..complexity {
            match i % 8 {
                0 => {
                    builder.push(IROp::MovImm { dst: 1, imm: i as u64 });
                    builder.push(IROp::Add {
                        dst: 0,
                        src1: 0,
                        src2: 1,
                    });
                }
                1 => {
                    builder.push(IROp::Sub {
                        dst: 2,
                        src1: 0,
                        src2: 1,
                    });
                }
                2 => {
                    builder.push(IROp::Mul {
                        dst: 3,
                        src1: 2,
                        src2: 1,
                    });
                }
                3 => {
                    builder.push(IROp::Div {
                        dst: 4,
                        src1: 3,
                        src2: 1,
                        signed: false,
                    });
                }
                4 => {
                    builder.push(IROp::Load {
                        dst: 5,
                        base: 0,
                        offset: (i * 8) as i64,
                        size: 8,
                        flags: MemFlags::default(),
                    });
                }
                5 => {
                    builder.push(IROp::Store {
                        base: 0,
                        offset: (i * 8) as i64,
                        size: 8,
                        src: 5,
                        flags: MemFlags::default(),
                    });
                }
                6 => {
                    builder.push(IROp::SllImm {
                        dst: 6,
                        src: 0,
                        sh: 2,
                    });
                }
                _ => {
                    builder.push(IROp::SrlImm {
                        dst: 7,
                        src: 6,
                        sh: 1,
                    });
                }
            }
        }
        
        builder.set_term(Terminator::Jmp { target: addr + (complexity * 32) as u64 });
        builder.build()
    }

    /// 创建SIMD测试IR块
    fn create_simd_test_ir_block(&self, addr: GuestAddr, vector_length: usize) -> IRBlock {
        let mut builder = IRBuilder::new(addr);
        
        // 创建适合SIMD向量化的操作序列
        for i in 0..vector_length {
            // 连续的加法操作 - 适合SLP向量化
            builder.push(IROp::MovImm { dst: (i % 8) as u32, imm: (i * 10) as u64 });
            builder.push(IROp::Add {
                dst: (i % 8) as u32,
                src1: (i % 8) as u32,
                src2: ((i + 1) % 8) as u32,
            });
            builder.push(IROp::Mul {
                dst: (i % 8) as u32,
                src1: (i % 8) as u32,
                src2: 2,
            });
        }
        
        builder.set_term(Terminator::Jmp { target: addr + (vector_length * 24) as u64 });
        builder.build()
    }
}

/// 运行JIT引擎性能测试
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jit_performance_suite() {
        let config = TestConfig {
            iterations: 10,
            warmup_iterations: 2,
            timeout_seconds: 30,
        };
        
        let mut test_suite = JITPerformanceTestSuite::new(config);
        let result = test_suite.run_all_tests();
        
        assert!(result.is_ok(), "JIT性能测试套件运行失败");
    }
}

/// 主函数 - 运行性能测试
fn main() -> Result<(), VmError> {
    let config = TestConfig::default();
    let mut test_suite = JITPerformanceTestSuite::new(config);
    test_suite.run_all_tests()
}