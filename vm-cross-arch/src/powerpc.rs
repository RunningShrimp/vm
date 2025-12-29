//! PowerPC 架构支持
//!
//! 提供 PowerPC 指令集的解码、编码和转换功能

use vm_core::{GuestArch, VmError};
use vm_ir::{IRInstruction, MemFlags, Operand, RegId, RegIdExt};

/// PowerPC 寄存器
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowerPCReg {
    GPR(u8),  // 通用寄存器 R0-R31
    FPR(u8),  // 浮点寄存器 F0-F31
    CR(u8),   // 条件寄存器 CR0-CR7
    XER,      // 异常寄存器
    LR,       // 链接寄存器
    CTR,      // 计数寄存器
    SPR(u16), // 特殊寄存器
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
    pc: u64, // Program counter for relative branches
}

impl PowerPCDecoder {
    pub fn new(arch: GuestArch) -> Self {
        Self { arch, pc: 0 }
    }

    /// Set the program counter for decoding relative branches
    pub fn set_pc(&mut self, pc: u64) {
        self.pc = pc;
    }

    /// Get the current program counter
    pub fn get_pc(&self) -> u64 {
        self.pc
    }

    /// Check if the decoder is configured for 64-bit PowerPC
    pub fn is_64bit(&self) -> bool {
        matches!(self.arch, GuestArch::PowerPC64)
    }

    /// 解码 PowerPC 指令
    pub fn decode(&self, bytes: &[u8]) -> Result<IRInstruction, VmError> {
        if bytes.len() < 4 {
            return Err(VmError::Core(vm_core::CoreError::DecodeError {
                message: "Instruction too short".to_string(),
                position: None,
                module: "powerpc".to_string(),
            }));
        }

        // Validate architecture-specific constraints
        self.validate_architecture()?;

        let instr = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let opcode = (instr >> 26) & 0x3F;

        match opcode {
            0x10 => self.decode_branch(instr),
            0x11 => self.decode_cond_branch(instr),
            0x0E => self.decode_addi(instr), // Fixed: ADDI opcode is 14 (0x0E), not 20 (0x14)
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

    /// Validate that the architecture is properly configured
    fn validate_architecture(&self) -> Result<(), VmError> {
        match self.arch {
            GuestArch::PowerPC64 => Ok(()),
            _ => Err(VmError::Core(vm_core::CoreError::DecodeError {
                message: format!("Invalid architecture for PowerPC decoder: {:?}", self.arch),
                position: None,
                module: "powerpc".to_string(),
            })),
        }
    }

    fn decode_branch(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let link_bit = instr & 0x1 != 0;
        let aa_bit = (instr >> 1) & 0x1 != 0;
        let li = (instr & 0x3FFFFFC) as i32;

        // Use architecture to determine address size
        let target = if aa_bit {
            li as u64
        } else {
            // Sign-extend for 64-bit if needed
            let offset = if self.is_64bit() {
                li as i64 as u64
            } else {
                (li as u32) as u64
            };
            self.pc.wrapping_add(offset)
        };

        Ok(IRInstruction::Branch {
            target: Operand::Imm64(target),
            link: link_bit,
        })
    }

    fn decode_cond_branch(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let _bo = (instr >> 21) & 0x1F;
        let bi = (instr >> 16) & 0x1F;
        let bd = ((instr & 0xFFFC) as i16) as i32;
        let link_bit = instr & 0x1 != 0;
        let aa_bit = (instr >> 1) & 0x1 != 0;

        let target = if aa_bit {
            bd as u64
        } else {
            // Sign-extend based on architecture
            let offset = if self.is_64bit() {
                bd as i64 as u64
            } else {
                (bd as u16) as u64
            };
            self.pc.wrapping_add(offset)
        };

        Ok(IRInstruction::CondBranch {
            condition: Operand::Reg(RegId::new(bi)),
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

        Ok(IRInstruction::LoadExt {
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
            flags: MemFlags::default(),
        })
    }

    fn decode_lbz(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let rt = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::LoadExt {
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
            flags: MemFlags::default(),
        })
    }

    fn decode_stw(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let rs = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::StoreExt {
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
            flags: MemFlags::default(),
        })
    }

    fn decode_stb(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let rs = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::StoreExt {
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
            flags: MemFlags::default(),
        })
    }

    fn decode_lha(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let rt = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::LoadExt {
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
            flags: MemFlags::default(),
        })
    }

    fn decode_lhz(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let rt = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::LoadExt {
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
            flags: MemFlags::default(),
        })
    }

    fn decode_sth(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let rs = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::StoreExt {
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
            flags: MemFlags::default(),
        })
    }

    fn decode_load_float(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let frt = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::LoadExt {
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
            flags: MemFlags::default(),
        })
    }

    fn decode_store_float(&self, instr: u32) -> Result<IRInstruction, VmError> {
        let frs = (instr >> 21) & 0x1F;
        let ra = (instr >> 16) & 0x1F;
        let d = ((instr & 0xFFFF) as i16) as i64;

        Ok(IRInstruction::StoreExt {
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
            flags: MemFlags::default(),
        })
    }

    fn decode_primary(&self, _opcode: u32, instr: u32) -> Result<IRInstruction, VmError> {
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
}

/// PowerPC 编码器
pub struct PowerPCEncoder {
    arch: GuestArch,
}

impl PowerPCEncoder {
    pub fn new(arch: GuestArch) -> Self {
        Self { arch }
    }

    /// Check if the encoder is configured for 64-bit PowerPC
    pub fn is_64bit(&self) -> bool {
        matches!(self.arch, GuestArch::PowerPC64)
    }

    pub fn encode(&self, ir_instr: &IRInstruction) -> Result<Vec<u8>, VmError> {
        // Validate architecture before encoding
        self.validate_architecture()?;

        match ir_instr {
            IRInstruction::BinaryOp {
                op,
                dest,
                src1,
                src2,
            } => self.encode_binary_op(*op, *dest, src1, src2),
            IRInstruction::Branch { target, .. } => self.encode_branch(target),
            IRInstruction::LoadExt {
                dest,
                addr,
                size,
                flags: _,
            } => self.encode_load(*dest, addr, (*size) as usize),
            IRInstruction::StoreExt {
                value,
                addr,
                size,
                flags: _,
            } => self.encode_store(value, addr, (*size) as usize),
            _ => Ok(vec![0x00, 0x00, 0x00, 0x00]),
        }
    }

    /// Validate that the architecture is properly configured
    fn validate_architecture(&self) -> Result<(), VmError> {
        match self.arch {
            GuestArch::PowerPC64 => Ok(()),
            _ => Err(VmError::Core(vm_core::CoreError::NotSupported {
                feature: format!("Invalid architecture for PowerPC encoder: {:?}", self.arch),
                module: "powerpc".to_string(),
            })),
        }
    }

    fn encode_binary_op(
        &self,
        op: vm_ir::BinaryOperator,
        dest: RegId,
        _src1: &Operand,
        _src2: &Operand,
    ) -> Result<Vec<u8>, VmError> {
        let (rt, ra, rb) = (dest.id() as u8, 0u8, 0u8);
        let primary = match op {
            vm_ir::BinaryOperator::Add => 266,
            vm_ir::BinaryOperator::Sub => 40,
            _ => return Ok(vec![0x00, 0x00, 0x00, 0x00]),
        };

        let instr = ((rt as u32) << 21)
            | ((ra as u32) << 16)
            | ((rb as u32) << 11)
            | ((primary as u32) << 1);
        Ok(instr.to_be_bytes().to_vec())
    }

    fn encode_branch(&self, target: &Operand) -> Result<Vec<u8>, VmError> {
        if let Operand::Imm64(target_addr) = target {
            // Use architecture to validate branch target
            let offset = if self.is_64bit() {
                *target_addr as i64 as i32
            } else {
                *target_addr as i32
            };

            let instr = (0x10u32 << 26) | ((offset as u32) & 0x3FFFFFC);
            Ok(instr.to_be_bytes().to_vec())
        } else {
            Err(VmError::Core(vm_core::CoreError::NotSupported {
                feature: "encode_branch".to_string(),
                module: "powerpc".to_string(),
            }))
        }
    }

    fn encode_load(&self, dest: RegId, _addr: &Operand, size: usize) -> Result<Vec<u8>, VmError> {
        // Validate size based on architecture
        let max_size = if self.is_64bit() { 8 } else { 4 };
        if size > max_size {
            return Err(VmError::Core(vm_core::CoreError::NotSupported {
                feature: format!("load size {} for {:?}", size, self.arch),
                module: "powerpc".to_string(),
            }));
        }

        let opcode = match size {
            1 => 0x21,
            2 => 0x2C,
            4 => 0x20,
            _ => {
                return Err(VmError::Core(vm_core::CoreError::NotSupported {
                    feature: format!("load size {}", size),
                    module: "powerpc".to_string(),
                }));
            }
        };

        let rt = dest.id() as u8;
        let ra = 0u8;
        let d = 0i16;

        let instr =
            (opcode as u32) << 26 | (rt as u32) << 21 | (ra as u32) << 16 | (d as u16) as u32;
        Ok(instr.to_be_bytes().to_vec())
    }

    fn encode_store(
        &self,
        value: &Operand,
        _addr: &Operand,
        size: usize,
    ) -> Result<Vec<u8>, VmError> {
        let opcode = match size {
            1 => 0x23,
            2 => 0x2D,
            4 => 0x22,
            _ => {
                return Err(VmError::Core(vm_core::CoreError::NotSupported {
                    feature: format!("store size {}", size),
                    module: "powerpc".to_string(),
                }));
            }
        };

        let rs = if let Operand::Reg(reg) = value {
            reg.id() as u8
        } else {
            return Err(VmError::Core(vm_core::CoreError::NotSupported {
                feature: "encode_store".to_string(),
                module: "powerpc".to_string(),
            }));
        };

        let ra = 0u8;
        let d = 0i16;

        let instr =
            (opcode as u32) << 26 | (rs as u32) << 21 | (ra as u32) << 16 | (d as u16) as u32;
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

        let result = decoder
            .decode(&instr)
            .expect("Failed to decode ADDI instruction");
        if let IRInstruction::BinaryOp {
            op,
            dest: _,
            src1: _,
            src2: _,
        } = result
        {
            assert_eq!(op, vm_ir::BinaryOperator::Add);
        } else {
            panic!("Expected BinaryOp");
        }
    }

    #[test]
    fn test_encode_branch() {
        let encoder = PowerPCEncoder::new(GuestArch::PowerPC64);
        let target = Operand::Imm64(0x1000);

        let result = encoder
            .encode_branch(&target)
            .expect("Failed to encode branch");
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_decode_lwz() {
        let decoder = PowerPCDecoder::new(GuestArch::PowerPC64);
        let instr = 0x80000000u32.to_be_bytes();

        let result = decoder
            .decode(&instr)
            .expect("Failed to decode LWZ instruction");
        if let IRInstruction::LoadExt { size, .. } = result {
            assert_eq!(size, 4);
        } else {
            panic!("Expected Load");
        }
    }
}
