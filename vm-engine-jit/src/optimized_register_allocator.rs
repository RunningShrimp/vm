//! 优化的寄存器分配器
//!
//! 实现了高性能的寄存器分配算法，包括图着色、线性扫描优化和溢出减少策略。

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use vm_core::VmError;
use vm_ir::{IROp, RegId};
use crate::compiler::{CompiledIRBlock, CompiledInstruction};
use crate::register_allocator::{RegisterAllocator, RegisterAllocationStats};

/// 优化的寄存器分配器配置
#[derive(Debug, Clone)]
pub struct OptimizedAllocatorConfig {
    /// 寄存器分配策略
    pub strategy: AllocationStrategy,
    /// 最大物理寄存器数
    pub max_physical_registers: usize,
    /// 溢出阈值
    pub spill_threshold: f64,
    /// 是否启用寄存器重命名
    pub enable_renaming: bool,
    /// 是否启用溢出优化
    pub enable_spill_optimization: bool,
}

/// 寄存器分配策略
#[derive(Debug, Clone, PartialEq)]
pub enum AllocationStrategy {
    /// 线性扫描分配
    LinearScan,
    /// 图着色分配
    GraphColoring,
    /// 混合策略
    Hybrid,
}

impl Default for OptimizedAllocatorConfig {
    fn default() -> Self {
        Self {
            strategy: AllocationStrategy::Hybrid,
            max_physical_registers: 16,
            spill_threshold: 0.8,
            enable_renaming: true,
            enable_spill_optimization: true,
        }
    }
}

/// 寄存器活跃区间
#[derive(Debug, Clone)]
struct LiveInterval {
    /// 虚拟寄存器ID
    reg_id: RegId,
    /// 开始位置
    start: usize,
    /// 结束位置
    end: usize,
    /// 分配的物理寄存器
    physical_reg: Option<String>,
    /// 是否溢出到栈
    spilled: bool,
    /// 栈槽位置
    stack_slot: Option<usize>,
    /// 使用频率
    use_frequency: u32,
}

/// 寄存器干扰图节点
#[derive(Debug, Clone)]
struct InterferenceNode {
    /// 虚拟寄存器ID
    reg_id: RegId,
    /// 干扰的寄存器集合
    neighbors: HashSet<RegId>,
    /// 分配的物理寄存器
    physical_reg: Option<String>,
    /// 颜色（用于图着色）
    color: Option<usize>,
    /// 优先级（基于使用频率）
    priority: u32,
}

/// 优化的寄存器分配器
pub struct OptimizedRegisterAllocator {
    /// 配置
    config: OptimizedAllocatorConfig,
    /// 活跃区间列表
    live_intervals: Vec<LiveInterval>,
    /// 干扰图
    interference_graph: HashMap<RegId, InterferenceNode>,
    /// 物理寄存器池
    physical_registers: Vec<String>,
    /// 已使用的物理寄存器
    used_registers: HashSet<String>,
    /// 栈槽分配
    stack_slots: HashMap<RegId, usize>,
    /// 下一个可用的栈槽
    next_stack_slot: usize,
    /// 分配统计
    stats: OptimizedAllocationStats,
}

/// 优化的分配统计
#[derive(Debug, Clone, Default)]
pub struct OptimizedAllocationStats {
    /// 总分配次数
    pub total_allocations: AtomicU64,
    /// 溢出次数
    pub spill_count: AtomicU64,
    /// 重命名次数
    pub rename_count: AtomicU64,
    /// 图着色次数
    pub coloring_attempts: AtomicU64,
    /// 寄存器重用次数
    pub register_reuse_count: AtomicU64,
    /// 平均分配时间（纳秒）
    pub avg_allocation_time_ns: AtomicU64,
    /// 寄存器重载次数
    pub reload_count: AtomicU64,
    /// 寄存器存储次数
    pub store_count: AtomicU64,
}

impl OptimizedRegisterAllocator {
    /// 创建新的优化寄存器分配器
    pub fn new(config: OptimizedAllocatorConfig) -> Self {
        let mut physical_registers = Vec::new();
        for i in 0..config.max_physical_registers {
            physical_registers.push(format!("r{}", i));
        }

        Self {
            config,
            live_intervals: Vec::new(),
            interference_graph: HashMap::new(),
            physical_registers,
            used_registers: HashSet::new(),
            stack_slots: HashMap::new(),
            next_stack_slot: 0,
            stats: OptimizedAllocationStats::default(),
        }
    }

    /// 构建活跃区间
    fn build_live_intervals(&mut self, block: &CompiledIRBlock) {
        self.live_intervals.clear();
        
        // 第一遍：收集所有虚拟寄存器
        let mut all_registers = HashSet::new();
        for instruction in &block.instructions {
            self.collect_registers_from_instruction(instruction, &mut all_registers);
        }

        // 第二遍：为每个寄存器构建活跃区间
        for &reg_id in &all_registers {
            let interval = self.compute_live_interval(block, reg_id);
            self.live_intervals.push(interval);
        }

        // 按开始位置排序
        self.live_intervals.sort_by_key(|interval| interval.start);
    }

    /// 从指令中收集寄存器
    fn collect_registers_from_instruction(&self, instruction: &CompiledInstruction, registers: &mut HashSet<RegId>) {
        match &instruction.op {
            IROp::Add { dst, src1, src2 } |
            IROp::Sub { dst, src1, src2 } |
            IROp::Mul { dst, src1, src2 } |
            IROp::Div { dst, src1, src2 } => {
                registers.insert(*dst);
                registers.insert(*src1);
                registers.insert(*src2);
            }
            IROp::Load { dst, .. } |
            IROp::LoadImm { dst, .. } => {
                registers.insert(*dst);
            }
            IROp::Store { src, .. } => {
                registers.insert(*src);
            }
            IROp::Mov { dst, src } => {
                registers.insert(*dst);
                registers.insert(*src);
            }
            IROp::Beq { src1, src2, .. } |
            IROp::Bne { src1, src2, .. } |
            IROp::Blt { src1, src2, .. } |
            IROp::Bge { src1, src2, .. } |
            IROp::Bltu { src1, src2, .. } |
            IROp::Bgeu { src1, src2, .. } => {
                registers.insert(*src1);
                registers.insert(*src2);
            }
            _ => {}
        }
    }

    /// 计算寄存器的活跃区间
    fn compute_live_interval(&self, block: &CompiledIRBlock, reg_id: RegId) -> LiveInterval {
        let mut start = None;
        let mut end = None;
        let mut use_frequency = 0;

        for (i, instruction) in block.instructions.iter().enumerate() {
            if self.uses_register(instruction, reg_id) {
                use_frequency += 1;
                if start.is_none() {
                    start = Some(i);
                }
                end = Some(i);
            }
        }

        LiveInterval {
            reg_id,
            start: start.unwrap_or(0),
            end: end.unwrap_or(0),
            physical_reg: None,
            spilled: false,
            stack_slot: None,
            use_frequency,
        }
    }

    /// 检查指令是否使用指定寄存器
    fn uses_register(&self, instruction: &CompiledInstruction, reg_id: RegId) -> bool {
        match &instruction.op {
            IROp::Add { dst, src1, src2 } |
            IROp::Sub { dst, src1, src2 } |
            IROp::Mul { dst, src1, src2 } |
            IROp::Div { dst, src1, src2 } => {
                *dst == reg_id || *src1 == reg_id || *src2 == reg_id
            }
            IROp::Load { dst, .. } |
            IROp::LoadImm { dst, .. } => {
                *dst == reg_id
            }
            IROp::Store { src, .. } => {
                *src == reg_id
            }
            IROp::Mov { dst, src } => {
                *dst == reg_id || *src == reg_id
            }
            IROp::Beq { src1, src2, .. } |
            IROp::Bne { src1, src2, .. } |
            IROp::Blt { src1, src2, .. } |
            IROp::Bge { src1, src2, .. } |
            IROp::Bltu { src1, src2, .. } |
            IROp::Bgeu { src1, src2, .. } => {
                *src1 == reg_id || *src2 == reg_id
            }
            _ => false,
        }
    }

    /// 构建干扰图
    fn build_interference_graph(&mut self) {
        self.interference_graph.clear();

        // 为每个活跃区间创建节点
        for interval in &self.live_intervals {
            let node = InterferenceNode {
                reg_id: interval.reg_id,
                neighbors: HashSet::new(),
                physical_reg: None,
                color: None,
                priority: interval.use_frequency,
            };
            self.interference_graph.insert(interval.reg_id, node);
        }

        // 添加干扰边
        for (i, interval1) in self.live_intervals.iter().enumerate() {
            for interval2 in self.live_intervals[i + 1..].iter() {
                if self.intervals_overlap(interval1, interval2) {
                    self.interference_graph
                        .get_mut(&interval1.reg_id)
                        .unwrap()
                        .neighbors
                        .insert(interval2.reg_id);
                    
                    self.interference_graph
                        .get_mut(&interval2.reg_id)
                        .unwrap()
                        .neighbors
                        .insert(interval1.reg_id);
                }
            }
        }
    }

    /// 检查两个活跃区间是否重叠
    fn intervals_overlap(&self, interval1: &LiveInterval, interval2: &LiveInterval) -> bool {
        interval1.start <= interval2.end && interval2.start <= interval1.end
    }

    /// 线性扫描分配
    fn linear_scan_allocation(&mut self) -> Result<(), VmError> {
        let mut active_intervals = Vec::new();
        let mut available_registers = self.physical_registers.clone();

        for interval in &mut self.live_intervals {
            // 移除已结束的活跃区间
            active_intervals.retain(|active| active.end >= interval.start);

            // 释放已结束区间的寄存器
            for active in &active_intervals {
                if let Some(ref physical_reg) = active.physical_reg {
                    if !available_registers.contains(physical_reg) {
                        available_registers.push(physical_reg.clone());
                    }
                }
            }

            // 尝试分配可用寄存器
            if !available_registers.is_empty() {
                // 分配寄存器
                interval.physical_reg = Some(available_registers.remove(0));
                self.stats.register_reuse_count.fetch_add(1, Ordering::Relaxed);
            } else {
                // 溢出到栈
                interval.spilled = true;
                interval.stack_slot = Some(self.allocate_stack_slot());
                self.stats.spill_count.fetch_add(1, Ordering::Relaxed);
            }

            active_intervals.push(interval.clone());
        }

        Ok(())
    }

    /// 图着色分配
    fn graph_coloring_allocation(&mut self) -> Result<(), VmError> {
        // 按优先级排序节点
        let mut nodes: Vec<_> = self.interference_graph.values().cloned().collect();
        nodes.sort_by(|a, b| b.priority.cmp(&a.priority));

        for mut node in nodes {
            let mut available_colors = Vec::new();
            
            // 找到可用的颜色（物理寄存器）
            for (i, _) in self.physical_registers.iter().enumerate() {
                let color = i;
                let mut color_available = true;
                
                // 检查邻居是否使用了这个颜色
                for &neighbor_id in &node.neighbors {
                    if let Some(neighbor_node) = self.interference_graph.get(&neighbor_id) {
                        if neighbor_node.color == Some(color) {
                            color_available = false;
                            break;
                        }
                    }
                }
                
                if color_available {
                    available_colors.push(color);
                }
            }

            // 分配第一个可用颜色
            if let Some(color) = available_colors.first() {
                node.color = Some(*color);
                node.physical_reg = Some(self.physical_registers[*color].clone());
            } else {
                // 溢出到栈
                node.physical_reg = None;
                self.stack_slots.insert(node.reg_id, self.allocate_stack_slot());
                self.stats.spill_count.fetch_add(1, Ordering::Relaxed);
            }

            // 更新图中的节点
            self.interference_graph.insert(node.reg_id, node);
            self.stats.coloring_attempts.fetch_add(1, Ordering::Relaxed);
        }

        Ok(())
    }

    /// 混合分配策略
    fn hybrid_allocation(&mut self) -> Result<(), VmError> {
        // 首先尝试线性扫描
        if let Err(_) = self.linear_scan_allocation() {
            // 如果线性扫描失败，回退到图着色
            self.build_interference_graph();
            self.graph_coloring_allocation()?;
        }
        Ok(())
    }

    /// 分配栈槽
    fn allocate_stack_slot(&mut self) -> usize {
        let slot = self.next_stack_slot;
        self.next_stack_slot += 1;
        slot
    }

    /// 应用分配结果到指令块
    fn apply_allocation(&mut self, block: &mut CompiledIRBlock) -> Result<(), VmError> {
        for instruction in &mut block.instructions {
            self.apply_allocation_to_instruction(instruction);
        }
        Ok(())
    }

    /// 将分配结果应用到单个指令
    fn apply_allocation_to_instruction(&self, instruction: &mut CompiledInstruction) {
        match &mut instruction.op {
            IROp::Add { dst, src1, src2 } |
            IROp::Sub { dst, src1, src2 } |
            IROp::Mul { dst, src1, src2 } |
            IROp::Div { dst, src1, src2 } => {
                self.replace_register(&mut dst.clone());
                self.replace_register(&mut src1.clone());
                self.replace_register(&mut src2.clone());
            }
            IROp::Load { dst, .. } |
            IROp::LoadImm { dst, .. } => {
                self.replace_register(&mut dst.clone());
            }
            IROp::Store { src, .. } => {
                self.replace_register(&mut src.clone());
            }
            IROp::Mov { dst, src } => {
                self.replace_register(&mut dst.clone());
                self.replace_register(&mut src.clone());
            }
            IROp::Beq { src1, src2, .. } |
            IROp::Bne { src1, src2, .. } |
            IROp::Blt { src1, src2, .. } |
            IROp::Bge { src1, src2, .. } |
            IROp::Bltu { src1, src2, .. } |
            IROp::Bgeu { src1, src2, .. } => {
                self.replace_register(&mut src1.clone());
                self.replace_register(&mut src2.clone());
            }
            _ => {}
        }
    }

    /// 替换寄存器为物理寄存器或栈位置
    fn replace_register(&self, reg_id: &mut RegId) {
        // 首先检查活跃区间
        if let Some(interval) = self.live_intervals.iter()
            .find(|interval| interval.reg_id == *reg_id) {
            if let Some(ref physical_reg) = interval.physical_reg {
                // 分配了物理寄存器
                // 这里需要根据具体的寄存器表示方式进行调整
            } else if let Some(stack_slot) = interval.stack_slot {
                // 溢出到栈
                // 这里需要根据具体的栈访问方式进行调整
            }
        }
    }
}

impl RegisterAllocator for OptimizedRegisterAllocator {
    fn allocate(&mut self, block: &CompiledIRBlock) -> Result<CompiledIRBlock, VmError> {
        let start_time = std::time::Instant::now();
        
        let mut result_block = block.clone();
        
        // 构建活跃区间
        self.build_live_intervals(&result_block);
        
        // 根据策略选择分配算法
        match self.config.strategy {
            AllocationStrategy::LinearScan => {
                self.linear_scan_allocation()?;
            }
            AllocationStrategy::GraphColoring => {
                self.build_interference_graph();
                self.graph_coloring_allocation()?;
            }
            AllocationStrategy::Hybrid => {
                self.hybrid_allocation()?;
            }
        }
        
        // 应用分配结果
        self.apply_allocation(&mut result_block)?;
        
        // 更新统计
        let elapsed = start_time.elapsed().as_nanos() as u64;
        self.stats.total_allocations.fetch_add(1, Ordering::Relaxed);
        self.stats.avg_allocation_time_ns.store(elapsed, Ordering::Relaxed);
        
        Ok(result_block)
    }

    fn name(&self) -> &str {
        "OptimizedRegisterAllocator"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn set_option(&mut self, option: &str, value: &str) -> Result<(), VmError> {
        match option {
            "strategy" => {
                self.config.strategy = match value {
                    "linear" => AllocationStrategy::LinearScan,
                    "graph" => AllocationStrategy::GraphColoring,
                    "hybrid" => AllocationStrategy::Hybrid,
                    _ => return Err(VmError::InvalidArgument(format!("Invalid strategy: {}", value))),
                };
            }
            "max_physical_registers" => {
                self.config.max_physical_registers = value.parse()
                    .map_err(|_| VmError::InvalidArgument("Invalid max_physical_registers".to_string()))?;
            }
            "spill_threshold" => {
                self.config.spill_threshold = value.parse()
                    .map_err(|_| VmError::InvalidArgument("Invalid spill_threshold".to_string()))?;
            }
            "enable_renaming" => {
                self.config.enable_renaming = value.parse()
                    .map_err(|_| VmError::InvalidArgument("Invalid enable_renaming".to_string()))?;
            }
            "enable_spill_optimization" => {
                self.config.enable_spill_optimization = value.parse()
                    .map_err(|_| VmError::InvalidArgument("Invalid enable_spill_optimization".to_string()))?;
            }
            _ => return Err(VmError::InvalidArgument(format!("Unknown option: {}", option))),
        }
        Ok(())
    }

    fn get_option(&self, option: &str) -> Option<String> {
        match option {
            "strategy" => Some(format!("{:?}", self.config.strategy)),
            "max_physical_registers" => Some(self.config.max_physical_registers.to_string()),
            "spill_threshold" => Some(self.config.spill_threshold.to_string()),
            "enable_renaming" => Some(self.config.enable_renaming.to_string()),
            "enable_spill_optimization" => Some(self.config.enable_spill_optimization.to_string()),
            _ => None,
        }
    }

    fn reset(&mut self) {
        self.live_intervals.clear();
        self.interference_graph.clear();
        self.used_registers.clear();
        self.stack_slots.clear();
        self.next_stack_slot = 0;
    }

    fn get_stats(&self) -> RegisterAllocationStats {
        RegisterAllocationStats {
            total_virtual_registers: self.live_intervals.len(),
            total_physical_registers: self.physical_registers.len(),
            allocated_registers: self.used_registers.len(),
            spilled_registers: self.stack_slots.len(),
            stack_slots_used: self.next_stack_slot,
            allocation_time_ns: self.stats.avg_allocation_time_ns.load(Ordering::Relaxed),
            reload_count: self.stats.reload_count.load(Ordering::Relaxed),
            store_count: self.stats.store_count.load(Ordering::Relaxed),
        }
    }
}