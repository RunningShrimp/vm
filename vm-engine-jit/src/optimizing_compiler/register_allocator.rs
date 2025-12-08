//! 寄存器分配器实现
//!
//! 实现线性扫描和图着色寄存器分配算法

use std::collections::{HashMap, HashSet};

use vm_ir::{IROp, RegId};

/// 寄存器分配器接口
pub trait RegisterAllocatorTrait {
    /// 分析寄存器生命周期
    fn analyze_lifetimes(&mut self, ops: &[IROp]);
    
    /// 分配寄存器
    fn allocate_registers(&mut self, ops: &[IROp]) -> HashMap<RegId, RegisterAllocation>;
    
    /// 获取统计信息
    fn get_stats(&self) -> RegisterAllocatorStats;
}

/// 寄存器分配结果
#[derive(Debug, Clone)]
pub enum RegisterAllocation {
    /// 分配到物理寄存器
    Register(RegId),
    /// 溢出到栈内存
    Stack(i32),
}

/// 寄存器分配器统计信息
#[derive(Debug, Clone, Default)]
pub struct RegisterAllocatorStats {
    /// 总分配次数
    pub total_allocations: u64,
    /// 溢出次数
    pub spills: u64,
    /// 使用的物理寄存器数量
    pub physical_regs_used: u32,
    /// 平均分配时间（纳秒）
    pub avg_allocation_time_ns: u64,
}

/// 线性扫描寄存器分配器
#[derive(Clone)]
pub struct LinearScanAllocator {
    /// 寄存器生命周期 (start, end)
    reg_lifetimes: HashMap<RegId, (usize, usize)>,
    /// 小块阈值（指令数）
    small_block_threshold: usize,
    /// 已溢出的寄存器
    spilled_regs: HashMap<RegId, i32>,
    /// 下一个溢出偏移
    next_spill_offset: i32,
    /// 统计信息
    stats: RegisterAllocatorStats,
}

impl LinearScanAllocator {
    /// 创建新的线性扫描分配器
    pub fn new() -> Self {
        Self {
            reg_lifetimes: HashMap::new(),
            small_block_threshold: 50, // 默认50条指令为小块
            spilled_regs: HashMap::new(),
            next_spill_offset: 0,
            stats: RegisterAllocatorStats::default(),
        }
    }
    
    /// 设置小块阈值
    pub fn set_small_block_threshold(&mut self, threshold: usize) {
        self.small_block_threshold = threshold;
    }
}

impl RegisterAllocatorTrait for LinearScanAllocator {
    fn analyze_lifetimes(&mut self, ops: &[IROp]) {
        self.reg_lifetimes.clear();

        let mut read_regs = Vec::new();
        let mut written_regs = Vec::new();

        for (idx, op) in ops.iter().enumerate() {
            // 收集读取的寄存器
            Self::collect_read_regs(op, &mut read_regs);
            // 收集写入的寄存器
            Self::collect_written_regs(op, &mut written_regs);

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
    
    fn allocate_registers(&mut self, ops: &[IROp]) -> HashMap<RegId, RegisterAllocation> {
        let start_time = std::time::Instant::now();
        let mut allocations = HashMap::new();

        // 简单的线性扫描实现
        let mut active_regs = HashMap::new();
        let mut written_regs = Vec::new();

        for (idx, op) in ops.iter().enumerate() {
            // 移除不再活跃的寄存器
            let regs_to_remove: Vec<RegId> = active_regs
                .iter()
                .filter(|&(_, &end_idx)| end_idx < idx)
                .map(|(&reg, _)| reg)
                .collect();
            
            for reg in regs_to_remove {
                active_regs.remove(&reg);
            }
            
            // 分配新寄存器
            Self::collect_written_regs(op, &mut written_regs);
            for &reg in &written_regs {
                if let Some((_, end_idx)) = self.reg_lifetimes.get(&reg) {
                    // 尝试分配物理寄存器
                    if let Some(phys_reg) = self.find_free_physical_reg(idx) {
                        allocations.insert(reg, RegisterAllocation::Register(phys_reg));
                        active_regs.insert(reg, *end_idx);
                    } else {
                        // 溢出到栈
                        let spill_offset = self.next_spill_offset;
                        self.next_spill_offset += 8; // 64位寄存器
                        self.spilled_regs.insert(reg, spill_offset);
                        allocations.insert(reg, RegisterAllocation::Stack(spill_offset));
                        active_regs.insert(reg, *end_idx);
                        
                        // 更新统计
                        self.stats.spills += 1;
                    }
                    
                    // 更新统计
                    self.stats.total_allocations += 1;
                }
            }
        }
        
        // 更新统计信息
        let elapsed = start_time.elapsed().as_nanos() as u64;
        self.stats.total_allocations += 1;
        // 修复平均时间计算：使用累加平均
        self.stats.avg_allocation_time_ns =
            (self.stats.avg_allocation_time_ns * (self.stats.total_allocations - 1) + elapsed) / self.stats.total_allocations;
        self.stats.physical_regs_used = allocations
            .values()
            .filter(|alloc| matches!(alloc, RegisterAllocation::Register(_)))
            .count() as u32;
        
        allocations
    }
    
    fn get_stats(&self) -> RegisterAllocatorStats {
        self.stats.clone()
    }
}

impl LinearScanAllocator {
    /// 查找空闲的物理寄存器
    fn find_free_physical_reg(&self, current_idx: usize) -> Option<RegId> {
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
            IROp::Add { dst, .. } => regs.push(*dst),
            IROp::Sub { dst, .. } => regs.push(*dst),
            IROp::Mul { dst, .. } => regs.push(*dst),
            IROp::Div { dst, .. } => regs.push(*dst),
            IROp::Rem { dst, .. } => regs.push(*dst),
            IROp::And { dst, .. } => regs.push(*dst),
            IROp::Or { dst, .. } => regs.push(*dst),
            IROp::Xor { dst, .. } => regs.push(*dst),
            IROp::Not { dst, .. } => regs.push(*dst),
            IROp::Sll { dst, .. } => regs.push(*dst),
            IROp::Srl { dst, .. } => regs.push(*dst),
            IROp::Sra { dst, .. } => regs.push(*dst),
            IROp::AddImm { dst, .. } => regs.push(*dst),
            IROp::MulImm { dst, .. } => regs.push(*dst),
            IROp::SllImm { dst, .. } => regs.push(*dst),
            IROp::SrlImm { dst, .. } => regs.push(*dst),
            IROp::SraImm { dst, .. } => regs.push(*dst),
            IROp::CmpEq { dst, .. } => regs.push(*dst),
            IROp::CmpNe { dst, .. } => regs.push(*dst),
            IROp::CmpLt { dst, .. } => regs.push(*dst),
            IROp::CmpLtU { dst, .. } => regs.push(*dst),
            IROp::CmpGe { dst, .. } => regs.push(*dst),
            IROp::CmpGeU { dst, .. } => regs.push(*dst),
            IROp::Select { dst, .. } => regs.push(*dst),
            IROp::Load { dst, .. } => regs.push(*dst),
            IROp::Store { .. } => {}, // Store 不写入寄存器
            IROp::AtomicRMW { dst, .. } => regs.push(*dst),
            IROp::AtomicRMWOrder { dst, .. } => regs.push(*dst),
            IROp::AtomicCmpXchg { dst, .. } => regs.push(*dst),
            IROp::AtomicCmpXchgOrder { dst, .. } => regs.push(*dst),
            IROp::AtomicLoadReserve { dst, .. } => regs.push(*dst),
            IROp::AtomicStoreCond { dst_flag, .. } => regs.push(*dst_flag),
            IROp::AtomicCmpXchgFlag { dst_old, dst_flag, .. } => {
                regs.push(*dst_old);
                regs.push(*dst_flag);
            }
            IROp::AtomicRmwFlag { dst_old, dst_flag, .. } => {
                regs.push(*dst_old);
                regs.push(*dst_flag);
            }
            IROp::VecAdd { dst, .. } => regs.push(*dst),
            IROp::VecSub { dst, .. } => regs.push(*dst),
            IROp::VecMul { dst, .. } => regs.push(*dst),
            IROp::VecAddSat { dst, .. } => regs.push(*dst),
            IROp::VecSubSat { dst, .. } => regs.push(*dst),
            IROp::VecMulSat { dst, .. } => regs.push(*dst),
            IROp::Fadd { dst, .. } => regs.push(*dst),
            IROp::Fsub { dst, .. } => regs.push(*dst),
            IROp::Fmul { dst, .. } => regs.push(*dst),
            IROp::Fdiv { dst, .. } => regs.push(*dst),
            IROp::Fsqrt { dst, .. } => regs.push(*dst),
            IROp::Fmin { dst, .. } => regs.push(*dst),
            IROp::Fmax { dst, .. } => regs.push(*dst),
            IROp::Fload { dst, .. } => regs.push(*dst),
            IROp::Atomic { dst, .. } => regs.push(*dst),
            IROp::Cpuid { dst_eax, dst_ebx, dst_ecx, dst_edx, .. } => {
                regs.push(*dst_eax);
                regs.push(*dst_ebx);
                regs.push(*dst_ecx);
                regs.push(*dst_edx);
            }
            IROp::CsrRead { dst, .. } => regs.push(*dst),
            IROp::CsrWriteImm { dst, .. } => regs.push(*dst),
            IROp::CsrSetImm { dst, .. } => regs.push(*dst),
            IROp::CsrClearImm { dst, .. } => regs.push(*dst),
            IROp::ReadPstateFlags { dst, .. } => regs.push(*dst),
            IROp::EvalCondition { dst, .. } => regs.push(*dst),
            IROp::VendorLoad { dst, .. } => regs.push(*dst),
            IROp::VendorMatrixOp { dst, .. } => regs.push(*dst),
            IROp::VendorVectorOp { dst, .. } => regs.push(*dst),
            _ => {}
        }
    }
}

/// 干扰图节点
struct InterferenceNode {
    /// 虚拟寄存器ID
    reg_id: RegId,
    /// 相邻节点（有干扰关系的寄存器）
    neighbors: HashSet<RegId>,
    /// 节点度数（相邻节点数量）
    degree: usize,
    /// 已分配的颜色（物理寄存器ID）或None
    color: Option<RegId>,
}

/// 干扰图
struct InterferenceGraph {
    /// 节点映射：寄存器ID -> 节点
    nodes: HashMap<RegId, InterferenceNode>,
}

/// 区间树节点
struct IntervalNode {
    /// 区间左边界
    low: usize,
    /// 区间右边界
    high: usize,
    /// 子树中的最大右边界（用于快速剪枝）
    max: usize,
    /// 左右子节点
    left: Option<Box<IntervalNode>>,
    right: Option<Box<IntervalNode>>,
    /// 该区间覆盖的寄存器ID列表
    intervals: Vec<RegId>,
}

impl IntervalNode {
    /// 创建新的区间树节点
    fn new(low: usize, high: usize, reg_id: RegId) -> Self {
        Self {
            low,
            high,
            max: high,
            left: None,
            right: None,
            intervals: vec![reg_id],
        }
    }

    /// 更新子树的最大右边界
    fn update_max(&mut self) {
        let mut max_val = self.high;

        // 更新左子树最大值
        if let Some(left) = &self.left {
            if left.max > max_val {
                max_val = left.max;
            }
        }

        // 更新右子树最大值
        if let Some(right) = &self.right {
            if right.max > max_val {
                max_val = right.max;
            }
        }

        self.max = max_val;
    }
}

/// 区间树
struct IntervalTree {
    root: Option<Box<IntervalNode>>,
}

impl IntervalTree {
    /// 创建新的区间树
    fn new() -> Self {
        Self { root: None }
    }

    /// 插入一个区间
    fn insert(&mut self, low: usize, high: usize, reg_id: RegId) {
        self.root = Self::insert_recursive(self.root.take(), low, high, reg_id);
    }

    /// 递归插入区间
    fn insert_recursive(
        node: Option<Box<IntervalNode>>,
        low: usize,
        high: usize,
        reg_id: RegId
    ) -> Option<Box<IntervalNode>> {
        let mut node = match node {
            Some(n) => n,
            None => return Some(Box::new(IntervalNode::new(low, high, reg_id))),
        };

        // 如果区间完全相同，直接添加到该节点的intervals
        if node.low == low && node.high == high {
            node.intervals.push(reg_id);
            return Some(node);
        }

        // 根据low值决定插入左子树还是右子树
        if low < node.low {
            node.left = Self::insert_recursive(node.left.take(), low, high, reg_id);
        } else {
            node.right = Self::insert_recursive(node.right.take(), low, high, reg_id);
        }

        // 更新当前节点的max值
        node.update_max();

        Some(node)
    }

    /// 查找与给定区间重叠的所有寄存器
    fn search_overlap(&self, low: usize, high: usize) -> Vec<RegId> {
        let mut results = Vec::new();
        Self::search_overlap_recursive(&self.root, low, high, &mut results);
        results
    }

    /// 递归查找重叠区间
    fn search_overlap_recursive(
        node: &Option<Box<IntervalNode>>,
        low: usize,
        high: usize,
        results: &mut Vec<RegId>
    ) {
        let Some(node) = node else {
            return;
        };

        // 检查当前节点是否与目标区间重叠
        if !(node.high < low || high < node.low) {
            // 重叠，添加所有寄存器到结果
            results.extend_from_slice(&node.intervals);
        }

        // 如果左子节点存在且左子节点的max >= low，左子树可能有重叠区间
        if let Some(left) = &node.left {
            if left.max >= low {
                Self::search_overlap_recursive(&node.left, low, high, results);
            }
        }

        // 检查右子树
        Self::search_overlap_recursive(&node.right, low, high, results);
    }
}

impl InterferenceGraph {
    /// 创建新的干扰图
    fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// 添加节点
    fn add_node(&mut self, reg_id: RegId) {
        if !self.nodes.contains_key(&reg_id) {
            self.nodes.insert(reg_id, InterferenceNode {
                reg_id,
                neighbors: HashSet::new(),
                degree: 0,
                color: None,
            });
        }
    }

    /// 添加边（两个寄存器之间的干扰关系）
    fn add_edge(&mut self, reg1: RegId, reg2: RegId) {
        if reg1 == reg2 {
            return; // 自环不处理
        }

        // 确保两个节点都存在
        self.add_node(reg1);
        self.add_node(reg2);

        // 在两个方向添加边
        let node1 = self.nodes.get_mut(&reg1).unwrap();
        if node1.neighbors.insert(reg2) {
            node1.degree += 1;
        }

        let node2 = self.nodes.get_mut(&reg2).unwrap();
        if node2.neighbors.insert(reg1) {
            node2.degree += 1;
        }
    }

    /// 移除节点及其所有边
    fn remove_node(&mut self, reg_id: RegId) -> Option<InterferenceNode> {
        if let Some(node) = self.nodes.remove(&reg_id) {
            // 从所有邻居中移除对此节点的引用
            for neighbor_id in &node.neighbors {
                if let Some(neighbor_node) = self.nodes.get_mut(neighbor_id) {
                    neighbor_node.neighbors.remove(&reg_id);
                    neighbor_node.degree -= 1;
                }
            }
            Some(node)
        } else {
            None
        }
    }

    /// 获取度数最小的节点
    fn get_lowest_degree_node(&self) -> Option<RegId> {
        self.nodes
            .iter()
            .filter(|(_, node)| node.color.is_none()) // 只考虑未着色的节点
            .min_by_key(|(_, node)| node.degree)
            .map(|(reg_id, _)| *reg_id)
    }

    /// 检查是否有节点度数大于等于给定值
    fn has_high_degree_node(&self, k: usize) -> bool {
        self.nodes
            .values()
            .any(|node| node.color.is_none() && node.degree >= k)
    }
}

/// 图着色寄存器分配器
#[derive(Clone)]
pub struct GraphColoringAllocator {
    /// 寄存器生命周期
    reg_lifetimes: HashMap<RegId, (usize, usize)>,
    /// 已溢出的寄存器
    spilled_regs: HashMap<RegId, i32>,
    /// 下一个溢出偏移
    next_spill_offset: i32,
    /// 统计信息
    stats: RegisterAllocatorStats,
    /// 物理寄存器数量限制
    k: usize,  // K是可用物理寄存器数量
}

impl GraphColoringAllocator {
    /// 创建新的图着色分配器
    pub fn new() -> Self {
        // 对于ARM64架构，我们可以使用x1-x30寄存器（x0保留为零，x31为SP），所以K=30
        let k = 30; 

        Self {
            reg_lifetimes: HashMap::new(),
            spilled_regs: HashMap::new(),
            next_spill_offset: 0,
            stats: RegisterAllocatorStats::default(),
            k,
        }
    }

    /// 构建干扰图 - 使用区间树优化的版本，复杂度为 O(r log r)
    fn build_interference_graph(&self) -> InterferenceGraph {
        let mut graph = InterferenceGraph::new();

        // 创建区间树
        let mut interval_tree = IntervalTree::new();

        // 第一步：创建所有寄存器节点并将寄存器生命周期插入区间树
        let mut reg_list: Vec<(RegId, (usize, usize))> = Vec::new();
        
        for (&reg_id, &lifetime) in &self.reg_lifetimes {
            graph.add_node(reg_id);
            reg_list.push((reg_id, lifetime));
            
            // 将生命周期插入区间树
            interval_tree.insert(lifetime.0, lifetime.1, reg_id);
        }

        // 第二步：为每个寄存器查找所有重叠的寄存器并添加干扰边
        for (reg_id, (low, high)) in &reg_list {
            // 查找与当前寄存器生命周期重叠的所有寄存器
            let overlapping_regs = interval_tree.search_overlap(*low, *high);
            
            // 为每个重叠的寄存器添加干扰边
            for overlapping_reg in overlapping_regs {
                if *reg_id != overlapping_reg {
                    graph.add_edge(*reg_id, overlapping_reg);
                }
            }
        }

        graph
    }

    /// 简化阶段：移除度数小于K的节点并压入栈，如果遇到无法简化的情况则进行溢出
    fn simplify(&mut self, graph: &mut InterferenceGraph) -> Vec<RegId> {
        let mut stack = Vec::new();
        let mut processed = HashSet::new();

        loop {
            // 首先尝试找到度数小于K的节点
            let low_degree_node = graph.get_lowest_degree_node()
                .filter(|reg_id| {
                    let node = &graph.nodes[reg_id];
                    node.degree < self.k && !processed.contains(reg_id)
                });

            if let Some(reg_id) = low_degree_node {
                // 从图中移除节点并加入栈
                let _removed_node = graph.remove_node(reg_id).unwrap();
                stack.push(reg_id);
                processed.insert(reg_id);
                continue; // 继续处理下一个低度数节点
            }

            // 如果没有度数小于K的节点，检查是否所有节点都已处理
            let remaining_nodes: Vec<RegId> = graph.nodes
                .keys()
                .filter(|reg_id| !processed.contains(reg_id))
                .cloned()
                .collect();

            if remaining_nodes.is_empty() {
                break; // 所有节点都已处理
            }

            // 所有剩余节点度数都 >= K，需要选择一个节点进行溢出
            // 使用启发式：选择度数最高的节点（或根据使用频率等其他启发式）
            let spill_candidate = *remaining_nodes.iter()
                .max_by_key(|reg_id| graph.nodes[reg_id].degree)
                .unwrap();

            // 将溢出候选节点标记为溢出
            let spill_offset = self.next_spill_offset;
            self.next_spill_offset += 8; // 64位寄存器
            self.spilled_regs.insert(spill_candidate, spill_offset);
            self.stats.spills += 1;

            // 从图中移除溢出节点
            graph.remove_node(spill_candidate);
            processed.insert(spill_candidate);
        }

        stack
    }

    /// 选择阶段：从栈中弹出节点并为其分配颜色
    fn select(&mut self, mut graph: InterferenceGraph, stack: Vec<RegId>) -> HashMap<RegId, RegisterAllocation> {
        let mut allocations = HashMap::new();

        // 从栈顶开始处理
        for reg_id in stack.into_iter().rev() {
            // 获取所有邻居已使用的颜色
            let neighbor_colors: HashSet<RegId> = graph.nodes
                .get(&reg_id)
                .map(|node| {
                    node.neighbors
                        .iter()
                        .filter_map(|neighbor_id| {
                            graph.nodes.get(neighbor_id).and_then(|n| n.color)
                        })
                        .collect()
                })
                .unwrap_or_default();

            // 为当前节点分配一个未被邻居使用的颜色
            let mut assigned_color = None;
            // 尝试分配物理寄存器 x1 到 x30
            for phys_reg in 1..=self.k as RegId {
                if !neighbor_colors.contains(&phys_reg) {
                    assigned_color = Some(phys_reg);
                    break;
                }
            }

            if let Some(color) = assigned_color {
                // 成功分配颜色，更新图中的节点
                if let Some(node) = graph.nodes.get_mut(&reg_id) {
                    node.color = Some(color);
                }
                allocations.insert(reg_id, RegisterAllocation::Register(color));
            } else {
                // 无法分配颜色，需要溢出到栈
                let spill_offset = self.next_spill_offset;
                self.next_spill_offset += 8; // 64位寄存器
                self.spilled_regs.insert(reg_id, spill_offset);
                allocations.insert(reg_id, RegisterAllocation::Stack(spill_offset));
                self.stats.spills += 1;
            }

            // 将节点重新插入图中以便后续节点可以引用它
            // 注意：在实际Chaitin-Briggs算法中，这里我们已经完成了对它的处理
            // 所以不需要真正重新插入，只需要确保后续处理正确
        }

        // 处理剩余未分配的节点（这些可能是由于溢出导致的）
        for (reg_id, node) in &graph.nodes {
            if node.color.is_none() {
                // 如果还有未着色的节点，它们必须溢出到栈
                let spill_offset = self.next_spill_offset;
                self.next_spill_offset += 8; // 64位寄存器
                self.spilled_regs.insert(*reg_id, spill_offset);
                allocations.insert(*reg_id, RegisterAllocation::Stack(spill_offset));
                self.stats.spills += 1;
            } else {
                allocations.insert(*reg_id, RegisterAllocation::Register(node.color.unwrap()));
            }
        }

        allocations
    }
}

impl RegisterAllocatorTrait for GraphColoringAllocator {
    fn analyze_lifetimes(&mut self, ops: &[IROp]) {
        self.reg_lifetimes.clear();

        let mut read_regs = Vec::new();
        let mut written_regs = Vec::new();

        for (idx, op) in ops.iter().enumerate() {
            // 收集读取的寄存器
            LinearScanAllocator::collect_read_regs(op, &mut read_regs);
            // 收集写入的寄存器
            LinearScanAllocator::collect_written_regs(op, &mut written_regs);

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

    fn allocate_registers(&mut self, ops: &[IROp]) -> HashMap<RegId, RegisterAllocation> {
        let start_time = std::time::Instant::now();

        // 分析生命周期
        self.analyze_lifetimes(ops);

        // 构建干扰图
        let mut graph = self.build_interference_graph();

        // 简化阶段
        let stack = self.simplify(&mut graph);

        // 选择阶段
        let allocations = self.select(graph, stack);

        // 更新统计
        let elapsed = start_time.elapsed().as_nanos() as u64;
        self.stats.total_allocations = allocations.len() as u64;
        // 修复平均时间计算：使用累加平均
        self.stats.avg_allocation_time_ns =
            (self.stats.avg_allocation_time_ns * (self.stats.total_allocations - 1) + elapsed) / self.stats.total_allocations;
        self.stats.physical_regs_used = allocations
            .values()
            .filter(|alloc| matches!(alloc, RegisterAllocation::Register(_)))
            .count() as u32;
        self.stats.total_allocations = allocations.len() as u64;

        allocations
    }

    fn get_stats(&self) -> RegisterAllocatorStats {
        self.stats.clone()
    }
}
/// 存根图着色寄存器分配器（用于测试）
#[derive(Clone)]
pub struct StubGraphColoringAllocator {
    /// 内部使用的图着色分配器
    inner: GraphColoringAllocator,
}

impl StubGraphColoringAllocator {
    /// 创建新的存根图着色分配器
    pub fn new() -> Self {
        Self {
            inner: GraphColoringAllocator::new(),
        }
    }
}

impl RegisterAllocatorTrait for StubGraphColoringAllocator {
    fn analyze_lifetimes(&mut self, ops: &[IROp]) {
        self.inner.analyze_lifetimes(ops);
    }
    
    fn allocate_registers(&mut self, ops: &[IROp]) -> HashMap<RegId, RegisterAllocation> {
        self.inner.allocate_registers(ops)
    }
    
    fn get_stats(&self) -> RegisterAllocatorStats {
        self.inner.get_stats()
    }
}


/// 寄存器分配器（自适应）
pub struct RegisterAllocator {
    /// 线性扫描分配器
    linear_scan: LinearScanAllocator,
    /// 图着色分配器
    graph_coloring: GraphColoringAllocator,
    /// 当前使用的分配器
    current_allocator: Box<dyn RegisterAllocatorTrait>,
    /// 小块阈值
    small_block_threshold: usize,
}

impl RegisterAllocator {
    /// 创建新的寄存器分配器
    pub fn new() -> Self {
        let linear_scan = LinearScanAllocator::new();
        Self {
            linear_scan,
            graph_coloring: GraphColoringAllocator::new(),
            current_allocator: Box::new(LinearScanAllocator::new()),
            small_block_threshold: 50,
        }
    }
    
    /// 设置小块阈值
    pub fn set_small_block_threshold(&mut self, threshold: usize) {
        self.small_block_threshold = threshold;
        self.linear_scan.set_small_block_threshold(threshold);
    }
    
    /// 分析寄存器生命周期
    pub fn analyze_lifetimes(&mut self, ops: &[IROp]) {
        // 根据块大小选择分配器
        if ops.len() <= self.small_block_threshold {
            self.current_allocator = Box::new(self.linear_scan.clone());
        } else {
            self.current_allocator = Box::new(self.graph_coloring.clone());
        }

        self.current_allocator.as_mut().analyze_lifetimes(ops);
    }
    
    /// 分配寄存器
    pub fn allocate_registers(&mut self, ops: &[IROp]) -> HashMap<RegId, RegisterAllocation> {
        self.current_allocator.as_mut().allocate_registers(ops)
    }
    
    /// 获取统计信息
    pub fn get_stats(&self) -> RegisterAllocatorStats {
        self.current_allocator.get_stats()
    }
}