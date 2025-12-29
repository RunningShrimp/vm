// VM Performance Benchmark Suite
//
// This benchmark suite measures the performance of critical VM components:
// - JIT compilation (L1/L2/L3 tiers)
// - TLB lookup
// - Memory allocation
// - Instruction decoding
//
// Performance targets (based on implementation plan):
// - JIT compile L1: < 1ms
// - JIT compile L3: < 100ms
// - TLB hit: < 5 ns
// - TLB miss: < 50 ns
// - TLB hit rate: > 95%
// - x86_64 decode: < 100 ns
// - 4KB page alloc: < 1 μs

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use vm_core::{GuestAddr, GuestArch};
use vm_engine::jit::{Jit, CodePtr};
use vm_mem::{SoftMMU, PagingMode};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Benchmark JIT compilation performance at different tiers
fn bench_jit_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_compile");

    // L1 (Cold code): < 1ms target
    group.bench_function("L1_cold_10_instrs", |b| {
        let code = generate_test_code(10);
        b.iter(|| {
            let mut jit = Jit::new();
            jit.compile(black_box(&code))
        });
    });

    // L2 (Warm code): < 10ms target
    group.bench_function("L2_warm_100_instrs", |b| {
        let code = generate_test_code(100);
        b.iter(|| {
            let mut jit = Jit::new();
            jit.compile(black_box(&code))
        });
    });

    // L3 (Hot code): < 100ms target
    group.bench_function("L3_hot_1000_instrs", |b| {
        let code = generate_test_code(1000);
        b.iter(|| {
            let mut jit = Jit::new();
            jit.compile(black_box(&code))
        });
    });

    group.finish();
}

/// Benchmark TLB lookup performance
fn bench_tlb_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("tlb");

    // TLB hit: < 5 ns target
    group.bench_function("hit", |b| {
        let mut tlb = vm_mem::unified_tlb::UnifiedTlb::new(64);
        tlb.insert(GuestAddr(0x1000), GuestAddr(0x2000), vm_mem::PageFlags::RW);

        b.iter(|| {
            black_box(tlb.lookup(black_box(GuestAddr(0x1000)), vm_core::AccessType::Read))
        });
    });

    // TLB miss: < 50 ns target (without page table walk)
    group.bench_function("miss", |b| {
        let tlb = vm_mem::unified_tlb::UnifiedTlb::new(64);

        b.iter(|| {
            black_box(tlb.lookup(black_box(GuestAddr(0x9999)), vm_core::AccessType::Read))
        });
    });

    group.finish();
}

/// Benchmark instruction decoding performance
fn bench_instruction_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode");

    // x86_64: < 100 ns target
    group.bench_function("x86_64", |b| {
        let mut decoder = vm_frontend::x86_64::X86Decoder::new();
        let code = vec![0x48, 0x89, 0xd8]; // MOV %rbx, %rax

        b.iter(|| {
            black_box(decoder.decode_insn(black_box(&code), GuestAddr(0)))
        });
    });

    // ARM64: < 80 ns target
    group.bench_function("arm64", |b| {
        let mut decoder = vm_frontend::arm64::Arm64Decoder::new();
        let code = vec![0x00, 0x00, 0x10, 0xb5]; // BL instruction

        b.iter(|| {
            black_box(decoder.decode_insn(black_box(&code), GuestAddr(0)))
        });
    });

    // RISC-V64: < 70 ns target
    group.bench_function("riscv64", |b| {
        let decoder = vm_frontend::riscv64::RiscvDecoder;
        let code = vec![0x33, 0x05, 0xb0, 0x00]; // ADD a0, a1, a2

        b.iter(|| {
            black_box(decoder.decode_insn(black_box(&code), GuestAddr(0)))
        });
    });

    group.finish();
}

/// Benchmark memory allocation performance
fn bench_memory_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_alloc");

    // 4KB page: < 1 μs target
    group.bench_function("4kb_page", |b| {
        let allocator = vm_mem::NumaAllocator::new(vm_mem::NumaAllocPolicy::Local);

        b.iter(|| {
            black_box(allocator.allocate_page(black_box(4096)))
        });
    });

    // 2MB huge page: < 5 μs target
    group.bench_function("2mb_hugepage", |b| {
        let allocator = vm_mem::NumaAllocator::new(vm_mem::NumaAllocPolicy::Local);

        b.iter(|| {
            black_box(allocator.allocate_page(black_box(2 * 1024 * 1024)))
        });
    });

    group.finish();
}

/// Benchmark cross-arch translation
fn bench_cross_arch_translation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cross_arch");

    // x86_64 -> ARM64
    group.bench_function("x86_64_to_arm64", |b| {
        use vm_cross_arch::ArchTranslator;
        use vm_ir::IRBlock;

        let translator = ArchTranslator::new(
            vm_cross_arch::SourceArch::X86_64,
            vm_cross_arch::TargetArch::ARM64,
        );

        let source_block = IRBlock {
            instructions: vec![],
            terminator: vm_ir::Terminator::Ret,
        };

        b.iter(|| {
            black_box(translator.translate_block(black_box(&source_block)))
        });
    });

    group.finish();
}

/// Helper function to generate test code for JIT compilation
fn generate_test_code(size: usize) -> Vec<u8> {
    // Simple RISC-V instructions for testing
    let mut code = Vec::with_capacity(size * 4);
    for i in 0..size {
        // ADDI x1, x2, i
        code.push(0x93); // ADDI opcode
        code.push(0x05); // x1, x2
        code.push((i & 0xFF) as u8);
        code.push(((i >> 8) & 0xFF) as u8);
    }
    code
}

criterion_group!(
    benches,
    bench_jit_compilation,
    bench_tlb_lookup,
    bench_instruction_decode,
    bench_memory_allocation,
    bench_cross_arch_translation
);

criterion_main!(benches);
