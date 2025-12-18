//! 优化的指令调度器
//!
//! 实现了高性能的指令调度算法，包括依赖分析、关键路径优化和并行执行机会识别。

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use vm_core::VmError;
use vm_ir::IROp;
use crate::compiler::{CompiledIRBlock, CompiledInstruction};
use crate::instruction_scheduler::{InstructionScheduler, InstructionSchedulingStats};

/// 优化的调度器配置
#[derive(Debug, Clone)]
pub struct OptimizedSchedulerConfig {
    /// 调度策略
    pub strategy: SchedulingStrategy,
    /// 最大并行度
    pub max_parallelism: usize,
    /// 是否启用指令重排序
    pub enable_reordering: bool,
    /// 是否启用流水线优化
    pub enable_pipeline_optimization: bool,
    /// 延迟模型
    pub latency_model: LatencyModel,
}

/// 调度策略
#[derive(Debug, Clone, PartialEq)]
pub enum SchedulingStrategy {
    /// 列表调度
    ListScheduling,
    /// 关键路径调度
    CriticalPathScheduling,
    /// 贪婪调度
    GreedyScheduling,
    /// 动态调度
    DynamicScheduling,
}

/// 延迟模型
#[derive(Debug, Clone)]
pub struct LatencyModel {
    /// 加载指令延迟
    pub load_latency: u8,
    /// 存储指令延迟
    pub store_latency: u8,
    /// 算术指令延迟
    pub arithmetic_latency: u8,
    /// 分支指令延迟
    pub branch_latency: u8,
    /// 调用指令延迟
    pub call_latency: u8,
}

impl Default for OptimizedSchedulerConfig {
    fn default() -> Self {
        Self {
            strategy: SchedulingStrategy::CriticalPathScheduling,
            max_parallelism: 4,
            enable_reordering: true,
            enable_pipeline_optimization: true,
            latency_model: LatencyModel {
                load_latency: 3,
                store_latency: 2,
                arithmetic_latency: 1,
                branch_latency: 2,
                call_latency: 5,
            },
        }
    }
}

/// 指令依赖节点
#[derive(Debug, Clone)]
struct DependencyNode {
    /// 指令索引
    instruction_index: usize,
    /// 指令
    instruction: CompiledInstruction,
    /// 前驱节点（依赖的指令）
    predecessors: HashSet<usize>,
    /// 后继节点（依赖此指令的指令）
    successors: HashSet<usize>,
    /// 就绪时间
    ready_time: u32,
    /// 调度优先级
    priority: i32,
    /// 是否已调度
    scheduled: bool,
    /// 估计执行时间
    estimated_time: u32,
}

/// 资源使用信息
#[derive(Debug, Clone)]
struct ResourceUsage {
    /// 使用的寄存器
    registers: HashSet<String>,
    /// 使用的功能单元
    functional_units: HashSet<String>,
    /// 内存访问标记
    memory_access: bool,
}

/// 优化的指令调度器
pub struct OptimizedInstructionScheduler {
    /// 配置
    config: OptimizedSchedulerConfig,
    /// 依赖图
    dependency_graph: Vec<DependencyNode>,
    /// 资源使用跟踪
    resource_tracker: HashMap<usize, ResourceUsage>,
    /// 当前时间
    current_time: u32,
    /// 调度统计
    stats: OptimizedSchedulingStats,
}

/// 优化的调度统计
#[derive(Debug, Clone, Default)]
pub struct OptimizedSchedulingStats {
    /// 总调度指令数
    pub total_instructions: AtomicU64,
    /// 重排序指令数
    pub reordered_instructions: AtomicU64,
    /// 并行执行指令数
    pub parallel_instructions: AtomicU64,
    /// 关键路径长度
    pub critical_path_length: AtomicU64,
    /// 平均调度时间（纳秒）
    pub avg_scheduling_time_ns: AtomicU64,
    /// 流水线停顿周期数
    pub pipeline_stalls: AtomicU64,
}

impl OptimizedInstructionScheduler {
    /// 创建新的优化指令调度器
    pub fn new(config: OptimizedSchedulerConfig) -> Self {
        Self {
            config,
            dependency_graph: Vec::new(),
            resource_tracker: HashMap::new(),
            current_time: 0,
            stats: OptimizedSchedulingStats::default(),
        }
    }

    /// 构建依赖图
    fn build_dependency_graph(&mut self, block: &CompiledIRBlock) {
        self.dependency_graph.clear();
        
        // 为每个指令创建节点
        for (i, instruction) in block.instructions.iter().enumerate() {
            let node = DependencyNode {
                instruction_index: i,
                instruction: instruction.clone(),
                predecessors: HashSet::new(),
                successors: HashSet::new(),
                ready_time: 0,
                priority: self.calculate_instruction_priority(instruction),
                scheduled: false,
                estimated_time: self.estimate_instruction_time(instruction),
            };
            self.dependency_graph.push(node);
        }

        // 分析指令间的依赖关系
        for (i, node) in self.dependency_graph.iter().enumerate() {
            for (j, other_node) in self.dependency_graph.iter().enumerate() {
                if i != j && self.has_dependency(&node.instruction, &other_node.instruction) {
                    node.predecessors.insert(j);
                    other_node.successors.insert(i);
                }
            }
        }
    }

    /// 计算指令优先级
    fn calculate_instruction_priority(&self, instruction: &CompiledInstruction) -> i32 {
        match &instruction.op {
            IROp::Load { .. } | IROp::LoadImm { .. } => {
                // 加载指令优先级较高（尽早开始内存访问）
                10
            }
            IROp::Store { .. } => {
                // 存储指令优先级中等
                5
            }
            IROp::Add { .. } | IROp::Sub { .. } | IROp::Mul { .. } | IROp::Div { .. } => {
                // 算术指令优先级较高
                8
            }
            IROp::Beq { .. } | IROp::Bne { .. } | IROp::Blt { .. } | IROp::Bge { .. } => {
                // 分支指令优先级较低（延迟决策）
                3
            }
            IROp::Call { .. } => {
                // 调用指令优先级最低
                1
            }
            _ => 0,
        }
    }

    /// 估计指令执行时间
    fn estimate_instruction_time(&self, instruction: &CompiledInstruction) -> u32 {
        match &instruction.op {
            IROp::Load { .. } | IROp::LoadImm { .. } => {
                self.config.latency_model.load_latency as u32
            }
            IROp::Store { .. } => {
                self.config.latency_model.store_latency as u32
            }
            IROp::Add { .. } | IROp::Sub { .. } | IROp::Mul { .. } | IROp::Div { .. } => {
                self.config.latency_model.arithmetic_latency as u32
            }
            IROp::Beq { .. } | IROp::Bne { .. } | IROp::Blt { .. } | IROp::Bge { .. } => {
                self.config.latency_model.branch_latency as u32
            }
            IROp::Call { .. } => {
                self.config.latency_model.call_latency as u32
            }
            _ => 1,
        }
    }

    /// 检查两个指令之间的依赖关系
    fn has_dependency(&self, first: &CompiledInstruction, second: &CompiledInstruction) -> bool {
        // 检查RAW（Read-After-Write）依赖
        if self.has_raw_dependency(first, second) {
            return true;
        }

        // 检查WAR（Write-After-Read）依赖
        if self.has_war_dependency(first, second) {
            return true;
        }

        // 检查WAW（Write-After-Write）依赖
        if self.has_waw_dependency(first, second) {
            return true;
        }

        false
    }

    /// 检查RAW依赖
    fn has_raw_dependency(&self, first: &CompiledInstruction, second: &CompiledInstruction) -> bool {
        let first_writes = self.get_written_registers(first);
        let second_reads = self.get_read_registers(second);
        
        !first_writes.is_disjoint(&second_reads)
    }

    /// 检查WAR依赖
    fn has_war_dependency(&self, first: &CompiledInstruction, second: &CompiledInstruction) -> bool {
        let first_reads = self.get_read_registers(first);
        let second_writes = self.get_written_registers(second);
        
        !first_reads.is_disjoint(&second_writes)
    }

    /// 检查WAW依赖
    fn has_waw_dependency(&self, first: &CompiledInstruction, second: &CompiledInstruction) -> bool {
        let first_writes = self.get_written_registers(first);
        let second_writes = self.get_written_registers(second);
        
        !first_writes.is_disjoint(&second_writes)
    }

    /// 获取指令写入的寄存器
    fn get_written_registers(&self, instruction: &CompiledInstruction) -> HashSet<String> {
        let mut registers = HashSet::new();
        
        match &instruction.op {
            IROp::Add { dst, .. } |
            IROp::Sub { dst, .. } |
            IROp::Mul { dst, .. } |
            IROp::Div { dst, .. } |
            IROp::Load { dst, .. } |
            IROp::LoadImm { dst, .. } |
            IROp::Mov { dst, .. } => {
                registers.insert(format!("r{}", dst));
            }
            _ => {}
        }
        
        registers
    }

    /// 获取指令读取的寄存器
    fn get_read_registers(&self, instruction: &CompiledInstruction) -> HashSet<String> {
        let mut registers = HashSet::new();
        
        match &instruction.op {
            IROp::Add { src1, src2, .. } |
            IROp::Sub { src1, src2, .. } |
            IROp::Mul { src1, src2, .. } |
            IROp::Div { src1, src2, .. } => {
                registers.insert(format!("r{}", src1));
                registers.insert(format!("r{}", src2));
            }
            IROp::Load { .. } => {
                // 加载指令从内存读取，不读取寄存器
            }
            IROp::Store { src, .. } => {
                registers.insert(format!("r{}", src));
            }
            IROp::Mov { src, .. } => {
                registers.insert(format!("r{}", src));
            }
            IROp::Beq { src1, src2, .. } |
            IROp::Bne { src1, src2, .. } |
            IROp::Blt { src1, src2, .. } |
            IROp::Bge { src1, src2, .. } => {
                registers.insert(format!("r{}", src1));
                registers.insert(format!("r{}", src2));
            }
            _ => {}
        }
        
        registers
    }

    /// 列表调度算法
    fn list_scheduling(&mut self) -> Vec<usize> {
        let mut ready_list = VecDeque::new();
        let mut scheduled_order = Vec::new();
        
        // 初始化就绪列表
        for (i, node) in self.dependency_graph.iter().enumerate() {
            if node.predecessors.is_empty() {
                ready_list.push_back(i);
            }
        }

        // 按优先级排序就绪列表
        let mut ready_vec: Vec<_> = ready_list.drain(..).collect();
        ready_vec.sort_by(|&a, &b| {
            self.dependency_graph[b].priority.cmp(&self.dependency_graph[a].priority)
        });
        ready_list.extend(ready_vec);

        while !ready_list.is_empty() {
            // 选择最高优先级的指令
            if let Some(selected) = ready_list.pop_front() {
                let node = &mut self.dependency_graph[selected];
                node.scheduled = true;
                scheduled_order.push(selected);
                
                // 更新后继节点的就绪时间
                for &successor in &node.successors {
                    let successor_node = &mut self.dependency_graph[successor];
                    successor_node.predecessors.remove(&selected);
                    
                    if successor_node.predecessors.is_empty() {
                        successor_node.ready_time = self.current_time + node.estimated_time;
                        ready_list.push_back(successor);
                    }
                }
                
                self.current_time += node.estimated_time;
            }
            
            // 重新排序就绪列表
            let mut ready_vec: Vec<_> = ready_list.drain(..).collect();
            ready_vec.sort_by(|&a, &b| {
                self.dependency_graph[b].priority.cmp(&self.dependency_graph[a].priority)
            });
            ready_list.extend(ready_vec);
        }

        scheduled_order
    }

    /// 关键路径调度算法
    fn critical_path_scheduling(&mut self) -> Vec<usize> {
        let mut scheduled_order = Vec::new();
        let mut current_time = 0u32;
        
        // 计算每个节点的最迟开始时间
        let mut latest_start_times = vec![0u32; self.dependency_graph.len()];
        
        // 从后向前计算最迟开始时间
        let mut sorted_nodes: Vec<usize> = (0..self.dependency_graph.len()).collect();
        sorted_nodes.sort_by(|&a, &b| {
            self.dependency_graph[b].priority.cmp(&self.dependency_graph[a].priority)
        });
        
        for &node_idx in sorted_nodes.iter().rev() {
            let node = &self.dependency_graph[node_idx];
            let mut min_successor_time = u32::MAX;
            
            for &successor in &node.successors {
                min_successor_time = min_successor_time.min(latest_start_times[successor]);
            }
            
            if min_successor_time == u32::MAX {
                latest_start_times[node_idx] = 0;
            } else {
                latest_start_times[node_idx] = min_successor_time - node.estimated_time;
            }
        }
        
        // 按关键路径调度
        let mut remaining_nodes: HashSet<_> = (0..self.dependency_graph.len()).collect();
        
        while !remaining_nodes.is_empty() {
            let mut best_node = None;
            let mut best_priority = i32::MIN;
            
            for &node_idx in &remaining_nodes {
                let node = &self.dependency_graph[node_idx];
                
                // 检查所有前驱是否已调度
                let all_predecessors_scheduled = node.predecessors.iter()
                    .all(|&pred| !remaining_nodes.contains(&pred));
                
                if all_predecessors_scheduled {
                    let slack = latest_start_times[node_idx] as i32 - current_time as i32;
                    let priority = node.priority + slack;
                    
                    if priority > best_priority {
                        best_priority = priority;
                        best_node = Some(node_idx);
                    }
                }
            }
            
            if let Some(selected) = best_node {
                let node = &self.dependency_graph[selected];
                scheduled_order.push(selected);
                remaining_nodes.remove(&selected);
                current_time += node.estimated_time;
            }
        }
        
        scheduled_order
    }

    /// 贪婪调度算法
    fn greedy_scheduling(&mut self) -> Vec<usize> {
        let mut scheduled_order = Vec::new();
        let mut available_resources = self.config.max_parallelism;
        let mut current_time = 0u32;
        
        // 按优先级排序所有节点
        let mut sorted_nodes: Vec<usize> = (0..self.dependency_graph.len()).collect();
        sorted_nodes.sort_by(|&a, &b| {
            self.dependency_graph[b].priority.cmp(&self.dependency_graph[a].priority)
        });
        
        for &node_idx in &sorted_nodes {
            let node = &self.dependency_graph[node_idx];
            
            // 检查资源是否可用
            if self.check_resource_availability(node_idx, available_resources) {
                scheduled_order.push(node_idx);
                self.allocate_resources(node_idx);
                available_resources -= 1;
                
                // 释放已完成的指令资源
                self.release_completed_resources(current_time, &mut available_resources);
            }
        }
        
        scheduled_order
    }

    /// 检查资源可用性
    fn check_resource_availability(&self, node_idx: usize, available_resources: usize) -> bool {
        if available_resources == 0 {
            return false;
        }
        
        // 检查寄存器冲突
        let node = &self.dependency_graph[node_idx];
        let required_registers = self.get_required_registers(&node.instruction);
        
        for &scheduled_idx in &self.dependency_graph.iter()
            .enumerate()
            .filter_map(|(i, n)| if n.scheduled { Some(i) } else { None })
            .collect::<Vec<_>>() {
            let scheduled_node = &self.dependency_graph[scheduled_idx];
            let used_registers = self.get_required_registers(&scheduled_node.instruction);
            
            if !required_registers.is_disjoint(&used_registers) {
                return false;
            }
        }
        
        true
    }

    /// 获取指令所需的寄存器
    fn get_required_registers(&self, instruction: &CompiledInstruction) -> HashSet<String> {
        let mut registers = HashSet::new();
        
        match &instruction.op {
            IROp::Add { dst, src1, src2 } |
            IROp::Sub { dst, src1, src2 } |
            IROp::Mul { dst, src1, src2 } |
            IROp::Div { dst, src1, src2 } => {
                registers.insert(format!("r{}", dst));
                registers.insert(format!("r{}", src1));
                registers.insert(format!("r{}", src2));
            }
            IROp::Load { dst, .. } |
            IROp::LoadImm { dst, .. } => {
                registers.insert(format!("r{}", dst));
            }
            IROp::Store { src, .. } => {
                registers.insert(format!("r{}", src));
            }
            IROp::Mov { dst, src } => {
                registers.insert(format!("r{}", dst));
                registers.insert(format!("r{}", src));
            }
            _ => {}
        }
        
        registers
    }

    /// 分配资源
    fn allocate_resources(&mut self, node_idx: usize) {
        let node = &self.dependency_graph[node_idx];
        let resource_usage = ResourceUsage {
            registers: self.get_required_registers(&node.instruction),
            functional_units: HashSet::new(), // TODO: implement functional unit tracking
            memory_access: self.is_memory_access(&node.instruction),
        };
        
        self.resource_tracker.insert(node_idx, resource_usage);
    }

    /// 释放已完成的指令资源
    fn release_completed_resources(&mut self, current_time: u32, available_resources: &mut usize) {
        for (node_idx, node) in self.dependency_graph.iter().enumerate() {
            if node.scheduled {
                if let Some(resource_usage) = self.resource_tracker.get(&node_idx) {
                    if node.ready_time + node.estimated_time <= current_time {
                        self.resource_tracker.remove(&node_idx);
                        *available_resources += 1;
                    }
                }
            }
        }
    }

    /// 检查是否为内存访问指令
    fn is_memory_access(&self, instruction: &CompiledInstruction) -> bool {
        matches!(&instruction.op, IROp::Load { .. } | IROp::Store { .. })
    }

    /// 应用调度结果到指令块
    fn apply_scheduling(&mut self, block: &mut CompiledIRBlock, scheduled_order: Vec<usize>) {
        let mut new_instructions = Vec::with_capacity(block.instructions.len());
        
        for &instruction_idx in &scheduled_order {
            new_instructions.push(self.dependency_graph[instruction_idx].instruction.clone());
        }
        
        block.instructions = new_instructions;
        
        // 更新统计
        self.stats.total_instructions.fetch_add(block.instructions.len() as u64, Ordering::Relaxed);
        self.stats.reordered_instructions.fetch_add(
            (block.instructions.len() - scheduled_order.len()) as u64, 
            Ordering::Relaxed
        );
    }
}

impl InstructionScheduler for OptimizedInstructionScheduler {
    fn schedule(&mut self, block: &CompiledIRBlock) -> Result<CompiledIRBlock, VmError> {
        let start_time = std::time::Instant::now();
        
        let mut result_block = block.clone();
        
        // 构建依赖图
        self.build_dependency_graph(&result_block);
        
        // 根据策略选择调度算法
        let scheduled_order = match self.config.strategy {
            SchedulingStrategy::ListScheduling => {
                self.list_scheduling()
            }
            SchedulingStrategy::CriticalPathScheduling => {
                self.critical_path_scheduling()
            }
            SchedulingStrategy::GreedyScheduling => {
                self.greedy_scheduling()
            }
            SchedulingStrategy::DynamicScheduling => {
                // 动态调度结合多种策略
                if self.dependency_graph.len() > 100 {
                    self.critical_path_scheduling()
                } else {
                    self.list_scheduling()
                }
            }
        };
        
        // 应用调度结果
        self.apply_scheduling(&mut result_block, scheduled_order);
        
        // 更新统计
        let elapsed = start_time.elapsed().as_nanos() as u64;
        self.stats.avg_scheduling_time_ns.store(elapsed, Ordering::Relaxed);
        
        Ok(result_block)
    }

    fn name(&self) -> &str {
        "OptimizedInstructionScheduler"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn set_option(&mut self, option: &str, value: &str) -> Result<(), VmError> {
        match option {
            "strategy" => {
                self.config.strategy = match value {
                    "list" => SchedulingStrategy::ListScheduling,
                    "critical_path" => SchedulingStrategy::CriticalPathScheduling,
                    "greedy" => SchedulingStrategy::GreedyScheduling,
                    "dynamic" => SchedulingStrategy::DynamicScheduling,
                    _ => return Err(VmError::InvalidArgument(format!("Invalid strategy: {}", value))),
                };
            }
            "max_parallelism" => {
                self.config.max_parallelism = value.parse()
                    .map_err(|_| VmError::InvalidArgument("Invalid max_parallelism".to_string()))?;
            }
            "enable_reordering" => {
                self.config.enable_reordering = value.parse()
                    .map_err(|_| VmError::InvalidArgument("Invalid enable_reordering".to_string()))?;
            }
            "enable_pipeline_optimization" => {
                self.config.enable_pipeline_optimization = value.parse()
                    .map_err(|_| VmError::InvalidArgument("Invalid enable_pipeline_optimization".to_string()))?;
            }
            _ => return Err(VmError::InvalidArgument(format!("Unknown option: {}", option))),
        }
        Ok(())
    }

    fn get_option(&self, option: &str) -> Option<String> {
        match option {
            "strategy" => Some(format!("{:?}", self.config.strategy)),
            "max_parallelism" => Some(self.config.max_parallelism.to_string()),
            "enable_reordering" => Some(self.config.enable_reordering.to_string()),
            "enable_pipeline_optimization" => Some(self.config.enable_pipeline_optimization.to_string()),
            _ => None,
        }
    }

    fn reset(&mut self) {
        self.dependency_graph.clear();
        self.resource_table.clear();
        self.stats.total_instructions.store(0, Ordering::Relaxed);
        self.stats.reordered_instructions.store(0, Ordering::Relaxed);
        self.stats.avg_scheduling_time_ns.store(0, Ordering::Relaxed);
    }

    fn get_stats(&self) -> InstructionSchedulingStats {
        InstructionSchedulingStats {
            total_instructions: self.stats.total_instructions.load(Ordering::Relaxed) as usize,
            scheduled_instructions: 0, // TODO: track scheduled instructions
            reordered_instructions: self.stats.reordered_instructions.load(Ordering::Relaxed) as usize,
            scheduling_time_ns: self.stats.avg_scheduling_time_ns.load(Ordering::Relaxed),
            pipeline_efficiency: 0.0, // TODO: calculate pipeline efficiency
        }
    }
}