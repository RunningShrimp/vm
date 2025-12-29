//! Device I/O Performance Benchmark
//!
//! Benchmarks device I/O performance:
//! - Throughput (MB/s)
//! - Latency (μs)
//! - IOPS (I/O operations per second)
//!
//! Test cases: Block device, Network device, GPU passthrough
//!
//! Run: cargo bench --bench device_io_bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Mock block device
#[derive(Debug)]
struct BlockDevice {
    block_size: usize,
    total_blocks: usize,
    data: Vec<Vec<u8>>,
}

impl BlockDevice {
    fn new(block_size: usize, total_blocks: usize) -> Self {
        Self {
            block_size,
            total_blocks,
            data: vec![vec![0u8; block_size]; total_blocks],
        }
    }

    fn read_block(&self, block_id: usize) -> Option<Vec<u8>> {
        if block_id < self.total_blocks {
            Some(self.data[block_id].clone())
        } else {
            None
        }
    }

    fn write_block(&mut self, block_id: usize, data: &[u8]) -> bool {
        if block_id < self.total_blocks && data.len() == self.block_size {
            self.data[block_id].copy_from_slice(data);
            true
        } else {
            false
        }
    }

    fn read(&self, offset: usize, size: usize) -> Vec<u8> {
        let mut result = Vec::with_capacity(size);
        let start_block = offset / self.block_size;
        let end_block = (offset + size + self.block_size - 1) / self.block_size;

        for block_id in start_block..end_block.min(self.total_blocks) {
            result.extend_from_slice(&self.data[block_id]);
        }

        result.truncate(size);
        result
    }

    fn write(&mut self, offset: usize, data: &[u8]) -> usize {
        let start_block = offset / self.block_size;
        let mut written = 0;

        for (i, chunk) in data.chunks(self.block_size).enumerate() {
            let block_id = start_block + i;
            if block_id < self.total_blocks {
                self.data[block_id][..chunk.len()].copy_from_slice(chunk);
                written += chunk.len();
            }
        }

        written
    }
}

/// Mock network device
#[derive(Debug)]
struct NetworkDevice {
    mtu: usize,
    packets: Vec<Vec<u8>>,
}

impl NetworkDevice {
    fn new(mtu: usize) -> Self {
        Self {
            mtu,
            packets: Vec::new(),
        }
    }

    fn send_packet(&mut self, packet: &[u8]) -> bool {
        if packet.len() <= self.mtu {
            self.packets.push(packet.to_vec());
            true
        } else {
            false
        }
    }

    fn receive_packet(&mut self) -> Option<Vec<u8>> {
        self.packets.pop()
    }

    fn get_packet_count(&self) -> usize {
        self.packets.len()
    }
}

/// Mock GPU device
#[derive(Debug)]
struct GPUDevice {
    memory_size: usize,
    memory: Vec<u8>,
    transfer_count: usize,
}

impl GPUDevice {
    fn new(memory_size: usize) -> Self {
        Self {
            memory_size,
            memory: vec![0u8; memory_size],
            transfer_count: 0,
        }
    }

    fn memory_to_gpu(&mut self, data: &[u8], offset: usize) -> bool {
        if offset + data.len() <= self.memory_size {
            self.memory[offset..offset + data.len()].copy_from_slice(data);
            self.transfer_count += 1;
            true
        } else {
            false
        }
    }

    fn memory_from_gpu(&self, offset: usize, size: usize) -> Option<Vec<u8>> {
        if offset + size <= self.memory_size {
            Some(self.memory[offset..offset + size].to_vec())
        } else {
            None
        }
    }

    fn execute_kernel(&mut self, instructions: u32) -> Duration {
        // Simulate GPU kernel execution time
        let simulated_time = instructions as u64 * 10; // 10ns per instruction
        self.transfer_count += 1;
        Duration::from_nanos(simulated_time)
    }
}

/// Benchmark block device sequential read
fn bench_block_sequential_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_io/block/sequential_read");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    let transfer_sizes = [4096, 65536, 1024 * 1024, 10 * 1024 * 1024]; // 4KB, 64KB, 1MB, 10MB

    for &size in &transfer_sizes {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("size", format!("{}KB", size / 1024)),
            &size,
            |b, &size| {
                let device = BlockDevice::new(4096, 10000);

                b.iter(|| {
                    let data = device.read(black_box(0), black_box(size));
                    black_box(data)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark block device sequential write
fn bench_block_sequential_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_io/block/sequential_write");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    let transfer_sizes = [4096, 65536, 1024 * 1024, 10 * 1024 * 1024];

    for &size in &transfer_sizes {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("size", format!("{}KB", size / 1024)),
            &size,
            |b, &size| {
                let mut device = BlockDevice::new(4096, 10000);
                let data = vec![0u8; size];

                b.iter(|| {
                    let written = device.write(black_box(0), black_box(&data));
                    black_box(written)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark block device random I/O
fn bench_block_random_io(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_io/block/random_io");

    let io_sizes = [512, 4096, 65536]; // 512B, 4KB, 64KB

    for &size in &io_sizes {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                let mut device = BlockDevice::new(4096, 10000);

                b.iter(|| {
                    let mut total = 0;
                    for _ in 0..1000 {
                        let offset = (rand::random::<usize>() % 100) * 4096;
                        let data = vec![0u8; size];
                        total += device.write(offset, &data);
                    }
                    black_box(total)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark block device IOPS
fn bench_block_iops(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_io/block/iops");

    group.bench_function("4k_random_reads", |b| {
        let device = BlockDevice::new(4096, 10000);

        b.iter(|| {
            let mut count = 0;
            for _ in 0..10000 {
                let offset = (rand::random::<usize>() % 1000) * 4096;
                if device.read_block(offset / 4096).is_some() {
                    count += 1;
                }
            }
            black_box(count)
        });
    });

    group.bench_function("4k_random_writes", |b| {
        let mut device = BlockDevice::new(4096, 10000);
        let data = vec![0u8; 4096];

        b.iter(|| {
            let mut count = 0;
            for _ in 0..10000 {
                let block_id = rand::random::<usize>() % 1000;
                if device.write_block(block_id, &data) {
                    count += 1;
                }
            }
            black_box(count)
        });
    });

    group.finish();
}

/// Benchmark network device throughput
fn bench_network_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_io/network/throughput");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    let packet_sizes = [64, 256, 1024, 1518, 9216]; // Standard MTU sizes

    for &packet_size in &packet_sizes {
        group.throughput(Throughput::Bytes(packet_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(packet_size),
            &packet_size,
            |b, &packet_size| {
                let mut device = NetworkDevice::new(9216);
                let packet = vec![0u8; packet_size];

                b.iter(|| {
                    for _ in 0..1000 {
                        device.send_packet(black_box(&packet));
                    }
                    black_box(device.get_packet_count())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark network device packet rate
fn bench_network_packet_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_io/network/packet_rate");

    group.bench_function("packets_per_second", |b| {
        let mut device = NetworkDevice::new(1518);
        let packet = vec![0u8; 1518];

        b.iter(|| {
            let start = std::time::Instant::now();
            for _ in 0..100000 {
                device.send_packet(&packet);
            }
            let duration = start.elapsed();
            black_box(duration)
        });
    });

    group.finish();
}

/// Benchmark GPU memory transfers
fn bench_gpu_memory_transfers(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_io/gpu/memory_transfers");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    let transfer_sizes = [
        1024 * 1024,       // 1MB
        10 * 1024 * 1024,  // 10MB
        100 * 1024 * 1024, // 100MB
    ];

    for &size in &transfer_sizes {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("to_gpu", format!("{}MB", size / 1024 / 1024)),
            &size,
            |b, &size| {
                let mut gpu = GPUDevice::new(1024 * 1024 * 1024); // 1GB GPU memory
                let data = vec![0u8; size];

                b.iter(|| {
                    let success = gpu.memory_to_gpu(black_box(&data), 0);
                    black_box(success)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("from_gpu", format!("{}MB", size / 1024 / 1024)),
            &size,
            |b, &size| {
                let gpu = GPUDevice::new(1024 * 1024 * 1024);

                b.iter(|| {
                    let data = gpu.memory_from_gpu(black_box(0), black_box(size));
                    black_box(data)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark GPU kernel execution
fn bench_gpu_kernel_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_io/gpu/kernel_execution");

    let instruction_counts = [1000, 10000, 100000, 1000000];

    for &count in &instruction_counts {
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &count,
            |b, &count| {
                let mut gpu = GPUDevice::new(1024 * 1024 * 1024);

                b.iter(|| {
                    let duration = gpu.execute_kernel(black_box(count as u32));
                    black_box(duration)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent device I/O
fn bench_concurrent_device_io(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_io/concurrent");

    let thread_counts = [1, 2, 4, 8];

    for &thread_count in &thread_counts {
        group.bench_with_input(
            BenchmarkId::new("block_io", thread_count),
            &thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let device = Arc::new(Mutex::new(BlockDevice::new(4096, 10000)));
                    let mut handles = Vec::new();

                    for _ in 0..thread_count {
                        let dev = device.clone();
                        let handle = std::thread::spawn(move || {
                            let mut total = 0;
                            for i in 0..1000 {
                                let offset = i * 4096;
                                let mut d = dev.lock().unwrap();
                                let data = d.read(offset, 4096);
                                total += data.len();
                            }
                            total
                        });
                        handles.push(handle);
                    }

                    let mut total_bytes = 0;
                    for handle in handles {
                        total_bytes += handle.join().unwrap();
                    }

                    black_box(total_bytes)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark device I/O latency
fn bench_device_io_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_io/latency");

    group.bench_function("block_read_latency", |b| {
        let device = BlockDevice::new(4096, 10000);

        b.iter(|| {
            let start = std::time::Instant::now();
            let _data = device.read(0, 4096);
            let latency = start.elapsed();
            black_box(latency)
        });
    });

    group.bench_function("network_send_latency", |b| {
        let mut device = NetworkDevice::new(1518);
        let packet = vec![0u8; 1518];

        b.iter(|| {
            let start = std::time::Instant::now();
            device.send_packet(&packet);
            let latency = start.elapsed();
            black_box(latency)
        });
    });

    group.bench_function("gpu_transfer_latency", |b| {
        let mut gpu = GPUDevice::new(1024 * 1024 * 1024);
        let data = vec![0u8; 1024];

        b.iter(|| {
            let start = std::time::Instant::now();
            gpu.memory_to_gpu(&data, 0);
            let latency = start.elapsed();
            black_box(latency)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_block_sequential_read,
    bench_block_sequential_write,
    bench_block_random_io,
    bench_block_iops,
    bench_network_throughput,
    bench_network_packet_rate,
    bench_gpu_memory_transfers,
    bench_gpu_kernel_execution,
    bench_concurrent_device_io,
    bench_device_io_latency
);

criterion_main!(benches);
