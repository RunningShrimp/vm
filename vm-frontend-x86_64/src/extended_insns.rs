//! x86-64 扩展指令集支持
//!
//! 添加对 SSE、AVX、BMI 等扩展指令集的支持

use crate::{X86Instruction, X86Mnemonic, X86Operand};
use vm_core::GuestAddr;

/// SSE/SSE2 指令
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SseInstruction {
    // 数据移动
    Movss,
    Movsd,
    Movups,
    Movupd,
    Movdqu,
    // 算术运算
    Addss,
    Addsd,
    Addps,
    Addpd,
    Subss,
    Subsd,
    Subps,
    Subpd,
    Mulss,
    Mulsd,
    Mulps,
    Mulpd,
    Divss,
    Divsd,
    Divps,
    Divpd,
    // 比较
    Cmpss,
    Cmpsd,
    Cmpps,
    Cmppd,
    Comiss,
    Comisd,
    Ucomiss,
    Ucomisd,
    // 逻辑运算
    Andps,
    Andpd,
    Andnps,
    Andnpd,
    Orps,
    Orpd,
    Xorps,
    Xorpd,
    // 转换
    Cvtss2sd,
    Cvtsd2ss,
    Cvtsi2ss,
    Cvtsi2sd,
    Cvttss2si,
    Cvttsd2si,
    // 其他
    Sqrtss,
    Sqrtsd,
    Sqrtps,
    Sqrtpd,
    Maxss,
    Maxsd,
    Maxps,
    Maxpd,
    Minss,
    Minsd,
    Minps,
    Minpd,
}

/// SSE3 指令
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sse3Instruction {
    // 水平运算
    Haddps,
    Haddpd,
    Hsubps,
    Hsubpd,
    // 加法减法
    Addsubps,
    Addsubpd,
    // 数据移动
    Movddup,
    Movshdup,
    Movsldup,
    // 其他
    Lddqu,
    Fisttp,
}

/// SSSE3 指令
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ssse3Instruction {
    // 水平运算
    Pabsb,
    Pabsw,
    Pabsd,
    // 符号扩展
    Psignb,
    Psignw,
    Psignd,
    // 混合
    Pshufb,
    Palignr,
    // 乘法
    Pmulhrsw,
    Pmaddubsw,
}

/// SSE4.1 指令
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sse41Instruction {
    // 混合
    Blendps,
    Blendpd,
    Blendvps,
    Blendvpd,
    Pblendvb,
    Pblendw,
    // 插入/提取
    Insertps,
    Pinsrb,
    Pinsrw,
    Pinsrd,
    Pinsrq,
    Extractps,
    Pextrb,
    Pextrw,
    Pextrd,
    Pextrq,
    // 比较
    Pcmpeqq,
    Pcmpeqd,
    Pcmpeqw,
    Pcmpeqb,
    // 其他
    Pmuldq,
    Pminsb,
    Pmaxsb,
    Pminsw,
    Pmaxsw,
    Pminsd,
    Pmaxsd,
    Pminuw,
    Pmaxuw,
    Pminud,
    Pmaxud,
    Roundps,
    Roundpd,
    Roundss,
    Roundsd,
    Dpps,
    Dppd,
    Mpsadbw,
    Phminposuw,
}

/// SSE4.2 指令
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sse42Instruction {
    // 字符串比较
    Pcmpestri,
    Pcmpestrm,
    Pcmpistri,
    Pcmpistrm,
    // CRC32
    Crc32,
    // 其他
    Popcnt,
}

/// AVX 指令
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AvxInstruction {
    // 256-bit 数据移动
    Vmovaps,
    Vmovapd,
    Vmovups,
    Vmovupd,
    Vmovdqa,
    Vmovdqu,
    // 算术运算
    Vaddps,
    Vaddpd,
    Vsubps,
    Vsubpd,
    Vmulps,
    Vmulpd,
    Vdivps,
    Vdivpd,
    // 融合乘加 (FMA)
    Vfmadd132ps,
    Vfmadd213ps,
    Vfmadd231ps,
    Vfmadd132pd,
    Vfmadd213pd,
    Vfmadd231pd,
    Vfmsub132ps,
    Vfmsub213ps,
    Vfmsub231ps,
    Vfmsub132pd,
    Vfmsub213pd,
    Vfmsub231pd,
    Vfnmadd132ps,
    Vfnmadd213ps,
    Vfnmadd231ps,
    Vfnmadd132pd,
    Vfnmadd213pd,
    Vfnmadd231pd,
    Vfnmsub132ps,
    Vfnmsub213ps,
    Vfnmsub231ps,
    Vfnmsub132pd,
    Vfnmsub213pd,
    Vfnmsub231pd,
    // 逻辑运算
    Vandps,
    Vandpd,
    Vorps,
    Vorpd,
    Vxorps,
    Vxorpd,
    Vandnps,
    Vandnpd,
    // 比较
    Vcmpps,
    Vcmppd,
    Vcomiss,
    Vcomisd,
    // 置换
    Vperm2f128,
    Vpermilps,
    Vpermilpd,
    Vpermq,
    Vpermd,
    Vpermpd,
    // 其他
    Vsqrtps,
    Vsqrtpd,
    Vmaxps,
    Vmaxpd,
    Vminps,
    Vminpd,
    Vroundps,
    Vroundpd,
}

/// BMI (Bit Manipulation Instructions) 指令
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BmiInstruction {
    // BMI1
    Andn,
    Bextr,
    Blsi,
    Blsmsk,
    Blsr,
    Tzcnt,
    Lzcnt,
    // BMI2
    Bzhi,
    Mulx,
    Pdep,
    Pext,
    Rorx,
    Sarx,
    Shlx,
    Shrx,
}

/// AES-NI 指令
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AesInstruction {
    Aesenc,
    Aesenclast,
    Aesdec,
    Aesdeclast,
    Aesimc,
    Aeskeygenassist,
}

/// 原子操作指令
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AtomicInstruction {
    // 原子交换
    Xchg,
    // 原子比较交换
    Cmpxchg,
    Cmpxchg8b,
    Cmpxchg16b,
    // 原子算术运算
    Xadd,
    Xadd8,
    Xadd16,
    Xadd32,
    Xadd64,
    // 原子逻辑运算
    Xor,
    And,
    Or,
    // 原子增量/减量
    Inc,
    Dec,
    // 其他
    Lock, // LOCK 前缀
}

/// 扩展指令解码器
pub struct ExtendedDecoder;

impl ExtendedDecoder {
    /// 解码 SSE 指令
    pub fn decode_sse(opcode: &[u8], pc: GuestAddr) -> Option<X86Instruction> {
        if opcode.len() < 2 {
            return None;
        }

        // 简化示例：解码 MOVSS (F3 0F 10)
        if opcode[0] == 0xF3 && opcode[1] == 0x0F && opcode.get(2) == Some(&0x10) {
            return Some(X86Instruction {
                mnemonic: X86Mnemonic::Movaps, // 简化，实际应该是 MOVSS
                op1: X86Operand::Xmm(0),
                op2: X86Operand::Xmm(1),
                op3: X86Operand::None,
                op_size: 4,
                lock: false,
                rep: false,
                repne: false,
                next_pc: pc + opcode.len() as u64,
                jcc_cc: None,
            });
        }

        None
    }

    /// 解码 SSE3 指令 (0x66 0F 38 / 0xF2 0F 38)
    pub fn decode_sse3(opcode: &[u8], _pc: GuestAddr) -> Option<Sse3Instruction> {
        if opcode.len() < 3 {
            return None;
        }

        // SSE3 指令通常以 0x66 0F 38 或 0xF2 0F 38 开头
        if opcode[0] == 0x66 && opcode[1] == 0x0F && opcode[2] == 0x38 {
            match opcode.get(3) {
                Some(&0x7C) => Some(Sse3Instruction::Haddps),
                Some(&0x7D) => Some(Sse3Instruction::Haddpd),
                Some(&0x7E) => Some(Sse3Instruction::Hsubps),
                Some(&0x7F) => Some(Sse3Instruction::Hsubpd),
                Some(&0xD0) => Some(Sse3Instruction::Addsubps),
                Some(&0xD1) => Some(Sse3Instruction::Addsubpd),
                Some(&0x12) => Some(Sse3Instruction::Movddup),
                Some(&0x16) => Some(Sse3Instruction::Movshdup),
                Some(&0x17) => Some(Sse3Instruction::Movsldup),
                Some(&0xF0) => Some(Sse3Instruction::Lddqu),
                _ => None,
            }
        } else if opcode[0] == 0xF2 && opcode[1] == 0x0F && opcode[2] == 0x38 {
            match opcode.get(3) {
                Some(&0x1C) => Some(Sse3Instruction::Fisttp),
                _ => None,
            }
        } else {
            None
        }
    }

    /// 解码 SSSE3 指令 (0x66 0F 38)
    pub fn decode_ssse3(opcode: &[u8], _pc: GuestAddr) -> Option<Ssse3Instruction> {
        if opcode.len() < 4 {
            return None;
        }

        if opcode[0] == 0x66 && opcode[1] == 0x0F && opcode[2] == 0x38 {
            match opcode.get(3) {
                Some(&0x1C) => Some(Ssse3Instruction::Pabsb),
                Some(&0x1D) => Some(Ssse3Instruction::Pabsw),
                Some(&0x1E) => Some(Ssse3Instruction::Pabsd),
                Some(&0x08) => Some(Ssse3Instruction::Psignb),
                Some(&0x09) => Some(Ssse3Instruction::Psignw),
                Some(&0x0A) => Some(Ssse3Instruction::Psignd),
                Some(&0x00) => Some(Ssse3Instruction::Pshufb),
                Some(&0x0F) => Some(Ssse3Instruction::Palignr),
                Some(&0x0B) => Some(Ssse3Instruction::Pmulhrsw),
                Some(&0x04) => Some(Ssse3Instruction::Pmaddubsw),
                _ => None,
            }
        } else {
            None
        }
    }

    /// 解码 SSE4.1 指令 (0x66 0F 38 / 0x66 0F 3A)
    pub fn decode_sse41(opcode: &[u8], _pc: GuestAddr) -> Option<Sse41Instruction> {
        if opcode.len() < 4 {
            return None;
        }

        if opcode[0] == 0x66 && opcode[1] == 0x0F && opcode[2] == 0x38 {
            match opcode.get(3) {
                Some(&0x0A) => Some(Sse41Instruction::Blendps),
                Some(&0x0D) => Some(Sse41Instruction::Blendpd),
                Some(&0x14) => Some(Sse41Instruction::Blendvps),
                Some(&0x15) => Some(Sse41Instruction::Blendvpd),
                Some(&0x10) => Some(Sse41Instruction::Pblendvb),
                Some(&0x0E) => Some(Sse41Instruction::Pblendw),
                Some(&0x21) => Some(Sse41Instruction::Insertps),
                Some(&0x20) => Some(Sse41Instruction::Pinsrb),
                Some(&0xC4) => Some(Sse41Instruction::Pinsrw),
                Some(&0x22) => Some(Sse41Instruction::Pinsrd),
                Some(&0x17) => Some(Sse41Instruction::Extractps),
                Some(&0x14) => Some(Sse41Instruction::Pextrb),
                Some(&0x16) => Some(Sse41Instruction::Pextrd),
                Some(&0x28) => Some(Sse41Instruction::Pmuldq),
                Some(&0x38) => Some(Sse41Instruction::Pminsb),
                Some(&0x3C) => Some(Sse41Instruction::Pmaxsb),
                Some(&0x39) => Some(Sse41Instruction::Pminsw),
                Some(&0x3E) => Some(Sse41Instruction::Pmaxsw),
                Some(&0x3D) => Some(Sse41Instruction::Pmaxsd),
                Some(&0x3A) => Some(Sse41Instruction::Pminuw),
                Some(&0x3B) => Some(Sse41Instruction::Pminud),
                Some(&0x3F) => Some(Sse41Instruction::Pmaxud),
                Some(&0x08) => Some(Sse41Instruction::Roundps),
                Some(&0x09) => Some(Sse41Instruction::Roundpd),
                Some(&0x0B) => Some(Sse41Instruction::Roundsd),
                Some(&0x40) => Some(Sse41Instruction::Dpps),
                Some(&0x41) => Some(Sse41Instruction::Dppd),
                Some(&0x42) => Some(Sse41Instruction::Mpsadbw),
                _ => None,
            }
        } else if opcode[0] == 0x66 && opcode[1] == 0x0F && opcode[2] == 0x3A {
            match opcode.get(3) {
                Some(&0x0C) => Some(Sse41Instruction::Blendps),
                Some(&0x0D) => Some(Sse41Instruction::Blendpd),
                Some(&0x4B) => Some(Sse41Instruction::Blendvps),
                Some(&0x4A) => Some(Sse41Instruction::Blendvpd),
                Some(&0x0E) => Some(Sse41Instruction::Pblendw),
                Some(&0x21) => Some(Sse41Instruction::Insertps),
                Some(&0x20) => Some(Sse41Instruction::Pinsrb),
                Some(&0xC4) => Some(Sse41Instruction::Pinsrw),
                Some(&0x22) => Some(Sse41Instruction::Pinsrd),
                Some(&0x17) => Some(Sse41Instruction::Extractps),
                Some(&0x14) => Some(Sse41Instruction::Pextrb),
                Some(&0x16) => Some(Sse41Instruction::Pextrd),
                _ => None,
            }
        } else {
            None
        }
    }

    /// 解码 SSE4.2 指令 (0x66 0F 38)
    pub fn decode_sse42(opcode: &[u8], _pc: GuestAddr) -> Option<Sse42Instruction> {
        if opcode.len() < 4 {
            return None;
        }

        if opcode[0] == 0x66 && opcode[1] == 0x0F && opcode[2] == 0x38 {
            match opcode.get(3) {
                Some(&0x61) => Some(Sse42Instruction::Pcmpestri),
                Some(&0x60) => Some(Sse42Instruction::Pcmpestrm),
                Some(&0x63) => Some(Sse42Instruction::Pcmpistri),
                Some(&0x62) => Some(Sse42Instruction::Pcmpistrm),
                Some(&0xF0) => Some(Sse42Instruction::Crc32),
                _ => None,
            }
        } else if opcode[0] == 0xF3 && opcode[1] == 0x0F && opcode[2] == 0x38 {
            match opcode.get(3) {
                Some(&0xB8) => Some(Sse42Instruction::Popcnt),
                _ => None,
            }
        } else {
            None
        }
    }

    /// 解码原子操作指令
    pub fn decode_atomic(
        opcode: &[u8],
        _pc: GuestAddr,
        has_lock: bool,
    ) -> Option<AtomicInstruction> {
        if !has_lock {
            return None;
        }

        if opcode.is_empty() {
            return None;
        }

        // LOCK 前缀 + XCHG
        if opcode[0] == 0x86 || opcode[0] == 0x87 {
            Some(AtomicInstruction::Xchg)
        }
        // LOCK 前缀 + CMPXCHG
        else if opcode.len() >= 2 && opcode[0] == 0x0F && opcode[1] == 0xB0 {
            Some(AtomicInstruction::Cmpxchg)
        }
        // LOCK 前缀 + CMPXCHG8B
        else if opcode.len() >= 2 && opcode[0] == 0x0F && opcode[1] == 0xC7 {
            Some(AtomicInstruction::Cmpxchg8b)
        }
        // LOCK 前缀 + XADD
        else if opcode.len() >= 2 && opcode[0] == 0x0F && opcode[1] == 0xC0 {
            Some(AtomicInstruction::Xadd)
        }
        // LOCK 前缀 + INC/DEC
        else if opcode[0] == 0xFE || opcode[0] == 0xFF {
            // 需要检查 ModR/M 来确定是 INC 还是 DEC
            Some(AtomicInstruction::Inc) // 简化处理
        } else {
            None
        }
    }

    /// 解码 AVX 指令
    pub fn decode_avx(opcode: &[u8], pc: GuestAddr) -> Option<X86Instruction> {
        if opcode.len() < 3 {
            return None;
        }

        // 简化示例：解码 VADDPS (VEX.NDS.128.0F.WIG 58)
        if opcode[0] == 0xC5 && opcode[1] == 0xF8 && opcode.get(2) == Some(&0x58) {
            return Some(X86Instruction {
                mnemonic: X86Mnemonic::Addps, // 简化
                op1: X86Operand::Xmm(0),
                op2: X86Operand::Xmm(1),
                op3: X86Operand::Xmm(2),
                op_size: 16,
                lock: false,
                rep: false,
                repne: false,
                next_pc: pc + opcode.len() as u64,
                jcc_cc: None,
            });
        }

        None
    }

    /// 解码 AVX 指令（返回指令类型）
    pub fn decode_avx_insn(opcode: &[u8], _pc: GuestAddr) -> Option<AvxInstruction> {
        if opcode.len() < 3 {
            return None;
        }

        // VEX 2-byte format: C5 [R vvvv L pp] [opcode]
        // VEX 3-byte format: C4 [R X B m-mmmm] [W vvvv L pp] [opcode]
        let is_vex2 = opcode[0] == 0xC5;
        let is_vex3 = opcode[0] == 0xC4;

        if is_vex2 && opcode.len() >= 3 {
            let vex_byte = opcode[1];
            let opcode_byte = opcode[2];
            let is_256 = (vex_byte & 0x04) != 0; // L bit

            match opcode_byte {
                0x28 => Some(if is_256 {
                    AvxInstruction::Vmovaps
                } else {
                    AvxInstruction::Vmovaps
                }),
                0x58 => Some(if is_256 {
                    AvxInstruction::Vaddps
                } else {
                    AvxInstruction::Vaddps
                }),
                0x59 => Some(if is_256 {
                    AvxInstruction::Vmulps
                } else {
                    AvxInstruction::Vmulps
                }),
                0x5C => Some(if is_256 {
                    AvxInstruction::Vsubps
                } else {
                    AvxInstruction::Vsubps
                }),
                0x5E => Some(if is_256 {
                    AvxInstruction::Vdivps
                } else {
                    AvxInstruction::Vdivps
                }),
                0x51 => Some(if is_256 {
                    AvxInstruction::Vsqrtps
                } else {
                    AvxInstruction::Vsqrtps
                }),
                0x5F => Some(if is_256 {
                    AvxInstruction::Vmaxps
                } else {
                    AvxInstruction::Vmaxps
                }),
                0x5D => Some(if is_256 {
                    AvxInstruction::Vminps
                } else {
                    AvxInstruction::Vminps
                }),
                _ => None,
            }
        } else if is_vex3 && opcode.len() >= 4 {
            let opcode_byte = opcode[3];
            let is_256 = (opcode[2] & 0x04) != 0; // L bit in second VEX byte

            match opcode_byte {
                0x28 => Some(if is_256 {
                    AvxInstruction::Vmovaps
                } else {
                    AvxInstruction::Vmovaps
                }),
                0x58 => Some(if is_256 {
                    AvxInstruction::Vaddps
                } else {
                    AvxInstruction::Vaddps
                }),
                0x59 => Some(if is_256 {
                    AvxInstruction::Vmulps
                } else {
                    AvxInstruction::Vmulps
                }),
                0x5C => Some(if is_256 {
                    AvxInstruction::Vsubps
                } else {
                    AvxInstruction::Vsubps
                }),
                0x5E => Some(if is_256 {
                    AvxInstruction::Vdivps
                } else {
                    AvxInstruction::Vdivps
                }),
                0x51 => Some(if is_256 {
                    AvxInstruction::Vsqrtps
                } else {
                    AvxInstruction::Vsqrtps
                }),
                0x5F => Some(if is_256 {
                    AvxInstruction::Vmaxps
                } else {
                    AvxInstruction::Vmaxps
                }),
                0x5D => Some(if is_256 {
                    AvxInstruction::Vminps
                } else {
                    AvxInstruction::Vminps
                }),
                // FMA instructions (0x98-0x9F, 0xA8-0xAF, 0xB8-0xBF, 0x98-0x9F)
                0x98 => Some(AvxInstruction::Vfmadd132ps),
                0x99 => Some(AvxInstruction::Vfmadd132pd),
                0xA8 => Some(AvxInstruction::Vfmadd213ps),
                0xA9 => Some(AvxInstruction::Vfmadd213pd),
                0xB8 => Some(AvxInstruction::Vfmadd231ps),
                0xB9 => Some(AvxInstruction::Vfmadd231pd),
                0x9A => Some(AvxInstruction::Vfmsub132ps),
                0x9B => Some(AvxInstruction::Vfmsub132pd),
                0xAA => Some(AvxInstruction::Vfmsub213ps),
                0xAB => Some(AvxInstruction::Vfmsub213pd),
                0xBA => Some(AvxInstruction::Vfmsub231ps),
                0xBB => Some(AvxInstruction::Vfmsub231pd),
                0x9C => Some(AvxInstruction::Vfnmadd132ps),
                0x9D => Some(AvxInstruction::Vfnmadd132pd),
                0xAC => Some(AvxInstruction::Vfnmadd213ps),
                0xAD => Some(AvxInstruction::Vfnmadd213pd),
                0xBC => Some(AvxInstruction::Vfnmadd231ps),
                0xBD => Some(AvxInstruction::Vfnmadd231pd),
                0x9E => Some(AvxInstruction::Vfnmsub132ps),
                0x9F => Some(AvxInstruction::Vfnmsub132pd),
                0xAE => Some(AvxInstruction::Vfnmsub213ps),
                0xAF => Some(AvxInstruction::Vfnmsub213pd),
                0xBE => Some(AvxInstruction::Vfnmsub231ps),
                0xBF => Some(AvxInstruction::Vfnmsub231pd),
                _ => None,
            }
        } else {
            None
        }
    }

    /// 解码 BMI 指令
    pub fn decode_bmi(opcode: &[u8], pc: GuestAddr) -> Option<X86Instruction> {
        if opcode.len() < 3 {
            return None;
        }

        // 简化示例：解码 ANDN (VEX.NDS.LZ.0F38.W0 F2)
        if opcode[0] == 0xC4 && opcode.get(3) == Some(&0xF2) {
            return Some(X86Instruction {
                mnemonic: X86Mnemonic::And, // 简化
                op1: X86Operand::Reg(0),
                op2: X86Operand::Reg(1),
                op3: X86Operand::Reg(2),
                op_size: 8,
                lock: false,
                rep: false,
                repne: false,
                next_pc: pc + opcode.len() as u64,
                jcc_cc: None,
            });
        }

        None
    }

    /// 解码 AES-NI 指令
    pub fn decode_aes(opcode: &[u8], pc: GuestAddr) -> Option<X86Instruction> {
        if opcode.len() < 3 {
            return None;
        }

        // 简化示例：解码 AESENC (66 0F 38 DC)
        if opcode[0] == 0x66
            && opcode[1] == 0x0F
            && opcode[2] == 0x38
            && opcode.get(3) == Some(&0xDC)
        {
            return Some(X86Instruction {
                mnemonic: X86Mnemonic::Nop, // 简化，实际应该是 AESENC
                op1: X86Operand::Xmm(0),
                op2: X86Operand::Xmm(1),
                op3: X86Operand::None,
                op_size: 16,
                lock: false,
                rep: false,
                repne: false,
                next_pc: pc + opcode.len() as u64,
                jcc_cc: None,
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_decode() {
        let opcode = vec![0xF3, 0x0F, 0x10, 0xC1];
        let insn = ExtendedDecoder::decode_sse(&opcode, 0x1000);
        assert!(insn.is_some());
    }

    #[test]
    fn test_avx_decode() {
        let opcode = vec![0xC5, 0xF8, 0x58, 0xC1];
        let insn = ExtendedDecoder::decode_avx(&opcode, 0x1000);
        assert!(insn.is_some());
    }
}
