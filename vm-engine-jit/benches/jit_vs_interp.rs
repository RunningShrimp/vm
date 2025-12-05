use criterion::{Criterion, black_box, criterion_group, criterion_main};
use vm_core::ExecutionEngine;
use vm_engine_interpreter::Interpreter;
use vm_engine_jit::Jit;
use vm_ir::{IRBlock, IROp, Terminator};
use vm_mem::SoftMmu;

fn bench_simple_add(c: &mut Criterion) {
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let block = IRBlock {
        start_pc: 0x7000,
        ops: vec![
            IROp::AddImm {
                dst: 2,
                src: 2,
                imm: 1,
            },
            IROp::AddImm {
                dst: 3,
                src: 3,
                imm: 2,
            },
            IROp::Add {
                dst: 4,
                src1: 2,
                src2: 3,
            },
        ],
        term: Terminator::Jmp { target: 0x7000 },
    };

    c.bench_function("interpreter_add_loop", |b| {
        let mut interp = Interpreter::new();
        b.iter(|| {
            let _ = interp.run(&mut mmu, &block);
            black_box(interp.get_reg(4));
        });
    });

    c.bench_function("jit_add_loop", |b| {
        let mut jit = Jit::new();
        jit.set_pc(block.start_pc);
        // Drive to compile
        for _ in 0..vm_engine_jit::HOT_THRESHOLD {
            let _ = jit.run(&mut mmu, &block);
        }
        b.iter(|| {
            let _res = jit.run(&mut mmu, &block);
            black_box(jit.get_reg(4));
        });
    });
}

criterion_group!(benches, bench_simple_add);
criterion_main!(benches);
