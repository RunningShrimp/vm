use vm_core::{VirtualMachine, MMU, ExecutionEngine, Decoder};
use vm_frontend_riscv64::{RiscvDecoder, api::*};
use vm_engine_interpreter::Interpreter;
use vm_osal::{host_os, host_arch};
use vm_device::virtio::{VirtioBlock, VirtioNet};
use vm_device::gpu::GpuDevice;

fn main() {
    println!("Initializing VM...");
    println!("Host: {} / {}", host_os(), host_arch());

    let mut vm = VirtualMachine::new();
    let mmu_arc = vm.mmu();

    // Initialize Devices
    {
        let mut mmu = mmu_arc.lock().unwrap();
        mmu.map_mmio(0x1000_0000, 0x1000, Box::new(VirtioBlock::new(1024 * 1024)));
        mmu.map_mmio(0x1000_1000, 0x1000, Box::new(VirtioNet::new()));
        mmu.map_mmio(0x1000_2000, 0x1000, Box::new(GpuDevice::new()));
    }

    // 1. Load Program
    let code_base = 0x1000;
    let data_base = 0x100;
    
    let code = vec![
        encode_addi(1, 0, 10),          // 0x1000: li x1, 10
        encode_addi(2, 0, 20),          // 0x1004: li x2, 20
        encode_add(3, 1, 2),            // 0x1008: add x3, x1, x2
        encode_addi(10, 0, data_base),  // 0x100C: li x10, 0x100
        encode_sw(10, 3, 0),            // 0x1010: sw x3, 0(x10)
        encode_lw(4, 10, 0),            // 0x1014: lw x4, 0(x10)
        encode_beq(3, 4, 8),            // 0x1018: beq x3, x4, +8 (-> 0x1020)
        encode_addi(5, 0, 1),           // 0x101C: li x5, 1 (skipped)
        encode_addi(6, 0, 2),           // 0x1020: li x6, 2
        encode_jal(0, 0),               // 0x1024: j . (halt)
    ];

    {
        let mut mmu = mmu_arc.lock().unwrap();
        for (i, insn) in code.iter().enumerate() {
            mmu.write(code_base + (i as u64 * 4), *insn as u64, 4).unwrap();
        }
    }

    println!("Program loaded. Starting execution (Interpreter)...");

    let mut decoder = RiscvDecoder;
    let mut interp = Interpreter::new();
    interp.set_reg(0, 0); // x0 = 0
    
    let mut pc = code_base;
    
    // Simple run loop
    for _ in 0..20 {
        let mut mmu = mmu_arc.lock().unwrap();
        match decoder.decode(&*mmu, pc) {
            Ok(block) => {
                let _res = interp.run(&mut *mmu, &block);
                // Simplified PC update
                match block.terminator {
                    vm_ir::Terminator::Jmp { target } => pc = target,
                    vm_ir::Terminator::CondJmp { cond, target_true, target_false } => {
                        if interp.get_reg(cond) != 0 { pc = target_true; } else { pc = target_false; }
                    }
                    _ => pc += 4,
                }
                if pc == code_base + (code.len() as u64 * 4) - 4 { break; } // Halt loop
            }
            Err(e) => {
                println!("Decode error at {:#x}: {:?}", pc, e);
                break;
            }
        }
    }
    
    println!("Execution finished. x3 = {}", interp.get_reg(3));
}
