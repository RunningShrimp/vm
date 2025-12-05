use vm_core::{Decoder, Fault, GuestAddr, Instruction, MMU, VmError};
use vm_ir::{IRBlock, IROp, MemFlags, Terminator};

mod vector;
use vector::VectorDecoder;

/// RISC-V 指令表示
#[derive(Debug, Clone)]
pub struct RiscvInstruction {
    pub mnemonic: &'static str,
    pub next_pc: GuestAddr,
    pub has_memory_op: bool,
    pub is_branch: bool,
}

impl Instruction for RiscvInstruction {
    fn next_pc(&self) -> GuestAddr {
        self.next_pc
    }

    fn size(&self) -> u8 {
        4 // RISC-V RV64I 指令固定 4 字节
    }

    fn operand_count(&self) -> usize {
        1 // 简化实现
    }

    fn mnemonic(&self) -> &str {
        self.mnemonic
    }

    fn is_control_flow(&self) -> bool {
        self.is_branch
    }

    fn is_memory_access(&self) -> bool {
        self.has_memory_op
    }
}

pub struct RiscvDecoder;

fn sext21(x: u32) -> i64 {
    if ((x >> 20) & 1) != 0 {
        (x as i64) | (!0i64 << 21)
    } else {
        x as i64
    }
}

impl Decoder for RiscvDecoder {
    type Instruction = RiscvInstruction;
    type Block = IRBlock;

    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Instruction, VmError> {
        let insn = mmu.fetch_insn(pc)? as u32;
        let opcode = insn & 0x7f;

        // Determine mnemonic based on opcode
        let mnemonic = match opcode {
            0x37 => "lui",
            0x17 => "auipc",
            0x6f => "jal",
            0x67 => "jalr",
            0x63 => "branch",
            0x03 => "load",
            0x23 => "store",
            0x13 => "addi",
            0x33 => "arith",
            0x0f => "fence",
            0x73 => "system",
            0x57 => "vector", // RV64V 向量扩展
            _ => "unknown",
        };

        let is_branch = matches!(opcode, 0x63 | 0x6f | 0x67);
        let has_memory_op = matches!(opcode, 0x03 | 0x23 | 0x57); // 向量加载/存储也算内存操作

        Ok(RiscvInstruction {
            mnemonic,
            next_pc: pc + 4,
            has_memory_op,
            is_branch,
        })
    }

    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, VmError> {
        let insn = mmu.fetch_insn(pc)? as u32;
        let mut reg_file = vm_ir::RegisterFile::new(32, vm_ir::RegisterMode::SSA);
        let mut b = vm_ir::IRBuilder::new(pc);

        // Check for compressed instructions (RV64C)
        // Compressed instructions have bits [1:0] != 11
        if (insn & 0x3) != 0x3 {
            // This is a 16-bit compressed instruction
            let op = (insn >> 13) & 0x7;
            let rd_rs1 = ((insn >> 7) & 0x1F) as u32;
            let rs2 = ((insn >> 2) & 0x1F) as u32;

            match op {
                0x0 => {
                    // C.ADDI4SPN: Add immediate to stack pointer
                    // rd = x2 + uimm
                    let uimm =
                        (((insn >> 2) & 0x1F) | ((insn >> 7) & 0x60) | ((insn >> 20) & 0x18)) << 2;
                    let dst = reg_file.write(rd_rs1 as usize);
                    b.push(IROp::AddImm {
                        dst,
                        src: reg_file.read(2),
                        imm: uimm as i64,
                    });
                    b.set_term(Terminator::Jmp {
                        target: pc.wrapping_add(2),
                    });
                    return Ok(b.build());
                }
                0x2 => {
                    // C.LW: Load word
                    let uimm = (((insn >> 5) & 0x7) | ((insn >> 10) & 0x38)) << 2;
                    let rs1 = ((insn >> 7) & 0x7) as u32;
                    let rd = ((insn >> 2) & 0x7) as u32;
                    let dst = reg_file.write(rd as usize);
                    b.push(IROp::Load {
                        dst,
                        base: reg_file.read(rs1 as usize),
                        offset: uimm as i64,
                        size: 4,
                        flags: MemFlags::default(),
                    });
                    b.set_term(Terminator::Jmp {
                        target: pc.wrapping_add(2),
                    });
                    return Ok(b.build());
                }
                0x6 => {
                    // C.SW: Store word
                    let uimm = (((insn >> 5) & 0x7) | ((insn >> 10) & 0x38)) << 2;
                    let rs1 = ((insn >> 7) & 0x7) as u32;
                    let rs2 = ((insn >> 2) & 0x7) as u32;
                    b.push(IROp::Store {
                        src: reg_file.read(rs2 as usize),
                        base: reg_file.read(rs1 as usize),
                        offset: uimm as i64,
                        size: 4,
                        flags: MemFlags::default(),
                    });
                    b.set_term(Terminator::Jmp {
                        target: pc.wrapping_add(2),
                    });
                    return Ok(b.build());
                }
                0x1 => {
                    // C.ADDI / C.NOP: Add immediate
                    if rd_rs1 == 0 {
                        // C.NOP
                        b.push(IROp::Nop);
                    } else {
                        // C.ADDI: rd = rd + sext(imm)
                        let imm = ((insn >> 2) & 0x1F) as i64;
                        let imm = if (imm & 0x10) != 0 {
                            imm | 0xFFFFFFE0
                        } else {
                            imm
                        };
                        let dst = reg_file.write(rd_rs1 as usize);
                        b.push(IROp::AddImm {
                            dst,
                            src: reg_file.read(rd_rs1 as usize),
                            imm,
                        });
                    }
                    b.set_term(Terminator::Jmp {
                        target: pc.wrapping_add(2),
                    });
                    return Ok(b.build());
                }
                0x3 => {
                    // C.ADDIW: Add immediate word (64-bit only, sign-extends result)
                    let imm = ((insn >> 2) & 0x1F) as i64;
                    let imm = if (imm & 0x10) != 0 {
                        imm | 0xFFFFFFE0
                    } else {
                        imm
                    };
                    let dst = reg_file.write(rd_rs1 as usize);
                    let tmp = reg_file.write(64);
                    b.push(IROp::AddImm {
                        dst: tmp,
                        src: reg_file.read(rd_rs1 as usize),
                        imm,
                    });
                    // Sign extend to 64 bits
                    let sign_mask = 0xFFFFFFFF00000000u64;
                    let sign_bit = reg_file.write(65);
                    b.push(IROp::SrlImm {
                        dst: sign_bit,
                        src: tmp,
                        sh: 31,
                    });
                    let one_reg = reg_file.write(69);
                    b.push(IROp::MovImm {
                        dst: one_reg,
                        imm: 1,
                    });
                    b.push(IROp::And {
                        dst: sign_bit,
                        src1: sign_bit,
                        src2: one_reg,
                    });
                    let extended = reg_file.write(66);
                    let sign_mask_reg = reg_file.write(67);
                    let zero_reg = reg_file.write(68);
                    b.push(IROp::MovImm {
                        dst: sign_mask_reg,
                        imm: sign_mask,
                    });
                    b.push(IROp::MovImm {
                        dst: zero_reg,
                        imm: 0,
                    });
                    b.push(IROp::Select {
                        dst: extended,
                        cond: sign_bit,
                        true_val: sign_mask_reg,
                        false_val: zero_reg,
                    });
                    b.push(IROp::Or {
                        dst,
                        src1: tmp,
                        src2: extended,
                    });
                    b.set_term(Terminator::Jmp {
                        target: pc.wrapping_add(2),
                    });
                    return Ok(b.build());
                }
                0x5 => {
                    // C.J: Jump
                    let imm =
                        ((insn >> 2) & 0x3FF) | ((insn >> 11) & 0x400) | ((insn >> 1) & 0x800);
                    let imm = if (imm & 0x800) != 0 {
                        (imm | 0xFFFFF000) as i32
                    } else {
                        imm as i32
                    };
                    let target = pc.wrapping_add((imm as i64 * 2) as u64);
                    b.set_term(Terminator::Jmp { target });
                    return Ok(b.build());
                }
                0x4 => {
                    // C.LI: Load immediate (rd != 0)
                    if rd_rs1 != 0 {
                        let imm = ((insn >> 2) & 0x1F) as i64;
                        let imm = if (imm & 0x10) != 0 {
                            imm | 0xFFFFFFE0
                        } else {
                            imm
                        };
                        let dst = reg_file.write(rd_rs1 as usize);
                        b.push(IROp::MovImm {
                            dst,
                            imm: imm as u64,
                        });
                    }
                    b.set_term(Terminator::Jmp {
                        target: pc.wrapping_add(2),
                    });
                    return Ok(b.build());
                }
                0x7 => {
                    // C.ANDI: AND immediate
                    let imm = ((insn >> 2) & 0x1F) as i64;
                    let imm = if (imm & 0x10) != 0 {
                        imm | 0xFFFFFFE0
                    } else {
                        imm
                    };
                    let dst = reg_file.write(rd_rs1 as usize);
                    let imm_reg = reg_file.write(64);
                    b.push(IROp::MovImm {
                        dst: imm_reg,
                        imm: imm as u64,
                    });
                    b.push(IROp::And {
                        dst,
                        src1: reg_file.read(rd_rs1 as usize),
                        src2: imm_reg,
                    });
                    b.set_term(Terminator::Jmp {
                        target: pc.wrapping_add(2),
                    });
                    return Ok(b.build());
                }
                _ => {}
            }

            // Additional compressed instruction patterns (need to check op bits more carefully)
            if (insn & 0x3) != 0x3 {
                let op = (insn >> 13) & 0x7;
                let rd_rs1 = ((insn >> 7) & 0x1F) as u32;
                let funct3 = (insn >> 10) & 0x7;
                let rs2 = ((insn >> 2) & 0x1F) as u32;

                // C.LUI: Load upper immediate (op = 0x1, rd_rs1 != 0, rd_rs1 != 2)
                if op == 0x1 && rd_rs1 != 0 && rd_rs1 != 2 {
                    let imm = ((insn >> 2) & 0x1F) as i64;
                    let imm_high = (imm & 0x1F) << 12;
                    let sign_bit = (imm & 0x20) << 7; // Bit 5 -> bit 12
                    let imm_full = if sign_bit != 0 {
                        (imm_high | sign_bit) | 0xFFFFE000
                    } else {
                        imm_high | sign_bit
                    };
                    let dst = reg_file.write(rd_rs1 as usize);
                    b.push(IROp::MovImm {
                        dst,
                        imm: imm_full as u64,
                    });
                    b.set_term(Terminator::Jmp {
                        target: pc.wrapping_add(2),
                    });
                    return Ok(b.build());
                }

                // C.ADDI16SP: Add immediate to stack pointer (op = 0x1, rd_rs1 = 2)
                if op == 0x1 && rd_rs1 == 2 {
                    let imm = ((insn >> 2) & 0x3F) as i64;
                    let imm_sign = (imm & 0x20) != 0;
                    let imm_val = if imm_sign {
                        (imm | 0xFFFFFFC0) << 4
                    } else {
                        imm << 4
                    };
                    let dst = reg_file.write(2);
                    b.push(IROp::AddImm {
                        dst,
                        src: reg_file.read(2),
                        imm: imm_val,
                    });
                    b.set_term(Terminator::Jmp {
                        target: pc.wrapping_add(2),
                    });
                    return Ok(b.build());
                }

                // C.SRLI, C.SRAI (op = 0x1, funct3 = 0x4 or 0x5)
                if op == 0x1 && (funct3 == 0x4 || funct3 == 0x5) {
                    let imm = ((insn >> 2) & 0x1F) as u8;
                    let dst = reg_file.write(rd_rs1 as usize);
                    if funct3 == 0x4 {
                        // C.SRLI: Shift right logical immediate
                        b.push(IROp::SrlImm {
                            dst,
                            src: reg_file.read(rd_rs1 as usize),
                            sh: imm,
                        });
                    } else {
                        // C.SRAI: Shift right arithmetic immediate
                        b.push(IROp::SraImm {
                            dst,
                            src: reg_file.read(rd_rs1 as usize),
                            sh: imm,
                        });
                    }
                    b.set_term(Terminator::Jmp {
                        target: pc.wrapping_add(2),
                    });
                    return Ok(b.build());
                }

                // C.SUB, C.XOR, C.OR, C.AND (op = 0x1, funct3 bits)
                if op == 0x1 && funct3 < 0x4 {
                    let dst = reg_file.write(rd_rs1 as usize);
                    match funct3 {
                        0x0 => {
                            // C.SUB: Subtract
                            b.push(IROp::Sub {
                                dst,
                                src1: reg_file.read(rd_rs1 as usize),
                                src2: reg_file.read(rs2 as usize),
                            });
                        }
                        0x1 => {
                            // C.XOR: XOR
                            b.push(IROp::Xor {
                                dst,
                                src1: reg_file.read(rd_rs1 as usize),
                                src2: reg_file.read(rs2 as usize),
                            });
                        }
                        0x2 => {
                            // C.OR: OR
                            b.push(IROp::Or {
                                dst,
                                src1: reg_file.read(rd_rs1 as usize),
                                src2: reg_file.read(rs2 as usize),
                            });
                        }
                        0x3 => {
                            // C.AND: AND
                            b.push(IROp::And {
                                dst,
                                src1: reg_file.read(rd_rs1 as usize),
                                src2: reg_file.read(rs2 as usize),
                            });
                        }
                        _ => {}
                    }
                    b.set_term(Terminator::Jmp {
                        target: pc.wrapping_add(2),
                    });
                    return Ok(b.build());
                }

                // C.JR, C.JALR (op = 0x2, rs2 = 0)
                if op == 0x2 && rs2 == 0 {
                    if rd_rs1 != 0 {
                        // C.JR: Jump register
                        b.set_term(Terminator::JmpReg {
                            base: reg_file.read(rd_rs1 as usize),
                            offset: 0,
                        });
                        return Ok(b.build());
                    } else if rd_rs1 == 1 {
                        // C.JALR: Jump and link register (x1 = ra)
                        let dst = reg_file.write(1);
                        b.push(IROp::MovImm {
                            dst,
                            imm: pc.wrapping_add(2),
                        });
                        b.set_term(Terminator::JmpReg {
                            base: reg_file.read(rd_rs1 as usize),
                            offset: 0,
                        });
                        return Ok(b.build());
                    }
                }

                // C.BEQZ, C.BNEZ (op = 0x6)
                if op == 0x6 {
                    let rs1 = ((insn >> 7) & 0x7) as u32;
                    let imm =
                        (((insn >> 2) & 0x7) | ((insn >> 10) & 0x18) | ((insn >> 3) & 0x20)) as i64;
                    let imm_sign = (imm & 0x40) != 0;
                    let imm_val = if imm_sign {
                        (imm | 0xFFFFFF80) << 1
                    } else {
                        imm << 1
                    };
                    let target = pc.wrapping_add(imm_val as u64);

                    let cond = reg_file.write(100);
                    let zero = reg_file.write(101);
                    b.push(IROp::MovImm { dst: zero, imm: 0 });

                    if (insn >> 12) & 0x1 == 0 {
                        // C.BEQZ: Branch if equal to zero
                        b.push(IROp::CmpEq {
                            dst: cond,
                            lhs: reg_file.read(rs1 as usize),
                            rhs: zero,
                        });
                        b.set_term(Terminator::CondJmp {
                            cond,
                            target_true: target,
                            target_false: pc.wrapping_add(2),
                        });
                    } else {
                        // C.BNEZ: Branch if not equal to zero
                        b.push(IROp::CmpNe {
                            dst: cond,
                            lhs: reg_file.read(rs1 as usize),
                            rhs: zero,
                        });
                        b.set_term(Terminator::CondJmp {
                            cond,
                            target_true: target,
                            target_false: pc.wrapping_add(2),
                        });
                    }
                    return Ok(b.build());
                }

                // C.LWSP, C.SWSP (op = 0x2, different encoding)
                if op == 0x2 && rs2 != 0 {
                    let rd = rd_rs1;
                    let imm = ((insn >> 2) & 0x1F) as i64;
                    let imm_val = imm << 2;

                    if rd != 0 {
                        // C.LWSP: Load word from stack pointer
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Load {
                            dst,
                            base: reg_file.read(2),
                            offset: imm_val,
                            size: 4,
                            flags: MemFlags::default(),
                        });
                        b.set_term(Terminator::Jmp {
                            target: pc.wrapping_add(2),
                        });
                        return Ok(b.build());
                    } else {
                        // C.SWSP: Store word to stack pointer
                        b.push(IROp::Store {
                            src: reg_file.read(rs2 as usize),
                            base: reg_file.read(2),
                            offset: imm_val,
                            size: 4,
                            flags: MemFlags::default(),
                        });
                        b.set_term(Terminator::Jmp {
                            target: pc.wrapping_add(2),
                        });
                        return Ok(b.build());
                    }
                }

                // C.MV, C.ADD (op = 0x2, funct3 = 0x0)
                if op == 0x2 && funct3 == 0x0 && rs2 != 0 {
                    let rd = rd_rs1;
                    if rd != 0 {
                        if rs2 != 0 {
                            // C.MV: Move (rd = rs2)
                            let dst = reg_file.write(rd as usize);
                            b.push(IROp::AddImm {
                                dst,
                                src: reg_file.read(rs2 as usize),
                                imm: 0,
                            });
                        } else {
                            // C.ADD: Add (rd = rd + rs2)
                            let dst = reg_file.write(rd as usize);
                            b.push(IROp::Add {
                                dst,
                                src1: reg_file.read(rd as usize),
                                src2: reg_file.read(rs2 as usize),
                            });
                        }
                        b.set_term(Terminator::Jmp {
                            target: pc.wrapping_add(2),
                        });
                        return Ok(b.build());
                    }
                }
            }
        }

        // Standard 32-bit instructions
        let opcode = insn & 0x7f;
        let rd = ((insn >> 7) & 0x1f) as u32;
        let funct3 = ((insn >> 12) & 0x7) as u32;
        let rs1 = ((insn >> 15) & 0x1f) as u32;
        let rs2 = ((insn >> 20) & 0x1f) as u32;

        // 检查是否为向量扩展指令 (RV64V)
        if opcode == 0x57 {
            return VectorDecoder::to_ir(insn, &mut reg_file, &mut b, mmu, pc);
        }

        match opcode {
            0x37 => {
                let imm = ((insn & 0xfffff000) as i32) as i64;
                let imm = imm; // upper placed
                let dst = reg_file.write(rd as usize);
                b.push(IROp::AddImm {
                    dst,
                    src: reg_file.read(0),
                    imm,
                });
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x13 => {
                let imm = ((insn as i32) >> 20) as i64;
                match funct3 {
                    0 => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AddImm {
                            dst,
                            src: reg_file.read(rs1 as usize),
                            imm,
                        });
                    }
                    1 => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::SllImm {
                            dst,
                            src: reg_file.read(rs1 as usize),
                            sh: (imm as u64 & 0x3f) as u8,
                        });
                    }
                    2 => {
                        // SLTI: Set Less Than Immediate (signed)
                        let dst = reg_file.write(rd as usize);
                        let rs1_val = reg_file.read(rs1 as usize);
                        let imm_reg = reg_file.write(64);
                        b.push(IROp::MovImm {
                            dst: imm_reg,
                            imm: imm as u64,
                        });
                        b.push(IROp::CmpLt {
                            dst,
                            lhs: rs1_val,
                            rhs: imm_reg,
                        });
                    }
                    3 => {
                        // SLTIU: Set Less Than Immediate Unsigned
                        let dst = reg_file.write(rd as usize);
                        let rs1_val = reg_file.read(rs1 as usize);
                        let imm_reg = reg_file.write(64);
                        b.push(IROp::MovImm {
                            dst: imm_reg,
                            imm: imm as u64,
                        });
                        b.push(IROp::CmpLtU {
                            dst,
                            lhs: rs1_val,
                            rhs: imm_reg,
                        });
                    }
                    5 => {
                        let funct7 = (insn >> 25) & 0x7f;
                        let dst = reg_file.write(rd as usize);
                        if funct7 == 0x00 {
                            b.push(IROp::SrlImm {
                                dst,
                                src: reg_file.read(rs1 as usize),
                                sh: (imm as u64 & 0x3f) as u8,
                            })
                        } else if funct7 == 0x20 {
                            b.push(IROp::SraImm {
                                dst,
                                src: reg_file.read(rs1 as usize),
                                sh: (imm as u64 & 0x3f) as u8,
                            })
                        }
                    }
                    _ => {}
                }
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x33 => {
                let funct7 = (insn >> 25) & 0x7f;
                match (funct3, funct7) {
                    (0, 0x00) => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Add {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                        })
                    }
                    (0, 0x20) => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Sub {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                        })
                    }
                    // RV64M: Multiply/Divide extension
                    (0, 0x01) => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Mul {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                        })
                    } // MUL
                    (1, 0x01) => {
                        // MULH: High 64 bits of signed multiplication
                        // For 64-bit: compute full 128-bit product and extract high 64 bits
                        // Simplified: use host's 128-bit multiplication if available
                        // Otherwise, decompose into multiple operations
                        let dst = reg_file.write(rd as usize);
                        let rs1_val = reg_file.read(rs1 as usize);
                        let rs2_val = reg_file.read(rs2 as usize);

                        // For now, use a placeholder that computes high bits
                        // Full implementation requires 128-bit arithmetic
                        // Algorithm: (a * b) >> 64 for signed multiplication
                        // We can approximate by computing partial products
                        let low_prod = reg_file.write(32);
                        b.push(IROp::Mul {
                            dst: low_prod,
                            src1: rs1_val,
                            src2: rs2_val,
                        });

                        // Extract high 32 bits of each operand
                        let rs1_hi = reg_file.write(33);
                        let rs2_hi = reg_file.write(34);
                        b.push(IROp::SrlImm {
                            dst: rs1_hi,
                            src: rs1_val,
                            sh: 32,
                        });
                        b.push(IROp::SrlImm {
                            dst: rs2_hi,
                            src: rs2_val,
                            sh: 32,
                        });

                        // Compute high bits: (rs1_hi * rs2_val) + (rs1_val * rs2_hi) + (rs1_hi * rs2_hi) << 32
                        let term1 = reg_file.write(35);
                        let term2 = reg_file.write(36);
                        let term3 = reg_file.write(37);
                        b.push(IROp::Mul {
                            dst: term1,
                            src1: rs1_hi,
                            src2: rs2_val,
                        });
                        b.push(IROp::Mul {
                            dst: term2,
                            src1: rs1_val,
                            src2: rs2_hi,
                        });
                        b.push(IROp::Mul {
                            dst: term3,
                            src1: rs1_hi,
                            src2: rs2_hi,
                        });

                        // Shift term3 left by 32
                        let term3_shifted = reg_file.write(38);
                        b.push(IROp::SllImm {
                            dst: term3_shifted,
                            src: term3,
                            sh: 32,
                        });

                        // Add terms and extract high 64 bits
                        let sum1 = reg_file.write(39);
                        b.push(IROp::Add {
                            dst: sum1,
                            src1: term1,
                            src2: term2,
                        });
                        let sum2 = reg_file.write(40);
                        b.push(IROp::Add {
                            dst: sum2,
                            src1: sum1,
                            src2: term3_shifted,
                        });

                        // Extract high 64 bits (shift right by 32 and add carry from low product)
                        let low_hi = reg_file.write(41);
                        b.push(IROp::SrlImm {
                            dst: low_hi,
                            src: low_prod,
                            sh: 32,
                        });
                        b.push(IROp::Add {
                            dst: sum2,
                            src1: sum2,
                            src2: low_hi,
                        });
                        b.push(IROp::SrlImm {
                            dst,
                            src: sum2,
                            sh: 32,
                        });
                    }
                    (2, 0x01) => {
                        // MULHSU: High 64 bits of signed-unsigned multiplication
                        // rs1 is signed, rs2 is unsigned
                        let dst = reg_file.write(rd as usize);
                        let rs1_val = reg_file.read(rs1 as usize);
                        let rs2_val = reg_file.read(rs2 as usize);

                        // Similar to MULH but rs1 is treated as signed
                        let rs1_hi = reg_file.write(33);
                        let rs2_hi = reg_file.write(34);
                        b.push(IROp::SraImm {
                            dst: rs1_hi,
                            src: rs1_val,
                            sh: 32,
                        }); // Arithmetic shift for signed
                        b.push(IROp::SrlImm {
                            dst: rs2_hi,
                            src: rs2_val,
                            sh: 32,
                        }); // Logical shift for unsigned

                        let term1 = reg_file.write(35);
                        let term2 = reg_file.write(36);
                        let term3 = reg_file.write(37);
                        b.push(IROp::Mul {
                            dst: term1,
                            src1: rs1_hi,
                            src2: rs2_val,
                        });
                        b.push(IROp::Mul {
                            dst: term2,
                            src1: rs1_val,
                            src2: rs2_hi,
                        });
                        b.push(IROp::Mul {
                            dst: term3,
                            src1: rs1_hi,
                            src2: rs2_hi,
                        });

                        let term3_shifted = reg_file.write(38);
                        b.push(IROp::SllImm {
                            dst: term3_shifted,
                            src: term3,
                            sh: 32,
                        });

                        let sum1 = reg_file.write(39);
                        b.push(IROp::Add {
                            dst: sum1,
                            src1: term1,
                            src2: term2,
                        });
                        let sum2 = reg_file.write(40);
                        b.push(IROp::Add {
                            dst: sum2,
                            src1: sum1,
                            src2: term3_shifted,
                        });
                        b.push(IROp::SrlImm {
                            dst,
                            src: sum2,
                            sh: 32,
                        });
                    }
                    (3, 0x01) => {
                        // MULHU: High 64 bits of unsigned multiplication
                        let dst = reg_file.write(rd as usize);
                        let rs1_val = reg_file.read(rs1 as usize);
                        let rs2_val = reg_file.read(rs2 as usize);

                        // Both operands are unsigned
                        let rs1_hi = reg_file.write(33);
                        let rs2_hi = reg_file.write(34);
                        b.push(IROp::SrlImm {
                            dst: rs1_hi,
                            src: rs1_val,
                            sh: 32,
                        });
                        b.push(IROp::SrlImm {
                            dst: rs2_hi,
                            src: rs2_val,
                            sh: 32,
                        });

                        let term1 = reg_file.write(35);
                        let term2 = reg_file.write(36);
                        let term3 = reg_file.write(37);
                        b.push(IROp::Mul {
                            dst: term1,
                            src1: rs1_hi,
                            src2: rs2_val,
                        });
                        b.push(IROp::Mul {
                            dst: term2,
                            src1: rs1_val,
                            src2: rs2_hi,
                        });
                        b.push(IROp::Mul {
                            dst: term3,
                            src1: rs1_hi,
                            src2: rs2_hi,
                        });

                        let term3_shifted = reg_file.write(38);
                        b.push(IROp::SllImm {
                            dst: term3_shifted,
                            src: term3,
                            sh: 32,
                        });

                        let sum1 = reg_file.write(39);
                        b.push(IROp::Add {
                            dst: sum1,
                            src1: term1,
                            src2: term2,
                        });
                        let sum2 = reg_file.write(40);
                        b.push(IROp::Add {
                            dst: sum2,
                            src1: sum1,
                            src2: term3_shifted,
                        });
                        b.push(IROp::SrlImm {
                            dst,
                            src: sum2,
                            sh: 32,
                        });
                    }
                    (4, 0x01) => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Div {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                            signed: true,
                        })
                    } // DIV
                    (5, 0x01) => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Div {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                            signed: false,
                        })
                    } // DIVU
                    (6, 0x01) => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Rem {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                            signed: true,
                        })
                    } // REM
                    (7, 0x01) => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Rem {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                            signed: false,
                        })
                    } // REMU
                    (1, 0x00) => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Sll {
                            dst,
                            src: reg_file.read(rs1 as usize),
                            shreg: reg_file.read(rs2 as usize),
                        })
                    }
                    (5, 0x00) => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Srl {
                            dst,
                            src: reg_file.read(rs1 as usize),
                            shreg: reg_file.read(rs2 as usize),
                        })
                    }
                    (5, 0x20) => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Sra {
                            dst,
                            src: reg_file.read(rs1 as usize),
                            shreg: reg_file.read(rs2 as usize),
                        })
                    }
                    (2, 0x00) => {
                        // SLT: Set Less Than (signed)
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::CmpLt {
                            dst,
                            lhs: reg_file.read(rs1 as usize),
                            rhs: reg_file.read(rs2 as usize),
                        });
                    }
                    (3, 0x00) => {
                        // SLTU: Set Less Than Unsigned
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::CmpLtU {
                            dst,
                            lhs: reg_file.read(rs1 as usize),
                            rhs: reg_file.read(rs2 as usize),
                        });
                    }
                    (7, 0x00) => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::And {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                        })
                    }
                    (6, 0x00) => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Or {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                        })
                    }
                    (4, 0x00) => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Xor {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                        })
                    }
                    _ => {}
                }
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x0F => {
                // FENCE / FENCE.I / SFENCE.VMA
                let funct3 = (insn >> 12) & 0x7;
                match funct3 {
                    0 => {
                        // FENCE: Memory ordering fence
                        // FENCE pred, succ - orders memory operations
                        // Simplified: just emit a memory barrier
                        let pred = (insn >> 24) & 0xF;
                        let succ = (insn >> 20) & 0xF;
                        // For now, emit a generic memory barrier
                        b.push(IROp::Nop); // Placeholder for memory barrier
                    }
                    1 => {
                        // FENCE.I: Instruction fence
                        // Flushes instruction cache and ensures instruction fetch ordering
                        b.push(IROp::Nop); // Placeholder for instruction fence
                    }
                    _ => {}
                }
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x73 => {
                // System instructions (CSR, SFENCE.VMA, etc.)
                let funct3 = (insn >> 12) & 0x7;
                let funct7 = (insn >> 25) & 0x7F;
                let rs1 = ((insn >> 15) & 0x1f) as u32;
                let rs2 = ((insn >> 20) & 0x1f) as u32;

                // SFENCE.VMA: Supervisor Fence Virtual Memory Address
                // Format: SFENCE.VMA rs1, rs2
                // Opcode: 0x73, funct3: 0x0, funct7: 0x09
                if funct3 == 0x0 && funct7 == 0x09 {
                    // SFENCE.VMA orders memory accesses and TLB operations
                    // rs1: address register (0 = all addresses)
                    // rs2: ASID register (0 = all ASIDs)
                    // This is a memory barrier that ensures all previous memory accesses
                    // are visible before TLB invalidation
                    let mut flags = MemFlags::default();
                    flags.fence_before = true;
                    flags.fence_after = true;
                    flags.order = vm_ir::MemOrder::SeqCst;

                    // Emit TLB flush operation
                    let vaddr = if rs1 == 0 {
                        None // Flush all
                    } else {
                        Some(pc) // Use current PC as placeholder, actual implementation should use rs1 value
                    };
                    b.push(IROp::TlbFlush { vaddr });
                    b.set_term(Terminator::Jmp {
                        target: pc.wrapping_add(4),
                    });
                    return Ok(b.build());
                }

                // CSR instructions
                let csr = ((insn >> 20) & 0xFFF) as u16;
                match funct3 {
                    0x1 => {
                        // CSRRW
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::CsrRead { dst, csr });
                        b.push(IROp::CsrWrite {
                            csr,
                            src: reg_file.read(rs1 as usize),
                        });
                        b.set_term(Terminator::Jmp {
                            target: pc.wrapping_add(4),
                        });
                    }
                    0x2 => {
                        // CSRRS
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::CsrRead { dst, csr });
                        b.push(IROp::CsrSet {
                            csr,
                            src: reg_file.read(rs1 as usize),
                        });
                        b.set_term(Terminator::Jmp {
                            target: pc.wrapping_add(4),
                        });
                    }
                    0x3 => {
                        // CSRRC
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::CsrRead { dst, csr });
                        b.push(IROp::CsrClear {
                            csr,
                            src: reg_file.read(rs1 as usize),
                        });
                        b.set_term(Terminator::Jmp {
                            target: pc.wrapping_add(4),
                        });
                    }
                    0x5 => {
                        // CSRRWI
                        let zimm = ((insn >> 15) & 0x1f) as u32;
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::CsrRead { dst, csr });
                        b.push(IROp::CsrWriteImm {
                            csr,
                            imm: zimm,
                            dst,
                        });
                        b.set_term(Terminator::Jmp {
                            target: pc.wrapping_add(4),
                        });
                    }
                    0x6 => {
                        // CSRRSI
                        let zimm = ((insn >> 15) & 0x1f) as u32;
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::CsrRead { dst, csr });
                        b.push(IROp::CsrSetImm {
                            csr,
                            imm: zimm,
                            dst,
                        });
                        b.set_term(Terminator::Jmp {
                            target: pc.wrapping_add(4),
                        });
                    }
                    0x7 => {
                        // CSRRCI
                        let zimm = ((insn >> 15) & 0x1f) as u32;
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::CsrRead { dst, csr });
                        b.push(IROp::CsrClearImm {
                            csr,
                            imm: zimm,
                            dst,
                        });
                        b.set_term(Terminator::Jmp {
                            target: pc.wrapping_add(4),
                        });
                    }
                    0x0 => {
                        // System instructions (ECALL, EBREAK, MRET, SRET, WFI)
                        if insn == 0x00000073 {
                            b.push(IROp::SysCall);
                            b.set_term(Terminator::Jmp {
                                target: pc.wrapping_add(4),
                            });
                        } else if insn == 0x00100073 {
                            b.push(IROp::DebugBreak);
                            b.set_term(Terminator::Jmp {
                                target: pc.wrapping_add(4),
                            });
                        } else if insn == 0x30200073 {
                            b.push(IROp::SysMret);
                            b.set_term(Terminator::Jmp {
                                target: pc.wrapping_add(4),
                            });
                        } else if insn == 0x10200073 {
                            b.push(IROp::SysSret);
                            b.set_term(Terminator::Jmp {
                                target: pc.wrapping_add(4),
                            });
                        } else if insn == 0x10500073 {
                            b.push(IROp::SysWfi);
                            b.set_term(Terminator::Jmp {
                                target: pc.wrapping_add(4),
                            });
                        } else {
                            b.set_term(Terminator::Jmp {
                                target: pc.wrapping_add(4),
                            });
                        }
                    }
                    _ => {
                        b.set_term(Terminator::Jmp {
                            target: pc.wrapping_add(4),
                        });
                    }
                }
            }
            0x07 => {
                // RV64F: FLW (Floating-point Load Word)
                let imm = ((insn as i32) >> 20) as i64;
                let dst = reg_file.write(rd as usize);
                b.push(IROp::Load {
                    dst,
                    base: reg_file.read(rs1 as usize),
                    offset: imm,
                    size: 4,
                    flags: MemFlags::default(),
                });
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x27 => {
                // RV64F: FSW (Floating-point Store Word)
                let imm = (((insn >> 7) & 0x1f) | (((insn >> 25) & 0x7f) << 5)) as i32;
                let imm = ((imm as i32) << 20 >> 20) as i64;
                b.push(IROp::Store {
                    src: reg_file.read(rs2 as usize),
                    base: reg_file.read(rs1 as usize),
                    offset: imm,
                    size: 4,
                    flags: MemFlags::default(),
                });
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x53 => {
                // RV64F/RV64D: Floating-point operations
                let funct7 = (insn >> 25) & 0x7f;
                let rs2 = ((insn >> 20) & 0x1f) as u32;
                let rs3 = ((insn >> 27) & 0x1f) as u32;
                let rm = (insn >> 12) & 0x7;

                match funct7 {
                    0x00 => {
                        // FADD.S / FADD.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::FaddS {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                        });
                    }
                    0x04 => {
                        // FSUB.S / FSUB.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::FsubS {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                        });
                    }
                    0x08 => {
                        // FMUL.S / FMUL.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::FmulS {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                        });
                    }
                    0x0C => {
                        // FDIV.S / FDIV.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::FdivS {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                        });
                    }
                    0x2C => {
                        // FSQRT.S / FSQRT.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::FsqrtS {
                            dst,
                            src: reg_file.read(rs1 as usize),
                        });
                    }
                    0x10 => {
                        // FSGNJ.S / FSGNJ.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::FsgnjS {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                        });
                    }
                    0x11 => {
                        // FSGNJN.S / FSGNJN.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::FsgnjnS {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                        });
                    }
                    0x12 => {
                        // FSGNJX.S / FSGNJX.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::FsgnjxS {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                        });
                    }
                    0x14 => {
                        // FMIN.S / FMIN.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::FminS {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                        });
                    }
                    0x15 => {
                        // FMAX.S / FMAX.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::FmaxS {
                            dst,
                            src1: reg_file.read(rs1 as usize),
                            src2: reg_file.read(rs2 as usize),
                        });
                    }
                    0x20 => {
                        // FCVT.W.S / FCVT.W.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Fcvtws {
                            dst,
                            src: reg_file.read(rs1 as usize),
                        });
                    }
                    0x21 => {
                        // FCVT.WU.S / FCVT.WU.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Fcvtwus {
                            dst,
                            src: reg_file.read(rs1 as usize),
                        });
                    }
                    0x60 => {
                        // FCVT.S.W / FCVT.D.W
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Fcvtsw {
                            dst,
                            src: reg_file.read(rs1 as usize),
                        });
                    }
                    0x61 => {
                        // FCVT.S.WU / FCVT.D.WU
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Fcvtswu {
                            dst,
                            src: reg_file.read(rs1 as usize),
                        });
                    }
                    0x50 => {
                        match funct3 {
                            0 => {
                                // FLE.S / FLE.D
                                let dst = reg_file.write(rd as usize);
                                b.push(IROp::FleS {
                                    dst,
                                    src1: reg_file.read(rs1 as usize),
                                    src2: reg_file.read(rs2 as usize),
                                });
                            }
                            1 => {
                                // FLT.S / FLT.D
                                let dst = reg_file.write(rd as usize);
                                b.push(IROp::FltS {
                                    dst,
                                    src1: reg_file.read(rs1 as usize),
                                    src2: reg_file.read(rs2 as usize),
                                });
                            }
                            2 => {
                                // FEQ.S / FEQ.D
                                let dst = reg_file.write(rd as usize);
                                b.push(IROp::FeqS {
                                    dst,
                                    src1: reg_file.read(rs1 as usize),
                                    src2: reg_file.read(rs2 as usize),
                                });
                            }
                            _ => {}
                        }
                    }
                    0x68 => {
                        // FCVT.L.S / FCVT.L.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Fcvtls {
                            dst,
                            src: reg_file.read(rs1 as usize),
                        });
                    }
                    0x69 => {
                        // FCVT.LU.S / FCVT.LU.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Fcvtlus {
                            dst,
                            src: reg_file.read(rs1 as usize),
                        });
                    }
                    0x70 => {
                        // FCVT.S.L / FCVT.D.L
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Fcvtsl {
                            dst,
                            src: reg_file.read(rs1 as usize),
                        });
                    }
                    0x71 => {
                        // FCVT.S.LU / FCVT.D.LU
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Fcvtslu {
                            dst,
                            src: reg_file.read(rs1 as usize),
                        });
                    }
                    0x78 => {
                        // FMV.X.W / FMV.X.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::FmvXW {
                            dst,
                            src: reg_file.read(rs1 as usize),
                        });
                    }
                    0x79 => {
                        // FCLASS.S / FCLASS.D
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::FclassS {
                            dst,
                            src: reg_file.read(rs1 as usize),
                        });
                    }
                    0x7C => {
                        // FMV.W.X / FMV.D.X
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::FmvWX {
                            dst,
                            src: reg_file.read(rs1 as usize),
                        });
                    }
                    _ => {}
                }
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x43 => {
                // RV64F: FMADD.S / FMADD.D
                let rs3 = ((insn >> 27) & 0x1f) as u32;
                let dst = reg_file.write(rd as usize);
                b.push(IROp::FmaddS {
                    dst,
                    src1: reg_file.read(rs1 as usize),
                    src2: reg_file.read(rs2 as usize),
                    src3: reg_file.read(rs3 as usize),
                });
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x47 => {
                // RV64F: FMSUB.S / FMSUB.D
                let rs3 = ((insn >> 27) & 0x1f) as u32;
                let dst = reg_file.write(rd as usize);
                b.push(IROp::FmsubS {
                    dst,
                    src1: reg_file.read(rs1 as usize),
                    src2: reg_file.read(rs2 as usize),
                    src3: reg_file.read(rs3 as usize),
                });
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x4B => {
                // RV64F: FNMSUB.S / FNMSUB.D
                let rs3 = ((insn >> 27) & 0x1f) as u32;
                let dst = reg_file.write(rd as usize);
                b.push(IROp::FnmaddS {
                    dst,
                    src1: reg_file.read(rs1 as usize),
                    src2: reg_file.read(rs2 as usize),
                    src3: reg_file.read(rs3 as usize),
                });
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x4F => {
                // RV64F: FNMADD.S / FNMADD.D
                let rs3 = ((insn >> 27) & 0x1f) as u32;
                let dst = reg_file.write(rd as usize);
                b.push(IROp::FnmsubS {
                    dst,
                    src1: reg_file.read(rs1 as usize),
                    src2: reg_file.read(rs2 as usize),
                    src3: reg_file.read(rs3 as usize),
                });
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x03 => {
                // RV64D: FLD (Floating-point Load Double)
                if funct3 == 3 {
                    let imm = ((insn as i32) >> 20) as i64;
                    let dst = reg_file.write(rd as usize);
                    b.push(IROp::Load {
                        dst,
                        base: reg_file.read(rs1 as usize),
                        offset: imm,
                        size: 8,
                        flags: MemFlags::default(),
                    });
                }
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x23 => {
                // RV64D: FSD (Floating-point Store Double)
                if funct3 == 3 {
                    let imm = (((insn >> 7) & 0x1f) | (((insn >> 25) & 0x7f) << 5)) as i32;
                    let imm = ((imm as i32) << 20 >> 20) as i64;
                    b.push(IROp::Store {
                        src: reg_file.read(rs2 as usize),
                        base: reg_file.read(rs1 as usize),
                        offset: imm,
                        size: 8,
                        flags: MemFlags::default(),
                    });
                }
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x17 => {
                let imm = ((insn & 0xfffff000) as i32) as i64;
                let dst = reg_file.write(rd as usize);
                b.push(IROp::MovImm { dst, imm: pc });
                b.push(IROp::AddImm { dst, src: dst, imm });
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x03 => {
                let imm = ((insn as i32) >> 20) as i64;
                match funct3 {
                    0x0 => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Load {
                            dst,
                            base: reg_file.read(rs1 as usize),
                            size: 1,
                            offset: imm,
                            flags: MemFlags::default(),
                        });
                    }
                    0x1 => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Load {
                            dst,
                            base: reg_file.read(rs1 as usize),
                            size: 2,
                            offset: imm,
                            flags: MemFlags::default(),
                        });
                    }
                    0x2 => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Load {
                            dst,
                            base: reg_file.read(rs1 as usize),
                            size: 4,
                            offset: imm,
                            flags: MemFlags::default(),
                        });
                    }
                    0x3 => {
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::Load {
                            dst,
                            base: reg_file.read(rs1 as usize),
                            size: 8,
                            offset: imm,
                            flags: MemFlags::default(),
                        });
                    }
                    _ => {}
                }
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x23 => {
                let imm = (((insn >> 7) & 0x1f) | (((insn >> 25) & 0x7f) << 5)) as i32;
                let imm = ((imm as i32) << 20 >> 20) as i64;
                match funct3 {
                    0x0 => b.push(IROp::Store {
                        src: reg_file.read(rs2 as usize),
                        base: reg_file.read(rs1 as usize),
                        size: 1,
                        offset: imm,
                        flags: MemFlags::default(),
                    }),
                    0x1 => b.push(IROp::Store {
                        src: reg_file.read(rs2 as usize),
                        base: reg_file.read(rs1 as usize),
                        size: 2,
                        offset: imm,
                        flags: MemFlags::default(),
                    }),
                    0x2 => b.push(IROp::Store {
                        src: reg_file.read(rs2 as usize),
                        base: reg_file.read(rs1 as usize),
                        size: 4,
                        offset: imm,
                        flags: MemFlags::default(),
                    }),
                    0x3 => b.push(IROp::Store {
                        src: reg_file.read(rs2 as usize),
                        base: reg_file.read(rs1 as usize),
                        size: 8,
                        offset: imm,
                        flags: MemFlags::default(),
                    }),
                    _ => {}
                }
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x63 => {
                let imm = ((((insn >> 31) & 0x1) << 12)
                    | (((insn >> 7) & 0x1) << 11)
                    | (((insn >> 25) & 0x3f) << 5)
                    | (((insn >> 8) & 0xf) << 1)) as i32;
                let imm = ((imm as i32) << 19 >> 19) as i64;
                let target = ((pc as i64).wrapping_add(imm)) as u64;
                match funct3 {
                    0x0 => {
                        b.push(IROp::CmpEq {
                            dst: 31,
                            lhs: reg_file.read(rs1 as usize),
                            rhs: reg_file.read(rs2 as usize),
                        });
                        b.set_term(Terminator::CondJmp {
                            cond: 31,
                            target_true: target,
                            target_false: pc.wrapping_add(4),
                        });
                    }
                    0x1 => {
                        b.push(IROp::CmpNe {
                            dst: 31,
                            lhs: reg_file.read(rs1 as usize),
                            rhs: reg_file.read(rs2 as usize),
                        });
                        b.set_term(Terminator::CondJmp {
                            cond: 31,
                            target_true: target,
                            target_false: pc.wrapping_add(4),
                        });
                    }
                    0x4 => {
                        b.push(IROp::CmpLt {
                            dst: 31,
                            lhs: reg_file.read(rs1 as usize),
                            rhs: reg_file.read(rs2 as usize),
                        });
                        b.set_term(Terminator::CondJmp {
                            cond: 31,
                            target_true: target,
                            target_false: pc.wrapping_add(4),
                        });
                    }
                    0x5 => {
                        b.push(IROp::CmpGe {
                            dst: 31,
                            lhs: reg_file.read(rs1 as usize),
                            rhs: reg_file.read(rs2 as usize),
                        });
                        b.set_term(Terminator::CondJmp {
                            cond: 31,
                            target_true: target,
                            target_false: pc.wrapping_add(4),
                        });
                    }
                    0x6 => {
                        b.push(IROp::CmpLtU {
                            dst: 31,
                            lhs: reg_file.read(rs1 as usize),
                            rhs: reg_file.read(rs2 as usize),
                        });
                        b.set_term(Terminator::CondJmp {
                            cond: 31,
                            target_true: target,
                            target_false: pc.wrapping_add(4),
                        });
                    }
                    0x7 => {
                        b.push(IROp::CmpGeU {
                            dst: 31,
                            lhs: reg_file.read(rs1 as usize),
                            rhs: reg_file.read(rs2 as usize),
                        });
                        b.set_term(Terminator::CondJmp {
                            cond: 31,
                            target_true: target,
                            target_false: pc.wrapping_add(4),
                        });
                    }
                    _ => {
                        b.set_term(Terminator::Jmp {
                            target: pc.wrapping_add(4),
                        });
                    }
                }
            }
            0x6f => {
                let i = insn;
                let raw = (((i >> 31) & 0x1) << 20)
                    | (((i >> 21) & 0x3ff) << 1)
                    | (((i >> 20) & 0x1) << 11)
                    | (((i >> 12) & 0xff) << 12);
                let imm = sext21(raw);
                let dst = reg_file.write(rd as usize);
                b.push(IROp::MovImm {
                    dst,
                    imm: pc.wrapping_add(4),
                });
                b.set_term(Terminator::Jmp {
                    target: ((pc as i64).wrapping_add(imm)) as u64,
                });
            }
            0x2f => {
                // AMO (Atomic Memory Operations)
                let funct5 = (insn >> 27) & 0x1f;
                let aq = (insn >> 26) & 0x1;
                let rl = (insn >> 25) & 0x1;
                let mut flags = MemFlags::default();
                flags.atomic = true;
                if aq != 0 {
                    flags.fence_before = true;
                    flags.order = vm_ir::MemOrder::Acquire;
                }
                if rl != 0 {
                    flags.fence_after = true;
                    flags.order = vm_ir::MemOrder::Release;
                }
                if aq != 0 && rl != 0 {
                    flags.order = vm_ir::MemOrder::AcqRel;
                }

                let size = match funct3 {
                    2 => 4, // .W
                    3 => 8, // .D
                    _ => 4,
                };

                match funct5 {
                    0x02 => {
                        // LR (Load Reserved)
                        let dst = reg_file.write(rd as usize);
                        let mut lr_flags = flags;
                        b.push(IROp::AtomicLoadReserve {
                            dst,
                            base: reg_file.read(rs1 as usize),
                            offset: 0,
                            size,
                            flags: lr_flags,
                        });
                    }
                    0x03 => {
                        // SC (Store Conditional)
                        let dst_flag = reg_file.write(rd as usize);
                        b.push(IROp::AtomicStoreCond {
                            src: reg_file.read(rs2 as usize),
                            base: reg_file.read(rs1 as usize),
                            offset: 0,
                            size,
                            dst_flag,
                            flags,
                        });
                    }
                    0x01 => {
                        // AMOSWAP
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder {
                            dst,
                            base: reg_file.read(rs1 as usize),
                            src: reg_file.read(rs2 as usize),
                            op: vm_ir::AtomicOp::Xchg,
                            size,
                            flags,
                        });
                    }
                    0x00 => {
                        // AMOADD
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder {
                            dst,
                            base: reg_file.read(rs1 as usize),
                            src: reg_file.read(rs2 as usize),
                            op: vm_ir::AtomicOp::Add,
                            size,
                            flags,
                        });
                    }
                    0x04 => {
                        // AMOXOR
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder {
                            dst,
                            base: reg_file.read(rs1 as usize),
                            src: reg_file.read(rs2 as usize),
                            op: vm_ir::AtomicOp::Xor,
                            size,
                            flags,
                        });
                    }
                    0x0c => {
                        // AMOAND
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder {
                            dst,
                            base: reg_file.read(rs1 as usize),
                            src: reg_file.read(rs2 as usize),
                            op: vm_ir::AtomicOp::And,
                            size,
                            flags,
                        });
                    }
                    0x08 => {
                        // AMOOR
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder {
                            dst,
                            base: reg_file.read(rs1 as usize),
                            src: reg_file.read(rs2 as usize),
                            op: vm_ir::AtomicOp::Or,
                            size,
                            flags,
                        });
                    }
                    0x10 => {
                        // AMOMIN
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder {
                            dst,
                            base: reg_file.read(rs1 as usize),
                            src: reg_file.read(rs2 as usize),
                            op: vm_ir::AtomicOp::MinS,
                            size,
                            flags,
                        });
                    }
                    0x14 => {
                        // AMOMAX
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder {
                            dst,
                            base: reg_file.read(rs1 as usize),
                            src: reg_file.read(rs2 as usize),
                            op: vm_ir::AtomicOp::MaxS,
                            size,
                            flags,
                        });
                    }
                    0x18 => {
                        // AMOMINU
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder {
                            dst,
                            base: reg_file.read(rs1 as usize),
                            src: reg_file.read(rs2 as usize),
                            op: vm_ir::AtomicOp::Min,
                            size,
                            flags,
                        });
                    }
                    0x1c => {
                        // AMOMAXU
                        let dst = reg_file.write(rd as usize);
                        b.push(IROp::AtomicRMWOrder {
                            dst,
                            base: reg_file.read(rs1 as usize),
                            src: reg_file.read(rs2 as usize),
                            op: vm_ir::AtomicOp::Max,
                            size,
                            flags,
                        });
                    }
                    _ => {}
                }
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
            0x67 => {
                let imm = ((insn as i32) >> 20) as i64;
                let dst = reg_file.write(rd as usize);
                b.push(IROp::MovImm {
                    dst,
                    imm: pc.wrapping_add(4),
                });

                let t29 = reg_file.write(29);
                b.push(IROp::AddImm {
                    dst: t29,
                    src: reg_file.read(rs1 as usize),
                    imm,
                });

                let t28 = reg_file.write(28);
                b.push(IROp::MovImm {
                    dst: t28,
                    imm: !1u64,
                });

                let t29_new = reg_file.write(29);
                b.push(IROp::And {
                    dst: t29_new,
                    src1: t29,
                    src2: t28,
                });

                b.set_term(Terminator::JmpReg {
                    base: t29_new,
                    offset: 0,
                });
            }
            // CSR instructions are handled above in the 0x73 opcode section
            // This section is kept for backward compatibility but should not be reached
            // as CSR instructions are now handled in the 0x73 opcode match above
            _ => {
                b.set_term(Terminator::Jmp {
                    target: pc.wrapping_add(4),
                });
            }
        }

        let block = b.build();

        Ok(block)
    }
}

pub fn encode_jal(rd: u32, imm: i32) -> u32 {
    let min = (-(1 << 20)) << 1;
    let max = (((1 << 20) - 1) << 1) as i32;
    let mut v = imm;
    if (v & 1) != 0 {
        v &= !1;
    }
    if v < min {
        v = min;
    }
    if v > max {
        v = max;
    }
    let u = v as u32;
    let b20 = ((u >> 20) & 0x1) << 31;
    let b10_1 = ((u >> 1) & 0x3ff) << 21;
    let b11 = ((u >> 11) & 0x1) << 20;
    let b19_12 = ((u >> 12) & 0xff) << 12;
    b20 | b10_1 | b11 | b19_12 | (rd << 7) | 0x6f
}

fn clamp_i_imm(mut imm: i32) -> i32 {
    let min = -(1 << 11) - 1;
    let max = (1 << 11) - 1;
    if imm < min {
        imm = min;
    }
    if imm > max {
        imm = max;
    }
    imm
}

pub fn encode_jalr(rd: u32, rs1: u32, imm: i32) -> u32 {
    let v = clamp_i_imm(imm) as u32;
    (v << 20) | (rs1 << 15) | (0 << 12) | (rd << 7) | 0x67
}

pub fn encode_jalr_with_align(rd: u32, rs1: u32, imm: i32, align_even: bool) -> u32 {
    let mut v = clamp_i_imm(imm);
    if align_even {
        v &= !1;
    }
    ((v as u32) << 20) | (rs1 << 15) | (0 << 12) | (rd << 7) | 0x67
}

pub fn encode_auipc(rd: u32, upper: u32) -> u32 {
    ((upper & 0xfffff) << 12) | (rd << 7) | 0x17
}

pub mod api {
    pub use super::{
        encode_add, encode_addi, encode_auipc, encode_beq, encode_bge, encode_bgeu, encode_blt,
        encode_bltu, encode_bne, encode_branch, encode_jal, encode_jalr, encode_jalr_with_align,
        encode_lw, encode_sub, encode_sw,
    };
}

fn clamp_b_imm(mut imm: i32) -> i32 {
    let min = (-(1 << 12)) << 1;
    let max = (((1 << 12) - 1) << 1) as i32;
    if (imm & 1) != 0 {
        imm &= !1;
    }
    if imm < min {
        imm = min;
    }
    if imm > max {
        imm = max;
    }
    imm
}

pub fn encode_branch(funct3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 {
    let v = clamp_b_imm(imm) as u32;
    let b12 = ((v >> 12) & 0x1) << 31;
    let b11 = ((v >> 11) & 0x1) << 7;
    let b10_5 = ((v >> 5) & 0x3f) << 25;
    let b4_1 = ((v >> 1) & 0xf) << 8;
    b12 | b10_5 | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | b4_1 | b11 | 0x63
}

pub fn encode_beq(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_branch(0x0, rs1, rs2, imm)
}
pub fn encode_bne(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_branch(0x1, rs1, rs2, imm)
}
pub fn encode_blt(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_branch(0x4, rs1, rs2, imm)
}
pub fn encode_bge(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_branch(0x5, rs1, rs2, imm)
}
pub fn encode_bltu(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_branch(0x6, rs1, rs2, imm)
}
pub fn encode_bgeu(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_branch(0x7, rs1, rs2, imm)
}

pub fn encode_r_type(opcode: u32, funct3: u32, funct7: u32, rd: u32, rs1: u32, rs2: u32) -> u32 {
    (funct7 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}

pub fn encode_i_type(opcode: u32, funct3: u32, rd: u32, rs1: u32, imm: i32) -> u32 {
    let imm = clamp_i_imm(imm) as u32;
    (imm << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}

pub fn encode_s_type(opcode: u32, funct3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 {
    let imm = clamp_i_imm(imm) as u32;
    let imm11_5 = (imm >> 5) & 0x7f;
    let imm4_0 = imm & 0x1f;
    (imm11_5 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (imm4_0 << 7) | opcode
}

pub fn encode_add(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0x33, 0x0, 0x00, rd, rs1, rs2)
}
pub fn encode_sub(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0x33, 0x0, 0x20, rd, rs1, rs2)
}
pub fn encode_addi(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0x13, 0x0, rd, rs1, imm)
}
pub fn encode_lw(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0x03, 0x2, rd, rs1, imm)
}
pub fn encode_sw(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_s_type(0x23, 0x2, rs1, rs2, imm)
}
