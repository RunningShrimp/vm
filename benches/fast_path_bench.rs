//! 快速路径翻译器性能基准测试
//!
//! 测试快速路径缓存的性能提升

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use vm_cross_arch::fast_path::{FastPathTranslator, SourceInsnKey};
use vm_cross_arch::TargetInstruction;

fn bench_fast_path_hit(c: &mut Criterion) {
    let mut group = c.benchmark_group("fast_path_hit");

    // 测试不同缓存大小的性能
    for size in [1024usize, 4096, 16384] {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b: &mut criterion::Bencher, cache_size: &usize| {
            let translator = FastPathTranslator::new(*cache_size, vm_cross_arch::fast_path::CachePolicy::Lfu);

            // 预热缓存
            for i in 0..*cache_size {
                let key = SourceInsnKey::from_opcode(1, 2, i as u32);
                translator.insert_fast(key, TargetInstruction {
                    bytes: vec![0x90],
                    length: 1,
                    mnemonic: "nop".to_string(),
                    is_control_flow: false,
                    is_memory_op: false,
                });
            }

            // 测试缓存命中性能
            b.iter(|| {
                let key = SourceInsnKey::from_opcode(1, 2, 1000);
                black_box(translator.translate_fast(&key));
            });
        });
    }

    group.finish();
}

fn bench_fast_path_miss(c: &mut Criterion) {
    let translator = FastPathTranslator::new(4096, vm_cross_arch::fast_path::CachePolicy::Lfu);

    c.bench_function("fast_path_miss", |b| {
        b.iter(|| {
            // 每次使用不同的key，确保缓存未命中
            let key = SourceInsnKey::from_opcode(1, 2, fastrand::u32(..));
            black_box(translator.translate_fast(&key));
        });
    });
}

fn bench_fast_path_mixed_hit_rate(c: &mut Criterion) {
    let translator = FastPathTranslator::new(4096, vm_cross_arch::fast_path::CachePolicy::Lfu);

    // 预热50%的缓存
    for i in 0..2048 {
        let key = SourceInsnKey::from_opcode(1, 2, i);
        translator.insert_fast(key, TargetInstruction {
            bytes: vec![0x90],
            length: 1,
            mnemonic: "nop".to_string(),
            is_control_flow: false,
            is_memory_op: false,
        });
    }

    c.bench_function("fast_path_50_percent_hit_rate", |b| {
        b.iter(|| {
            // 50%概率命中
            let opcode = if fastrand::bool() { 1000 } else { 5000 };
            let key = SourceInsnKey::from_opcode(1, 2, opcode);
            black_box(translator.translate_fast(&key));
        });
    });
}

fn bench_cache_policies(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_policies");

    for policy in [
        vm_cross_arch::fast_path::CachePolicy::Lru,
        vm_cross_arch::fast_path::CachePolicy::Lfu,
        vm_cross_arch::fast_path::CachePolicy::Fifo,
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(format!("{:?}", policy)), &policy, |b: &mut criterion::Bencher, p: &vm_cross_arch::fast_path::CachePolicy| {
            let translator = FastPathTranslator::new(4096, *p);

            // 预热缓存
            for i in 0..2048 {
                let key = SourceInsnKey::from_opcode(1, 2, i);
                translator.insert_fast(key, TargetInstruction {
                    bytes: vec![0x90],
                    length: 1,
                    mnemonic: "nop".to_string(),
                    is_control_flow: false,
                    is_memory_op: false,
                });
            }

            b.iter(|| {
                let key = SourceInsnKey::from_opcode(1, 2, 1000);
                black_box(translator.translate_fast(&key));
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_fast_path_hit, bench_fast_path_miss, bench_fast_path_mixed_hit_rate, bench_cache_policies);
criterion_main!(benches);
