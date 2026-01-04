///
/// 测试无锁TLB实现的性能指标
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
/// 无锁TLB性能基准测试
use vm_core::AddressTranslator;
use vm_mem::tlb::core::lockfree::{LockFreeTlb, TlbEntry};

fn bench_lockfree_tlb_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_tlb_lookup");

    // 测试不同TLB大小的查找性能
    for size in [16, 32, 64, 128, 256, 512, 1024].iter() {
        let tlb = LockFreeTlb::new();

        // 预填充TLB
        for i in 0..*size {
            let entry = TlbEntry::new(i * 4096, i * 4096, 0x1, 0);
            tlb.insert(entry);
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &_size| {
            b.iter(|| {
                black_box(tlb.lookup(0x1000, 0));
            });
        });
    }

    group.finish();
}

fn bench_lockfree_tlb_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_tlb_insert");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, size| {
            let tlb = LockFreeTlb::new();

            b.iter(|| {
                for i in 0..*size {
                    let entry = TlbEntry::new(i * 4096, i * 4096, 0x1, 0);
                    black_box(tlb.insert(entry));
                }
            });
        });
    }

    group.finish();
}

fn bench_lockfree_tlb_batch(c: &mut Criterion) {
    let tlb = LockFreeTlb::new();

    // 预填充TLB
    for i in 0..1000 {
        let entry = TlbEntry::new(i * 4096, i * 4096, 0x1, 0);
        tlb.insert(entry);
    }

    let mut group = c.benchmark_group("lockfree_tlb_batch");

    for batch_size in [10, 50, 100, 500].iter() {
        let requests: Vec<_> = (0..*batch_size).map(|i| (i * 4096, 0)).collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &_size| {
                b.iter(|| {
                    black_box(tlb.lookup_batch(&requests));
                });
            },
        );
    }

    group.finish();
}

fn bench_lockfree_tlb_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_tlb_concurrent");

    for num_threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            num_threads,
            |b, &num_threads| {
                let tlb = std::sync::Arc::new(LockFreeTlb::new());
                let barrier = std::sync::Arc::new(std::sync::Barrier::new(*num_threads));

                // 预填充TLB
                for i in 0..1000 {
                    let entry = TlbEntry::new(i * 4096, i * 4096, 0x1, 0);
                    tlb.insert(entry);
                }

                b.iter(|| {
                    let mut handles = vec![];

                    for thread_id in 0..*num_threads {
                        let tlb_clone = std::sync::Arc::clone(&tlb);
                        let barrier_clone = std::sync::Arc::clone(&barrier);

                        handles.push(std::thread::spawn(move || {
                            barrier_clone.wait();

                            for i in 0..1000 {
                                black_box(tlb_clone.lookup(i * 4096, 0));
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_lockfree_tlb_lookup,
    bench_lockfree_tlb_insert,
    bench_lockfree_tlb_batch,
    bench_lockfree_tlb_concurrent
);
criterion_main!(benches);
