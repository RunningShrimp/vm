//! x86-64 操作数解码阶段
//! 解析 ModR/M、SIB 和立即数

use super::opcode_decode::OperandKind;
use super::prefix_decode::PrefixInfo;

/// 解析结果的内存寻址模式
#[derive(Debug, Clone)]
pub enum MemoryOperand {
    Direct {
        base: u8,
        disp: i64,
    },
    Indexed {
        base: Option<u8>,
        index: u8,
        scale: u8,
        disp: i64,
    },
    Rip {
        disp: i32,
    },
}

/// 解析的操作数
#[derive(Debug, Clone)]
pub enum Operand {
    None,
    Reg { reg: u8, size: u8 },
    Xmm { reg: u8 },
    Memory { addr: MemoryOperand, size: u8 },
    Immediate { value: i64, size: u8 },
    Relative { offset: i32 },
}

/// ModR/M 字节解析
#[derive(Debug, Clone, Copy)]
pub struct ModRM {
    pub mode: u8, // mod (bits 7-6)
    pub reg: u8,  // reg (bits 5-3)
    pub rm: u8,   // rm (bits 2-0)
}

impl ModRM {
    pub fn from_byte(byte: u8) -> Self {
        Self {
            mode: (byte >> 6) & 0x3,
            reg: (byte >> 3) & 0x7,
            rm: byte & 0x7,
        }
    }

    pub fn with_rex_r(self, rex_r: bool) -> ModRM {
        Self {
            reg: if rex_r { self.reg | 0x8 } else { self.reg },
            ..self
        }
    }

    pub fn with_rex_b(self, rex_b: bool) -> ModRM {
        Self {
            rm: if rex_b { self.rm | 0x8 } else { self.rm },
            ..self
        }
    }
}

/// SIB 字节解析
#[derive(Debug, Clone, Copy)]
pub struct SIB {
    pub scale: u8, // bits 7-6
    pub index: u8, // bits 5-3
    pub base: u8,  // bits 2-0
}

impl SIB {
    pub fn from_byte(byte: u8) -> Self {
        Self {
            scale: (byte >> 6) & 0x3,
            index: (byte >> 3) & 0x7,
            base: byte & 0x7,
        }
    }

    pub fn with_rex_x(self, rex_x: bool) -> SIB {
        Self {
            index: if rex_x { self.index | 0x8 } else { self.index },
            ..self
        }
    }

    pub fn with_rex_b(self, rex_b: bool) -> SIB {
        Self {
            base: if rex_b { self.base | 0x8 } else { self.base },
            ..self
        }
    }
}

/// 操作数解码器
pub struct OperandDecoder<'a> {
    bytes: &'a [u8],
    pos: usize,
    opcode_byte: u8, // 存储操作码字节用于OpReg解码
}

impl<'a> OperandDecoder<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            opcode_byte: bytes.first().copied().unwrap_or(0),
        }
    }

    pub fn new_with_opcode(bytes: &'a [u8], opcode_byte: u8) -> Self {
        Self {
            bytes,
            pos: 0,
            opcode_byte,
        }
    }

    pub fn read_u8(&mut self) -> Result<u8, String> {
        if self.pos >= self.bytes.len() {
            return Err("Unexpected end of instruction".to_string());
        }
        let b = self.bytes[self.pos];
        self.pos += 1;
        Ok(b)
    }

    pub fn read_i8(&mut self) -> Result<i8, String> {
        self.read_u8().map(|b| b as i8)
    }

    pub fn read_i32(&mut self) -> Result<i32, String> {
        if self.pos + 4 > self.bytes.len() {
            return Err("Unexpected end of instruction".to_string());
        }
        let val = i32::from_le_bytes([
            self.bytes[self.pos],
            self.bytes[self.pos + 1],
            self.bytes[self.pos + 2],
            self.bytes[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(val)
    }

    /// 解码单个操作数
    pub fn decode_operand(
        &mut self,
        kind: OperandKind,
        modrm: Option<ModRM>,
        prefix: &PrefixInfo,
        default_size: u8,
    ) -> Result<Operand, String> {
        match kind {
            OperandKind::None => Ok(Operand::None),

            OperandKind::Reg => {
                let modrm = modrm.ok_or("ModR/M required for Reg operand")?;
                Ok(Operand::Reg {
                    reg: modrm.reg,
                    size: default_size,
                })
            }

            OperandKind::Rm => {
                let modrm = modrm.ok_or("ModR/M required for R/M operand")?;
                if modrm.mode == 3 {
                    // 寄存器模式
                    Ok(Operand::Reg {
                        reg: modrm.rm,
                        size: default_size,
                    })
                } else {
                    // 内存寻址模式
                    let addr = self.decode_modrm_memory(modrm, prefix)?;
                    Ok(Operand::Memory {
                        addr,
                        size: default_size,
                    })
                }
            }

            OperandKind::Imm8 => {
                let val = self.read_i8()? as i64;
                Ok(Operand::Immediate {
                    value: val,
                    size: 1,
                })
            }

            OperandKind::Imm32 => {
                let val = self.read_i32()? as i64;
                Ok(Operand::Immediate {
                    value: val,
                    size: 4,
                })
            }

            OperandKind::Rel8 => {
                let offset = self.read_i8()?;
                Ok(Operand::Relative {
                    offset: offset as i32,
                })
            }

            OperandKind::Rel32 => {
                let offset = self.read_i32()?;
                Ok(Operand::Relative { offset })
            }

            OperandKind::XmmReg => {
                let modrm = modrm.ok_or("ModR/M required for XMM reg")?;
                Ok(Operand::Xmm { reg: modrm.reg })
            }

            OperandKind::XmmRm => {
                let modrm = modrm.ok_or("ModR/M required for XMM R/M")?;
                if modrm.mode == 3 {
                    Ok(Operand::Xmm { reg: modrm.rm })
                } else {
                    let addr = self.decode_modrm_memory(modrm, prefix)?;
                    Ok(Operand::Memory {
                        addr,
                        size: 16, // XMM size
                    })
                }
            }

            OperandKind::OpReg => {
                // 操作码低3位表示寄存器号
                let reg_id = self.opcode_byte & 0x07;
                Ok(Operand::Reg {
                    reg: reg_id,
                    size: default_size,
                })
            }

            OperandKind::Moffs => {
                // 直接内存地址操作数，读取64位地址
                let disp64_low = self.read_i32()? as u64;
                let disp64_high = self.read_i32()? as u64;
                let disp64 = disp64_low | (disp64_high << 32);
                Ok(Operand::Memory {
                    addr: MemoryOperand::Direct {
                        base: 0xFF,
                        disp: disp64 as i64,
                    }, // 使用特殊基址表示moffs
                    size: default_size,
                })
            }

            _ => Err(format!("Unsupported operand kind: {:?}", kind)),
        }
    }

    /// 解码 ModR/M 字节的内存寻址模式
    fn decode_modrm_memory(
        &mut self,
        modrm: ModRM,
        _prefix: &PrefixInfo,
    ) -> Result<MemoryOperand, String> {
        match modrm.mode {
            0 => {
                // [base]
                if modrm.rm == 4 {
                    let sib = SIB::from_byte(self.read_u8()?);
                    Ok(MemoryOperand::Indexed {
                        base: if sib.base == 5 { None } else { Some(sib.base) },
                        index: sib.index,
                        scale: 1 << sib.scale,
                        disp: 0,
                    })
                } else if modrm.rm == 5 {
                    let disp = self.read_i32()?;
                    Ok(MemoryOperand::Rip { disp })
                } else {
                    Ok(MemoryOperand::Direct {
                        base: modrm.rm,
                        disp: 0,
                    })
                }
            }
            1 => {
                // [base + disp8]
                let disp = self.read_i8()? as i64;
                if modrm.rm == 4 {
                    let sib = SIB::from_byte(self.read_u8()?);
                    Ok(MemoryOperand::Indexed {
                        base: if sib.base == 5 { None } else { Some(sib.base) },
                        index: sib.index,
                        scale: 1 << sib.scale,
                        disp,
                    })
                } else {
                    Ok(MemoryOperand::Direct {
                        base: modrm.rm,
                        disp,
                    })
                }
            }
            2 => {
                // [base + disp32]
                let disp = self.read_i32()? as i64;
                if modrm.rm == 4 {
                    let sib = SIB::from_byte(self.read_u8()?);
                    Ok(MemoryOperand::Indexed {
                        base: if sib.base == 5 { None } else { Some(sib.base) },
                        index: sib.index,
                        scale: 1 << sib.scale,
                        disp,
                    })
                } else {
                    Ok(MemoryOperand::Direct {
                        base: modrm.rm,
                        disp,
                    })
                }
            }
            3 => {
                // 寄存器模式 (不调用此函数)
                Err("ModR/M mode 3 should not reach memory decoder".to_string())
            }
            _ => unreachable!(),
        }
    }

    pub fn position(&self) -> usize {
        self.pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modrm_reg_reg() {
        let modrm = ModRM::from_byte(0xC3); // mode=11, reg=0, rm=3
        assert_eq!(modrm.mode, 3);
        assert_eq!(modrm.reg, 0);
        assert_eq!(modrm.rm, 3);
    }

    #[test]
    fn test_sib_decode() {
        let sib = SIB::from_byte(0x65); // scale=01, index=100, base=101
        assert_eq!(sib.scale, 1);
        assert_eq!(sib.index, 4);
        assert_eq!(sib.base, 5);
        assert_eq!(sib.scale << sib.scale, 4); // scale factor
    }

    #[test]
    fn test_decode_imm8() {
        let bytes = [0xFF];
        let mut decoder = OperandDecoder::new(&bytes);
        let operand = decoder
            .decode_operand(OperandKind::Imm8, None, &PrefixInfo::default(), 1)
            .expect("Failed to decode Imm8 operand");
        match operand {
            Operand::Immediate { value: -1, size: 1 } => {}
            _ => panic!("Unexpected operand: {:?}", operand),
        }
    }

    #[test]
    fn test_decode_rel32() {
        let bytes = [0x10, 0x00, 0x00, 0x00];
        let mut decoder = OperandDecoder::new(&bytes);
        let operand = decoder
            .decode_operand(OperandKind::Rel32, None, &PrefixInfo::default(), 4)
            .expect("Failed to decode Rel32 operand");
        match operand {
            Operand::Relative { offset: 16 } => {}
            _ => panic!("Unexpected operand: {:?}", operand),
        }
    }
}
