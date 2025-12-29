//! 块级缓存性能基准测试
//!
//! 测试增强块级缓存的性能

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vm_cross_arch::block_cache::{SourceBlockKey, TranslatedBlock};
use vm_cross_arch::enhanced_block_cache::EnhancedBlockCache;
use vm_cross_arch::translation_impl::TranslationStats;
use vm_ir::IRBlock;
use vm_cross_arch::{SourceArch, TargetArch};

fn bench_block_cache_lookup_hit(c: &mut Criterion) {
    let cache = EnhancedBlockCache::with_default_config();

    // 创建一个测试块
    let key = SourceBlockKey::new(
        SourceArch::X86_64,
        TargetArch::ARM64,
        0x1000,
        &IRBlock {
            ops: vec![],
            term: vm_ir::Terminator::Ret,
        },
    );

    let block = TranslatedBlock {
        instructions: vec![],
        stats: TranslationStats::default(),
    };

    cache.insert(key.clone(), block);

    c.bench_function("block_cache_lookup_hit", |b| {
        b.iter(|| {
            black_box(cache.lookup(&key));
        });
    });
}

fn bench_block_cache_lookup_miss(c: &mut Criterion) {
    let cache = EnhancedBlockCache::with_default_config();

    c.bench_function("block_cache_lookup_miss", |b| {
        b.iter(|| {
            // 每次使用不同的地址，确保未命中
            let addr = fastrand::u64(..);
            let key = SourceBlockKey::new(
                SourceArch::X86_64,
                TargetArch::ARM64,
                addr,
                &IRBlock {
                    ops: vec![],
                    term: vm_ir::Terminator::Ret,
                },
            );
            black_box(cache.lookup(&key));
        });
    });
}

fn bench_hot_block_detection(c: &mut Criterion) {
    let cache = EnhancedBlockCache::with_default_config(); // hot_threshold=100

    // 创建一个热块（访问150次）
    let key = SourceBlockKey::new(
        SourceArch::X86_64,
        TargetArch::ARM64,
        0x1000,
        &IRBlock {
            ops: vec![],
            term: vm_ir::Terminator::Ret,
        },
    );

    let block = TranslatedBlock {
        instructions: vec![],
        stats: TranslationStats::default(),
    };

    cache.insert(key.clone(), block);

    // 预热：访问150次
    for _ in 0..150 {
        cache.lookup(&key);
    }

    c.bench_function("hot_block_detection", |b| {
        b.iter(|| {
            black_box(cache.get_hot_blocks());
        });
    });
}

fn bench_cache_stats(c: &mut Criterion) {
    let cache = EnhancedBlockCache::with_default_config();

    // 预热一些块
    for i in 0..100 {
        let key = SourceBlockKey::new(
            SourceArch::X86_64,
            TargetArch::ARM64,
            i * 0x1000,
            &IRBlock {
                ops: vec![],
                term: vm_ir::Terminator::Ret,
            },
        );

        let block = TranslatedBlock {
            instructions: vec![],
            stats: TranslationStats::default(),
        };

        cache.insert(key, block);

        // 部分块访问多次以创建热块
        if i < 20 {
            for _ in 0..150 {
                cache.lookup(&key);
            }
        }
    }

    c.bench_function("get_enhanced_stats", |b| {
        b.iter(|| {
            black_box(cache.get_enhanced_stats());
        });
    });
}

fn bench_warm_up(c: &mut Criterion) {
    c.bench_function("warm_up_100_blocks", |b| {
        b.iter(|| {
            let cache = EnhancedBlockCache::with_default_config();

            // 准备100个块
            let mut blocks = Vec::new();
            for i in 0..100 {
                let key = SourceBlockKey::new(
                    SourceArch::X86_64,
                    TargetArch::ARM64,
                    i * 0x1000,
                    &IRBlock {
                        ops: vec![],
                        term: vm_ir::Terminator::Ret,
                    },
                );

                let block = TranslatedBlock {
                    instructions: vec![],
                    stats: TranslationStats::default(),
                };

                blocks.push((key, block));
            }

            // 预热
            black_box(cache.warm_up(blocks));
        });
    });
}

criterion_group!(benches, bench_block_cache_lookup_hit, bench_block_cache_lookup_miss, bench_hot_block_detection, bench_cache_stats, bench_warm_up);
criterion_main!(benches);
