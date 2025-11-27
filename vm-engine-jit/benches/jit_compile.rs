//! JIT 编译器性能基准测试
//!
//! 测试内容:
//! - 编译延迟
//! - 执行性能
//! - 热点追踪开销
//! - 自适应阈值性能

use criterion::{criterion_group, criterion_main, Criterion, black_box, BenchmarkId, Throughput};
use vm_engine_jit::{Jit, AdaptiveThreshold, AdaptiveThresholdConfig};
use vm_ir::{IRBlock, IROp, Terminator, IRBuilder};
use vm_core::GuestAddr;

/// 创建简单的算术操作块
fn create_arithmetic_block(pc: GuestAddr, num_ops: usize) -> IRBlock {
    let mut builder = IRBuilder::new(pc);
    
    // 生成一系列算术操作
    for i in 0..num_ops {
        let dst = ((i % 30) + 1) as u32; // x1-x30
        let src1 = ((i % 29) + 1) as u32;
        let src2 = ((i % 28) + 2) as u32;
        
        match i % 4 {
            0 => builder.push(IROp::Add { dst, src1, src2 }),
            1 => builder.push(IROp::Sub { dst, src1, src2 }),
            2 => builder.push(IROp::And { dst, src1, src2 }),
            _ => builder.push(IROp::Or { dst, src1, src2 }),
        }
    }
    
    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 创建内存操作块
fn create_memory_block(pc: GuestAddr, num_ops: usize) -> IRBlock {
    let mut builder = IRBuilder::new(pc);
    
    for i in 0..num_ops {
        let dst = ((i % 30) + 1) as u32;
        let base = 1u32; // x1 作为基址
        let offset = (i * 8) as i64;
        
        if i % 2 == 0 {
            builder.push(IROp::Load {
                dst,
                base,
                offset,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            });
        } else {
            builder.push(IROp::Store {
                src: dst,
                base,
                offset,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            });
        }
    }
    
    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 基准测试: 编译延迟
fn bench_compile_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_compile_latency");
    
    // 测试不同块大小的编译时间
    for num_ops in [10, 50, 100, 200].iter() {
        let block = create_arithmetic_block(0x1000, *num_ops);
        
        group.throughput(Throughput::Elements(*num_ops as u64));
        group.bench_with_input(
            BenchmarkId::new("arithmetic_ops", num_ops),
            &block,
            |b, block| {
                b.iter(|| {
                    let mut jit = Jit::new();
                    // 强制编译 (绕过热点检测)
                    for _ in 0..101 {
                        jit.record_execution(block.start_pc);
                    }
                    // 返回编译后的缓存大小作为结果
                    jit.total_compiled + jit.total_interpreted
                })
            },
        );
    }
    
    group.finish();
}

/// 基准测试: 内存操作编译
fn bench_compile_memory_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_compile_memory");
    
    for num_ops in [10, 50, 100].iter() {
        let block = create_memory_block(0x2000, *num_ops);
        
        group.throughput(Throughput::Elements(*num_ops as u64));
        group.bench_with_input(
            BenchmarkId::new("memory_ops", num_ops),
            &block,
            |b, block| {
                b.iter(|| {
                    let mut jit = Jit::new();
                    for _ in 0..101 {
                        jit.record_execution(block.start_pc);
                    }
                    jit.total_compiled + jit.total_interpreted
                })
            },
        );
    }
    
    group.finish();
}

/// 基准测试: 热点追踪开销
fn bench_hot_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_hot_tracking");
    
    // 测试记录执行的开销
    group.bench_function("record_execution", |b| {
        let mut jit = Jit::new();
        let mut pc = 0x1000u64;
        
        b.iter(|| {
            let result = jit.record_execution(pc);
            pc = pc.wrapping_add(4);
            if pc > 0x100000 {
                pc = 0x1000;
            }
            black_box(result)
        })
    });
    
    // 测试热点检查开销
    group.bench_function("is_hot", |b| {
        let mut jit = Jit::new();
        // 预热一些地址
        for i in 0..1000 {
            for _ in 0..100 {
                jit.record_execution(0x1000 + i * 4);
            }
        }
        
        b.iter(|| {
            let result = jit.is_hot(black_box(0x1000 + (black_box(500u64) * 4)));
            black_box(result)
        })
    });
    
    group.finish();
}

/// 基准测试: 自适应阈值性能
fn bench_adaptive_threshold(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_threshold");
    
    // 测试阈值调整开销
    group.bench_function("adjust", |b| {
        let mut threshold = AdaptiveThreshold::new();
        
        // 模拟一些执行数据
        for _ in 0..500 {
            threshold.record_compile(1_000_000); // 1ms
            threshold.record_compiled_hit(50_000, 100_000);
            threshold.record_interpreted();
        }
        
        b.iter(|| {
            threshold.adjust();
            black_box(threshold.threshold())
        })
    });
    
    // 测试记录编译事件开销
    group.bench_function("record_compile", |b| {
        let mut threshold = AdaptiveThreshold::new();
        
        b.iter(|| {
            threshold.record_compile(black_box(1_000_000));
        })
    });
    
    // 测试记录执行事件开销
    group.bench_function("record_hit", |b| {
        let mut threshold = AdaptiveThreshold::new();
        
        b.iter(|| {
            threshold.record_compiled_hit(black_box(50_000), black_box(100_000));
        })
    });
    
    group.finish();
}

/// 基准测试: 不同配置的自适应阈值
fn bench_adaptive_configs(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_configs");
    
    let configs = vec![
        ("default", AdaptiveThresholdConfig::default()),
        ("aggressive", AdaptiveThresholdConfig {
            min_threshold: 5,
            max_threshold: 50,
            sample_window: 50,
            compile_time_weight: 0.2,
            exec_benefit_weight: 0.8,
        }),
        ("conservative", AdaptiveThresholdConfig {
            min_threshold: 50,
            max_threshold: 2000,
            sample_window: 200,
            compile_time_weight: 0.5,
            exec_benefit_weight: 0.5,
        }),
    ];
    
    for (name, config) in configs {
        group.bench_with_input(
            BenchmarkId::new("threshold_convergence", name),
            &config,
            |b, config| {
                b.iter(|| {
                    let mut threshold = AdaptiveThreshold::with_config(config.clone());
                    
                    // 模拟 1000 次执行
                    for i in 0..1000 {
                        if i % 10 == 0 {
                            threshold.record_compile(500_000);
                        }
                        if i % 2 == 0 {
                            threshold.record_compiled_hit(30_000, 80_000);
                        } else {
                            threshold.record_interpreted();
                        }
                        if i % 100 == 0 {
                            threshold.adjust();
                        }
                    }
                    
                    black_box(threshold.threshold())
                })
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_compile_latency,
    bench_compile_memory_ops,
    bench_hot_tracking,
    bench_adaptive_threshold,
    bench_adaptive_configs
);
criterion_main!(benches);

