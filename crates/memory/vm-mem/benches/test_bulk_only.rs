//! 专门测试bulk_read的简单基准测试
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use vm_core::AddressTranslator;
use vm_core::mmu_traits::MemoryAccess;
use vm_mem::PhysicalMemory;

fn bench_bulk_read_simple(c: &mut Criterion) {
    let mem = PhysicalMemory::new(1024 * 1024 * 1024, false);
    let test_addr = vm_core::GuestAddr(0x1000);

    let mut group = c.benchmark_group("bulk_read_simple");

    for size in [256u64].iter() {
        group.throughput(Throughput::Bytes(*size));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, size| {
            let mut buffer = vec![0u8; *size as usize];

            b.iter(|| {
                let addr = black_box(test_addr);
                match mem.read_bulk(addr, &mut buffer) {
                    Ok(_) => black_box(buffer.len()),
                    Err(e) => {
                        eprintln!("Error: {:?}", e);
                        panic!("Bulk read failed");
                    }
                }
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_bulk_read_simple);
criterion_main!(benches);
