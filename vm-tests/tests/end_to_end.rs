use vm_core::{MMU, MmioDevice};
use vm_device::block::{VirtioBlock, VirtioBlockMmio};
use vm_mem::SoftMmu;

#[test]
fn smoke_mmio_notify_block_device() {
    let mut mmu = SoftMmu::new(1024 * 1024, false);

    // Create a VirtioBlock device without a backing file; queue not configured
    let virtio = VirtioBlock::new();
    let mmio = VirtioBlockMmio::new(virtio);

    // Map MMIO region at 0x1000
    mmu.map_mmio(0x1000, 0x1000, Box::new(mmio));

    // Write to offset 0x20 to trigger notify; should not panic
    mmu.write(0x1020, 0x1, 4).expect("Failed to write to MMIO");
}

#[test]
fn test_riscv64_simple_execution() {
    use vm_core::{Decoder, ExecMode, ExecutionEngine, GuestArch, VirtualMachine, VmConfig};
    use vm_engine::interpreter::Interpreter;
    use vm_frontend_riscv64::RiscvDecoder;
    use vm_ir::IRBlock;

    // 1. Setup VM
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 1024 * 1024, // 1MB
        vcpu_count: 1,
        exec_mode: ExecMode::Interpreter,
        ..Default::default()
    };

    let mmu = SoftMmu::new(config.memory_size, false);
    let mut vm: VirtualMachine<IRBlock> = VirtualMachine::with_mmu(config, Box::new(mmu));
    let mmu_arc = vm.mmu();

    // 2. Load "Kernel" (Machine Code)
    // li x1, 42           => 0x02a00093
    // li x2, 0x100        => 0x10000113 (addi x2, x0, 256)
    // sw x1, 0(x2)        => 0x00112023
    // ebreak              => 0x00100073
    let code: [u8; 16] = [
        0x93, 0x00, 0xa0, 0x02, // li x1, 42
        0x13, 0x01, 0x00, 0x10, // li x2, 256 (0x100)
        0x23, 0x20, 0x11, 0x00, // sw x1, 0(x2)
        0x73, 0x00, 0x10, 0x00, // ebreak
    ];

    let entry_point = 0x0; // Load at 0x0 for simplicity in this test
    vm.load_kernel(&code, entry_point)
        .expect("Failed to load kernel");

    // 3. Run Loop
    let mut decoder = RiscvDecoder;
    let mut interp = Interpreter::new();
    let mut pc = entry_point;

    // Run for a few steps
    for _ in 0..10 {
        let mut mmu = mmu_arc.lock().expect("Failed to acquire MMU lock");
        let block = decoder.decode(mmu.as_ref(), pc).expect("Decode failed");

        // Execute
        let res = interp.run(mmu.as_mut(), &block);

        // Update PC
        match block.term {
            vm_ir::Terminator::Jmp { target } => pc = target,
            vm_ir::Terminator::Ret => break,
            _ => pc = res.next_pc,
        }

        if pc >= entry_point + 12 {
            break; // Reached ebreak
        }
    }

    // 4. Verify Result
    let mmu = mmu_arc.lock().expect("Failed to acquire MMU lock");
    let val = mmu.read(0x100, 8).expect("Read failed");
    assert_eq!(val, 42, "Memory at 0x100 should be 42");
}
