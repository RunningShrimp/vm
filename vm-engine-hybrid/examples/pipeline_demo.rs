use vm_engine_hybrid::HybridEngine;
use vm_ir::{IRBuilder, IROp, Terminator};
use vm_mem::SoftMmu;

fn main() {
    let mut engine = HybridEngine::new();
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let mut builder = IRBuilder::new(0x1000);
    builder.push(IROp::MovImm { dst: 1, imm: 100 });
    builder.push(IROp::MovImm { dst: 2, imm: 5 });
    builder.push(IROp::Add {
        dst: 3,
        src1: 1,
        src2: 2,
    });
    builder.push(IROp::AddImm {
        dst: 4,
        src: 3,
        imm: 7,
    });
    builder.push(IROp::SllImm {
        dst: 5,
        src: 4,
        sh: 2,
    });
    builder.push(IROp::Store {
        src: 5,
        base: 1,
        offset: 64,
        size: 8,
        flags: vm_ir::MemFlags::default(),
    });
    builder.push(IROp::Load {
        dst: 6,
        base: 1,
        offset: 64,
        size: 8,
        flags: vm_ir::MemFlags::default(),
    });
    builder.push(IROp::CmpEq {
        dst: 7,
        lhs: 5,
        rhs: 6,
    });
    builder.set_term(Terminator::CondJmp {
        cond: 7,
        target_true: 0x1008,
        target_false: 0x1010,
    });
    let block = builder.build();

    for _ in 0..200 {
        let _ = engine.run(&mut mmu, &block);
    }

    println!(
        "R5={} R6={} R7={}",
        engine.get_reg(5),
        engine.get_reg(6),
        engine.get_reg(7)
    );
    engine.print_stats();
}
