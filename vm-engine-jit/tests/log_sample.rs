use vm_core::ExecutionEngine;
use vm_engine_jit::{HOT_THRESHOLD, Jit};
use vm_ir::{IRBlock, IROp, Terminator};
use vm_mem::SoftMmu;

#[test]
fn jit_log_emits_sample() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();
    let mut mmu = SoftMmu::new(1024 * 1024, false);
    let mut jit = Jit::new();
    let block = IRBlock {
        start_pc: 0x8000,
        ops: vec![IROp::AddImm {
            dst: 2,
            src: 2,
            imm: 1,
        }],
        term: Terminator::Jmp { target: 0x8000 },
    };
    jit.set_pc(block.start_pc);
    for _ in 0..(HOT_THRESHOLD + 300) {
        let _ = jit.run(&mut mmu, &block);
    }
    assert!(jit.total_compiled >= 1);
}
