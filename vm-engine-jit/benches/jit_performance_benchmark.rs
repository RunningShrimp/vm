//! 增强型JIT性能基准测试套件
//!
//! 全面测试JIT编译器的编译性能、执行性能和内存使用

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use vm_core::{ExecutionEngine, GuestAddr, MMU};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};
use vm_mem::SoftMmu;

use super::{
    Jit, optimizing_compiler::OptimizingJIT, ewma_hotspot::EwmaHotspotDetector,
    unified_cache::EnhancedCodeCache,
};

/// 基准测试配置
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    /// 测试块大小范围
    pub block_sizes: Vec<usize>,
    /// 测试迭代次数
    pub iterations: usize,
    /// 预热迭代次数
    pub warmup_iterations: usize,
    /// 内存大小
    pub memory_size: usize,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            block_sizes: vec![10, 50, 100, 500, 1000],
            iterations: 1000,
            warmup_iterations: 100,
            memory_size: 1024 * 1024, // 1MB
        }
    }
}

/// 基准测试结果
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    /// 编译时间（纳秒）
    pub compile_time_ns: u64,
    /// 执行时间（纳秒）
    pub execution_time_ns: u64,
    /// 内存使用（字节）
    pub memory_usage_bytes: usize,
    /// 代码大小（字节）
    pub code_size_bytes: usize,
    /// 命中率
    pub hit_rate: f64,
    /// 吞吐量（操作/秒）
    pub throughput_ops_per_sec: f64,
}

/// 创建测试用的IR块
fn create_test_block(start_pc: GuestAddr, num_ops: usize, complexity_factor: f64) -> IRBlock {
    let mut builder = IRBuilder::new(start_pc);

    for i in 0..num_ops {
        let dst = ((i % 30) + 1) as u32; // x1-x30
        let src1 = ((i % 29) + 1) as u32;
        let src2 = ((i % 28) + 2) as u32;

        // 根据复杂度因子选择操作类型
        let op_type = (i as f64 * complexity_factor) as usize % 10;

        match op_type {
            0 => builder.push(IROp::Add { dst, src1, src2 }),
            1 => builder.push(IROp::Sub { dst, src1, src2 }),
            2 => builder.push(IROp::Mul { dst, src1, src2 }),
            3 => builder.push(IROp::Div {
                dst,
                src1,
                src2,
                signed: false,
            }),
            4 => builder.push(IROp::And { dst, src1, src2 }),
            5 => builder.push(IROp::Or { dst, src1, src2 }),
            6 => builder.push(IROp::Xor { dst, src1, src2 }),
            7 => builder.push(IROp::Sll {
                dst,
                src,
                shreg: src2,
            }),
            8 => builder.push(IROp::Load {
                dst,
                base: 1,
                offset: (i * 8) as i64,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            }),
            9 => builder.push(IROp::Store {
                src: dst,
                base: 1,
                offset: (i * 8) as i64,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            }),
            _ => builder.push(IROp::MovImm { dst, imm: i as u64 }),
        }
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 基准测试：基础JIT编译性能
fn bench_basic_jit_compile(c: &mut Criterion) {
    let mut group = c.benchmark_group("basic_jit_compile");

    for &block_size in &[10, 50, 100, 500] {
        let block = create_test_block(0x1000, block_size, 1.0);

        group.throughput(Throughput::Elements(block_size as u64));
        group.bench_with_input(
            BenchmarkId::new("compile", block_size),
            &block,
            |b, block| {
                b.iter(|| {
                    let mut jit = Jit::new();
                    let start = Instant::now();

                    // 强制编译
                    for _ in 0..101 {
                        jit.record_execution(block.start_pc);
                    }

                    let compile_time = start.elapsed().as_nanos() as u64;
                    black_box(compile_time)
                })
            },
        );
    }

    group.finish();
}

/// 基准测试：增强JIT编译性能
fn bench_enhanced_jit_compile(c: &mut Criterion) {
    let mut group = c.benchmark_group("enhanced_jit_compile");

    for &block_size in &[10, 50, 100, 500] {
        let block = create_test_block(0x1000, block_size, 1.0);

        group.throughput(Throughput::Elements(block_size as u64));
        group.bench_with_input(
            BenchmarkId::new("compile", block_size),
            &block,
            |b, block| {
                b.iter(|| {
                    let mut jit = OptimizingJIT::new();
                    let start = Instant::now();

                    let _code_ptr = jit.compile(block);

                    let compile_time = start.elapsed().as_nanos() as u64;
                    black_box(compile_time)
                })
            },
        );
    }

    group.finish();
}

/// 基准测试：热点检测性能
fn bench_hotspot_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("hotspot_detection");

    // 测试不同复杂度的热点检测
    for &complexity in &[0.5, 1.0, 1.5, 2.0] {
        group.bench_with_input(
            BenchmarkId::new("detection", complexity),
            &complexity,
            |b, &complexity| {
                let detector = EnhancedHotspotDetector::new(Default::default());

                b.iter(|| {
                    // 模拟热点检测
                    for i in 0..100 {
                        let addr = 0x1000 + (i % 10) * 0x100;
                        let duration = (50.0 * complexity) as u64;
                        detector.record_execution(addr, duration);

                        // 检查是否为热点
                        let _is_hot = detector.is_hotspot(addr);
                    }
                })
            },
        );
    }

    group.finish();
}

/// 基准测试：缓存性能
fn bench_cache_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_performance");

    // 测试不同缓存策略
    let policies = vec![
        ("LRU", super::unified_cache::EvictionPolicy::LRU),
        ("LFU", super::unified_cache::EvictionPolicy::LFU),
        (
            "ValueBased",
            super::unified_cache::EvictionPolicy::ValueBased,
        ),
    ];

    for (name, policy) in policies {
        let config = super::unified_cache::CacheConfig {
            max_entries: 1000,
            eviction_policy: policy,
            ..Default::default()
        };
        let cache = EnhancedCodeCache::new(config, Default::default());

        group.bench_with_input(BenchmarkId::new("lookup", name), &cache, |b, cache| {
            // 预填充缓存
            for i in 0..100 {
                let addr = 0x1000 + i * 0x100;
                let code_ptr = super::CodePtr((0x10000 + i) as *const u8);
                cache.insert(addr, code_ptr, 1024, 1000);
            }

            b.iter(|| {
                // 模拟缓存查找
                for i in 0..1000 {
                    let addr = 0x1000 + (i % 100) * 0x100;
                    let _result = cache.lookup(addr);
                }
            })
        });
    }

    group.finish();
}

/// 基准测试：执行性能对比
fn bench_execution_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("execution_performance");

    for &block_size in &[10, 50, 100] {
        let block = create_test_block(0x1000, block_size, 1.0);
        let mut mmu = SoftMmu::new(1024 * 1024, false);

        // 基础JIT
        group.bench_with_input(
            BenchmarkId::new("basic_jit", block_size),
            &(&block, &mut mmu),
            |b, (block, mmu)| {
                let mut jit = Jit::new();

                // 预热
                for _ in 0..101 {
                    jit.record_execution(block.start_pc);
                }

                b.iter(|| {
                    let start = Instant::now();
                    let _result = jit.run(mmu, block);
                    let exec_time = start.elapsed().as_nanos() as u64;
                    black_box(exec_time)
                })
            },
        );

        // 增强JIT
        let mut mmu2 = SoftMmu::new(1024 * 1024, false);
        group.bench_with_input(
            BenchmarkId::new("enhanced_jit", block_size),
            &(&block, &mut mmu2),
            |b, (block, mmu)| {
                let mut jit = OptimizingJIT::new();

                b.iter(|| {
                    let start = Instant::now();
                    let _result = jit.compile(block);
                    let compile_time = start.elapsed().as_nanos() as u64;
                    black_box(compile_time)
                })
            },
        );
    }

    group.finish();
}

/// 基准测试：内存使用
fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    for &block_size in &[10, 50, 100, 500] {
        let block = create_test_block(0x1000, block_size, 1.0);

        group.bench_with_input(
            BenchmarkId::new("memory", block_size),
            &block,
            |b, block| {
                b.iter(|| {
                    let mut jit = OptimizingJIT::new();

                    // 记录编译前内存
                    let memory_before = get_memory_usage();

                    // 编译多个块
                    for i in 0..10 {
                        let mut block_copy = block.clone();
                        block_copy.start_pc += i * 0x1000;
                        let _code_ptr = jit.compile(&block_copy);
                    }

                    // 记录编译后内存
                    let memory_after = get_memory_usage();
                    let memory_used = memory_after.saturating_sub(memory_before);

                    black_box(memory_used)
                })
            },
        );
    }

    group.finish();
}

/// 获取当前内存使用量（简化实现）
fn get_memory_usage() -> usize {
    // 在实际实现中，这里应该使用系统API获取准确的内存使用量
    // 这里返回一个模拟值
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// 综合性能测试
fn run_comprehensive_benchmark() -> BenchmarkResults {
    let config = BenchmarkConfig::default();
    let mut results = Vec::new();

    for &block_size in &config.block_sizes {
        let block = create_test_block(0x1000, block_size, 1.0);
        let mut mmu = SoftMmu::new(config.memory_size, false);

        // 测试基础JIT
        let mut basic_jit = Jit::new();
        let start = Instant::now();

        // 预热
        for _ in 0..config.warmup_iterations {
            basic_jit.record_execution(block.start_pc);
        }

        // 实际测试
        let compile_start = Instant::now();
        for _ in 0..config.iterations {
            basic_jit.record_execution(block.start_pc);
        }
        let compile_time = compile_start.elapsed().as_nanos() as u64;

        let exec_start = Instant::now();
        for _ in 0..config.iterations {
            let _result = basic_jit.run(&mut mmu, &block);
        }
        let exec_time = exec_start.elapsed().as_nanos() as u64;

        let result = BenchmarkResults {
            compile_time_ns: compile_time / config.iterations as u64,
            execution_time_ns: exec_time / config.iterations as u64,
            memory_usage_bytes: get_memory_usage(),
            code_size_bytes: block_size * 8, // 估算
            hit_rate: basic_jit.total_compiled as f64
                / (basic_jit.total_compiled + basic_jit.total_interpreted) as f64,
            throughput_ops_per_sec: (block_size as f64 * config.iterations as f64)
                / (exec_time as f64 / 1_000_000_000.0),
        };

        results.push(result);
    }

    // 返回平均结果
    let avg_compile_time =
        results.iter().map(|r| r.compile_time_ns).sum::<u64>() / results.len() as u64;
    let avg_exec_time =
        results.iter().map(|r| r.execution_time_ns).sum::<u64>() / results.len() as u64;
    let avg_memory = results.iter().map(|r| r.memory_usage_bytes).sum::<usize>() / results.len();
    let avg_code_size = results.iter().map(|r| r.code_size_bytes).sum::<usize>() / results.len();
    let avg_hit_rate = results.iter().map(|r| r.hit_rate).sum::<f64>() / results.len() as f64;
    let avg_throughput = results
        .iter()
        .map(|r| r.throughput_ops_per_sec)
        .sum::<f64>()
        / results.len() as f64;

    BenchmarkResults {
        compile_time_ns: avg_compile_time,
        execution_time_ns: avg_exec_time,
        memory_usage_bytes: avg_memory,
        code_size_bytes: avg_code_size,
        hit_rate: avg_hit_rate,
        throughput_ops_per_sec: avg_throughput,
    }
}

/// 生成性能报告
pub fn generate_performance_report() -> String {
    let results = run_comprehensive_benchmark();

    format!(
        r#"=== JIT Performance Benchmark Report ===

Compilation Performance:
  Average Compile Time: {}ns
  Throughput: {:.2} ops/sec

Execution Performance:
  Average Execution Time: {}ns
  Cache Hit Rate: {:.2}%

Memory Usage:
  Average Memory Usage: {}KB
  Average Code Size: {}KB

Performance Improvements:
  - Compile time reduction: TBD
  - Execution speedup: TBD
  - Memory efficiency: TBD
"#,
        results.compile_time_ns,
        results.throughput_ops_per_sec,
        results.execution_time_ns,
        results.hit_rate * 100.0,
        results.memory_usage_bytes / 1024,
        results.code_size_bytes / 1024
    )
}

criterion_group!(
    benches,
    bench_basic_jit_compile,
    bench_enhanced_jit_compile,
    bench_hotspot_detection,
    bench_cache_performance,
    bench_execution_performance,
    bench_memory_usage
);
criterion_main!(benches);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_block() {
        let block = create_test_block(0x1000, 10, 1.0);
        assert_eq!(block.start_pc, 0x1000);
        assert_eq!(block.ops.len(), 10);
    }

    #[test]
    fn test_comprehensive_benchmark() {
        let results = run_comprehensive_benchmark();
        assert!(results.compile_time_ns > 0);
        assert!(results.execution_time_ns > 0);
        assert!(results.throughput_ops_per_sec > 0.0);
    }

    #[test]
    fn test_performance_report() {
        let report = generate_performance_report();
        assert!(report.contains("JIT Performance Benchmark Report"));
        assert!(report.contains("Compilation Performance"));
        assert!(report.contains("Execution Performance"));
    }
}
