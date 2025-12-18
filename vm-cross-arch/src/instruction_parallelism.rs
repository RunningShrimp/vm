//! 指令级并行优化
//!
//! 识别可并行执行的指令，提高跨架构转换的效率

use std::collections::HashSet;
use vm_ir::{IROp, RegId};
use super::Architecture;

/// 指令依赖关系
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyType {
    /// 真依赖（数据依赖）
    True,
    /// 反依赖（输出依赖）
    Anti,
    /// 输出依赖（写后写）
    Output,
}

/// 指令依赖边
#[derive(Debug, Clone)]
pub struct DependencyEdge {
    /// 源指令索引
    pub from: usize,
    /// 目标指令索引
    pub to: usize,
    /// 依赖类型
    pub dep_type: DependencyType,
    /// 相关的寄存器
    pub register: RegId,
}

/// 指令节点
#[derive(Debug, Clone)]
pub struct InstructionNode {
    /// 指令索引
    pub index: usize,
    /// 指令操作
    pub op: IROp,
    /// 读取的寄存器
    pub reads: Vec<RegId>,
    /// 写入的寄存器
    pub writes: Vec<RegId>,
    /// 前驱节点（依赖的指令）
    pub predecessors: Vec<usize>,
    /// 后继节点（依赖此指令的指令）
    pub successors: Vec<usize>,
    /// 是否可以并行执行
    pub can_parallel: bool,
    /// 并行组ID
    pub parallel_group: Option<usize>,
    /// 延迟（周期数）
    pub latency: u32,
}

/// 并行组
#[derive(Debug, Clone)]
pub struct ParallelGroup {
    /// 组ID
    pub id: usize,
    /// 组中的指令索引
    pub instructions: Vec<usize>,
    /// 组的延迟（最大延迟）
    pub latency: u32,
    /// 组的资源需求
    pub resource_requirements: ResourceRequirements,
}

/// 资源需求
#[derive(Debug, Clone, Default)]
pub struct ResourceRequirements {
    /// 需要的ALU单元数
    pub alu_units: u32,
    /// 需要的内存端口数
    pub memory_ports: u32,
    /// 需要的向量单元数
    pub vector_units: u32,
    /// 需要的分支单元数
    pub branch_units: u32,
}

impl ResourceRequirements {
    /// 根据架构获取资源需求
    pub fn for_architecture(arch: Architecture) -> Self {
        match arch {
            Architecture::X86_64 => ResourceRequirements {
                alu_units: 8,
                memory_ports: 4,
                vector_units: 2,
                branch_units: 2,
            },
            Architecture::ARM64 => ResourceRequirements {
                alu_units: 8,
                memory_ports: 3,
                vector_units: 2,
                branch_units: 2,
            },
            Architecture::RISCV64 => ResourceRequirements {
                alu_units: 6,
                memory_ports: 2,
                vector_units: 1,
                branch_units: 1,
            },
        }
    }
}

/// 并行优化统计
#[derive(Debug, Clone, Default)]
pub struct ParallelismStats {
    /// 总指令数
    pub total_instructions: usize,
    /// 并行组数
    pub parallel_groups: usize,
    /// 最大并行度
    pub max_parallelism: usize,
    /// 平均并行度
    pub avg_parallelism: f64,
    /// 识别的并行指令数
    pub parallel_instructions: usize,
    /// 并行化率
    pub parallelization_rate: f64,
}

/// 指令级并行优化器
pub struct InstructionParallelizer {
    /// 指令节点
    nodes: Vec<InstructionNode>,
    /// 依赖边
    dependencies: Vec<DependencyEdge>,
    /// 并行组
    parallel_groups: Vec<ParallelGroup>,
    /// 目标架构资源
    target_resources: ResourceRequirements,
    /// 优化统计
    stats: ParallelismStats,
}

impl InstructionParallelizer {
    /// 创建新的指令并行优化器
    pub fn new(target_resources: ResourceRequirements) -> Self {
        Self {
            nodes: Vec::new(),
            dependencies: Vec::new(),
            parallel_groups: Vec::new(),
            target_resources,
            stats: ParallelismStats::default(),
        }
    }

    /// 分析指令并行性
    pub fn analyze_parallelism(&mut self, instructions: &[IROp]) -> Result<(), String> {
        // 构建依赖图
        self.build_dependency_graph(instructions)?;

        // 识别并行组
        self.identify_parallel_groups()?;

        // 计算统计信息
        self.calculate_stats();

        Ok(())
    }

    /// 优化指令序列以提高并行性
    pub fn optimize_instruction_sequence(&mut self, ops: &[IROp]) -> Result<Vec<IROp>, String> {
        // 分析并行性
        self.analyze_parallelism(ops)?;

        // 将指令按并行组重组
        let mut optimized_ops = Vec::new();
        
        // 按并行组顺序添加指令
        // 这里简化实现，实际应该考虑资源约束和指令调度
        for group in &self.parallel_groups {
            for &instr_idx in &group.instructions {
                if instr_idx < self.nodes.len() {
                    optimized_ops.push(self.nodes[instr_idx].op.clone());
                }
            }
        }
        
        // 如果没有生成优化的指令，返回原始序列
        if optimized_ops.is_empty() {
            optimized_ops.extend_from_slice(ops);
        }
        
        Ok(optimized_ops)
    }

    /// 构建指令依赖图
    fn build_dependency_graph(&mut self, instructions: &[IROp]) -> Result<(), String> {
        self.nodes.clear();
        self.dependencies.clear();

        // 创建指令节点
        for (idx, op) in instructions.iter().enumerate() {
            let (reads, writes) = self.analyze_instruction_io(op);
            let latency = self.estimate_instruction_latency(op);

            self.nodes.push(InstructionNode {
                index: idx,
                op: op.clone(),
                reads,
                writes,
                predecessors: Vec::new(),
                successors: Vec::new(),
                can_parallel: false,
                parallel_group: None,
                latency,
            });
        }

        // 构建依赖边
        for i in 0..self.nodes.len() {
            for j in (i + 1)..self.nodes.len() {
                if let Some(dep) = self.check_dependency(&self.nodes[i], &self.nodes[j]) {
                    self.dependencies.push(dep);
                    self.nodes[i].successors.push(j);
                    self.nodes[j].predecessors.push(i);
                }
            }
        }

        Ok(())
    }

    /// 分析指令的输入输出寄存器
    fn analyze_instruction_io(&self, op: &IROp) -> (Vec<RegId>, Vec<RegId>) {
        let mut reads = Vec::new();
        let mut writes = Vec::new();

        match op {
            IROp::Add { dst, src1, src2 } |
            IROp::Sub { dst, src1, src2 } |
            IROp::Mul { dst, src1, src2 } |
            IROp::Div { dst, src1, src2, signed: _ } |
            IROp::Rem { dst, src1, src2, signed: _ } |
            IROp::And { dst, src1, src2 } |
            IROp::Or { dst, src1, src2 } |
            IROp::Xor { dst, src1, src2 } => {
                reads.push(*src1);
                reads.push(*src2);
                writes.push(*dst);
            }
            IROp::Sll { dst, src, shreg } |
            IROp::Srl { dst, src, shreg } |
            IROp::Sra { dst, src, shreg } => {
                reads.push(*src);
                reads.push(*shreg);
                writes.push(*dst);
            }
            IROp::Load { dst, .. } => {
                writes.push(*dst);
            }
            IROp::Store { src, .. } => {
                reads.push(*src);
            }
            IROp::MovImm { dst, .. } => {
                writes.push(*dst);
            }
            IROp::Mov { dst, src } => {
                reads.push(*src);
                writes.push(*dst);
            },
            _ => {} // 处理所有其他未明确列出的 IROp 变体

        }

        (reads, writes)
    }

    /// 估算指令延迟
    fn estimate_instruction_latency(&self, op: &IROp) -> u32 {
        match op {
            IROp::Add { .. } |
            IROp::Sub { .. } |
            IROp::And { .. } |
            IROp::Or { .. } |
            IROp::Xor { .. } |
            IROp::Sll { .. } |
            IROp::Srl { .. } |
            IROp::Sra { .. } |
            IROp::Mov { .. } => 1,
            IROp::Mul { .. } => 3,
            IROp::Div { .. } |
            IROp::Rem { .. } => 10,
            IROp::Load { .. } => 3,
            IROp::Store { .. } => 2,
            IROp::MovImm { .. } => 1, // 替换Const
            _ => 1, // 处理其他不存在的变体
        }
    }

    /// 检查两个指令之间的依赖关系
    fn check_dependency(&self, from: &InstructionNode, to: &InstructionNode) -> Option<DependencyEdge> {
        // 检查真依赖（RAW - Read After Write）
        for &write_reg in &from.writes {
            if to.reads.contains(&write_reg) {
                return Some(DependencyEdge {
                    from: from.index,
                    to: to.index,
                    dep_type: DependencyType::True,
                    register: write_reg,
                });
            }
        }

        // 检查反依赖（WAR - Write After Read）
        for &read_reg in &from.reads {
            if to.writes.contains(&read_reg) {
                return Some(DependencyEdge {
                    from: from.index,
                    to: to.index,
                    dep_type: DependencyType::Anti,
                    register: read_reg,
                });
            }
        }

        // 检查输出依赖（WAW - Write After Write）
        for &write_reg in &from.writes {
            if to.writes.contains(&write_reg) {
                return Some(DependencyEdge {
                    from: from.index,
                    to: to.index,
                    dep_type: DependencyType::Output,
                    register: write_reg,
                });
            }
        }

        None
    }

    /// 识别并行组
    fn identify_parallel_groups(&mut self) -> Result<(), String> {
        self.parallel_groups.clear();

        // 使用拓扑排序和资源约束来识别并行组
        let mut ready: Vec<usize> = (0..self.nodes.len()).collect();
        let mut processed = HashSet::new();
        let mut group_id = 0;

        // 初始就绪集合：没有前驱的节点
        ready.retain(|&i| self.nodes[i].predecessors.is_empty());

        while !ready.is_empty() {
            // 创建新的并行组
            let mut group = ParallelGroup {
                id: group_id,
                instructions: Vec::new(),
                latency: 0,
                resource_requirements: ResourceRequirements::default(),
            };

            // 选择可以并行执行的指令
            let mut selected = Vec::new();
            let mut current_resources = ResourceRequirements::default();

            // 按优先级排序就绪指令（延迟高的优先）
            ready.sort_by(|&a, &b| self.nodes[b].latency.cmp(&self.nodes[a].latency));

            for &idx in &ready {
                if processed.contains(&idx) {
                    continue;
                }

                let node = &self.nodes[idx];
                let node_resources = self.estimate_resource_requirements(&node.op);

                // 检查资源约束
                if self.can_add_to_group(&node_resources, &current_resources) {
                    selected.push(idx);
                    group.instructions.push(idx);
                    group.latency = group.latency.max(node.latency);
                    self.add_resources(&mut current_resources, &node_resources);
                    self.nodes[idx].parallel_group = Some(group_id);
                    self.nodes[idx].can_parallel = true;
                }
            }

            // 如果没有指令可以添加到当前组，创建单指令组
            if group.instructions.is_empty() && !ready.is_empty() {
                let idx = ready[0];
                let node = &self.nodes[idx];
                let node_resources = self.estimate_resource_requirements(&node.op);
                
                group.instructions.push(idx);
                group.latency = node.latency;
                self.add_resources(&mut current_resources, &node_resources);
                self.nodes[idx].parallel_group = Some(group_id);
                self.nodes[idx].can_parallel = true;
            }

            group.resource_requirements = current_resources.clone();
            let instructions = group.instructions.clone();
            self.parallel_groups.push(group);

            // 更新就绪集合
            for &idx in &instructions {
                processed.insert(idx);
                ready.retain(|&i| i != idx);
                
                // 添加新的就绪指令：所有前驱都已处理
                for &succ_idx in &self.nodes[idx].successors {
                    if !processed.contains(&succ_idx) {
                        let all_predecessors_processed = self.nodes[succ_idx].predecessors
                            .iter()
                            .all(|&pred_idx| processed.contains(&pred_idx));
                        
                        if all_predecessors_processed && !ready.contains(&succ_idx) {
                            ready.push(succ_idx);
                        }
                    }
                }
            }

            group_id += 1;
        }

        Ok(())
    }

    /// 估算指令的资源需求
    fn estimate_resource_requirements(&self, op: &IROp) -> ResourceRequirements {
        match op {
            IROp::Add { .. } |
            IROp::Sub { .. } |
            IROp::And { .. } |
            IROp::Or { .. } |
            IROp::Xor { .. } |
            IROp::Sll { .. } |
            IROp::Srl { .. } |
            IROp::Sra { .. } |
            IROp::CmpEq { .. } |
            IROp::CmpNe { .. } |
            IROp::CmpLt { .. } |
            IROp::CmpLtU { .. } |
            IROp::CmpGe { .. } |
            IROp::CmpGeU { .. } => {
                ResourceRequirements {
                    alu_units: 1,
                    ..Default::default()
                }
            }
            IROp::Mul { .. } => {
                ResourceRequirements {
                    alu_units: 1,
                    ..Default::default()
                }
            }
            IROp::Div { .. } |
            IROp::Rem { .. } => {
                ResourceRequirements {
                    alu_units: 1,
                    ..Default::default()
                }
            }
            IROp::Load { .. } => {
                ResourceRequirements {
                    memory_ports: 1,
                    ..Default::default()
                }
            }
            IROp::Store { .. } => {
                ResourceRequirements {
                    memory_ports: 1,
                    ..Default::default()
                }
            }

            IROp::Mov { .. } => {
                ResourceRequirements {
                    alu_units: 1,
                    ..Default::default()
                }
            }
            IROp::MovImm { .. } => {
                ResourceRequirements {
                    alu_units: 1,
                    ..Default::default()
                }
            }
            _ => {
                ResourceRequirements {
                    alu_units: 1,
                    branch_units: 1,
                    ..Default::default()
                }
            }
        }
    }

    /// 检查是否可以将指令添加到当前并行组
    fn can_add_to_group(
        &self,
        node_resources: &ResourceRequirements,
        current_resources: &ResourceRequirements,
    ) -> bool {
        current_resources.alu_units + node_resources.alu_units <= self.target_resources.alu_units
            && current_resources.memory_ports + node_resources.memory_ports <= self.target_resources.memory_ports
            && current_resources.vector_units + node_resources.vector_units <= self.target_resources.vector_units
            && current_resources.branch_units + node_resources.branch_units <= self.target_resources.branch_units
    }

    /// 添加资源需求
    fn add_resources(&self, current: &mut ResourceRequirements, to_add: &ResourceRequirements) {
        current.alu_units += to_add.alu_units;
        current.memory_ports += to_add.memory_ports;
        current.vector_units += to_add.vector_units;
        current.branch_units += to_add.branch_units;
    }

    /// 计算并行性统计信息
    fn calculate_stats(&mut self) {
        let total_instructions = self.nodes.len();
        let parallel_groups = self.parallel_groups.len();
        let parallel_instructions = self.nodes.iter()
            .filter(|n| n.can_parallel)
            .count();
        
        let max_parallelism = self.parallel_groups
            .iter()
            .map(|g| g.instructions.len())
            .max()
            .unwrap_or(0);
        
        let avg_parallelism = if parallel_groups > 0 {
            self.parallel_groups
                .iter()
                .map(|g| g.instructions.len())
                .sum::<usize>() as f64 / parallel_groups as f64
        } else {
            0.0
        };
        
        let parallelization_rate = if total_instructions > 0 {
            parallel_instructions as f64 / total_instructions as f64
        } else {
            0.0
        };

        self.stats = ParallelismStats {
            total_instructions,
            parallel_groups,
            max_parallelism,
            avg_parallelism,
            parallel_instructions,
            parallelization_rate,
        };
    }

    /// 获取并行组
    pub fn get_parallel_groups(&self) -> &[ParallelGroup] {
        &self.parallel_groups
    }

    /// 获取指令节点
    pub fn get_instruction_nodes(&self) -> &[InstructionNode] {
        &self.nodes
    }

    /// 获取依赖边
    pub fn get_dependencies(&self) -> &[DependencyEdge] {
        &self.dependencies
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &ParallelismStats {
        &self.stats
    }

    /// 重新调度指令以优化并行性
    pub fn reschedule_for_parallelism(&self, original_instructions: &[IROp]) -> Vec<IROp> {
        let mut rescheduled = Vec::with_capacity(original_instructions.len());
        
        // 按并行组重新组织指令
        for group in &self.parallel_groups {
            for &idx in &group.instructions {
                if let Some(node) = self.nodes.get(idx) {
                    rescheduled.push(node.op.clone());
                }
            }
        }
        
        // 添加未并行化的指令
        for node in &self.nodes {
            if !node.can_parallel {
                rescheduled.push(node.op.clone());
            }
        }
        
        rescheduled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::IRBuilder;

    #[test]
    fn test_dependency_analysis() {
        let target_resources = ResourceRequirements {
            alu_units: 2,
            memory_ports: 1,
            vector_units: 1,
            branch_units: 1,
        };
        
        let mut parallelizer = InstructionParallelizer::new(target_resources);
        
        // 创建测试指令序列
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::Const { dst: 0, value: 10 });
        builder.push(IROp::Const { dst: 1, value: 20 });
        builder.push(IROp::Add { dst: 2, src1: 0, src2: 1 });
        builder.push(IROp::Const { dst: 3, value: 30 });
        builder.push(IROp::Mul { dst: 4, src1: 2, src2: 3 });
        let instructions = builder.build().ops;
        
        // 分析并行性
        let result = parallelizer.analyze_parallelism(&instructions);
        assert!(result.is_ok());
        
        // 验证依赖关系
        let dependencies = parallelizer.get_dependencies();
        assert!(!dependencies.is_empty());
        
        // 验证并行组
        let groups = parallelizer.get_parallel_groups();
        assert!(!groups.is_empty());
        
        // 验证统计信息
        let stats = parallelizer.get_stats();
        assert_eq!(stats.total_instructions, 5);
        assert!(stats.parallel_groups > 0);
        assert!(stats.parallelization_rate > 0.0);
    }

    #[test]
    fn test_parallel_rescheduling() {
        let target_resources = ResourceRequirements {
            alu_units: 2,
            memory_ports: 1,
            vector_units: 1,
            branch_units: 1,
        };
        
        let mut parallelizer = InstructionParallelizer::new(target_resources);
        
        // 创建测试指令序列
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::Const { dst: 0, value: 10 });
        builder.push(IROp::Const { dst: 1, value: 20 });
        builder.push(IROp::Add { dst: 2, src1: 0, src2: 1 });
        builder.push(IROp::Const { dst: 3, value: 30 });
        builder.push(IROp::Mul { dst: 4, src1: 2, src2: 3 });
        let instructions = builder.build().ops;
        
        // 分析并行性
        let _ = parallelizer.analyze_parallelism(&instructions);
        
        // 重新调度指令
        let rescheduled = parallelizer.reschedule_for_parallelism(&instructions);
        
        // 验证指令数量不变
        assert_eq!(rescheduled.len(), instructions.len());
        
        // 验证所有指令都存在
        for original_insn in &instructions {
            assert!(rescheduled.contains(original_insn));
        }
    }
}