//! Memory Operations Performance Benchmarks
//!
//! Comprehensive benchmarks for memory management performance:
//! - Memory copy speed
//! - MMU translation latency
//! - TLB hit/miss rates
//! - Memory allocation/deallocation
//!
//! Run: cargo bench --bench memory_operations

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

// Mock MMU (Memory Management Unit)
struct MockMMU {
    page_size: u64,
    translation_cache: std::collections::HashMap<u64, u64>,
}

impl MockMMU {
    fn new(page_size: u64) -> Self {
        Self {
            page_size,
            translation_cache: std::collections::HashMap::new(),
        }
    }

    /// Translate virtual address to physical address
    fn translate(&mut self, virt_addr: u64) -> u64 {
        let page_offset = virt_addr % self.page_size;
        let page_num = virt_addr / self.page_size;

        // Check cache first
        if let Some(&phys_page) = self.translation_cache.get(&page_num) {
            return phys_page * self.page_size + page_offset;
        }

        // Simulate page table walk (cache miss)
        let phys_page = page_num + 0x1000; // Simple 1-1 mapping with offset
        self.translation_cache.insert(page_num, phys_page);

        // Simulate cache size limit
        if self.translation_cache.len() > 1024 {
            // Remove oldest entry (simplified LRU)
            let key = *self.translation_cache.keys().next().unwrap();
            self.translation_cache.remove(&key);
        }

        phys_page * self.page_size + page_offset
    }

    /// Flush TLB
    fn flush_tlb(&mut self) {
        self.translation_cache.clear();
    }
}

// Mock TLB (Translation Lookaside Buffer)
struct MockTLB {
    entries: Vec<(u64, u64)>, // (virtual_page, physical_page)
    capacity: usize,
    access_count: u64,
    hit_count: u64,
}

impl MockTLB {
    fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
            access_count: 0,
            hit_count: 0,
        }
    }

    /// Lookup address in TLB
    fn lookup(&mut self, virt_addr: u64, page_size: u64) -> Option<u64> {
        self.access_count += 1;
        let page_num = virt_addr / page_size;

        // Linear search (simulating hardware lookup)
        for entry in &self.entries {
            if entry.0 == page_num {
                self.hit_count += 1;
                return Some(entry.1 * page_size + (virt_addr % page_size));
            }
        }

        None
    }

    /// Insert entry into TLB
    fn insert(&mut self, virt_addr: u64, phys_addr: u64, page_size: u64) {
        let virt_page = virt_addr / page_size;
        let phys_page = phys_addr / page_size;

        // If at capacity, replace first entry (simplified replacement policy)
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }

        self.entries.push((virt_page, phys_page));
    }

    /// Get hit rate
    fn hit_rate(&self) -> f64 {
        if self.access_count == 0 {
            0.0
        } else {
            self.hit_count as f64 / self.access_count as f64
        }
    }

    /// Flush TLB
    fn flush(&mut self) {
        self.entries.clear();
    }
}

// Mock memory allocator
struct MockMemoryAllocator {
    memory: Vec<u8>,
    allocated_blocks: Vec<(usize, usize)>, // (offset, size)
}

impl MockMemoryAllocator {
    fn new(capacity: usize) -> Self {
        Self {
            memory: vec![0u8; capacity],
            allocated_blocks: Vec::new(),
        }
    }

    /// Allocate memory block
    fn allocate(&mut self, size: usize) -> Option<usize> {
        // First-fit allocation
        let mut offset = 0;

        for &(alloc_offset, alloc_size) in &self.allocated_blocks {
            if offset + size <= alloc_offset {
                // Found free space
                self.allocated_blocks.push((offset, size));
                self.allocated_blocks.sort_by_key(|&(off, _)| off);
                return Some(offset);
            }
            offset = alloc_offset + alloc_size;
        }

        // Check if there's space at the end
        if offset + size <= self.memory.len() {
            self.allocated_blocks.push((offset, size));
            Some(offset)
        } else {
            None
        }
    }

    /// Deallocate memory block
    fn deallocate(&mut self, offset: usize) {
        self.allocated_blocks.retain(|&(off, _)| off != offset);
    }

    /// Copy memory
    fn copy(&mut self, src: usize, dst: usize, size: usize) {
        if src + size <= self.memory.len() && dst + size <= self.memory.len() {
            self.memory.copy_within(src..src + size, dst);
        }
    }
}

/// Benchmark: Memory copy speed
fn bench_memory_copy(c: &mut Criterion) {
    let mut allocator = MockMemoryAllocator::new(10 * 1024 * 1024); // 10 MB

    let mut group = c.benchmark_group("memory_copy");
    group.measurement_time(Duration::from_secs(10));

    for size in [64, 256, 1024, 4096, 16384, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let src = allocator.allocate(size).unwrap();
            let dst = allocator.allocate(size).unwrap();

            // Initialize source
            for i in 0..size {
                allocator.memory[src + i] = i as u8;
            }

            b.iter(|| {
                black_box(allocator.copy(src, dst, size));
            });

            allocator.deallocate(src);
            allocator.deallocate(dst);
        });
    }

    group.finish();
}

/// Benchmark: MMU translation latency
fn bench_mmu_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmu_translation");
    group.measurement_time(Duration::from_secs(10));

    // Single translation
    group.bench_function("single_translation", |b| {
        let mut mmu = MockMMU::new(4096);
        let virt_addr = black_box(0x1000);

        b.iter(|| {
            black_box(mmu.translate(virt_addr));
        });
    });

    // Sequential translations (cache-friendly)
    group.bench_function("sequential_translation", |b| {
        let mut mmu = MockMMU::new(4096);

        b.iter(|| {
            for i in 0..1000 {
                black_box(mmu.translate(i * 4096));
            }
        });
    });

    // Random translations (cache-unfriendly)
    group.bench_function("random_translation", |b| {
        let mut mmu = MockMMU::new(4096);
        let mut rng = rand::thread_rng();
        let addresses: Vec<u64> = (0..1000).map(|_| rng.r#gen::<u64>() & 0xFFFFF000).collect();

        b.iter(|| {
            for &addr in &addresses {
                black_box(mmu.translate(addr));
            }
        });
    });

    group.finish();
}

/// Benchmark: TLB hit rate
fn bench_tlb_hit_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_hit_rate");
    group.measurement_time(Duration::from_secs(10));

    // Small working set (high hit rate)
    group.bench_function("high_hit_rate", |b| {
        let mut tlb = MockTLB::new(256);
        let page_size = 4096;

        // Pre-fill TLB
        for i in 0..100 {
            tlb.insert(i * page_size, (i + 0x1000) * page_size, page_size);
        }

        b.iter(|| {
            for i in 0..1000 {
                let addr = (i % 100) * page_size;
                if tlb.lookup(addr, page_size).is_none() {
                    tlb.insert(addr, (addr / page_size + 0x1000) * page_size, page_size);
                }
            }
        });

        // Verify high hit rate
        assert!(tlb.hit_rate() > 0.95);
    });

    // Large working set (low hit rate)
    group.bench_function("low_hit_rate", |b| {
        let mut tlb = MockTLB::new(256);
        let page_size = 4096;

        b.iter(|| {
            for i in 0..1000 {
                let addr = i * page_size;
                if tlb.lookup(addr, page_size).is_none() {
                    tlb.insert(addr, (addr / page_size + 0x1000) * page_size, page_size);
                }
            }
        });

        // Verify low hit rate
        assert!(tlb.hit_rate() < 0.3);
    });

    group.finish();
}

/// Benchmark: TLB lookup latency
fn bench_tlb_lookup_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_lookup_latency");
    group.measurement_time(Duration::from_secs(10));

    for tlb_size in [64, 256, 1024].iter() {
        group.bench_with_input(
            BenchmarkId::new("tlb_size", tlb_size),
            tlb_size,
            |b, &tlb_size| {
                let mut tlb = MockTLB::new(tlb_size);
                let page_size = 4096;

                // Pre-fill TLB
                for i in 0..(tlb_size / 2) {
                    tlb.insert(i * page_size, (i + 0x1000) * page_size, page_size);
                }

                b.iter(|| {
                    black_box(tlb.lookup(0, page_size));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: TLB flush overhead
fn bench_tlb_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb_flush");
    group.measurement_time(Duration::from_secs(10));

    for tlb_size in [64, 256, 1024].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(tlb_size),
            tlb_size,
            |b, &tlb_size| {
                let mut tlb = MockTLB::new(tlb_size);
                let page_size = 4096;

                // Fill TLB
                for i in 0..tlb_size {
                    tlb.insert(i * page_size, (i + 0x1000) * page_size, page_size);
                }

                b.iter(|| {
                    black_box(tlb.flush());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Memory allocation/deallocation
fn bench_memory_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocation");
    group.measurement_time(Duration::from_secs(10));

    // Small allocations
    group.bench_function("small_allocations", |b| {
        let mut allocator = MockMemoryAllocator::new(10 * 1024 * 1024);

        b.iter(|| {
            let offsets: Vec<_> = (0..100)
                .map(|_| allocator.allocate(1024).unwrap())
                .collect();

            for offset in offsets {
                allocator.deallocate(offset);
            }
        });
    });

    // Large allocations
    group.bench_function("large_allocations", |b| {
        let mut allocator = MockMemoryAllocator::new(100 * 1024 * 1024);

        b.iter(|| {
            let offsets: Vec<_> = (0..10)
                .map(|_| allocator.allocate(1024 * 1024).unwrap())
                .collect();

            for offset in offsets {
                allocator.deallocate(offset);
            }
        });
    });

    // Mixed size allocations
    group.bench_function("mixed_allocations", |b| {
        let mut allocator = MockMemoryAllocator::new(50 * 1024 * 1024);
        let sizes: Vec<usize> = vec![64, 256, 1024, 4096, 16384];
        let mut rng = rand::thread_rng();

        b.iter(|| {
            let offsets: Vec<_> = sizes
                .iter()
                .map(|_| {
                    allocator
                        .allocate(sizes[rng.gen_range(0..sizes.len())])
                        .unwrap()
                })
                .collect();

            for offset in offsets {
                allocator.deallocate(offset);
            }
        });
    });

    group.finish();
}

/// Benchmark: Memory access patterns
fn bench_memory_access_patterns(c: &mut Criterion) {
    let mut allocator = MockMemoryAllocator::new(10 * 1024 * 1024);

    let mut group = c.benchmark_group("memory_access_patterns");
    group.measurement_time(Duration::from_secs(10));

    // Sequential access
    group.bench_function("sequential", |b| {
        let size = 1024 * 1024;
        let offset = allocator.allocate(size).unwrap();

        b.iter(|| {
            let mut sum = 0u64;
            for i in 0..size {
                sum += allocator.memory[offset + i] as u64;
            }
            black_box(sum);
        });

        allocator.deallocate(offset);
    });

    // Random access
    group.bench_function("random", |b| {
        let size = 1024 * 1024;
        let offset = allocator.allocate(size).unwrap();
        let mut rng = rand::thread_rng();

        b.iter(|| {
            let mut sum = 0u64;
            for _ in 0..size {
                let idx = rng.gen_range(0..size);
                sum += allocator.memory[offset + idx] as u64;
            }
            black_box(sum);
        });

        allocator.deallocate(offset);
    });

    // Strided access
    group.bench_function("strided", |b| {
        let size = 1024 * 1024;
        let offset = allocator.allocate(size).unwrap();
        let stride = 64;

        b.iter(|| {
            let mut sum = 0u64;
            for i in (0..size).step_by(stride) {
                sum += allocator.memory[offset + i] as u64;
            }
            black_box(sum);
        });

        allocator.deallocate(offset);
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_memory_copy,
    bench_mmu_translation,
    bench_tlb_hit_rate,
    bench_tlb_lookup_latency,
    bench_tlb_flush,
    bench_memory_allocation,
    bench_memory_access_patterns
);
criterion_main!(benches);
