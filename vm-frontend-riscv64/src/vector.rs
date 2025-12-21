//! RISC-V Vector Extension (RV64V) 支持
//!
//! 实现 RISC-V 向量扩展指令的解码和执行

use vm_core::{GuestAddr, MMU, VmError};
use vm_ir::{IRBlock, IROp, MemFlags, Terminator};

/// RV64V 向量指令类型
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)] // 暂时允许未使用的变体，因为向量扩展仍在开发中
pub enum VectorInstruction {
    // 向量加载/存储
    Vle8,
    Vle16,
    Vle32,
    Vle64,
    Vse8,
    Vse16,
    Vse32,
    Vse64,
    // 向量算术
    Vadd,
    Vsub,
    Vmul,
    Vdiv,
    Vaddw,
    Vsubw,
    Vmulw,
    // 向量逻辑
    Vand,
    Vor,
    Vxor,
    Vsll,
    Vsrl,
    Vsra,
    // 向量比较
    Vmseq,
    Vmsne,
    Vmslt,
    Vmsle,
    Vmsgt,
    Vmsge,
    // 向量归约
    Vredsum,
    Vredmax,
    Vredmin,
    Vredand,
    Vredor,
    Vredxor,
    // 向量掩码
    Vmand,
    Vmor,
    Vmxor,
    Vmnot,
}

/// 向量扩展解码器
pub struct VectorDecoder;

impl VectorDecoder {
    /// 解码向量指令
    /// RV64V 使用 opcode = 0x57
    pub fn decode(insn: u32) -> Option<VectorInstruction> {
        let opcode = insn & 0x7f;

        // RV64V 使用 opcode 0x57
        if opcode != 0x57 {
            return None;
        }

        let funct3 = (insn >> 12) & 0x7;
        let funct6 = (insn >> 26) & 0x3f;
        let _vm = (insn >> 25) & 0x1;

        // 根据 funct3 和 funct6 解码指令
        match funct3 {
            0b000 => {
                // 向量加载指令
                match funct6 & 0x7 {
                    0b000000 => Some(VectorInstruction::Vle8),
                    0b000101 => Some(VectorInstruction::Vle16),
                    0b000110 => Some(VectorInstruction::Vle32),
                    0b000111 => Some(VectorInstruction::Vle64),
                    _ => None,
                }
            }
            0b101 => {
                // 向量存储指令
                match funct6 & 0x7 {
                    0b000000 => Some(VectorInstruction::Vse8),
                    0b000101 => Some(VectorInstruction::Vse16),
                    0b000110 => Some(VectorInstruction::Vse32),
                    0b000111 => Some(VectorInstruction::Vse64),
                    _ => None,
                }
            }
            0b001 => {
                // 向量算术指令 (OP-VV)
                match funct6 {
                    0b000000 => Some(VectorInstruction::Vadd),
                    0b000010 => Some(VectorInstruction::Vsub),
                    0b100101 => Some(VectorInstruction::Vmul),
                    0b100000 => Some(VectorInstruction::Vdiv),
                    _ => None,
                }
            }
            0b010 => {
                // 向量逻辑指令
                match funct6 {
                    0b000000 => Some(VectorInstruction::Vand),
                    0b000010 => Some(VectorInstruction::Vor),
                    0b000011 => Some(VectorInstruction::Vxor),
                    0b000100 => Some(VectorInstruction::Vsrl),
                    0b000101 => Some(VectorInstruction::Vsra),
                    _ => None,
                }
            }
            0b011 => {
                // 向量比较指令
                match funct6 {
                    0b011000 => Some(VectorInstruction::Vmseq),
                    0b011001 => Some(VectorInstruction::Vmsne),
                    0b011011 => Some(VectorInstruction::Vmslt),
                    0b011100 => Some(VectorInstruction::Vmsle),
                    0b011101 => Some(VectorInstruction::Vmsgt),
                    0b011110 => Some(VectorInstruction::Vmsge),
                    _ => None,
                }
            }
            0b100 => {
                // 向量归约指令
                match funct6 {
                    0b000000 => Some(VectorInstruction::Vredsum),
                    0b000001 => Some(VectorInstruction::Vredmax),
                    0b000010 => Some(VectorInstruction::Vredmin),
                    0b000110 => Some(VectorInstruction::Vredand),
                    0b000100 => Some(VectorInstruction::Vredor),
                    0b000101 => Some(VectorInstruction::Vredxor),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// 将向量指令转换为 IR
    pub fn to_ir(
        insn: u32,
        reg_file: &mut vm_ir::RegisterFile,
        builder: &mut vm_ir::IRBuilder,
        _mmu: &dyn MMU,
        pc: GuestAddr,
    ) -> Result<IRBlock, VmError> {
        let rd = (insn >> 7) & 0x1f;
        let rs1 = (insn >> 15) & 0x1f;
        let rs2 = (insn >> 20) & 0x1f;
        let _vm = (insn >> 25) & 0x1;
        let _funct3 = (insn >> 12) & 0x7;

        // 提取向量元素大小（从 funct6 或指令格式推断）
        // 对于向量指令，元素大小通常由 vtype 寄存器决定，这里简化处理
        let element_size = Self::extract_element_size(insn);

        // 将向量指令映射到 IR 的向量操作
        match Self::decode(insn) {
            Some(VectorInstruction::Vadd) => {
                // vadd.vv: vd = vs1 + vs2
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::VecAdd {
                    dst,
                    src1,
                    src2,
                    element_size,
                });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vsub) => {
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::VecSub {
                    dst,
                    src1,
                    src2,
                    element_size,
                });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vmul) => {
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::VecMul {
                    dst,
                    src1,
                    src2,
                    element_size,
                });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vand) => {
                // 向量逻辑操作映射到标量操作（IR 中没有向量逻辑操作）
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::And { dst, src1, src2 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vor) => {
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::Or { dst, src1, src2 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vxor) => {
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::Xor { dst, src1, src2 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vle8)
            | Some(VectorInstruction::Vle16)
            | Some(VectorInstruction::Vle32)
            | Some(VectorInstruction::Vle64) => {
                // 向量加载：简化实现，只加载第一个元素
                let size = match Self::decode(insn) {
                    Some(VectorInstruction::Vle8) => 1,
                    Some(VectorInstruction::Vle16) => 2,
                    Some(VectorInstruction::Vle32) => 4,
                    Some(VectorInstruction::Vle64) => 8,
                    _ => {
                        return Err(VmError::Execution(
                            vm_core::ExecutionError::InvalidInstruction {
                                opcode: insn as u64,
                                pc,
                            },
                        ));
                    }
                };
                let dst = reg_file.write(rd as usize);
                let base = reg_file.read(rs1 as usize);
                // 简化：使用立即数偏移 0
                builder.push(IROp::Load {
                    dst,
                    base,
                    offset: 0,
                    size,
                    flags: MemFlags::default(),
                });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vse8)
            | Some(VectorInstruction::Vse16)
            | Some(VectorInstruction::Vse32)
            | Some(VectorInstruction::Vse64) => {
                // 向量存储：简化实现，只存储第一个元素
                let size = match Self::decode(insn) {
                    Some(VectorInstruction::Vse8) => 1,
                    Some(VectorInstruction::Vse16) => 2,
                    Some(VectorInstruction::Vse32) => 4,
                    Some(VectorInstruction::Vse64) => 8,
                    _ => {
                        return Err(VmError::Execution(
                            vm_core::ExecutionError::InvalidInstruction {
                                opcode: insn as u64,
                                pc,
                            },
                        ));
                    }
                };
                let src = reg_file.read(rs2 as usize);
                let base = reg_file.read(rs1 as usize);
                builder.push(IROp::Store {
                    src,
                    base,
                    offset: 0,
                    size,
                    flags: MemFlags::default(),
                });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vdiv) => {
                // vdiv.vv: vd = vs1 / vs2
                // 注意：IR 中没有向量除法操作，映射到标量除法
                // 向量除法通常是有符号的
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::Div { dst, src1, src2, signed: true });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vsll) => {
                // vsll.vv: vd = vs1 << vs2
                let dst = reg_file.write(rd as usize);
                let src = reg_file.read(rs1 as usize);
                let shreg = reg_file.read(rs2 as usize);
                builder.push(IROp::Sll { dst, src, shreg });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vsrl) => {
                // vsrl.vv: vd = vs1 >> vs2 (逻辑右移)
                let dst = reg_file.write(rd as usize);
                let src = reg_file.read(rs1 as usize);
                let shreg = reg_file.read(rs2 as usize);
                builder.push(IROp::Srl { dst, src, shreg });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vsra) => {
                // vsra.vv: vd = vs1 >> vs2 (算术右移)
                let dst = reg_file.write(rd as usize);
                let src = reg_file.read(rs1 as usize);
                let shreg = reg_file.read(rs2 as usize);
                builder.push(IROp::Sra { dst, src, shreg });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vmseq) => {
                // vmseq.vv: vd = (vs1 == vs2)
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::CmpEq { dst, lhs: src1, rhs: src2 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vmsne) => {
                // vmsne.vv: vd = (vs1 != vs2)
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::CmpNe { dst, lhs: src1, rhs: src2 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vmslt) => {
                // vmslt.vv: vd = (vs1 < vs2)
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::CmpLt { dst, lhs: src1, rhs: src2 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vmsle) => {
                // vmsle.vv: vd = (vs1 <= vs2)
                // IR 中没有 <= 操作，使用 !(vs1 > vs2) 或 (vs1 < vs2) || (vs1 == vs2)
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                // 简化：使用 CmpGe 的逆
                builder.push(IROp::CmpGe { dst, lhs: src2, rhs: src1 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vmsgt) => {
                // vmsgt.vv: vd = (vs1 > vs2)
                // 使用 CmpLt 的逆：vs1 > vs2 等价于 vs2 < vs1
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::CmpLt { dst, lhs: src2, rhs: src1 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vmsge) => {
                // vmsge.vv: vd = (vs1 >= vs2)
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::CmpGe { dst, lhs: src1, rhs: src2 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vredsum)
            | Some(VectorInstruction::Vredmax)
            | Some(VectorInstruction::Vredmin)
            | Some(VectorInstruction::Vredand)
            | Some(VectorInstruction::Vredor)
            | Some(VectorInstruction::Vredxor) => {
                // 向量归约指令：将向量归约为标量
                // 简化实现：映射到标量操作（实际应该实现真正的归约）
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                match Self::decode(insn) {
                    Some(VectorInstruction::Vredsum) => {
                        builder.push(IROp::Add { dst, src1, src2 });
                    }
                    Some(VectorInstruction::Vredmax) | Some(VectorInstruction::Vredmin) => {
                        // 归约最大/最小值需要特殊处理，这里简化
                        // 使用 CmpGe 来比较
                        builder.push(IROp::CmpGe { dst, lhs: src1, rhs: src2 });
                    }
                    Some(VectorInstruction::Vredand) => {
                        builder.push(IROp::And { dst, src1, src2 });
                    }
                    Some(VectorInstruction::Vredor) => {
                        builder.push(IROp::Or { dst, src1, src2 });
                    }
                    Some(VectorInstruction::Vredxor) => {
                        builder.push(IROp::Xor { dst, src1, src2 });
                    }
                    _ => {}
                }
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vmand)
            | Some(VectorInstruction::Vmor)
            | Some(VectorInstruction::Vmxor)
            | Some(VectorInstruction::Vmnot) => {
                // 向量掩码操作：操作掩码寄存器
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = if matches!(
                    Self::decode(insn),
                    Some(VectorInstruction::Vmnot)
                ) {
                    src1 // Vmnot 只需要一个源操作数
                } else {
                    reg_file.read(rs2 as usize)
                };
                match Self::decode(insn) {
                    Some(VectorInstruction::Vmand) => {
                        builder.push(IROp::And { dst, src1, src2 });
                    }
                    Some(VectorInstruction::Vmor) => {
                        builder.push(IROp::Or { dst, src1, src2 });
                    }
                    Some(VectorInstruction::Vmxor) => {
                        builder.push(IROp::Xor { dst, src1, src2 });
                    }
            Some(VectorInstruction::Vmnot) => {
                // Vmnot: vd = !vs1
                builder.push(IROp::Not { dst, src: src1 });
            }
                    _ => {}
                }
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            _ => Err(VmError::Execution(
                vm_core::ExecutionError::InvalidInstruction {
                    opcode: insn as u64,
                    pc,
                },
            )),
        }
    }

    /// 提取向量元素大小
    /// 
    /// 从指令编码中推断元素大小，或使用默认值
    pub fn extract_element_size(insn: u32) -> u8 {
        let funct6 = (insn >> 26) & 0x3f;
        let funct3 = (insn >> 12) & 0x7;

        // 对于加载/存储指令，可以从 funct6 的低3位推断元素大小
        if funct3 == 0b000 || funct3 == 0b101 {
            match funct6 & 0x7 {
                0b000000 => 1,  // 8位
                0b000101 => 2,  // 16位
                0b000110 => 4,  // 32位
                0b000111 => 8,  // 64位
                _ => 4,         // 默认32位
            }
        } else {
            // 对于其他向量指令，默认使用32位元素
            // 实际应该从 vtype 寄存器读取 SEW
            4
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_decode() {
        // VADD.VV vd, vs1, vs2
        // opcode=0x57, funct6=0b000000, funct3=0b001, vm=0, vs2=0, vs1=0, vd=0
        let insn: u32 = 0x57 // opcode
            | (0b001 << 12) // funct3 for OP-VV
            | (0b000000 << 26); // funct6 for VADD.VV
        assert_eq!(VectorDecoder::decode(insn), Some(VectorInstruction::Vadd));
    }
}
