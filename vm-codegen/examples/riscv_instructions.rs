//! RISC-V指令集定义示例
//!
//! 使用代码生成工具定义RISC-V指令集。

use vm_codegen::{CodegenConfig, InstructionSet, instruction_set, instruction_spec};

fn main() {
    // 创建RISC-V指令集
    let mut riscv_set = instruction_set!(
        "RISC-V",
        // LUI
        instruction_spec!(
            "LUI",
            "Load upper immediate",
            0x7F,
            0x37,
            r#"                let imm = ((insn & 0xfffff000) as i32) as i64;
                let rd = ((insn >> 7) & 0x1f) as u32;
                builder.push(IROp::AddImm { dst: rd, src: 0, imm });"#
        ),
        // ADDI
        instruction_spec!(
            "ADDI",
            "Add immediate",
            0x707F,
            0x13,
            r#"                let imm = ((insn as i32) >> 20) as i64;
                let rs1 = ((insn >> 15) & 0x1f) as u32;
                let rd = ((insn >> 7) & 0x1f) as u32;
                builder.push(IROp::AddImm { dst: rd, src: rs1, imm });"#
        ),
        // ADD/SUB/MUL/DIV等R-type指令
        instruction_spec!(
            "R_TYPE_ARITH",
            "R-type arithmetic operations",
            0x707F,
            0x33,
            r#"                let funct7 = (insn >> 25) & 0x7f;
                let rs2 = ((insn >> 20) & 0x1f) as u32;
                let rs1 = ((insn >> 15) & 0x1f) as u32;
                let funct3 = ((insn >> 12) & 0x7) as u32;
                let rd = ((insn >> 7) & 0x1f) as u32;

                let irop = match (funct3, funct7) {
                    (0, 0x00) => IROp::Add { dst: rd, src1: rs1, src2: rs2 }, // ADD
                    (0, 0x20) => IROp::Sub { dst: rd, src1: rs1, src2: rs2 }, // SUB
                    (0, 0x01) => IROp::Mul { dst: rd, src1: rs1, src2: rs2 }, // MUL
                    (4, 0x01) => IROp::Div { dst: rd, src1: rs1, src2: rs2, signed: true }, // DIV
                    (5, 0x01) => IROp::Div { dst: rd, src1: rs1, src2: rs2, signed: false }, // DIVU
                    (6, 0x01) => IROp::Rem { dst: rd, src1: rs1, src2: rs2, signed: true }, // REM
                    (7, 0x01) => IROp::Rem { dst: rd, src1: rs1, src2: rs2, signed: false }, // REMU
                    (1, 0x00) => IROp::Sll { dst: rd, src: rs1, shreg: rs2 }, // SLL
                    (5, 0x00) => IROp::Srl { dst: rd, src: rs1, shreg: rs2 }, // SRL
                    (5, 0x20) => IROp::Sra { dst: rd, src: rs1, shreg: rs2 }, // SRA
                    (7, 0x00) => IROp::And { dst: rd, src1: rs1, src2: rs2 }, // AND
                    (6, 0x00) => IROp::Or { dst: rd, src1: rs1, src2: rs2 }, // OR
                    (4, 0x00) => IROp::Xor { dst: rd, src1: rs1, src2: rs2 }, // XOR
                    _ => return Ok(false),
                };
                builder.push(irop);"#
        ),
        // LOAD指令
        instruction_spec!(
            "LOAD",
            "Load operations",
            0x7F,
            0x03,
            r#"                let imm = ((insn as i32) >> 20) as i64;
                let rs1 = ((insn >> 15) & 0x1f) as u32;
                let funct3 = ((insn >> 12) & 0x7) as u32;
                let rd = ((insn >> 7) & 0x1f) as u32;

                let size = match funct3 {
                    0x0 => 1, // LB
                    0x1 => 2, // LH
                    0x2 => 4, // LW
                    0x3 => 8, // LD
                    0x4 => 1, // LBU
                    0x5 => 2, // LHU
                    0x6 => 4, // LWU
                    _ => 4,
                };

                builder.push(IROp::Load {
                    dst: rd,
                    base: rs1,
                    offset: imm,
                    size,
                    flags: MemFlags::default()
                });"#
        ),
        // STORE指令
        instruction_spec!(
            "STORE",
            "Store operations",
            0x7F,
            0x23,
            r#"                let imm = (((insn >> 7) & 0x1f) | (((insn >> 25) & 0x7f) << 5)) as i32;
                let imm = ((imm as i32) << 20 >> 20) as i64;
                let rs2 = ((insn >> 20) & 0x1f) as u32;
                let rs1 = ((insn >> 15) & 0x1f) as u32;
                let funct3 = ((insn >> 12) & 0x7) as u32;

                let size = match funct3 {
                    0x0 => 1, // SB
                    0x1 => 2, // SH
                    0x2 => 4, // SW
                    0x3 => 8, // SD
                    _ => 4,
                };

                builder.push(IROp::Store {
                    src: rs2,
                    base: rs1,
                    offset: imm,
                    size,
                    flags: MemFlags::default()
                });"#
        ),
        // BRANCH指令
        instruction_spec!(
            "BRANCH",
            "Branch operations",
            0x7F,
            0x63,
            r#"                let imm = ((((insn >> 31) & 0x1) << 12)
                    | (((insn >> 7) & 0x1) << 11)
                    | (((insn >> 25) & 0x3f) << 5)
                    | (((insn >> 8) & 0xf) << 1)) as i32;
                let imm = ((imm as i32) << 19 >> 19) as i64;
                let rs2 = ((insn >> 20) & 0x1f) as u32;
                let rs1 = ((insn >> 15) & 0x1f) as u32;
                let funct3 = ((insn >> 12) & 0x7) as u32;

                let target = current_pc.wrapping_add(imm as u64);
                let cond_reg = 32; // Temporary register for condition

                match funct3 {
                    0x0 => { // BEQ
                        builder.push(IROp::CmpEq { dst: cond_reg, lhs: rs1, rhs: rs2 });
                        builder.set_term(Terminator::CondJmp {
                            cond: cond_reg,
                            target_true: target,
                            target_false: current_pc + 4
                        });
                    }
                    0x1 => { // BNE
                        builder.push(IROp::CmpNe { dst: cond_reg, lhs: rs1, rhs: rs2 });
                        builder.set_term(Terminator::CondJmp {
                            cond: cond_reg,
                            target_true: target,
                            target_false: current_pc + 4
                        });
                    }
                    0x4 => { // BLT
                        builder.push(IROp::CmpLt { dst: cond_reg, lhs: rs1, rhs: rs2 });
                        builder.set_term(Terminator::CondJmp {
                            cond: cond_reg,
                            target_true: target,
                            target_false: current_pc + 4
                        });
                    }
                    0x5 => { // BGE
                        builder.push(IROp::CmpGe { dst: cond_reg, lhs: rs1, rhs: rs2 });
                        builder.set_term(Terminator::CondJmp {
                            cond: cond_reg,
                            target_true: target,
                            target_false: current_pc + 4
                        });
                    }
                    0x6 => { // BLTU
                        builder.push(IROp::CmpLtU { dst: cond_reg, lhs: rs1, rhs: rs2 });
                        builder.set_term(Terminator::CondJmp {
                            cond: cond_reg,
                            target_true: target,
                            target_false: current_pc + 4
                        });
                    }
                    0x7 => { // BGEU
                        builder.push(IROp::CmpGeU { dst: cond_reg, lhs: rs1, rhs: rs2 });
                        builder.set_term(Terminator::CondJmp {
                            cond: cond_reg,
                            target_true: target,
                            target_false: current_pc + 4
                        });
                    }
                    _ => {}
                }
                break;"#
        ),
        // JAL
        instruction_spec!(
            "JAL",
            "Jump and link",
            0x7F,
            0x6F,
            r#"                let rd = ((insn >> 7) & 0x1f) as u32;
                let i = insn;
                let imm = (((i >> 31) & 0x1) << 20)
                    | (((i >> 21) & 0x3ff) << 1)
                    | (((i >> 20) & 0x1) << 11)
                    | (((i >> 12) & 0xff) << 12);
                let imm = sext21(imm);
                let target = current_pc.wrapping_add(imm as u64);

                builder.push(IROp::MovImm { dst: rd, imm: current_pc + 4 });
                builder.set_term(Terminator::Jmp { target });
                break;"#
        ),
        // JALR
        instruction_spec!(
            "JALR",
            "Jump and link register",
            0x707F,
            0x67,
            r#"                let rd = ((insn >> 7) & 0x1f) as u32;
                let rs1 = ((insn >> 15) & 0x1f) as u32;
                let imm = ((insn as i32) >> 20) as i64;

                builder.push(IROp::MovImm { dst: rd, imm: current_pc + 4 });

                let temp_reg = 33;
                builder.push(IROp::AddImm { dst: temp_reg, src: rs1, imm });
                builder.push(IROp::MovImm { dst: 34, imm: !1u64 });
                builder.push(IROp::And { dst: temp_reg, src1: temp_reg, src2: 34 });

                builder.set_term(Terminator::JmpReg { base: temp_reg, offset: 0 });
                break;"#
        ),
        // SYSTEM指令 (ECALL/EBREAK)
        instruction_spec!(
            "SYSTEM",
            "System calls and breakpoints",
            0x707F,
            0x73,
            r#"                let funct3 = ((insn >> 12) & 0x7) as u32;
                match funct3 {
                    0x0 => {
                        if insn == 0x00000073 {
                            builder.push(IROp::SysCall);
                        } else if insn == 0x00100073 {
                            builder.push(IROp::DebugBreak);
                        } else if insn == 0x30200073 {
                            builder.push(IROp::SysMret);
                        } else if insn == 0x10200073 {
                            builder.push(IROp::SysSret);
                        } else if insn == 0x10500073 {
                            builder.push(IROp::SysWfi);
                        }
                    }
                    _ => {} // CSR instructions would go here
                }
                builder.set_term(Terminator::Jmp { target: current_pc + 4 });
                break;"#
        )
    );

    // 生成代码
    let config = CodegenConfig {
        target_arch: "riscv64".to_string(),
        isa_version: "2.1".to_string(),
        optimization_level: 2,
        enable_debug: true,
    };

    let generated_code = riscv_set.generate_decoder(&config);

    // 输出生成的代码
    println!("{}", generated_code);

    // 或者保存到文件
    std::fs::write("riscv_decoder_generated.rs", generated_code)
        .expect("Failed to write generated code");
}

// RISC-V符号扩展辅助函数
fn sext21(x: u32) -> i64 {
    if ((x >> 20) & 1) != 0 {
        (x as i64) | (!0i64 << 21)
    } else {
        x as i64
    }
}
