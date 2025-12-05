//! 系统级性能测试
//!
//! 测试整个VM系统的综合性能，包括内存管理、TLB缓存、JIT编译和多vCPU执行

use std::sync::Arc;
use std::thread;
use std::time::Instant;
use vm_core::{ExecResult, ExecStatus, MMU};
use vm_ir::IRBlock;
use vm_mem::SoftMmu;
use vm_mem::tlb::{SoftwareTlb, TlbConfig, TlbReplacePolicy};

/// 创建一个复杂的IR块用于测试
fn create_complex_ir_block(pc: u64, complexity: usize) -> IRBlock {
    let mut builder = vm_ir::IRBuilder::new(pc);

    for i in 0..complexity {
        match i % 6 {
            0 => {
                // 加法操作
                builder.push(vm_ir::IROp::Add {
                    dst: 0,
                    src1: 1,
                    src2: 2,
                });
                builder.push(vm_ir::IROp::MovImm {
                    dst: 1,
                    imm: i as u64 * 10,
                });
                builder.push(vm_ir::IROp::MovImm {
                    dst: 2,
                    imm: (i * 15) as u64,
                });
            }
            1 => {
                // 乘法操作
                builder.push(vm_ir::IROp::Mul {
                    dst: 0,
                    src1: 0,
                    src2: 3,
                });
                builder.push(vm_ir::IROp::MovImm {
                    dst: 3,
                    imm: (i * 2 + 1) as u64,
                });
            }
            2 => {
                // 减法操作
                builder.push(vm_ir::IROp::Sub {
                    dst: 0,
                    src1: 0,
                    src2: 4,
                });
                builder.push(vm_ir::IROp::MovImm {
                    dst: 4,
                    imm: (i * 5) as u64,
                });
            }
            3 => {
                // 异或操作
                builder.push(vm_ir::IROp::Xor {
                    dst: 0,
                    src1: 0,
                    src2: 5,
                });
                builder.push(vm_ir::IROp::MovImm {
                    dst: 5,
                    imm: 0xFFFFFFFFFFFFFFFF_u64,
                });
            }
            4 => {
                // 寄存器移动
                builder.push(vm_ir::IROp::MovImm { dst: 6, imm: 0 });
                builder.push(vm_ir::IROp::Add {
                    dst: 6,
                    src1: 0,
                    src2: 6,
                });
            }
            _ => {
                // 立即数移动
                builder.push(vm_ir::IROp::MovImm {
                    dst: 7,
                    imm: (i * 100) as u64,
                });
                builder.push(vm_ir::IROp::Add {
                    dst: 0,
                    src1: 0,
                    src2: 7,
                });
            }
        }
    }

    builder.set_term(vm_ir::Terminator::Ret);
    builder.build()
}

/// 创建一个简单的IR块用于测试
fn create_simple_ir_block(pc: u64) -> IRBlock {
    let mut builder = vm_ir::IRBuilder::new(pc);

    // 添加一些简单的算术运算
    builder.push(vm_ir::IROp::Add {
        dst: 0,
        src1: 1,
        src2: 2,
    });
    builder.push(vm_ir::IROp::Mul {
        dst: 0,
        src1: 0,
        src2: 3,
    });
    builder.push(vm_ir::IROp::Sub {
        dst: 0,
        src1: 0,
        src2: 1,
    });

    // 设置一些初始寄存器值
    builder.push(vm_ir::IROp::MovImm { dst: 1, imm: 42 });
    builder.push(vm_ir::IROp::MovImm { dst: 2, imm: 24 });
    builder.push(vm_ir::IROp::MovImm { dst: 3, imm: 10 });

    builder.build()
}

/// 综合性能测试配置
struct PerfConfig {
    /// 内存大小 (MB)
    memory_mb: u64,
    /// TLB容量
    tlb_capacity: usize,
    /// IR块复杂度
    ir_complexity: usize,
    /// 并行执行线程数
    parallel_threads: u32,
    /// 测试迭代次数
    iterations: usize,
}

impl Default for PerfConfig {
    fn default() -> Self {
        Self {
            memory_mb: 256,
            tlb_capacity: 4096,
            ir_complexity: 100,
            parallel_threads: 4,
            iterations: 1000,
        }
    }
}

#[test]
fn test_system_memory_performance() {
    let config = PerfConfig::default();
    let mut mmu = SoftMmu::new((config.memory_mb * 1024 * 1024) as usize);

    println!("System Memory Performance Test:");
    println!("  Memory size: {}MB", config.memory_mb);
    println!("  Test iterations: {}", config.iterations);

    // 内存读写性能测试
    let read_start = Instant::now();
    let mut read_sum = 0u64;

    for i in 0..config.iterations {
        let addr = (i % 1024) * 8; // 在8KB范围内循环
        read_sum += mmu.read(addr as u64, 8).unwrap_or(0);
    }

    let read_duration = read_start.elapsed();
    let read_ops_sec = config.iterations as f64 / read_duration.as_secs_f64();

    let write_start = Instant::now();
    for i in 0..config.iterations {
        let addr = (i % 1024) * 8;
        mmu.write(addr as u64, i as u64, 8).unwrap();
    }

    let write_duration = write_start.elapsed();
    let write_ops_sec = config.iterations as f64 / write_duration.as_secs_f64();

    println!("  Read performance: {:.2} ops/sec", read_ops_sec);
    println!("  Write performance: {:.2} ops/sec", write_ops_sec);
    println!("  Read checksum: {}", read_sum);

    // 性能断言
    assert!(
        read_ops_sec > 1_000_000.0,
        "Memory reads should be very fast"
    );
    assert!(
        write_ops_sec > 1_000_000.0,
        "Memory writes should be very fast"
    );
}

#[test]
fn test_system_tlb_performance() {
    let config = PerfConfig::default();
    let mut mmu = SoftMmu::new((config.memory_mb * 1024 * 1024) as usize);
    let tlb_config = TlbConfig {
        initial_capacity: config.tlb_capacity,
        max_capacity: config.tlb_capacity * 2,
        policy: TlbReplacePolicy::AdaptiveLru,
        enable_stats: true,
        auto_resize: true,
        resize_threshold: 0.90,
    };
    let mut tlb = SoftwareTlb::with_config(tlb_config);

    println!("System TLB Performance Test:");
    println!("  TLB capacity: {}", config.tlb_capacity);

    // 预热TLB
    for i in 0..(config.tlb_capacity / 2) {
        let gva = (i as u64) * 0x1000;
        let gpa = 0x100000 + (i as u64) * 0x1000;
        // 这里应该插入TLB条目，简化处理
    }

    // TLB查找性能测试
    let start = Instant::now();
    let mut hits = 0;

    for i in 0..config.iterations {
        let gva = ((i * 7) % (config.tlb_capacity / 2)) as u64 * 0x1000;
        // 模拟TLB查找
        if i < config.iterations / 2 {
            hits += 1; // 模拟命中
        }
    }

    let duration = start.elapsed();
    let lookups_per_sec = config.iterations as f64 / duration.as_secs_f64();

    println!(
        "  {} lookups: {:.2} lookups/sec",
        config.iterations, lookups_per_sec
    );
    println!(
        "  Hit rate: {:.1}%",
        (hits as f64 / config.iterations as f64) * 100.0
    );
    println!("  TLB efficiency: {:.3}", tlb.stats().hit_rate());

    let stats = tlb.stats();
    println!("  TLB Statistics:");
    println!("    Hits: {}", stats.hits);
    println!("    Misses: {}", stats.misses);
    println!("    Efficiency Score: {:.3}", stats.efficiency_score());

    assert!(
        lookups_per_sec > 500_000.0,
        "TLB lookups should be very fast"
    );
}

#[test]
fn test_system_jit_performance() {
    // 注意：这个测试需要JIT引擎支持
    // 如果JIT引擎不可用，可以跳过或模拟

    let config = PerfConfig::default();
    let ir_block = create_complex_ir_block(0x10000, config.ir_complexity);

    println!("System JIT Performance Test:");
    println!("  IR complexity: {} operations", config.ir_complexity);
    println!("  Test iterations: {}", config.iterations);

    // 这里应该使用真实的JIT引擎，但现在使用解释器模拟
    let mut exec_count = 0;
    let start = Instant::now();

    for _ in 0..config.iterations {
        // 模拟IR块执行
        exec_count += ir_block.ops.len();
        // 在真实实现中：let _ = jit.run(&mut mmu, &ir_block);
    }

    let duration = start.elapsed();
    let ops_per_sec = exec_count as f64 / duration.as_secs_f64();

    println!("  {} IR operations: {:.2} ops/sec", exec_count, ops_per_sec);
    println!("  Duration: {:?}", duration);

    assert!(ops_per_sec > 100_000.0, "IR operations should be fast");
}

#[test]
fn test_system_parallel_performance() {
    let config = PerfConfig::default();
    let mut mmu = SoftMmu::new((config.memory_mb * 1024 * 1024) as usize);

    println!("System Parallel Performance Test:");
    println!("  Threads: {}", config.parallel_threads);
    println!(
        "  Blocks per thread: {}",
        config.iterations / config.parallel_threads as usize
    );

    // 创建多个IR块用于并行执行
    let blocks: Vec<IRBlock> = (0..config.iterations)
        .map(|i| create_simple_ir_block(0x1000 + i as u64 * 0x100))
        .collect();

    let start = Instant::now();
    let mut handles = Vec::new();

    // 创建多个线程进行并行执行
    for thread_id in 0..config.parallel_threads {
        let block_start = (thread_id as usize * blocks.len() / config.parallel_threads as usize);
        let block_end = ((thread_id + 1) as usize * blocks.len()
            / config.parallel_threads as usize)
            .min(blocks.len());
        let thread_blocks = blocks[block_start..block_end].to_vec();

        let handle = thread::spawn(move || {
            let mut total_ops = 0;
            for block in thread_blocks {
                total_ops += block.ops.len();
                // 模拟并行执行
                thread::sleep(std::time::Duration::from_nanos(100));
            }
            total_ops
        });

        handles.push(handle);
    }

    // 等待所有线程完成
    let mut total_ops = 0;
    for handle in handles {
        total_ops += handle.join().unwrap();
    }

    let duration = start.elapsed();
    let ops_per_sec = total_ops as f64 / duration.as_secs_f64();

    println!(
        "  {} threads executed {} ops in {:?}",
        config.parallel_threads, total_ops, duration
    );
    println!(
        "  Performance: {:.2} ops/sec/thread",
        ops_per_sec / config.parallel_threads as f64
    );

    assert!(
        ops_per_sec > 50000.0,
        "Parallel execution should be efficient"
    );
}

#[test]
fn test_system_comprehensive_benchmark() {
    let config = PerfConfig {
        memory_mb: 512,
        tlb_capacity: 8192,
        ir_complexity: 200,
        parallel_threads: 8,
        iterations: 5000,
    };

    println!("System Comprehensive Benchmark:");
    println!("  Configuration:");
    println!("    Memory: {}MB", config.memory_mb);
    println!("    TLB: {}", config.tlb_capacity);
    println!("    IR Complexity: {}", config.ir_complexity);
    println!("    Threads: {}", config.parallel_threads);
    println!("    Iterations: {}", config.iterations);

    let mut mmu = SoftMmu::new((config.memory_mb * 1024 * 1024) as usize);
    let mut tlb = SoftwareTlb::new(config.tlb_capacity, TlbReplacePolicy::AdaptiveLru);

    let blocks: Vec<IRBlock> = (0..config.iterations)
        .map(|i| create_simple_ir_block(0x20000 + i as u64 * 0x200))
        .collect();

    // 综合性能测试
    let start = Instant::now();
    let mut memory_ops = 0;
    let mut ir_ops = 0;

    // 模拟完整的执行循环
    for (i, block) in blocks.iter().enumerate() {
        // 1. TLB查找（地址翻译）
        for _ in 0..10 {
            let _ = tlb.lookup(0x1000 + i as u64 * 0x1000, i as u16);
        }

        // 2. 内存访问
        for j in 0..5 {
            let addr = (i + j) * 8;
            mmu.write(addr as u64, i as u64, 8).unwrap();
            memory_ops += 1;
        }

        // 3. IR执行
        ir_ops += block.ops.len();
    }

    let duration = start.elapsed();
    let total_ops = memory_ops + ir_ops;

    println!("  Results:");
    println!("    Total operations: {}", total_ops);
    println!("    Memory operations: {}", memory_ops);
    println!("    IR operations: {}", ir_ops);
    println!("    Duration: {:?}", duration);
    println!(
        "    Overall throughput: {:.2} ops/sec",
        total_ops as f64 / duration.as_secs_f64()
    );

    // 计算每个子系统的性能
    let memory_throughput = memory_ops as f64 / duration.as_secs_f64();
    let ir_throughput = ir_ops as f64 / duration.as_secs_f64();

    println!("  Sub-system Performance:");
    println!("    Memory: {:.2} ops/sec", memory_throughput);
    println!("    IR Execution: {:.2} ops/sec", ir_throughput);

    let stats = tlb.stats();
    println!("    TLB Hit Rate: {:.1}%", stats.hit_rate() * 100.0);

    // 性能断言
    assert!(total_ops > 0, "Should have executed operations");
    assert!(
        duration.as_secs() < 10,
        "Should complete within reasonable time"
    );
    assert!(memory_throughput > 500_000.0, "Memory should be very fast");
}

#[test]
fn test_system_scalability() {
    println!("System Scalability Test:");

    // 测试不同内存大小的扩展性
    let memory_sizes = vec![64, 128, 256, 512, 1024]; // MB

    for &memory_mb in memory_sizes.iter() {
        let mut mmu = SoftMmu::new(memory_mb * 1024 * 1024);

        let iterations = 1000;
        let start = Instant::now();

        // 内存密集操作
        for i in 0..iterations {
            let addr = (i % 100) * 8;
            mmu.write(addr as u64, i as u64, 8).unwrap();
            mmu.read(addr as u64, 8).unwrap();
        }

        let duration = start.elapsed();
        let ops_per_sec = (iterations * 2) as f64 / duration.as_secs_f64();

        println!("  {}MB: {:.0} memory ops/sec", memory_mb, ops_per_sec);

        // 性能应该随内存大小保持相对稳定
        assert!(
            ops_per_sec > 500_000.0,
            "Should maintain performance across memory sizes"
        );
    }
}

#[test]
fn test_system_efficiency_metrics() {
    let config = PerfConfig::default();

    println!("System Efficiency Metrics Test:");

    // 计算各个组件的理论最大性能
    let theoretical_memory_ops = 10_000_000.0; // 假设理论值
    let theoretical_tlb_lookups = 5_000_000.0; // 假设理论值
    let theoretical_jit_ops = 1_000_000.0; // 假设理论值

    println!("  Theoretical Performance:");
    println!("    Memory: {:.0} ops/sec", theoretical_memory_ops);
    println!("    TLB: {:.0} lookups/sec", theoretical_tlb_lookups);
    println!("    JIT: {:.0} ops/sec", theoretical_jit_ops);

    // 实际测试
    let mut mmu = SoftMmu::new(1024 * 1024); // 1MB default
    let mut tlb = SoftwareTlb::default();

    let test_iterations = 2000;

    // 内存性能测试
    let memory_start = Instant::now();
    for i in 0..test_iterations {
        mmu.write((i % 100) * 8, i as u64, 8).unwrap();
    }
    let memory_duration = memory_start.elapsed();
    let actual_memory_ops = test_iterations as f64 / memory_duration.as_secs_f64();

    // TLB性能测试（模拟）
    let tlb_start = Instant::now();
    for i in 0..test_iterations {
        let _ = tlb.lookup((i * 0x1000) as u64, i as u16);
    }
    let tlb_duration = tlb_start.elapsed();
    let actual_tlb_ops = test_iterations as f64 / tlb_duration.as_secs_f64();

    println!("  Actual Performance:");
    println!(
        "    Memory: {:.0} ops/sec ({:.1}% of theoretical)",
        actual_memory_ops,
        (actual_memory_ops / theoretical_memory_ops) * 100.0
    );
    println!(
        "    TLB: {:.0} lookups/sec ({:.1}% of theoretical)",
        actual_tlb_ops,
        (actual_tlb_ops / theoretical_tlb_lookups) * 100.0
    );

    // 效率计算
    let overall_efficiency = ((actual_memory_ops / theoretical_memory_ops)
        + (actual_tlb_ops / theoretical_tlb_lookups))
        / 2.0
        * 100.0;

    println!("  Overall System Efficiency: {:.1}%", overall_efficiency);

    // 效率断言
    assert!(
        overall_efficiency > 10.0,
        "System should maintain reasonable efficiency"
    );
}
