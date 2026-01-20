//! x86-64 架构特定优化
//!
//! 针对复杂指令解码、SSE/AVX 指令处理和前缀解码进行优化。
//!
//! ## 性能目标
//!
//! - 复杂指令解码性能提升 30%+
//! - SSE/AVX 指令解码优化
//! - 前缀解码快速路径
//! - 使用查找表代替多层 if-else

use std::collections::HashMap;

use vm_core::VmError;

/// 优化的前缀信息
#[derive(Debug, Clone, Copy, Default)]
pub struct OptimizedPrefixInfo {
    /// REX 前缀
    pub rex: Option<RexPrefix>,
    /// legacy 前缀
    pub legacy: LegacyPrefixes,
    /// EVEX 前缀 (AVX-512)
    pub evex: Option<EvexPrefix>,
    /// VEX 前缀 (AVX)
    pub vex: Option<VexPrefix>,
    /// XOP 前缀 (AMD)
    pub xop: Option<XopPrefix>,
}

/// REX 前缀（64位模式）
#[derive(Debug, Clone, Copy, Default)]
pub struct RexPrefix {
    pub w: bool, // 64-bit operand size
    pub r: bool, // extension of the ModR/M reg field
    pub x: bool, // extension of the SIB index field
    pub b: bool, // extension of the ModR/M r/m field or SIB base field
}

/// Legacy 前缀
#[derive(Debug, Clone, Copy, Default)]
pub struct LegacyPrefixes {
    pub lock: bool,
    pub repne: bool,
    pub rep: bool,
    pub operand_size: bool, // 0x66
    pub addr_size: bool,    // 0x67
    pub cs_override: bool,
    pub ss_override: bool,
    pub ds_override: bool,
    pub es_override: bool,
    pub fs_override: bool,
    pub gs_override: bool,
    pub op_size_count: u8,
}

/// EVEX 前缀 (AVX-512)
#[derive(Debug, Clone, Copy)]
pub struct EvexPrefix {
    pub mm: u8,   // 0: reserved, 1: 0F, 2: 0F 38, 3: 0F 3A
    pub p: bool,  // compressed legacy prefix
    pub l: u8,    // vector length: 0: 128-bit, 1: 256-bit, 2: 512-bit
    pub ll: u8,   // rounding and suppress exceptions
    pub b: bool,  // broadcast
    pub v2: bool, // extended register enable
    pub aaa: u8,  // rounding control
}

/// VEX 前缀 (AVX)
#[derive(Debug, Clone, Copy)]
pub struct VexPrefix {
    pub m: u8,    // 0: 0F, 1: 0F 38, 2: 0F 3A, 3: reserved
    pub p: u8,    // 0: none, 1: 66, 2: F3, 3: F2
    pub l: u8,    // 0: 128-bit, 1: 256-bit
    pub r: bool,  // R register specifier
    pub vvvv: u8, // VEX register specifier (inverted)
}

/// XOP 前缀 (AMD)
#[derive(Debug, Clone, Copy)]
pub struct XopPrefix {
    pub m: u8,    // 0: reserved, 1: 0F, 2: 0F 38, 3: 0F 3A
    pub p: u8,    // 0: none, 1: 66, 2: F3, 3: F2
    pub l: u8,    // 0: 128-bit, 1: 256-bit
    pub r: bool,  // R register specifier
    pub vvvv: u8, // register specifier (inverted)
}

/// SIMD 指令类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdInstructionType {
    SSE,
    SSE2,
    SSE3,
    SSSE3,
    SSE4_1,
    SSE4_2,
    AVX,
    AVX2,
    AVX512,
}

/// x86-64 优化解码器
pub struct X86Optimizer {
    /// 操作码查找表
    opcode_table: HashMap<u32, OpcodeInfo>,
    /// SIMD 指令查找表
    simd_table: HashMap<u32, SimdInstructionType>,
}

/// 操作码信息（优化版本）
#[derive(Debug, Clone, Copy)]
pub struct OpcodeInfo {
    pub mnemonic: &'static str,
    pub operand_count: u8,
    pub has_modrm: bool,
    pub has_sib: bool,
    pub has_disp: bool,
    pub has_imm: bool,
    pub is_simd: bool,
    pub simd_type: Option<SimdInstructionType>,
}

impl X86Optimizer {
    /// 创建优化的 x86-64 解码器
    pub fn new() -> Self {
        let mut optimizer = Self {
            opcode_table: HashMap::new(),
            simd_table: HashMap::new(),
        };

        // 预填充操作码表
        optimizer.build_opcode_table();
        optimizer.build_simd_table();

        optimizer
    }

    /// 构建操作码查找表
    fn build_opcode_table(&mut self) {
        // 算术指令
        self.insert_opcode(0, 0x01, "ADD", 2, true, false, false, false, false, None);
        self.insert_opcode(0, 0x03, "ADD", 2, true, false, false, false, false, None);
        self.insert_opcode(0, 0x05, "ADD", 2, false, false, false, true, false, None);

        // SIMD: SSE
        self.insert_opcode(
            0x0F,
            0x28,
            "MOVAPS",
            2,
            true,
            false,
            false,
            false,
            true,
            Some(SimdInstructionType::SSE),
        );
        self.insert_opcode(
            0x0F,
            0x58,
            "ADDPS",
            2,
            true,
            false,
            false,
            false,
            true,
            Some(SimdInstructionType::SSE),
        );
        self.insert_opcode(
            0x0F,
            0x5C,
            "SUBPS",
            2,
            true,
            false,
            false,
            false,
            true,
            Some(SimdInstructionType::SSE),
        );
        self.insert_opcode(
            0x0F,
            0x59,
            "MULPS",
            2,
            true,
            false,
            false,
            false,
            true,
            Some(SimdInstructionType::SSE),
        );

        // SIMD: SSE2
        self.insert_opcode(
            0x0F,
            0x66,
            "PCMPISTRI",
            2,
            true,
            false,
            false,
            false,
            true,
            Some(SimdInstructionType::SSE2),
        );
        self.insert_opcode(
            0x0F,
            0x2F,
            "COMISS",
            2,
            true,
            false,
            false,
            false,
            true,
            Some(SimdInstructionType::SSE2),
        );

        // SIMD: AVX (使用 VEX 前缀)
        // AVX 指令通过 VEX 前缀识别，不在这里插入

        // 控制流指令
        self.insert_opcode(0, 0xE8, "CALL", 1, false, false, false, true, false, None);
        self.insert_opcode(0, 0xC3, "RET", 0, false, false, false, false, false, None);
        self.insert_opcode(0, 0xE9, "JMP", 1, false, false, false, true, false, None);
        self.insert_opcode(0, 0xEB, "JMP", 1, false, false, false, true, false, None);

        // 数据移动
        self.insert_opcode(0, 0x8B, "MOV", 2, true, false, false, false, false, None);
        self.insert_opcode(0, 0x89, "MOV", 2, true, false, false, false, false, None);
        self.insert_opcode(0, 0xB8, "MOV", 1, false, false, false, true, false, None); // MOV r64, imm64
    }

    /// 构建 SIMD 指令查找表
    fn build_simd_table(&mut self) {
        // AVX-512 instructions (EVEX prefix)
        let evex_base = 0x62000000u32;

        // AVX-512: VADDPS, VSUBPS, VMULPS
        self.simd_table
            .insert(evex_base | 0x58, SimdInstructionType::AVX512);
        self.simd_table
            .insert(evex_base | 0x5C, SimdInstructionType::AVX512);
        self.simd_table
            .insert(evex_base | 0x59, SimdInstructionType::AVX512);

        // VEX-encoded AVX instructions
        let vex_base = 0xC4000000u32;

        // AVX: VADDPS, VSUBPS, VMULPS (VEX.128.66.0F.WIG 58 / VEX.256.66.0F.WIG 58)
        self.simd_table
            .insert(vex_base | 0x58, SimdInstructionType::AVX);
        self.simd_table
            .insert(vex_base | 0x5C, SimdInstructionType::AVX);
        self.simd_table
            .insert(vex_base | 0x59, SimdInstructionType::AVX);
    }

    /// 插入操作码到查找表
    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn insert_opcode(
        &mut self,
        prefix: u8,
        opcode: u8,
        mnemonic: &'static str,
        operand_count: u8,
        has_modrm: bool,
        has_sib: bool,
        has_disp: bool,
        has_imm: bool,
        is_simd: bool,
        simd_type: Option<SimdInstructionType>,
    ) {
        let key = ((prefix as u32) << 8) | (opcode as u32);
        self.opcode_table.insert(
            key,
            OpcodeInfo {
                mnemonic,
                operand_count,
                has_modrm,
                has_sib,
                has_disp,
                has_imm,
                is_simd,
                simd_type,
            },
        );
    }

    /// 快速操作码查找
    #[inline]
    pub fn lookup_opcode(&self, prefix: u8, opcode: u8) -> Option<&OpcodeInfo> {
        // 先检查双字节操作码
        if prefix != 0 {
            let key = ((prefix as u32) << 8) | (opcode as u32);
            self.opcode_table.get(&key)
        } else {
            // 单字节操作码
            self.opcode_table.get(&(opcode as u32))
        }
    }

    /// 识别 SIMD 指令类型
    #[inline]
    pub fn identify_simd(&self, vex_prefix: u32, opcode: u8) -> Option<SimdInstructionType> {
        let key = vex_prefix | (opcode as u32);
        self.simd_table.get(&key).copied()
    }

    /// 优化的前缀解码（使用快速路径）
    pub fn decode_prefixes_optimized(&self, bytes: &[u8]) -> Result<OptimizedPrefixInfo, VmError> {
        let mut prefixes = OptimizedPrefixInfo::default();
        let mut i = 0;

        // 最多解析 15 个前缀（Intel 限制）
        while i < bytes.len() && i < 15 {
            match bytes[i] {
                // REX prefix (64-bit mode only)
                0x40..=0x4F => {
                    prefixes.rex = Some(RexPrefix {
                        w: (bytes[i] & 0x8) != 0,
                        r: (bytes[i] & 0x4) != 0,
                        x: (bytes[i] & 0x2) != 0,
                        b: (bytes[i] & 0x1) != 0,
                    });
                }
                // Legacy prefixes
                0xF0 => prefixes.legacy.lock = true,
                0xF2 => {
                    prefixes.legacy.repne = true;
                    prefixes.legacy.op_size_count += 1;
                }
                0xF3 => {
                    prefixes.legacy.rep = true;
                    prefixes.legacy.op_size_count += 1;
                }
                0x66 => {
                    prefixes.legacy.operand_size = true;
                    prefixes.legacy.op_size_count += 1;
                }
                0x67 => prefixes.legacy.addr_size = true,
                0x2E => prefixes.legacy.cs_override = true,
                0x36 => prefixes.legacy.ss_override = true,
                0x3E => prefixes.legacy.ds_override = true,
                0x26 => prefixes.legacy.es_override = true,
                0x64 => prefixes.legacy.fs_override = true,
                0x65 => prefixes.legacy.gs_override = true,
                // VEX/XOP/EVEX prefixes (3-byte or 4-byte)
                0xC4 => {
                    // VEX 3-byte
                    if i + 2 < bytes.len() {
                        // 简化版本，完整解析需要更多逻辑
                        return Ok(prefixes);
                    }
                }
                0xC5 => {
                    // VEX 2-byte
                    if i + 1 < bytes.len() {
                        // 简化版本
                        return Ok(prefixes);
                    }
                }
                0x62 => {
                    // EVEX (AVX-512)
                    if i + 3 < bytes.len() {
                        // 简化版本
                        return Ok(prefixes);
                    }
                }
                0x8F => {
                    // XOP (AMD)
                    if i + 2 < bytes.len() {
                        // 简化版本
                        return Ok(prefixes);
                    }
                }
                // 非前缀字节，停止解析
                _ => break,
            }
            i += 1;
        }

        Ok(prefixes)
    }
}

impl Default for X86Optimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimizer_creation() {
        let optimizer = X86Optimizer::new();
        assert!(!optimizer.opcode_table.is_empty());
    }

    #[test]
    fn test_opcode_lookup() {
        let optimizer = X86Optimizer::new();

        // ADD EAX, imm32 (opcode 0x05)
        let info = optimizer.lookup_opcode(0, 0x05);
        assert!(info.is_some());
        assert_eq!(info.unwrap().mnemonic, "ADD");

        // MOVAPS (0F 28)
        let info = optimizer.lookup_opcode(0x0F, 0x28);
        assert!(info.is_some());
        assert_eq!(info.unwrap().mnemonic, "MOVAPS");
        assert!(info.unwrap().is_simd);
    }

    #[test]
    fn test_simd_identification() {
        let optimizer = X86Optimizer::new();

        // AVX-512 VADDPS
        let simd_type = optimizer.identify_simd(0x62000000, 0x58);
        assert_eq!(simd_type, Some(SimdInstructionType::AVX512));

        // AVX VADDPS
        let simd_type = optimizer.identify_simd(0xC4000000, 0x58);
        assert_eq!(simd_type, Some(SimdInstructionType::AVX));
    }

    #[test]
    fn test_prefix_decode() {
        let optimizer = X86Optimizer::new();

        // REX.W prefix (0x48)
        let bytes = vec![0x48, 0x01, 0xC8]; // ADD RAX, RCX
        let prefixes = optimizer.decode_prefixes_optimized(&bytes).unwrap();

        assert!(prefixes.rex.is_some());
        assert!(prefixes.rex.unwrap().w);
    }

    #[test]
    fn test_legacy_prefixes() {
        let optimizer = X86Optimizer::new();

        // LOCK prefix (0xF0)
        let bytes = vec![0xF0, 0x01, 0x08];
        let prefixes = optimizer.decode_prefixes_optimized(&bytes).unwrap();

        assert!(prefixes.legacy.lock);
    }

    #[test]
    fn test_operand_size_override() {
        let optimizer = X86Optimizer::new();

        // 0x66 prefix
        let bytes = vec![0x66, 0x01, 0xC8];
        let prefixes = optimizer.decode_prefixes_optimized(&bytes).unwrap();

        assert!(prefixes.legacy.operand_size);
        assert_eq!(prefixes.legacy.op_size_count, 1);
    }
}
