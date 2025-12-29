use criterion::{criterion_group, criterion_main, Criterion};
use vm_engine::jit::unified_cache::ShardedCache;

pub fn adaptive_sharding_bench(c: &mut Criterion) {
    c.bench_function("sharded_insert_10k", |b| {
        b.iter(|| {
            let cache = ShardedCache::new(4);
            for i in 0..10_000u64 {
                let entry = vm_engine::jit::unified_cache::CacheEntry::new(
                    vm_engine::jit::CodePtr((i * 1024) as *const u8),
                    1024,
                );
                cache.insert(i, entry);
            }
        });
    });

    c.bench_function("sharded_lookup_10k", |b| {
        let cache = ShardedCache::new(8);
        for i in 0..10_000u64 {
            let entry = vm_engine::jit::unified_cache::CacheEntry::new(
                vm_engine::jit::CodePtr((i * 1024) as *const u8),
                1024,
            );
            cache.insert(i, entry);
        }

        b.iter(|| {
            for i in 0..10_000u64 {
                let _ = cache.get(i);
            }
        });
    });
}

criterion_group!(benches, adaptive_sharding_bench);
criterion_main!(benches);
