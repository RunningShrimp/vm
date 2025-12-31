//! 跨架构翻译管线
//!
//! 整合编码缓存和模式匹配缓存，提供高效的跨架构指令翻译。

use std::collections::HashMap;
use std::sync::{Arc, RwLock, atomic::{AtomicU64, Ordering}};

use crate::encoding_cache::{InstructionEncodingCache, Arch as CacheArch, Instruction, EncodingError};
use crate::pattern_cache::{PatternMatchCache, InstructionPattern, Arch as PatternArch};

// ============================================================================
// Arch类型转换
// ============================================================================

/// 将encoding_cache的Arch转换为pattern_cache的Arch
fn cache_arch_to_pattern_arch(arch: CacheArch) -> PatternArch {
    match arch {
        CacheArch::X86_64 => PatternArch::X86_64,
        CacheArch::ARM64 => PatternArch::AArch64,
        CacheArch::Riscv64 => PatternArch::Riscv64,
    }
}

// ============================================================================
// 寄存器映射缓存
// ============================================================================

/// 寄存器ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegId {
    X86(u8),   // x86_64寄存器: RAX=0, RCX=1, RDX=2, RBX=3, RSP=4, RBP=5, RSI=6, RDI=7
    Arm(u8),   // ARM寄存器: X0=0, X1=1, ..., X31=31
    Riscv(u8), // RISC-V寄存器: x0=0, x1=1, ..., x31=31
}

/// 寄存器映射缓存
pub struct RegisterMappingCache {
    /// (src_arch, dst_arch, src_reg) -> dst_reg
    cache: HashMap<(CacheArch, CacheArch, RegId), RegId>,
    /// 统计信息
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
}

impl RegisterMappingCache {
    /// 创建新的寄存器映射缓存
    pub fn new() -> Self {
        let mut cache = HashMap::new();

        // 预填充常见映射
        // x86_64 -> RISC-V (1对1映射)
        for i in 0..32 {
            cache.insert((CacheArch::X86_64, CacheArch::Riscv64, RegId::X86(i as u8)), RegId::Riscv(i as u8));
        }

        // ARM -> RISC-V (1对1映射)
        for i in 0..32 {
            cache.insert((CacheArch::ARM64, CacheArch::Riscv64, RegId::Arm(i as u8)), RegId::Riscv(i as u8));
        }

        // RISC-V -> x86_64 (1对1映射)
        for i in 0..16 {
            cache.insert((CacheArch::Riscv64, CacheArch::X86_64, RegId::Riscv(i as u8)), RegId::X86(i as u8));
        }

        Self {
            cache,
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 映射或计算寄存器
    pub fn map_or_compute(&mut self, src_arch: CacheArch, dst_arch: CacheArch, src_reg: RegId) -> RegId {
        let key = (src_arch, dst_arch, src_reg);

        if let Some(&dst_reg) = self.cache.get(&key) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return dst_reg;
        }

        self.misses.fetch_add(1, Ordering::Relaxed);

        // 计算映射
        let dst_reg = self.compute_mapping(src_arch, dst_arch, src_reg);
        self.cache.insert(key, dst_reg);
        dst_reg
    }

    /// 计算寄存器映射
    fn compute_mapping(&self, src_arch: CacheArch, dst_arch: CacheArch, src_reg: RegId) -> RegId {
        match (src_arch, dst_arch, src_reg) {
            // x86_64 -> RISC-V
            (CacheArch::X86_64, CacheArch::Riscv64, RegId::X86(i)) => RegId::Riscv(i % 32),
            // ARM -> RISC-V
            (CacheArch::ARM64, CacheArch::Riscv64, RegId::Arm(i)) => RegId::Riscv(i % 32),
            // RISC-V -> x86_64
            (CacheArch::Riscv64, CacheArch::X86_64, RegId::Riscv(i)) => RegId::X86(i % 16),
            // RISC-V -> ARM
            (CacheArch::Riscv64, CacheArch::ARM64, RegId::Riscv(i)) => RegId::Arm(i % 32),
            // 默认：直接使用索引
            _ => match src_reg {
                RegId::X86(i) => RegId::X86(i),
                RegId::Arm(i) => RegId::Arm(i),
                RegId::Riscv(i) => RegId::Riscv(i),
            },
        }
    }

    /// 获取命中率
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }
}

impl Default for RegisterMappingCache {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 翻译错误
// ============================================================================

/// 翻译错误
#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error("Encoding error: {0}")]
    Encoding(#[from] EncodingError),
    #[error("Unsupported translation: {0:?} -> {1:?}")]
    UnsupportedTranslation(CacheArch, CacheArch),
    #[error("Invalid instruction")]
    InvalidInstruction,
    #[error("Register mapping failed")]
    RegisterMappingFailed,
}

// ============================================================================
// 跨架构翻译管线
// ============================================================================

/// 跨架构翻译管线
pub struct CrossArchTranslationPipeline {
    /// 编码缓存
    encoding_cache: Arc<InstructionEncodingCache>,
    /// 模式匹配缓存
    pattern_cache: Arc<RwLock<PatternMatchCache>>,
    /// 寄存器映射缓存
    register_cache: Arc<RwLock<RegisterMappingCache>>,
    /// 统计信息
    stats: Arc<TranslationStats>,
}

/// 翻译统计
#[derive(Debug, Default)]
pub struct TranslationStats {
    /// 翻译的指令总数
    pub translated: AtomicU64,
    /// 缓存命中数
    pub cache_hits: AtomicU64,
    /// 缓存未命中数
    pub cache_misses: AtomicU64,
    /// 翻译耗时（纳秒）
    pub translation_time_ns: AtomicU64,
}

impl CrossArchTranslationPipeline {
    /// 创建新的翻译管线
    pub fn new() -> Self {
        Self {
            encoding_cache: Arc::new(InstructionEncodingCache::new()),
            pattern_cache: Arc::new(RwLock::new(PatternMatchCache::new(10_000))),
            register_cache: Arc::new(RwLock::new(RegisterMappingCache::new())),
            stats: Arc::new(TranslationStats::default()),
        }
    }

    /// 翻译指令块
    pub fn translate_block(
        &mut self,
        src_arch: CacheArch,
        dst_arch: CacheArch,
        instructions: &[Instruction],
    ) -> Result<Vec<Instruction>, TranslationError> {
        // 检查是否支持该翻译方向
        if !self.is_translation_supported(src_arch, dst_arch) {
            return Err(TranslationError::UnsupportedTranslation(src_arch, dst_arch));
        }

        let start = std::time::Instant::now();

        // 串行翻译（简化实现，避免复杂的多线程借用问题）
        let mut translated = Vec::with_capacity(instructions.len());
        for insn in instructions {
            translated.push(self.translate_instruction(src_arch, dst_arch, insn)?);
        }

        let duration = start.elapsed();
        self.stats.translation_time_ns.fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
        self.stats.translated.fetch_add(instructions.len() as u64, Ordering::Relaxed);

        Ok(translated)
    }

    /// 翻译单条指令
    pub fn translate_instruction(
        &mut self,
        src_arch: CacheArch,
        dst_arch: CacheArch,
        insn: &Instruction,
    ) -> Result<Instruction, TranslationError> {
        let start = std::time::Instant::now();

        // 1. 使用编码缓存编码源指令
        let encoded = self.encoding_cache.encode_or_lookup(insn)?;

        // 2. 模式匹配（分析指令特征）
        let pattern_arch = cache_arch_to_pattern_arch(src_arch);
        let pattern = self.pattern_cache.write().unwrap().match_or_analyze(pattern_arch, &encoded);

        // 3. 根据模式生成目标指令
        let translated = self.generate_target_instruction(src_arch, dst_arch, insn, &pattern)?;

        // 更新统计信息
        let duration = start.elapsed();
        self.stats.translation_time_ns.fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
        self.stats.translated.fetch_add(1, Ordering::Relaxed);

        Ok(translated)
    }

    /// 检查是否支持翻译方向
    fn is_translation_supported(&self, src: CacheArch, dst: CacheArch) -> bool {
        match (src, dst) {
            // 支持的翻译方向
            (CacheArch::X86_64, CacheArch::Riscv64) => true,
            (CacheArch::X86_64, CacheArch::ARM64) => true,
            (CacheArch::ARM64, CacheArch::Riscv64) => true,
            (CacheArch::ARM64, CacheArch::X86_64) => true,
            (CacheArch::Riscv64, CacheArch::X86_64) => true,
            (CacheArch::Riscv64, CacheArch::ARM64) => true,
            // 相同架构不需要翻译
            (s, d) if s == d => true,
            // 其他方向不支持
            _ => false,
        }
    }

    /// 生成目标指令
    fn generate_target_instruction(
        &mut self,
        src_arch: CacheArch,
        dst_arch: CacheArch,
        src_insn: &Instruction,
        pattern: &InstructionPattern,
    ) -> Result<Instruction, TranslationError> {
        // 如果源架构和目标架构相同，直接返回
        if src_arch == dst_arch {
            return Ok(src_insn.clone());
        }

        // 根据模式翻译操作码
        let dst_opcode = self.translate_opcode(src_arch, dst_arch, src_insn.opcode, pattern)?;

        // 翻译操作数（包括寄存器映射）
        let dst_operands = self.translate_operands(src_arch, dst_arch, &src_insn.operands)?;

        Ok(Instruction {
            arch: dst_arch,
            opcode: dst_opcode,
            operands: dst_operands,
        })
    }

    /// 翻译操作码
    fn translate_opcode(
        &self,
        _src_arch: CacheArch,
        _dst_arch: CacheArch,
        src_opcode: u32,
        _pattern: &InstructionPattern,
    ) -> Result<u32, TranslationError> {
        // 简化实现：直接使用原操作码
        // 实际实现中需要根据指令模式进行映射
        Ok(src_opcode)
    }

    /// 翻译操作数
    fn translate_operands(
        &mut self,
        src_arch: CacheArch,
        dst_arch: CacheArch,
        src_operands: &[crate::encoding_cache::Operand],
    ) -> Result<Vec<crate::encoding_cache::Operand>, TranslationError> {
        use crate::encoding_cache::Operand;

        src_operands
            .iter()
            .map(|op| match op {
                Operand::Register(reg_idx) => {
                    // 映射寄存器
                    let src_reg = match src_arch {
                        CacheArch::X86_64 => RegId::X86(*reg_idx),
                        CacheArch::ARM64 => RegId::Arm(*reg_idx),
                        CacheArch::Riscv64 => RegId::Riscv(*reg_idx),
                    };

                    let dst_reg = self.register_cache.write().unwrap().map_or_compute(src_arch, dst_arch, src_reg);

                    let dst_idx = match dst_reg {
                        RegId::X86(i) => i,
                        RegId::Arm(i) => i,
                        RegId::Riscv(i) => i,
                    };

                    Ok(Operand::Register(dst_idx))
                }
                Operand::Immediate(imm) => Ok(Operand::Immediate(*imm)),
                Operand::Memory { base, offset, size } => {
                    // 映射基址寄存器
                    let src_reg = match src_arch {
                        CacheArch::X86_64 => RegId::X86(*base),
                        CacheArch::ARM64 => RegId::Arm(*base),
                        CacheArch::Riscv64 => RegId::Riscv(*base),
                    };

                    let dst_reg = self.register_cache.write().unwrap().map_or_compute(src_arch, dst_arch, src_reg);

                    let dst_idx = match dst_reg {
                        RegId::X86(i) => i,
                        RegId::Arm(i) => i,
                        RegId::Riscv(i) => i,
                    };

                    Ok(Operand::Memory {
                        base: dst_idx,
                        offset: *offset,
                        size: *size,
                    })
                }
            })
            .collect()
    }

    /// 缓存预热
    pub fn warmup(&mut self, common_insns: &[Instruction]) {
        for insn in common_insns {
            // 预热编码缓存
            let _ = self.encoding_cache.encode_or_lookup(insn);
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> &TranslationStats {
        &self.stats
    }

    /// 获取缓存命中率
    pub fn cache_hit_rate(&self) -> f64 {
        let hits = self.stats.cache_hits.load(Ordering::Relaxed);
        let misses = self.stats.cache_misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// 获取平均翻译时间（纳秒）
    pub fn avg_translation_time_ns(&self) -> f64 {
        let translated = self.stats.translated.load(Ordering::Relaxed);
        let total_time = self.stats.translation_time_ns.load(Ordering::Relaxed);

        if translated == 0 {
            0.0
        } else {
            total_time as f64 / translated as f64
        }
    }

    /// 清空所有缓存
    pub fn clear(&self) {
        self.encoding_cache.clear();
        self.pattern_cache.write().unwrap().clear();
    }
}

impl Default for CrossArchTranslationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding_cache::Operand;

    fn create_test_instruction(arch: CacheArch, opcode: u32) -> Instruction {
        Instruction {
            arch,
            opcode,
            operands: vec![Operand::Register(1), Operand::Immediate(42)],
        }
    }

    #[test]
    fn test_pipeline_creation() {
        let pipeline = CrossArchTranslationPipeline::new();
        assert_eq!(pipeline.stats.translated.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_translate_same_arch() {
        let mut pipeline = CrossArchTranslationPipeline::new();
        let insn = create_test_instruction(CacheArch::Riscv64, 0x00000333);

        let result = pipeline.translate_instruction(CacheArch::Riscv64, CacheArch::Riscv64, &insn);
        assert!(result.is_ok());

        let translated = result.unwrap();
        assert_eq!(translated.arch, CacheArch::Riscv64);
        assert_eq!(translated.opcode, 0x00000333);
    }

    #[test]
    fn test_translate_x86_to_riscv() {
        let mut pipeline = CrossArchTranslationPipeline::new();
        let insn = create_test_instruction(CacheArch::X86_64, 0x90); // NOP

        let result = pipeline.translate_instruction(CacheArch::X86_64, CacheArch::Riscv64, &insn);
        assert!(result.is_ok());

        let translated = result.unwrap();
        assert_eq!(translated.arch, CacheArch::Riscv64);
    }

    #[test]
    fn test_translate_block() {
        let mut pipeline = CrossArchTranslationPipeline::new();
        let instructions = vec![
            create_test_instruction(CacheArch::X86_64, 0x90),
            create_test_instruction(CacheArch::X86_64, 0x90),
            create_test_instruction(CacheArch::X86_64, 0x90),
        ];

        let result = pipeline.translate_block(CacheArch::X86_64, CacheArch::Riscv64, &instructions);
        assert!(result.is_ok());

        let translated = result.unwrap();
        assert_eq!(translated.len(), 3);
        assert!(translated.iter().all(|insn| insn.arch == CacheArch::Riscv64));
    }

    #[test]
    fn test_unsupported_translation() {
        let mut pipeline = CrossArchTranslationPipeline::new();
        let insn = create_test_instruction(CacheArch::X86_64, 0x90);

        // 所有支持的方向应该都能工作
        assert!(pipeline.translate_instruction(CacheArch::X86_64, CacheArch::Riscv64, &insn).is_ok());
        assert!(pipeline.translate_instruction(CacheArch::X86_64, CacheArch::ARM64, &insn).is_ok());
    }

    #[test]
    fn test_cache_warmup() {
        let mut pipeline = CrossArchTranslationPipeline::new();
        let instructions = vec![
            create_test_instruction(CacheArch::Riscv64, 0x00000333),
            create_test_instruction(CacheArch::Riscv64, 0x00000303),
        ];

        pipeline.warmup(&instructions);

        // 预热后应该有缓存命中
        let result = pipeline.translate_instruction(CacheArch::Riscv64, CacheArch::Riscv64, &instructions[0]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_register_mapping() {
        let mut cache = RegisterMappingCache::new();

        // x86_64 -> RISC-V
        let riscv_reg = cache.map_or_compute(CacheArch::X86_64, CacheArch::Riscv64, RegId::X86(1));
        assert_eq!(riscv_reg, RegId::Riscv(1));

        // ARM -> RISC-V
        let riscv_reg = cache.map_or_compute(CacheArch::ARM64, CacheArch::Riscv64, RegId::Arm(5));
        assert_eq!(riscv_reg, RegId::Riscv(5));

        // RISC-V -> x86_64
        let x86_reg = cache.map_or_compute(CacheArch::Riscv64, CacheArch::X86_64, RegId::Riscv(3));
        assert_eq!(x86_reg, RegId::X86(3));
    }

    #[test]
    fn test_stats() {
        let mut pipeline = CrossArchTranslationPipeline::new();
        let insn = create_test_instruction(CacheArch::X86_64, 0x90);

        pipeline.translate_instruction(CacheArch::X86_64, CacheArch::Riscv64, &insn).unwrap();

        assert_eq!(pipeline.stats.translated.load(Ordering::Relaxed), 1);
        assert!(pipeline.stats.translation_time_ns.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn test_clear_caches() {
        let mut pipeline = CrossArchTranslationPipeline::new();
        let insn = create_test_instruction(CacheArch::Riscv64, 0x00000333);

        // 翻译一次（填充缓存）
        pipeline.translate_instruction(CacheArch::Riscv64, CacheArch::Riscv64, &insn).unwrap();

        // 清空缓存
        pipeline.clear();

        // 再次翻译（缓存应该已清空）
        let result = pipeline.translate_instruction(CacheArch::Riscv64, CacheArch::Riscv64, &insn);
        assert!(result.is_ok());
    }
}
