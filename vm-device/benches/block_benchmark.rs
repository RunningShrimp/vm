/// VirtioBlock 充血模型性能基准测试
///
/// 本基准测试验证 VirtioBlock 充血模型的性能表现，确保没有性能回归
///
/// 测试内容包括：
/// - 读操作性能 (不同大小的数据块)
/// - 写操作性能 (不同大小的数据块)
/// - 请求验证方法性能
/// - 请求处理性能 (完整的请求-响应流程)
///
/// 运行方式：
/// ```bash
/// cargo bench --bench block_benchmark
/// ```
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::time::Duration;
use vm_device::block::{BlockRequest, BlockResponse, VirtioBlock};

/// 基准测试：读操作性能
///
/// 测试不同大小的读取操作性能
fn bench_read_operation(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_operation");

    // 测试不同扇区数量的读操作
    for sector_count in [1, 10, 100, 1000].iter() {
        let block = VirtioBlock::new_memory(10000, 512, false);
        let throughput = (*sector_count * 512) as u64;

        group.throughput(Throughput::Bytes(throughput));
        group.bench_with_input(
            BenchmarkId::new("sectors", sector_count),
            sector_count,
            |b, &count| {
                b.iter(|| black_box(block.read(0, count)));
            },
        );
    }

    // 测试不同偏移量的读操作
    for offset in [0, 100, 1000, 5000].iter() {
        let block = VirtioBlock::new_memory(10000, 512, false);

        group.bench_with_input(BenchmarkId::new("offset", offset), offset, |b, &offset| {
            b.iter(|| black_box(block.read(offset, 10)));
        });
    }

    group.finish();
}

/// 基准测试：写操作性能
///
/// 测试不同大小的写入操作性能
fn bench_write_operation(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_operation");

    // 测试不同扇区数量的写操作
    for sector_count in [1, 10, 100, 1000].iter() {
        let mut block = VirtioBlock::new_memory(10000, 512, false);
        let data = vec![0xABu8; (*sector_count * 512) as usize];
        let throughput = (*sector_count * 512) as u64;

        group.throughput(Throughput::Bytes(throughput));
        group.bench_with_input(
            BenchmarkId::new("sectors", sector_count),
            sector_count,
            |b, &_count| {
                b.iter(|| black_box(block.write(0, &data)));
            },
        );
    }

    // 测试不同偏移量的写操作
    for offset in [0, 100, 1000, 5000].iter() {
        let mut block = VirtioBlock::new_memory(10000, 512, false);
        let data = vec![0xCDu8; 5120];

        group.bench_with_input(BenchmarkId::new("offset", offset), offset, |b, &offset| {
            b.iter(|| black_box(block.write(offset, &data)));
        });
    }

    group.finish();
}

/// 基准测试：验证方法性能
///
/// 测试 validate_read_request 方法的性能
fn bench_validate_read_request(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_read");

    // 测试正常请求的验证性能
    let block = VirtioBlock::new_memory(10000, 512, false);
    group.bench_function("valid_request", |b| {
        b.iter(|| black_box(block.validate_read_request(0, 100)));
    });

    // 测试边界请求的验证性能
    group.bench_function("boundary_request", |b| {
        b.iter(|| black_box(block.validate_read_request(9900, 100)));
    });

    // 测试无效请求的验证性能 (超出范围)
    group.bench_function("out_of_range", |b| {
        b.iter(|| black_box(block.validate_read_request(9901, 100)));
    });

    // 测试零扇区数的验证性能
    group.bench_function("zero_count", |b| {
        b.iter(|| black_box(block.validate_read_request(0, 0)));
    });

    group.finish();
}

/// 基准测试：写请求验证性能
///
/// 测试 validate_write_request 方法的性能
fn bench_validate_write_request(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_write");

    // 测试正常写请求的验证性能
    let block = VirtioBlock::new_memory(10000, 512, false);
    let data = vec![0u8; 5120];
    group.bench_function("valid_request", |b| {
        b.iter(|| black_box(block.validate_write_request(0, &data)));
    });

    // 测试只读设备的验证性能
    let readonly_block = VirtioBlock::new_memory(10000, 512, true);
    group.bench_function("readonly_device", |b| {
        b.iter(|| black_box(readonly_block.validate_write_request(0, &data)));
    });

    // 测试非整数倍数据的验证性能
    let invalid_data = vec![0u8; 513];
    group.bench_function("invalid_size", |b| {
        b.iter(|| black_box(block.validate_write_request(0, &invalid_data)));
    });

    // 测试空数据的验证性能
    let empty_data = vec![0u8; 0];
    group.bench_function("empty_data", |b| {
        b.iter(|| black_box(block.validate_write_request(0, &empty_data)));
    });

    group.finish();
}

/// 基准测试：请求处理性能
///
/// 测试 process_request 方法的性能 (充血模型核心)
fn bench_process_request(c: &mut Criterion) {
    let mut group = c.benchmark_group("process_request");

    // 测试读请求处理性能
    let mut block = VirtioBlock::new_memory(10000, 512, false);
    group.bench_function("read_request", |b| {
        b.iter(|| {
            let request = BlockRequest::Read {
                sector: 0,
                count: 10,
            };
            black_box(block.process_request(request))
        });
    });

    // 测试写请求处理性能
    let mut block = VirtioBlock::new_memory(10000, 512, false);
    group.bench_function("write_request", |b| {
        b.iter(|| {
            let data = vec![0xABu8; 5120];
            let request = BlockRequest::Write { sector: 0, data };
            black_box(block.process_request(request))
        });
    });

    // 测试刷新请求处理性能
    let mut block = VirtioBlock::new_memory(10000, 512, false);
    group.bench_function("flush_request", |b| {
        b.iter(|| {
            let request = BlockRequest::Flush;
            black_box(block.process_request(request))
        });
    });

    group.finish();
}

/// 基准测试：错误处理性能
///
/// 测试各种错误情况的处理性能
fn bench_error_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_handling");

    // 只读设备写错误
    let mut readonly_block = VirtioBlock::new_memory(10000, 512, true);
    group.bench_function("readonly_write", |b| {
        b.iter(|| {
            let data = vec![0u8; 512];
            let request = BlockRequest::Write { sector: 0, data };
            match black_box(readonly_block.process_request(request)) {
                Ok(BlockResponse::Error { .. }) => {}
                _ => panic!("Expected error response"),
            }
        });
    });

    // 超出范围读错误
    let mut block = VirtioBlock::new_memory(10000, 512, false);
    group.bench_function("out_of_range_read", |b| {
        b.iter(|| {
            let request = BlockRequest::Read {
                sector: 9999,
                count: 10,
            };
            match black_box(block.process_request(request)) {
                Ok(BlockResponse::Error { .. }) => {}
                _ => panic!("Expected error response"),
            }
        });
    });

    group.finish();
}

/// 基准测试：连续读写操作性能
///
/// 测试混合读写操作的性能表现
fn bench_mixed_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_operations");

    // 读-写-读序列
    group.bench_function("read_write_read", |b| {
        let mut block = VirtioBlock::new_memory(10000, 512, false);

        b.iter(|| {
            // 读
            let _ = black_box(block.read(0, 10));

            // 写
            let data = vec![0xABu8; 5120];
            let _ = black_box(block.write(0, &data));

            // 再读
            let _ = black_box(block.read(0, 10));
        });
    });

    // 批量读操作
    group.bench_function("batch_reads", |b| {
        let block = VirtioBlock::new_memory(10000, 512, false);

        b.iter(|| {
            for i in 0..100 {
                let _ = black_box(block.read(i * 10, 10));
            }
        });
    });

    // 批量写操作
    group.bench_function("batch_writes", |b| {
        let mut block = VirtioBlock::new_memory(10000, 512, false);
        let data = vec![0xABu8; 5120];

        b.iter(|| {
            for i in 0..100 {
                let _ = black_box(block.write(i * 10, &data));
            }
        });
    });

    group.finish();
}

/// 基准测试：扇区大小影响
///
/// 测试不同扇区大小对性能的影响
fn bench_sector_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("sector_sizes");

    // 512字节扇区
    let block_512 = VirtioBlock::new_memory(10000, 512, false);
    group.bench_function("512_byte_sector", |b| {
        b.iter(|| black_box(block_512.read(0, 100)));
    });

    // 4096字节扇区
    let block_4096 = VirtioBlock::new_memory(10000, 4096, false);
    group.bench_function("4096_byte_sector", |b| {
        b.iter(|| black_box(block_4096.read(0, 100)));
    });

    group.finish();
}

/// 基准测试：内存分配影响
///
/// 测试不同内存分配模式的性能
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");

    // 小块随机访问
    group.bench_function("random_small_reads", |b| {
        let block = VirtioBlock::new_memory(10000, 512, false);

        b.iter(|| {
            for i in 0..100 {
                let offset = (i * 97) % 10000; // 伪随机偏移
                let _ = black_box(block.read(offset, 1));
            }
        });
    });

    // 大块顺序访问
    group.bench_function("sequential_large_reads", |b| {
        let block = VirtioBlock::new_memory(10000, 512, false);

        b.iter(|| {
            let _ = black_box(block.read(0, 1000));
        });
    });

    group.finish();
}

/// 基准测试：Getter 方法性能
///
/// 测试只读访问方法的性能
fn bench_getter_methods(c: &mut Criterion) {
    let mut group = c.benchmark_group("getter_methods");

    let block = VirtioBlock::new_memory(10000, 512, true);

    group.bench_function("capacity", |b| {
        b.iter(|| black_box(block.capacity()));
    });

    group.bench_function("sector_size", |b| {
        b.iter(|| black_box(block.sector_size()));
    });

    group.bench_function("is_read_only", |b| {
        b.iter(|| black_box(block.is_read_only()));
    });

    group.finish();
}

/// 基准测试：设备创建开销
///
/// 测试创建不同配置设备的开销
fn bench_device_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_creation");

    group.bench_function("new_basic", |b| {
        b.iter(|| black_box(VirtioBlock::new(10000, 512, false)));
    });

    group.bench_function("new_memory", |b| {
        b.iter(|| black_box(VirtioBlock::new_memory(10000, 512, false)));
    });

    group.bench_function("new_large", |b| {
        b.iter(|| black_box(VirtioBlock::new_memory(100000, 4096, false)));
    });

    group.finish();
}

// 配置 Criterion 基准测试参数
criterion_group!(
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3))
        .sample_size(100);
    targets =
        bench_read_operation,
        bench_write_operation,
        bench_validate_read_request,
        bench_validate_write_request,
        bench_process_request,
        bench_error_handling,
        bench_mixed_operations,
        bench_sector_sizes,
        bench_memory_patterns,
        bench_getter_methods,
        bench_device_creation
);

criterion_main!(benches);
