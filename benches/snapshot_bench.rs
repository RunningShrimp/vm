//! Snapshot Performance Benchmark
//!
//! Benchmarks VM snapshot save and restore operations:
//! - Save time (ms)
//! - Restore time (ms)
//! - Compression ratio
//! - Throughput (MB/s)
//!
//! Test cases: Small VM (128MB), Medium VM (1GB), Large VM (4GB)
//!
//! Run: cargo bench --bench snapshot_bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

/// Mock VM state
#[derive(Debug, Clone)]
struct MockVMState {
    memory: Vec<u8>,
    registers: Vec<u64>,
    devices: Vec<MockDeviceState>,
}

impl MockVMState {
    fn new(memory_size_mb: usize) -> Self {
        Self {
            memory: vec![0u8; memory_size_mb * 1024 * 1024],
            registers: vec![0u64; 32],
            devices: vec![],
        }
    }

    fn size_bytes(&self) -> usize {
        self.memory.len() + self.registers.len() * 8
    }
}

/// Mock device state
#[derive(Debug, Clone)]
struct MockDeviceState {
    device_id: u32,
    state_data: Vec<u8>,
}

/// Snapshot data
#[derive(Debug, Clone)]
struct Snapshot {
    data: Vec<u8>,
    compressed: bool,
    compression_ratio: f64,
}

impl Snapshot {
    fn new(data: Vec<u8>, compressed: bool) -> Self {
        let compression_ratio = if compressed {
            data.len() as f64 / (data.len() as f64 * 0.6) // Simulate 60% compression
        } else {
            1.0
        };

        Self {
            data,
            compressed,
            compression_ratio,
        }
    }

    fn size(&self) -> usize {
        self.data.len()
    }
}

/// Mock snapshot manager
struct SnapshotManager {
    use_compression: bool,
}

impl SnapshotManager {
    fn new(use_compression: bool) -> Self {
        Self { use_compression }
    }

    fn save(&self, state: &MockVMState) -> Snapshot {
        // Simulate serialization
        let mut data = Vec::new();

        // Serialize memory
        data.extend_from_slice(&state.memory);

        // Serialize registers
        for reg in &state.registers {
            data.extend_from_slice(&reg.to_le_bytes());
        }

        // Simulate compression if enabled
        if self.use_compression {
            // Simulate compression work
            let compressed_size = (data.len() as f64 * 0.6) as usize;
            data.truncate(compressed_size);
        }

        Snapshot::new(data, self.use_compression)
    }

    fn restore(&self, snapshot: &Snapshot) -> MockVMState {
        let data = &snapshot.data;

        // Restore memory (simplified)
        let memory_size = if snapshot.compressed {
            data.len() / 3 // Reverse the compression simulation
        } else {
            data.len()
        };

        MockVMState {
            memory: vec![0u8; memory_size],
            registers: vec![0u64; 32],
            devices: vec![],
        }
    }
}

/// Benchmark snapshot save operation
fn bench_snapshot_save(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot/save");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(10));

    let vm_sizes = [128, 512, 1024, 4096]; // MB

    for &size_mb in &vm_sizes {
        group.throughput(Throughput::Bytes((size_mb * 1024 * 1024) as u64));
        group.bench_with_input(
            BenchmarkId::new("uncompressed", size_mb),
            &size_mb,
            |b, &size_mb| {
                let manager = SnapshotManager::new(false);
                let state = MockVMState::new(size_mb);

                b.iter(|| {
                    let snapshot = manager.save(black_box(&state));
                    black_box(snapshot)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("compressed", size_mb),
            &size_mb,
            |b, &size_mb| {
                let manager = SnapshotManager::new(true);
                let state = MockVMState::new(size_mb);

                b.iter(|| {
                    let snapshot = manager.save(black_box(&state));
                    black_box(snapshot)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark snapshot restore operation
fn bench_snapshot_restore(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot/restore");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(10));

    let vm_sizes = [128, 512, 1024, 4096]; // MB

    for &size_mb in &vm_sizes {
        group.throughput(Throughput::Bytes((size_mb * 1024 * 1024) as u64));
        group.bench_with_input(
            BenchmarkId::new("uncompressed", size_mb),
            &size_mb,
            |b, &size_mb| {
                let manager = SnapshotManager::new(false);
                let state = MockVMState::new(size_mb);
                let snapshot = manager.save(&state);

                b.iter(|| {
                    let restored = manager.restore(black_box(&snapshot));
                    black_box(restored)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("compressed", size_mb),
            &size_mb,
            |b, &size_mb| {
                let manager = SnapshotManager::new(true);
                let state = MockVMState::new(size_mb);
                let snapshot = manager.save(&state);

                b.iter(|| {
                    let restored = manager.restore(black_box(&snapshot));
                    black_box(restored)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark compression ratio
fn bench_compression_ratio(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot/compression");

    let vm_sizes = [128, 512, 1024, 4096];

    for &size_mb in &vm_sizes {
        group.bench_with_input(
            BenchmarkId::from_parameter(size_mb),
            &size_mb,
            |b, &size_mb| {
                let manager = SnapshotManager::new(true);
                let state = MockVMState::new(size_mb);

                b.iter(|| {
                    let snapshot = manager.save(black_box(&state));
                    black_box(snapshot.compression_ratio)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark incremental snapshot
fn bench_incremental_snapshot(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot/incremental");

    let change_percentages = [1, 5, 10, 25];

    for &change_pct in &change_percentages {
        group.bench_with_input(
            BenchmarkId::new("save", change_pct),
            &change_pct,
            |b, &change_pct| {
                let manager = SnapshotManager::new(true);
                let mut state = MockVMState::new(1024); // 1GB VM

                // Simulate incremental changes
                let changed_bytes = (state.memory.len() * change_pct / 100) as u64;

                b.iter(|| {
                    // Simulate incremental save by only processing changed bytes
                    let start = std::time::Instant::now();
                    let _changed_data = &state.memory[..changed_bytes as usize];
                    let duration = start.elapsed();
                    black_box(duration)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark snapshot I/O throughput
fn bench_snapshot_io_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot/io_throughput");

    let vm_sizes = [128, 512, 1024, 4096];

    for &size_mb in &vm_sizes {
        group.throughput(Throughput::Bytes((size_mb * 1024 * 1024) as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size_mb),
            &size_mb,
            |b, &size_mb| {
                let state = MockVMState::new(size_mb);
                let temp_dir = std::env::temp_dir();
                let snapshot_path = temp_dir.join(format!("snapshot_{}.dat", size_mb));

                b.iter(|| {
                    // Simulate file I/O
                    let start = std::time::Instant::now();
                    // In real implementation, this would write to disk
                    let _data = &state.memory[..];
                    let duration = start.elapsed();
                    black_box(duration)
                });

                // Cleanup
                let _ = std::fs::remove_file(snapshot_path);
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent snapshot operations
fn bench_concurrent_snapshots(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot/concurrent");

    let vm_counts = [1, 2, 4, 8];

    for &vm_count in &vm_counts {
        group.bench_with_input(
            BenchmarkId::new("parallel_save", vm_count),
            &vm_count,
            |b, &vm_count| {
                b.iter(|| {
                    let mut handles = Vec::new();

                    for i in 0..vm_count {
                        let manager = SnapshotManager::new(true);
                        let state = MockVMState::new(512); // 512MB each

                        let handle = std::thread::spawn(move || {
                            manager.save(&state)
                        });

                        handles.push(handle);
                    }

                    let snapshots: Vec<_> = handles
                        .into_iter()
                        .map(|h| h.join().unwrap())
                        .collect();

                    black_box(snapshots)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_snapshot_save,
    bench_snapshot_restore,
    bench_compression_ratio,
    bench_incremental_snapshot,
    bench_snapshot_io_throughput,
    bench_concurrent_snapshots
);

criterion_main!(benches);
