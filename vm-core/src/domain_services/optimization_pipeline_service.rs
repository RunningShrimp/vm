//! # Optimization Pipeline Domain Service
//!
//! This service manages multi-stage optimization pipelines for cross-architecture translation.
//! It coordinates different optimization stages and ensures proper sequencing of optimizations.
//!
//! ## Domain Responsibilities
//!
//! The optimization pipeline service is responsible for:
//!
//! 1. **Pipeline Configuration**: Defining and validating optimization pipeline stages
//! 2. **Stage Orchestration**: Coordinating the execution of optimization stages
//! 3. **Performance Validation**: Ensuring performance requirements are met
//! 4. **Progress Tracking**: Monitoring pipeline execution and collecting metrics
//!
//! ## DDD Patterns
//!
//! ### Domain Service Pattern
//! This is a **Domain Service** because:
//! - It orchestrates multiple optimization stages (different aggregates)
//! - It enforces business rules for pipeline configuration
//! - It coordinates between translation, optimization, and code generation
//!
//! ### Domain Events Published
//!
//! - **`OptimizationEvent::PipelineConfigCreated`**: Published when pipeline configuration is created
//! - **`OptimizationEvent::StageCompleted`**: Published when each optimization stage completes
//! - **`OptimizationEvent::PipelineCompleted`**: Published when the entire pipeline completes
//!
//! ## Pipeline Stages
//!
//! The optimization pipeline consists of the following stages (in order):
//!
//! 1. **IR Generation** (`IrGeneration`): Generate intermediate representation
//! 2. **Basic Block Optimization** (`BasicBlockOptimization`): Optimize basic blocks
//! 3. **Register Allocation** (`RegisterAllocation`): Allocate virtual registers to physical registers
//! 4. **Instruction Scheduling** (`InstructionScheduling`): Reorder instructions for optimal execution
//! 5. **Target Optimization** (`TargetOptimization`): Apply target-specific optimizations
//! 6. **Code Generation** (`CodeGeneration`): Generate target machine code
//!
//! ## Usage Examples
//!
//! ### Creating a Pipeline Configuration
//!
//! ```rust
//! use crate::domain_services::optimization_pipeline_service::{
//!     OptimizationPipelineService, OptimizationPipelineConfig,
//!     PerformanceRequirements, OptimizationPriority
//! };
//! use crate::GuestArch;
//!
//! let config = OptimizationPipelineConfig::new(
//!     GuestArch::X86_64,
//!     GuestArch::ARM64,
//!     2,  // optimization level
//! );
//!
//! let service = OptimizationPipelineService::new(config.clone());
//! ```
//!
//! ### Executing a Pipeline
//!
//! ```rust
//! let result = service.execute_pipeline(
//!     &ir_code,
//!     &performance_requirements,
//!     &business_rules,
//! )?;
//!
//! if result.success {
//!     println!("Pipeline completed in {}ms", result.total_time_ms);
//!     println!("Stages completed: {}", result.completed_stages.len());
//!
//!     for (stage, time) in &result.stage_times {
//!         println!("  {}: {}ms", stage.name(), time);
//!     }
//! }
//! ```
//!
//! ### Customizing Pipeline Stages
//!
//! ```rust
//! let mut config = OptimizationPipelineConfig::new(
//!     GuestArch::X86_64,
//!     GuestArch::ARM64,
//!     2,
//! );
//!
//! // Disable instruction scheduling for faster compilation
//! config.enabled_stages.retain(|s| s != &OptimizationStage::InstructionScheduling);
//!
//! // Adjust optimization level
//! config.optimization_level = 1;  // Basic optimization
//! ```
//!
//! ### Performance Requirements
//!
//! ```rust
//! use std::time::Duration;
//!
//! let requirements = PerformanceRequirements {
//!     max_compilation_time: Some(Duration::from_secs(5)),
//!     max_memory_usage: Some(1024 * 1024 * 1024),  // 1GB
//!     target_throughput: Some(1000.0),  // instructions/ms
//!     optimization_priority: OptimizationPriority::Speed,
//! };
//!
//! config.performance_requirements = requirements;
//! ```
//!
//! ## Pipeline Execution Flow
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │          Optimization Pipeline Execution                 │
//! └─────────────────────────────────────────────────────────┘
//!                            │
//!                            ▼
//!              ┌─────────────────────────┐
//!              │  Validate Configuration  │
//!              └─────────────────────────┘
//!                            │
//!                            ▼
//!              ┌─────────────────────────┐
//!              │    IR Generation        │
//!              └─────────────────────────┘
//!                            │
//!                            ▼
//!              ┌─────────────────────────┐
//!              │  Basic Block Opt        │
//!              └─────────────────────────┘
//!                            │
//!                            ▼
//!              ┌─────────────────────────┐
//!              │   Register Allocation   │
//!              └─────────────────────────┘
//!                            │
//!                            ▼
//!              ┌─────────────────────────┐
//!              │  Instruction Scheduling │
//!              └─────────────────────────┘
//!                            │
//!                            ▼
//!              ┌─────────────────────────┐
//!              │  Target Optimization    │
//!              └─────────────────────────┘
//!                            │
//!                            ▼
//!              ┌─────────────────────────┐
//!              │     Code Generation     │
//!              └─────────────────────────┘
//!                            │
//!                            ▼
//!              ┌─────────────────────────┐
//!              │   Performance Check     │
//!              └─────────────────────────┘
//!                            │
//!                    ┌──────┴──────┐
//!                    ▼             ▼
//!                Success         Failure
//! ```
//!
//! ## Optimization Levels
//!
//! | Level | Description | Stages Enabled |
//! |-------|-------------|----------------|
//! | 0 | No optimization | IR Generation, Code Generation |
//! | 1 | Basic optimization | Level 0 + Basic Block Optimization |
//! | 2 | Standard optimization | Level 1 + Register Allocation |
//! | 3 | Aggressive optimization | Level 2 + Instruction Scheduling, Target Optimization |
//!
//! ## Integration with Aggregate Roots
//!
//! This service works with:
//! - **`VirtualMachineAggregate`**: VM-level optimization coordination
//! - **`TranslationAggregate`**: Cross-architecture translation
//! - **`CodeBlockAggregate`**: Code block optimization

use crate::domain_services::events::{DomainEventBus, DomainEventEnum, OptimizationEvent};
use crate::{VmError, VmResult};
use crate::{CoreError};
use crate::aggregate_root::VirtualMachineAggregate;
use std::sync::Arc;
use std::collections::HashMap;

/// Optimization stage in the pipeline
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationStage {
    /// Initial IR generation
    IrGeneration,
    /// Basic block optimization
    BasicBlockOptimization,
    /// Register allocation
    RegisterAllocation,
    /// Instruction scheduling
    InstructionScheduling,
    /// Target-specific optimizations
    TargetOptimization,
    /// Final code generation
    CodeGeneration,
}

impl OptimizationStage {
    /// Get the stage name
    pub fn name(&self) -> &'static str {
        match self {
            OptimizationStage::IrGeneration => "IR Generation",
            OptimizationStage::BasicBlockOptimization => "Basic Block Optimization",
            OptimizationStage::RegisterAllocation => "Register Allocation",
            OptimizationStage::InstructionScheduling => "Instruction Scheduling",
            OptimizationStage::TargetOptimization => "Target Optimization",
            OptimizationStage::CodeGeneration => "Code Generation",
        }
    }
    
    /// Get the stage order
    pub fn order(&self) -> u8 {
        match self {
            OptimizationStage::IrGeneration => 1,
            OptimizationStage::BasicBlockOptimization => 2,
            OptimizationStage::RegisterAllocation => 3,
            OptimizationStage::InstructionScheduling => 4,
            OptimizationStage::TargetOptimization => 5,
            OptimizationStage::CodeGeneration => 6,
        }
    }
}

/// Optimization pipeline configuration
#[derive(Debug, Clone)]
pub struct OptimizationPipelineConfig {
    source_arch: crate::GuestArch,
    target_arch: crate::GuestArch,
    optimization_level: u8,
}

impl Default for OptimizationPipelineConfig {
    fn default() -> Self {
        Self {
            source_arch: crate::GuestArch::X86_64,
            target_arch: crate::GuestArch::X86_64,
            optimization_level: 2,
        }
    }
}

impl OptimizationPipelineConfig {
    /// Source architecture
    pub source_arch: crate::GuestArch,
    /// Target architecture
    pub target_arch: crate::GuestArch,
    /// Optimization level (0-3)
    pub optimization_level: u8,
    /// Enabled stages
    pub enabled_stages: Vec<OptimizationStage>,
    /// Stage-specific options
    pub stage_options: HashMap<String, String>,
    /// Performance requirements
    pub performance_requirements: PerformanceRequirements,
}

impl OptimizationPipelineConfig {
    /// Create a new pipeline configuration
    pub fn new(
        source_arch: crate::GuestArch,
        target_arch: crate::GuestArch,
        optimization_level: u8,
    ) -> Self {
        let enabled_stages = match optimization_level {
            0 => vec![OptimizationStage::IrGeneration, OptimizationStage::CodeGeneration],
            1 => vec![
                OptimizationStage::IrGeneration,
                OptimizationStage::BasicBlockOptimization,
                OptimizationStage::CodeGeneration,
            ],
            2 => vec![
                OptimizationStage::IrGeneration,
                OptimizationStage::BasicBlockOptimization,
                OptimizationStage::RegisterAllocation,
                OptimizationStage::CodeGeneration,
            ],
            3 => vec![
                OptimizationStage::IrGeneration,
                OptimizationStage::BasicBlockOptimization,
                OptimizationStage::RegisterAllocation,
                OptimizationStage::InstructionScheduling,
                OptimizationStage::TargetOptimization,
                OptimizationStage::CodeGeneration,
            ],
            _ => vec![],
        };
        
        Self {
            source_arch,
            target_arch,
            optimization_level,
            enabled_stages,
            stage_options: HashMap::new(),
            performance_requirements: PerformanceRequirements::default(),
        }
    }
    
    /// Check if a stage is enabled
    pub fn is_stage_enabled(&self, stage: &OptimizationStage) -> bool {
        self.enabled_stages.contains(stage)
    }
    
    /// Get the next stage in the pipeline
    pub fn next_stage(&self, current: &OptimizationStage) -> Option<OptimizationStage> {
        let current_order = current.order();
        self.enabled_stages
            .iter()
            .find(|s| s.order() > current_order)
            .cloned()
    }
}

/// Performance requirements for optimization
#[derive(Debug, Clone)]
pub struct PerformanceRequirements {
    /// Maximum compilation time in milliseconds
    pub max_compilation_time_ms: Option<u64>,
    /// Maximum memory usage in MB
    pub max_memory_usage_mb: Option<u64>,
    /// Target execution speedup factor
    pub target_speedup: Option<f32>,
    /// Priority (speed vs size)
    pub priority: OptimizationPriority,
}

impl Default for PerformanceRequirements {
    fn default() -> Self {
        Self {
            max_compilation_time_ms: Some(5000), // 5 seconds
            max_memory_usage_mb: Some(512),      // 512MB
            target_speedup: Some(2.0),           // 2x speedup
            priority: OptimizationPriority::Balanced,
        }
    }
}

/// Optimization priority
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationPriority {
    /// Optimize for speed
    Speed,
    /// Optimize for size
    Size,
    /// Balanced approach
    Balanced,
    /// Optimize for compilation time
    CompilationTime,
}

/// Pipeline execution result
#[derive(Debug, Clone)]
pub struct PipelineExecutionResult {
    /// Execution success
    pub success: bool,
    /// Completed stages
    pub completed_stages: Vec<OptimizationStage>,
    /// Execution time per stage (in milliseconds)
    pub stage_times: HashMap<String, u64>,
    /// Total execution time (in milliseconds)
    pub total_time_ms: u64,
    /// Memory usage per stage (in MB)
    pub stage_memory_usage: HashMap<String, f32>,
    /// Peak memory usage (in MB)
    pub peak_memory_usage_mb: f32,
    /// Optimization statistics
    pub optimization_stats: OptimizationStats,
    /// Error message if failed
    pub error_message: Option<String>,
}

/// Optimization statistics
#[derive(Debug, Clone, Default)]
pub struct OptimizationStats {
    /// Number of basic blocks optimized
    pub basic_blocks_optimized: u32,
    /// Number of instructions optimized
    pub instructions_optimized: u32,
    /// Number of registers allocated
    pub registers_allocated: u32,
    /// Number of instructions scheduled
    pub instructions_scheduled: u32,
    /// Estimated performance improvement
    pub estimated_improvement_percent: f32,
}

/// Trait for optimization pipeline business rules
pub trait OptimizationPipelineBusinessRule: Send + Sync {
    /// Validate pipeline configuration
    fn validate_pipeline_config(&self, config: &OptimizationPipelineConfig) -> VmResult<()>;
    
    /// Validate stage execution
    fn validate_stage_execution(
        &self,
        stage: &OptimizationStage,
        config: &OptimizationPipelineConfig,
    ) -> VmResult<()>;
    
    /// Check if pipeline should continue to next stage
    fn should_continue_pipeline(
        &self,
        current_stage: &OptimizationStage,
        result: &PipelineExecutionResult,
        config: &OptimizationPipelineConfig,
    ) -> VmResult<bool>;
}

/// Optimization Pipeline Domain Service
pub struct OptimizationPipelineDomainService {
    business_rules: Vec<Box<dyn OptimizationPipelineBusinessRule>>,
    event_bus: Option<Arc<dyn DomainEventBus>>,
}

impl OptimizationPipelineDomainService {
    /// Create a new optimization pipeline domain service
    pub fn new() -> Self {
        let service = Self {
            business_rules: Vec::new(),
            event_bus: None,
        };

        service
    }
    
    /// Set the event bus
    pub fn with_event_bus(mut self, event_bus: Arc<dyn DomainEventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }
    
    /// Add a business rule
    pub fn add_business_rule(&mut self, rule: Box<dyn OptimizationPipelineBusinessRule>) {
        self.business_rules.push(rule);
    }
    
    /// Create an optimization pipeline configuration
    pub fn create_pipeline_config(
        &self,
        source_arch: crate::GuestArch,
        target_arch: crate::GuestArch,
        optimization_level: u8,
        performance_requirements: PerformanceRequirements,
    ) -> VmResult<OptimizationPipelineConfig> {
        let mut config = OptimizationPipelineConfig::new(
            source_arch,
            target_arch,
            optimization_level,
        );
        config.performance_requirements = performance_requirements;
        
        // Validate configuration
        for rule in &self.business_rules {
            rule.validate_pipeline_config(&config)?;
        }
        
        // Publish event
        if let Some(event_bus) = &self.event_bus {
            let event = DomainEventEnum::Optimization(OptimizationEvent::PipelineConfigCreated {
                source_arch: format!("{:?}", source_arch),
                target_arch: format!("{:?}", target_arch),
                optimization_level,
                stages_count: config.enabled_stages.len(),
                occurred_at: std::time::SystemTime::now(),
            });
            event_bus.publish(event);
        }
        
        Ok(config)
    }
    
    /// Execute the optimization pipeline
    pub fn execute_pipeline(
        &self,
        config: &OptimizationPipelineConfig,
        aggregate: &mut VirtualMachineAggregate,
    ) -> VmResult<PipelineExecutionResult> {
        let start_time = std::time::Instant::now();
        let mut result = PipelineExecutionResult {
            success: false,
            completed_stages: Vec::new(),
            stage_times: HashMap::new(),
            total_time_ms: 0,
            stage_memory_usage: HashMap::new(),
            peak_memory_usage_mb: 0.0,
            optimization_stats: OptimizationStats::default(),
            error_message: None,
        };
        
        // Get the first stage
        let mut current_stage = config.enabled_stages.first().cloned();
        
        while let Some(stage) = current_stage {
            // Validate stage execution
            for rule in &self.business_rules {
                rule.validate_stage_execution(&stage, config)?;
            }
            
            // Execute the stage
            let stage_start = std::time::Instant::now();
            match self.execute_stage(&stage, config, aggregate) {
                Ok(stage_result) => {
                    let stage_time = stage_start.elapsed().as_millis() as u64;
                    result.stage_times.insert(stage.name().to_string(), stage_time);
                    result.stage_memory_usage.insert(stage.name().to_string(), stage_result.memory_usage_mb);
                    result.peak_memory_usage_mb = result.peak_memory_usage_mb.max(stage_result.memory_usage_mb);
                    
                    // Update optimization stats
                    self.update_optimization_stats(&mut result.optimization_stats, &stage_result);
                    
                    result.completed_stages.push(stage.clone());
                    
                    // Publish stage completion event
                    if let Some(event_bus) = &self.event_bus {
                        let event = DomainEventEnum::Optimization(OptimizationEvent::StageCompleted {
                            stage_name: stage.name().to_string(),
                            execution_time_ms: stage_time,
                            memory_usage_mb: stage_result.memory_usage_mb,
                            success: true,
                            occurred_at: std::time::SystemTime::now(),
                        });
                        event_bus.publish(event);
                    }
                    
                    // Check if pipeline should continue
                    let mut should_continue = true;
                    for rule in &self.business_rules {
                        should_continue &= rule.should_continue_pipeline(&stage, &result, config)?;
                    }
                    
                    if !should_continue {
                        break;
                    }
                }
                Err(e) => {
                    result.error_message = Some(format!("Stage {} failed: {}", stage.name(), e));
                    
                    // Publish stage failure event
                    if let Some(event_bus) = &self.event_bus {
                        let event = DomainEventEnum::Optimization(OptimizationEvent::StageCompleted {
                            stage_name: stage.name().to_string(),
                            execution_time_ms: stage_start.elapsed().as_millis() as u64,
                            memory_usage_mb: 0.0,
                            success: false,
                            occurred_at: std::time::SystemTime::now(),
                        });
                        event_bus.publish(event);
                    }
                    
                    break;
                }
            }
            
            // Get next stage
            current_stage = config.next_stage(&stage);
        }
        
        result.total_time_ms = start_time.elapsed().as_millis() as u64;
        result.success = result.error_message.is_none() && 
                        result.completed_stages.len() == config.enabled_stages.len();
        
        // Publish pipeline completion event
        if let Some(event_bus) = &self.event_bus {
            let event = DomainEventEnum::Optimization(OptimizationEvent::PipelineCompleted {
                success: result.success,
                total_time_ms: result.total_time_ms,
                stages_completed: result.completed_stages.len(),
                peak_memory_usage_mb: result.peak_memory_usage_mb,
                occurred_at: std::time::SystemTime::now(),
            });
            event_bus.publish(event);
        }
        
        Ok(result)
    }
    
    /// Execute a single optimization stage
    fn execute_stage(
        &self,
        stage: &OptimizationStage,
        config: &OptimizationPipelineConfig,
        _aggregate: &mut VirtualMachineAggregate,
    ) -> VmResult<StageExecutionResult> {
        // Simulate stage execution with realistic timing and memory usage
        let (execution_time_ms, memory_usage_mb, stats) = match stage {
            OptimizationStage::IrGeneration => {
                let time = match config.optimization_level {
                    0 => 50,
                    1 => 80,
                    2 => 120,
                    3 => 200,
                    _ => 100,
                };
                (time, 32.0, StageStats::IrGeneration { ir_instructions: 10000 })
            }
            OptimizationStage::BasicBlockOptimization => {
                let time = match config.optimization_level {
                    0 => 0,
                    1 => 100,
                    2 => 200,
                    3 => 350,
                    _ => 150,
                };
                (time, 64.0, StageStats::BasicBlockOptimization { blocks_optimized: 500 })
            }
            OptimizationStage::RegisterAllocation => {
                let time = match config.optimization_level {
                    0 | 1 => 0,
                    2 => 150,
                    3 => 250,
                    _ => 100,
                };
                (time, 48.0, StageStats::RegisterAllocation { registers_used: 32 })
            }
            OptimizationStage::InstructionScheduling => {
                let time = match config.optimization_level {
                    0 | 1 | 2 => 0,
                    3 => 200,
                    _ => 100,
                };
                (time, 56.0, StageStats::InstructionScheduling { instructions_scheduled: 8000 })
            }
            OptimizationStage::TargetOptimization => {
                let time = match config.optimization_level {
                    0 | 1 | 2 => 0,
                    3 => 180,
                    _ => 90,
                };
                (time, 72.0, StageStats::TargetOptimization { optimizations_applied: 150 })
            }
            OptimizationStage::CodeGeneration => {
                let time = match config.optimization_level {
                    0 => 30,
                    1 => 60,
                    2 => 90,
                    3 => 120,
                    _ => 75,
                };
                (time, 40.0, StageStats::CodeGeneration { native_instructions: 12000 })
            }
        };
        
        // Check performance constraints
        if let Some(max_time) = config.performance_requirements.max_compilation_time_ms {
            if execution_time_ms > max_time {
                return Err(VmError::Core(CoreError::InvalidState {
                    message: format!("Stage {} exceeded maximum compilation time", stage.name()),
                    current: format!("{}ms", execution_time_ms),
                    expected: format!("<= {}ms", max_time),
                }));
            }
        }
        
        if let Some(max_memory) = config.performance_requirements.max_memory_usage_mb {
            if memory_usage_mb > max_memory as f32 {
                return Err(VmError::Core(CoreError::InvalidState {
                    message: format!("Stage {} exceeded maximum memory usage", stage.name()),
                    current: format!("{}MB", memory_usage_mb),
                    expected: format!("<= {}MB", max_memory),
                }));
            }
        }
        
        Ok(StageExecutionResult {
            execution_time_ms,
            memory_usage_mb,
            stats,
        })
    }
    
    /// Update optimization statistics based on stage result
    fn update_optimization_stats(
        &self,
        stats: &mut OptimizationStats,
        stage_result: &StageExecutionResult,
    ) {
        match &stage_result.stats {
            StageStats::IrGeneration { ir_instructions } => {
                stats.instructions_optimized += *ir_instructions;
            }
            StageStats::BasicBlockOptimization { blocks_optimized } => {
                stats.basic_blocks_optimized += *blocks_optimized;
            }
            StageStats::RegisterAllocation { registers_used } => {
                stats.registers_allocated += *registers_used;
            }
            StageStats::InstructionScheduling { instructions_scheduled } => {
                stats.instructions_scheduled += *instructions_scheduled;
            }
            StageStats::TargetOptimization { optimizations_applied } => {
                stats.instructions_optimized += *optimizations_applied;
            }
            StageStats::CodeGeneration { native_instructions } => {
                stats.estimated_improvement_percent = 
                    (stats.instructions_optimized as f32 / *native_instructions as f32) * 100.0;
            }
        }
    }
}

impl Default for OptimizationPipelineDomainService {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of executing a single stage
#[derive(Debug, Clone)]
struct StageExecutionResult {
    execution_time_ms: u64,
    memory_usage_mb: f32,
    stats: StageStats,
}

/// Statistics for a specific stage
#[derive(Debug, Clone)]
enum StageStats {
    IrGeneration { ir_instructions: u32 },
    BasicBlockOptimization { blocks_optimized: u32 },
    RegisterAllocation { registers_used: u32 },
    InstructionScheduling { instructions_scheduled: u32 },
    TargetOptimization { optimizations_applied: u32 },
    CodeGeneration { native_instructions: u32 },
}

/// Pipeline configuration validation rule
struct PipelineConfigValidationRule;

impl OptimizationPipelineBusinessRule for PipelineConfigValidationRule {
    fn validate_pipeline_config(&self, config: &OptimizationPipelineConfig) -> VmResult<()> {
        // Validate optimization level
        if config.optimization_level > 3 {
            return Err(VmError::Core(CoreError::InvalidConfig {
                field: "optimization_level".to_string(),
                message: "Optimization level must be between 0 and 3".to_string(),
            }));
        }
        
        // Validate that at least IR generation and code generation are enabled
        if !config.is_stage_enabled(&OptimizationStage::IrGeneration) {
            return Err(VmError::Core(CoreError::InvalidConfig {
                field: "enabled_stages".to_string(),
                message: "IR Generation stage must be enabled".to_string(),
            }));
        }
        
        if !config.is_stage_enabled(&OptimizationStage::CodeGeneration) {
            return Err(VmError::Core(CoreError::InvalidConfig {
                field: "enabled_stages".to_string(),
                message: "Code Generation stage must be enabled".to_string(),
            }));
        }
        
        // Validate stage ordering
        let mut last_order = 0;
        for stage in &config.enabled_stages {
            if stage.order() <= last_order {
                return Err(VmError::Core(CoreError::InvalidConfig {
                    field: "enabled_stages".to_string(),
                    message: "Stages must be in correct order".to_string(),
                }));
            }
            last_order = stage.order();
        }
        
        Ok(())
    }
    
    fn validate_stage_execution(
        &self,
        _stage: &OptimizationStage,
        _config: &OptimizationPipelineConfig,
    ) -> VmResult<()> {
        Ok(())
    }
    
    fn should_continue_pipeline(
        &self,
        _current_stage: &OptimizationStage,
        _result: &PipelineExecutionResult,
        _config: &OptimizationPipelineConfig,
    ) -> VmResult<bool> {
        Ok(true)
    }
}

/// Stage execution validation rule
struct StageExecutionValidationRule;

impl OptimizationPipelineBusinessRule for StageExecutionValidationRule {
    fn validate_pipeline_config(&self, _config: &OptimizationPipelineConfig) -> VmResult<()> {
        Ok(())
    }
    
    fn validate_stage_execution(
        &self,
        stage: &OptimizationStage,
        config: &OptimizationPipelineConfig,
    ) -> VmResult<()> {
        // Check if stage is enabled
        if !config.is_stage_enabled(stage) {
            return Err(VmError::Core(CoreError::InvalidState {
                message: format!("Stage {} is not enabled in the configuration", stage.name()),
                current: "disabled".to_string(),
                expected: "enabled".to_string(),
            }));
        }
        
        Ok(())
    }
    
    fn should_continue_pipeline(
        &self,
        _current_stage: &OptimizationStage,
        _result: &PipelineExecutionResult,
        _config: &OptimizationPipelineConfig,
    ) -> VmResult<bool> {
        Ok(true)
    }
}

/// Pipeline continuation rule
struct PipelineContinuationRule;

impl OptimizationPipelineBusinessRule for PipelineContinuationRule {
    fn validate_pipeline_config(&self, _config: &OptimizationPipelineConfig) -> VmResult<()> {
        Ok(())
    }
    
    fn validate_stage_execution(
        &self,
        _stage: &OptimizationStage,
        _config: &OptimizationPipelineConfig,
    ) -> VmResult<()> {
        Ok(())
    }
    
    fn should_continue_pipeline(
        &self,
        _current_stage: &OptimizationStage,
        result: &PipelineExecutionResult,
        config: &OptimizationPipelineConfig,
    ) -> VmResult<bool> {
        // Check if we've exceeded maximum compilation time
        if let Some(max_time) = config.performance_requirements.max_compilation_time_ms {
            if result.total_time_ms > max_time {
                return Ok(false);
            }
        }
        
        // Check if we've exceeded maximum memory usage
        if let Some(max_memory) = config.performance_requirements.max_memory_usage_mb {
            if result.peak_memory_usage_mb > max_memory as f32 {
                return Ok(false);
            }
        }
        
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_services::events::MockDomainEventBus;
    
    #[test]
    fn test_create_pipeline_config() {
        let service = OptimizationPipelineDomainService::new();
        let perf_req = PerformanceRequirements::default();
        
        let config = service.create_pipeline_config(
            crate::GuestArch::X86_64,
            crate::GuestArch::ARM64,
            2,
            perf_req,
        ).expect("Failed to create pipeline config");
        
        assert_eq!(config.source_arch, crate::GuestArch::X86_64);
        assert_eq!(config.target_arch, crate::GuestArch::ARM64);
        assert_eq!(config.optimization_level, 2);
        assert_eq!(config.enabled_stages.len(), 4);
        assert!(config.is_stage_enabled(&OptimizationStage::IrGeneration));
        assert!(config.is_stage_enabled(&OptimizationStage::CodeGeneration));
    }
    
    #[test]
    fn test_pipeline_config_validation() {
        let service = OptimizationPipelineDomainService::new();
        let perf_req = PerformanceRequirements::default();
        
        // Test invalid optimization level
        let result = service.create_pipeline_config(
            crate::GuestArch::X86_64,
            crate::GuestArch::ARM64,
            5, // Invalid level
            perf_req.clone(),
        );
        assert!(result.is_err());
        
        // Test valid optimization levels
        for level in 0..=3 {
            let result = service.create_pipeline_config(
                crate::GuestArch::X86_64,
                crate::GuestArch::ARM64,
                level,
                perf_req.clone(),
            );
            assert!(result.is_ok());
        }
    }
    
    #[test]
    fn test_execute_pipeline() {
        let service = OptimizationPipelineDomainService::new();
        let perf_req = PerformanceRequirements::default();
        let config = service.create_pipeline_config(
            crate::GuestArch::X86_64,
            crate::GuestArch::ARM64,
            2,
            perf_req,
        ).expect("Failed to create pipeline config");
        
        let mut aggregate = VirtualMachineAggregate::new(
            "test-vm".to_string(),
            crate::GuestArch::X86_64,
            1024 * 1024 * 1024, // 1GB
        );
        
        let result = service.execute_pipeline(&config, &mut aggregate).expect("Failed to execute pipeline");
        
        assert!(result.success);
        assert_eq!(result.completed_stages.len(), 4);
        assert!(result.total_time_ms > 0);
        assert!(result.peak_memory_usage_mb > 0.0);
    }
    
    #[test]
    fn test_pipeline_with_performance_constraints() {
        let service = OptimizationPipelineDomainService::new();
        let perf_req = PerformanceRequirements {
            max_compilation_time_ms: Some(100), // Very low limit
            max_memory_usage_mb: Some(1024),
            target_speedup: None,
            priority: OptimizationPriority::Speed,
        };
        
        let config = service.create_pipeline_config(
            crate::GuestArch::X86_64,
            crate::GuestArch::ARM64,
            3, // Highest optimization level
            perf_req,
        ).expect("Failed to create pipeline config with high optimization level");
        
        let mut aggregate = VirtualMachineAggregate::new(
            "test-vm".to_string(),
            crate::GuestArch::X86_64,
            1024 * 1024 * 1024,
        );
        
        let result = service.execute_pipeline(&config, &mut aggregate).expect("Failed to execute pipeline");
        
        // Pipeline should stop early due to time constraint
        assert!(!result.success || result.completed_stages.len() < 6);
    }
    
    #[test]
    fn test_optimization_stage_ordering() {
        assert!(OptimizationStage::IrGeneration.order() < OptimizationStage::CodeGeneration.order());
        assert!(OptimizationStage::BasicBlockOptimization.order() < OptimizationStage::RegisterAllocation.order());
        assert!(OptimizationStage::InstructionScheduling.order() < OptimizationStage::TargetOptimization.order());
    }
    
    #[test]
    fn test_next_stage() {
        let config = OptimizationPipelineConfig::new(
            crate::GuestArch::X86_64,
            crate::GuestArch::ARM64,
            2,
        );
        
        assert_eq!(
            config.next_stage(&OptimizationStage::IrGeneration),
            Some(OptimizationStage::BasicBlockOptimization)
        );
        
        assert_eq!(
            config.next_stage(&OptimizationStage::CodeGeneration),
            None
        );
    }
    
    #[test]
    fn test_pipeline_with_event_bus() {
        let event_bus = Arc::new(crate::domain_services::events::MockDomainEventBus::new());
        let service = OptimizationPipelineDomainService::new().with_event_bus(event_bus.clone());
        
        let perf_req = PerformanceRequirements::default();
        let config = service.create_pipeline_config(
            crate::GuestArch::X86_64,
            crate::GuestArch::ARM64,
            1,
            perf_req,
        ).expect("Failed to create pipeline config with optimization level 1");
        
        let mut aggregate = VirtualMachineAggregate::new(
            "test-vm".to_string(),
            crate::GuestArch::X86_64,
            1024 * 1024 * 1024,
        );
        
        let result = service.execute_pipeline(&config, &mut aggregate).expect("Failed to execute pipeline");
        
        assert!(result.success);
        assert!(event_bus.published_events().len() > 0);
    }
}