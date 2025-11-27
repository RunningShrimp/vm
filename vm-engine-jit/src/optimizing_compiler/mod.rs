//! 优化型JIT编译器模块
//!
//! 包含寄存器分配、指令调度、优化Pass等子模块

pub mod register_allocator;
pub mod instruction_scheduler;
pub mod optimization_passes;
pub mod ir_utils;

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::Mutex;
use vm_ir::IRBlock;

// 重新导出主要类型
pub use register_allocator::{
    RegisterAllocator, RegisterAllocation, RegisterAllocatorTrait,
    LinearScanAllocator, GraphColoringAllocator, StubGraphColoringAllocator,
    RegisterAllocatorStats,
};

pub use instruction_scheduler::{InstructionScheduler, Dependency, DependencyType};

pub use optimization_passes::{OptimizationPassManager, OptimizationPass};

/// 寄存器分配策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAllocationStrategy {
    /// 自适应策略（根据块大小自动选择）
    Adaptive,
    /// 线性扫描（快速，适用于小块）
    LinearScan,
    /// 图着色（更优，适用于大块）
    GraphColoring,
}

/// 优化型JIT统计信息
#[derive(Debug, Clone, Default)]
pub struct OptimizingJITStats {
    /// 总编译次数
    pub total_compiles: u64,
    /// 寄存器溢出次数
    pub register_spills: u64,
    /// 指令调度重排次数
    pub instruction_reorders: u64,
    /// 优化Pass执行时间 (纳秒)
    pub optimization_time_ns: u64,
    /// 各优化Pass执行次数
    pub pass_executions: HashMap<String, u64>,
}

/// 优化型JIT编译器
pub struct OptimizingJIT {
    /// 寄存器分配器（自适应）
    reg_allocator: RegisterAllocator,
    /// 图着色分配器（可选，用于大块）
    graph_coloring_allocator: Option<GraphColoringAllocator>,
    /// 寄存器分配策略
    allocation_strategy: RegisterAllocationStrategy,
    /// 指令调度器
    instruction_scheduler: InstructionScheduler,
    /// 优化Pass管理器
    pass_manager: OptimizationPassManager,
    /// 编译统计
    stats: Arc<Mutex<OptimizingJITStats>>,
}

impl OptimizingJIT {
    /// 创建新的优化型JIT编译器（使用自适应策略）
    pub fn new() -> Self {
        Self::with_allocation_strategy(RegisterAllocationStrategy::Adaptive)
    }

    /// 使用指定寄存器分配策略创建
    pub fn with_allocation_strategy(strategy: RegisterAllocationStrategy) -> Self {
        Self {
            reg_allocator: RegisterAllocator::new(),
            graph_coloring_allocator: match strategy {
                RegisterAllocationStrategy::GraphColoring => Some(GraphColoringAllocator::new()),
                _ => None,
            },
            allocation_strategy: strategy,
            instruction_scheduler: InstructionScheduler::new(),
            pass_manager: OptimizationPassManager::new(),
            stats: Arc::new(Mutex::new(OptimizingJITStats::default())),
        }
    }

    /// 设置寄存器分配策略
    pub fn set_allocation_strategy(&mut self, strategy: RegisterAllocationStrategy) {
        self.allocation_strategy = strategy;
        match strategy {
            RegisterAllocationStrategy::GraphColoring => {
                if self.graph_coloring_allocator.is_none() {
                    self.graph_coloring_allocator = Some(GraphColoringAllocator::new());
                }
            }
            _ => {
                self.graph_coloring_allocator = None;
            }
        }
    }

    /// 编译IR块
    pub fn compile(&mut self, block: &IRBlock) -> *const u8 {
        let start_time = std::time::Instant::now();

        // 1. 运行优化Pass
        let mut optimized_block = block.clone();
        self.pass_manager.run_optimizations(&mut optimized_block);

        let optimization_time = start_time.elapsed().as_nanos() as u64;

        // 2. 寄存器分配（根据策略选择分配器）
        let _allocations = match self.allocation_strategy {
            RegisterAllocationStrategy::GraphColoring => {
                if let Some(ref mut gc_allocator) = self.graph_coloring_allocator {
                    gc_allocator.analyze_lifetimes(&optimized_block.ops);
                    gc_allocator.allocate_registers(&optimized_block.ops)
                } else {
                    self.reg_allocator.analyze_lifetimes(&optimized_block.ops);
                    self.reg_allocator.allocate_registers(&optimized_block.ops)
                }
            }
            RegisterAllocationStrategy::LinearScan => {
                // 使用线性扫描策略
                self.reg_allocator.analyze_lifetimes(&optimized_block.ops);
                self.reg_allocator.allocate_registers(&optimized_block.ops)
            }
            RegisterAllocationStrategy::Adaptive => {
                self.reg_allocator.analyze_lifetimes(&optimized_block.ops);
                self.reg_allocator.allocate_registers(&optimized_block.ops)
            }
        };

        // 3. 指令调度
        self.instruction_scheduler.build_dependency_graph(&optimized_block.ops);
        let _schedule = self.instruction_scheduler.schedule(&optimized_block.ops);

        // 4. 更新统计信息
        let mut stats = self.stats.lock();
        stats.total_compiles += 1;
        stats.optimization_time_ns += optimization_time;

        // 返回空指针（实际编译需要后端支持）
        std::ptr::null()
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> OptimizingJITStats {
        self.stats.lock().clone()
    }
}

impl Default for OptimizingJIT {
    fn default() -> Self {
        Self::new()
    }
}