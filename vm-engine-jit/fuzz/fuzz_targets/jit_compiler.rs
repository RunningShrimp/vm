#![no_main]
use libfuzzer_sys::fuzz_target;
use vm_engine_jit::TieredJITCompiler;
use vm_engine_jit::tiered_compiler::TieredCompilerConfig;
use vm_ir::{IRBlock, IROp, Terminator};
use vm_core::GuestAddr;

fuzz_target!(|data: &[u8]| {
    let config = TieredCompilerConfig::default();
    let mut compiler = TieredJITCompiler::new(config);
    
    if data.len() < 4 {
        return;
    }
    
    let start_pc = GuestAddr(u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as u64);
    
    let mut ops = Vec::new();
    let mut idx = 4;
    
    while idx + 8 <= data.len() && ops.len() < 100 {
        let opcode = data[idx];
        idx += 1;
        
        match opcode % 5 {
            0 => {
                if idx + 9 <= data.len() {
                    ops.push(IROp::MovImm {
                        rd: data[idx],
                        imm: u64::from_le_bytes([
                            data[idx + 1], data[idx + 2], data[idx + 3], data[idx + 4],
                            data[idx + 5], data[idx + 6], data[idx + 7], data[idx + 8],
                        ]),
                    });
                    idx += 9;
                }
            },
            1 => {
                if idx + 3 <= data.len() {
                    ops.push(IROp::Add {
                        rd: data[idx],
                        rs1: data[idx + 1],
                        rs2: data[idx + 2],
                    });
                    idx += 3;
                }
            },
            2 => {
                if idx + 3 <= data.len() {
                    ops.push(IROp::Sub {
                        rd: data[idx],
                        rs1: data[idx + 1],
                        rs2: data[idx + 2],
                    });
                    idx += 3;
                }
            },
            3 => {
                if idx + 6 <= data.len() {
                    let size = match data[idx + 1] % 4 {
                        0 => 8,
                        1 => 16,
                        2 => 32,
                        _ => 64,
                    };
                    ops.push(IROp::Load {
                        rd: data[idx],
                        addr: u32::from_le_bytes([data[idx + 2], data[idx + 3], data[idx + 4], data[idx + 5]]) as u64,
                        size,
                    });
                    idx += 6;
                }
            },
            _ => {
                if idx + 6 <= data.len() {
                    let size = match data[idx + 1] % 4 {
                        0 => 8,
                        1 => 16,
                        2 => 32,
                        _ => 64,
                    };
                    ops.push(IROp::Store {
                        rs: data[idx],
                        addr: u32::from_le_bytes([data[idx + 2], data[idx + 3], data[idx + 4], data[idx + 5]]) as u64,
                        size,
                    });
                    idx += 6;
                }
            }
        }
    }
    
    if !ops.is_empty() {
        let block = IRBlock {
            start_pc,
            ops,
            term: Terminator::Return,
        };
        
        let _ = compiler.execute(&block);
    }
});
