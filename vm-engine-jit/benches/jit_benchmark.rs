use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use vm_engine_jit::{
    TieredJITCompiler,
    tiered_compiler::TieredCompilerConfig,
    inline_cache::InlineCache,
    code_cache::LRUCache,
};
use vm_ir::{IRBlock, IROp};
use vm_core::GuestAddr;

/// 基准测试配置
pub fn benchmark_config() -> Criterion {
    Criterion::default()
        .sample_size(100)
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(5))
}

/// 创建测试 IR 块
fn create_test_block(size: usize) -> IRBlock {
    let mut ops = Vec::with_capacity(size);
    for i in 0..size {
        ops.push(IROp::MovImm {
            rd: (i % 32) as u8,
            imm: (i as u64) % 1000,
        });
        ops.push(IROp::Add {
            rd: (i % 32) as u8,
            rs1: (i % 32) as u8,
            rs2: ((i + 1) % 32) as u8,
        });
    }
    IRBlock {
        start_pc: GuestAddr(0x1000),
        ops,
        term: vm_ir::Terminator::Return,
    }
}

/// 分层编译器性能基准测试
fn bench_tiered_compiler(c: &mut Criterion) {
    let config = TieredCompilerConfig::default();
    let mut compiler = TieredJITCompiler::new(config);
    
    let mut group = c.benchmark_group("tiered_compiler");
    
    for size in [10, 50, 100, 500, 1000].iter() {
        let block = create_test_block(*size);
        
        group.bench_with_input(BenchmarkId::new("compile", size), size, |b, _| {
            b.iter(|| {
                let _ = compiler.execute(black_box(&block));
            });
        });
        
        group.bench_with_input(BenchmarkId::new("execute_baseline", size), size, |b, _| {
            let _ = compiler.execute(&block);
            b.iter(|| {
                let _ = compiler.execute(black_box(&block));
            });
        });
        
        group.bench_with_input(BenchmarkId::new("execute_optimized", size), size, |b, _| {
            for _ in 0..150 {
                let _ = compiler.execute(&block);
            }
            b.iter(|| {
                let _ = compiler.execute(black_box(&block));
            });
        });
    }
    
    group.finish();
}

/// 内联缓存性能基准测试
fn bench_inline_cache(c: &mut Criterion) {
    let cache = InlineCache::default();
    
    let mut group = c.benchmark_group("inline_cache");
    
    group.bench_function("lookup_hit", |b| {
        let call_site = GuestAddr(0x2000);
        cache.update(call_site, 42, GuestAddr(0x3000));
        
        b.iter(|| {
            black_box(cache.lookup(black_box(call_site), black_box(42)));
        });
    });
    
    group.bench_function("lookup_miss", |b| {
        let call_site = GuestAddr(0x2000);
        cache.update(call_site, 42, GuestAddr(0x3000));
        
        b.iter(|| {
            black_box(cache.lookup(black_box(call_site), black_box(99)));
        });
    });
    
    group.bench_function("update", |b| {
        let call_site = GuestAddr(0x2000);
        b.iter(|| {
            cache.update(black_box(call_site), black_box(42), black_box(GuestAddr(0x3000)));
        });
    });
    
    group.bench_function("polymorphic_lookup", |b| {
        let call_site = GuestAddr(0x2000);
        for i in 0..10 {
            cache.update(call_site, i, GuestAddr(0x3000 + i));
        }
        
        b.iter(|| {
            let receiver = black_box(5);
            black_box(cache.lookup(black_box(call_site), receiver));
        });
    });
    
    group.finish();
}

/// LRU 缓存性能基准测试
fn bench_lru_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_cache");
    
    for cache_size in [100, 1000, 10000].iter() {
        let mut cache = LRUCache::new(*cache_size * 1024);
        
        group.bench_with_input(BenchmarkId::new("insert", cache_size), cache_size, |b, size| {
            b.iter(|| {
                for i in 0..*size {
                    cache.insert(GuestAddr(i as u64 * 4), vec![0x90; 100]);
                }
            });
        });
        
        group.bench_with_input(BenchmarkId::new("get_hit", cache_size), cache_size, |b, size| {
            for i in 0..*size {
                cache.insert(GuestAddr(i as u64 * 4), vec![0x90; 100]);
            }
            b.iter(|| {
                black_box(cache.get(black_box(GuestAddr(0))));
            });
        });
        
        group.bench_with_input(BenchmarkId::new("get_miss", cache_size), cache_size, |b, size| {
            for i in 0..*size {
                cache.insert(GuestAddr(i as u64 * 4), vec![0x90; 100]);
            }
            b.iter(|| {
                black_box(cache.get(black_box(GuestAddr(0x7FFF_FFFF))));
            });
        });
    }
    
    group.finish();
}

/// 分层缓存性能基准测试
fn bench_tiered_cache(c: &mut Criterion) {
    use vm_engine_jit::tiered_cache::TieredCodeCache;
    
    let config = vm_engine_jit::tiered_cache::TieredCacheConfig::default();
    let cache = TieredCodeCache::new(config);
    
    let mut group = c.benchmark_group("tiered_cache");
    
    group.bench_function("l1_lookup", |b| {
        let pc = GuestAddr(0x4000);
        cache.insert(pc, vec![0x90; 100]);
        for _ in 0..1000 {
            let _ = cache.get(pc);
        }
        
        b.iter(|| {
            black_box(cache.get(black_box(pc)));
        });
    });
    
    group.bench_function("l2_lookup", |b| {
        let pc = GuestAddr(0x5000);
        cache.insert(pc, vec![0x90; 100]);
        for _ in 0..200 {
            let _ = cache.get(pc);
        }
        
        b.iter(|| {
            black_box(cache.get(black_box(pc)));
        });
    });
    
    group.bench_function("l3_lookup", |b| {
        let pc = GuestAddr(0x6000);
        cache.insert(pc, vec![0x90; 100]);
        for _ in 0..10 {
            let _ = cache.get(pc);
        }
        
        b.iter(|| {
            black_box(cache.get(black_box(pc)));
        });
    });
    
    group.bench_function("insert_promotion", |b| {
        let pc = GuestAddr(0x7000);
        b.iter(|| {
            cache.insert(black_box(pc), vec![0x90; 100]);
        });
    });
    
    group.finish();
}

/// 编译器综合基准测试
fn bench_compiler_comprehensive(c: &mut Criterion) {
    let config = TieredCompilerConfig::default();
    let mut compiler = TieredJITCompiler::new(config);
    
    let mut group = c.benchmark_group("compiler_comprehensive");
    
    group.bench_function("cold_start", |b| {
        let block = create_test_block(100);
        b.iter(|| {
            let mut compiler = TieredJITCompiler::new(TieredCompilerConfig::default());
            black_box(compiler.execute(black_box(&block)));
        });
    });
    
    group.bench_function("warm_execution", |b| {
        let block = create_test_block(100);
        for _ in 0..200 {
            let _ = compiler.execute(&block);
        }
        b.iter(|| {
            black_box(compiler.execute(black_box(&block)));
        });
    });
    
    group.bench_function("multiple_blocks", |b| {
        let blocks: Vec<_> = (0..10).map(|i| {
            let mut ops = Vec::new();
            for j in 0..50 {
                ops.push(IROp::MovImm {
                    rd: (j % 32) as u8,
                    imm: (i * 50 + j) as u64,
                });
            }
            IRBlock {
                start_pc: GuestAddr(0x1000 + i as u64 * 0x100),
                ops,
                term: vm_ir::Terminator::Return,
            }
        });
        
        b.iter(|| {
            for block in &blocks {
                black_box(compiler.execute(black_box(block)));
            }
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_tiered_compiler,
    bench_inline_cache,
    bench_lru_cache,
    bench_tiered_cache,
    bench_compiler_comprehensive
);
criterion_main!(benches);
