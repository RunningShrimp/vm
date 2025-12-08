//! 优化型JIT编译器模块
//!
//! 包含寄存器分配、指令调度、优化Pass等子模块

pub mod register_allocator;
pub mod instruction_scheduler;
pub mod optimization_passes;
pub mod ir_utils;

// 重新导出主要类型
pub use register_allocator::{
    RegisterAllocator, RegisterAllocation, RegisterAllocatorTrait,
    LinearScanAllocator, GraphColoringAllocator, StubGraphColoringAllocator,
    RegisterAllocatorStats,
};

pub use instruction_scheduler::{InstructionScheduler, Dependency, DependencyType};

pub use optimization_passes::{OptimizationPassManager, OptimizationPass};