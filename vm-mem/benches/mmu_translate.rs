use criterion::{criterion_group, criterion_main, Criterion, black_box, BenchmarkId, Throughput};
use vm_mem::SoftMmu;
use vm_core::{MMU, AccessType};

/// 基准测试: Bare 模式地址翻译 (无 TLB 查找)
fn bench_translate_bare(c: &mut Criterion) {
    let mut mmu = SoftMmu::new(64 * 1024 * 1024, false);
    
    c.bench_function("translate_bare", |b| {
        b.iter(|| {
            let va = black_box(0x1000_0000);
            let _ = mmu.translate(va, AccessType::Read).unwrap();
        })
    });
}

/// 基准测试: 内存读取性能
fn bench_memory_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_read");
    let mut mmu = SoftMmu::new(64 * 1024 * 1024, false);
    
    // 初始化测试数据
    for i in 0..1000 {
        let _ = mmu.write(i * 8, 0xDEADBEEF_u64, 8);
    }
    
    for size in [1u8, 2, 4, 8].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let addr = black_box(0x1000);
                mmu.read(addr, size).unwrap()
            })
        });
    }
    group.finish();
}

/// 基准测试: 内存写入性能
fn bench_memory_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_write");
    let mut mmu = SoftMmu::new(64 * 1024 * 1024, false);
    
    for size in [1u8, 2, 4, 8].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let addr = black_box(0x1000);
                let val = black_box(0xDEADBEEF_u64);
                mmu.write(addr, val, size).unwrap()
            })
        });
    }
    group.finish();
}

/// 基准测试: 顺序内存访问
fn bench_sequential_access(c: &mut Criterion) {
    let mut mmu = SoftMmu::new(64 * 1024 * 1024, false);
    
    c.bench_function("sequential_read_1k", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let addr = black_box(i * 8);
                let _ = mmu.read(addr, 8);
            }
        })
    });
}

/// 基准测试: 随机内存访问
fn bench_random_access(c: &mut Criterion) {
    let mut mmu = SoftMmu::new(64 * 1024 * 1024, false);
    
    // 预生成随机地址 (简单的 LCG 伪随机)
    let mut addresses: Vec<u64> = Vec::with_capacity(1000);
    let mut seed: u64 = 12345;
    for _ in 0..1000 {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        addresses.push((seed % (64 * 1024 * 1024)) & !7); // 8 字节对齐
    }
    
    c.bench_function("random_read_1k", |b| {
        b.iter(|| {
            for &addr in &addresses {
                let _ = mmu.read(addr, 8);
            }
        })
    });
}

/// 基准测试: TLB 命中率影响
fn bench_tlb_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_performance");
    
    // 测试不同数量的唯一页面访问
    for num_pages in [1, 10, 64, 128, 256].iter() {
        let mut mmu = SoftMmu::new(64 * 1024 * 1024, false);
        let page_size = 4096u64;
        
        group.bench_with_input(
            BenchmarkId::new("pages", num_pages),
            num_pages,
            |b, &num_pages| {
                b.iter(|| {
                    for i in 0..*num_pages {
                        let addr = black_box(i as u64 * page_size);
                        let _ = mmu.translate(addr, AccessType::Read);
                    }
                })
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_translate_bare,
    bench_memory_read,
    bench_memory_write,
    bench_sequential_access,
    bench_random_access,
    bench_tlb_performance
);
criterion_main!(benches);
