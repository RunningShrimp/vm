//! 指令调度器模块
//!
//! 实现延迟感知调度、关键路径优化和超标量调度

use std::collections::{HashMap, HashSet, VecDeque};
use vm_ir::{IROp, RegId};

/// 依赖类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyType {
    /// RAW (Read-After-Write) 数据依赖
    Raw,
    /// WAR (Write-After-Read) 反依赖
    War,
    /// WAW (Write-After-Write) 输出依赖
    Waw,
    /// 控制依赖（指令依赖于分支结果）
    Control,
    /// 内存依赖（Load/Store之间的别名依赖）
    Memory,
    /// 资源依赖（功能单元冲突）
    Resource,
}

/// 依赖关系
#[derive(Debug, Clone)]
pub struct Dependency {
    /// 依赖类型
    pub dep_type: DependencyType,
    /// 依赖的指令索引
    pub from: usize,
    /// 被依赖的指令索引
    pub to: usize,
    /// 延迟（cycles）
    pub latency: u32,
}

/// 指令调度器
///
/// 优化：实现更复杂的依赖分析，包括：
/// - 数据依赖（RAW, WAR, WAW）
/// - 控制依赖
/// - 内存别名分析
/// - 资源依赖（功能单元）
/// - 延迟感知调度
/// - 超标量调度（支持每个周期调度多条指令）
pub struct InstructionScheduler {
    /// 依赖图（包含所有类型的依赖）
    dependency_graph: HashMap<usize, Vec<Dependency>>,
    /// 指令延迟表
    instruction_latencies: HashMap<usize, u32>,
    /// 资源使用表（功能单元）
    resource_usage: HashMap<usize, Vec<String>>,
    /// 关键路径长度
    critical_path_length: u32,
    /// 超标量宽度（每个周期可以调度的指令数）
    issue_width: usize,
    /// 功能单元数量限制
    functional_units: HashMap<String, usize>,
}

impl InstructionScheduler {
    pub fn new() -> Self {
        Self {
            dependency_graph: HashMap::new(),
            instruction_latencies: HashMap::new(),
            resource_usage: HashMap::new(),
            critical_path_length: 0,
            issue_width: 4, // 默认4路超标量
            functional_units: {
                let mut units = HashMap::new();
                units.insert("alu_unit".to_string(), 2); // 2个ALU单元
                units.insert("memory_unit".to_string(), 1); // 1个内存单元
                units.insert("multiply_unit".to_string(), 1); // 1个乘法单元
                units
            },
        }
    }

    /// 创建指定宽度的超标量调度器
    pub fn with_issue_width(issue_width: usize) -> Self {
        Self {
            dependency_graph: HashMap::new(),
            instruction_latencies: HashMap::new(),
            resource_usage: HashMap::new(),
            critical_path_length: 0,
            issue_width,
            functional_units: {
                let mut units = HashMap::new();
                units.insert("alu_unit".to_string(), 2);
                units.insert("memory_unit".to_string(), 1);
                units.insert("multiply_unit".to_string(), 1);
                units
            },
        }
    }

    fn latency(&self, op: &IROp) -> u32 {
        match op {
            IROp::Load { .. } | IROp::Store { .. } => 3,
            IROp::Mul { .. } | IROp::Div { .. } | IROp::Rem { .. } => 3,
            _ => 1,
        }
    }

    /// 构建依赖图（增强版：支持RAW, WAR, WAW, 控制依赖, 内存别名分析）
    pub fn build_dependency_graph(&mut self, ops: &[IROp]) {
        self.dependency_graph.clear();
        self.instruction_latencies.clear();
        self.resource_usage.clear();

        // 计算指令延迟和资源使用
        for (i, op) in ops.iter().enumerate() {
            self.instruction_latencies.insert(i, self.latency(op));
            self.resource_usage
                .insert(i, self.get_resource_requirements(op));
        }

        // 识别条件比较指令（用于控制依赖分析）
        let condition_regs: HashSet<RegId> = ops
            .iter()
            .filter_map(|op| match op {
                IROp::CmpEq { dst, .. }
                | IROp::CmpNe { dst, .. }
                | IROp::CmpLt { dst, .. }
                | IROp::CmpLtU { dst, .. }
                | IROp::CmpGe { dst, .. }
                | IROp::CmpGeU { dst, .. } => Some(*dst),
                _ => None,
            })
            .collect();

        // 构建完整的依赖图（包括RAW, WAR, WAW, 控制依赖, 内存依赖）
        for (i, op) in ops.iter().enumerate() {
            let mut dependencies = Vec::new();

            let current_reads = self.collect_read_regs(op);
            let current_writes = self.collect_written_regs(op);
            let current_latency = self.instruction_latencies.get(&i).copied().unwrap_or(1);

            // 检查与前面指令的依赖关系
            for (j, prev_op) in ops.iter().enumerate().take(i) {
                let prev_reads = self.collect_read_regs(prev_op);
                let prev_writes = self.collect_written_regs(prev_op);
                let prev_latency = self.instruction_latencies.get(&j).copied().unwrap_or(1);

                // RAW (Read-After-Write) 数据依赖
                for &read_reg in &current_reads {
                    if prev_writes.contains(&read_reg) {
                        dependencies.push(Dependency {
                            dep_type: DependencyType::Raw,
                            from: j,
                            to: i,
                            latency: prev_latency,
                        });
                        break;
                    }
                }

                // WAR (Write-After-Read) 反依赖
                for &write_reg in &current_writes {
                    if prev_reads.contains(&write_reg) {
                        dependencies.push(Dependency {
                            dep_type: DependencyType::War,
                            from: j,
                            to: i,
                            latency: 0,
                        });
                        break;
                    }
                }

                // WAW (Write-After-Write) 输出依赖
                for &write_reg in &current_writes {
                    if prev_writes.contains(&write_reg) {
                        dependencies.push(Dependency {
                            dep_type: DependencyType::Waw,
                            from: j,
                            to: i,
                            latency: 0,
                        });
                        break;
                    }
                }

                // 控制依赖
                for &read_reg in &current_reads {
                    if condition_regs.contains(&read_reg) {
                        if let Some(cond_producer) = self.find_condition_producer(j, read_reg, ops)
                        {
                            dependencies.push(Dependency {
                                dep_type: DependencyType::Control,
                                from: cond_producer,
                                to: i,
                                latency: 1,
                            });
                        }
                    }
                }

                // 内存依赖
                if self.has_memory_dependency_enhanced(op, prev_op, i, j, ops) {
                    dependencies.push(Dependency {
                        dep_type: DependencyType::Memory,
                        from: j,
                        to: i,
                        latency: prev_latency,
                    });
                }

                // 资源依赖
                if self.has_resource_conflict(i, j) {
                    dependencies.push(Dependency {
                        dep_type: DependencyType::Resource,
                        from: j,
                        to: i,
                        latency: prev_latency,
                    });
                }
            }

            self.dependency_graph.insert(i, dependencies);
        }

        // 计算关键路径长度
        self.critical_path_length = self.compute_critical_path(ops);
    }

    /// 查找产生条件寄存器的指令
    fn find_condition_producer(
        &self,
        max_idx: usize,
        cond_reg: RegId,
        ops: &[IROp],
    ) -> Option<usize> {
        for j in (0..max_idx).rev() {
            let prev_writes = self.collect_written_regs(&ops[j]);
            if prev_writes.contains(&cond_reg) {
                match &ops[j] {
                    IROp::CmpEq { .. }
                    | IROp::CmpNe { .. }
                    | IROp::CmpLt { .. }
                    | IROp::CmpLtU { .. }
                    | IROp::CmpGe { .. }
                    | IROp::CmpGeU { .. } => return Some(j),
                    _ => {}
                }
            }
        }
        None
    }

    /// 检查内存依赖（增强版）
    fn has_memory_dependency_enhanced(
        &self,
        current: &IROp,
        previous: &IROp,
        current_idx: usize,
        previous_idx: usize,
        ops: &[IROp],
    ) -> bool {
        match (current, previous) {
            (
                IROp::Load {
                    base: base1,
                    offset: offset1,
                    ..
                },
                IROp::Store {
                    base: base2,
                    offset: offset2,
                    ..
                },
            ) => {
                if base1 == base2 {
                    if let (Some(off1), Some(off2)) = (
                        self.get_constant_offset(*base1, *offset1, previous_idx, ops),
                        self.get_constant_offset(*base2, *offset2, current_idx, ops),
                    ) {
                        if off1 == off2 {
                            return true;
                        }
                    }
                    return true;
                }
                self.may_alias(*base1, *base2, current_idx, previous_idx, ops)
            }
            (
                IROp::Store {
                    base: base1,
                    offset: offset1,
                    ..
                },
                IROp::Load {
                    base: base2,
                    offset: offset2,
                    ..
                },
            ) => {
                if base1 == base2 {
                    if let (Some(off1), Some(off2)) = (
                        self.get_constant_offset(*base1, *offset1, previous_idx, ops),
                        self.get_constant_offset(*base2, *offset2, current_idx, ops),
                    ) {
                        if off1 == off2 {
                            return true;
                        }
                    }
                    return true;
                }
                self.may_alias(*base1, *base2, current_idx, previous_idx, ops)
            }
            (
                IROp::Store {
                    base: base1,
                    offset: offset1,
                    ..
                },
                IROp::Store {
                    base: base2,
                    offset: offset2,
                    ..
                },
            ) => {
                if base1 == base2 {
                    if let (Some(off1), Some(off2)) = (
                        self.get_constant_offset(*base1, *offset1, previous_idx, ops),
                        self.get_constant_offset(*base2, *offset2, current_idx, ops),
                    ) {
                        if off1 == off2 {
                            return true;
                        }
                    }
                    return true;
                }
                self.may_alias(*base1, *base2, current_idx, previous_idx, ops)
            }
            (IROp::Load { .. }, IROp::Load { .. }) => false,
            _ => false,
        }
    }

    /// 获取常量offset
    fn get_constant_offset(
        &self,
        _base: RegId,
        offset: i64,
        _max_idx: usize,
        _ops: &[IROp],
    ) -> Option<i64> {
        Some(offset)
    }

    /// 检查两个base寄存器是否可能指向同一内存区域
    fn may_alias(
        &self,
        base1: RegId,
        base2: RegId,
        idx1: usize,
        idx2: usize,
        ops: &[IROp],
    ) -> bool {
        if base1 == base2 {
            return true;
        }

        for j in 0..idx2.min(idx1) {
            let prev_writes = self.collect_written_regs(&ops[j]);
            if prev_writes.contains(&base2) {
                match &ops[j] {
                    IROp::Add { dst, src1, src2 } if *dst == base2 => {
                        if *src1 == base1 || *src2 == base1 {
                            return true;
                        }
                    }
                    IROp::MovImm { dst, .. } if *dst == base2 => {
                        return true;
                    }
                    _ => {}
                }
            }
        }

        false
    }

    /// 获取指令的资源需求
    fn get_resource_requirements(&self, op: &IROp) -> Vec<String> {
        match op {
            IROp::Load { .. } | IROp::Store { .. } => vec!["memory_unit".to_string()],
            IROp::Mul { .. } | IROp::Div { .. } | IROp::Rem { .. } => {
                vec!["multiply_unit".to_string()]
            }
            _ => vec!["alu_unit".to_string()],
        }
    }

    /// 检查资源冲突
    fn has_resource_conflict(&self, inst1: usize, inst2: usize) -> bool {
        if let (Some(res1), Some(res2)) = (
            self.resource_usage.get(&inst1),
            self.resource_usage.get(&inst2),
        ) {
            for r1 in res1.iter() {
                if res2.contains(r1) {
                    let available_units = self.functional_units.get(r1).copied().unwrap_or(1);
                    if available_units <= 1 {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// 检查指令是否可以并行执行
    fn can_execute_parallel(&self, inst1: usize, inst2: usize, ops: &[IROp]) -> bool {
        if let Some(deps) = self.dependency_graph.get(&inst2) {
            for dep in deps {
                if dep.from == inst1 {
                    return false;
                }
            }
        }

        if self.has_resource_conflict(inst1, inst2) {
            return false;
        }

        true
    }

    /// 计算关键路径长度
    fn compute_critical_path(&self, ops: &[IROp]) -> u32 {
        let mut max_path_length = 0u32;
        let mut path_lengths = vec![0u32; ops.len()];

        for i in 0..ops.len() {
            let mut max_dep_length = 0u32;

            if let Some(deps) = self.dependency_graph.get(&i) {
                for dep in deps {
                    let dep_path_length = path_lengths[dep.from] + dep.latency;
                    max_dep_length = max_dep_length.max(dep_path_length);
                }
            }

            let latency = self.instruction_latencies.get(&i).copied().unwrap_or(1);
            path_lengths[i] = max_dep_length + latency;
            max_path_length = max_path_length.max(path_lengths[i]);
        }

        max_path_length
    }

    /// 调度指令（增强版：延迟感知调度 + 关键路径优化 + 超标量调度）
    pub fn schedule(&self, ops: &[IROp]) -> Vec<usize> {
        let mut ready_queue = VecDeque::new();
        let mut scheduled = Vec::new();
        let mut scheduled_set = HashSet::new();
        let mut in_degree: HashMap<usize, usize> = HashMap::new();

        // 初始化入度计数
        for i in 0..ops.len() {
            let degree = self.dependency_graph.get(&i).map_or(0, |deps| deps.len());
            in_degree.insert(i, degree);
            if degree == 0 {
                ready_queue.push_back(i);
            }
        }

        // 超标量调度：每个周期可以调度多条指令
        while !ready_queue.is_empty() || !scheduled_set.is_empty() {
            let mut cycle_schedule = Vec::new();
            let mut cycle_resources = HashMap::new();

            let mut candidates: Vec<usize> = ready_queue.iter().copied().collect();

            // 按优先级排序
            candidates.sort_by(|&a, &b| {
                let priority_a = self.compute_instruction_priority(a, &scheduled_set, ops);
                let priority_b = self.compute_instruction_priority(b, &scheduled_set, ops);
                priority_b.cmp(&priority_a)
            });

            // 选择可以并行执行的指令
            for &inst_idx in &candidates {
                if cycle_schedule.len() >= self.issue_width {
                    break;
                }

                let mut can_schedule = true;
                if let Some(resources) = self.resource_usage.get(&inst_idx) {
                    for resource in resources {
                        let used = cycle_resources.get(resource).copied().unwrap_or(0);
                        let available = self.functional_units.get(resource).copied().unwrap_or(1);
                        if used >= available {
                            can_schedule = false;
                            break;
                        }
                    }
                }

                if can_schedule {
                    for &scheduled_idx in &cycle_schedule {
                        if !self.can_execute_parallel(inst_idx, scheduled_idx, ops) {
                            can_schedule = false;
                            break;
                        }
                    }
                }

                if can_schedule {
                    cycle_schedule.push(inst_idx);
                    if let Some(resources) = self.resource_usage.get(&inst_idx) {
                        for resource in resources {
                            *cycle_resources.entry(resource.clone()).or_insert(0) += 1;
                        }
                    }
                }
            }

            // 将选中的指令加入调度序列
            for inst_idx in &cycle_schedule {
                ready_queue.retain(|&x| x != *inst_idx);
                scheduled.push(*inst_idx);
                scheduled_set.insert(*inst_idx);
            }

            // 更新依赖计数
            for i in 0..ops.len() {
                if !scheduled_set.contains(&i) {
                    let can_schedule = self.dependency_graph.get(&i).map_or(true, |dependencies| {
                        dependencies
                            .iter()
                            .all(|dep| scheduled_set.contains(&dep.from))
                    });
                    if can_schedule {
                        if !ready_queue.contains(&i) {
                            ready_queue.push_back(i);
                        }
                    }
                }
            }
        }

        scheduled
    }

    /// 计算指令优先级
    fn compute_instruction_priority(
        &self,
        inst_idx: usize,
        scheduled: &HashSet<usize>,
        ops: &[IROp],
    ) -> u32 {
        let latency = self
            .instruction_latencies
            .get(&inst_idx)
            .copied()
            .unwrap_or(1);
        let critical_path_remaining =
            self.compute_remaining_critical_path(inst_idx, scheduled, ops);
        latency + critical_path_remaining
    }

    /// 计算从指定指令到结束的剩余关键路径长度
    fn compute_remaining_critical_path(
        &self,
        start: usize,
        scheduled: &HashSet<usize>,
        ops: &[IROp],
    ) -> u32 {
        let mut max_path = 0u32;
        let mut visited = HashSet::new();
        let mut stack = vec![(start, 0u32)];

        while let Some((inst_idx, path_length)) = stack.pop() {
            if visited.contains(&inst_idx) || scheduled.contains(&inst_idx) {
                continue;
            }
            visited.insert(inst_idx);

            let latency = self
                .instruction_latencies
                .get(&inst_idx)
                .copied()
                .unwrap_or(1);
            let new_length = path_length + latency;
            max_path = max_path.max(new_length);

            for (j, _) in ops.iter().enumerate() {
                if let Some(deps) = self.dependency_graph.get(&j) {
                    for dep in deps {
                        if dep.from == inst_idx && !scheduled.contains(&j) {
                            stack.push((j, new_length + dep.latency));
                        }
                    }
                }
            }
        }

        max_path
    }

    /// 收集操作中读取的寄存器
    fn collect_read_regs(&self, op: &IROp) -> Vec<RegId> {
        match op {
            IROp::Add { src1, src2, .. }
            | IROp::Sub { src1, src2, .. }
            | IROp::Mul { src1, src2, .. }
            | IROp::Div { src1, src2, .. }
            | IROp::Rem { src1, src2, .. }
            | IROp::And { src1, src2, .. }
            | IROp::Or { src1, src2, .. }
            | IROp::Xor { src1, src2, .. } => vec![*src1, *src2],

            IROp::Sll { src, shreg, .. }
            | IROp::Srl { src, shreg, .. }
            | IROp::Sra { src, shreg, .. } => vec![*src, *shreg],

            IROp::Not { src, .. } => vec![*src],
            IROp::Load { base, .. } => vec![*base],

            IROp::Store { src, base, .. } => vec![*src, *base],

            IROp::CmpEq { lhs, rhs, .. }
            | IROp::CmpNe { lhs, rhs, .. }
            | IROp::CmpLt { lhs, rhs, .. }
            | IROp::CmpLtU { lhs, rhs, .. }
            | IROp::CmpGe { lhs, rhs, .. }
            | IROp::CmpGeU { lhs, rhs, .. } => vec![*lhs, *rhs],

            _ => vec![],
        }
    }

    /// 收集操作中写入的寄存器
    fn collect_written_regs(&self, op: &IROp) -> Vec<RegId> {
        match op {
            IROp::Add { dst, .. }
            | IROp::Sub { dst, .. }
            | IROp::Mul { dst, .. }
            | IROp::Div { dst, .. }
            | IROp::Rem { dst, .. }
            | IROp::AddImm { dst, .. }
            | IROp::MulImm { dst, .. }
            | IROp::MovImm { dst, .. }
            | IROp::And { dst, .. }
            | IROp::Or { dst, .. }
            | IROp::Xor { dst, .. }
            | IROp::Not { dst, .. }
            | IROp::Sll { dst, .. }
            | IROp::Srl { dst, .. }
            | IROp::Sra { dst, .. }
            | IROp::SllImm { dst, .. }
            | IROp::SrlImm { dst, .. }
            | IROp::SraImm { dst, .. }
            | IROp::Load { dst, .. }
            | IROp::CmpEq { dst, .. }
            | IROp::CmpNe { dst, .. }
            | IROp::CmpLt { dst, .. }
            | IROp::CmpLtU { dst, .. }
            | IROp::CmpGe { dst, .. }
            | IROp::CmpGeU { dst, .. } => vec![*dst],

            _ => vec![],
        }
    }
}


