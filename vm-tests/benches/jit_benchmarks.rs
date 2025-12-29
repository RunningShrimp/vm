//! JIT性能基准测试
//!
//! 测量JIT编译器的编译速度和执行性能

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::time::Duration;

use vm_core::{Block, ExecutionEngine};
use vm_engine::jit::JitEngine;

fn jit_compilation_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_compilation");
    group.measurement_time(Duration::from_secs(10));

    // 测试不同大小的代码块编译性能
    for size in [10, 100, 1000, 10000].iter() {
        let test_code: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();

        group.bench_with_input(BenchmarkId::new("compile_bytes", size), size, |b, _| {
            let mut jit = JitEngine::new();

            b.iter(|| {
                let block = Block::new(test_code.clone());
                black_box(jit.compile(black_box(&block)))
            });
        });
    }

    group.finish();
}

fn jit_execution_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_execution");
    group.measurement_time(Duration::from_secs(5));

    // 创建简单的测试代码：累加循环
    let simple_code = vec![
        0x48, 0x31, 0xC0, // xor rax, rax
        0x48, 0xC7, 0xC1, 0x64, 0x00, 0x00, 0x00, // mov rcx, 100
        // loop:
        0x48, 0xFF, 0xC0, // inc rax
        0x48, 0xFF, 0xC9, // dec rcx
        0x75, 0xFC, // jnz loop
        0xC3, // ret
    ];

    // 创建复杂一点的测试代码：内存操作
    let memory_code = vec![
        0x48, 0x31, 0xC0, // xor rax, rax
        0x48, 0xB8, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, // mov rax, 0x100000
        // loop:
        0x48, 0x8B, 0x08, // mov rcx, [rax]
        0x48, 0x01, 0xC8, // add rax, rcx
        0x48, 0x3D, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, // cmp rax, 0x200000
        0x72, 0xF4, // jb loop
        0xC3, // ret
    ];

    let test_cases = vec![("simple_loop", simple_code), ("memory_ops", memory_code)];

    for (name, code) in test_cases {
        let mut jit = JitEngine::new();
        let block = Block::new(code);
        let compiled_fn = jit.compile(&block).expect("Failed to compile");

        group.bench_with_input(BenchmarkId::new("execution", name), name, |b, _| {
            b.iter(|| black_box(jit.execute(black_box(compiled_fn))));
        });
    }

    group.finish();
}

fn jit_vs_interpreter_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_vs_interpreter");
    group.measurement_time(Duration::from_secs(10));

    // 创建测试代码：计算斐波那契数列
    let fib_code = vec![
        0x55, // push rbp
        0x48, 0x89, 0xE5, // mov rbp, rsp
        0x48, 0x83, 0xEC, 0x10, // sub rsp, 16
        0x48, 0x89, 0x7D, 0xF8, // mov [rbp-8], rdi
        0x48, 0x83, 0x7D, 0xF8, 0x02, // cmp qword [rbp-8], 2
        0x7C, 0x1B, // jl base_case
        0x48, 0x8B, 0x45, 0xF8, // mov rax, [rbp-8]
        0x48, 0x83, 0xE8, 0x01, // sub rax, 1
        0x48, 0x89, 0xC7, // mov rdi, rax
        0xE8, 0x00, 0x00, 0x00, 0x00, // call fib
        0x48, 0x89, 0x45, 0xF0, // mov [rbp-16], rax
        0x48, 0x8B, 0x45, 0xF8, // mov rax, [rbp-8]
        0x48, 0x83, 0xE8, 0x02, // sub rax, 2
        0x48, 0x89, 0xC7, // mov rdi, rax
        0xE8, 0x00, 0x00, 0x00, 0x00, // call fib
        0x48, 0x03, 0x45, 0xF0, // add rax, [rbp-16]
        0xEB, 0x05, // jmp end
        // base_case:
        0x48, 0xB8, 0x01, 0x00, 0x00, 0x00, // mov rax, 1
        // end:
        0x48, 0x83, 0xC4, 0x10, // add rsp, 16
        0x5D, // pop rbp
        0xC3, // ret
    ];

    let block = Block::new(fib_code);

    // JIT基准测试
    group.bench_function("jit_fibonacci", |b| {
        let mut jit = JitEngine::new();
        let compiled_fn = jit.compile(&block).expect("Failed to compile");

        b.iter(|| black_box(jit.execute(black_box(compiled_fn))));
    });

    // 解释器基准测试（如果可用）
    #[cfg(feature = "interpreter")]
    {
        use vm_engine::interpreter::InterpreterEngine;

        group.bench_function("interpreter_fibonacci", |b| {
            let mut interpreter = InterpreterEngine::new();

            b.iter(|| black_box(interpreter.execute(black_box(&block))));
        });
    }

    group.finish();
}

fn jit_hotspot_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit_hotspot");
    group.measurement_time(Duration::from_secs(15));

    // 创建会被频繁执行的代码块
    let hotspot_code = vec![
        0x48, 0x31, 0xC0, // xor rax, rax
        0x48, 0xC7, 0xC1, 0x0A, 0x00, 0x00, 0x00, // mov rcx, 10
        // loop:
        0x48, 0x83, 0xC0, 0x01, // add rax, 1
        0x48, 0xFF, 0xC9, // dec rcx
        0x75, 0xF8, // jnz loop
        0xC3, // ret
    ];

    let mut jit = JitEngine::new();
    let block = Block::new(hotspot_code);
    let compiled_fn = jit.compile(&block).expect("Failed to compile");

    group.bench_function("hotspot_execution", |b| {
        b.iter(|| {
            // 模拟热点代码执行
            for _ in 0..100 {
                black_box(jit.execute(black_box(compiled_fn)));
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    jit_compilation_benchmark,
    jit_execution_benchmark,
    jit_vs_interpreter_benchmark,
    jit_hotspot_benchmark
);

criterion_main!(benches);
