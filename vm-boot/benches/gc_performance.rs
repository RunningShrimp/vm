//! GC 性能基准测试

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vm_boot::gc::mark_sweep::{MarkSweepCollector, RootReference, RootSource};
use std::sync::Arc;

fn bench_full_gc(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_full");
    group.bench_function("full_collect_1000_objects", |b| {
        let collector = Arc::new(MarkSweepCollector::new());
        
        // 创建1000个对象
        for i in 0..1000 {
            collector.register_object(0x1000 + i * 100, 100, 1);
        }
        
        // 添加10个根对象
        for i in 0..10 {
            collector.add_root(RootReference {
                addr: 0x1000 + i * 100,
                source: RootSource::Register { reg_id: i as u32 },
            });
        }
        
        let collector_clone = Arc::clone(&collector);
        b.iter(|| {
            black_box(collector_clone.collect());
        });
    });
    group.finish();
}

fn bench_incremental_gc(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_incremental");
    group.bench_function("incremental_step_1000_objects", |b| {
        let collector = Arc::new(MarkSweepCollector::new());
        
        // 创建1000个对象
        for i in 0..1000 {
            collector.register_object(0x1000 + i * 100, 100, 1);
        }
        
        // 添加10个根对象
        for i in 0..10 {
            collector.add_root(RootReference {
                addr: 0x1000 + i * 100,
                source: RootSource::Register { reg_id: i as u32 },
            });
        }
        
        // 启动增量GC
        collector.incremental_step();
        
        let collector_clone = Arc::clone(&collector);
        b.iter(|| {
            black_box(collector_clone.incremental_step());
        });
    });
    group.finish();
}

fn bench_gc_mark_phase(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_mark");
    group.bench_function("mark_10000_objects", |b| {
        let collector = Arc::new(MarkSweepCollector::new());
        
        // 创建10000个对象
        for i in 0..10000 {
            collector.register_object(0x1000 + i * 100, 100, 1);
        }
        
        // 添加100个根对象
        for i in 0..100 {
            collector.add_root(RootReference {
                addr: 0x1000 + i * 100,
                source: RootSource::Register { reg_id: i as u32 },
            });
        }
        
        let collector_clone = Arc::clone(&collector);
        b.iter(|| {
            // 直接调用标记阶段（通过 collect 间接调用）
            black_box(collector_clone.collect());
        });
    });
    group.finish();
}

criterion_group!(benches, bench_full_gc, bench_incremental_gc, bench_gc_mark_phase);
criterion_main!(benches);

