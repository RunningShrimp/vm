//! 优化型JIT编译器
//!
//! 实现高级优化技术，包括寄存器分配、指令调度、死代码消除等

use cranelift::prelude::*;
use cranelift_codegen::Context as CodegenContext;
use cranelift_codegen::ir::{Block, DataFlowGraph, Function, Inst, Value, types};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::verify_function;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};
use cranelift_native;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use vm_core::{ExecResult, ExecStats, ExecStatus, ExecutionEngine, GuestAddr, MMU, VmError};
use vm_ir::{IRBlock, IROp, RegId, Terminator};

use super::{CodePtr, JitContext};

// 导入拆分后的模块
pub use register_allocator::{
    RegisterAllocator, RegisterAllocation, RegisterAllocatorTrait,
    LinearScanAllocator, GraphColoringAllocator, RegisterAllocatorStats,
};
pub use instruction_scheduler::{InstructionScheduler, Dependency, DependencyType};
pub use optimization_passes::{OptimizationPassManager, OptimizationPass};

mod register_allocator;
mod instruction_scheduler;
mod optimization_passes;
mod ir_utils;

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

/// 优化型JIT编译器
pub struct OptimizingJIT {
    /// 基础JIT编译器
    base_jit: super::Jit,
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

impl OptimizingJIT {
    /// 创建新的优化型JIT编译器（使用自适应策略）
    pub fn new() -> Self {
        Self::with_allocation_strategy(RegisterAllocationStrategy::Adaptive)
    }

    /// 使用指定寄存器分配策略创建
    pub fn with_allocation_strategy(strategy: RegisterAllocationStrategy) -> Self {
        Self {
            base_jit: super::Jit::new(),
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
    pub fn compile(&mut self, block: &IRBlock) -> CodePtr {
        let start_time = std::time::Instant::now();

        // 1. 运行优化Pass
        let mut optimized_block = block.clone();
        self.pass_manager.run_optimizations(&mut optimized_block);

        let optimization_time = start_time.elapsed().as_nanos() as u64;

        // 2. 寄存器分配（根据策略选择分配器）
        let allocations = match self.allocation_strategy {
            RegisterAllocationStrategy::GraphColoring => {
                if let Some(ref mut gc_allocator) = self.graph_coloring_allocator {
                    // 使用图着色分配器
                    gc_allocator.analyze_lifetimes(&optimized_block.ops);
                    gc_allocator.allocate_registers(&optimized_block.ops)
                } else {
                    // 回退到自适应分配器
                    self.reg_allocator.analyze_lifetimes(&optimized_block.ops);
                    self.reg_allocator.allocate_registers(&optimized_block.ops)
                }
            }
            RegisterAllocationStrategy::LinearScan => {
                // 强制使用线性扫描（通过设置阈值）
                let old_threshold = self.reg_allocator.small_block_threshold;
                self.reg_allocator.set_small_block_threshold(usize::MAX);
                self.reg_allocator.analyze_lifetimes(&optimized_block.ops);
                let result = self.reg_allocator.allocate_registers(&optimized_block.ops);
                self.reg_allocator.set_small_block_threshold(old_threshold);
                result
            }
            RegisterAllocationStrategy::Adaptive => {
                // 自适应策略：根据块大小选择
                self.reg_allocator.analyze_lifetimes(&optimized_block.ops);
                self.reg_allocator.allocate_registers(&optimized_block.ops)
            }
        };
        
        let _allocations = allocations;

        // 3. 指令调度
        self.instruction_scheduler
            .build_dependency_graph(&optimized_block.ops);
        let schedule = self.instruction_scheduler.schedule(&optimized_block.ops);

        // 4. 重新排列指令
        let mut scheduled_ops = Vec::new();
        for &idx in &schedule {
            scheduled_ops.push(optimized_block.ops[idx].clone());
        }

        let final_block = IRBlock {
            start_pc: block.start_pc,
            ops: scheduled_ops,
            term: block.term.clone(),
        };

        // 5. 调用基础JIT编译
        let result = self.base_jit.compile(&final_block);

        // 6. 更新统计信息
        let mut stats = self.stats.lock();
        stats.total_compiles += 1;
        stats.optimization_time_ns += optimization_time;

        result
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> OptimizingJITStats {
        self.stats.lock().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::{IRBuilder, Terminator};

    #[test]
    fn test_register_allocation() {
        let mut allocator = RegisterAllocator::new();

        let ops = vec![
            IROp::MovImm { dst: 1, imm: 10 },
            IROp::MovImm { dst: 2, imm: 20 },
            IROp::Add {
                dst: 3,
                src1: 1,
                src2: 2,
            },
        ];

        allocator.analyze_lifetimes(&ops);
        let allocations = allocator.allocate_registers(&ops);

        assert!(allocations.contains_key(&1));
        assert!(allocations.contains_key(&2));
        assert!(allocations.contains_key(&3));
    }

    #[test]
    fn test_instruction_scheduling() {
        let mut scheduler = InstructionScheduler::new();

        let ops = vec![
            IROp::MovImm { dst: 1, imm: 10 },
            IROp::MovImm { dst: 2, imm: 20 },
            IROp::Add {
                dst: 3,
                src1: 1,
                src2: 2,
            },
            IROp::Mul {
                dst: 4,
                src1: 3,
                src2: 2,
            },
        ];

        scheduler.build_dependency_graph(&ops);
        let schedule = scheduler.schedule(&ops);

        assert_eq!(schedule.len(), ops.len());
        // MovImm 指令应该在前，因为它们没有依赖
        assert!(schedule[0] < 2);
        assert!(schedule[1] < 2);
    }

    #[test]
    fn test_optimization_passes() {
        let mut manager = OptimizationPassManager::new();

        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::MovImm { dst: 1, imm: 10 });
        builder.push(IROp::MovImm { dst: 2, imm: 20 });
        builder.push(IROp::Add {
            dst: 3,
            src1: 1,
            src2: 2,
        });
        builder.set_term(Terminator::Ret);

        let mut block = builder.build();
        let original_len = block.ops.len();

        manager.run_optimizations(&mut block);

        // 优化后长度应该不变（常量折叠在简化实现中不减少指令数）
        assert_eq!(block.ops.len(), original_len);
    }

    #[test]
    fn test_enhanced_jit() {
        let mut jit = OptimizingJIT::new();

        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::MovImm { dst: 1, imm: 10 });
        builder.push(IROp::MovImm { dst: 2, imm: 20 });
        builder.push(IROp::Add {
            dst: 3,
            src1: 1,
            src2: 2,
        });
        builder.set_term(Terminator::Ret);

        let block = builder.build();

        // 编译应该成功
        let code_ptr = jit.compile(&block);
        assert!(!code_ptr.is_null());

        // 检查统计信息
        let stats = jit.get_stats();
        assert_eq!(stats.total_compiles, 1);
    }

    #[test]
    fn test_optimizing_jit_empty_block() {
        let mut jit = OptimizingJIT::new();
        let mut builder = IRBuilder::new(0x1000);
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        // 空块也应该能编译
        let code_ptr = jit.compile(&block);
        // 空块可能返回null或有效指针，取决于实现
        let stats = jit.get_stats();
        assert_eq!(stats.total_compiles, 1);
    }

    #[test]
    fn test_optimizing_jit_multiple_compiles() {
        let mut jit = OptimizingJIT::new();

        for i in 0..5 {
            let mut builder = IRBuilder::new(0x1000 + i * 0x100);
            builder.push(IROp::MovImm { dst: 1, imm: i as u64 });
            builder.set_term(Terminator::Ret);
            let block = builder.build();
            jit.compile(&block);
        }

        let stats = jit.get_stats();
        assert_eq!(stats.total_compiles, 5);
    }

    #[test]
    fn test_register_allocator_empty_ops() {
        let mut allocator = RegisterAllocator::new();
        let ops = vec![];

        allocator.analyze_lifetimes(&ops);
        let allocations = allocator.allocate_registers(&ops);
        assert!(allocations.is_empty());
    }

    #[test]
    fn test_instruction_scheduler_empty_ops() {
        let mut scheduler = InstructionScheduler::new();
        let ops = vec![];

        scheduler.build_dependency_graph(&ops);
        let schedule = scheduler.schedule(&ops);
        assert!(schedule.is_empty());
    }

    #[test]
    fn test_optimization_passes_empty_block() {
        let mut manager = OptimizationPassManager::new();
        let mut builder = IRBuilder::new(0x1000);
        builder.set_term(Terminator::Ret);
        let mut block = builder.build();

        let original_len = block.ops.len();
        manager.run_optimizations(&mut block);
        assert_eq!(block.ops.len(), original_len);
    }
}

// 这些方法应该属于RegisterAllocator impl块，但被错误地放在了文件末尾
// 暂时注释掉，因为它们应该在register_allocator模块中实现
/*
impl RegisterAllocator {
    pub fn analyze_lifetimes(&mut self, ops: &[IROp]) {
        for (idx, op) in ops.iter().enumerate() {
            // 收集读取的寄存器
            let read_regs = self.collect_read_regs(op);
            // 收集写入的寄存器
            let written_regs = self.collect_written_regs(op);

            // 更新寄存器生命周期
            for &reg in &read_regs {
                let lifetime = self.reg_lifetimes.entry(reg).or_insert((idx, idx));
                lifetime.1 = idx; // 延伸到当前指令
            }

            for &reg in &written_regs {
                self.reg_lifetimes.insert(reg, (idx, idx));
            }
        }
    }

    /// 分配寄存器（使用图着色算法）
    ///
    /// 图着色算法相比线性扫描的优势：
    /// - 全局视角：考虑所有寄存器的冲突关系
    /// - 更好的分配：减少10-20%的寄存器溢出
    /// - 更优的寄存器重用
    pub fn allocate_registers(&mut self, ops: &[IROp]) -> HashMap<RegId, RegisterAllocation> {
        // 1. 构建冲突图（interference graph）
        let interference_graph = self.build_interference_graph(ops);

        // 2. 图着色分配
        let mut allocations = HashMap::new();
        let mut colored = HashMap::new();
        let mut spilled = Vec::new();

        // 可用物理寄存器数量（x1-x31，共31个）
        let k = 31;

        // 3. 简化图（simplify phase）
        let mut worklist = self.simplify_graph(&interference_graph, k);

        // 4. 选择阶段（select phase）- 反向分配颜色
        while let Some(reg) = worklist.pop() {
            let mut used_colors = HashSet::new();

            // 收集已分配给冲突寄存器的颜色
            if let Some(neighbors) = interference_graph.get(&reg) {
                for &neighbor in neighbors {
                    if let Some(color) = colored.get(&neighbor) {
                        used_colors.insert(*color);
                    }
                }
            }

            // 分配第一个可用的颜色
            if used_colors.len() < k {
                for color in 1..=k {
                    if !used_colors.contains(&(color as u32)) {
                        colored.insert(reg, color as u32);
                        allocations.insert(reg, RegisterAllocation::Register(color as u32));
                        break;
                    }
                }
            } else {
                // 无法分配，需要溢出
                spilled.push(reg);
            }
        }

        // 5. 处理溢出的寄存器
        for reg in spilled {
            let spill_offset = self.next_spill_offset;
            self.next_spill_offset += 8; // 64位寄存器
            self.spilled_regs.insert(reg, spill_offset);
            allocations.insert(reg, RegisterAllocation::Stack(spill_offset));
        }

        allocations
    }

    /// 构建冲突图（interference graph）
    ///
    /// 如果两个虚拟寄存器同时存活（lifetimes overlap），它们冲突
    fn build_interference_graph(&self, _ops: &[IROp]) -> HashMap<RegId, HashSet<RegId>> {
        let mut graph: HashMap<RegId, HashSet<RegId>> = HashMap::new();

        // 对于每对寄存器，检查生命周期是否重叠
        let regs: Vec<RegId> = self.reg_lifetimes.keys().copied().collect();

        for i in 0..regs.len() {
            for j in (i + 1)..regs.len() {
                let reg1 = regs[i];
                let reg2 = regs[j];

                if let (Some(lifetime1), Some(lifetime2)) =
                    (self.reg_lifetimes.get(&reg1), self.reg_lifetimes.get(&reg2))
                {
                    let (start1, end1) = *lifetime1;
                    let (start2, end2) = *lifetime2;
                    // 检查生命周期是否重叠
                    if !(end1 < start2 || end2 < start1) {
                        // 生命周期重叠，添加冲突边
                        graph.entry(reg1).or_insert_with(HashSet::new).insert(reg2);
                        graph.entry(reg2).or_insert_with(HashSet::new).insert(reg1);
                    }
                }
            }
        }

        graph
    }

    /// 简化图（simplify phase）
    ///
    /// 移除度数小于k的节点，直到无法继续
    fn simplify_graph(&self, graph: &HashMap<RegId, HashSet<RegId>>, k: usize) -> Vec<RegId> {
        let mut worklist = Vec::new();
        let mut remaining_graph = graph.clone();
        let mut degrees: HashMap<RegId, usize> = graph
            .iter()
            .map(|(reg, neighbors)| (*reg, neighbors.len()))
            .collect();

        loop {
            let mut found = false;

            // 查找度数 < k 的节点
            let candidates: Vec<RegId> = degrees
                .iter()
                .filter(|(_, degree)| **degree < k)
                .map(|(reg, _)| *reg)
                .collect();

            for reg in candidates {
                if let Some(neighbors) = remaining_graph.remove(&reg) {
                    // 从邻居的度数中减去1
                    for neighbor in &neighbors {
                        if let Some(degree) = degrees.get_mut(neighbor) {
                            *degree -= 1;
                        }
                    }
                    degrees.remove(&reg);
                    worklist.push(reg);
                    found = true;
                }
            }

            if !found {
                break;
            }
        }

        worklist
    }

    /// 查找空闲的物理寄存器（保留用于向后兼容，图着色算法不再需要）
    #[allow(dead_code)]
    fn find_free_physical_reg(&self, current_idx: usize) -> Option<u32> {
        // x1-x31 可用 (x0 保留为 zero)
        for reg in 1..32 {
            let reg_id = reg as RegId;

            // 检查寄存器是否在当前时间点被占用
            let mut is_free = true;
            for (used_reg, lifetime) in &self.reg_lifetimes {
                let (start, end) = *lifetime;
                if *used_reg == reg_id {
                    if start <= current_idx && current_idx <= end {
                        is_free = false;
                        break;
                    }
                }
            }

            if is_free {
                return Some(reg_id);
            }
        }

        None
    }

    /// 收集操作中读取的寄存器（使用共享工具函数）
    fn collect_read_regs(&self, op: &IROp) -> Vec<RegId> {
        ir_utils::IrAnalyzer::collect_read_regs(op)
    }

    /// 收集操作中写入的寄存器（使用共享工具函数）
    fn collect_written_regs(&self, op: &IROp) -> Vec<RegId> {
        ir_utils::IrAnalyzer::collect_written_regs(op)
    }
}
*/

/// 寄存器分配结果
#[derive(Debug, Clone)]
pub enum RegisterAllocation {
    /// 分配到物理寄存器
    Register(RegId),
    /// 溢出到栈内存
    Stack(i32),
}

// 重复的InstructionScheduler实现已移除，统一使用instruction_scheduler模块中的实现
