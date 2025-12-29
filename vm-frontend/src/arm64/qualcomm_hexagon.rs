//! Qualcomm Hexagon DSP 指令解码器
//!
//! Hexagon DSP 使用 VLIW (Very Long Instruction Word) 架构
//! 支持32位和64位指令包

use vm_core::{GuestAddr, VmError};
use vm_ir::{IRBuilder, IROp, RegId, RegisterFile};

/// Hexagon DSP 指令类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HexagonInstruction {
    /// HEX_LD - 加载指令
    HexLd {
        dst: RegId,
        base: RegId,
        offset: i64,
        size: u8,
    },
    /// HEX_ST - 存储指令
    HexSt {
        src: RegId,
        base: RegId,
        offset: i64,
        size: u8,
    },
    /// HEX_ADD - 加法
    HexAdd {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    /// HEX_MUL - 乘法
    HexMul {
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
    /// HEX_MAC - 乘加
    HexMac {
        dst: RegId,
        src1: RegId,
        src2: RegId,
        acc: RegId,
    },
    /// HEX_VECTOR - 向量运算
    HexVector {
        op: HexVectorOp,
        dst: RegId,
        src1: RegId,
        src2: RegId,
    },
}

/// Hexagon 向量操作类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HexVectorOp {
    Vadd,
    Vsub,
    Vmul,
    Vmax,
    Vmin,
    Vabs,
}

/// Hexagon DSP 指令解码器
pub struct HexagonDecoder;

impl HexagonDecoder {
    /// 创建新的 Hexagon 解码器
    pub fn new() -> Self {
        Self
    }

    /// 解码 Hexagon DSP 指令
    ///
    /// Hexagon 使用 VLIW 格式：
    /// - 32位指令：单个操作
    /// - 64位指令包：多个并行操作
    ///
    /// 简化实现：检测 Hexagon 指令模式
    /// 格式：bits [31:28] = 0b1110 表示 Hexagon 指令
    ///       bits [27:24] = 操作码
    pub fn decode(&self, insn: u32, _pc: GuestAddr) -> Result<Option<HexagonInstruction>, VmError> {
        // 检查是否为 Hexagon 指令（简化检测）
        // bits [31:28] = 0b1110 表示 Hexagon DSP 指令
        if (insn >> 28) & 0xF == 0xE {
            let opcode = (insn >> 24) & 0xF;
            let dst = (insn >> 16) & 0x1F;
            let src1 = (insn >> 8) & 0x1F;
            let src2 = insn & 0x1F;
            let _imm = ((insn & 0xFF) as i8) as i64; // 仅在某些指令中使用

            match opcode {
                0x0 => {
                    // HEX_LD
                    Ok(Some(HexagonInstruction::HexLd {
                        dst: dst as RegId,
                        base: src1 as RegId,
                        offset: _imm,
                        size: ((src2 & 0x3) + 1) as u8, // 1-4 bytes
                    }))
                }
                0x1 => {
                    // HEX_ST
                    Ok(Some(HexagonInstruction::HexSt {
                        src: dst as RegId,
                        base: src1 as RegId,
                        offset: _imm,
                        size: ((src2 & 0x3) + 1) as u8,
                    }))
                }
                0x2 => {
                    // HEX_ADD
                    Ok(Some(HexagonInstruction::HexAdd {
                        dst: dst as RegId,
                        src1: src1 as RegId,
                        src2: src2 as RegId,
                    }))
                }
                0x3 => {
                    // HEX_MUL
                    Ok(Some(HexagonInstruction::HexMul {
                        dst: dst as RegId,
                        src1: src1 as RegId,
                        src2: src2 as RegId,
                    }))
                }
                0x4 => {
                    // HEX_MAC
                    Ok(Some(HexagonInstruction::HexMac {
                        dst: dst as RegId,
                        src1: src1 as RegId,
                        src2: src2 as RegId,
                        acc: ((insn >> 5) & 0x1F) as RegId,
                    }))
                }
                0x5 => {
                    // HEX_VECTOR
                    let vec_op = match (insn >> 8) & 0x7 {
                        0 => HexVectorOp::Vadd,
                        1 => HexVectorOp::Vsub,
                        2 => HexVectorOp::Vmul,
                        3 => HexVectorOp::Vmax,
                        4 => HexVectorOp::Vmin,
                        5 => HexVectorOp::Vabs,
                        _ => HexVectorOp::Vadd,
                    };
                    Ok(Some(HexagonInstruction::HexVector {
                        op: vec_op,
                        dst: dst as RegId,
                        src1: src1 as RegId,
                        src2: src2 as RegId,
                    }))
                }
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    /// 将 Hexagon 指令转换为 IR
    pub fn to_ir(
        &self,
        insn: &HexagonInstruction,
        builder: &mut IRBuilder,
        reg_file: &mut RegisterFile,
    ) -> Result<(), VmError> {
        match insn {
            HexagonInstruction::HexLd {
                dst,
                base,
                offset,
                size,
            } => {
                let dst_reg = reg_file.write(*dst as usize);
                builder.push(IROp::Load {
                    dst: dst_reg,
                    base: *base,
                    offset: *offset,
                    size: *size,
                    flags: vm_ir::MemFlags::default(),
                });
            }
            HexagonInstruction::HexSt {
                src,
                base,
                offset,
                size,
            } => {
                builder.push(IROp::Store {
                    src: *src,
                    base: *base,
                    offset: *offset,
                    size: *size,
                    flags: vm_ir::MemFlags::default(),
                });
            }
            HexagonInstruction::HexAdd { dst, src1, src2 } => {
                let dst_reg = reg_file.write(*dst as usize);
                builder.push(IROp::Add {
                    dst: dst_reg,
                    src1: reg_file.read(*src1 as usize),
                    src2: reg_file.read(*src2 as usize),
                });
            }
            HexagonInstruction::HexMul { dst, src1, src2 } => {
                let dst_reg = reg_file.write(*dst as usize);
                builder.push(IROp::Mul {
                    dst: dst_reg,
                    src1: reg_file.read(*src1 as usize),
                    src2: reg_file.read(*src2 as usize),
                });
            }
            HexagonInstruction::HexMac {
                dst,
                src1,
                src2,
                acc,
            } => {
                // MAC: dst = src1 * src2 + acc
                let dst_reg = reg_file.write(*dst as usize);
                let mul_temp = reg_file.alloc_temp();
                builder.push(IROp::Mul {
                    dst: mul_temp,
                    src1: reg_file.read(*src1 as usize),
                    src2: reg_file.read(*src2 as usize),
                });
                builder.push(IROp::Add {
                    dst: dst_reg,
                    src1: mul_temp,
                    src2: reg_file.read(*acc as usize),
                });
            }
            HexagonInstruction::HexVector {
                op,
                dst,
                src1,
                src2,
            } => {
                let dst_reg = reg_file.write(*dst as usize);
                let op_str = match op {
                    HexVectorOp::Vadd => "add",
                    HexVectorOp::Vsub => "sub",
                    HexVectorOp::Vmul => "mul",
                    HexVectorOp::Vmax => "max",
                    HexVectorOp::Vmin => "min",
                    HexVectorOp::Vabs => "abs",
                };
                builder.push(IROp::VendorVectorOp {
                    dst: dst_reg,
                    op: op_str.to_string(),
                    src1: reg_file.read(*src1 as usize),
                    src2: reg_file.read(*src2 as usize),
                    vendor: "qualcomm_hexagon".to_string(),
                });
            }
        }
        Ok(())
    }
}

impl Default for HexagonDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hexagon_decode_add() {
        let decoder = HexagonDecoder::new();
        // 构造 HEX_ADD 指令：dst=0, src1=1, src2=2
        let insn = 0xE2_00_01_02;
        let result = decoder.decode(insn, 0x1000);
        assert!(result.is_ok());
        if let Ok(Some(HexagonInstruction::HexAdd { dst, src1, src2 })) = result {
            assert_eq!(dst, 0);
            assert_eq!(src1, 1);
            assert_eq!(src2, 2);
        } else {
            panic!("Failed to decode HEX_ADD");
        }
    }
}
