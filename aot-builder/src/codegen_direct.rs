//! 直接代码生成（不通过LLVM）
//!
//! 从lib.rs中提取的代码生成逻辑，提升可维护性

use vm_ir::{IROp, IRBlock, RegId, Terminator};
use vm_ir_lift::ISA;
use std::collections::HashMap;

/// 直接代码生成（从 IR 指令生成机器码）
pub fn generate_direct_code(ir_instructions: &[String], target_isa: ISA) -> Vec<u8> {
    let mut code = Vec::new();

    // 根据目标 ISA 生成代码
    match target_isa {
        ISA::X86_64 => generate_x86_64_code(ir_instructions, &mut code),
        ISA::ARM64 => generate_arm64_code(ir_instructions, &mut code),
        ISA::RISCV64 => generate_riscv64_code(ir_instructions, &mut code),
    }

    code
}

/// 生成 x86-64 代码
fn generate_x86_64_code(ir_instructions: &[String], code: &mut Vec<u8>) {
    for ir in ir_instructions {
        // 简单的 IR 到 x86-64 映射
        if ir.contains("add") {
            // add i64 %a, %b -> mov %rax, %rbx; add %rax, %rbx
            code.extend_from_slice(&[0x48, 0x89, 0xD8]); // mov rax, rbx
            code.extend_from_slice(&[0x48, 0x01, 0xD8]); // add rax, rbx
        } else if ir.contains("sub") {
            code.extend_from_slice(&[0x48, 0x29, 0xD8]); // sub rax, rbx
        } else if ir.contains("mul") {
            code.extend_from_slice(&[0x48, 0xF7, 0xE3]); // mul rbx
        } else if ir.contains("ret") {
            code.push(0xC3); // ret
        } else {
            // 默认生成 NOP
            code.push(0x90); // nop
        }
    }

    // 如果没有显式返回，添加 ret
    if !code.ends_with(&[0xC3]) {
        code.push(0xC3); // ret
    }
}

/// 生成 ARM64 代码
fn generate_arm64_code(ir_instructions: &[String], code: &mut Vec<u8>) {
    for ir in ir_instructions {
        if ir.contains("add") {
            // add -> ADD X0, X1, X2 (简化)
            code.extend_from_slice(&[0x00, 0x00, 0x02, 0x8B]); // add x0, x1, x2
        } else if ir.contains("ret") {
            code.extend_from_slice(&[0xC0, 0x03, 0x5F, 0xD6]); // ret
        } else {
            // NOP 等价指令
            code.extend_from_slice(&[0x1F, 0x20, 0x03, 0xD5]); // nop
        }
    }

    // 如果没有显式返回，添加 ret
    if !code.ends_with(&[0xC0, 0x03, 0x5F, 0xD6]) {
        code.extend_from_slice(&[0xC0, 0x03, 0x5F, 0xD6]); // ret
    }
}

/// 生成 RISC-V64 代码
fn generate_riscv64_code(ir_instructions: &[String], code: &mut Vec<u8>) {
    for ir in ir_instructions {
        if ir.contains("add") {
            // add -> ADD x0, x1, x2 (简化)
            code.extend_from_slice(&[0x33, 0x00, 0x20, 0x00]); // add x0, x1, x2
        } else if ir.contains("ret") {
            code.extend_from_slice(&[0x67, 0x80, 0x00, 0x00]); // ret (jalr x0, 0(x1))
        } else {
            // NOP 等价指令
            code.extend_from_slice(&[0x13, 0x00, 0x00, 0x00]); // nop (addi x0, x0, 0)
        }
    }

    // 如果没有显式返回，添加 ret
    if !code.ends_with(&[0x67, 0x80, 0x00, 0x00]) {
        code.extend_from_slice(&[0x67, 0x80, 0x00, 0x00]); // ret
    }
}

/// 编码 x86-64 寄存器到 ModR/M 字节
pub fn encode_x86_64_reg(reg: RegId) -> u8 {
    (reg & 0x7) as u8
}

/// 编码 ARM64 寄存器
pub fn encode_arm64_reg(reg: RegId) -> u32 {
    reg & 0x1F
}

/// 编码 RISC-V64 寄存器
pub fn encode_riscv64_reg(reg: RegId) -> u32 {
    reg & 0x1F
}

/// 编译IRBlock到机器码（直接模式）
/// 
/// 这是从lib.rs中提取的主要代码生成接口
pub fn compile_ir_block_direct(
    block: &IRBlock,
    target_isa: ISA,
    optimization_level: u32,
) -> Result<Vec<u8>, String> {
    let mut code = Vec::new();
    
    // 编译每个IR操作
    for op in &block.ops {
        let op_code = compile_ir_op(op, target_isa)?;
        code.extend_from_slice(&op_code);
    }
    
    // 编译终结符
    let term_code = compile_terminator(&block.term, target_isa)?;
    code.extend_from_slice(&term_code);
    
    Ok(code)
}

/// 编译IR操作到机器码
pub fn compile_ir_op(op: &IROp, target_isa: ISA) -> Result<Vec<u8>, String> {
    match target_isa {
        ISA::X86_64 => compile_ir_op_x86_64(op),
        ISA::ARM64 => compile_ir_op_arm64(op),
        ISA::RISCV64 => compile_ir_op_riscv64(op),
    }
}

/// 编译IR操作到x86-64机器码
/// 
/// 注意：这是一个简化实现，完整的实现在lib.rs中
/// 为了拆分，这里提供一个基础实现
fn compile_ir_op_x86_64(op: &IROp) -> Result<Vec<u8>, String> {
    let mut code = Vec::new();
    match op {
        IROp::Add { dst, src1, src2 } => {
            let dst_reg = encode_x86_64_reg(*dst);
            let src1_reg = encode_x86_64_reg(*src1);
            code.push(0x48); // REX.W
            code.push(0x89); // mov
            code.push(0xC0 | (src1_reg << 3) | dst_reg);
            let src2_reg = encode_x86_64_reg(*src2);
            code.push(0x48); // REX.W
            code.push(0x01); // add
            code.push(0xC0 | (src2_reg << 3) | dst_reg);
        }
        IROp::Sub { dst, src1, src2 } => {
            let dst_reg = encode_x86_64_reg(*dst);
            let src1_reg = encode_x86_64_reg(*src1);
            code.push(0x48);
            code.push(0x89); // mov
            code.push(0xC0 | (src1_reg << 3) | dst_reg);
            let src2_reg = encode_x86_64_reg(*src2);
            code.push(0x48);
            code.push(0x29); // sub
            code.push(0xC0 | (src2_reg << 3) | dst_reg);
        }
        _ => {
            // 其他操作暂时返回NOP
            code.push(0x90); // nop
        }
    }
    Ok(code)
}

/// 编译IR操作到ARM64机器码
fn compile_ir_op_arm64(op: &IROp) -> Result<Vec<u8>, String> {
    let mut code = Vec::new();
    match op {
        IROp::Add { dst, src1, src2 } => {
            let dst_reg = encode_arm64_reg(*dst);
            let src1_reg = encode_arm64_reg(*src1);
            let src2_reg = encode_arm64_reg(*src2);
            // ADD指令编码（简化）
            let inst = 0x8B000000u32 | (dst_reg << 0) | (src1_reg << 5) | (src2_reg << 16);
            code.extend_from_slice(&inst.to_le_bytes());
        }
        _ => {
            // NOP
            code.extend_from_slice(&[0x1F, 0x20, 0x03, 0xD5]);
        }
    }
    Ok(code)
}

/// 编译IR操作到RISC-V64机器码
fn compile_ir_op_riscv64(op: &IROp) -> Result<Vec<u8>, String> {
    let mut code = Vec::new();
    match op {
        IROp::Add { dst, src1, src2 } => {
            let dst_reg = encode_riscv64_reg(*dst);
            let src1_reg = encode_riscv64_reg(*src1);
            let src2_reg = encode_riscv64_reg(*src2);
            // ADD指令编码（简化）
            let inst = 0x33u32 | (dst_reg << 7) | (src1_reg << 15) | (src2_reg << 20);
            code.extend_from_slice(&inst.to_le_bytes());
        }
        _ => {
            // NOP (addi x0, x0, 0)
            code.extend_from_slice(&[0x13, 0x00, 0x00, 0x00]);
        }
    }
    Ok(code)
}

/// 编译终结符到机器码
fn compile_terminator(term: &Terminator, target_isa: ISA) -> Result<Vec<u8>, String> {
    match target_isa {
        ISA::X86_64 => {
            match term {
                Terminator::Jmp { target: _ } => Ok(vec![0xC3]), // ret (简化)
                Terminator::Ret => Ok(vec![0xC3]), // ret
                _ => Ok(vec![0xC3]), // 默认返回
            }
        }
        ISA::ARM64 => {
            match term {
                Terminator::Ret => Ok(vec![0xC0, 0x03, 0x5F, 0xD6]), // ret
                _ => Ok(vec![0xC0, 0x03, 0x5F, 0xD6]),
            }
        }
        ISA::RISCV64 => {
            match term {
                Terminator::Ret => Ok(vec![0x67, 0x80, 0x00, 0x00]), // ret
                _ => Ok(vec![0x67, 0x80, 0x00, 0x00]),
            }
        }
    }
}

