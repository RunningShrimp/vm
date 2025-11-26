use vm_core::{MMU, GuestAddr};
use vm_mem::SoftMMU;
use vm_frontend_riscv64::{RiscvDecoder, api::*};
use vm_engine_interpreter::{Interpreter, run_chain};

fn main() {
    println!("Initializing VM...");

    // 1. Setup Memory
    let mut mmu = SoftMMU::new();
    
    // 2. Load Program
    // x1 = 10
    // x2 = 20
    // x3 = x1 + x2 = 30
    // Store x3 to [0x100]
    // Load x4 from [0x100]
    // if x3 == x4 goto end
    // x5 = 1 (should skip)
    // end: x6 = 2
    
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

    for (i, insn) in code.iter().enumerate() {
        mmu.write(code_base + (i as u64 * 4), *insn as u64, 4).unwrap();
    }

    // 3. Setup Engine
    let mut decoder = RiscvDecoder;
    let mut interp = Interpreter::new();
    
    println!("Starting execution at 0x{:x}", code_base);
    
    // 4. Run
    // Run for a few blocks
    let res = run_chain(&mut decoder, &mut mmu, &mut interp, code_base, 10);
    
    println!("Execution finished: {:?}", res);
    
    // 5. Verify State
    println!("Registers:");
    for i in 0..11 {
        println!("x{}: {}", i, interp.get_reg(i));
    }
    
    let mem_val = mmu.read(data_base as u64, 4).unwrap();
    println!("Memory[0x{:x}]: {}", data_base, mem_val);
    
    assert_eq!(interp.get_reg(3), 30);
    assert_eq!(interp.get_reg(4), 30);
    assert_eq!(interp.get_reg(5), 0); // Skipped
    assert_eq!(interp.get_reg(6), 2); // Executed
    assert_eq!(mem_val, 30);
    
    println!("Test Passed!");
}
