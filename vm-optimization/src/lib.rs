//! Common optimization framework for VM cross-architecture translation
//!
//! This module provides a unified optimization pipeline with composable passes
//! that can be applied to IR blocks for cross-architecture translation.

use std::collections::{HashMap, HashSet};
use thiserror::Error;
use vm_error::{Architecture, RegId};

/// Errors that can occur during optimization
#[derive(Error, Debug, Clone, PartialEq)]
pub enum OptimizationError {
    #[error("Invalid IR block: {0}")]
    InvalidBlock(String),
    #[error("Optimization failed: {0}")]
    OptimizationFailed(String),
    #[error("Unsupported optimization for architecture: {0:?}")]
    UnsupportedArchitecture(Architecture),
    #[error("Optimization dependency not satisfied: {0}")]
    DependencyNotSatisfied(String),
    #[error("Optimization conflict: {0}")]
    OptimizationConflict(String),
}

/// Optimization levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OptimizationLevel {
    None = 0,
    Basic = 1,
    Standard = 2,
    Aggressive = 3,
    Max = 4,
}

/// Optimization result
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    pub success: bool,
    pub changed: bool,
    pub message: String,
    pub metrics: OptimizationMetrics,
}

/// Optimization metrics
#[derive(Debug, Clone, Default)]
pub struct OptimizationMetrics {
    pub instructions_removed: usize,
    pub instructions_added: usize,
    pub cycles_saved: u32,
    pub memory_saved: usize,
    pub register_pressure_reduced: i32,
    pub branch_predictions_improved: u32,
    pub cache_misses_reduced: u32,
}

/// Optimization context
#[derive(Debug, Clone)]
pub struct OptimizationContext {
    pub source_arch: Architecture,
    pub target_arch: Architecture,
    pub optimization_level: OptimizationLevel,
    pub target_features: HashMap<String, bool>,
    pub register_pressure: HashMap<RegId, u32>,
    pub loop_depth: u32,
    pub hotness: f32,
}

impl OptimizationContext {
    pub fn new(source_arch: Architecture, target_arch: Architecture) -> Self {
        Self {
            source_arch,
            target_arch,
            optimization_level: OptimizationLevel::Standard,
            target_features: HashMap::new(),
            register_pressure: HashMap::new(),
            loop_depth: 0,
            hotness: 0.0,
        }
    }

    pub fn with_optimization_level(mut self, level: OptimizationLevel) -> Self {
        self.optimization_level = level;
        self
    }

    pub fn with_feature(mut self, feature: impl Into<String>, enabled: bool) -> Self {
        self.target_features.insert(feature.into(), enabled);
        self
    }

    pub fn has_feature(&self, feature: &str) -> bool {
        self.target_features.get(feature).copied().unwrap_or(false)
    }

    pub fn get_register_pressure(&self, reg: RegId) -> u32 {
        self.register_pressure.get(&reg).copied().unwrap_or(0)
    }

    pub fn set_register_pressure(&mut self, reg: RegId, pressure: u32) {
        self.register_pressure.insert(reg, pressure);
    }
}

/// IR block representation (simplified for optimization)
#[derive(Debug, Clone)]
pub struct IRBlock {
    pub id: String,
    pub instructions: Vec<IRInstruction>,
    pub predecessors: Vec<String>,
    pub successors: Vec<String>,
    pub loop_header: bool,
    pub loop_depth: u32,
    pub frequency: f32,
}

/// IR instruction representation (simplified for optimization)
#[derive(Debug, Clone)]
pub struct IRInstruction {
    pub id: String,
    pub opcode: String,
    pub operands: Vec<IROperand>,
    pub flags: IRFlags,
    pub metadata: HashMap<String, String>,
}

/// IR operand representation
#[derive(Debug, Clone, PartialEq)]
pub enum IROperand {
    Register(RegId),
    Immediate(i64),
    Memory(MemoryOperand),
    Label(String),
    RegisterPair(RegId, RegId),
    RegisterList(Vec<RegId>),
}

/// Memory operand for IR
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryOperand {
    pub base: Option<RegId>,
    pub index: Option<RegId>,
    pub scale: u8,
    pub displacement: i64,
    pub size: u8,
}

/// IR instruction flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IRFlags {
    pub sets_flags: bool,
    pub reads_flags: bool,
    pub is_conditional: bool,
    pub is_predicated: bool,
    pub is_atomic: bool,
    pub is_volatile: bool,
    pub is_privileged: bool,
    pub is_terminal: bool,
}

/// Trait for optimization passes
pub trait OptimizationPass {
    /// Get the name of this optimization pass
    fn name(&self) -> &str;

    /// Get the description of this optimization pass
    fn description(&self) -> &str;

    /// Check if this pass should run given the context
    fn should_run(&self, context: &OptimizationContext) -> bool;

    /// Get the dependencies of this pass
    fn dependencies(&self) -> Vec<&'static str>;

    /// Apply this optimization pass to an IR block
    fn apply(
        &self,
        block: &mut IRBlock,
        context: &OptimizationContext,
    ) -> Result<OptimizationResult, OptimizationError>;

    /// Get the optimization level required for this pass
    fn required_level(&self) -> OptimizationLevel;
}

/// Optimization pipeline that manages multiple passes
pub struct OptimizationPipeline {
    passes: Vec<Box<dyn OptimizationPass>>,
    context: OptimizationContext,
    stats: PipelineStats,
}

impl std::fmt::Debug for OptimizationPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OptimizationPipeline")
            .field("passes_count", &self.passes.len())
            .field("context", &self.context)
            .field("stats", &self.stats)
            .finish()
    }
}

/// Pipeline statistics
#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    pub total_passes: usize,
    pub successful_passes: usize,
    pub failed_passes: usize,
    pub total_instructions_removed: usize,
    pub total_instructions_added: usize,
    pub total_cycles_saved: u32,
    pub total_time_ms: u64,
}

impl OptimizationPipeline {
    pub fn new(context: OptimizationContext) -> Self {
        Self {
            passes: Vec::new(),
            context,
            stats: PipelineStats::default(),
        }
    }

    /// Add an optimization pass to the pipeline
    pub fn add_pass(&mut self, pass: Box<dyn OptimizationPass>) {
        self.passes.push(pass);
    }

    /// Run the optimization pipeline on a list of blocks
    #[allow(clippy::ptr_arg)]
    pub fn run(&mut self, blocks: &mut Vec<IRBlock>) -> Result<PipelineStats, OptimizationError> {
        let start_time = std::time::Instant::now();
        self.stats.total_passes = self.passes.len();

        // Sort passes by dependencies
        self.sort_passes_by_dependencies()?;

        // Run each pass
        for pass in &self.passes {
            if !pass.should_run(&self.context) {
                continue;
            }

            // Check dependencies
            if !self.check_dependencies(pass.as_ref()) {
                continue;
            }

            // Apply pass to all blocks
            let mut pass_success = true;
            for block in blocks.iter_mut() {
                let result = pass.apply(block, &self.context);
                match result {
                    Ok(result) => {
                        if result.success {
                            self.stats.successful_passes += 1;
                            self.stats.total_instructions_removed +=
                                result.metrics.instructions_removed;
                            self.stats.total_instructions_added +=
                                result.metrics.instructions_added;
                            self.stats.total_cycles_saved += result.metrics.cycles_saved;
                        } else {
                            pass_success = false;
                        }
                    }
                    Err(_) => {
                        self.stats.failed_passes += 1;
                        pass_success = false;
                    }
                }
            }

            if !pass_success {
                log::warn!("Optimization pass '{}' failed", pass.name());
            }
        }

        self.stats.total_time_ms = start_time.elapsed().as_millis() as u64;
        Ok(self.stats.clone())
    }

    /// Get pipeline statistics
    pub fn get_stats(&self) -> &PipelineStats {
        &self.stats
    }

    /// Get optimization context
    pub fn get_context(&self) -> &OptimizationContext {
        &self.context
    }

    /// Sort passes by dependencies
    fn sort_passes_by_dependencies(&mut self) -> Result<(), OptimizationError> {
        let mut sorted_names = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();

        for pass in &self.passes {
            self.visit_pass(
                pass.as_ref(),
                &mut sorted_names,
                &mut visited,
                &mut visiting,
            )?;
        }

        // Reorder passes based on sorted names
        let mut sorted_passes = Vec::new();
        for name in sorted_names {
            if let Some(pos) = self.passes.iter().position(|p| p.name() == name) {
                sorted_passes.push(self.passes.remove(pos));
            }
        }
        self.passes = sorted_passes;
        Ok(())
    }

    /// Visit a pass for topological sorting
    fn visit_pass(
        &self,
        pass: &dyn OptimizationPass,
        sorted: &mut Vec<String>,
        visited: &mut HashSet<String>,
        visiting: &mut HashSet<String>,
    ) -> Result<(), OptimizationError> {
        let name = pass.name();

        if visiting.contains(name) {
            return Err(OptimizationError::DependencyNotSatisfied(format!(
                "Circular dependency detected for pass '{}'",
                name
            )));
        }

        if visited.contains(name) {
            return Ok(());
        }

        visiting.insert(name.to_string());

        for dep in pass.dependencies() {
            let dep_pass = self
                .passes
                .iter()
                .find(|p| p.name() == dep)
                .ok_or_else(|| {
                    OptimizationError::DependencyNotSatisfied(format!(
                        "Dependency '{}' not found for pass '{}'",
                        dep, name
                    ))
                })?;

            self.visit_pass(dep_pass.as_ref(), sorted, visited, visiting)?;
        }

        visiting.remove(name);
        visited.insert(name.to_string());
        sorted.push(name.to_string());

        Ok(())
    }

    /// Check if all dependencies of a pass are satisfied
    fn check_dependencies(&self, pass: &dyn OptimizationPass) -> bool {
        for dep in pass.dependencies() {
            if !self.passes.iter().any(|p| p.name() == dep) {
                return false;
            }
        }
        true
    }
}

/// Common optimization passes
/// Dead code elimination pass
#[derive(Debug)]
pub struct DeadCodeEliminationPass;

impl OptimizationPass for DeadCodeEliminationPass {
    fn name(&self) -> &str {
        "dead_code_elimination"
    }

    fn description(&self) -> &str {
        "Remove dead code that is never executed"
    }

    fn should_run(&self, context: &OptimizationContext) -> bool {
        context.optimization_level >= OptimizationLevel::Basic
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec![]
    }

    fn apply(
        &self,
        block: &mut IRBlock,
        _context: &OptimizationContext,
    ) -> Result<OptimizationResult, OptimizationError> {
        let mut removed = 0;
        let mut live_instructions = HashSet::new();

        // Mark live instructions (simplified)
        let instruction_ids: Vec<_> = block
            .instructions
            .iter()
            .map(|instr| instr.id.clone())
            .collect();
        for instr_id in &instruction_ids {
            if let Some(instr) = block.instructions.iter().find(|i| &i.id == instr_id) {
                if instr.flags.is_terminal || instr.opcode == "store" {
                    live_instructions.insert(instr_id);
                }
            }
        }

        // Remove dead instructions
        block.instructions.retain(|instr| {
            let is_live = live_instructions.contains(&instr.id);
            if !is_live {
                removed += 1;
            }
            is_live
        });

        Ok(OptimizationResult {
            success: true,
            changed: removed > 0,
            message: format!("Removed {} dead instructions", removed),
            metrics: OptimizationMetrics {
                instructions_removed: removed,
                instructions_added: 0,
                cycles_saved: removed as u32,
                memory_saved: 0,
                register_pressure_reduced: 0,
                branch_predictions_improved: 0,
                cache_misses_reduced: 0,
            },
        })
    }

    fn required_level(&self) -> OptimizationLevel {
        OptimizationLevel::Basic
    }
}

/// Constant folding pass
#[derive(Debug)]
pub struct ConstantFoldingPass;

impl OptimizationPass for ConstantFoldingPass {
    fn name(&self) -> &str {
        "constant_folding"
    }

    fn description(&self) -> &str {
        "Fold constant expressions at compile time"
    }

    fn should_run(&self, context: &OptimizationContext) -> bool {
        context.optimization_level >= OptimizationLevel::Basic
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec![]
    }

    fn apply(
        &self,
        block: &mut IRBlock,
        _context: &OptimizationContext,
    ) -> Result<OptimizationResult, OptimizationError> {
        let mut folded = 0;

        // Simplified constant folding
        for instr in &mut block.instructions {
            match instr.opcode.as_str() {
                "add" => {
                    if let (Some(IROperand::Immediate(a)), Some(IROperand::Immediate(b))) =
                        (instr.operands.get(1), instr.operands.get(2))
                    {
                        instr.opcode = "mov".to_string();
                        instr.operands =
                            vec![instr.operands[0].clone(), IROperand::Immediate(a + b)];
                        folded += 1;
                    }
                }
                "sub" => {
                    if let (Some(IROperand::Immediate(a)), Some(IROperand::Immediate(b))) =
                        (instr.operands.get(1), instr.operands.get(2))
                    {
                        instr.opcode = "mov".to_string();
                        instr.operands =
                            vec![instr.operands[0].clone(), IROperand::Immediate(a - b)];
                        folded += 1;
                    }
                }
                _ => {}
            }
        }

        Ok(OptimizationResult {
            success: true,
            changed: folded > 0,
            message: format!("Folded {} constant expressions", folded),
            metrics: OptimizationMetrics {
                instructions_removed: 0,
                instructions_added: 0,
                cycles_saved: folded as u32,
                memory_saved: 0,
                register_pressure_reduced: 0,
                branch_predictions_improved: 0,
                cache_misses_reduced: 0,
            },
        })
    }

    fn required_level(&self) -> OptimizationLevel {
        OptimizationLevel::Basic
    }
}

/// Common subexpression elimination pass
#[derive(Debug)]
pub struct CommonSubexpressionEliminationPass;

impl OptimizationPass for CommonSubexpressionEliminationPass {
    fn name(&self) -> &str {
        "common_subexpression_elimination"
    }

    fn description(&self) -> &str {
        "Eliminate common subexpressions to avoid redundant computation"
    }

    fn should_run(&self, context: &OptimizationContext) -> bool {
        context.optimization_level >= OptimizationLevel::Standard
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["constant_folding"]
    }

    fn apply(
        &self,
        block: &mut IRBlock,
        _context: &OptimizationContext,
    ) -> Result<OptimizationResult, OptimizationError> {
        let mut eliminated = 0;
        let mut expressions = HashMap::new();

        // Find common subexpressions (simplified)
        for instr in &mut block.instructions {
            let key = format!("{}:{:?}", instr.opcode, instr.operands);

            if let Some(existing_reg) = expressions.get(&key) {
                // Replace with existing result
                instr.opcode = "mov".to_string();
                instr.operands = vec![
                    instr.operands[0].clone(),
                    IROperand::Register(*existing_reg),
                ];
                eliminated += 1;
            } else if let Some(IROperand::Register(reg)) = instr.operands.first() {
                expressions.insert(key, *reg);
            }
        }

        Ok(OptimizationResult {
            success: true,
            changed: eliminated > 0,
            message: format!("Eliminated {} common subexpressions", eliminated),
            metrics: OptimizationMetrics {
                instructions_removed: 0,
                instructions_added: 0,
                cycles_saved: eliminated as u32 * 2,
                memory_saved: 0,
                register_pressure_reduced: 0,
                branch_predictions_improved: 0,
                cache_misses_reduced: 0,
            },
        })
    }

    fn required_level(&self) -> OptimizationLevel {
        OptimizationLevel::Standard
    }
}

/// Instruction scheduling pass
#[derive(Debug)]
pub struct InstructionSchedulingPass;

impl OptimizationPass for InstructionSchedulingPass {
    fn name(&self) -> &str {
        "instruction_scheduling"
    }

    fn description(&self) -> &str {
        "Schedule instructions to improve parallelism and reduce stalls"
    }

    fn should_run(&self, context: &OptimizationContext) -> bool {
        context.optimization_level >= OptimizationLevel::Aggressive
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["common_subexpression_elimination"]
    }

    fn apply(
        &self,
        block: &mut IRBlock,
        _context: &OptimizationContext,
    ) -> Result<OptimizationResult, OptimizationError> {
        // Simplified instruction scheduling
        let mut scheduled = Vec::new();
        let mut dependencies = HashMap::new();

        // Build dependency graph
        for (i, instr) in block.instructions.iter().enumerate() {
            let mut deps = HashSet::new();

            for operand in &instr.operands {
                if let IROperand::Register(reg) = operand {
                    deps.insert(*reg);
                }
            }

            dependencies.insert(i, deps);
        }

        // Schedule instructions (simplified)
        let mut scheduled_indices = HashSet::new();
        while scheduled_indices.len() < block.instructions.len() {
            for (i, instr) in block.instructions.iter().enumerate() {
                if scheduled_indices.contains(&i) {
                    continue;
                }

                let deps = dependencies.get(&i).unwrap();
                let can_schedule = deps.iter().all(|&reg| {
                    !scheduled_indices.iter().any(|&scheduled_idx| {
                        let instr: &IRInstruction = &block.instructions[scheduled_idx];
                        let operand: Option<&IROperand> = instr.operands.first();
                        if let Some(IROperand::Register(scheduled_reg)) = operand {
                            *scheduled_reg == reg
                        } else {
                            false
                        }
                    })
                });

                if can_schedule {
                    scheduled.push(instr.clone());
                    scheduled_indices.insert(i);
                    break;
                }
            }
        }

        block.instructions = scheduled;

        Ok(OptimizationResult {
            success: true,
            changed: true,
            message: "Scheduled instructions to improve parallelism".to_string(),
            metrics: OptimizationMetrics {
                instructions_removed: 0,
                instructions_added: 0,
                cycles_saved: (block.instructions.len() / 4) as u32, // Estimate
                memory_saved: 0,
                register_pressure_reduced: 0,
                branch_predictions_improved: 0,
                cache_misses_reduced: 0,
            },
        })
    }

    fn required_level(&self) -> OptimizationLevel {
        OptimizationLevel::Aggressive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimization_context() {
        let context = OptimizationContext::new(Architecture::X86_64, Architecture::ARM64)
            .with_optimization_level(OptimizationLevel::Aggressive)
            .with_feature("avx512", true);

        assert_eq!(context.source_arch, Architecture::X86_64);
        assert_eq!(context.target_arch, Architecture::ARM64);
        assert_eq!(context.optimization_level, OptimizationLevel::Aggressive);
        assert!(context.has_feature("avx512"));
    }

    #[test]
    fn test_dead_code_elimination() {
        let pass = DeadCodeEliminationPass;
        assert_eq!(pass.name(), "dead_code_elimination");
        assert!(pass.should_run(
            &OptimizationContext::new(Architecture::X86_64, Architecture::ARM64)
                .with_optimization_level(OptimizationLevel::Basic)
        ));
        assert_eq!(pass.required_level(), OptimizationLevel::Basic);
    }

    #[test]
    fn test_constant_folding() {
        let pass = ConstantFoldingPass;
        let mut block = IRBlock {
            id: "test".to_string(),
            instructions: vec![IRInstruction {
                id: "1".to_string(),
                opcode: "add".to_string(),
                operands: vec![
                    IROperand::Register(0),
                    IROperand::Immediate(10),
                    IROperand::Immediate(20),
                ],
                flags: IRFlags::default(),
                metadata: HashMap::new(),
            }],
            predecessors: Vec::new(),
            successors: Vec::new(),
            loop_header: false,
            loop_depth: 0,
            frequency: 1.0,
        };

        let context = OptimizationContext::new(Architecture::X86_64, Architecture::ARM64);
        let result = pass.apply(&mut block, &context).unwrap();

        assert!(result.success);
        assert!(result.changed);
        assert_eq!(block.instructions[0].opcode, "mov");
        assert_eq!(block.instructions[0].operands[1], IROperand::Immediate(30));
    }

    #[test]
    fn test_optimization_pipeline() {
        let context = OptimizationContext::new(Architecture::X86_64, Architecture::ARM64)
            .with_optimization_level(OptimizationLevel::Standard);

        let mut pipeline = OptimizationPipeline::new(context);
        pipeline.add_pass(Box::new(ConstantFoldingPass));
        pipeline.add_pass(Box::new(DeadCodeEliminationPass));

        let mut blocks = vec![IRBlock {
            id: "test".to_string(),
            instructions: vec![IRInstruction {
                id: "1".to_string(),
                opcode: "add".to_string(),
                operands: vec![
                    IROperand::Register(0),
                    IROperand::Immediate(10),
                    IROperand::Immediate(20),
                ],
                flags: IRFlags::default(),
                metadata: HashMap::new(),
            }],
            predecessors: Vec::new(),
            successors: Vec::new(),
            loop_header: false,
            loop_depth: 0,
            frequency: 1.0,
        }];

        let stats = pipeline.run(&mut blocks).unwrap();
        assert_eq!(stats.total_passes, 2);
        assert!(stats.successful_passes > 0);
    }
}
