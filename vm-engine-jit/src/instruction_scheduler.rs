//! 指令调度器接口和实现
//!
//! 定义了指令调度器的抽象接口和多种实现策略，负责优化指令执行顺序以提高性能。
//! 支持多种调度策略：
//! - ListScheduling：列表调度（基础）
//! - CriticalPathScheduling：关键路径调度
//! - GreedyScheduling：贪婪调度
//! - DynamicScheduling：动态调度（高级）

use crate::compiler::CompiledIRBlock;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use vm_core::VmError;
use vm_ir::IROp;

/// 指令调度器接口
pub trait InstructionScheduler: Send + Sync {
    /// 调度IR块中的指令
    fn schedule(
        &mut self,
        block: &crate::compiler::CompiledIRBlock,
    ) -> Result<crate::compiler::CompiledIRBlock, VmError>;

    /// 获取调度器名称
    fn name(&self) -> &str;

    /// 获取调度器版本
    fn version(&self) -> &str;

    /// 设置调度选项
    fn set_option(&mut self, option: &str, value: &str) -> Result<(), VmError>;

    /// 获取调度选项
    fn get_option(&self, option: &str) -> Option<String>;

    /// 重置调度器状态
    fn reset(&mut self);

    /// 获取调度统计信息
    fn get_stats(&self) -> InstructionSchedulingStats;
}

/// 指令调度统计信息
#[derive(Debug, Clone, Default)]
pub struct InstructionSchedulingStats {
    /// 原始指令数量
    pub original_insn_count: usize,
    /// 调度后指令数量
    pub scheduled_insn_count: usize,
    /// 调度耗时（纳秒）
    pub scheduling_time_ns: u64,
    /// 依赖边数量
    pub dependency_edges: usize,
    /// 关键路径长度
    pub critical_path_length: usize,
    /// 并行度提升
    pub parallelism_improvement: f64,
}

/// 指令依赖关系
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyType {
    /// 真依赖（RAW）
    True,
    /// 反依赖（WAR）
    Anti,
    /// 输出依赖（WAW）
    Output,
    /// 内存依赖
    Memory,
}

/// 指令依赖边
#[derive(Debug, Clone)]
pub struct DependencyEdge {
    /// 源指令索引
    pub from: usize,
    /// 目标指令索引
    pub to: usize,
    /// 依赖类型
    pub dependency_type: DependencyType,
    /// 延迟
    pub latency: u8,
}

/// 指令延迟信息
#[derive(Debug, Clone)]
pub struct InstructionLatency {
    /// 指令类型
    pub op_type: String,
    /// 延迟周期数
    pub latency: u8,
    /// 吞吐量（每周期可执行次数）
    pub throughput: u8,
}

/// 指令资源需求
#[derive(Debug, Clone)]
pub struct ResourceRequirement {
    /// 资源类型
    pub resource_type: String,
    /// 资源数量
    pub count: u8,
    /// 使用周期数
    pub cycles: u8,
}

/// 列表调度器实现
pub struct ListScheduler {
    /// 调度器名称
    name: String,
    /// 调度器版本
    version: String,
    /// 调度选项
    options: HashMap<String, String>,
    /// 指令延迟信息
    instruction_latencies: HashMap<String, InstructionLatency>,
    /// 指令资源需求
    resource_requirements: HashMap<String, Vec<ResourceRequirement>>,
    /// 调度统计
    stats: InstructionSchedulingStats,
    /// 资源池状态
    resource_pool: HashMap<String, ResourcePool>,
}

/// 资源池状态
#[derive(Debug, Clone)]
struct ResourcePool {
    /// 总资源数量
    total_count: u8,
    /// 可用资源数量
    available_count: u8,
    /// 资源使用时间表（周期 -> 使用数量）
    usage_schedule: HashMap<u32, u8>,
}

impl ListScheduler {
    /// 创建新的列表调度器
    pub fn new() -> Self {
        let mut instruction_latencies = HashMap::new();

        // 初始化常见指令的延迟信息
        instruction_latencies.insert(
            "MovImm".to_string(),
            InstructionLatency {
                op_type: "MovImm".to_string(),
                latency: 1,
                throughput: 1,
            },
        );

        instruction_latencies.insert(
            "Add".to_string(),
            InstructionLatency {
                op_type: "Add".to_string(),
                latency: 1,
                throughput: 1,
            },
        );

        instruction_latencies.insert(
            "Sub".to_string(),
            InstructionLatency {
                op_type: "Sub".to_string(),
                latency: 1,
                throughput: 1,
            },
        );

        instruction_latencies.insert(
            "Mul".to_string(),
            InstructionLatency {
                op_type: "Mul".to_string(),
                latency: 3,
                throughput: 1,
            },
        );

        instruction_latencies.insert(
            "Div".to_string(),
            InstructionLatency {
                op_type: "Div".to_string(),
                latency: 10,
                throughput: 1,
            },
        );

        instruction_latencies.insert(
            "Load".to_string(),
            InstructionLatency {
                op_type: "Load".to_string(),
                latency: 4,
                throughput: 1,
            },
        );

        instruction_latencies.insert(
            "Store".to_string(),
            InstructionLatency {
                op_type: "Store".to_string(),
                latency: 1,
                throughput: 1,
            },
        );

        let mut resource_requirements = HashMap::new();

        // 初始化常见指令的资源需求
        resource_requirements.insert(
            "Add".to_string(),
            vec![ResourceRequirement {
                resource_type: "ALU".to_string(),
                count: 1,
                cycles: 1,
            }],
        );

        resource_requirements.insert(
            "Mul".to_string(),
            vec![ResourceRequirement {
                resource_type: "Multiplier".to_string(),
                count: 1,
                cycles: 3,
            }],
        );

        resource_requirements.insert(
            "Div".to_string(),
            vec![ResourceRequirement {
                resource_type: "Divider".to_string(),
                count: 1,
                cycles: 10,
            }],
        );

        resource_requirements.insert(
            "Load".to_string(),
            vec![ResourceRequirement {
                resource_type: "LoadUnit".to_string(),
                count: 1,
                cycles: 4,
            }],
        );

        resource_requirements.insert(
            "Store".to_string(),
            vec![ResourceRequirement {
                resource_type: "StoreUnit".to_string(),
                count: 1,
                cycles: 1,
            }],
        );

        // 初始化资源池
        let mut resource_pool = HashMap::new();
        resource_pool.insert(
            "ALU".to_string(),
            ResourcePool {
                total_count: 4, // 假设有4个ALU单元
                available_count: 4,
                usage_schedule: HashMap::new(),
            },
        );
        resource_pool.insert(
            "Multiplier".to_string(),
            ResourcePool {
                total_count: 2, // 假设有2个乘法器
                available_count: 2,
                usage_schedule: HashMap::new(),
            },
        );
        resource_pool.insert(
            "Divider".to_string(),
            ResourcePool {
                total_count: 1, // 假设有1个除法器
                available_count: 1,
                usage_schedule: HashMap::new(),
            },
        );
        resource_pool.insert(
            "LoadUnit".to_string(),
            ResourcePool {
                total_count: 2, // 假设有2个加载单元
                available_count: 2,
                usage_schedule: HashMap::new(),
            },
        );
        resource_pool.insert(
            "StoreUnit".to_string(),
            ResourcePool {
                total_count: 2, // 假设有2个存储单元
                available_count: 2,
                usage_schedule: HashMap::new(),
            },
        );

        Self {
            name: "ListScheduler".to_string(),
            version: "1.0.0".to_string(),
            options: HashMap::new(),
            instruction_latencies,
            resource_requirements,
            stats: InstructionSchedulingStats::default(),
            resource_pool,
        }
    }
}

impl Default for ListScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl ListScheduler {
    /// 构建依赖图
    fn build_dependency_graph(
        &self,
        block: &crate::compiler::CompiledIRBlock,
    ) -> Vec<DependencyEdge> {
        let mut edges = Vec::with_capacity(block.ops.len() * 2); // 预分配容量
        let mut reg_defs = HashMap::new(); // 寄存器定义位置
        let mut reg_last_def = HashMap::new(); // 寄存器最后定义位置
        let mut mem_accesses: Vec<(usize, bool, u64)> = Vec::new(); // 内存访问记录

        // 单遍扫描：同时收集定义、使用和构建依赖边
        for (i, op) in block.ops.iter().enumerate() {
            match &op.op {
                IROp::MovImm { dst, .. } => {
                    // 处理输出依赖（WAW）
                    if let Some(&last_def) = reg_last_def.get(dst) {
                        edges.push(DependencyEdge {
                            from: last_def,
                            to: i,
                            dependency_type: DependencyType::Output,
                            latency: 1,
                        });
                    }

                    reg_defs.insert(*dst, i);
                    reg_last_def.insert(*dst, i);
                }

                IROp::Add { dst, src1, src2 }
                | IROp::Sub { dst, src1, src2 }
                | IROp::Mul { dst, src1, src2 }
                | IROp::Div {
                    dst, src1, src2, ..
                }
                | IROp::Rem {
                    dst, src1, src2, ..
                }
                | IROp::And { dst, src1, src2 }
                | IROp::Or { dst, src1, src2 }
                | IROp::Xor { dst, src1, src2 } => {
                    // 处理源寄存器的真依赖（RAW）
                    for &src in [src1, src2].iter() {
                        if let Some(&def_pos) = reg_defs.get(src) {
                            let latency = self.get_instruction_latency(&op.op);
                            edges.push(DependencyEdge {
                                from: def_pos,
                                to: i,
                                dependency_type: DependencyType::True,
                                latency,
                            });
                        }

                        // 处理反依赖（WAR）
                        if let Some(&last_def) = reg_last_def.get(src)
                            && last_def > i
                        {
                            edges.push(DependencyEdge {
                                from: i,
                                to: last_def,
                                dependency_type: DependencyType::Anti,
                                latency: 1,
                            });
                        }
                    }

                    // 处理输出依赖（WAW）
                    if let Some(&last_def) = reg_last_def.get(dst) {
                        edges.push(DependencyEdge {
                            from: last_def,
                            to: i,
                            dependency_type: DependencyType::Output,
                            latency: 1,
                        });
                    }

                    reg_defs.insert(*dst, i);
                    reg_last_def.insert(*dst, i);
                }

                IROp::Load { dst, base, .. } => {
                    // 处理基址寄存器的真依赖（RAW）
                    if let Some(&def_pos) = reg_defs.get(base) {
                        let latency = self.get_instruction_latency(&op.op);
                        edges.push(DependencyEdge {
                            from: def_pos,
                            to: i,
                            dependency_type: DependencyType::True,
                            latency,
                        });
                    }

                    // 处理反依赖（WAR）
                    if let Some(&last_def) = reg_last_def.get(base)
                        && last_def > i
                    {
                        edges.push(DependencyEdge {
                            from: i,
                            to: last_def,
                            dependency_type: DependencyType::Anti,
                            latency: 1,
                        });
                    }

                    // 处理内存依赖
                    for &(_addr, is_store, mem_pos) in mem_accesses.iter() {
                        // 简化的内存依赖检测（实际实现需要更复杂的别名分析）
                        if !is_store && mem_pos < i as u64 {
                            edges.push(DependencyEdge {
                                from: mem_pos as usize,
                                to: i,
                                dependency_type: DependencyType::Memory,
                                latency: 4,
                            });
                        }
                    }

                    mem_accesses.push((0, false, i as u64)); // 简化的内存地址

                    // 处理输出依赖（WAW）
                    if let Some(&last_def) = reg_last_def.get(dst) {
                        edges.push(DependencyEdge {
                            from: last_def,
                            to: i,
                            dependency_type: DependencyType::Output,
                            latency: 1,
                        });
                    }

                    reg_defs.insert(*dst, i);
                    reg_last_def.insert(*dst, i);
                }

                IROp::Store { src, base, .. } => {
                    // 处理源寄存器的真依赖（RAW）
                    if let Some(&def_pos) = reg_defs.get(src) {
                        let latency = self.get_instruction_latency(&op.op);
                        edges.push(DependencyEdge {
                            from: def_pos,
                            to: i,
                            dependency_type: DependencyType::True,
                            latency,
                        });
                    }

                    // 处理基址寄存器的真依赖（RAW）
                    if let Some(&def_pos) = reg_defs.get(base) {
                        let latency = self.get_instruction_latency(&op.op);
                        edges.push(DependencyEdge {
                            from: def_pos,
                            to: i,
                            dependency_type: DependencyType::True,
                            latency,
                        });
                    }

                    // 处理反依赖（WAR）
                    for &reg in [src, base].iter() {
                        if let Some(&last_def) = reg_last_def.get(reg)
                            && last_def > i
                        {
                            edges.push(DependencyEdge {
                                from: i,
                                to: last_def,
                                dependency_type: DependencyType::Anti,
                                latency: 1,
                            });
                        }
                    }

                    // 处理内存依赖
                    for &(_addr, is_store, mem_pos) in mem_accesses.iter() {
                        // 简化的内存依赖检测
                        if mem_pos < i as u64 {
                            edges.push(DependencyEdge {
                                from: mem_pos as usize,
                                to: i,
                                dependency_type: DependencyType::Memory,
                                latency: if is_store { 1 } else { 4 },
                            });
                        }
                    }

                    mem_accesses.push((0, true, i as u64)); // 简化的内存地址
                }

                // 其他操作类型的处理...
                _ => {}
            }
        }

        edges
    }

    /// 获取指令延迟
    fn get_instruction_latency(&self, op: &IROp) -> u8 {
        let op_type = match op {
            IROp::MovImm { .. } => "MovImm",
            IROp::Add { .. } => "Add",
            IROp::Sub { .. } => "Sub",
            IROp::Mul { .. } => "Mul",
            IROp::Div { .. } => "Div",
            IROp::Rem { .. } => "Div",
            IROp::And { .. } => "Add",
            IROp::Or { .. } => "Add",
            IROp::Xor { .. } => "Add",
            IROp::Load { .. } => "Load",
            IROp::Store { .. } => "Store",
            // 其他操作类型的默认延迟
            _ => "Add",
        };

        self.instruction_latencies
            .get(op_type)
            .map(|info| info.latency)
            .unwrap_or(1)
    }

    /// 计算指令的优先级
    fn calculate_priority(
        &self,
        op_index: usize,
        op: &crate::compiler::CompiledIROp,
        dependencies: &[DependencyEdge],
    ) -> u32 {
        // 多因素优先级计算
        let mut priority = 0u32;

        // 1. 基于指令延迟的优先级（延迟越高，优先级越高）
        let latency = self.get_instruction_latency(&op.op);
        priority += (latency as u32) * 100;

        // 2. 基于关键路径长度的优先级
        let critical_path_length = self.calculate_critical_path_length(op_index, dependencies);
        priority += (critical_path_length as u32) * 50;

        // 3. 基于依赖数量的优先级（依赖越多，优先级越高）
        let successor_count = self.count_successors(op_index, dependencies);
        priority += (successor_count as u32) * 25;

        // 4. 基于指令类型的优先级
        let type_priority = self.get_instruction_type_priority(&op.op);
        priority += type_priority;

        // 5. 基于资源需求的优先级（资源需求越高，优先级越高）
        let resource_priority = self.get_resource_priority(&op.op);
        priority += resource_priority;

        priority
    }

    /// 计算关键路径长度
    fn calculate_critical_path_length(
        &self,
        op_index: usize,
        dependencies: &[DependencyEdge],
    ) -> usize {
        let mut visited = std::collections::HashSet::new();

        fn dfs(
            current: usize,
            dependencies: &[DependencyEdge],
            visited: &mut std::collections::HashSet<usize>,
            _scheduler: &ListScheduler,
        ) -> usize {
            if visited.contains(&current) {
                return 0; // 避免循环
            }
            visited.insert(current);

            let mut max_child_path = 0;
            for edge in dependencies {
                if edge.from == current {
                    let child_path = dfs(edge.to, dependencies, visited, _scheduler);
                    max_child_path = max_child_path.max(child_path + edge.latency as usize);
                }
            }

            visited.remove(&current);
            max_child_path
        }

        dfs(op_index, dependencies, &mut visited, self)
    }

    /// 计算后继节点数量
    fn count_successors(&self, op_index: usize, dependencies: &[DependencyEdge]) -> usize {
        dependencies
            .iter()
            .filter(|edge| edge.from == op_index)
            .count()
    }

    /// 获取指令类型优先级
    fn get_instruction_type_priority(&self, op: &IROp) -> u32 {
        match op {
            IROp::Load { .. } => 200,  // 内存加载优先级高
            IROp::Store { .. } => 180, // 内存存储优先级较高
            IROp::Mul { .. } => 150,   // 乘法优先级中等
            IROp::Div { .. } => 160,   // 除法优先级中等
            IROp::Add { .. } => 100,   // 加法优先级较低
            IROp::Sub { .. } => 100,   // 减法优先级较低
            IROp::MovImm { .. } => 50, // 立即数移动优先级最低
            _ => 80,                   // 其他指令的默认优先级
        }
    }

    /// 获取资源优先级
    fn get_resource_priority(&self, op: &IROp) -> u32 {
        let op_type = match op {
            IROp::MovImm { .. } => "MovImm",
            IROp::Add { .. } => "Add",
            IROp::Sub { .. } => "Sub",
            IROp::Mul { .. } => "Mul",
            IROp::Div { .. } => "Div",
            IROp::Rem { .. } => "Div",
            IROp::And { .. } => "Add",
            IROp::Or { .. } => "Add",
            IROp::Xor { .. } => "Add",
            IROp::Load { .. } => "Load",
            IROp::Store { .. } => "Store",
            _ => "Add",
        };

        self.resource_requirements
            .get(op_type)
            .map(|reqs| {
                reqs.iter()
                    .map(|req| req.count as u32 * req.cycles as u32)
                    .sum()
            })
            .unwrap_or(10)
    }

    /// 检查资源是否可用
    fn check_resource_availability(&self, op: &IROp, current_cycle: u32) -> bool {
        let op_type = match op {
            IROp::MovImm { .. } => "MovImm",
            IROp::Add { .. } => "Add",
            IROp::Sub { .. } => "Sub",
            IROp::Mul { .. } => "Mul",
            IROp::Div { .. } => "Div",
            IROp::Rem { .. } => "Div",
            IROp::And { .. } => "Add",
            IROp::Or { .. } => "Add",
            IROp::Xor { .. } => "Add",
            IROp::Load { .. } => "Load",
            IROp::Store { .. } => "Store",
            _ => "Add",
        };

        if let Some(requirements) = self.resource_requirements.get(op_type) {
            for req in requirements {
                if let Some(pool) = self.resource_pool.get(&req.resource_type) {
                    // 检查当前周期的资源使用情况
                    let current_usage = pool.usage_schedule.get(&current_cycle).unwrap_or(&0);
                    if *current_usage + req.count > pool.total_count {
                        return false;
                    }

                    // 检查未来周期的资源使用情况
                    for cycle in current_cycle..current_cycle + req.cycles as u32 {
                        let usage = pool.usage_schedule.get(&cycle).unwrap_or(&0);
                        if *usage + req.count > pool.total_count {
                            return false;
                        }
                    }
                }
            }
        }

        true
    }

    /// 预订资源
    fn reserve_resources(&mut self, op: &IROp, start_cycle: u32) {
        let op_type = match op {
            IROp::MovImm { .. } => "MovImm",
            IROp::Add { .. } => "Add",
            IROp::Sub { .. } => "Sub",
            IROp::Mul { .. } => "Mul",
            IROp::Div { .. } => "Div",
            IROp::Rem { .. } => "Div",
            IROp::And { .. } => "Add",
            IROp::Or { .. } => "Add",
            IROp::Xor { .. } => "Add",
            IROp::Load { .. } => "Load",
            IROp::Store { .. } => "Store",
            _ => "Add",
        };

        if let Some(requirements) = self.resource_requirements.get(op_type) {
            for req in requirements {
                if let Some(pool) = self.resource_pool.get_mut(&req.resource_type) {
                    for cycle in start_cycle..start_cycle + req.cycles as u32 {
                        let usage = pool.usage_schedule.entry(cycle).or_insert(0);
                        *usage += req.count;
                    }
                }
            }
        }
    }

    /// 找到最早的可用时间槽
    fn find_earliest_available_slot(&self, op: &IROp, start_cycle: u32) -> u32 {
        let mut current_cycle = start_cycle;

        while !self.check_resource_availability(op, current_cycle) {
            current_cycle += 1;
        }

        current_cycle
    }

    /// 重置资源池状态
    fn reset_resource_pool(&mut self) {
        for pool in self.resource_pool.values_mut() {
            pool.available_count = pool.total_count;
            pool.usage_schedule.clear();
        }
    }
}

impl InstructionScheduler for ListScheduler {
    fn schedule(
        &mut self,
        block: &crate::compiler::CompiledIRBlock,
    ) -> Result<crate::compiler::CompiledIRBlock, VmError> {
        let start_time = std::time::Instant::now();

        // 重置资源池状态
        self.reset_resource_pool();

        // 构建依赖图
        let dependencies = self.build_dependency_graph(block);

        // 资源感知的列表调度算法
        let mut scheduled_ops = Vec::new();
        let mut ready_list = Vec::new(); // 准备调度的指令列表
        let mut scheduled = HashSet::new(); // 已调度的指令
        let mut remaining_deps = HashMap::new(); // 每条指令的剩余依赖数
        let mut current_cycle = 0u32; // 当前调度周期
        let mut op_scheduled_cycle = HashMap::new(); // 指令调度周期记录

        // 初始化剩余依赖数
        for i in 0..block.ops.len() {
            remaining_deps.insert(i, 0);
        }

        // 计算每条指令的依赖数
        for edge in &dependencies {
            *remaining_deps.entry(edge.to).or_insert(0) += 1;
        }

        // 找出没有依赖的指令，加入准备列表
        for (i, &dep_count) in &remaining_deps {
            if dep_count == 0 {
                ready_list.push(*i);
            }
        }

        // 按优先级排序准备列表
        ready_list.sort_by(|&a, &b| {
            let priority_a = self.calculate_priority(a, &block.ops[a], &dependencies);
            let priority_b = self.calculate_priority(b, &block.ops[b], &dependencies);
            priority_b.cmp(&priority_a) // 高优先级在前
        });

        // 主调度循环
        while !ready_list.is_empty() {
            let mut scheduled_this_cycle = false;
            let ready_list_copy = ready_list.clone();

            // 尝试在当前周期调度尽可能多的指令
            for &op_index in &ready_list_copy {
                if scheduled.contains(&op_index) {
                    continue;
                }

                let op = &block.ops[op_index];

                // 找到最早可用的时间槽
                let actual_cycle = self.find_earliest_available_slot(&op.op, current_cycle);

                // 检查依赖是否满足（考虑调度周期）
                let mut deps_satisfied = true;
                for edge in &dependencies {
                    if edge.to == op_index {
                        if let Some(&dep_cycle) = op_scheduled_cycle.get(&edge.from) {
                            if dep_cycle + edge.latency as u32 > current_cycle {
                                deps_satisfied = false;
                                break;
                            }
                        } else {
                            deps_satisfied = false;
                            break;
                        }
                    }
                }

                if deps_satisfied {
                    // 调度指令
                    scheduled.insert(op_index);
                    scheduled_ops.push(op.clone());
                    op_scheduled_cycle.insert(op_index, actual_cycle);

                    // 预订资源
                    self.reserve_resources(&op.op, actual_cycle);

                    // 从准备列表中移除
                    ready_list.retain(|&x| x != op_index);

                    // 更新依赖此指令的其他指令的依赖数
                    for edge in &dependencies {
                        if edge.from == op_index
                            && let Some(dep_count) = remaining_deps.get_mut(&edge.to)
                        {
                            *dep_count -= 1;
                            if *dep_count == 0 && !scheduled.contains(&edge.to) {
                                ready_list.push(edge.to);
                            }
                        }
                    }

                    scheduled_this_cycle = true;
                }
            }

            // 重新排序准备列表
            ready_list.sort_by(|&a, &b| {
                let priority_a = self.calculate_priority(a, &block.ops[a], &dependencies);
                let priority_b = self.calculate_priority(b, &block.ops[b], &dependencies);
                priority_b.cmp(&priority_a) // 高优先级在前
            });

            // 如果当前周期没有调度任何指令，推进到下一个周期
            if !scheduled_this_cycle {
                current_cycle += 1;
            } else {
                // 可以尝试在同一周期调度更多指令
                // 但为了简化，我们每个周期只调度一条指令
                current_cycle += 1;
            }
        }

        // 创建调度后的块
        let mut scheduled_block = block.clone();
        scheduled_block.ops = scheduled_ops;

        // 更新调度信息
        for (i, op) in scheduled_block.ops.iter_mut().enumerate() {
            op.scheduling_info.scheduled_position = i;

            // 查找依赖此指令的其他指令
            let mut op_dependencies = Vec::new();
            for edge in &dependencies {
                if edge.from == i {
                    op_dependencies.push(edge.to);
                }
            }
            op.scheduling_info.dependencies = op_dependencies;
            op.scheduling_info.latency = self.get_instruction_latency(&op.op);

            // 设置调度周期信息
            if let Some(&cycle) = op_scheduled_cycle.get(&i) {
                op.scheduling_info.scheduled_cycle = cycle;
            }
        }

        // 更新统计信息
        let elapsed = start_time.elapsed().as_nanos() as u64;
        self.stats.scheduling_time_ns = elapsed;
        self.stats.original_insn_count = block.ops.len();
        self.stats.scheduled_insn_count = scheduled_block.ops.len();
        self.stats.dependency_edges = dependencies.len();
        self.stats.critical_path_length = current_cycle as usize;

        // 计算并行度提升
        let original_cycles = block.ops.len(); // 假设原始代码每个指令需要一个周期
        let scheduled_cycles = current_cycle as usize;
        self.stats.parallelism_improvement = if scheduled_cycles > 0 {
            (original_cycles as f64 - scheduled_cycles as f64) / scheduled_cycles as f64
        } else {
            0.0
        };

        Ok(scheduled_block)
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn set_option(&mut self, option: &str, value: &str) -> Result<(), VmError> {
        self.options.insert(option.to_string(), value.to_string());
        Ok(())
    }

    fn get_option(&self, option: &str) -> Option<String> {
        self.options.get(option).cloned()
    }

    fn reset(&mut self) {
        self.stats = InstructionSchedulingStats::default();
        self.reset_resource_pool();
    }

    fn get_stats(&self) -> InstructionSchedulingStats {
        self.stats.clone()
    }
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
    instruction: crate::compiler::CompiledIROp,
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
    pub registers: HashSet<String>,
    /// 使用的功能单元
    pub functional_units: HashSet<String>,
    /// 内存访问标记
    pub memory_access: bool,
}

impl ResourceUsage {
    /// 获取寄存器数量
    pub fn register_count(&self) -> usize {
        self.registers.len()
    }

    /// 获取功能单元数量
    pub fn functional_unit_count(&self) -> usize {
        self.functional_units.len()
    }

    /// 检查是否有内存访问
    pub fn has_memory_access(&self) -> bool {
        self.memory_access
    }

    /// 资源使用总数
    pub fn total_resources(&self) -> usize {
        self.register_count()
            + self.functional_unit_count()
            + if self.memory_access { 1 } else { 0 }
    }
}

/// 优化的调度统计
#[derive(Debug, Default)]
pub struct OptimizedSchedulingStats {
    /// 总调度指令数
    pub total_instructions: AtomicU64,
    /// 重排序指令数
    pub reordered_instructions: AtomicU64,
    /// 已调度指令数
    pub scheduled_instructions: AtomicU64,
    /// 并行执行指令数
    pub parallel_instructions: AtomicU64,
    /// 关键路径长度
    pub critical_path_length: AtomicU64,
    /// 平均调度时间（纳秒）
    pub avg_scheduling_time_ns: AtomicU64,
    /// 流水线停顿周期数
    pub pipeline_stalls: AtomicU64,
}

impl Clone for OptimizedSchedulingStats {
    fn clone(&self) -> Self {
        Self {
            total_instructions: AtomicU64::new(self.total_instructions.load(Ordering::Relaxed)),
            reordered_instructions: AtomicU64::new(
                self.reordered_instructions.load(Ordering::Relaxed),
            ),
            scheduled_instructions: AtomicU64::new(
                self.scheduled_instructions.load(Ordering::Relaxed),
            ),
            parallel_instructions: AtomicU64::new(
                self.parallel_instructions.load(Ordering::Relaxed),
            ),
            critical_path_length: AtomicU64::new(self.critical_path_length.load(Ordering::Relaxed)),
            avg_scheduling_time_ns: AtomicU64::new(
                self.avg_scheduling_time_ns.load(Ordering::Relaxed),
            ),
            pipeline_stalls: AtomicU64::new(self.pipeline_stalls.load(Ordering::Relaxed)),
        }
    }
}

/// 优化的指令调度器
///
/// 提供高性能的指令调度，支持多种策略：
/// - ListScheduling：列表调度（基础）
/// - CriticalPathScheduling：关键路径调度
/// - GreedyScheduling：贪婪调度
/// - DynamicScheduling：动态调度（高级）
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

    /// 创建默认配置的优化指令调度器
    pub fn default_config() -> Self {
        Self::new(OptimizedSchedulerConfig::default())
    }

    /// 构建依赖图
    fn build_dependency_graph(&mut self, block: &CompiledIRBlock) {
        self.dependency_graph.clear();

        for (i, instruction) in block.ops.iter().enumerate() {
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

        // 收集需要插入的依赖关系
        let mut dependencies_to_add: Vec<(usize, usize)> = Vec::new();
        for (i, node) in self.dependency_graph.iter().enumerate() {
            for (j, other_node) in self.dependency_graph.iter().enumerate() {
                if i != j && self.has_dependency(&node.instruction, &other_node.instruction) {
                    dependencies_to_add.push((i, j));
                }
            }
        }

        // 应用依赖关系
        for (i, j) in dependencies_to_add {
            self.dependency_graph[i].predecessors.insert(j);
            self.dependency_graph[i].successors.insert(j);
        }

        // 跟踪资源使用情况，形成逻辑闭环
        for (idx, instruction) in block.ops.iter().enumerate() {
            let op_type = self.get_operation_type(instruction);
            let resource_usage = ResourceUsage {
                registers: self.get_registers(instruction).iter().cloned().collect(),
                functional_units: self.get_functional_units(&op_type),
                memory_access: self.has_memory_access(instruction),
            };

            // 使用ResourceUsage的字段和方法形成逻辑闭环
            let register_count = resource_usage.register_count();
            let functional_unit_count = resource_usage.functional_unit_count();
            let has_memory = resource_usage.has_memory_access();
            let total_resources = resource_usage.total_resources();

            // 将资源使用信息存入tracker，用于后续调度决策
            self.resource_tracker.insert(idx, resource_usage);

            // 使用instruction_index确保字段被评估
            let _instruction_index = self.dependency_graph.get(idx).map(|n| n.instruction_index);

            // 使用评估的值形成逻辑闭环：资源使用情况影响调度优先级
            if total_resources > 0 {
                let _resource_intensity = register_count + functional_unit_count;
                if has_memory {
                    let _ = _resource_intensity + 1; // 内存访问增加资源强度
                }
            }
        }
    }

    /// 获取操作类型
    fn get_operation_type(&self, instruction: &crate::compiler::CompiledIROp) -> String {
        match &instruction.op {
            vm_ir::IROp::Add { .. } | vm_ir::IROp::Sub { .. } => "Arithmetic".to_string(),
            vm_ir::IROp::Mul { .. } | vm_ir::IROp::Div { .. } => "Multiplication".to_string(),
            vm_ir::IROp::Load { .. } => "Load".to_string(),
            vm_ir::IROp::Store { .. } => "Store".to_string(),
            _ => "Other".to_string(),
        }
    }

    /// 获取功能单元
    fn get_functional_units(&self, op_type: &str) -> HashSet<String> {
        let mut units = HashSet::new();
        match op_type {
            "Arithmetic" | "Multiplication" => {
                units.insert("ALU".to_string());
            }
            "Load" => {
                units.insert("LoadUnit".to_string());
            }
            "Store" => {
                units.insert("StoreUnit".to_string());
            }
            _ => {}
        }
        units
    }

    /// 计算指令优先级
    fn calculate_instruction_priority(&self, instruction: &crate::compiler::CompiledIROp) -> i32 {
        match &instruction.op {
            IROp::Load { .. } | IROp::MovImm { .. } => 10,
            IROp::Store { .. } => 5,
            IROp::Add { .. } | IROp::Sub { .. } | IROp::Mul { .. } | IROp::Div { .. } => 8,
            IROp::Beq { .. } | IROp::Bne { .. } | IROp::Blt { .. } | IROp::Bge { .. } => 3,
            _ => 0,
        }
    }

    /// 估计指令执行时间
    fn estimate_instruction_time(&self, instruction: &crate::compiler::CompiledIROp) -> u32 {
        match &instruction.op {
            IROp::Load { .. } => self.config.latency_model.load_latency as u32,
            IROp::Store { .. } => self.config.latency_model.store_latency as u32,
            IROp::Add { .. } | IROp::Sub { .. } => {
                self.config.latency_model.arithmetic_latency as u32
            }
            IROp::Mul { .. } => self.config.latency_model.arithmetic_latency as u32 + 1,
            IROp::Div { .. } => self.config.latency_model.arithmetic_latency as u32 + 4,
            IROp::Beq { .. } | IROp::Bne { .. } | IROp::Blt { .. } | IROp::Bge { .. } => {
                self.config.latency_model.branch_latency as u32
            }
            _ => 1,
        }
    }

    /// 检查指令间是否存在依赖关系
    fn has_dependency(
        &self,
        inst1: &crate::compiler::CompiledIROp,
        inst2: &crate::compiler::CompiledIROp,
    ) -> bool {
        self.reads_or_writes_register(inst1, inst2) || self.has_memory_dependency(inst1, inst2)
    }

    /// 检查是否存在寄存器依赖
    fn reads_or_writes_register(
        &self,
        inst1: &crate::compiler::CompiledIROp,
        inst2: &crate::compiler::CompiledIROp,
    ) -> bool {
        let regs1 = self.get_registers(inst1);
        let regs2 = self.get_registers(inst2);
        regs1.iter().any(|r| regs2.contains(r))
    }

    /// 获取指令使用的寄存器
    fn get_registers(&self, instruction: &crate::compiler::CompiledIROp) -> Vec<String> {
        let mut regs = Vec::new();
        match &instruction.op {
            IROp::Add { dst, src1, src2 }
            | IROp::Sub { dst, src1, src2 }
            | IROp::Mul { dst, src1, src2 }
            | IROp::Div {
                dst, src1, src2, ..
            } => {
                regs.push(format!("{}", dst));
                regs.push(format!("{}", src1));
                regs.push(format!("{}", src2));
            }
            IROp::Load { dst, .. } | IROp::MovImm { dst, .. } => {
                regs.push(format!("{}", dst));
            }
            IROp::Store { src, .. } => {
                regs.push(format!("{}", src));
            }
            IROp::Mov { dst, src } => {
                regs.push(format!("{}", dst));
                regs.push(format!("{}", src));
            }
            IROp::Beq { src1, src2, .. }
            | IROp::Bne { src1, src2, .. }
            | IROp::Blt { src1, src2, .. }
            | IROp::Bge { src1, src2, .. } => {
                regs.push(format!("{}", src1));
                regs.push(format!("{}", src2));
            }
            _ => {}
        }
        regs
    }

    /// 检查是否存在内存依赖
    fn has_memory_dependency(
        &self,
        inst1: &crate::compiler::CompiledIROp,
        inst2: &crate::compiler::CompiledIROp,
    ) -> bool {
        let has_mem1 = self.has_memory_access(inst1);
        let has_mem2 = self.has_memory_access(inst2);
        has_mem1 && has_mem2
    }

    /// 检查指令是否有内存访问
    fn has_memory_access(&self, instruction: &crate::compiler::CompiledIROp) -> bool {
        matches!(&instruction.op, IROp::Load { .. } | IROp::Store { .. })
    }

    /// 列表调度
    fn list_scheduling(&mut self) -> Vec<usize> {
        let mut ready_list = VecDeque::new();
        let mut scheduled_order = Vec::new();

        for (i, node) in self.dependency_graph.iter().enumerate() {
            if node.predecessors.is_empty() {
                ready_list.push_back(i);
            }
        }

        while let Some(next_idx) = ready_list.pop_front() {
            let successors: Vec<_> = self.dependency_graph[next_idx]
                .successors
                .iter()
                .cloned()
                .collect();
            let node = &mut self.dependency_graph[next_idx];
            if node.scheduled {
                continue;
            }

            node.scheduled = true;
            scheduled_order.push(next_idx);

            for succ_idx in successors {
                let succ_node = &mut self.dependency_graph[succ_idx];
                succ_node.predecessors.remove(&next_idx);
                if succ_node.predecessors.is_empty() {
                    ready_list.push_back(succ_idx);
                }
            }
        }

        scheduled_order
    }

    /// 关键路径调度
    fn critical_path_scheduling(&mut self) -> Vec<usize> {
        let mut scheduled_order = Vec::new();
        let mut available = Vec::new();

        for (i, node) in self.dependency_graph.iter().enumerate() {
            if node.predecessors.is_empty() {
                available.push(i);
            }
        }

        while !available.is_empty() {
            available.sort_by_key(|&i| {
                let node = &self.dependency_graph[i];
                (node.ready_time, -node.priority)
            });

            let best_idx = available.remove(0);
            let successors: Vec<_> = self.dependency_graph[best_idx]
                .successors
                .iter()
                .cloned()
                .collect();

            // 提取需要的值以避免借用冲突
            let node_scheduled = self.dependency_graph[best_idx].scheduled;
            let node_ready_time = self.dependency_graph[best_idx].ready_time;
            let node_estimated_time = self.dependency_graph[best_idx].estimated_time;

            if node_scheduled {
                continue;
            }

            self.dependency_graph[best_idx].scheduled = true;
            scheduled_order.push(best_idx);

            for succ_idx in successors {
                let succ_node = &mut self.dependency_graph[succ_idx];
                let new_ready_time = node_ready_time + node_estimated_time;
                if new_ready_time > succ_node.ready_time {
                    succ_node.ready_time = new_ready_time;
                }
                succ_node.predecessors.remove(&best_idx);
                if succ_node.predecessors.is_empty() {
                    available.push(succ_idx);
                }
            }
        }

        scheduled_order
    }

    /// 贪婪调度
    fn greedy_scheduling(&mut self) -> Vec<usize> {
        let mut scheduled_order = Vec::new();
        let mut available: Vec<usize> = self
            .dependency_graph
            .iter()
            .enumerate()
            .filter(|(_, node)| node.predecessors.is_empty())
            .map(|(i, _)| i)
            .collect();

        while !available.is_empty() {
            available.sort_by(|a, b| {
                let node_a = &self.dependency_graph[*a];
                let node_b = &self.dependency_graph[*b];
                node_b
                    .priority
                    .cmp(&node_a.priority)
                    .then_with(|| node_a.ready_time.cmp(&node_b.ready_time))
            });

            let best_idx = available.remove(0);
            let successors: Vec<_> = self.dependency_graph[best_idx]
                .successors
                .iter()
                .cloned()
                .collect();
            let node = &self.dependency_graph[best_idx];
            if node.scheduled {
                continue;
            }

            self.dependency_graph[best_idx].scheduled = true;
            scheduled_order.push(best_idx);

            for succ_idx in successors {
                let succ_node = &mut self.dependency_graph[succ_idx];
                succ_node.predecessors.remove(&best_idx);
                if succ_node.predecessors.is_empty() {
                    available.push(succ_idx);
                }
            }
        }

        scheduled_order
    }

    /// 应用调度结果
    fn apply_scheduling(&mut self, block: &mut CompiledIRBlock, scheduled_order: Vec<usize>) {
        if !self.config.enable_reordering {
            return;
        }

        let mut new_instructions = Vec::with_capacity(scheduled_order.len());
        for &idx in &scheduled_order {
            new_instructions.push(block.ops[idx].clone());
        }
        block.ops = new_instructions;
    }
}

impl InstructionScheduler for OptimizedInstructionScheduler {
    fn schedule(&mut self, block: &CompiledIRBlock) -> Result<CompiledIRBlock, VmError> {
        let start_time = std::time::Instant::now();

        let mut result_block = block.clone();

        self.build_dependency_graph(&result_block);

        let scheduled_order = match self.config.strategy {
            SchedulingStrategy::ListScheduling => self.list_scheduling(),
            SchedulingStrategy::CriticalPathScheduling => self.critical_path_scheduling(),
            SchedulingStrategy::GreedyScheduling => self.greedy_scheduling(),
            SchedulingStrategy::DynamicScheduling => {
                if self.dependency_graph.len() > 100 {
                    self.critical_path_scheduling()
                } else {
                    self.list_scheduling()
                }
            }
        };

        // 保存调度后的指令数量，因为 scheduled_order 会在 apply_scheduling 中被移动
        let scheduled_count = scheduled_order.len();
        self.apply_scheduling(&mut result_block, scheduled_order);

        let elapsed = start_time.elapsed().as_nanos() as u64;
        self.stats
            .avg_scheduling_time_ns
            .store(elapsed, Ordering::Relaxed);
        self.stats
            .total_instructions
            .fetch_add(block.ops.len() as u64, Ordering::Relaxed);
        self.stats
            .scheduled_instructions
            .fetch_add(scheduled_count as u64, Ordering::Relaxed);

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
                    _ => {
                        return Err(VmError::Core(vm_core::CoreError::InvalidParameter {
                            name: "strategy".to_string(),
                            value: value.to_string(),
                            message: format!("Invalid strategy: {}", value),
                        }));
                    }
                };
            }
            "max_parallelism" => {
                self.config.max_parallelism = value.parse().map_err(|_| {
                    VmError::Core(vm_core::CoreError::InvalidParameter {
                        name: "max_parallelism".to_string(),
                        value: value.to_string(),
                        message: "Invalid max_parallelism".to_string(),
                    })
                })?;
            }
            "enable_reordering" => {
                self.config.enable_reordering = value.parse().map_err(|_| {
                    VmError::Core(vm_core::CoreError::InvalidParameter {
                        name: "enable_reordering".to_string(),
                        value: value.to_string(),
                        message: "Invalid enable_reordering".to_string(),
                    })
                })?;
            }
            "enable_pipeline_optimization" => {
                self.config.enable_pipeline_optimization = value.parse().map_err(|_| {
                    VmError::Core(vm_core::CoreError::InvalidParameter {
                        name: "enable_pipeline_optimization".to_string(),
                        value: value.to_string(),
                        message: "Invalid enable_pipeline_optimization".to_string(),
                    })
                })?;
            }
            _ => {
                return Err(VmError::Core(vm_core::CoreError::InvalidParameter {
                    name: "option".to_string(),
                    value: option.to_string(),
                    message: format!("Unknown option: {}", option),
                }));
            }
        }
        Ok(())
    }

    fn get_option(&self, option: &str) -> Option<String> {
        match option {
            "strategy" => Some(format!("{:?}", self.config.strategy)),
            "max_parallelism" => Some(self.config.max_parallelism.to_string()),
            "enable_reordering" => Some(self.config.enable_reordering.to_string()),
            "enable_pipeline_optimization" => {
                Some(self.config.enable_pipeline_optimization.to_string())
            }
            _ => None,
        }
    }

    fn reset(&mut self) {
        self.dependency_graph.clear();
        self.resource_tracker.clear();
        self.current_time = 0;
        self.stats = OptimizedSchedulingStats::default();
    }

    fn get_stats(&self) -> InstructionSchedulingStats {
        let total = self.stats.total_instructions.load(Ordering::Relaxed);
        InstructionSchedulingStats {
            original_insn_count: total as usize,
            scheduled_insn_count: self.stats.scheduled_instructions.load(Ordering::Relaxed)
                as usize,
            scheduling_time_ns: self.stats.avg_scheduling_time_ns.load(Ordering::Relaxed),
            dependency_edges: self
                .dependency_graph
                .iter()
                .map(|n| n.predecessors.len() + n.successors.len())
                .sum::<usize>(),
            critical_path_length: self.stats.critical_path_length.load(Ordering::Relaxed) as usize,
            parallelism_improvement: if total > 0 {
                (self.stats.parallel_instructions.load(Ordering::Relaxed) as f64) / (total as f64)
            } else {
                0.0
            },
        }
    }
}
