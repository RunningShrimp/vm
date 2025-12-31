//! ML增强特征提取器
//!
//! 为JIT编译决策提供增强的特征工程支持。

use vm_ir::IRBlock;
use std::collections::HashMap;

// ============================================================================
// 增强特征定义
// ============================================================================

/// 增强的执行特征
///
/// 包含原有特征和新增的高级特征
#[derive(Clone, Debug)]
pub struct ExecutionFeaturesEnhanced {
    // === 原有基础特征 ===
    /// IR块大小（指令数）
    pub block_size: usize,
    /// 指令计数
    pub instr_count: usize,
    /// 分支指令计数
    pub branch_count: usize,
    /// 内存访问计数
    pub memory_access_count: usize,
    /// 执行次数
    pub execution_count: u64,
    /// 缓存命中率
    pub cache_hit_rate: f64,

    // === 新增特征：指令混合 ===
    /// 指令混合特征
    pub instruction_mix: InstMixFeatures,

    // === 新增特征：控制流 ===
    /// 控制流复杂度（圈复杂度）
    pub control_flow_complexity: f64,
    /// 循环嵌套深度
    pub loop_nest_depth: u8,
    /// 是否有递归调用
    pub has_recursion: bool,

    // === 新增特征：数据局部性 ===
    /// 数据局部性评分
    pub data_locality: f64,
    /// 内存访问模式顺序性
    pub memory_sequentiality: f64,

    // === 新增特征：编译历史 ===
    /// 历史编译信息
    pub compilation_history: CompilationHistory,

    // === 新增特征：寄存器压力 ===
    /// 寄存器压力（0-1，1表示最高压力）
    pub register_pressure: f64,

    // === 新增特征：代码热度和稳定性 ===
    /// 代码热度（执行频率）
    pub code_heat: f64,
    /// 代码稳定性（变化频率）
    pub code_stability: f64,
}

/// 指令混合特征
#[derive(Clone, Debug)]
pub struct InstMixFeatures {
    /// 算术指令比例
    pub arithmetic_ratio: f64,
    /// 内存指令比例
    pub memory_ratio: f64,
    /// 分支指令比例
    pub branch_ratio: f64,
    /// 向量指令比例
    pub vector_ratio: f64,
    /// 浮点指令比例
    pub float_ratio: f64,
    /// 调用指令比例
    pub call_ratio: f64,
}

/// 编译历史信息
#[derive(Clone, Debug)]
pub struct CompilationHistory {
    /// 之前编译次数
    pub previous_compilations: u32,
    /// 平均编译时间（微秒）
    pub avg_compilation_time_us: f64,
    /// 上次编译收益（加速比）
    pub last_compile_benefit: f64,
    /// 上次编译是否成功
    pub last_compile_success: bool,
}

// ============================================================================
// 特征提取器
// ============================================================================

/// 增强特征提取器
pub struct FeatureExtractorEnhanced {
    /// 历史窗口大小
    history_window: usize,
    /// 记录的执行历史
    execution_history: HashMap<u64, Vec<ExecutionRecord>>,
}

/// 执行记录
#[derive(Clone, Debug)]
struct ExecutionRecord {
    timestamp: u64,
    execution_time_ns: u64,
    memory_accesses: Vec<(u64, u8)>, // (address, size)
}

impl FeatureExtractorEnhanced {
    /// 创建新的增强特征提取器
    pub fn new(history_window: usize) -> Self {
        Self {
            history_window,
            execution_history: HashMap::new(),
        }
    }

    /// 提取增强特征
    pub fn extract_enhanced(&mut self, block: &IRBlock) -> ExecutionFeaturesEnhanced {
        // 1. 提取基础特征
        let block_size = block.instructions.len();
        let instr_count = block.instructions.len();
        let branch_count = self.count_branches(block);
        let memory_access_count = self.count_memory_accesses(block);

        // 2. 提取指令混合特征
        let instruction_mix = self.analyze_instruction_mix(block);

        // 3. 计算控制流复杂度（圈复杂度）
        let control_flow_complexity = self.compute_cyclomatic_complexity(block);

        // 4. 检测循环嵌套
        let loop_nest_depth = self.detect_loop_nesting(block);

        // 5. 检测递归
        let has_recursion = self.detect_recursion(block);

        // 6. 计算数据局部性
        let data_locality = self.compute_data_locality(block);

        // 7. 计算内存访问顺序性
        let memory_sequentiality = self.compute_memory_sequentiality(block);

        // 8. 获取编译历史
        let compilation_history = self.get_compilation_history(block);

        // 9. 计算寄存器压力
        let register_pressure = self.compute_register_pressure(block);

        // 10. 计算代码热度和稳定性
        let block_hash = self.hash_block(block);
        let (code_heat, code_stability) = self.compute_heat_and_stability(&block_hash);

        // 从历史记录中获取执行统计
        let (execution_count, cache_hit_rate) = self.get_execution_stats(&block_hash);

        ExecutionFeaturesEnhanced {
            block_size,
            instr_count,
            branch_count,
            memory_access_count,
            execution_count,
            cache_hit_rate,
            instruction_mix,
            control_flow_complexity,
            loop_nest_depth,
            has_recursion,
            data_locality,
            memory_sequentiality,
            compilation_history,
            register_pressure,
            code_heat,
            code_stability,
        }
    }

    /// 分析指令混合
    fn analyze_instruction_mix(&self, block: &IRBlock) -> InstMixFeatures {
        let mut arithmetic = 0usize;
        let mut memory = 0usize;
        let mut branch = 0usize;
        let mut vector = 0usize;
        let mut float = 0usize;
        let mut call = 0usize;

        for insn in &block.instructions {
            match insn.opcode {
                // 算术指令
                x if x >= 0x10 && x < 0x20 => arithmetic += 1,
                // 内存指令
                x if x >= 0x20 && x < 0x30 => memory += 1,
                // 分支指令
                x if x >= 0x30 && x < 0x40 => branch += 1,
                // 向量指令
                x if x >= 0x50 && x < 0x60 => vector += 1,
                // 浮点指令
                x if x >= 0x60 && x < 0x70 => float += 1,
                // 调用指令
                x if x >= 0x70 && x < 0x80 => call += 1,
                _ => {}
            }
        }

        let total = block.instructions.len();
        if total == 0 {
            return InstMixFeatures {
                arithmetic_ratio: 0.0,
                memory_ratio: 0.0,
                branch_ratio: 0.0,
                vector_ratio: 0.0,
                float_ratio: 0.0,
                call_ratio: 0.0,
            };
        }

        InstMixFeatures {
            arithmetic_ratio: arithmetic as f64 / total as f64,
            memory_ratio: memory as f64 / total as f64,
            branch_ratio: branch as f64 / total as f64,
            vector_ratio: vector as f64 / total as f64,
            float_ratio: float as f64 / total as f64,
            call_ratio: call as f64 / total as f64,
        }
    }

    /// 计算圈复杂度
    ///
    /// 圈复杂度 = E - N + 2P
    /// - E: 边数
    /// - N: 节点数
    /// - P: 连通组件数
    fn compute_cyclomatic_complexity(&self, block: &IRBlock) -> f64 {
        let nodes = block.instructions.len() as f64;
        let edges = self.count_edges(block) as f64;
        let p = 1.0; // 单个连通组件

        edges - nodes + 2.0 * p
    }

    /// 计算CFG边数
    fn count_edges(&self, block: &IRBlock) -> usize {
        let mut edges = 0;
        for insn in &block.instructions {
            // 每个分支指令增加2条边（true和false分支）
            if self.is_branch(insn) {
                edges += 2;
            } else {
                edges += 1; // 顺序边
            }
        }
        edges
    }

    /// 检测是否是分支指令
    fn is_branch(&self, insn: &vm_ir::Instruction) -> bool {
        matches!(insn.opcode, 0x30..=0x3F)
    }

    /// 统计分支指令数
    fn count_branches(&self, block: &IRBlock) -> usize {
        block
            .instructions
            .iter()
            .filter(|insn| self.is_branch(insn))
            .count()
    }

    /// 统计内存访问数
    fn count_memory_accesses(&self, block: &IRBlock) -> usize {
        block
            .instructions
            .iter()
            .filter(|insn| matches!(insn.opcode, 0x20..=0x2F))
            .count()
    }

    /// 检测循环嵌套深度
    fn detect_loop_nesting(&self, block: &IRBlock) -> u8 {
        let mut depth = 0u8;
        let mut current_depth = 0u8;

        for insn in &block.instructions {
            // 检测循环开始（向后跳转）
            if insn.opcode == 0x35 {
                // 条件分支指令
                current_depth = current_depth.saturating_add(1);
                depth = depth.max(current_depth);
            }
            // 检测循环结束
            else if insn.opcode == 0x36 {
                current_depth = current_depth.saturating_sub(1);
            }
        }

        depth
    }

    /// 检测递归调用
    fn detect_recursion(&self, block: &IRBlock) -> bool {
        // 简化实现：检测是否有调用自身的指令
        for insn in &block.instructions {
            if insn.opcode >= 0x70 && insn.opcode < 0x80 {
                // 调用指令
                if let Some(target) = insn.operands.first() {
                    if let vm_ir::Operand::Addr(addr) = target {
                        // 如果调用地址在当前块范围内，可能是递归
                        let block_start = block.address;
                        let block_end = block_start + block.instructions.len() as u64 * 4;
                        if *addr >= block_start && *addr < block_end {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// 计算数据局部性
    ///
    /// 评分越高表示数据局部性越好
    fn compute_data_locality(&self, block: &IRBlock) -> f64 {
        let memory_accesses: Vec<_> = block
            .instructions
            .iter()
            .filter_map(|insn| {
                if matches!(insn.opcode, 0x20..=0x2F) {
                    insn.operands.first().and_then(|op| {
                        if let vm_ir::Operand::Addr(addr) = op {
                            Some(*addr)
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            })
            .collect();

        if memory_accesses.len() < 2 {
            return 0.5; // 默认中等局部性
        }

        // 计算地址间距的标准差（越小表示局部性越好）
        let mut diffs = Vec::new();
        for i in 1..memory_accesses.len() {
            let diff = (memory_accesses[i] as i64 - memory_accesses[i - 1] as i64).abs();
            diffs.push(diff);
        }

        let mean = diffs.iter().sum::<i64>() as f64 / diffs.len() as f64;
        let variance = diffs
            .iter()
            .map(|&d| (d as f64 - mean).powi(2))
            .sum::<f64>()
            / diffs.len() as f64;

        // 标准差越小，局部性越好
        // 归一化到0-1范围
        1.0 / (1.0 + variance.sqrt() / 1024.0)
    }

    /// 计算内存访问顺序性
    fn compute_memory_sequentiality(&self, block: &IRBlock) -> f64 {
        let memory_accesses: Vec<_> = block
            .instructions
            .iter()
            .filter_map(|insn| {
                if matches!(insn.opcode, 0x20..=0x2F) {
                    insn.operands.first().and_then(|op| {
                        if let vm_ir::Operand::Addr(addr) = op {
                            Some(*addr)
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            })
            .collect();

        if memory_accesses.len() < 2 {
            return 1.0; // 单个访问是完全顺序的
        }

        let mut sequential_count = 0;
        for i in 1..memory_accesses.len() {
            // 检查是否顺序递增（步长为4字节）
            if memory_accesses[i] == memory_accesses[i - 1] + 4 {
                sequential_count += 1;
            }
        }

        sequential_count as f64 / (memory_accesses.len() - 1) as f64
    }

    /// 获取编译历史
    fn get_compilation_history(&self, _block: &IRBlock) -> CompilationHistory {
        // 简化实现：返回默认值
        // 实际实现中应该从历史记录中查询
        CompilationHistory {
            previous_compilations: 0,
            avg_compilation_time_us: 0.0,
            last_compile_benefit: 1.0,
            last_compile_success: true,
        }
    }

    /// 计算寄存器压力
    fn compute_register_pressure(&self, block: &IRBlock) -> f64 {
        // 统计使用的唯一寄存器数
        let mut used_regs = std::collections::HashSet::new();

        for insn in &block.instructions {
            for op in &insn.operands {
                if let vm_ir::Operand::Reg(reg) = op {
                    used_regs.insert(*reg);
                }
            }
        }

        // 假设有32个通用寄存器
        used_regs.len() as f64 / 32.0
    }

    /// 计算代码热度和稳定性
    fn compute_heat_and_stability(&self, block_hash: &u64) -> (f64, f64) {
        let history = self.execution_history.get(block_hash);

        if let Some(records) = history {
            if records.is_empty() {
                return (0.0, 1.0);
            }

            // 代码热度：基于执行频率
            let heat = records.len() as f64 / self.history_window as f64;

            // 代码稳定性：基于执行时间的一致性
            let times: Vec<f64> = records
                .iter()
                .map(|r| r.execution_time_ns as f64)
                .collect();

            let mean = times.iter().sum::<f64>() / times.len() as f64;
            let variance = times
                .iter()
                .map(|&t| (t - mean).powi(2))
                .sum::<f64>()
                / times.len() as f64;

            let stability = 1.0 / (1.0 + variance.sqrt() / mean);

            (heat.min(1.0), stability)
        } else {
            (0.0, 1.0)
        }
    }

    /// 获取执行统计
    fn get_execution_stats(&self, block_hash: &u64) -> (u64, f64) {
        if let Some(records) = self.execution_history.get(block_hash) {
            let execution_count = records.len() as u64;
            // 简化：假设缓存命中率为0.9
            let cache_hit_rate = 0.9;
            (execution_count, cache_hit_rate)
        } else {
            (0, 0.0)
        }
    }

    /// 计算块哈希
    fn hash_block(&self, block: &IRBlock) -> u64 {
        // 简化哈希实现
        let mut hash = 0u64;
        for insn in &block.instructions {
            hash = hash.wrapping_add(insn.opcode as u64);
            hash = hash.wrapping_mul(31);
        }
        hash
    }

    /// 记录执行（用于更新历史）
    pub fn record_execution(&mut self, block_hash: u64, execution_time_ns: u64, memory_accesses: Vec<(u64, u8)>) {
        let record = ExecutionRecord {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            execution_time_ns,
            memory_accesses,
        };

        let history = self.execution_history.entry(block_hash).or_insert_with(Vec::new);
        history.push(record);

        // 限制历史大小
        if history.len() > self.history_window {
            history.remove(0);
        }
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::{Instruction, Operand};

    fn create_test_block() -> IRBlock {
        IRBlock {
            address: 0x1000,
            instructions: vec![
                Instruction {
                    opcode: 0x10, // 算术指令
                    operands: vec![],
                },
                Instruction {
                    opcode: 0x20, // 内存指令
                    operands: vec![Operand::Addr(0x2000)],
                },
                Instruction {
                    opcode: 0x30, // 分支指令
                    operands: vec![],
                },
            ],
        }
    }

    #[test]
    fn test_instruction_mix_analysis() {
        let extractor = FeatureExtractorEnhanced::new(100);
        let block = create_test_block();

        let mix = extractor.analyze_instruction_mix(&block);

        assert!(mix.arithmetic_ratio > 0.0);
        assert!(mix.memory_ratio > 0.0);
        assert!(mix.branch_ratio > 0.0);
    }

    #[test]
    fn test_cyclomatic_complexity() {
        let extractor = FeatureExtractorEnhanced::new(100);
        let block = create_test_block();

        let complexity = extractor.compute_cyclomatic_complexity(&block);

        assert!(complexity > 0.0);
    }

    #[test]
    fn test_data_locality() {
        let extractor = FeatureExtractorEnhanced::new(100);
        let block = create_test_block();

        let locality = extractor.compute_data_locality(&block);

        assert!(locality >= 0.0 && locality <= 1.0);
    }

    #[test]
    fn test_memory_sequentiality() {
        let extractor = FeatureExtractorEnhanced::new(100);
        let block = create_test_block();

        let sequentiality = extractor.compute_memory_sequentiality(&block);

        assert!(sequentiality >= 0.0 && sequentiality <= 1.0);
    }

    #[test]
    fn test_extract_enhanced_features() {
        let mut extractor = FeatureExtractorEnhanced::new(100);
        let block = create_test_block();

        let features = extractor.extract_enhanced(&block);

        assert_eq!(features.block_size, 3);
        assert_eq!(features.instr_count, 3);
        assert_eq!(features.branch_count, 1);
        assert_eq!(features.memory_access_count, 1);
    }

    #[test]
    fn test_register_pressure() {
        let extractor = FeatureExtractorEnhanced::new(100);
        let block = create_test_block();

        let pressure = extractor.compute_register_pressure(&block);

        assert!(pressure >= 0.0 && pressure <= 1.0);
    }

    #[test]
    fn test_record_execution() {
        let mut extractor = FeatureExtractorEnhanced::new(10);
        let block_hash = 12345;

        extractor.record_execution(block_hash, 1000, vec![(0x2000, 4)]);

        let (count, _) = extractor.get_execution_stats(&block_hash);
        assert_eq!(count, 1);
    }
}
