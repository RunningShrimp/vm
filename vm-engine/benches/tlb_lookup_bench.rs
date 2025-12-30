//! TLB查找性能基准测试
//!
//! 测试虚拟内存管理中TLB（Translation Lookaside Buffer）的性能

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use rand::Rng;
use std::collections::HashMap;
use std::time::Duration;

/// 模拟TLB条目
#[derive(Debug, Clone)]
struct TLBEntry {
    virtual_addr: u64,
    physical_addr: u64,
    access_count: u64,
    last_access: std::time::Instant,
}

/// 模拟TLB缓存
struct TLBCache {
    entries: HashMap<u64, TLBEntry>,
    max_size: usize,
    hits: u64,
    misses: u64,
}

impl TLBCache {
    fn new(max_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_size,
            hits: 0,
            misses: 0,
        }
    }

    /// TLB查找（无缓存）
    fn lookup_no_cache(&mut self, virtual_addr: u64) -> Option<u64> {
        self.misses += 1;
        // 模拟页表查找
        Some(virtual_addr + 0x1000_0000)
    }

    /// TLB查找（有缓存）
    fn lookup_with_cache(&mut self, virtual_addr: u64) -> Option<u64> {
        if let Some(entry) = self.entries.get_mut(&virtual_addr) {
            self.hits += 1;
            entry.access_count += 1;
            entry.last_access = std::time::Instant::now();
            Some(entry.physical_addr)
        } else {
            self.misses += 1;
            // 缓存未命中，插入新条目
            if self.entries.len() >= self.max_size {
                // 简单的FIFO替换策略
                let key_to_remove = self.entries.keys().next().copied()?;
                self.entries.remove(&key_to_remove);
            }

            let physical_addr = virtual_addr + 0x1000_0000;
            self.entries.insert(
                virtual_addr,
                TLBEntry {
                    virtual_addr,
                    physical_addr,
                    access_count: 1,
                    last_access: std::time::Instant::now(),
                },
            );

            Some(physical_addr)
        }
    }

    /// 批量TLB查找
    fn batch_lookup(&mut self, addrs: &[u64]) -> Vec<Option<u64>> {
        addrs
            .iter()
            .map(|&addr| self.lookup_with_cache(addr))
            .collect()
    }

    /// 清空TLB
    fn flush(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    /// 获取命中率
    fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64
    }
}

/// TLB基准测试：无缓存 vs 有缓存
fn bench_tlb_cache_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_cache_comparison");

    // 无缓存的TLB查找
    group.bench_function("no_cache", |b| {
        let mut tlb = TLBCache::new(0);

        b.iter(|| {
            for addr in 0..1000 {
                black_box(tlb.lookup_no_cache(black_box(addr)));
            }
        });
    });

    // 有缓存的TLB查找
    group.bench_function("with_cache", |b| {
        let mut tlb = TLBCache::new(256);

        // 预热缓存
        for addr in 0..256 {
            tlb.lookup_with_cache(addr);
        }

        b.iter(|| {
            for addr in 0..256 {
                black_box(tlb.lookup_with_cache(black_box(addr)));
            }
        });
    });

    group.finish();
}

/// TLB不同大小缓存的性能测试
fn bench_tlb_cache_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_cache_sizes");

    for cache_size in [16, 32, 64, 128, 256, 512, 1024].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(cache_size),
            cache_size,
            |b, &size| {
                let mut tlb = TLBCache::new(size);

                // 预热缓存
                for addr in 0..size as u64 {
                    tlb.lookup_with_cache(addr);
                }

                b.iter(|| {
                    for addr in 0..size as u64 {
                        black_box(tlb.lookup_with_cache(black_box(addr)));
                    }
                });
            },
        );
    }

    group.finish();
}

/// TLB访问模式性能测试
fn bench_tlb_access_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_access_patterns");

    let cache_size = 256;

    // 顺序访问（空间局部性好）
    group.bench_function("sequential_access", |b| {
        let mut tlb = TLBCache::new(cache_size);

        b.iter(|| {
            for addr in 0..1000 {
                black_box(tlb.lookup_with_cache(black_box(addr)));
            }
        });
    });

    // 随机访问（无局部性）
    group.bench_function("random_access", |b| {
        let mut tlb = TLBCache::new(cache_size);
        let mut rng = rand::thread_rng();
        let random_addrs: Vec<u64> = (0..1000).map(|_| rng.gen_range(0..10000)).collect();

        b.iter(|| {
            for &addr in &random_addrs {
                black_box(tlb.lookup_with_cache(black_box(addr)));
            }
        });
    });

    // 循环访问（时间局部性好）
    group.bench_function("repeated_access", |b| {
        let mut tlb = TLBCache::new(cache_size);
        let loop_addrs: Vec<u64> = (0..100).collect();

        b.iter(|| {
            for _ in 0..10 {
                for &addr in &loop_addrs {
                    black_box(tlb.lookup_with_cache(black_box(addr)));
                }
            }
        });
    });

    group.finish();
}

/// TLB批量查找性能测试
fn bench_tlb_batch_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_batch");

    for batch_size in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &size| {
                let mut tlb = TLBCache::new(256);
                let addrs: Vec<u64> = (0..size).map(|i| i * 4).collect();

                b.iter(|| {
                    black_box(tlb.batch_lookup(black_box(&addrs)));
                });
            },
        );
    }

    group.finish();
}

/// TLB刷新开销测试
fn bench_tlb_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_flush");

    group.bench_function("flush_empty_cache", |b| {
        let mut tlb = TLBCache::new(256);

        b.iter(|| {
            black_box(tlb.flush());
        });
    });

    group.bench_function("flush_full_cache", |b| {
        let mut tlb = TLBCache::new(256);

        // 填满缓存
        for addr in 0..256 {
            tlb.lookup_with_cache(addr);
        }

        b.iter(|| {
            black_box(tlb.flush());
        });
    });

    group.finish();
}

/// TLB竞争条件性能测试（模拟多线程）
fn bench_tlb_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_contention");

    // 单线程性能
    group.bench_function("single_thread", |b| {
        let mut tlb = TLBCache::new(256);

        b.iter(|| {
            for addr in 0..1000 {
                black_box(tlb.lookup_with_cache(black_box(addr)));
            }
        });
    });

    // 模拟多线程竞争（通过快速切换地址空间）
    group.bench_function("address_space_switch", |b| {
        let mut tlb1 = TLBCache::new(128);
        let mut tlb2 = TLBCache::new(128);

        b.iter(|| {
            for addr in 0..500 {
                black_box(tlb1.lookup_with_cache(black_box(addr)));
                black_box(tlb2.lookup_with_cache(black_box(addr + 0x1000)));
            }
        });
    });

    group.finish();
}

/// TLB替换策略性能测试
fn bench_tlb_replacement_policies(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_replacement");

    let cache_size = 256;
    let test_addrs: Vec<u64> = (0..1000).map(|i| i * 4).collect();

    // FIFO策略（默认）
    group.bench_function("fifo", |b| {
        let mut tlb = TLBCache::new(cache_size);

        b.iter(|| {
            for &addr in &test_addrs {
                black_box(tlb.lookup_with_cache(black_box(addr)));
            }
        });
    });

    // 模拟LRU策略（通过重新排序访问时间）
    group.bench_function("lru_simulation", |b| {
        let mut tlb = TLBCache::new(cache_size);

        // 使用循环访问模式来模拟LRU的优势
        let loop_addrs: Vec<u64> = (0..cache_size as u64).collect();

        b.iter(|| {
            for &addr in &loop_addrs {
                black_box(tlb.lookup_with_cache(black_box(addr)));
            }
        });
    });

    group.finish();
}

/// TLB预取性能测试
fn bench_tlb_prefetching(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_prefetch");

    // 无预取
    group.bench_function("no_prefetch", |b| {
        let mut tlb = TLBCache::new(256);

        b.iter(|| {
            for addr in (0..1000).step_by(16) {
                black_box(tlb.lookup_with_cache(black_box(addr)));
            }
        });
    });

    // 模拟预取（提前访问下一个可能的地址）
    group.bench_function("with_prefetch", |b| {
        let mut tlb = TLBCache::new(256);

        b.iter(|| {
            for addr in (0..1000).step_by(16) {
                // 预取下一个可能的地址
                if addr + 16 < 1000 {
                    black_box(tlb.lookup_with_cache(addr + 16));
                }
                black_box(tlb.lookup_with_cache(black_box(addr)));
            }
        });
    });

    group.finish();
}

/// 配置基准测试
fn configure_criterion() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5))
        .sample_size(100)
}

criterion_group! {
    name = tlb_benches;
    config = configure_criterion();
    targets =
        bench_tlb_cache_comparison,
        bench_tlb_cache_sizes,
        bench_tlb_access_patterns,
        bench_tlb_batch_lookup,
        bench_tlb_flush,
        bench_tlb_contention,
        bench_tlb_replacement_policies,
        bench_tlb_prefetching
}

criterion_main!(tlb_benches);
