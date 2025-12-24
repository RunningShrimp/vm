//! PowerPC 架构支持
//!
//! 提供 PowerPC 指令集的解码、编码和转换功能

use vm_ir::{IRBlock, IRInstruction, RegId, Operand};
use vm_core::{GuestArch, VmError};

/// PowerPC 寄存器
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowerPCReg {
    GPR(u8),     // 通用寄存器 R0-R31
    FPR(u8),     // 浮点寄存器 F0-F31
    CR(u8),      // 条件寄存器 CR0-CR7
    XER,         // 异常寄存器
    LR,          // 链接寄存器
    CTR,         // 计数寄存器
    SPR(u16),     // 特殊寄存器
}

impl PowerPCReg {
    pub fn is_gpr(&self) -> bool {
        matches!(self, PowerPCReg::GPR(_))
    }

    pub fn is_fpr(&self) -> bool {
        matches!(self, PowerPCReg::FPR(_))
    }
}

/// PowerPC 指令格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerPCOpcode {
    /// 整数运算
    Add,
    Sub,
    Mul,
    Div,
    Addi,
    Addis,
    Subfic,
    Mulli,

    /// 逻辑运算
    And,
    Or,
    Xor,
    Nor,
    Andi,
    Ori,
    Xori,

    /// 移位操作
    Sld,
    Srd,
    Slw,
    Srw,
    Sldi,
    Srwi,
    Srawi,

    /// 分支指令
    B,
    Bl,
    Bc,
    Bclr,
    Bctr,
    Beq,
    Bne,
    Blt,
    Bgt,
    Bge,
    Ble,

    /// 比较指令
    Cmplwi,
    Cmplw,
    Cmpldi,
    Cmpld,

    /// 加载/存储指令
    Lwz,
    Lbz,
    Lha,
    Lhz,
    Lwzu,
    Stw,
    Stb,
    Sth,
    Stwu,
    Lfs,
    Lfd,
    Stfs,
    Stfd,

    /// 浮点运算
    Fadd,
    Fsub,
    Fmul,
    Fdiv,
    Fmadd,
    Fmsub,
    Fnmsub,
    Fnmadd,

    /// 系统调用
    Sc,

    /// 其他
    Nop,
    Mfcr,
    Mtcrf,
    Mflr,
    Mtlr,
    Mfctr,
    Mtctr,
}

/// PowerPC 指令解码器
pub struct PowerPCDecoder {
    arch: GuestArch,
}

impl PowerPCDecoder {
    pub fn new(arch: GuestArch) -> Self {
        Self { arch }
    }

    /// 解码 PowerPC 指令
    pub fn decode(&self, bytes: &[u8]) -> Result<IRInstruction, VmError> {
        if bytes.len() < 4 {
            return Err(VmError::InvalidOperation {
                operation: "decode".to_string(),
                reason: "Instruction too short".to_string(),
            });
        }

        let instr = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let opcode = (instr >> 26) & 0x3F;

        match opcode {
            0x10 => self.decode_branch(instr),
            0x11 => self.decode_cond_branch(instr),
            0x14 => self.decode_addi(instr),
            0x15 => self.decode_addis(instr),
            0x20 => self.decode_lwz(instr),
            0x21 => self.decode_lbz(instr),
            0x22 => self.decode_stw(instr),
            0x23 => self.decode_stb(instr),
            0x28 => self.decode_lha(instr),
            0x2C => self.decode_lhz(instr),
            0x2D => self.decode_sth(instr),
            0x3A => self.decode_load_float(instr),
            0x3B => self.decode_store_float(instr),
            _ => self.decode_primary(opcode, instr),
        }
    }

    fn decode_branch(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let link_bit = (instr >> 0) & 0x1 != 0;
        let aa_bit = (instr >> 1) & 0x1 != 0;
        let li = (instr & 0x3FFFFFC) as i32;
        let target = if aa_bit {
            li as u64
        } else {
            (self.get_pc() as i64 + li as i64) as u64
        };

        Ok(IRInstruction::Branch {
            target: Operand::Imm64(target),
            link: link_bit,
        })
    }

    fn decode_cond_branch(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let bo = (instr >> 21) & 0x1F;
        let bi = (instr >> 16) & 0x1F;
        let bd = ((instr & 0xFFFC) as i16) as i32;
        let link_bit = (instr >> 0) & 0x1 != 0;
        let aa_bit = (instr >> 1) & 0x1 != 0;

        let target = if aa_bit {
            bd as u64
        } else {
            (self.get_pc() as i64 + bd as i64) as u64
        };

        Ok(IRInstruction::CondBranch {
            condition: Operand::Reg(RegId::new(bi as u32)),
            target: Operand::Imm64(target),
            link: link_bit,
        })
    }

    fn decode_addi(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let rt = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let si = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::BinaryOp {
            op: vm_ir::BinaryOperator::Add,
            dest: RegId::new(rt),
            src1: Operand::Reg(RegId::new(ra)),
            src2: Operand::Imm64(si as u64),
        })
    }

    fn decode_addis(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let rt = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let si = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::BinaryOp {
            op: vm_ir::BinaryOperator::Add,
            dest: RegId::new(rt),
            src1: Operand::Reg(RegId::new(ra)),
            src2: Operand::Imm64((si << 16) as u64),
        })
    }

    fn decode_lwz(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let rt = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::Load {
            dest: RegId::new(rt),
            addr: Operand::Binary {
                op: vm_ir::BinaryOperator::Add,
                left: Box::new(if ra != 0 {
                    Operand::Reg(RegId::new(ra))
                } else {
                    Operand::Imm64(0)
                }),
                right: Box::new(Operand::Imm64(d as u64)),
            },
            size: 4,
        })
    }

    fn decode_lbz(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let rt = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::Load {
            dest: RegId::new(rt),
            addr: Operand::Binary {
                op: vm_ir::BinaryOperator::Add,
                left: Box::new(if ra != 0 {
                    Operand::Reg(RegId::new(ra))
                } else {
                    Operand::Imm64(0)
                }),
                right: Box::new(Operand::Imm64(d as u64)),
            },
            size: 1,
        })
    }

    fn decode_stw(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let rs = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::Store {
            value: Operand::Reg(RegId::new(rs)),
            addr: Operand::Binary {
                op: vm_ir::BinaryOperator::Add,
                left: Box::new(if ra != 0 {
                    Operand::Reg(RegId::new(ra))
                } else {
                    Operand::Imm64(0)
                }),
                right: Box::new(Operand::Imm64(d as u64)),
            },
            size: 4,
        })
    }

    fn decode_stb(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let rs = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::Store {
            value: Operand::Reg(RegId::new(rs)),
            addr: Operand::Binary {
                op: vm_ir::BinaryOperator::Add,
                left: Box::new(if ra != 0 {
                    Operand::Reg(RegId::new(ra))
                } else {
                    Operand::Imm64(0)
                }),
                right: Box::new(Operand::Imm64(d as u64)),
            },
            size: 1,
        })
    }

    fn decode_lha(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let rt = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::Load {
            dest: RegId::new(rt),
            addr: Operand::Binary {
                op: vm_ir::BinaryOperator::Add,
                left: Box::new(if ra != 0 {
                    Operand::Reg(RegId::new(ra))
                } else {
                    Operand::Imm64(0)
                }),
                right: Box::new(Operand::Imm64(d as u64)),
            },
            size: 4,
        })
    }

    fn decode_lhz(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let rt = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::Load {
            dest: RegId::new(rt),
            addr: Operand::Binary {
                op: vm_ir::BinaryOperator::Add,
                left: Box::new(if ra != 0 {
                    Operand::Reg(RegId::new(ra))
                } else {
                    Operand::Imm64(0)
                }),
                right: Box::new(Operand::Imm64(d as u64)),
            },
            size: 2,
        })
    }

    fn decode_sth(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let rs = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::Store {
            value: Operand::Reg(RegId::new(rs)),
            addr: Operand::Binary {
                op: vm_ir::BinaryOperator::Add,
                left: Box::new(if ra != 0 {
                    Operand::Reg(RegId::new(ra))
                } else {
                    Operand::Imm64(0)
                }),
                right: Box::new(Operand::Imm64(d as u64)),
            },
            size: 2,
        })
    }

    fn decode_load_float(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let frt = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::Load {
            dest: RegId::new(frt),
            addr: Operand::Binary {
                op: vm_ir::BinaryOperator::Add,
                left: Box::new(if ra != 0 {
                    Operand::Reg(RegId::new(ra))
                } else {
                    Operand::Imm64(0)
                }),
                right: Box::new(Operand::Imm64(d as u64)),
            },
            size: 4,
        })
    }

    fn decode_store_float(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let frs = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::Store {
            value: Operand::Reg(RegId::new(frs)),
            addr: Operand::Binary {
                op: vm_ir::BinaryOperator::Add,
                left: Box::new(if ra != 0 {
                    Operand::Reg(RegId::new(ra))
                } else {
                    Operand::Imm64(0)
                }),
                right: Box::new(Operand::Imm64(d as u64)),
            },
            size: 4,
        })
    }

    fn decode_primary(&self, opcode: u32, instr: u32) -> Result<IRInstruction, VmError> {
        let primary = (instr >> 1) & 0x3FF;
        let rt = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let rb = (instr >> 11) & 0x1F;

        match primary {
            266 => Ok(IRInstruction::BinaryOp {
                op: vm_ir::BinaryOperator::Add,
                dest: RegId::new(rt),
                src1: Operand::Reg(RegId::new(ra)),
                src2: Operand::Reg(RegId::new(rb)),
            }),
            40 => Ok(IRInstruction::BinaryOp {
                op: vm_ir::BinaryOperator::Sub,
                dest: RegId::new(rt),
                src1: Operand::Reg(RegId::new(ra)),
                src2: Operand::Reg(RegId::new(rb)),
            }),
            _ => Ok(IRInstruction::Nop),
        }
    }

    fn get_pc(&self) -> u64 {
        0
    }
}

/// PowerPC 编码器
pub struct PowerPCEncoder {
    arch: GuestArch,
}

impl PowerPCEncoder {
    pub fn new(arch: GuestArch) -> Self {
        Self { arch }
    }

    pub fn encode(&self, ir_instr: &IRInstruction) -> Result<Vec<u8>, VmError> {
        match ir_instr {
            IRInstruction::BinaryOp { op, dest, src1, src2 } => {
                self.encode_binary_op(*op, *dest, src1, src2)
            }
            IRInstruction::Branch { target, .. } => {
                self.encode_branch(target)
            }
            IRInstruction::Load { dest, addr, size } => {
                self.encode_load(*dest, addr, *size)
            }
            IRInstruction::Store { value, addr, size } => {
                self.encode_store(value, addr, *size)
            }
            _ => Ok(vec![0x00, 0x00, 0x00, 0x00]),
        }
    }

    fn encode_binary_op(&self, op: vm_ir::BinaryOperator, dest: RegId, src1: &Operand, src2: &Operand) -> Result<Vec<u8>, VmError> {
        let (rt, ra, rb) = (dest.id() as u8, 0u8, 0u8);
        let primary = match op {
            vm_ir::BinaryOperator::Add => 266,
            vm_ir::BinaryOperator::Sub => 40,
            _ => return Ok(vec![0x00, 0x00, 0x00, 0x00]),
        };

        let instr = (0u32 << 26) | (rt as u32) << 21) | (ra as u32) << 16) | (rb as u32) << 11) | primary << 1;
        Ok(instr.to_be_bytes().to_vec())
    }

    fn encode_branch(&self, target: &Operand) -> Result<Vec<u8>, VmError> {
        if let Operand::Imm64(target_addr) = target {
            let offset = *target_addr as i32;
            let instr = (0x10u32 << 26) | ((offset & 0x3FFFFFC) as u32);
            Ok(instr.to_be_bytes().to_vec())
        } else {
            Err(VmError::InvalidOperation {
                operation: "encode_branch".to_string(),
                reason: "Target must be immediate".to_string(),
            })
        }
    }

    fn encode_load(&self, dest: RegId, addr: &Operand, size: usize) -> Result<Vec<u8>, VmError> {
        let opcode = match size {
            1 => 0x21,
            2 => 0x2C,
            4 => 0x20,
            _ => return Err(VmError::InvalidOperation {
                operation: "encode_load".to_string(),
                reason: format!("Unsupported load size: {}", size),
            }),
        };

        let rt = dest.id() as u8;
        let ra = 0u8;
        let d = 0i16;

        let instr = (opcode as u32) << 26 | (rt as u32) << 21 | (ra as u32) << 16 | (d as u16) as u32;
        Ok(instr.to_be_bytes().to_vec())
    }

    fn encode_store(&self, value: &Operand, addr: &Operand, size: usize) -> Result<Vec<u8>, VmError> {
        let opcode = match size {
            1 => 0x23,
            2 => 0x2D,
            4 => 0x22,
            _ => return Err(VmError::InvalidOperation {
                operation: "encode_store".to_string(),
                reason: format!("Unsupported store size: {}", size),
            }),
        };

        let rs = if let Operand::Reg(reg) = value {
            reg.id() as u8
        } else {
            return Err(VmError::InvalidOperation {
                operation: "encode_store".to_string(),
                reason: "Value must be register".to_string(),
            });
        };

        let ra = 0u8;
        let d = 0i16;

        let instr = (opcode as u32) << 26 | (rs as u32) << 21 | (ra as u32) << 16 | (d as u16) as u32;
        Ok(instr.to_be_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_addi() {
        let decoder = PowerPCDecoder::new(GuestArch::PowerPC64);
        let instr = 0x38210004u32.to_be_bytes();

        let result = decoder.decode(&instr).unwrap();
        if let IRInstruction::BinaryOp { op, dest, src1, src2 } = result {
            assert_eq!(op, vm_ir::BinaryOperator::Add);
        } else {
            panic!("Expected BinaryOp");
        }
    }

    #[test]
    fn test_encode_branch() {
        let encoder = PowerPCEncoder::new(GuestArch::PowerPC64);
        let target = Operand::Imm64(0x1000);

        let result = encoder.encode_branch(&target).unwrap();
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_decode_lwz() {
        let decoder = PowerPCDecoder::new(GuestArch::PowerPC64);
        let instr = 0x80000000u32.to_be_bytes();

        let result = decoder.decode(&instr).unwrap();
        if let IRInstruction::Load { size, .. } = result {
            assert_eq!(size, 4);
        } else {
            panic!("Expected Load");
        }
    }
}
