//! LLVM 优化管线
//!
//! 配置和管理 LLVM Pass 进行代码优化。

use crate::{LiftError, LiftResult};

/// 优化级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationLevel {
    /// O0: 快速编译，不优化
    O0,
    /// O1: 平衡编译速度和优化程度
    O1,
    /// O2: 优化程度最高，编译较慢
    O2,
}

impl OptimizationLevel {
    /// 获取优化级别对应的 LLVM 字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            OptimizationLevel::O0 => "0",
            OptimizationLevel::O1 => "1",
            OptimizationLevel::O2 => "2",
        }
    }

    /// 解析优化级别
    pub fn from_str(s: &str) -> LiftResult<Self> {
        match s {
            "0" | "O0" => Ok(OptimizationLevel::O0),
            "1" | "O1" => Ok(OptimizationLevel::O1),
            "2" | "O2" => Ok(OptimizationLevel::O2),
            _ => Err(LiftError::IRGenError(format!("Invalid optimization level: {}", s)).into()),
        }
    }
}

/// LLVM Pass 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LLVMPass {
    /// 常数折叠
    ConstantFolding,
    /// 死代码消除
    DeadCodeElimination,
    /// 指令合并
    InstructionCombining,
    /// CFG 简化
    CFGSimplification,
    /// 循环不变式外提
    LoopInvariantCodeMotion,
    /// 归纳变量消除
    InductionVariableElimination,
    /// 函数内联
    FunctionInlining,
    /// 内存优化
    MemoryOptimization,
    /// 向量化
    Vectorization,
}

impl LLVMPass {
    /// 获取 Pass 的 LLVM 名称
    pub fn llvm_name(&self) -> &'static str {
        match self {
            LLVMPass::ConstantFolding => "-constprop",
            LLVMPass::DeadCodeElimination => "-dce",
            LLVMPass::InstructionCombining => "-instcombine",
            LLVMPass::CFGSimplification => "-simplifycfg",
            LLVMPass::LoopInvariantCodeMotion => "-licm",
            LLVMPass::InductionVariableElimination => "-indvars",
            LLVMPass::FunctionInlining => "-inline",
            LLVMPass::MemoryOptimization => "-memdep",
            LLVMPass::Vectorization => "-loop-vectorize",
        }
    }

    /// 获取 Pass 的描述
    pub fn description(&self) -> &'static str {
        match self {
            LLVMPass::ConstantFolding => "Constant Propagation and Folding",
            LLVMPass::DeadCodeElimination => "Dead Code Elimination",
            LLVMPass::InstructionCombining => "Instruction Combining",
            LLVMPass::CFGSimplification => "Control Flow Graph Simplification",
            LLVMPass::LoopInvariantCodeMotion => "Loop Invariant Code Motion",
            LLVMPass::InductionVariableElimination => "Induction Variable Elimination",
            LLVMPass::FunctionInlining => "Function Inlining",
            LLVMPass::MemoryOptimization => "Memory Optimization",
            LLVMPass::Vectorization => "Loop Vectorization",
        }
    }
}

/// PassManager 配置
pub struct PassManager {
    optimization_level: OptimizationLevel,
    passes: Vec<LLVMPass>,
}

impl PassManager {
    /// 创建新的 PassManager
    pub fn new(level: OptimizationLevel) -> Self {
        let passes = Self::get_passes_for_level(level);
        Self {
            optimization_level: level,
            passes,
        }
    }

    /// 根据优化级别选择 Pass
    fn get_passes_for_level(level: OptimizationLevel) -> Vec<LLVMPass> {
        match level {
            OptimizationLevel::O0 => {
                // 快速编译：仅使用最基础的 Pass
                vec![LLVMPass::ConstantFolding, LLVMPass::DeadCodeElimination]
            }
            OptimizationLevel::O1 => {
                // 平衡优化
                vec![
                    LLVMPass::ConstantFolding,
                    LLVMPass::DeadCodeElimination,
                    LLVMPass::InstructionCombining,
                    LLVMPass::CFGSimplification,
                ]
            }
            OptimizationLevel::O2 => {
                // 最大优化
                vec![
                    LLVMPass::ConstantFolding,
                    LLVMPass::DeadCodeElimination,
                    LLVMPass::InstructionCombining,
                    LLVMPass::CFGSimplification,
                    LLVMPass::LoopInvariantCodeMotion,
                    LLVMPass::InductionVariableElimination,
                    LLVMPass::FunctionInlining,
                    LLVMPass::MemoryOptimization,
                    LLVMPass::Vectorization,
                ]
            }
        }
    }

    /// 获取优化级别
    pub fn optimization_level(&self) -> OptimizationLevel {
        self.optimization_level
    }

    /// 获取所有 Pass
    pub fn passes(&self) -> &[LLVMPass] {
        &self.passes
    }

    /// 添加 Pass
    pub fn add_pass(&mut self, pass: LLVMPass) {
        if !self.passes.contains(&pass) {
            self.passes.push(pass);
        }
    }

    /// 移除 Pass
    pub fn remove_pass(&mut self, pass: LLVMPass) {
        self.passes.retain(|&p| p != pass);
    }

    /// 清空所有 Pass
    pub fn clear_passes(&mut self) {
        self.passes.clear();
    }

    /// 生成 opt 命令行参数
    pub fn to_opt_args(&self) -> Vec<String> {
        let mut args = vec!["-O".to_string() + self.optimization_level.as_str()];
        for pass in &self.passes {
            args.push(pass.llvm_name().to_string());
        }
        args
    }

    /// 生成优化管线描述
    pub fn describe(&self) -> String {
        let mut desc = format!("PassManager (OptLevel: {:?})\n", self.optimization_level);
        for (idx, pass) in self.passes.iter().enumerate() {
            desc.push_str(&format!(
                "  [{}] {}: {}\n",
                idx + 1,
                pass.llvm_name(),
                pass.description()
            ));
        }
        desc
    }
}

impl Default for PassManager {
    fn default() -> Self {
        Self::new(OptimizationLevel::O1)
    }
}

/// 优化预设配置
pub struct OptimizationPreset;

impl OptimizationPreset {
    /// 快速 JIT 编译预设（O0）
    pub fn fast_jit() -> PassManager {
        PassManager::new(OptimizationLevel::O0)
    }

    /// 平衡 JIT 编译预设（O1）
    pub fn balanced_jit() -> PassManager {
        PassManager::new(OptimizationLevel::O1)
    }

    /// 最大优化 AOT 编译预设（O2）
    pub fn aggressive_aot() -> PassManager {
        PassManager::new(OptimizationLevel::O2)
    }

    /// 自定义预设：仅执行关键优化
    pub fn critical_only() -> PassManager {
        let mut pm = PassManager::new(OptimizationLevel::O0);
        pm.add_pass(LLVMPass::ConstantFolding);
        pm.add_pass(LLVMPass::DeadCodeElimination);
        pm.add_pass(LLVMPass::CFGSimplification);
        pm
    }
}

/// 优化统计信息
#[derive(Debug, Clone, Default)]
pub struct OptimizationStats {
    /// 常数折叠的指令数
    pub constants_folded: usize,
    /// 消除的死代码指令数
    pub dead_instrs_removed: usize,
    /// 合并的指令数
    pub instrs_combined: usize,
    /// 移除的基本块数
    pub blocks_removed: usize,
    /// 优化前的 IR 大小（字节）
    pub ir_size_before: usize,
    /// 优化后的 IR 大小（字节）
    pub ir_size_after: usize,
    /// 优化耗时（微秒）
    pub optimization_time_us: u64,
}

impl OptimizationStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// 计算压缩比
    pub fn compression_ratio(&self) -> f64 {
        if self.ir_size_before == 0 {
            1.0
        } else {
            self.ir_size_after as f64 / self.ir_size_before as f64
        }
    }

    /// 生成统计报告
    pub fn report(&self) -> String {
        format!(
            r#"Optimization Statistics:
  - Constants Folded:  {}
  - Dead Code Removed: {}
  - Instructions Combined: {}
  - Basic Blocks Removed: {}
  - IR Size: {} → {} bytes ({:.1}%)
  - Time: {:.3} ms
"#,
            self.constants_folded,
            self.dead_instrs_removed,
            self.instrs_combined,
            self.blocks_removed,
            self.ir_size_before,
            self.ir_size_after,
            self.compression_ratio() * 100.0,
            self.optimization_time_us as f64 / 1000.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimization_level_parsing() {
        assert_eq!(
            OptimizationLevel::from_str("O0").unwrap(),
            OptimizationLevel::O0
        );
        assert_eq!(
            OptimizationLevel::from_str("1").unwrap(),
            OptimizationLevel::O1
        );
        assert_eq!(
            OptimizationLevel::from_str("2").unwrap(),
            OptimizationLevel::O2
        );
    }

    #[test]
    fn test_pass_manager_o0() {
        let pm = PassManager::new(OptimizationLevel::O0);
        assert_eq!(pm.passes().len(), 2);
    }

    #[test]
    fn test_pass_manager_o1() {
        let pm = PassManager::new(OptimizationLevel::O1);
        assert_eq!(pm.passes().len(), 4);
    }

    #[test]
    fn test_pass_manager_o2() {
        let pm = PassManager::new(OptimizationLevel::O2);
        assert!(pm.passes().len() > 4);
    }

    #[test]
    fn test_pass_manager_add_pass() {
        let mut pm = PassManager::new(OptimizationLevel::O0);
        let initial_count = pm.passes().len();
        pm.add_pass(LLVMPass::Vectorization);
        assert_eq!(pm.passes().len(), initial_count + 1);
    }

    #[test]
    fn test_optimization_preset_fast_jit() {
        let pm = OptimizationPreset::fast_jit();
        assert_eq!(pm.optimization_level(), OptimizationLevel::O0);
    }

    #[test]
    fn test_optimization_stats() {
        let stats = OptimizationStats {
            constants_folded: 10,
            dead_instrs_removed: 5,
            ir_size_before: 1000,
            ir_size_after: 800,
            optimization_time_us: 1500,
            ..Default::default()
        };
        let report = stats.report();
        assert!(report.contains("10"));
        assert!(report.contains("Optimization Statistics"));
    }

    #[test]
    fn test_pass_manager_to_opt_args() {
        let pm = PassManager::new(OptimizationLevel::O1);
        let args = pm.to_opt_args();
        assert!(!args.is_empty());
        assert!(args[0].contains("-O"));
    }
}
