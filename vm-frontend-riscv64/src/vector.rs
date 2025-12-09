//! RISC-V Vector Extension (RV64V) 支持
//!
//! 实现 RISC-V 向量扩展指令的解码和执行

use vm_core::{GuestAddr, MMU, VmError};
use vm_ir::{IRBlock, IROp, MemFlags, Terminator};

/// RV64V 向量指令类型
#[derive(Debug, Clone, Copy, PartialEq)]
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
        let vm = (insn >> 25) & 0x1;

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
            0b000 => {
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
                    0b000101 => Some(VectorInstruction::Vsll),
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
            0b010 => {
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
        mmu: &dyn MMU,
        pc: GuestAddr,
    ) -> Result<IRBlock, VmError> {
        let rd = ((insn >> 7) & 0x1f);
        let rs1 = ((insn >> 15) & 0x1f);
        let rs2 = ((insn >> 20) & 0x1f);
        let vm = (insn >> 25) & 0x1;
        let funct3 = (insn >> 12) & 0x7;

        // 简化实现：将向量操作映射到标量操作
        // 实际实现应该使用 SIMD 库或向量寄存器
        match Self::decode(insn) {
            Some(VectorInstruction::Vadd) => {
                // vadd.vv: vd = vs1 + vs2
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::Add { dst, src1, src2 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vsub) => {
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::Sub { dst, src1, src2 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vmul) => {
                let dst = reg_file.write(rd as usize);
                let src1 = reg_file.read(rs1 as usize);
                let src2 = reg_file.read(rs2 as usize);
                builder.push(IROp::Mul { dst, src1, src2 });
                builder.set_term(Terminator::Jmp { target: pc + 4 });
                Ok(builder.build_ref())
            }
            Some(VectorInstruction::Vand) => {
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
            _ => Err(VmError::Execution(
                vm_core::ExecutionError::InvalidInstruction {
                    opcode: insn as u64,
                    pc,
                },
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_decode() {
        // VADD.VV vd, vs1, vs2
        // opcode=0x57, funct6=0b000000, vm=0, vs2=0, vs1=0, vd=0
        let insn = 0x00000057 | (0b000000 << 26) | (0 << 25) | (0 << 20) | (0 << 15) | (0 << 7);
        assert_eq!(VectorDecoder::decode(insn), Some(VectorInstruction::Vadd));
    }
}
