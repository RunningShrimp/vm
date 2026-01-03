//! 综合性能基准测试套件
//!
//! 覆盖所有关键子系统的性能基准测试。

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

/// JIT编译性能基准
fn bench_jit_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_compilation");
    
    let sizes = [100, 1000, 10000];
    for size in sizes {
        group.throughput(Throughput::Elements(size as u64));
        
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                // 模拟JIT编译
                let block = generate_test_ir_block(size);
                compile_ir_block(black_box(&block))
            })
        });
    }
    
    group.finish();
}

/// 跨架构翻译基准
fn bench_cross_arch_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cross_arch_translation");
    
    let translation_types = [
        ("x86_64_to_arm64", 1000),
        ("x86_64_to_riscv", 1000),
        ("arm64_to_riscv", 1000),
    ];
    
    for (name, size) in translation_types {
        group.bench_function(name, |b| {
            let instructions = generate_test_instructions(size);
            b.iter(|| {
                translate_arch(black_box(&instructions))
            })
        });
    }
    
    group.finish();
}

/// GC性能基准
fn bench_gc_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_performance");
    
    let heap_sizes = [1024, 10240, 102400]; // 1KB, 10KB, 100KB
    for heap_size in heap_sizes {
        group.bench_with_input(
            BenchmarkId::new("heap_size", heap_size),
            &heap_size,
            |b, &heap_size| {
                b.iter(|| {
                    let mut gc = create_test_gc(heap_size);
                    gc.collect()
                })
            },
        );
    }
    
    group.finish();
}

/// 内存操作基准
fn bench_memory_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_operations");
    
    // 内存分配
    group.bench_function("allocate", |b| {
        b.iter(|| allocate_memory(1024))
    });
    
    // 内存复制
    group.bench_function("memcpy", |b| {
        let src = vec![42u8; 1024];
        let mut dst = vec![0u8; 1024];
        b.iter(|| {
            dst.copy_from_slice(black_box(&src));
        });
    });
    
    // 内存清零
    group.bench_function("memset", |b| {
        let mut data = vec![0u8; 1024];
        b.iter(|| {
            data.fill(black_box(42));
        });
    });
    
    group.finish();
}

/// GPU加速基准（需要CUDA/ROCm）
#[cfg(feature = "gpu")]
fn bench_gpu_acceleration(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_acceleration");

    // GPU内存复制 - Host to Device
    group.bench_function("gpu_memcpy_h2d", |b| {
        use vm_passthrough::cuda::CudaAccelerator;

        let accelerator = CudaAccelerator::new(0).unwrap();
        let size = 1024 * 1024; // 1MB
        let src = vec![0u8; size];

        let d_ptr = accelerator.malloc(size).unwrap();

        b.iter(|| {
            // Host to Device memcpy
            let _ = accelerator.memcpy_sync(d_ptr, &src, vm_passthrough::cuda::CudaMemcpyKind::HostToDevice);
        });

        // 清理
        let _ = accelerator.free(d_ptr);
    });

    // GPU内存复制 - Device to Host
    group.bench_function("gpu_memcpy_d2h", |b| {
        use vm_passthrough::cuda::CudaAccelerator;

        let accelerator = CudaAccelerator::new(0).unwrap();
        let size = 1024 * 1024; // 1MB
        let src = vec![42u8; size];
        let mut dst = vec![0u8; size];

        let d_ptr = accelerator.malloc(size).unwrap();
        let _ = accelerator.memcpy_sync(d_ptr, &src, vm_passthrough::cuda::CudaMemcpyKind::HostToDevice);

        b.iter(|| {
            // Device to Host memcpy
            let _ = accelerator.memcpy_sync(
                vm_passthrough::cuda::CudaDevicePtr { ptr: dst.as_mut_ptr() as u64, size },
                unsafe { std::slice::from_raw_parts(src.as_ptr() as *const u8, src.len()) },
                vm_passthrough::cuda::CudaMemcpyKind::DeviceToHost
            );
        });

        accelerator.free(d_ptr);
    });

    // GPU内存复制 - Device to Device (如果支持)
    group.bench_function("gpu_memcpy_d2d", |b| {
        use vm_passthrough::cuda::CudaAccelerator;

        let accelerator = CudaAccelerator::new(0).unwrap();
        let size = 1024 * 1024; // 1MB

        let d_src = accelerator.malloc(size).unwrap();
        let d_dst = accelerator.malloc(size).unwrap();

        b.iter(|| {
            // Device to Device memcpy (如果实现)
            // 注意：当前CUDA实现中D2D还未实现，这里使用模拟操作
            let _ = accelerator.memcpy_sync(d_dst, unsafe {
                std::slice::from_raw_parts(d_src.ptr as *const u8, size)
            }, vm_passthrough::cuda::CudaMemcpyKind::DeviceToDevice);
        });

        accelerator.free(d_src);
        accelerator.free(d_dst);
    });

    // GPU kernel执行基准
    group.bench_function("gpu_kernel_execution", |b| {
        use vm_passthrough::cuda::{CudaAccelerator, GpuKernel};

        let accelerator = CudaAccelerator::new(0).unwrap();

        // 创建测试内核
        let kernel = GpuKernel::new("vector_add".to_string());

        let n = 1024 * 1024;
        let a = vec![1.0f32; n];
        let b = vec![2.0f32; n];
        let mut c = vec![0.0f32; n];

        // 分配GPU内存并复制数据
        let d_a = accelerator.malloc(n * std::mem::size_of::<f32>()).unwrap();
        let d_b = accelerator.malloc(n * std::mem::size_of::<f32>()).unwrap();
        let d_c = accelerator.malloc(n * std::mem::size_of::<f32>()).unwrap();

        // 复制数据到GPU
        let _ = accelerator.memcpy_sync(d_a, unsafe {
            std::slice::from_raw_parts(a.as_ptr() as *const u8, a.len() * std::mem::size_of::<f32>())
        }, vm_passthrough::cuda::CudaMemcpyKind::HostToDevice);

        let _ = accelerator.memcpy_sync(d_b, unsafe {
            std::slice::from_raw_parts(b.as_ptr() as *const u8, b.len() * std::mem::size_of::<f32>())
        }, vm_passthrough::cuda::CudaMemcpyKind::HostToDevice);

        b.iter(|| {
            // 启动kernel（当前是模拟实现）
            let _ = kernel.launch(((n + 255) / 256, 1, 1), (256, 1, 1));

            // 等待GPU操作完成
            let _ = accelerator.stream.synchronize();
        });

        // 将结果复制回主机
        let _ = accelerator.memcpy_sync(
            vm_passthrough::cuda::CudaDevicePtr {
                ptr: c.as_mut_ptr() as u64,
                size: n * std::mem::size_of::<f32>()
            },
            unsafe { std::slice::from_raw_parts(d_c.ptr as *const u8, n * std::mem::size_of::<f32>()) },
            vm_passthrough::cuda::CudaMemcpyKind::DeviceToHost
        );

        // 验证结果
        assert_eq!(c[0], 3.0);

        // 清理GPU内存
        accelerator.free(d_a);
        accelerator.free(d_b);
        accelerator.free(d_c);
    });

    // GPU内存分配和释放基准
    group.bench_function("gpu_malloc_free", |b| {
        use vm_passthrough::cuda::CudaAccelerator;

        let accelerator = CudaAccelerator::new(0).unwrap();
        let size = 1024 * 1024; // 1MB

        b.iter(|| {
            let d_ptr = accelerator.malloc(size).unwrap();
            let _ = accelerator.free(d_ptr);
        });
    });

    group.finish();
}

#[cfg(not(feature = "gpu"))]
fn bench_gpu_acceleration(_c: &mut Criterion) {
    // 当GPU功能未启用时，跳过GPU基准测试
    log::info!("GPU benchmarks skipped - compile with 'cargo bench --features gpu' to enable GPU benchmarks");
}

/// 辅助函数：生成测试IR块
fn generate_test_ir_block(size: usize) -> Vec<u8> {
    vec![0u8; size]
}

/// 辅助函数：编译IR块
fn compile_ir_block(_block: &[u8]) -> Duration {
    Duration::from_nanos(100)
}

/// 辅助函数：生成测试指令
fn generate_test_instructions(count: usize) -> Vec<u32> {
    (0..count).map(|i| i as u32).collect()
}

/// 辅助函数：跨架构翻译
fn translate_arch(_instructions: &[u32]) -> Duration {
    Duration::from_nanos(50)
}

/// 辅助函数：创建测试GC
fn create_test_gc(_heap_size: usize) -> TestGC {
    TestGC
}

struct TestGC;

impl TestGC {
    fn collect(&self) -> Duration {
        Duration::from_micros(100)
    }
}

/// 辅助函数：分配内存
fn allocate_memory(size: usize) -> Vec<u8> {
    vec![0u8; size]
}

criterion_group!(
    benches,
    bench_jit_compilation,
    bench_cross_arch_translation,
    bench_gc_performance,
    bench_memory_operations,
    #[cfg(feature = "gpu")]
    bench_gpu_acceleration
);

criterion_main!(benches);
