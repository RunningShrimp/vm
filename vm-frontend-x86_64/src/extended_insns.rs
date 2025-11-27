//! x86-64 扩展指令集支持
//!
//! 添加对 SSE、AVX、BMI 等扩展指令集的支持

use crate::{X86Mnemonic, X86Operand, X86Instruction};
use vm_core::GuestAddr;

/// SSE/SSE2 指令
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SseInstruction {
    // 数据移动
    Movss, Movsd, Movups, Movupd, Movdqu,
    // 算术运算
    Addss, Addsd, Addps, Addpd,
    Subss, Subsd, Subps, Subpd,
    Mulss, Mulsd, Mulps, Mulpd,
    Divss, Divsd, Divps, Divpd,
    // 比较
    Cmpss, Cmpsd, Cmpps, Cmppd,
    Comiss, Comisd, Ucomiss, Ucomisd,
    // 逻辑运算
    Andps, Andpd, Andnps, Andnpd,
    Orps, Orpd, Xorps, Xorpd,
    // 转换
    Cvtss2sd, Cvtsd2ss, Cvtsi2ss, Cvtsi2sd,
    Cvttss2si, Cvttsd2si,
    // 其他
    Sqrtss, Sqrtsd, Sqrtps, Sqrtpd,
    Maxss, Maxsd, Maxps, Maxpd,
    Minss, Minsd, Minps, Minpd,
}

/// AVX 指令
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AvxInstruction {
    // 256-bit 数据移动
    Vmovaps, Vmovapd, Vmovups, Vmovupd,
    Vmovdqa, Vmovdqu,
    // 算术运算
    Vaddps, Vaddpd, Vsubps, Vsubpd,
    Vmulps, Vmulpd, Vdivps, Vdivpd,
    // 融合乘加 (FMA)
    Vfmadd132ps, Vfmadd213ps, Vfmadd231ps,
    Vfmadd132pd, Vfmadd213pd, Vfmadd231pd,
    // 逻辑运算
    Vandps, Vandpd, Vorps, Vorpd,
    Vxorps, Vxorpd,
    // 比较
    Vcmpps, Vcmppd,
    // 置换
    Vperm2f128, Vpermilps, Vpermilpd,
}

/// BMI (Bit Manipulation Instructions) 指令
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BmiInstruction {
    // BMI1
    Andn, Bextr, Blsi, Blsmsk, Blsr,
    Tzcnt, Lzcnt,
    // BMI2
    Bzhi, Mulx, Pdep, Pext,
    Rorx, Sarx, Shlx, Shrx,
}

/// AES-NI 指令
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AesInstruction {
    Aesenc, Aesenclast,
    Aesdec, Aesdeclast,
    Aesimc, Aeskeygenassist,
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
        if opcode[0] == 0x66 && opcode[1] == 0x0F && opcode[2] == 0x38 && opcode.get(3) == Some(&0xDC) {
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
