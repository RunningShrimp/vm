//! 指令调度器实现
//!
//! 实现基于依赖关系的指令调度，优化指令执行顺序

use std::collections::{HashMap, HashSet, VecDeque};
use vm_ir::{IROp, RegId};

/// 指令依赖类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyType {
    /// 数据依赖（读后写）
    Data,
    /// 反依赖（写后读）
    Anti,
    /// 输出依赖（写后写）
    Output,
    /// 控制依赖（分支）
    Control,
}

/// 指令依赖关系
#[derive(Debug, Clone)]
pub struct Dependency {
    /// 源指令索引
    pub from: usize,
    /// 目标指令索引
    pub to: usize,
    /// 依赖类型
    pub dep_type: DependencyType,
    /// 延迟周期数
    pub latency: u32,
}

/// 指令调度器
pub struct InstructionScheduler {
    /// 依赖图
    dependency_graph: HashMap<usize, Vec<Dependency>>,
    /// 反向依赖图（用于拓扑排序）
    reverse_graph: HashMap<usize, Vec<usize>>,
    /// 指令就绪时间
    ready_times: HashMap<usize, u32>,
    /// 当前周期
    current_cycle: u32,
    /// 统计信息
    stats: SchedulerStats,
}

/// 调度器统计信息
#[derive(Debug, Clone, Default)]
pub struct SchedulerStats {
    /// 总调度次数
    pub total_schedules: u64,
    /// 重排指令数
    pub reordered_instructions: u64,
    /// 平均调度时间（纳秒）
    pub avg_scheduling_time_ns: u64,
    /// 关键路径长度
    pub critical_path_length: u32,
}

impl InstructionScheduler {
    /// 创建新的指令调度器
    pub fn new() -> Self {
        Self {
            dependency_graph: HashMap::new(),
            reverse_graph: HashMap::new(),
            ready_times: HashMap::new(),
            current_cycle: 0,
            stats: SchedulerStats::default(),
        }
    }
    
    /// 构建指令依赖图
    pub fn build_dependency_graph(&mut self, ops: &[IROp]) {
        let start_time = std::time::Instant::now();

        self.dependency_graph.clear();
        self.reverse_graph.clear();
        self.ready_times.clear();

        let mut last_writer: HashMap<RegId, usize> = HashMap::new(); // 记录每个寄存器的最新写入者
        let mut regs_read = Vec::new();
        let mut regs_written = Vec::new();

        // 从后向前遍历指令，这样可以O(N)时间复杂度构建依赖图
        // 注意：这里我们需要先从后向前遍历一次来找到所有依赖关系
        // 然后再将依赖关系正确添加到依赖图中
        // 因为依赖是从较早的指令到较晚的指令，所以我们需要先记录所有依赖
        let n = ops.len();
        let mut dependencies: Vec<Dependency> = Vec::new();
        
        for i in (0..n).rev() {
            let op_i = &ops[i];
            Self::collect_read_regs(op_i, &mut regs_read);
            Self::collect_written_regs(op_i, &mut regs_written);
            
            // 检查数据依赖：i 读取的寄存器 j 是否被后面的指令 (j > i) 写入
            for &reg in regs_read.iter() {
                if let Some(&j) = last_writer.get(&reg) {
                    // 数据依赖：i -> j (因为 i 写入 reg，j 读取 reg，且 i < j)
                    let dep = Dependency {
                        from: i,
                        to: j,
                        dep_type: DependencyType::Data,
                        latency: self.get_latency(op_i, &ops[j]),
                    };
                    dependencies.push(dep.clone());
                    
                    self.add_dependency(dep.from, dep.to, dep);
                }
            }
            
            // 检查输出依赖：i 写入的寄存器是否被后面的指令 (j > i) 写入
            for &reg in regs_written.iter() {
                if let Some(&j) = last_writer.get(&reg) {
                    // 输出依赖：i -> j (因为 i 和 j 都写入 reg，且 i < j)
                    let dep = Dependency {
                        from: i,
                        to: j,
                        dep_type: DependencyType::Output,
                        latency: 1,
                    };
                    dependencies.push(dep.clone());
                    
                    self.add_dependency(dep.from, dep.to, dep);
                }
                
                // 更新该寄存器的最新写入者为当前指令
                last_writer.insert(reg, i);
            }
        }
        
        // 检查反依赖：需要单独处理，因为反依赖是 j 读取 reg，i 写入 reg，且 j < i
        // 所以我们需要从前往后遍历，记录每个寄存器的最新读取者
        let mut last_reader: HashMap<RegId, usize> = HashMap::new();
        
        for i in 0..n {
            let op_i = &ops[i];
            Self::collect_read_regs(op_i, &mut regs_read);
            Self::collect_written_regs(op_i, &mut regs_written);
            
            // 检查反依赖：i 写入的寄存器是否被前面的指令 (j < i) 读取
            for &reg in regs_written.iter() {
                if let Some(&j) = last_reader.get(&reg) {
                    // 反依赖：j -> i (因为 j 读取 reg，i 写入 reg，且 j < i)
                    let dep = Dependency {
                        from: j,
                        to: i,
                        dep_type: DependencyType::Anti,
                        latency: 1,
                    };
                    
                    self.add_dependency(dep.from, dep.to, dep);
                }
            }
            
            // 更新该寄存器的最新读取者为当前指令
            for &reg in regs_read.iter() {
                last_reader.insert(reg, i);
            }
        }
        
        // 初始化就绪时间
        for i in 0..n {
            self.ready_times.insert(i, 0);
        }

        // 更新统计信息
        let elapsed = start_time.elapsed().as_nanos() as u64;
        // 修复平均时间计算：使用累加平均
        self.stats.avg_scheduling_time_ns =
            (self.stats.avg_scheduling_time_ns * (self.stats.total_schedules - 1) + elapsed) / self.stats.total_schedules;

        // 计算关键路径长度
        self.stats.critical_path_length = self.calculate_critical_path();
    }
    
    /// 调度指令（返回新的指令顺序）
    pub fn schedule(&mut self, ops: &[IROp]) -> Vec<usize> {
        let start_time = std::time::Instant::now();
        
        // 使用列表调度算法
        let mut ready_list = VecDeque::new();
        let mut scheduled = Vec::new();
        let mut scheduled_set = HashSet::new();
        
        // 找出没有前驱的指令
        for i in 0..ops.len() {
            if !self.reverse_graph.contains_key(&i) || self.reverse_graph[&i].is_empty() {
                ready_list.push_back(i);
            }
        }
        
        self.current_cycle = 0;
        
        while !ready_list.is_empty() {
            // 选择就绪时间最早的指令
            let mut earliest_idx = 0;
            let mut earliest_time = u32::MAX;
            
            for (idx, &instr_idx) in ready_list.iter().enumerate() {
                let ready_time = self.ready_times[&instr_idx];
                if ready_time <= self.current_cycle && ready_time < earliest_time {
                    earliest_time = ready_time;
                    earliest_idx = idx;
                }
            }
            
            // 调度该指令
            let instr_idx = ready_list.remove(earliest_idx).unwrap();
            scheduled.push(instr_idx);
            scheduled_set.insert(instr_idx);
            
            // 更新后继指令的就绪时间
            if let Some(deps) = self.dependency_graph.get(&instr_idx) {
                for dep in deps {
                    let to_idx = dep.to;
                    if !scheduled_set.contains(&to_idx) {
                        // 更新就绪时间
                        let current_ready = self.ready_times[&to_idx];
                        let new_ready = self.current_cycle + dep.latency;
                        self.ready_times.insert(to_idx, current_ready.max(new_ready));
                        
                        // 检查是否所有前驱都已调度
                        let mut all_ready = true;
                        if let Some(prev_deps) = self.reverse_graph.get(&to_idx) {
                            for &prev_idx in prev_deps {
                                if !scheduled_set.contains(&prev_idx) {
                                    all_ready = false;
                                    break;
                                }
                            }
                        }
                        
                        if all_ready && !ready_list.contains(&to_idx) {
                            ready_list.push_back(to_idx);
                        }
                    }
                }
            }
            
            // 推进当前周期
            self.current_cycle += 1;
        }
        
        // 更新统计信息
        let elapsed = start_time.elapsed().as_nanos() as u64;
        self.stats.total_schedules += 1;
        // 修复平均时间计算：使用累加平均
        self.stats.avg_scheduling_time_ns =
            (self.stats.avg_scheduling_time_ns * (self.stats.total_schedules - 1) + elapsed) / self.stats.total_schedules;
        
        // 计算重排的指令数
        let mut reordered = 0;
        for (i, &orig_idx) in scheduled.iter().enumerate() {
            if i != orig_idx {
                reordered += 1;
            }
        }
        self.stats.reordered_instructions += reordered;
        
        scheduled
    }
    
    /// 合并build_dependency_graph和schedule方法，减少中间步骤
    pub fn schedule_with_build(&mut self, ops: &[IROp]) -> Vec<usize> {
       // 首先构建依赖图
       self.build_dependency_graph(ops);
       // 然后调用现有的schedule方法
       self.schedule(ops)
    }
    
    /// 添加依赖关系
    fn add_dependency(&mut self, from: usize, to: usize, dep: Dependency) {
        self.dependency_graph
            .entry(from)
            .or_insert_with(Vec::new)
            .push(dep.clone());
        
        self.reverse_graph
            .entry(to)
            .or_insert_with(Vec::new)
            .push(from);
    }
    
    /// 检查数据依赖（RAW - Read After Write）
    fn check_data_dependency(&self, op_i: &IROp, op_j: &IROp, regs1: &mut Vec<RegId>, regs2: &mut Vec<RegId>) -> Option<Dependency> {
        // 获取op_i写入的寄存器和op_j读取的寄存器
        Self::collect_written_regs(op_i, regs1);
        Self::collect_read_regs(op_j, regs2);

        for reg_i in regs1 {
            for reg_j in &mut *regs2 {
                if reg_i == reg_j {
                    return Some(Dependency {
                        from: 0, // 将在调用处设置
                        to: 0,   // 将在调用处设置
                        dep_type: DependencyType::Data,
                        latency: self.get_latency(op_i, op_j),
                    });
                }
            }
        }

        None
    }
    
    /// 检查反依赖（WAR - Write After Read）
    fn check_anti_dependency(&self, op_i: &IROp, op_j: &IROp, regs1: &mut Vec<RegId>, regs2: &mut Vec<RegId>) -> Option<Dependency> {
        Self::collect_read_regs(op_i, regs1);
        Self::collect_written_regs(op_j, regs2);

        for reg_i in regs1 {
            for reg_j in &mut *regs2 {
                if reg_i == reg_j {
                    return Some(Dependency {
                        from: 0,
                        to: 0,
                        dep_type: DependencyType::Anti,
                        latency: 1, // 反依赖通常延迟较小
                    });
                }
            }
        }

        None
    }
    
    /// 检查输出依赖（WAW - Write After Write）
    fn check_output_dependency(&self, op_i: &IROp, op_j: &IROp, regs1: &mut Vec<RegId>, regs2: &mut Vec<RegId>) -> Option<Dependency> {
        Self::collect_written_regs(op_i, regs1);
        Self::collect_written_regs(op_j, regs2);

        for reg_i in regs1 {
            for reg_j in &mut *regs2 {
                if reg_i == reg_j {
                    return Some(Dependency {
                        from: 0,
                        to: 0,
                        dep_type: DependencyType::Output,
                        latency: 1, // 输出依赖通常延迟较小
                    });
                }
            }
        }

        None
    }
    
    /// 获取指令之间的延迟
    fn get_latency(&self, _op_i: &IROp, _op_j: &IROp) -> u32 {
        // 简化实现：固定延迟
        // 实际实现应该根据指令类型和目标架构确定延迟
        3
    }
    
    /// 计算关键路径长度
    fn calculate_critical_path(&self) -> u32 {
        let mut max_path = 0;
        
        // 对每个节点，计算到终点的最长路径
        for i in 0..self.dependency_graph.len() {
            let path_length = self.calculate_longest_path(i);
            max_path = max_path.max(path_length);
        }
        
        max_path
    }
    
    /// 计算从指定节点到终点的最长路径
    fn calculate_longest_path(&self, node: usize) -> u32 {
        if let Some(deps) = self.dependency_graph.get(&node) {
            let mut max_length = 0;
            
            for dep in deps {
                let sub_path = self.calculate_longest_path(dep.to);
                let total_length = dep.latency + sub_path;
                max_length = max_length.max(total_length);
            }
            
            max_length
        } else {
            0
        }
    }
    
    /// 收集操作中读取的寄存器
    fn collect_read_regs(op: &IROp, regs: &mut Vec<RegId>) {
        regs.clear();
        match op {
            IROp::Add { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Sub { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Mul { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Div { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Load { base, .. } => regs.push(*base),
            IROp::Store { src, base, .. } => {
                regs.push(*src);
                regs.push(*base);
            }
            IROp::CmpEq { lhs, rhs, .. } => {
                regs.push(*lhs);
                regs.push(*rhs);
            }
            IROp::CmpNe { lhs, rhs, .. } => {
                regs.push(*lhs);
                regs.push(*rhs);
            }
            IROp::CmpLt { lhs, rhs, .. } => {
                regs.push(*lhs);
                regs.push(*rhs);
            }
            IROp::CmpLtU { lhs, rhs, .. } => {
                regs.push(*lhs);
                regs.push(*rhs);
            }
            IROp::CmpGe { lhs, rhs, .. } => {
                regs.push(*lhs);
                regs.push(*rhs);
            }
            IROp::CmpGeU { lhs, rhs, .. } => {
                regs.push(*lhs);
                regs.push(*rhs);
            }
            IROp::Select { cond, true_val, false_val, .. } => {
                regs.push(*cond);
                regs.push(*true_val);
                regs.push(*false_val);
            }
            IROp::Beq { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Bne { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Blt { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Bge { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Bltu { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Bgeu { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::AtomicRMW { base, src, .. } => {
                regs.push(*base);
                regs.push(*src);
            }
            IROp::AtomicRMWOrder { base, src, .. } => {
                regs.push(*base);
                regs.push(*src);
            }
            IROp::AtomicCmpXchg { base, expected, new, .. } => {
                regs.push(*base);
                regs.push(*expected);
                regs.push(*new);
            }
            IROp::AtomicCmpXchgOrder { base, expected, new, .. } => {
                regs.push(*base);
                regs.push(*expected);
                regs.push(*new);
            }
            IROp::AtomicLoadReserve { base, .. } => {
                regs.push(*base);
            }
            IROp::AtomicStoreCond { src, base, dst_flag, .. } => {
                regs.push(*src);
                regs.push(*base);
                regs.push(*dst_flag);
            }
            IROp::AtomicCmpXchgFlag { base, expected, new, .. } => {
                regs.push(*base);
                regs.push(*expected);
                regs.push(*new);
            }
            IROp::AtomicRmwFlag { base, src, .. } => {
                regs.push(*base);
                regs.push(*src);
            }
            IROp::VecAdd { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::VecSub { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::VecMul { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::VecAddSat { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::VecSubSat { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::VecMulSat { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Fadd { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Fsub { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Fmul { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Fdiv { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Fsqrt { src, .. } => {
                regs.push(*src);
            }
            IROp::Fmin { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Fmax { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            IROp::Fload { base, .. } => {
                regs.push(*base);
            }
            IROp::Fstore { src, base, .. } => {
                regs.push(*src);
                regs.push(*base);
            }
            IROp::Atomic { base, src, .. } => {
                regs.push(*base);
                regs.push(*src);
            }
            IROp::Cpuid { leaf, subleaf, .. } => {
                regs.push(*leaf);
                regs.push(*subleaf);
            }
            IROp::CsrWrite { src, .. } => {
                regs.push(*src);
            }
            IROp::CsrSet { src, .. } => {
                regs.push(*src);
            }
            IROp::CsrClear { src, .. } => {
                regs.push(*src);
            }
            IROp::WritePstateFlags { src, .. } => {
                regs.push(*src);
            }
            IROp::EvalCondition { cond, .. } => {
                regs.push((*cond).into());
            }
            IROp::VendorLoad { base, .. } => {
                regs.push(*base);
            }
            IROp::VendorStore { src, base, .. } => {
                regs.push(*src);
                regs.push(*base);
            }
            IROp::VendorVectorOp { src1, src2, .. } => {
                regs.push(*src1);
                regs.push(*src2);
            }
            _ => {}
        }
    }
    
    /// 收集操作中写入的寄存器
    fn collect_written_regs(op: &IROp, regs: &mut Vec<RegId>) {
        regs.clear();
        match op {
            IROp::MovImm { dst, .. } => regs.push(*dst),
            // IROp::Mov variant doesn't exist, using existing variants
            IROp::Add { dst, .. } => regs.push(*dst),
            IROp::Sub { dst, .. } => regs.push(*dst),
            IROp::Mul { dst, .. } => regs.push(*dst),
            IROp::Div { dst, .. } => regs.push(*dst),
            IROp::Load { dst, .. } => regs.push(*dst),
            IROp::Store { .. } => {}, // Store 不写入寄存器
            IROp::Store { .. } => {}, // Store 不写入寄存器
            _ => {}
        }
    }
    
    /// 获取统计信息
    pub fn get_stats(&self) -> SchedulerStats {
        self.stats.clone()
    }
}

impl Default for InstructionScheduler {
    fn default() -> Self {
        Self::new()
    }
}