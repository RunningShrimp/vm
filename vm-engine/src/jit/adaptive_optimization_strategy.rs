//! 自适应优化策略
//!
//! 本模块实现智能的自适应优化策略，根据运行时性能数据和程序行为
//! 动态选择最适合的优化技术组合，最大化JIT编译的性能收益。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vm_core::{GuestAddr, VmError};
use vm_ir::IRBlock;
use crate::jit::simd_optimizer::SIMDOptimizer;
use serde::{Serialize, Deserialize};
use crate::jit::core::JITEngine;
use crate::jit::optimizer::IROptimizer;
use crate::jit::simd_optimizer::DefaultSIMDOptimizer;
use crate::jit::advanced_optimizer::AdvancedOptimizer;
use crate::jit::adaptive_threshold::{PerformanceMetrics, AdaptiveCompilationConfig};

/// 优化策略类型
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OptimizationStrategy {
    /// 无优化
    None,
    /// 基础优化
    Basic,
    /// 中级优化
    Intermediate,
    /// 高级优化
    Advanced,
    /// 激进优化
    Aggressive,
    /// 自适应选择
    Adaptive,
}

/// 优化技术组合
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationTechniqueSet {
    /// 是否启用常量折叠
    pub enable_constant_folding: bool,
    /// 是否启用死代码消除
    pub enable_dead_code_elimination: bool,
    /// 是否启用公共子表达式消除
    pub enable_common_subexpression_elimination: bool,
    /// 是否启用循环优化
    pub enable_loop_optimization: bool,
    /// 是否启用内联优化
    pub enable_inlining: bool,
    /// 是否启用SIMD向量化
    pub enable_simd_vectorization: bool,
    /// 是否启用指令调度
    pub enable_instruction_scheduling: bool,
    /// 是否启用寄存器重命名
    pub enable_register_renaming: bool,
    /// 是否启用强度削减
    pub enable_strength_reduction: bool,
    /// 是否启用窥孔优化
    pub enable_peephole_optimization: bool,
}

impl Default for OptimizationTechniqueSet {
    fn default() -> Self {
        Self {
            enable_constant_folding: true,
            enable_dead_code_elimination: true,
            enable_common_subexpression_elimination: false,
            enable_loop_optimization: false,
            enable_inlining: false,
            enable_simd_vectorization: false,
            enable_instruction_scheduling: false,
            enable_register_renaming: false,
            enable_strength_reduction: false,
            enable_peephole_optimization: false,
        }
    }
}

/// 预定义优化技术组合
impl OptimizationTechniqueSet {
    /// 基础优化组合
    pub fn basic() -> Self {
        Self {
            enable_constant_folding: true,
            enable_dead_code_elimination: true,
            enable_common_subexpression_elimination: false,
            enable_loop_optimization: false,
            enable_inlining: false,
            enable_simd_vectorization: false,
            enable_instruction_scheduling: false,
            enable_register_renaming: false,
            enable_strength_reduction: false,
            enable_peephole_optimization: true,
        }
    }

    /// 中级优化组合
    pub fn intermediate() -> Self {
        Self {
            enable_constant_folding: true,
            enable_dead_code_elimination: true,
            enable_common_subexpression_elimination: true,
            enable_loop_optimization: true,
            enable_inlining: false,
            enable_simd_vectorization: false,
            enable_instruction_scheduling: true,
            enable_register_renaming: false,
            enable_strength_reduction: true,
            enable_peephole_optimization: true,
        }
    }

    /// 高级优化组合
    pub fn advanced() -> Self {
        Self {
            enable_constant_folding: true,
            enable_dead_code_elimination: true,
            enable_common_subexpression_elimination: true,
            enable_loop_optimization: true,
            enable_inlining: true,
            enable_simd_vectorization: true,
            enable_instruction_scheduling: true,
            enable_register_renaming: true,
            enable_strength_reduction: true,
            enable_peephole_optimization: true,
        }
    }

    /// 激进优化组合
    pub fn aggressive() -> Self {
        Self {
            enable_constant_folding: true,
            enable_dead_code_elimination: true,
            enable_common_subexpression_elimination: true,
            enable_loop_optimization: true,
            enable_inlining: true,
            enable_simd_vectorization: true,
            enable_instruction_scheduling: true,
            enable_register_renaming: true,
            enable_strength_reduction: true,
            enable_peephole_optimization: true,
        }
    }
}

/// 优化策略评估结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyEvaluation {
    /// 策略类型
    pub strategy: OptimizationStrategy,
    /// 预期性能提升
    pub expected_performance_gain: f64,
    /// 预期编译时间
    pub expected_compilation_time: Duration,
    /// 预期内存开销
    pub expected_memory_overhead: u64,
    /// 适用性评分
    pub applicability_score: f64,
    /// 风险评分
    pub risk_score: f64,
}

/// 自适应优化策略管理器
pub struct AdaptiveOptimizationStrategyManager {
    /// JIT引擎引用
    jit_engine: Arc<Mutex<JITEngine>>,
    /// 配置
    config: AdaptiveCompilationConfig,
    /// 当前策略
    current_strategy: OptimizationStrategy,
    /// 当前技术组合
    current_techniques: OptimizationTechniqueSet,
    /// 策略历史
    strategy_history: VecDeque<StrategyEvaluation>,
    /// 性能历史
    performance_history: VecDeque<PerformanceMetrics>,
    /// 基础优化器
    base_optimizer: crate::jit::optimizer::DefaultIROptimizer,
    /// SIMD优化器
    simd_optimizer: DefaultSIMDOptimizer,
    /// 高级优化器
    advanced_optimizer: crate::advanced_optimizer::AdvancedOptimizer,
}

impl AdaptiveOptimizationStrategyManager {
    /// 创建新的自适应优化策略管理器
    pub fn new(
        jit_engine: Arc<Mutex<JITEngine>>,
        config: AdaptiveCompilationConfig,
    ) -> Self {
        Self {
            jit_engine,
            config,
            current_strategy: OptimizationStrategy::Basic,
            current_techniques: OptimizationTechniqueSet::basic(),
            strategy_history: VecDeque::with_capacity(100),
            performance_history: VecDeque::with_capacity(100),
            base_optimizer: crate::jit::optimizer::DefaultIROptimizer::new(crate::jit::core::JITConfig::default()),
            simd_optimizer: DefaultSIMDOptimizer::new(),
            advanced_optimizer: crate::advanced_optimizer::AdvancedOptimizer::new(crate::advanced_optimizer::AdvancedOptimizerConfig::default()),
        }
    }

    /// 分析IR块并选择最佳优化策略
    pub fn analyze_and_select_strategy(&mut self, ir_block: &IRBlock) -> Result<OptimizationStrategy, VmError> {
        // 分析IR块特征
        let characteristics = self.analyze_ir_characteristics(ir_block);
        
        // 评估所有可能的策略
        let evaluations = self.evaluate_strategies(&characteristics)?;
        
        // 选择最佳策略
        let best_strategy = self.select_best_strategy(&evaluations);
        
        // 更新当前策略
        self.current_strategy = best_strategy;
        self.current_techniques = self.get_techniques_for_strategy(best_strategy);
        
        Ok(best_strategy)
    }

    /// 分析IR块特征
    fn analyze_ir_characteristics(&self, ir_block: &IRBlock) -> IRCharacteristics {
        let mut characteristics = IRCharacteristics::default();
        
        // 统计指令类型
        for op in &ir_block.ops {
            match op {
                vm_ir::IROp::Add { .. } | vm_ir::IROp::Sub { .. } | vm_ir::IROp::Mul { .. } | vm_ir::IROp::Div { .. } => {
                    characteristics.arithmetic_instruction_count += 1;
                }
                vm_ir::IROp::Load { .. } | vm_ir::IROp::Store { .. } => {
                    characteristics.memory_instruction_count += 1;
                }
                vm_ir::IROp::VecAdd { .. } | vm_ir::IROp::VecSub { .. } | vm_ir::IROp::VecMul { .. } => {
                    characteristics.vector_instruction_count += 1;
                }
                // Call instructions are in Terminator, not IROp
                // Jmp instructions are in Terminator, not IROp
                _ => {
                    characteristics.other_instruction_count += 1;
                }
            }
        }
        
        // 计算指令密度
        characteristics.instruction_count = ir_block.ops.len();
        characteristics.instruction_density = characteristics.instruction_count as f64 / ir_block.ops.len() as f64;
        
        // 检测循环模式
        characteristics.has_loop = self.detect_loop_pattern(ir_block);
        
        // 检测向量化机会
        characteristics.vectorization_potential = self.estimate_vectorization_potential(ir_block);
        
        // 检测内联机会
        characteristics.inlining_potential = self.estimate_inlining_potential(ir_block);
        
        characteristics
    }

    /// 评估所有可能的策略
    fn evaluate_strategies(&self, characteristics: &IRCharacteristics) -> Result<Vec<StrategyEvaluation>, VmError> {
        let mut evaluations = Vec::new();
        
        // 评估基础优化策略
        evaluations.push(self.evaluate_basic_strategy(characteristics));
        
        // 评估中级优化策略
        evaluations.push(self.evaluate_intermediate_strategy(characteristics));
        
        // 评估高级优化策略
        evaluations.push(self.evaluate_advanced_strategy(characteristics));
        
        // 评估激进优化策略
        evaluations.push(self.evaluate_aggressive_strategy(characteristics));
        
        Ok(evaluations)
    }

    /// 评估基础优化策略
    fn evaluate_basic_strategy(&self, characteristics: &IRCharacteristics) -> StrategyEvaluation {
        let expected_performance_gain = 1.1; // 10%提升
        let expected_compilation_time = Duration::from_micros(100);
        let expected_memory_overhead = 1024 * 100; // 100KB
        let applicability_score = 1.0; // 总是适用
        let risk_score = 0.1; // 低风险
        
        StrategyEvaluation {
            strategy: OptimizationStrategy::Basic,
            expected_performance_gain,
            expected_compilation_time,
            expected_memory_overhead,
            applicability_score,
            risk_score,
        }
    }

    /// 评估中级优化策略
    fn evaluate_intermediate_strategy(&self, characteristics: &IRCharacteristics) -> StrategyEvaluation {
        let mut expected_performance_gain = 1.2; // 20%提升
        let expected_compilation_time = Duration::from_micros(500);
        let expected_memory_overhead = 1024 * 200; // 200KB
        let mut applicability_score = 0.8;
        let risk_score = 0.3;
        
        // 根据IR特征调整评分
        if characteristics.has_loop {
            expected_performance_gain += 0.1;
            applicability_score += 0.1;
        }
        
        if characteristics.memory_instruction_count > characteristics.arithmetic_instruction_count {
            expected_performance_gain += 0.05;
        }
        
        StrategyEvaluation {
            strategy: OptimizationStrategy::Intermediate,
            expected_performance_gain,
            expected_compilation_time,
            expected_memory_overhead,
            applicability_score,
            risk_score,
        }
    }

    /// 评估高级优化策略
    fn evaluate_advanced_strategy(&self, characteristics: &IRCharacteristics) -> StrategyEvaluation {
        let mut expected_performance_gain = 1.4; // 40%提升
        let expected_compilation_time = Duration::from_millis(2);
        let expected_memory_overhead = 1024 * 500; // 500KB
        let mut applicability_score = 0.6;
        let risk_score = 0.5;
        
        // 根据IR特征调整评分
        if characteristics.vectorization_potential > 0.5 {
            expected_performance_gain += 0.2;
            applicability_score += 0.2;
        }
        
        if characteristics.has_loop {
            expected_performance_gain += 0.1;
            applicability_score += 0.1;
        }
        
        if characteristics.inlining_potential > 0.3 {
            expected_performance_gain += 0.1;
            applicability_score += 0.1;
        }
        
        StrategyEvaluation {
            strategy: OptimizationStrategy::Advanced,
            expected_performance_gain,
            expected_compilation_time,
            expected_memory_overhead,
            applicability_score,
            risk_score,
        }
    }

    /// 评估激进优化策略
    fn evaluate_aggressive_strategy(&self, characteristics: &IRCharacteristics) -> StrategyEvaluation {
        let mut expected_performance_gain = 1.6; // 60%提升
        let expected_compilation_time = Duration::from_millis(5);
        let expected_memory_overhead = 1024 * 1024; // 1MB
        let mut applicability_score = 0.3;
        let risk_score = 0.8;
        
        // 根据IR特征调整评分
        if characteristics.vectorization_potential > 0.8 {
            expected_performance_gain += 0.2;
            applicability_score += 0.2;
        }
        
        if characteristics.instruction_count > 100 {
            expected_performance_gain += 0.1;
            applicability_score += 0.1;
        }
        
        if characteristics.has_loop && characteristics.instruction_count > 50 {
            expected_performance_gain += 0.1;
            applicability_score += 0.1;
        }
        
        StrategyEvaluation {
            strategy: OptimizationStrategy::Aggressive,
            expected_performance_gain,
            expected_compilation_time,
            expected_memory_overhead,
            applicability_score,
            risk_score,
        }
    }

    /// 选择最佳策略
    fn select_best_strategy(&self, evaluations: &[StrategyEvaluation]) -> OptimizationStrategy {
        let mut best_strategy = OptimizationStrategy::Basic;
        let mut best_score = 0.0;
        
        for evaluation in evaluations {
            // 计算综合评分
            let score = evaluation.applicability_score * evaluation.expected_performance_gain 
                      - evaluation.risk_score * 0.2 
                      - (evaluation.expected_compilation_time.as_micros() as f64 / 10000.0) * 0.1;
            
            if score > best_score {
                best_score = score;
                best_strategy = evaluation.strategy;
            }
        }
        
        best_strategy
    }

    /// 获取策略对应的优化技术组合
    fn get_techniques_for_strategy(&self, strategy: OptimizationStrategy) -> OptimizationTechniqueSet {
        match strategy {
            OptimizationStrategy::None => OptimizationTechniqueSet::default(),
            OptimizationStrategy::Basic => OptimizationTechniqueSet::basic(),
            OptimizationStrategy::Intermediate => OptimizationTechniqueSet::intermediate(),
            OptimizationStrategy::Advanced => OptimizationTechniqueSet::advanced(),
            OptimizationStrategy::Aggressive => OptimizationTechniqueSet::aggressive(),
            OptimizationStrategy::Adaptive => {
                // 自适应选择最佳技术组合
                self.select_adaptive_techniques()
            }
        }
    }

    /// 自适应选择优化技术组合
    fn select_adaptive_techniques(&self) -> OptimizationTechniqueSet {
        // 基于历史性能数据选择最佳技术组合
        let mut techniques = OptimizationTechniqueSet::default();
        
        // 分析历史数据，找出最有效的技术
        if let Some(latest_metrics) = self.performance_history.back() {
            if latest_metrics.execution_speed > 2000.0 {
                // 高执行速度，启用更多优化
                techniques = OptimizationTechniqueSet::intermediate();
            }
            
            if latest_metrics.compilation_benefit > 1.5 {
                // 高编译收益，启用高级优化
                techniques = OptimizationTechniqueSet::advanced();
            }
            
            if latest_metrics.cache_hit_rate < 0.5 {
                // 低缓存命中率，启用缓存优化
                techniques.enable_peephole_optimization = true;
            }
            
            if latest_metrics.memory_usage > 1024 * 1024 {
                // 高内存使用，启用内存优化
                techniques.enable_dead_code_elimination = true;
            }
        }
        
        techniques
    }

    /// 应用当前优化策略
    pub fn apply_optimization_strategy(&mut self, ir_block: &mut IRBlock) -> Result<(), VmError> {
        let techniques = &self.current_techniques;
        
        // 应用常量折叠
        if techniques.enable_constant_folding {
            self.base_optimizer.optimize(ir_block)?;
        }
        
        // 应用死代码消除
        if techniques.enable_dead_code_elimination {
            self.base_optimizer.optimize(ir_block)?;
        }
        
        // 应用公共子表达式消除
        if techniques.enable_common_subexpression_elimination {
            self.base_optimizer.optimize(ir_block)?;
        }
        
        // 应用循环优化
        if techniques.enable_loop_optimization {
            self.advanced_optimizer.optimize(ir_block)?;
        }
        
        // 应用内联优化
        if techniques.enable_inlining {
            self.advanced_optimizer.optimize(ir_block)?;
        }
        
        // 应用SIMD向量化
        if techniques.enable_simd_vectorization {
            self.simd_optimizer.optimize_simd(ir_block)?;
        }
        
        // 应用指令调度
        if techniques.enable_instruction_scheduling {
            self.advanced_optimizer.optimize(ir_block)?;
        }
        
        // 应用寄存器重命名
        if techniques.enable_register_renaming {
            self.advanced_optimizer.optimize(ir_block)?;
        }
        
        // 应用强度削减
        if techniques.enable_strength_reduction {
            self.advanced_optimizer.optimize(ir_block)?;
        }
        
        // 应用窥孔优化
        if techniques.enable_peephole_optimization {
            self.base_optimizer.optimize(ir_block)?;
        }
        
        Ok(())
    }

    /// 更新性能指标
    pub fn update_performance_metrics(&mut self, metrics: PerformanceMetrics) {
        // 添加到历史记录
        self.performance_history.push_back(metrics.clone());
        
        // 限制历史记录大小
        if self.performance_history.len() > self.config.history_size {
            self.performance_history.pop_front();
        }
        
        // 评估当前策略效果
        self.evaluate_current_strategy_effectiveness(&metrics.clone());
    }

    /// 评估当前策略效果
    fn evaluate_current_strategy_effectiveness(&mut self, metrics: &PerformanceMetrics) {
        // 创建策略评估记录
        let evaluation = StrategyEvaluation {
            strategy: self.current_strategy,
            expected_performance_gain: metrics.compilation_benefit,
            expected_compilation_time: metrics.compilation_time,
            expected_memory_overhead: metrics.memory_usage,
            applicability_score: metrics.cache_hit_rate,
            risk_score: 1.0 - metrics.execution_speed / 10000.0,
        };
        
        // 添加到历史记录
        self.strategy_history.push_back(evaluation);
        
        // 限制历史记录大小
        if self.strategy_history.len() > 100 {
            self.strategy_history.pop_front();
        }
    }

    /// 检测循环模式
    fn detect_loop_pattern(&self, ir_block: &IRBlock) -> bool {
        // 简化的循环检测：查找向后跳转
        for op in &ir_block.ops {
            // Check for backward branches using Beq, Bne, Blt, Bge, Bltu, Bgeu
            match op {
                vm_ir::IROp::Beq { target, .. } |
                vm_ir::IROp::Bne { target, .. } |
                vm_ir::IROp::Blt { target, .. } |
                vm_ir::IROp::Bge { target, .. } |
                vm_ir::IROp::Bltu { target, .. } |
                vm_ir::IROp::Bgeu { target, .. } => {
                    if *target < ir_block.start_pc {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// 估计向量化潜力
    fn estimate_vectorization_potential(&self, ir_block: &IRBlock) -> f64 {
        let mut vectorizable_ops = 0;
        let total_ops = ir_block.ops.len();
        
        if total_ops == 0 {
            return 0.0;
        }
        
        // 统计可向量化的操作
        for op in &ir_block.ops {
            match op {
                vm_ir::IROp::Add { .. } | vm_ir::IROp::Sub { .. } | vm_ir::IROp::Mul { .. } => {
                    vectorizable_ops += 1;
                }
                vm_ir::IROp::Load { .. } | vm_ir::IROp::Store { .. } => {
                    vectorizable_ops += 1;
                }
                _ => {}
            }
        }
        
        vectorizable_ops as f64 / total_ops as f64
    }

    /// 估计内联潜力
    fn estimate_inlining_potential(&self, ir_block: &IRBlock) -> f64 {
        let mut call_ops = 0;
        let total_ops = ir_block.ops.len();
        
        if total_ops == 0 {
            return 0.0;
        }
        
        // 统计函数调用操作 - check Terminator for Call
        if let vm_ir::Terminator::Call { .. } = &ir_block.term {
            call_ops += 1;
        }
        
        if call_ops == 0 {
            return 0.0;
        }
        
        // 简化计算：调用比例
        call_ops as f64 / total_ops as f64
    }

    /// 获取当前策略
    pub fn current_strategy(&self) -> OptimizationStrategy {
        self.current_strategy
    }

    /// 获取当前技术组合
    pub fn current_techniques(&self) -> &OptimizationTechniqueSet {
        &self.current_techniques
    }

    /// 获取策略历史
    pub fn strategy_history(&self) -> &VecDeque<StrategyEvaluation> {
        &self.strategy_history
    }

    /// 获取性能历史
    pub fn performance_history(&self) -> &VecDeque<PerformanceMetrics> {
        &self.performance_history
    }
}

/// IR块特征
#[derive(Debug, Clone, Default)]
struct IRCharacteristics {
    /// 指令总数
    instruction_count: usize,
    /// 算术指令数量
    arithmetic_instruction_count: usize,
    /// 内存指令数量
    memory_instruction_count: usize,
    /// 向量指令数量
    vector_instruction_count: usize,
    /// 调用指令数量
    call_instruction_count: usize,
    /// 分支指令数量
    branch_instruction_count: usize,
    /// 其他指令数量
    other_instruction_count: usize,
    /// 指令密度
    instruction_density: f64,
    /// 是否包含循环
    has_loop: bool,
    /// 向量化潜力
    vectorization_potential: f64,
    /// 内联潜力
    inlining_potential: f64,
}