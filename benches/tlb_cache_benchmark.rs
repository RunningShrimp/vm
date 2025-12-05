/// Week 4 - TLB 缓存性能基准测试
///
/// 测试异步 TLB 缓存的性能指标

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vm_core::{AsyncTLBCache, AccessType};

fn bench_tlb_lookup(c: &mut Criterion) {
    c.bench_function("tlb_single_lookup", |b| {
        let cache = AsyncTLBCache::new(1024);
        cache.insert(0x1000, 0x2000, AccessType::Read);
        
        b.iter(|| {
            let _ = cache.lookup(black_box(0x1000));
        });
    });
}

fn bench_tlb_insert(c: &mut Criterion) {
    c.bench_function("tlb_single_insert", |b| {
        let cache = AsyncTLBCache::new(1024);
        let mut addr = 0x1000u64;
        
        b.iter(|| {
            cache.insert(black_box(addr), 0x2000, AccessType::Read);
            addr += 0x1000;
        });
    });
}

fn bench_tlb_miss_rate(c: &mut Criterion) {
    c.bench_function("tlb_miss_rate_1k_lookups", |b| {
        let cache = AsyncTLBCache::new(256);
        
        // 插入 10 个表项
        for i in 0..10 {
            cache.insert(0x1000 + (i as u64 * 0x1000), 0x2000, AccessType::Read);
        }
        
        b.iter(|| {
            for i in 0..1000 {
                let addr = black_box(0x1000 + ((i % 20) as u64 * 0x1000));
                let _ = cache.lookup(addr);
            }
        });
    });
}

fn bench_tlb_hit_rate(c: &mut Criterion) {
    c.bench_function("tlb_hit_rate_working_set", |b| {
        let cache = AsyncTLBCache::new(256);
        
        // 填充 TLB，模拟工作集
        for i in 0..200 {
            cache.insert(0x1000 + (i as u64 * 0x1000), 0x2000, AccessType::Read);
        }
        
        b.iter(|| {
            // 访问已缓存的地址
            for i in 0..100 {
                let addr = black_box(0x1000 + ((i % 200) as u64 * 0x1000));
                let _ = cache.lookup(addr);
            }
        });
    });
}

fn bench_tlb_flush_operation(c: &mut Criterion) {
    c.bench_function("tlb_selective_flush_100", |b| {
        let cache = AsyncTLBCache::new(256);
        
        // 插入 200 个表项
        for i in 0..200 {
            cache.insert(0x1000 + (i as u64 * 0x1000), 0x2000, AccessType::Read);
        }
        
        let mut counter = 0u64;
        b.iter(|| {
            counter += 1;
            let predicate = move |va: &u64| (*va & 0x100) == 0;
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let _ = cache.selective_flush(predicate).await;
            });
        });
    });
}

fn bench_concurrent_tlb_access(c: &mut Criterion) {
    c.bench_function("concurrent_tlb_access_4threads", |b| {
        let cache = std::sync::Arc::new(AsyncTLBCache::new(1024));
        
        // 预填充 TLB
        for i in 0..100 {
            cache.insert(0x1000 + (i as u64 * 0x1000), 0x2000, AccessType::Read);
        }
        
        b.iter(|| {
            let mut handles = vec![];
            
            for thread_id in 0..4 {
                let cache_clone = cache.clone();
                let handle = std::thread::spawn(move || {
                    for i in 0..250 {
                        let addr = 0x1000 + ((thread_id * 25 + i) as u64 * 0x1000);
                        let _ = cache_clone.lookup(black_box(addr));
                    }
                });
                handles.push(handle);
            }
            
            for handle in handles {
                handle.join().unwrap();
            }
        });
    });
}

fn bench_occupancy_tracking(c: &mut Criterion) {
    c.bench_function("occupancy_calculation", |b| {
        let cache = AsyncTLBCache::new(256);
        
        // 填充一些表项
        for i in 0..128 {
            cache.insert(0x1000 + (i as u64 * 0x1000), 0x2000, AccessType::Read);
        }
        
        b.iter(|| {
            let _ = black_box(cache.get_occupancy());
        });
    });
}

fn bench_prefetch_operation(c: &mut Criterion) {
    c.bench_function("async_prefetch_100_addrs", |b| {
        let cache = AsyncTLBCache::new(512);
        let addresses: Vec<_> = (0..100).map(|i| 0x1000 + (i as u64 * 0x1000)).collect();
        
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let _ = cache.async_prefetch(&addresses).await;
            });
    });
}

criterion_group!(
    benches,
    bench_tlb_lookup,
    bench_tlb_insert,
    bench_tlb_miss_rate,
    bench_tlb_hit_rate,
    bench_tlb_flush_operation,
    bench_concurrent_tlb_access,
    bench_occupancy_tracking,
    bench_prefetch_operation,
);

criterion_main!(benches);
