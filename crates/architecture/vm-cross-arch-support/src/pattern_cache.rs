//! 模式匹配缓存
//!
//! 缓存指令模式以加速跨架构翻译中的模式识别和分析。

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// ============================================================================
// 架构类型
// ============================================================================

/// 支持的架构类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Arch {
    #[default]
    Unknown,
    X86_64,
    Riscv64,
    AArch64,
    Arm,
}

// ============================================================================
// 模式特征
// ============================================================================

/// 指令模式特征
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternFeatures {
    /// 是否有加载操作
    pub has_load: bool,
    /// 是否有存储操作
    pub has_store: bool,
    /// 是否有分支操作
    pub has_branch: bool,
    /// 是否有算术操作
    pub has_arithmetic: bool,
    /// 是否有逻辑操作
    pub has_logic: bool,
    /// 是否有向量操作
    pub has_vector: bool,
    /// 是否有浮点操作
    pub has_float: bool,
    /// 操作数个数
    pub operand_count: u8,
    /// 指令长度（字节）
    pub instruction_length: u8,
    /// 是否是压缩指令（RISC-V C扩展）
    pub is_compressed: bool,
}

impl PatternFeatures {
    /// 计算特征哈希
    pub fn hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.has_load.hash(&mut hasher);
        self.has_store.hash(&mut hasher);
        self.has_branch.hash(&mut hasher);
        self.has_arithmetic.hash(&mut hasher);
        self.has_logic.hash(&mut hasher);
        self.has_vector.hash(&mut hasher);
        self.has_float.hash(&mut hasher);
        self.operand_count.hash(&mut hasher);
        self.instruction_length.hash(&mut hasher);
        self.is_compressed.hash(&mut hasher);
        hasher.finish()
    }
}

// ============================================================================
// 指令模式
// ============================================================================

/// 指令模式
#[derive(Debug, Clone, PartialEq)]
pub struct InstructionPattern {
    /// 模式名称
    pub name: String,
    /// 架构类型
    pub arch: Arch,
    /// 模式特征
    pub features: PatternFeatures,
    /// 操作数类型
    pub operand_types: Vec<OperandType>,
    /// 是否是内存操作
    pub is_memory: bool,
    /// 是否是控制流
    pub is_control_flow: bool,
}

/// 操作数类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperandType {
    Register,
    Immediate,
    Memory,
    Float,
    Vector,
    Unknown,
}

// ============================================================================
// 模式匹配缓存
// ============================================================================

/// 模式匹配缓存
pub struct PatternMatchCache {
    /// (arch, instruction_bytes_hash) -> Pattern
    cache: HashMap<(Arch, u64), InstructionPattern>,
    /// 特征缓存
    feature_cache: HashMap<u64, PatternFeatures>,
    /// 最大缓存条目数
    max_entries: usize,
    /// 缓存命中次数
    hits: std::sync::atomic::AtomicU64,
    /// 缓存未命中次数
    misses: std::sync::atomic::AtomicU64,
}

impl PatternMatchCache {
    /// 创建新的模式匹配缓存
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: HashMap::new(),
            feature_cache: HashMap::new(),
            max_entries,
            hits: std::sync::atomic::AtomicU64::new(0),
            misses: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// 匹配或分析模式
    pub fn match_or_analyze(&mut self, arch: Arch, bytes: &[u8]) -> InstructionPattern {
        let hash = self.hash_bytes(bytes);

        // 快速路径：缓存命中
        if let Some(pattern) = self.cache.get(&(arch, hash)) {
            self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return pattern.clone();
        }

        // 缓存未命中，分析模式
        self.misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // 先提取特征（可能从feature_cache缓存）
        let features = if let Some(cached_features) = self.feature_cache.get(&hash) {
            cached_features.clone()
        } else {
            let extracted = self.extract_features(bytes);
            self.feature_cache.insert(hash, extracted.clone());
            extracted
        };

        // 创建指令模式
        let pattern = InstructionPattern {
            name: self.infer_pattern_name(&features, arch),
            arch,
            features: features.clone(),
            operand_types: self.infer_operand_types(bytes, arch),
            is_memory: features.has_load || features.has_store,
            is_control_flow: features.has_branch,
        };

        // 插入缓存（如果容量限制，使用LRU驱逐）
        if self.cache.len() >= self.max_entries {
            // 简单策略：移除第一个条目（实际应该使用LRU）
            let key_to_remove = self.cache.keys().next().copied();
            if let Some(key) = key_to_remove {
                self.cache.remove(&key);
            }
        }

        self.cache.insert((arch, hash), pattern.clone());
        pattern
    }

    /// 提取指令特征
    fn extract_features(&self, bytes: &[u8]) -> PatternFeatures {
        // 快速特征提取（位操作）
        let has_load = self.detect_load(bytes);
        let has_store = self.detect_store(bytes);
        let has_branch = self.detect_branch(bytes);
        let has_arithmetic = self.detect_arithmetic(bytes);
        let has_logic = self.detect_logic(bytes);
        let has_vector = self.detect_vector(bytes);
        let has_float = self.detect_float(bytes);
        let is_compressed = self.detect_compressed(bytes);

        // 估算操作数个数
        let operand_count = self.estimate_operand_count(bytes);

        // 指令长度
        let instruction_length = if is_compressed { 2 } else { 4 };

        PatternFeatures {
            has_load,
            has_store,
            has_branch,
            has_arithmetic,
            has_logic,
            has_vector,
            has_float,
            operand_count,
            instruction_length,
            is_compressed,
        }
    }

    /// 检测加载指令
    fn detect_load(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }

        // RISC-V: opcode[6:0] = 0x03 (LOAD)
        if !bytes.is_empty() {
            let opcode = bytes[0] & 0x7F;
            if opcode == 0x03 {
                return true;
            }
        }

        // x86-64: 检查常见加载操作码
        if !bytes.is_empty() {
            match bytes[0] {
                0x8B | 0x8D | 0xA1 | 0xA3 => return true, // MOV
                0xB8..=0xBF => return true,               // MOV r32, imm32
                _ => {}
            }
        }

        false
    }

    /// 检测存储指令
    fn detect_store(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }

        // RISC-V: opcode[6:0] = 0x23 (STORE)
        if !bytes.is_empty() {
            let opcode = bytes[0] & 0x7F;
            if opcode == 0x23 {
                return true;
            }
        }

        // x86-64: 检查常见存储操作码
        if !bytes.is_empty() {
            match bytes[0] {
                0x89 | 0x8C | 0xA2 | 0xA3 => return true, // MOV
                _ => {}
            }
        }

        false
    }

    /// 检测分支指令
    fn detect_branch(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }

        // RISC-V: 检查分支opcode
        if !bytes.is_empty() {
            let opcode = bytes[0] & 0x7F;
            // BRANCH (0x63), JAL (0x6F), JALR (0x67)
            if opcode == 0x63 || opcode == 0x6F || opcode == 0x67 {
                return true;
            }
        }

        // x86-64: 检查分支操作码
        if !bytes.is_empty() {
            match bytes[0] {
                0x70..=0x7F | 0xE8 | 0xE9 | 0xEB | 0xFF => return true, // Jcc, CALL, JMP
                _ => {}
            }
        }

        false
    }

    /// 检测算术指令
    fn detect_arithmetic(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }

        // RISC-V: 检查算术opcode (OP = 0x33, OP-IMM = 0x13)
        if !bytes.is_empty() {
            let opcode = bytes[0] & 0x7F;
            if opcode == 0x33 || opcode == 0x13 {
                return true;
            }
        }

        // x86-64: 检查算术操作码
        if !bytes.is_empty() {
            match bytes[0] {
                0x00..=0x05 | 0x08..=0x0D | 0x28..=0x2D | 0x38..=0x3D | 0x50..=0x5D => {
                    return true;
                }
                _ => {}
            }
        }

        false
    }

    /// 检测逻辑指令
    fn detect_logic(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }

        // RISC-V: 检查逻辑opcode (OP = 0x33, AND/OR/XOR funct3)
        if !bytes.is_empty() {
            let opcode = bytes[0] & 0x7F;
            if opcode == 0x33 || opcode == 0x13 {
                // 进一步检查funct3
                if bytes.len() >= 2 {
                    let funct3 = (bytes[1] >> 4) & 0x7;
                    if funct3 == 0x1 || funct3 == 0x4 || funct3 == 0x6 {
                        // SLLI, SRLI/SRAI, AND/OR/XOR
                        return true;
                    }
                }
            }
        }

        // x86-64: 检查逻辑操作码
        if !bytes.is_empty() {
            match bytes[0] {
                0x20..=0x25 | 0x30..=0x35 | 0x80..=0x83 | 0x84..=0x86 | 0xA8..=0xAF => {
                    return true;
                }
                _ => {}
            }
        }

        false
    }

    /// 检测向量指令
    fn detect_vector(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }

        // RISC-V: 向量扩展 opcode = 0x57 (0b1010111)
        if !bytes.is_empty() {
            let opcode = bytes[0] & 0x7F;
            if opcode == 0x57 {
                return true;
            }
        }

        // ARM/AArch64: NEON指令
        if !bytes.is_empty() {
            // 简化检测：检查NEON操作码范围
            if (bytes[0] & 0xE0) == 0x40 || (bytes[0] & 0xF0) == 0x00 {
                return true;
            }
        }

        false
    }

    /// 检测浮点指令
    fn detect_float(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }

        // RISC-V: 浮点扩展 opcode = 0x07 (LOAD-FP) 或 0x27 (STORE-FP)
        if !bytes.is_empty() {
            let opcode = bytes[0] & 0x7F;
            if opcode == 0x07 || opcode == 0x27 || opcode == 0x53 {
                return true;
            }
        }

        // x86-64: x87/SSE/AVX指令
        if !bytes.is_empty() {
            match bytes[0] {
                0xD8..=0xDF | 0xF0..=0xFF | 0x0F => return true,
                _ => {}
            }
        }

        false
    }

    /// 检测压缩指令（RISC-V C扩展）
    fn detect_compressed(&self, bytes: &[u8]) -> bool {
        if bytes.len() < 2 {
            return false;
        }

        // 压缩指令：bits[1:0] != 0b11
        let bits = bytes[0] & 0x3;
        bits != 0x3
    }

    /// 估算操作数个数
    fn estimate_operand_count(&self, bytes: &[u8]) -> u8 {
        if bytes.is_empty() {
            return 0;
        }

        // 简化策略：根据指令类型估算
        let is_load = self.detect_load(bytes);
        let is_store = self.detect_store(bytes);
        let is_branch = self.detect_branch(bytes);
        let is_arithmetic = self.detect_arithmetic(bytes);

        if is_load || is_store {
            2 // base + offset
        } else if is_branch {
            1 // target
        } else if is_arithmetic {
            3 // rd, rs1, rs2 或 rd, rs1, imm
        } else {
            2 // 默认
        }
    }

    /// 推断模式名称
    fn infer_pattern_name(&self, features: &PatternFeatures, arch: Arch) -> String {
        match arch {
            Arch::Riscv64 => {
                if features.is_compressed {
                    "riscv_c_insn".to_string()
                } else if features.has_load {
                    "riscv_load".to_string()
                } else if features.has_store {
                    "riscv_store".to_string()
                } else if features.has_branch {
                    "riscv_branch".to_string()
                } else if features.has_arithmetic {
                    "riscv_arith".to_string()
                } else if features.has_float {
                    "riscv_float".to_string()
                } else if features.has_vector {
                    "riscv_vector".to_string()
                } else {
                    "riscv_unknown".to_string()
                }
            }
            Arch::X86_64 => {
                if features.has_load {
                    "x86_load".to_string()
                } else if features.has_store {
                    "x86_store".to_string()
                } else if features.has_branch {
                    "x86_branch".to_string()
                } else if features.has_arithmetic {
                    "x86_arith".to_string()
                } else if features.has_float {
                    "x86_float".to_string()
                } else if features.has_vector {
                    "x86_vector".to_string()
                } else {
                    "x86_unknown".to_string()
                }
            }
            Arch::AArch64 | Arch::Arm => {
                if features.has_load || features.has_store {
                    "arm_mem".to_string()
                } else if features.has_branch {
                    "arm_branch".to_string()
                } else if features.has_arithmetic {
                    "arm_arith".to_string()
                } else if features.has_vector {
                    "arm_neon".to_string()
                } else {
                    "arm_unknown".to_string()
                }
            }
            Arch::Unknown => "unknown".to_string(),
        }
    }

    /// 推断操作数类型
    fn infer_operand_types(&self, _bytes: &[u8], _arch: Arch) -> Vec<OperandType> {
        // 简化实现：返回默认操作数类型
        vec![OperandType::Register, OperandType::Register]
    }

    /// 计算字节序列哈希
    fn hash_bytes(&self, bytes: &[u8]) -> u64 {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    }

    /// 失效特定架构的缓存
    pub fn invalidate_arch(&mut self, arch: Arch) {
        self.cache.retain(|key, _value| key.0 != arch);
    }

    /// 清空所有缓存
    pub fn clear(&mut self) {
        self.cache.clear();
        self.feature_cache.clear();
    }

    /// 获取缓存大小
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// 获取命中率
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.misses.load(std::sync::atomic::Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// �缓存统计信息
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.cache.len(),
            hits: self.hits.load(std::sync::atomic::Ordering::Relaxed),
            misses: self.misses.load(std::sync::atomic::Ordering::Relaxed),
            hit_rate: self.hit_rate(),
        }
    }
}

// ============================================================================
// 缓存统计
// ============================================================================

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// 缓存条目数
    pub entries: usize,
    /// 命中次数
    pub hits: u64,
    /// 未命中次数
    pub misses: u64,
    /// 命中率
    pub hit_rate: f64,
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_cache_creation() {
        let cache = PatternMatchCache::new(1000);
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_riscv_load_detection() {
        let mut cache = PatternMatchCache::new(1000);

        // LB instruction: 0x00000303 (opcode=0x03)
        let lb_insn: u32 = 0x00000303;
        let bytes = lb_insn.to_le_bytes();

        let pattern = cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);
        assert!(pattern.features.has_load);
        assert!(!pattern.features.has_store);
    }

    #[test]
    fn test_riscv_store_detection() {
        let mut cache = PatternMatchCache::new(1000);

        // SB instruction: 0x0010A023 (opcode=0x23)
        let sb_insn: u32 = 0x0010A023;
        let bytes = sb_insn.to_le_bytes();

        let pattern = cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);
        assert!(!pattern.features.has_load);
        assert!(pattern.features.has_store);
    }

    #[test]
    fn test_riscv_branch_detection() {
        let mut cache = PatternMatchCache::new(1000);

        // BEQ instruction: 0x00000063 (opcode=0x63)
        let beq_insn: u32 = 0x00000063;
        let bytes = beq_insn.to_le_bytes();

        let pattern = cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);
        assert!(pattern.features.has_branch);
        assert!(pattern.is_control_flow);
    }

    #[test]
    fn test_riscv_arithmetic_detection() {
        let mut cache = PatternMatchCache::new(1000);

        // ADD instruction: 0x00000333 (opcode=0x33)
        let add_insn: u32 = 0x00000333;
        let bytes = add_insn.to_le_bytes();

        let pattern = cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);
        assert!(pattern.features.has_arithmetic);
        assert!(!pattern.features.has_load);
        assert!(!pattern.features.has_store);
    }

    #[test]
    fn test_riscv_compressed_detection() {
        let mut cache = PatternMatchCache::new(1000);

        // C.ADDI: 0x0001 (not compressed pattern, but let's test)
        // 实际压缩指令: bits[1:0] != 0b11
        let compressed_insn: u16 = 0x0001; // 压缩指令
        let bytes = compressed_insn.to_le_bytes();

        let pattern = cache.match_or_analyze(Arch::Riscv64, &bytes[..2]);
        assert!(pattern.features.is_compressed);
        assert_eq!(pattern.features.instruction_length, 2);
    }

    #[test]
    fn test_cache_hit_rate() {
        let mut cache = PatternMatchCache::new(1000);

        // 第一次访问（未命中）
        let insn: u32 = 0x00000303;
        let bytes = insn.to_le_bytes();
        cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);

        // 第二次访问（命中）
        cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_pattern_features_hash() {
        let features1 = PatternFeatures {
            has_load: true,
            has_store: false,
            has_branch: false,
            has_arithmetic: false,
            has_logic: false,
            has_vector: false,
            has_float: false,
            operand_count: 2,
            instruction_length: 4,
            is_compressed: false,
        };

        let features2 = features1.clone();
        assert_eq!(features1.hash(), features2.hash());
    }

    #[test]
    fn test_invalidate_arch() {
        let mut cache = PatternMatchCache::new(1000);

        // 添加一些模式
        let insn: u32 = 0x00000303;
        let bytes = insn.to_le_bytes();
        cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);
        cache.match_or_analyze(Arch::X86_64, &bytes[..4]);

        assert_eq!(cache.len(), 2);

        // 失效RISC-V架构
        cache.invalidate_arch(Arch::Riscv64);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_clear_cache() {
        let mut cache = PatternMatchCache::new(1000);

        let insn: u32 = 0x00000303;
        let bytes = insn.to_le_bytes();
        cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);

        assert!(cache.len() > 0);

        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    // ========== New Comprehensive Tests ==========

    #[test]
    fn test_all_arch_enum_variants() {
        let archs = vec![
            Arch::Unknown,
            Arch::X86_64,
            Arch::Riscv64,
            Arch::AArch64,
            Arch::Arm,
        ];

        assert_eq!(archs.len(), 5); // Verify all variants
    }

    #[test]
    fn test_arch_default() {
        let arch = Arch::default();
        assert_eq!(arch, Arch::Unknown);
    }

    #[test]
    fn test_pattern_features_all_true() {
        let features = PatternFeatures {
            has_load: true,
            has_store: true,
            has_branch: true,
            has_arithmetic: true,
            has_logic: true,
            has_vector: true,
            has_float: true,
            operand_count: 5,
            instruction_length: 16,
            is_compressed: true,
        };

        assert!(features.has_load);
        assert!(features.has_store);
        assert!(features.has_branch);
        assert!(features.has_arithmetic);
        assert!(features.has_logic);
        assert!(features.has_vector);
        assert!(features.has_float);
        assert_eq!(features.operand_count, 5);
        assert_eq!(features.instruction_length, 16);
        assert!(features.is_compressed);
    }

    #[test]
    fn test_pattern_features_all_false() {
        let features = PatternFeatures {
            has_load: false,
            has_store: false,
            has_branch: false,
            has_arithmetic: false,
            has_logic: false,
            has_vector: false,
            has_float: false,
            operand_count: 0,
            instruction_length: 0,
            is_compressed: false,
        };

        assert!(!features.has_load);
        assert!(!features.has_store);
        assert!(!features.has_branch);
        assert!(!features.has_arithmetic);
        assert!(!features.has_logic);
        assert!(!features.has_vector);
        assert!(!features.has_float);
        assert_eq!(features.operand_count, 0);
        assert_eq!(features.instruction_length, 0);
        assert!(!features.is_compressed);
    }

    #[test]
    fn test_pattern_features_clone() {
        let features1 = PatternFeatures {
            has_load: true,
            has_store: false,
            has_branch: false,
            has_arithmetic: false,
            has_logic: false,
            has_vector: false,
            has_float: false,
            operand_count: 2,
            instruction_length: 4,
            is_compressed: false,
        };

        let features2 = features1.clone();
        assert_eq!(features1, features2);
        assert_eq!(features1.hash(), features2.hash());
    }

    #[test]
    fn test_all_operand_types() {
        let types = vec![
            OperandType::Register,
            OperandType::Immediate,
            OperandType::Memory,
            OperandType::Float,
            OperandType::Vector,
            OperandType::Unknown,
        ];

        assert_eq!(types.len(), 6); // Verify all variants
    }

    #[test]
    fn test_instruction_pattern_creation() {
        let pattern = InstructionPattern {
            name: "test_pattern".to_string(),
            arch: Arch::X86_64,
            features: PatternFeatures {
                has_load: true,
                has_store: false,
                has_branch: false,
                has_arithmetic: false,
                has_logic: false,
                has_vector: false,
                has_float: false,
                operand_count: 2,
                instruction_length: 4,
                is_compressed: false,
            },
            operand_types: vec![OperandType::Register, OperandType::Memory],
            is_memory: true,
            is_control_flow: false,
        };

        assert_eq!(pattern.name, "test_pattern");
        assert_eq!(pattern.arch, Arch::X86_64);
        assert!(pattern.is_memory);
        assert!(!pattern.is_control_flow);
        assert_eq!(pattern.operand_types.len(), 2);
    }

    #[test]
    fn test_instruction_pattern_clone() {
        let pattern1 = InstructionPattern {
            name: "test".to_string(),
            arch: Arch::Riscv64,
            features: PatternFeatures {
                has_load: false,
                has_store: false,
                has_branch: true,
                has_arithmetic: false,
                has_logic: false,
                has_vector: false,
                has_float: false,
                operand_count: 1,
                instruction_length: 4,
                is_compressed: false,
            },
            operand_types: vec![OperandType::Register],
            is_memory: false,
            is_control_flow: true,
        };

        let pattern2 = pattern1.clone();
        assert_eq!(pattern1.name, pattern2.name);
        assert_eq!(pattern1.arch, pattern2.arch);
        assert_eq!(pattern1.is_control_flow, pattern2.is_control_flow);
    }

    #[test]
    fn test_cache_eviction_when_full() {
        let mut cache = PatternMatchCache::new(3); // Small cache

        // Add 4 different patterns (should evict one)
        for i in 0..4 {
            let insn: u32 = 0x00000000 + (i as u32);
            let bytes = insn.to_le_bytes();
            cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);
        }

        // Cache should not grow beyond max_entries
        assert!(cache.len() <= 3);
    }

    #[test]
    fn test_multiple_architectures_in_cache() {
        let mut cache = PatternMatchCache::new(100);

        let insn: u32 = 0x00000303;
        let bytes = insn.to_le_bytes();

        // Add same instruction for different architectures
        cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);
        cache.match_or_analyze(Arch::X86_64, &bytes[..4]);
        cache.match_or_analyze(Arch::AArch64, &bytes[..4]);
        cache.match_or_analyze(Arch::Arm, &bytes[..4]);

        // Each architecture should have its own entry
        assert_eq!(cache.len(), 4);
    }

    #[test]
    fn test_cache_stats_after_operations() {
        let mut cache = PatternMatchCache::new(100);

        let insn1: u32 = 0x00000303;
        let insn2: u32 = 0x0010A023;
        let bytes1 = insn1.to_le_bytes();
        let bytes2 = insn2.to_le_bytes();

        // First access (miss)
        cache.match_or_analyze(Arch::Riscv64, &bytes1[..4]);
        // Second access (hit)
        cache.match_or_analyze(Arch::Riscv64, &bytes1[..4]);
        // Third access (miss)
        cache.match_or_analyze(Arch::Riscv64, &bytes2[..4]);
        // Fourth access (hit)
        cache.match_or_analyze(Arch::Riscv64, &bytes1[..4]);

        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 2);
        assert!((stats.hit_rate - 0.5).abs() < 0.01);
        assert_eq!(stats.hits + stats.misses, 4);
    }

    #[test]
    fn test_hit_rate_with_no_accesses() {
        let cache = PatternMatchCache::new(100);
        let stats = cache.stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.hit_rate, 0.0);
    }

    #[test]
    fn test_invalidate_nonexistent_arch() {
        let mut cache = PatternMatchCache::new(100);

        let insn: u32 = 0x00000303;
        let bytes = insn.to_le_bytes();
        cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);

        let len_before = cache.len();
        cache.invalidate_arch(Arch::X86_64); // This arch has no entries
        assert_eq!(cache.len(), len_before); // Should not change
    }

    #[test]
    fn test_invalidate_all_archs() {
        let mut cache = PatternMatchCache::new(100);

        let insn: u32 = 0x00000303;
        let bytes = insn.to_le_bytes();

        cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);
        cache.match_or_analyze(Arch::X86_64, &bytes[..4]);
        cache.match_or_analyze(Arch::AArch64, &bytes[..4]);

        assert_eq!(cache.len(), 3);

        // Invalidate all
        cache.invalidate_arch(Arch::Riscv64);
        cache.invalidate_arch(Arch::X86_64);
        cache.invalidate_arch(Arch::AArch64);

        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_empty_cache_operations() {
        let cache = PatternMatchCache::new(100);
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());

        // Clearing empty cache should not panic
        let mut cache = PatternMatchCache::new(100);
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_x86_64_pattern_detection() {
        let mut cache = PatternMatchCache::new(100);

        // MOV EAX, [EBX] - load instruction
        let mov_insn: u32 = 0x8B03; // Simplified x86 instruction
        let bytes = mov_insn.to_le_bytes();

        let pattern = cache.match_or_analyze(Arch::X86_64, &bytes[..2]);

        // Should detect it as some kind of pattern
        assert!(!pattern.name.is_empty());
        assert_eq!(pattern.arch, Arch::X86_64);
    }

    #[test]
    fn test_aarch64_pattern_detection() {
        let mut cache = PatternMatchCache::new(100);

        // LDR X0, [X1] - load instruction
        let ldr_insn: u32 = 0xF9400000; // AArch64 LDR instruction
        let bytes = ldr_insn.to_le_bytes();

        let pattern = cache.match_or_analyze(Arch::AArch64, &bytes[..4]);

        // Should detect it as some kind of pattern
        assert!(!pattern.name.is_empty());
        assert_eq!(pattern.arch, Arch::AArch64);
    }

    #[test]
    fn test_arm_pattern_detection() {
        let mut cache = PatternMatchCache::new(100);

        // LDR R0, [R1] - ARM load instruction
        let ldr_insn: u32 = 0xE5910000; // ARM LDR instruction
        let bytes = ldr_insn.to_le_bytes();

        let pattern = cache.match_or_analyze(Arch::Arm, &bytes[..4]);

        // Should detect it as some kind of pattern
        assert!(!pattern.name.is_empty());
        assert_eq!(pattern.arch, Arch::Arm);
    }

    #[test]
    fn test_pattern_with_memory_operands() {
        let mut cache = PatternMatchCache::new(100);

        // Load instruction should have memory operands
        let insn: u32 = 0x00000303; // LB
        let bytes = insn.to_le_bytes();

        let pattern = cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);

        assert!(pattern.is_memory);
        assert!(pattern.features.has_load);
        assert!(!pattern.features.has_store);
    }

    #[test]
    fn test_pattern_with_control_flow() {
        let mut cache = PatternMatchCache::new(100);

        // Branch instruction
        let insn: u32 = 0x00000063; // BEQ
        let bytes = insn.to_le_bytes();

        let pattern = cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);

        assert!(pattern.is_control_flow);
        assert!(pattern.features.has_branch);
    }

    #[test]
    fn test_pattern_operand_types() {
        let mut cache = PatternMatchCache::new(100);

        let insn: u32 = 0x00000303; // LB with register operands
        let bytes = insn.to_le_bytes();

        let pattern = cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);

        // Should have some operand types
        assert!(!pattern.operand_types.is_empty());
    }

    #[test]
    fn test_different_instruction_lengths() {
        let mut cache = PatternMatchCache::new(100);

        // 4-byte instruction
        let insn4: u32 = 0x00000303;
        let bytes4 = insn4.to_le_bytes();
        let pattern4 = cache.match_or_analyze(Arch::Riscv64, &bytes4[..4]);
        assert_eq!(pattern4.features.instruction_length, 4);

        // 2-byte compressed instruction
        let insn2: u16 = 0x0001;
        let bytes2 = insn2.to_le_bytes();
        let pattern2 = cache.match_or_analyze(Arch::Riscv64, &bytes2[..2]);
        assert_eq!(pattern2.features.instruction_length, 2);
    }
}
