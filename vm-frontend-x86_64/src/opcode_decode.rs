//! x86-64 操作码解码阶段
//! 识别指令操作码并返回指令的操作数模式

use super::prefix_decode::PrefixInfo;

/// 操作数类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandKind {
    None,
    Reg, // ModR/M.reg
    Rm,  // ModR/M.rm
    Imm8,
    Imm32,
    Imm64,
    Rel8,
    Rel32,
    OpReg,  // Lowest 3 bits of opcode
    XmmReg, // ModR/M.reg is XMM
    XmmRm,  // ModR/M.rm is XMM or memory
    Moffs,  // 直接内存地址操作数
}

/// 指令操作码的解码结果
#[derive(Debug, Clone)]
pub struct OpcodeInfo {
    pub mnemonic: &'static str,
    pub is_two_byte: bool,
    pub opcode_byte: u8,
    pub op1_kind: OperandKind,
    pub op2_kind: OperandKind,
    pub op3_kind: OperandKind,
    pub requires_modrm: bool,
}

/// 单字节操作码解码表
fn decode_single_byte_opcode(opcode: u8, _prefix: &PrefixInfo) -> Option<OpcodeInfo> {
    Some(match opcode {
        // 二进制算术运算
        0x00 => OpcodeInfo {
            mnemonic: "add",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rm,
            op2_kind: OperandKind::Reg,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },
        0x01 => OpcodeInfo {
            mnemonic: "add",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Rm,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },
        0x02 => OpcodeInfo {
            mnemonic: "add",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Rm,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },
        0x03 => OpcodeInfo {
            mnemonic: "add",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Rm,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },
        0x04 => OpcodeInfo {
            mnemonic: "add",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Imm8,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // ADD r/m8, imm8
        0x05 => OpcodeInfo {
            mnemonic: "add",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Imm32,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // ADD rax, imm32

        // 减法运算
        0x28 => OpcodeInfo {
            mnemonic: "sub",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rm,
            op2_kind: OperandKind::Reg,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },
        0x29 => OpcodeInfo {
            mnemonic: "sub",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Rm,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },
        0x2A => OpcodeInfo {
            mnemonic: "sub",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Rm,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },
        0x2B => OpcodeInfo {
            mnemonic: "sub",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Rm,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },

        // 移动指令
        0x88 => OpcodeInfo {
            mnemonic: "mov",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rm,
            op2_kind: OperandKind::Reg,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        }, // MOV r/m8, r8
        0x89 => OpcodeInfo {
            mnemonic: "mov",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rm,
            op2_kind: OperandKind::Reg,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        }, // MOV r/m64, r64
        0x8A => OpcodeInfo {
            mnemonic: "mov",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Rm,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        }, // MOV r8, r/m8
        0x8B => OpcodeInfo {
            mnemonic: "mov",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Rm,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        }, // MOV r64, r/m64
        0xA0 => OpcodeInfo {
            mnemonic: "mov",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Moffs,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // MOV AL, moffs8
        0xA1 => OpcodeInfo {
            mnemonic: "mov",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Moffs,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // MOV RAX, moffs64

        // 立即数移动
        0xB8..=0xBF => OpcodeInfo {
            mnemonic: "mov",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::OpReg,
            op2_kind: OperandKind::Imm32,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // MOV r64, imm32 (sign-extended)

        // 比较指令
        0x38 => OpcodeInfo {
            mnemonic: "cmp",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rm,
            op2_kind: OperandKind::Reg,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },
        0x39 => OpcodeInfo {
            mnemonic: "cmp",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Rm,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },
        0x3A => OpcodeInfo {
            mnemonic: "cmp",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Rm,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },
        0x3B => OpcodeInfo {
            mnemonic: "cmp",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Rm,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },

        // 逻辑运算
        0x20 => OpcodeInfo {
            mnemonic: "and",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rm,
            op2_kind: OperandKind::Reg,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },
        0x21 => OpcodeInfo {
            mnemonic: "and",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Rm,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },
        0x30 => OpcodeInfo {
            mnemonic: "xor",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rm,
            op2_kind: OperandKind::Reg,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },
        0x31 => OpcodeInfo {
            mnemonic: "xor",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::Rm,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },

        // NOP系列
        0x90 => OpcodeInfo {
            mnemonic: "nop",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::None,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        },

        // 无条件跳转
        0xE9 => OpcodeInfo {
            mnemonic: "jmp",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rel32,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // JMP rel32
        0xEB => OpcodeInfo {
            mnemonic: "jmp",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rel8,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // JMP rel8

        // 条件跳转
        0x70 => OpcodeInfo {
            mnemonic: "jo",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rel8,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        },
        0x71 => OpcodeInfo {
            mnemonic: "jno",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rel8,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        },
        0x72 => OpcodeInfo {
            mnemonic: "jb",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rel8,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        },
        0x73 => OpcodeInfo {
            mnemonic: "jnb",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rel8,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        },
        0x74 => OpcodeInfo {
            mnemonic: "jz",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rel8,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // JZ/JE
        0x75 => OpcodeInfo {
            mnemonic: "jnz",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rel8,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // JNZ/JNE

        // 调用和返回
        0xC3 => OpcodeInfo {
            mnemonic: "ret",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::None,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // RET
        0xE8 => OpcodeInfo {
            mnemonic: "call",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rel32,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // CALL rel32

        // 系统指令
        0xF4 => OpcodeInfo {
            mnemonic: "hlt",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::None,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // HLT
        0xCC => OpcodeInfo {
            mnemonic: "int3",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::None,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // INT3
        0xCD => OpcodeInfo {
            mnemonic: "int",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Imm8,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // INT imm8

        // 栈操作
        0x50..=0x57 => OpcodeInfo {
            mnemonic: "push",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // PUSH reg
        0x58..=0x5F => OpcodeInfo {
            mnemonic: "pop",
            is_two_byte: false,
            opcode_byte: opcode,
            op1_kind: OperandKind::Reg,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        }, // POP reg

        _ => return None,
    })
}

/// 两字节操作码解码表 (0x0F ...)
fn decode_two_byte_opcode(opcode: u8, _prefix: &PrefixInfo) -> Option<OpcodeInfo> {
    Some(match opcode {
        // 条件跳转
        0x80..=0x8F => OpcodeInfo {
            mnemonic: "jcc",
            is_two_byte: true,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rel32,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        },

        // 条件设置
        0x90..=0x9F => OpcodeInfo {
            mnemonic: "scc",
            is_two_byte: true,
            opcode_byte: opcode,
            op1_kind: OperandKind::Rm,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },

        // CPUID
        0xA2 => OpcodeInfo {
            mnemonic: "cpuid",
            is_two_byte: true,
            opcode_byte: opcode,
            op1_kind: OperandKind::None,
            op2_kind: OperandKind::None,
            op3_kind: OperandKind::None,
            requires_modrm: false,
        },

        // SSE 指令
        0x28 => OpcodeInfo {
            mnemonic: "movaps",
            is_two_byte: true,
            opcode_byte: opcode,
            op1_kind: OperandKind::XmmReg,
            op2_kind: OperandKind::XmmRm,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },
        0x58 => OpcodeInfo {
            mnemonic: "addps",
            is_two_byte: true,
            opcode_byte: opcode,
            op1_kind: OperandKind::XmmReg,
            op2_kind: OperandKind::XmmRm,
            op3_kind: OperandKind::None,
            requires_modrm: true,
        },

        _ => return None,
    })
}

/// 解码操作码和获取指令信息
///
/// # 参数
/// - `opcode`: 第一个操作码字节
/// - `prefix`: 前缀信息
/// - `needs_second_byte`: 是否需要读取第二个操作码字节 (0x0F)
pub fn decode_opcode(
    opcode: u8,
    prefix: &PrefixInfo,
    needs_second_byte: bool,
) -> Result<Option<OpcodeInfo>, String> {
    if needs_second_byte {
        Ok(decode_two_byte_opcode(opcode, prefix))
    } else {
        Ok(decode_single_byte_opcode(opcode, prefix))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_nop() {
        let prefix = PrefixInfo::default();
        let info = decode_opcode(0x90, &prefix, false)
            .expect("Failed to decode NOP")
            .expect("NOP opcode should be valid");
        assert_eq!(info.mnemonic, "nop");
        assert!(!info.requires_modrm);
    }

    #[test]
    fn test_decode_mov_rm_r() {
        let prefix = PrefixInfo::default();
        let info = decode_opcode(0x89, &prefix, false)
            .expect("Failed to decode MOV")
            .expect("MOV opcode should be valid");
        assert_eq!(info.mnemonic, "mov");
        assert_eq!(info.op1_kind, OperandKind::Rm);
        assert_eq!(info.op2_kind, OperandKind::Reg);
        assert!(info.requires_modrm);
    }

    #[test]
    fn test_decode_jmp_rel32() {
        let prefix = PrefixInfo::default();
        let info = decode_opcode(0xE9, &prefix, false)
            .expect("Failed to decode JMP")
            .expect("JMP opcode should be valid");
        assert_eq!(info.mnemonic, "jmp");
        assert_eq!(info.op1_kind, OperandKind::Rel32);
    }

    #[test]
    fn test_decode_invalid_opcode() {
        let prefix = PrefixInfo::default();
        let result = decode_opcode(0xFF, &prefix, false).expect("Failed to decode opcode");
        assert!(result.is_none());
    }
}
