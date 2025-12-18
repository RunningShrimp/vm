//! Optimization bounded context
//! 
//! This module defines the optimization domain, including optimization strategies,
//! passes, and analysis tools for JIT compiled code.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::common::{Config, Stats, JITErrorBuilder};
use vm_error::VmError;
use crate::ir::{IRBlock, IRInstruction, Operand};

/// Unique identifier for optimization contexts
pub type OptimizationId = u64;

/// Optimization level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptimizationLevel {
    /// No optimization
    None,
    /// Basic optimizations only
    Basic,
    /// Standard optimizations
    Standard,
    /// Aggressive optimizations
    Aggressive,
    /// Maximum optimizations
    Maximum,
}

impl Default for OptimizationLevel {
    fn default() -> Self {
        OptimizationLevel::Standard
    }
}

impl std::fmt::Display for OptimizationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptimizationLevel::None => write!(f, "None"),
            OptimizationLevel::Basic => write!(f, "Basic"),
            OptimizationLevel::Standard => write!(f, "Standard"),
            OptimizationLevel::Aggressive => write!(f, "Aggressive"),
            OptimizationLevel::Maximum => write!(f, "Maximum"),
        }
    }
}

/// Optimization pass type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptimizationPassType {
    /// Dead code elimination
    DeadCodeElimination,
    /// Constant folding
    ConstantFolding,
    /// Common subexpression elimination
    CommonSubexpressionElimination,
    /// Loop invariant code motion
    LoopInvariantCodeMotion,
    /// Instruction combining
    InstructionCombining,
    /// Register allocation
    RegisterAllocation,
    /// Instruction scheduling
    InstructionScheduling,
    /// Memory optimization
    MemoryOptimization,
    /// Vectorization
    Vectorization,
    /// Inline expansion
    InlineExpansion,
    /// Tail call optimization
    TailCallOptimization,
    /// Peephole optimization
    PeepholeOptimization,
    /// Custom optimization
    Custom,
}

impl std::fmt::Display for OptimizationPassType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptimizationPassType::DeadCodeElimination => write!(f, "DeadCodeElimination"),
            OptimizationPassType::ConstantFolding => write!(f, "ConstantFolding"),
            OptimizationPassType::CommonSubexpressionElimination => write!(f, "CommonSubexpressionElimination"),
            OptimizationPassType::LoopInvariantCodeMotion => write!(f, "LoopInvariantCodeMotion"),
            OptimizationPassType::InstructionCombining => write!(f, "InstructionCombining"),
            OptimizationPassType::RegisterAllocation => write!(f, "RegisterAllocation"),
            OptimizationPassType::InstructionScheduling => write!(f, "InstructionScheduling"),
            OptimizationPassType::MemoryOptimization => write!(f, "MemoryOptimization"),
            OptimizationPassType::Vectorization => write!(f, "Vectorization"),
            OptimizationPassType::InlineExpansion => write!(f, "InlineExpansion"),
            OptimizationPassType::TailCallOptimization => write!(f, "TailCallOptimization"),
            OptimizationPassType::PeepholeOptimization => write!(f, "PeepholeOptimization"),
            OptimizationPassType::Custom => write!(f, "Custom"),
        }
    }
}

/// Optimization status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptimizationStatus {
    /// Not started
    NotStarted,
    /// In progress
    InProgress,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed,
    /// Skipped
    Skipped,
}

impl Default for OptimizationStatus {
    fn default() -> Self {
        OptimizationStatus::NotStarted
    }
}

impl std::fmt::Display for OptimizationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptimizationStatus::NotStarted => write!(f, "NotStarted"),
            OptimizationStatus::InProgress => write!(f, "InProgress"),
            OptimizationStatus::Completed => write!(f, "Completed"),
            OptimizationStatus::Failed => write!(f, "Failed"),
            OptimizationStatus::Skipped => write!(f, "Skipped"),
        }
    }
}

/// Optimization configuration
#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    /// Optimization level
    pub level: OptimizationLevel,
    /// Enable specific passes
    pub enabled_passes: Vec<OptimizationPassType>,
    /// Disable specific passes
    pub disabled_passes: Vec<OptimizationPassType>,
    /// Maximum optimization time
    pub max_time: Duration,
    /// Enable parallel optimization
    pub parallel: bool,
    /// Optimization threshold
    pub threshold: f32,
    /// Enable profiling feedback
    pub enable_feedback: bool,
    /// Custom optimization parameters
    pub custom_params: HashMap<String, String>,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            level: OptimizationLevel::Standard,
            enabled_passes: Vec::new(),
            disabled_passes: Vec::new(),
            max_time: Duration::from_secs(30),
            parallel: true,
            threshold: 0.1,
            enable_feedback: false,
            custom_params: HashMap::new(),
        }
    }
}

impl Config for OptimizationConfig {
    fn validate(&self) -> Result<(), String> {
        if self.threshold < 0.0 || self.threshold > 1.0 {
            return Err("Optimization threshold must be between 0.0 and 1.0".to_string());
        }
        
        if self.max_time.is_zero() {
            return Err("Maximum optimization time cannot be zero".to_string());
        }
        
        Ok(())
    }
    
    fn summary(&self) -> String {
        format!(
            "OptimizationConfig(level={}, parallel={}, max_time={:?}, threshold={})",
            self.level, self.parallel, self.max_time, self.threshold
        )
    }
    
    fn merge(&mut self, other: &Self) {
        // Use the higher optimization level
        if other.level as u8 > self.level as u8 {
            self.level = other.level;
        }
        
        // Merge enabled passes
        for pass in &other.enabled_passes {
            if !self.enabled_passes.contains(pass) {
                self.enabled_passes.push(*pass);
            }
        }
        
        // Merge disabled passes
        for pass in &other.disabled_passes {
            if !self.disabled_passes.contains(pass) {
                self.disabled_passes.push(*pass);
            }
        }
        
        // Use the shorter max time
        if other.max_time < self.max_time {
            self.max_time = other.max_time;
        }
        
        // Merge parallel settings
        self.parallel = self.parallel && other.parallel;
        
        // Use the higher threshold
        if other.threshold > self.threshold {
            self.threshold = other.threshold;
        }
        
        // Merge feedback settings
        self.enable_feedback = self.enable_feedback || other.enable_feedback;
        
        // Merge custom parameters
        for (key, value) in &other.custom_params {
            self.custom_params.insert(key.clone(), value.clone());
        }
    }
}

/// Optimization pass result
#[derive(Debug, Clone)]
pub struct OptimizationPassResult {
    /// Pass type
    pub pass_type: OptimizationPassType,
    /// Pass status
    pub status: OptimizationStatus,
    /// Execution time
    pub execution_time: Duration,
    /// Number of instructions before optimization
    pub instructions_before: usize,
    /// Number of instructions after optimization
    pub instructions_after: usize,
    /// Number of optimizations performed
    pub optimizations_performed: usize,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Additional metrics
    pub metrics: HashMap<String, f64>,
}

impl Default for OptimizationPassResult {
    fn default() -> Self {
        Self {
            pass_type: OptimizationPassType::Custom,
            status: OptimizationStatus::NotStarted,
            execution_time: Duration::default(),
            instructions_before: 0,
            instructions_after: 0,
            optimizations_performed: 0,
            error_message: None,
            metrics: HashMap::new(),
        }
    }
}

/// Optimization result
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    /// Optimization ID
    pub optimization_id: OptimizationId,
    /// Original IR block
    pub original_block: IRBlock,
    /// Optimized IR block
    pub optimized_block: IRBlock,
    /// Optimization configuration
    pub config: OptimizationConfig,
    /// Pass results
    pub pass_results: Vec<OptimizationPassResult>,
    /// Total optimization time
    pub total_time: Duration,
    /// Overall optimization status
    pub status: OptimizationStatus,
    /// Performance improvement percentage
    pub performance_improvement: f32,
    /// Code size reduction percentage
    pub code_size_reduction: f32,
}

/// Optimization context
#[derive(Debug, Clone)]
pub struct OptimizationContext {
    /// Optimization ID
    pub optimization_id: OptimizationId,
    /// IR block to optimize
    pub ir_block: IRBlock,
    /// Optimization configuration
    pub config: OptimizationConfig,
    /// Current optimization status
    pub status: OptimizationStatus,
    /// Start time
    pub start_time: Instant,
    /// Pass results
    pub pass_results: Vec<OptimizationPassResult>,
    /// Current optimization level
    pub current_level: OptimizationLevel,
    /// Optimization metrics
    pub metrics: OptimizationMetrics,
}

impl OptimizationContext {
    /// Create a new optimization context
    pub fn new(ir_block: IRBlock, config: OptimizationConfig) -> Self {
        let optimization_id = generate_optimization_id();
        Self {
            optimization_id,
            ir_block,
            config,
            status: OptimizationStatus::NotStarted,
            start_time: Instant::now(),
            pass_results: Vec::new(),
            current_level: config.level,
            metrics: OptimizationMetrics::default(),
        }
    }
    
    /// Start optimization
    pub fn start(&mut self) {
        self.status = OptimizationStatus::InProgress;
        self.start_time = Instant::now();
    }
    
    /// Complete optimization
    pub fn complete(&mut self, optimized_block: IRBlock) -> OptimizationResult {
        self.status = OptimizationStatus::Completed;
        let total_time = self.start_time.elapsed();
        
        let performance_improvement = self.calculate_performance_improvement(&optimized_block);
        let code_size_reduction = self.calculate_code_size_reduction(&optimized_block);
        
        OptimizationResult {
            optimization_id: self.optimization_id,
            original_block: self.ir_block.clone(),
            optimized_block,
            config: self.config.clone(),
            pass_results: self.pass_results.clone(),
            total_time,
            status: self.status,
            performance_improvement,
            code_size_reduction,
        }
    }
    
    /// Fail optimization
    pub fn fail(&mut self, error_message: String) -> OptimizationResult {
        self.status = OptimizationStatus::Failed;
        let total_time = self.start_time.elapsed();
        
        OptimizationResult {
            optimization_id: self.optimization_id,
            original_block: self.ir_block.clone(),
            optimized_block: self.ir_block.clone(),
            config: self.config.clone(),
            pass_results: self.pass_results.clone(),
            total_time,
            status: self.status,
            performance_improvement: 0.0,
            code_size_reduction: 0.0,
        }
    }
    
    /// Add pass result
    pub fn add_pass_result(&mut self, result: OptimizationPassResult) {
        self.pass_results.push(result);
    }
    
    /// Calculate performance improvement
    fn calculate_performance_improvement(&self, optimized_block: &IRBlock) -> f32 {
        // Simple heuristic based on instruction count reduction
        let original_count = self.ir_block.instructions.len();
        let optimized_count = optimized_block.instructions.len();
        
        if original_count == 0 {
            return 0.0;
        }
        
        let reduction = (original_count - optimized_count) as f32 / original_count as f32;
        reduction * 100.0
    }
    
    /// Calculate code size reduction
    fn calculate_code_size_reduction(&self, optimized_block: &IRBlock) -> f32 {
        // Simple heuristic based on instruction count
        let original_count = self.ir_block.instructions.len();
        let optimized_count = optimized_block.instructions.len();
        
        if original_count == 0 {
            return 0.0;
        }
        
        let reduction = (original_count - optimized_count) as f32 / original_count as f32;
        reduction * 100.0
    }
}

/// Optimization metrics
#[derive(Debug, Clone, Default)]
pub struct OptimizationMetrics {
    /// Total optimizations performed
    pub total_optimizations: usize,
    /// Dead code eliminations
    pub dead_code_eliminations: usize,
    /// Constant foldings
    pub constant_foldings: usize,
    /// Common subexpression eliminations
    pub common_subexpression_eliminations: usize,
    /// Loop invariant code motions
    pub loop_invariant_code_motions: usize,
    /// Instruction combinations
    pub instruction_combinations: usize,
    /// Register allocations
    pub register_allocations: usize,
    /// Instruction schedulings
    pub instruction_schedulings: usize,
    /// Memory optimizations
    pub memory_optimizations: usize,
    /// Vectorizations
    pub vectorizations: usize,
    /// Inline expansions
    pub inline_expansions: usize,
    /// Tail call optimizations
    pub tail_call_optimizations: usize,
    /// Peephole optimizations
    pub peephole_optimizations: usize,
}

impl Stats for OptimizationMetrics {
    fn reset(&mut self) {
        *self = Self::default();
    }
    
    fn merge(&mut self, other: &Self) {
        self.total_optimizations += other.total_optimizations;
        self.dead_code_eliminations += other.dead_code_eliminations;
        self.constant_foldings += other.constant_foldings;
        self.common_subexpression_eliminations += other.common_subexpression_eliminations;
        self.loop_invariant_code_motions += other.loop_invariant_code_motions;
        self.instruction_combinations += other.instruction_combinations;
        self.register_allocations += other.register_allocations;
        self.instruction_schedulings += other.instruction_schedulings;
        self.memory_optimizations += other.memory_optimizations;
        self.vectorizations += other.vectorizations;
        self.inline_expansions += other.inline_expansions;
        self.tail_call_optimizations += other.tail_call_optimizations;
        self.peephole_optimizations += other.peephole_optimizations;
    }
    
    fn summary(&self) -> String {
        format!(
            "OptimizationMetrics(total={}, dead_code={}, constant_folding={}, cse={}, licm={}, \
             instruction_combining={}, register_alloc={}, scheduling={}, memory={}, vector={}, \
             inline={}, tail_call={}, peephole={})",
            self.total_optimizations,
            self.dead_code_eliminations,
            self.constant_foldings,
            self.common_subexpression_eliminations,
            self.loop_invariant_code_motions,
            self.instruction_combinations,
            self.register_allocations,
            self.instruction_schedulings,
            self.memory_optimizations,
            self.vectorizations,
            self.inline_expansions,
            self.tail_call_optimizations,
            self.peephole_optimizations
        )
    }
}

/// Optimization pass trait
pub trait OptimizationPass: Send + Sync {
    /// Get the pass type
    fn pass_type(&self) -> OptimizationPassType;
    
    /// Get the pass name
    fn name(&self) -> &str;
    
    /// Check if the pass should run for the given configuration
    fn should_run(&self, config: &OptimizationConfig) -> bool {
        // Check if explicitly disabled
        if config.disabled_passes.contains(&self.pass_type()) {
            return false;
        }
        
        // Check if explicitly enabled
        if !config.enabled_passes.is_empty() {
            return config.enabled_passes.contains(&self.pass_type());
        }
        
        // Check based on optimization level
        match config.level {
            OptimizationLevel::None => false,
            OptimizationLevel::Basic => self.is_basic_pass(),
            OptimizationLevel::Standard => self.is_standard_pass(),
            OptimizationLevel::Aggressive => self.is_aggressive_pass(),
            OptimizationLevel::Maximum => true,
        }
    }
    
    /// Check if this is a basic optimization pass
    fn is_basic_pass(&self) -> bool {
        matches!(
            self.pass_type(),
            OptimizationPassType::DeadCodeElimination |
            OptimizationPassType::ConstantFolding |
            OptimizationPassType::PeepholeOptimization
        )
    }
    
    /// Check if this is a standard optimization pass
    fn is_standard_pass(&self) -> bool {
        self.is_basic_pass() || matches!(
            self.pass_type(),
            OptimizationPassType::CommonSubexpressionElimination |
            OptimizationPassType::InstructionCombining |
            OptimizationPassType::RegisterAllocation
        )
    }
    
    /// Check if this is an aggressive optimization pass
    fn is_aggressive_pass(&self) -> bool {
        self.is_standard_pass() || matches!(
            self.pass_type(),
            OptimizationPassType::LoopInvariantCodeMotion |
            OptimizationPassType::InstructionScheduling |
            OptimizationPassType::MemoryOptimization |
            OptimizationPassType::InlineExpansion
        )
    }
    
    /// Run the optimization pass
    fn run(&self, ir_block: &mut IRBlock, config: &OptimizationConfig) -> JITResult<OptimizationPassResult>;
}

/// Optimization service
pub struct OptimizationService {
    /// Available optimization passes
    passes: Vec<Arc<dyn OptimizationPass>>,
    /// Global optimization metrics
    metrics: OptimizationMetrics,
}

impl OptimizationService {
    /// Create a new optimization service
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            metrics: OptimizationMetrics::default(),
        }
    }
    
    /// Add an optimization pass
    pub fn add_pass(&mut self, pass: Arc<dyn OptimizationPass>) {
        self.passes.push(pass);
    }
    
    /// Optimize an IR block
    pub fn optimize(&mut self, ir_block: IRBlock, config: OptimizationConfig) -> JITResult<OptimizationResult> {
        let mut context = OptimizationContext::new(ir_block, config);
        context.start();
        
        let mut optimized_block = context.ir_block.clone();
        
        // Run optimization passes
        for pass in &self.passes {
            if !pass.should_run(&context.config) {
                continue;
            }
            
            let start_time = Instant::now();
            let instructions_before = optimized_block.instructions.len();
            
            match pass.run(&mut optimized_block, &context.config) {
                Ok(mut result) => {
                    result.execution_time = start_time.elapsed();
                    result.instructions_before = instructions_before;
                    result.instructions_after = optimized_block.instructions.len();
                    
                    // Update metrics
                    self.update_metrics(&result);
                    context.metrics.merge(&self.metrics);
                    
                    context.add_pass_result(result);
                }
                Err(e) => {
                    let mut result = OptimizationPassResult::default();
                    result.pass_type = pass.pass_type();
                    result.status = OptimizationStatus::Failed;
                    result.execution_time = start_time.elapsed();
                    result.instructions_before = instructions_before;
                    result.instructions_after = optimized_block.instructions.len();
                    result.error_message = Some(e.to_string());
                    
                    context.add_pass_result(result);
                    
                    // Continue with other passes even if one fails
                }
            }
        }
        
        Ok(context.complete(optimized_block))
    }
    
    /// Update optimization metrics
    fn update_metrics(&mut self, result: &OptimizationPassResult) {
        self.metrics.total_optimizations += result.optimizations_performed;
        
        match result.pass_type {
            OptimizationPassType::DeadCodeElimination => {
                self.metrics.dead_code_eliminations += result.optimizations_performed;
            }
            OptimizationPassType::ConstantFolding => {
                self.metrics.constant_foldings += result.optimizations_performed;
            }
            OptimizationPassType::CommonSubexpressionElimination => {
                self.metrics.common_subexpression_eliminations += result.optimizations_performed;
            }
            OptimizationPassType::LoopInvariantCodeMotion => {
                self.metrics.loop_invariant_code_motions += result.optimizations_performed;
            }
            OptimizationPassType::InstructionCombining => {
                self.metrics.instruction_combinations += result.optimizations_performed;
            }
            OptimizationPassType::RegisterAllocation => {
                self.metrics.register_allocations += result.optimizations_performed;
            }
            OptimizationPassType::InstructionScheduling => {
                self.metrics.instruction_schedulings += result.optimizations_performed;
            }
            OptimizationPassType::MemoryOptimization => {
                self.metrics.memory_optimizations += result.optimizations_performed;
            }
            OptimizationPassType::Vectorization => {
                self.metrics.vectorizations += result.optimizations_performed;
            }
            OptimizationPassType::InlineExpansion => {
                self.metrics.inline_expansions += result.optimizations_performed;
            }
            OptimizationPassType::TailCallOptimization => {
                self.metrics.tail_call_optimizations += result.optimizations_performed;
            }
            OptimizationPassType::PeepholeOptimization => {
                self.metrics.peephole_optimizations += result.optimizations_performed;
            }
            OptimizationPassType::Custom => {
                // Custom optimizations are counted in total_optimizations only
            }
        }
    }
    
    /// Get optimization metrics
    pub fn metrics(&self) -> &OptimizationMetrics {
        &self.metrics
    }
    
    /// Reset optimization metrics
    pub fn reset_metrics(&mut self) {
        self.metrics.reset();
    }
}

impl Default for OptimizationService {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a unique optimization ID
fn generate_optimization_id() -> OptimizationId {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// Optimization analysis tools
pub mod analysis {
    use super::*;
    
    /// Analyze optimization potential
    pub fn analyze_optimization_potential(ir_block: &IRBlock) -> OptimizationPotential {
        let mut potential = OptimizationPotential::default();
        
        // Count different types of instructions
        for instruction in &ir_block.instructions {
            match instruction.opcode {
                // Instructions that can be constant folded
                crate::ir::Opcode::Add |
                crate::ir::Opcode::Sub |
                crate::ir::Opcode::Mul |
                crate::ir::Opcode::Div => {
                    if instruction.operands.iter().any(|op| matches!(op, Operand::Constant(_))) {
                        potential.constant_folding_opportunities += 1;
                    }
                }
                
                // Instructions that can be eliminated
                crate::ir::Opcode::Nop => {
                    potential.dead_code_elimination_opportunities += 1;
                }
                
                // Memory operations that can be optimized
                crate::ir::Opcode::Load |
                crate::ir::Opcode::Store => {
                    potential.memory_optimization_opportunities += 1;
                }
                
                _ => {}
            }
        }
        
        // Calculate overall potential
        potential.overall_potential = (
            potential.constant_folding_opportunities as f32 +
            potential.dead_code_elimination_opportunities as f32 +
            potential.memory_optimization_opportunities as f32
        ) / (ir_block.instructions.len() as f32);
        
        potential
    }
    
    /// Optimization potential analysis result
    #[derive(Debug, Clone, Default)]
    pub struct OptimizationPotential {
        /// Constant folding opportunities
        pub constant_folding_opportunities: usize,
        /// Dead code elimination opportunities
        pub dead_code_elimination_opportunities: usize,
        /// Memory optimization opportunities
        pub memory_optimization_opportunities: usize,
        /// Overall optimization potential (0.0 to 1.0)
        pub overall_potential: f32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_optimization_config_validation() {
        let mut config = OptimizationConfig::default();
        
        // Valid config
        assert!(config.validate().is_ok());
        
        // Invalid threshold
        config.threshold = -0.1;
        assert!(config.validate().is_err());
        
        config.threshold = 1.1;
        assert!(config.validate().is_err());
        
        // Invalid max time
        config.threshold = 0.5;
        config.max_time = Duration::ZERO;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_optimization_context() {
        let ir_block = IRBlock::default();
        let config = OptimizationConfig::default();
        let mut context = OptimizationContext::new(ir_block, config);
        
        assert_eq!(context.status, OptimizationStatus::NotStarted);
        
        context.start();
        assert_eq!(context.status, OptimizationStatus::InProgress);
        
        let optimized_block = IRBlock::default();
        let result = context.complete(optimized_block);
        
        assert_eq!(result.status, OptimizationStatus::Completed);
        assert!(result.total_time.as_nanos() > 0);
    }
    
    #[test]
    fn test_optimization_metrics() {
        let mut metrics1 = OptimizationMetrics::default();
        let mut metrics2 = OptimizationMetrics::default();
        
        metrics1.dead_code_eliminations = 5;
        metrics1.constant_foldings = 3;
        
        metrics2.dead_code_eliminations = 2;
        metrics2.constant_foldings = 7;
        
        metrics1.merge(&metrics2);
        
        assert_eq!(metrics1.dead_code_eliminations, 7);
        assert_eq!(metrics1.constant_foldings, 10);
        
        let summary = metrics1.summary();
        assert!(summary.contains("dead_code=7"));
        assert!(summary.contains("constant_folding=10"));
    }
    
    #[test]
    fn test_optimization_levels() {
        assert_eq!(OptimizationLevel::None as u8, 0);
        assert_eq!(OptimizationLevel::Basic as u8, 1);
        assert_eq!(OptimizationLevel::Standard as u8, 2);
        assert_eq!(OptimizationLevel::Aggressive as u8, 3);
        assert_eq!(OptimizationLevel::Maximum as u8, 4);
    }
    
    #[test]
    fn test_optimization_pass_types() {
        let pass_types = vec![
            OptimizationPassType::DeadCodeElimination,
            OptimizationPassType::ConstantFolding,
            OptimizationPassType::CommonSubexpressionElimination,
            OptimizationPassType::LoopInvariantCodeMotion,
            OptimizationPassType::InstructionCombining,
            OptimizationPassType::RegisterAllocation,
            OptimizationPassType::InstructionScheduling,
            OptimizationPassType::MemoryOptimization,
            OptimizationPassType::Vectorization,
            OptimizationPassType::InlineExpansion,
            OptimizationPassType::TailCallOptimization,
            OptimizationPassType::PeepholeOptimization,
            OptimizationPassType::Custom,
        ];
        
        for pass_type in pass_types {
            let display = format!("{}", pass_type);
            assert!(!display.is_empty());
        }
    }
}