//! 综合性能基准测试
//!
//! 本模块提供全面的性能基准测试，包括：
//! - 跨架构翻译性能
//! - JIT编译性能
//! - 内存管理性能
//! - 并发执行性能
//! - 资源使用效率

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use vm_cross_arch::UnifiedExecutor;
use vm_core::{GuestArch, MMU};
use vm_engine::jit::core::{JITEngine, JITConfig};
use vm_mem::{SoftMmu, MemoryManager};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator, MemFlags};

/// 基准测试：跨架构翻译性能
fn bench_cross_arch_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cross_arch_translation");
    
    let arch_combinations = vec![
        ("x86_64_to_arm64", GuestArch::X86_64, GuestArch::ARM64),
        ("x86_64_to_riscv64", GuestArch::X86_64, GuestArch::RISCV64),
        ("arm64_to_x86_64", GuestArch::ARM64, GuestArch::X86_64),
        ("arm64_to_riscv64", GuestArch::ARM64, GuestArch::RISCV64),
        ("riscv64_to_x86_64", GuestArch::RISCV64, GuestArch::X86_64),
        ("riscv64_to_arm64", GuestArch::RISCV64, GuestArch::ARM64),
    ];
    
    for (name, src_arch, _dst_arch) in arch_combinations {
        group.bench_with_input(
            BenchmarkId::new("translation", name),
            &src_arch,
            |b, &src_arch| {
                b.iter(|| {
                    let mut executor = UnifiedExecutor::auto_create(src_arch, 128 * 1024 * 1024)
                        .expect("创建执行器失败");
                    
                    let test_code = create_test_code(src_arch);
                    let code_base = 0x1000;
                    
                    // 加载代码
                    for (i, byte) in test_code.iter().enumerate() {
                        black_box(executor.mmu_mut().write(
                            code_base + i as u64, 
                            *byte as u64, 
                            1
                        ).expect("写入内存失败"));
                    }
                    
                    // 执行翻译
                    for _ in 0..10 {
                        black_box(executor.execute(code_base).expect("执行失败"));
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// 基准测试：JIT编译性能
fn bench_jit_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_compilation");
    
    for instruction_count in [100, 500, 1000, 5000, 10000].iter() {
        group.throughput(Throughput::Elements(*instruction_count as u64));
        group.bench_with_input(
            BenchmarkId::new("compile_instructions", instruction_count),
            instruction_count,
            |b, &instruction_count| {
                let mut jit = JITEngine::new(JITConfig::default());
                let block = create_basic_ir_block(0x1000, instruction_count);
                
                b.iter(|| {
                    black_box(jit.compile(black_box(&block)).unwrap());
                });
            },
        );
    }
    
    // 测试不同优化级别
    for opt_level in [0, 1, 2, 3].iter() {
        group.bench_with_input(
            BenchmarkId::new("optimization_level", opt_level),
            opt_level,
            |b, &opt_level| {
                let mut config = JITConfig::default();
                config.optimization_level = *opt_level;
                let mut jit = JITEngine::new(config);
                let block = create_complex_ir_block(0x2000, 1000);
                
                b.iter(|| {
                    black_box(jit.compile(black_box(&block)).unwrap());
                });
            },
        );
    }
    
    // 测试SIMD优化
    group.bench_function("simd_optimization", |b| {
        let mut config = JITConfig::default();
        config.enable_simd = true;
        let mut jit = JITEngine::new(config);
        let block = create_simd_ir_block(0x3000, 1000);
        
        b.iter(|| {
            black_box(jit.compile(black_box(&block)).unwrap());
        });
    });
    
    group.finish();
}

/// 基准测试：内存管理性能
fn bench_memory_management(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_management");
    
    // 测试内存分配性能
    for allocation_size in [1024, 10240, 102400, 1024000].iter() {
        group.throughput(Throughput::Bytes(*allocation_size as u64));
        group.bench_with_input(
            BenchmarkId::new("allocate", allocation_size),
            allocation_size,
            |b, &allocation_size| {
                b.iter(|| {
                    let mut memory_manager = MemoryManager::new(1024 * 1024 * 1024); // 1GB
                    for i in 0..100 {
                        let addr = i as u64 * allocation_size;
                        black_box(memory_manager.allocate(addr, allocation_size).unwrap());
                    }
                });
            },
        );
    }
    
    // 测试内存读写性能
    for access_size in [1, 2, 4, 8, 16, 32, 64].iter() {
        group.throughput(Throughput::Bytes(*access_size as u64));
        group.bench_with_input(
            BenchmarkId::new("read_write", access_size),
            access_size,
            |b, &access_size| {
                let mut mmu = SoftMmu::new(1024 * 1024, false);
                let base_addr = 0x1000;
                
                b.iter(|| {
                    for i in 0..1000 {
                        let addr = base_addr + i as u64 * access_size;
                        // 写入
                        black_box(mmu.write(addr, 0xDEADBEEF, access_size).unwrap());
                        // 读取
                        black_box(mmu.read(addr, access_size).unwrap());
                    }
                });
            },
        );
    }
    
    // 测试批量内存操作
    for batch_size in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("bulk_operations", batch_size),
            batch_size,
            |b, &batch_size| {
                let mut mmu = SoftMmu::new(1024 * 1024, false);
                let base_addr = 0x1000;
                let mut addrs = Vec::new();
                let mut values = Vec::new();
                
                for i in 0..*batch_size {
                    addrs.push(base_addr + i as u64 * 8);
                    values.push(0xDEADBEEF + i as u64);
                }
                
                b.iter(|| {
                    // 批量写入
                    black_box(mmu.write_bulk(&addrs, &values).unwrap());
                    // 批量读取
                    let mut results = vec![0u64; *batch_size];
                    black_box(mmu.read_bulk(&addrs, &mut results).unwrap());
                });
            },
        );
    }
    
    group.finish();
}

/// 基准测试：并发执行性能
fn bench_concurrent_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_execution");
    
    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("threads", thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        let mut handles = Vec::new();
                        
                        for i in 0..thread_count {
                            let handle = tokio::spawn(async move {
                                let mut executor = UnifiedExecutor::auto_create(GuestArch::X86_64, 128 * 1024 * 1024)
                                    .expect("创建执行器失败");
                                
                                let test_code = create_test_code(GuestArch::X86_64);
                                let code_base = 0x1000 + i as u64 * 0x10000;
                                
                                // 加载代码
                                for (j, byte) in test_code.iter().enumerate() {
                                    executor.mmu_mut().write(
                                        code_base + j as u64, 
                                        *byte as u64, 
                                        1
                                    ).expect("写入内存失败");
                                }
                                
                                // 执行代码
                                for _ in 0..10 {
                                    executor.execute(code_base).expect("执行失败");
                                }
                            });
                            handles.push(handle);
                        }
                        
                        for handle in handles {
                            handle.await.unwrap();
                        }
                    });
                });
            },
        );
    }
    
    group.finish();
}

/// 基准测试：资源使用效率
fn bench_resource_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("resource_efficiency");
    
    // 测试代码缓存效率
    group.bench_function("code_cache_hit", |b| {
        let mut jit = JITEngine::new(JITConfig::default());
        let block = create_basic_ir_block(0x1000, 1000);
        
        // 预先编译以填充缓存
        let _ = jit.compile(&block).unwrap();
        
        b.iter(|| {
            black_box(jit.compile(black_box(&block)).unwrap());
        });
    });
    
    group.bench_function("code_cache_miss", |b| {
        let mut jit = JITEngine::new(JITConfig::default());
        
        b.iter(|| {
            let addr = 0x1000 + (rand::random::<u64>() % 10000);
            let block = create_basic_ir_block(addr, 1000);
            black_box(jit.compile(black_box(&block)).unwrap());
        });
    });
    
    // 测试内存池效率
    group.bench_function("memory_pool", |b| {
        let mut memory_manager = MemoryManager::new(1024 * 1024 * 1024); // 1GB
        
        b.iter(|| {
            // 分配和释放内存
            for i in 0..100 {
                let addr = i as u64 * 4096;
                let _ = memory_manager.allocate(addr, 4096).unwrap();
                let _ = memory_manager.deallocate(addr).unwrap();
            }
        });
    });
    
    group.finish();
}

/// 基准测试：热点检测性能
fn bench_hotspot_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("hotspot_detection");
    
    // 测试热点检测开销
    group.bench_function("with_hotspot_detection", |b| {
        let mut config = JITConfig::default();
        config.enable_hotspot_detection = true;
        let mut jit = JITEngine::new(config);
        let block = create_basic_ir_block(0x1000, 1000);
        
        b.iter(|| {
            // 模拟多次执行以触发热点检测
            for _ in 0..150 {
                black_box(jit.compile(black_box(&block)).unwrap());
            }
        });
    });
    
    // 测试无热点检测
    group.bench_function("without_hotspot_detection", |b| {
        let mut config = JITConfig::default();
        config.enable_hotspot_detection = false;
        let mut jit = JITEngine::new(config);
        let block = create_basic_ir_block(0x1000, 1000);
        
        b.iter(|| {
            // 相同的执行次数，但无热点检测
            for _ in 0..150 {
                black_box(jit.compile(black_box(&block)).unwrap());
            }
        });
    });
    
    group.finish();
}

/// 创建基础IR块
fn create_basic_ir_block(addr: u64, instruction_count: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    
    for i in 0..instruction_count {
        builder.push(IROp::MovImm { dst: (i % 16) as u32, imm: (i * 42) as u64 });
        builder.push(IROp::Add {
            dst: 0,
            src1: 0,
            src2: (i % 16) as u32,
        });
    }
    
    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 创建复杂IR块
fn create_complex_ir_block(addr: u64, complexity: usize) -> IRBlock {
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
                builder.push(IROp::ShiftLeft {
                    dst: 6,
                    src: 0,
                    amount: 2,
                });
            }
            _ => {
                builder.push(IROp::ShiftRight {
                    dst: 7,
                    src: 6,
                    amount: 1,
                    signed: false,
                });
            }
        }
    }
    
    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 创建SIMD友好IR块
fn create_simd_ir_block(addr: u64, vector_length: usize) -> IRBlock {
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
    
    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 创建测试代码
fn create_test_code(arch: GuestArch) -> Vec<u8> {
    match arch {
        GuestArch::X86_64 => create_test_code_x86(),
        GuestArch::ARM64 => create_test_code_arm(),
        GuestArch::RISCV64 => create_test_code_riscv(),
    }
}

/// 创建x86测试代码
fn create_test_code_x86() -> Vec<u8> {
    vec![
        0xB8, 0x0A, 0x00, 0x00, 0x00,  // mov eax, 10
        0xBB, 0x14, 0x00, 0x00, 0x00,  // mov ebx, 20
        0x01, 0xD8,                     // add eax, ebx
        0x83, 0xC0, 0x05,               // add eax, 5
        0x29, 0xD8,                     // sub eax, ebx
        0xC3,                           // ret
    ]
}

/// 创建ARM测试代码
fn create_test_code_arm() -> Vec<u8> {
    vec![
        0x10, 0x00, 0x80, 0x52,  // mov w16, #10
        0x14, 0x00, 0x80, 0x52,  // mov w20, #20
        0x10, 0x04, 0x14, 0x8B,  // add w16, w16, w20
        0x50, 0x00, 0x80, 0x52,  // mov w16, #5
        0x10, 0x04, 0x10, 0x8B,  // add w16, w16, w16
        0x10, 0x04, 0x14, 0xCB,  // sub w16, w16, w20
        0xC0, 0x03, 0x5F, 0xD6,  // ret
    ]
}

/// 创建RISC-V测试代码
fn create_test_code_riscv() -> Vec<u8> {
    vec![
        0x0A, 0x00, 0x00, 0x93,  // addi x19, x0, 10
        0x14, 0x00, 0x00, 0x13,  // addi x2, x0, 20
        0x13, 0x04, 0x02, 0x13,  // addi x19, x19, 2
        0x93, 0x0A, 0x00, 0x00,  // addi x19, x0, 10
        0x93, 0x04, 0x02, 0x13,  // addi x19, x19, 2
        0x33, 0x04, 0x12, 0x41,  // sub x19, x19, x2
        0x67, 0x80, 0x00, 0x00,  // jalr x0, 0(x1)
    ]
}

criterion_group!(
    benches,
    bench_cross_arch_translation,
    bench_jit_compilation,
    bench_memory_management,
    bench_concurrent_execution,
    bench_resource_efficiency,
    bench_hotspot_detection
);
criterion_main!(benches);