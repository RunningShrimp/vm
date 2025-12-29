//! JIT 性能基准测试

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::time::Duration;
use vm_core::{GuestAddr, GuestArch};
use vm_engine_jit::{JITCompilationConfig, JITEngine, JITExecutionStats, core::IRBlock};

fn create_test_ir_block(instruction_count: usize) -> IRBlock {
    let mut instructions = Vec::with_capacity(instruction_count);

    for i in 0..instruction_count {
        instructions.push(vm_engine_jit::core::IRInstruction::Const {
            dest: (i % 32) as u32,
            value: (i * 42) as u64,
        });
    }

    for i in 0..instruction_count {
        instructions.push(vm_engine_jit::core::IRInstruction::BinaryOp {
            op: vm_engine_jit::core::BinaryOperator::Add,
            dest: (i % 32) as u32,
            src1: (i % 16) as u32,
            src2: ((i + 1) % 16) as u32,
        });
    }

    instructions.push(vm_engine_jit::core::IRInstruction::Return { value: 0 });

    IRBlock { instructions }
}

fn bench_jit_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_compilation");
    group.measurement_time(Duration::from_secs(10));

    for size in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let config = JITCompilationConfig::default();
            let mut engine = JITEngine::new(config);
            let ir_block = create_test_ir_block(size);

            b.iter(|| {
                engine.compile_block(black_box(&ir_block)).unwrap();
            });
        });
    }

    group.finish();
}

fn bench_tlb_lookup(c: &mut Criterion) {
    use vm_core::AccessType;
    use vm_mem::{
        GuestPhysAddr, MultiLevelTlb,
        tlb::{TlbConfig, TlbEntry},
    };

    let mut group = c.benchmark_group("tlb_lookup");
    group.measurement_time(Duration::from_secs(10));

    let config = TlbConfig::default();
    let mut tlb = MultiLevelTlb::new(config);

    for i in 0..1000 {
        let entry = TlbEntry {
            guest_addr: GuestAddr(i * 0x1000),
            phys_addr: GuestPhysAddr(i * 0x1000),
            asid: i % 10,
            flags: vm_mem::mmu::PageTableFlags::default(),
        };
        tlb.update(entry);
    }

    group.bench_function("sequential", |b| {
        b.iter(|| {
            for i in 0..1000 {
                black_box(tlb.lookup(GuestAddr(i * 0x1000), i % 10, AccessType::Read));
            }
        });
    });

    group.bench_function("random", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let idx = ((i * 7) % 1000) as usize;
                black_box(tlb.lookup(GuestAddr(idx * 0x1000), idx % 10, AccessType::Read));
            }
        });
    });

    group.bench_function("miss", |b| {
        b.iter(|| {
            for i in 0..1000 {
                black_box(tlb.lookup(GuestAddr(i * 0x1000 + 0x8000), i % 10, AccessType::Read));
            }
        });
    });

    group.finish();
}

fn bench_instruction_decoding(c: &mut Criterion) {
    use vm_core::Decoder;

    let mut group = c.benchmark_group("instruction_decoding");
    group.measurement_time(Duration::from_secs(10));

    let riscv_decoder = Decoder::new(GuestArch::Riscv64);

    let instructions: Vec<u32> = (0..1000)
        .map(|i| 0x13 | ((i % 32) << 7) | ((i % 32) << 15))
        .collect();

    group.bench_function("riscv_single", |b| {
        b.iter(|| {
            for &instr in &instructions {
                black_box(riscv_decoder.decode(&instr.to_le_bytes()));
            }
        });
    });

    group.bench_function("riscv_batch", |b| {
        b.iter(|| {
            let mut decoded = Vec::with_capacity(instructions.len());
            for &instr in &instructions {
                if let Ok(decoded_instr) = riscv_decoder.decode(&instr.to_le_bytes()) {
                    decoded.push(decoded_instr);
                }
            }
            black_box(decoded);
        });
    });

    group.finish();
}

fn bench_memory_operations(c: &mut Criterion) {
    use vm_mem::SoftwareMmu;

    let mut group = c.benchmark_group("memory_operations");
    group.measurement_time(Duration::from_secs(10));

    let memory = vec![0u8; 16 * 1024 * 1024];
    let memory_arc = std::sync::Arc::new(std::sync::Mutex::new(memory));

    let mut mmu = SoftwareMmu::new(vm_mem::mmu::MmuArch::RiscVSv39, {
        let memory_clone = std::sync::Arc::clone(&memory_arc);
        move |addr: GuestAddr, size: usize| -> Result<Vec<u8>, vm_core::VmError> {
            let mem = memory_clone.lock().unwrap();
            let start = addr.0 as usize;
            let end = (start + size).min(mem.len());
            Ok(mem[start..end].to_vec())
        }
    });

    group.bench_function("read_single", |b| {
        b.iter(|| {
            black_box(mmu.read(GuestAddr(0x1000), 8));
        });
    });

    group.bench_function("write_single", |b| {
        b.iter(|| {
            black_box(mmu.write(GuestAddr(0x1000), 0xDEADBEEF, 8));
        });
    });

    group.bench_function("read_batch", |b| {
        b.iter(|| {
            let mut buffer = vec![0u8; 4096];
            black_box(mmu.read_bulk(GuestAddr(0x1000), &mut buffer));
        });
    });

    group.bench_function("write_batch", |b| {
        let buffer = vec![0u8; 4096];
        b.iter(|| {
            black_box(mmu.write_bulk(GuestAddr(0x1000), &buffer));
        });
    });

    group.finish();
}

fn bench_gc_operations(c: &mut Criterion) {
    use vm_runtime::{GcConfig, GcRuntime};

    let mut group = c.benchmark_group("gc_operations");
    group.measurement_time(Duration::from_secs(10));

    let config = GcConfig::default();
    let gc = GcRuntime::new(config);

    let mut heap = vec![0u8; 1024 * 1024];

    group.bench_function("mark", |b| {
        b.iter(|| {
            let refs: Vec<usize> = (0..100).map(|i| i * 1024).collect();
            gc.mark(&mut heap, &refs);
        });
    });

    group.bench_function("sweep", |b| {
        b.iter(|| {
            gc.sweep(&mut heap);
        });
    });

    group.finish();
}

fn bench_coroutine_scheduling(c: &mut Criterion) {
    use vm_runtime::{CoroutineScheduler, SchedulerConfig};

    let mut group = c.benchmark_group("coroutine_scheduling");
    group.measurement_time(Duration::from_secs(10));

    let config = SchedulerConfig::default();
    let mut scheduler = CoroutineScheduler::new(config);

    group.bench_function("spawn_10", |b| {
        b.iter(|| {
            for i in 0..10 {
                let mut task = Box::pin(async move {
                    tokio::time::sleep(Duration::from_micros(1)).await;
                    i
                });
                scheduler.spawn_coroutine(&mut task);
            }
        });
    });

    group.bench_function("spawn_100", |b| {
        b.iter(|| {
            for i in 0..100 {
                let mut task = Box::pin(async move {
                    tokio::time::sleep(Duration::from_micros(1)).await;
                    i
                });
                scheduler.spawn_coroutine(&mut task);
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_jit_compilation,
    bench_tlb_lookup,
    bench_instruction_decoding,
    bench_memory_operations,
    bench_gc_operations,
    bench_coroutine_scheduling
);
criterion_main!(benches);
