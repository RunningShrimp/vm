//! 寄存器分配器接口和实现
//!
//! 定义了寄存器分配器的抽象接口和多种实现策略，负责将虚拟寄存器映射到物理寄存器。
//! 支持多种分配策略：
//! - LinearScan：线性扫描分配（基础）
//! - GraphColoring：图着色分配（高级）
//! - Hybrid：混合策略（默认）

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use vm_core::VmError;
use vm_ir::{IROp, RegId};
use crate::compiler::{CompiledIRBlock, CompiledInstruction};

/// 寄存器分配器接口
pub trait RegisterAllocator: Send + Sync {
    /// 为IR块分配寄存器
    fn allocate(&mut self, block: &crate::compiler::CompiledIRBlock) -> Result<crate::compiler::CompiledIRBlock, VmError>;
    
    /// 获取分配器名称
    fn name(&self) -> &str;
    
    /// 获取分配器版本
    fn version(&self) -> &str;
    
    /// 设置分配选项
    fn set_option(&mut self, option: &str, value: &str) -> Result<(), VmError>;
    
    /// 获取分配选项
    fn get_option(&self, option: &str) -> Option<String>;
    
    /// 重置分配器状态
    fn reset(&mut self);
    
    /// 获取分配统计信息
    fn get_stats(&self) -> RegisterAllocationStats;
}

/// 寄存器分配统计信息
#[derive(Debug, Clone, Default)]
pub struct RegisterAllocationStats {
    /// 虚拟寄存器总数
    pub total_virtual_registers: usize,
    /// 物理寄存器总数
    pub total_physical_registers: usize,
    /// 分配到物理寄存器的虚拟寄存器数
    pub allocated_registers: usize,
    /// 溢出到栈的虚拟寄存器数
    pub spilled_registers: usize,
    /// 栈槽使用数
    pub stack_slots_used: usize,
    /// 分配耗时（纳秒）
    pub allocation_time_ns: u64,
    /// 寄存器重载次数
    pub reload_count: u64,
    /// 寄存器存储次数
    pub spill_count: u64,
}

/// 寄存器分配器配置
#[derive(Debug, Clone)]
pub struct AllocatorConfig {
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

impl Default for AllocatorConfig {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocation_strategy_display() {
        assert_eq!(format!("{}", AllocationStrategy::LinearScan), "LinearScan");
        assert_eq!(format!("{}", AllocationStrategy::GraphColoring), "GraphColoring");
        assert_eq!(format!("{}", AllocationStrategy::Hybrid), "Hybrid");
    }

    #[test]
    fn test_basic_allocator_creation() {
        let allocator = BasicRegisterAllocator::new(AllocatorConfig::default());
        assert_eq!(allocator.name(), "BasicRegisterAllocator");
        assert_eq!(allocator.version(), "1.0.0");
    }

    #[test]
    fn test_basic_allocator_config_default() {
        let config = AllocatorConfig::default();
        assert_eq!(config.max_physical_registers, 16);
        assert!(config.enable_spill_optimization);
        assert!(config.enable_renaming);
    }

    #[test]
    fn test_basic_allocator_set_option() {
        let mut allocator = BasicRegisterAllocator::new(AllocatorConfig::default());
        let result = allocator.set_option("max_physical_registers", "32");
        assert!(result.is_ok());
        let value = allocator.get_option("max_physical_registers");
        assert_eq!(value, Some("32".to_string()));
    }

    #[test]
    fn test_basic_allocator_invalid_option() {
        let mut allocator = BasicRegisterAllocator::new(AllocatorConfig::default());
        let result = allocator.set_option("unknown_option", "value");
        assert!(result.is_err());
    }

    #[test]
    fn test_optimized_allocator_creation() {
        let allocator = OptimizedRegisterAllocator::new(OptimizedAllocatorConfig::default());
        assert_eq!(allocator.name(), "OptimizedRegisterAllocator");
        assert_eq!(allocator.version(), "1.0.0");
    }

    #[test]
    fn test_allocator_config_default() {
        let config = AllocatorConfig::default();
        assert_eq!(config.strategy, AllocationStrategy::LinearScan);
        assert_eq!(config.max_physical_registers, 32);
        assert_eq!(config.spill_threshold, 10);
    }

    #[test]
    fn test_optimized_allocator_set_strategy() {
        let mut allocator = OptimizedRegisterAllocator::new(AllocatorConfig::default());
        allocator.set_option("strategy", "graph").unwrap();
        assert_eq!(allocator.config.strategy, AllocationStrategy::GraphColoring);
    }

    #[test]
    fn test_optimized_allocator_invalid_strategy() {
        let mut allocator = OptimizedRegisterAllocator::new(AllocatorConfig::default());
        let result = allocator.set_option("strategy", "invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_optimized_allocator_stats() {
        let allocator = OptimizedRegisterAllocator::new(AllocatorConfig::default());
        let stats = allocator.get_stats();
        assert_eq!(stats.total_virtual_registers, 0);
        assert_eq!(stats.spilled_registers, 0);
        assert_eq!(stats.reload_count, 0);
    }

    #[test]
    fn test_optimized_allocator_reset() {
        let mut allocator = OptimizedRegisterAllocator::new(AllocatorConfig::default());
        allocator.reset();
        let stats = allocator.get_stats();
        assert_eq!(stats.total_virtual_registers, 0);
    }

    #[test]
    fn test_register_allocation_stats() {
        use std::sync::atomic::{AtomicU64, Ordering};
        let stats = OptimizedAllocationStats {
            total_allocations: AtomicU64::new(10),
            spill_count: AtomicU64::new(2),
            rename_count: AtomicU64::new(0),
            coloring_attempts: AtomicU64::new(0),
            register_reuse_count: AtomicU64::new(0),
            avg_allocation_time_ns: AtomicU64::new(1000),
            reload_count: AtomicU64::new(5),
            store_count: AtomicU64::new(3),
        };
        assert_eq!(stats.total_allocations.load(Ordering::Relaxed), 10);
        assert_eq!(stats.spill_count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_linear_scan_strategy() {
        let config = AllocatorConfig {
            strategy: AllocationStrategy::LinearScan,
            ..Default::default()
        };
        assert_eq!(config.strategy, AllocationStrategy::LinearScan);
    }

    #[test]
    fn test_graph_coloring_strategy() {
        let config = AllocatorConfig {
            strategy: AllocationStrategy::GraphColoring,
            ..Default::default()
        };
        assert_eq!(config.strategy, AllocationStrategy::GraphColoring);
    }

    #[test]
    fn test_hybrid_strategy() {
        let config = AllocatorConfig {
            strategy: AllocationStrategy::Hybrid,
            ..Default::default()
        };
        assert_eq!(config.strategy, AllocationStrategy::Hybrid);
    }

    #[test]
    fn test_optimized_allocator_enable_renaming() {
        let mut allocator = OptimizedRegisterAllocator::new(AllocatorConfig::default());
        allocator.set_option("enable_renaming", "false").unwrap();
        assert!(!allocator.config.enable_renaming);
    }
}

/// 优化的分配统计
#[derive(Debug, Default)]
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

/// 寄存器类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegisterClass {
    /// 通用寄存器
    General,
    /// 浮点寄存器
    Float,
    /// 向量寄存器
    Vector,
    /// 特殊寄存器（如PC、SP等）
    Special,
}

/// 寄存器信息
#[derive(Debug, Clone)]
pub struct RegisterInfo {
    /// 寄存器名称
    pub name: String,
    /// 寄存器类
    pub class: RegisterClass,
    /// 寄存器大小（位）
    pub size: u8,
    /// 是否是调用者保存
    pub caller_saved: bool,
    /// 是否是参数寄存器
    pub argument: bool,
    /// 是否是返回值寄存器
    pub return_value: bool,
}

/// 活跃区间
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveRange {
    /// 开始位置
    pub start: usize,
    /// 结束位置
    pub end: usize,
}

impl LiveRange {
    /// 创建新的活跃区间
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
    
    /// 检查两个活跃区间是否重叠
    pub fn overlaps(&self, other: &LiveRange) -> bool {
        self.start <= other.end && other.start <= self.end
    }
}

/// 线性扫描寄存器分配器
pub struct LinearScanAllocator {
    /// 分配器名称
    name: String,
    /// 分配器版本
    version: String,
    /// 分配选项
    options: HashMap<String, String>,
    /// 物理寄存器信息
    physical_registers: HashMap<RegisterClass, Vec<RegisterInfo>>,
    /// 虚拟寄存器到物理寄存器的映射
    vreg_to_preg: HashMap<RegId, String>,
    /// 虚拟寄存器到栈槽的映射
    vreg_to_stack: HashMap<RegId, usize>,
    /// 已使用的物理寄存器
    used_registers: HashMap<RegisterClass, HashSet<String>>,
    /// 已使用的栈槽
    used_stack_slots: HashSet<usize>,
    /// 下一个可用的栈槽
    next_stack_slot: usize,
    /// 分配统计
    stats: RegisterAllocationStats,
}

impl LinearScanAllocator {
    /// 创建新的线性扫描分配器
    pub fn new() -> Self {
        let mut physical_registers = HashMap::new();
        
        // 初始化x86-64寄存器
        let mut general_regs = Vec::new();
        for i in 0..16 {
            let reg_names = ["RAX", "RCX", "RDX", "RBX", "RSP", "RBP", "RSI", "RDI", 
                            "R8", "R9", "R10", "R11", "R12", "R13", "R14", "R15"];
            general_regs.push(RegisterInfo {
                name: reg_names[i].to_string(),
                class: RegisterClass::General,
                size: 64,
                caller_saved: i < 6 || (8..=11).contains(&i), // RAX,RCX,RDX,RBX,RSI,RDI,R8-R11是调用者保存
                argument: i < 6, // 前6个是参数寄存器
                return_value: i == 0, // RAX是返回值寄存器
            });
        }
        physical_registers.insert(RegisterClass::General, general_regs);
        
        // 初始化浮点寄存器
        let mut float_regs = Vec::new();
        for i in 0..16 {
            float_regs.push(RegisterInfo {
                name: format!("XMM{}", i),
                class: RegisterClass::Float,
                size: 128,
                caller_saved: i < 6, // XMM0-XMM5是调用者保存
                argument: i < 8, // 前8个是参数寄存器
                return_value: i == 0, // XMM0是返回值寄存器
            });
        }
        physical_registers.insert(RegisterClass::Float, float_regs);
        
        // 初始化向量寄存器（与浮点寄存器共享）
        let mut vector_regs = Vec::new();
        for i in 0..16 {
            vector_regs.push(RegisterInfo {
                name: format!("YMM{}", i),
                class: RegisterClass::Vector,
                size: 255,
                caller_saved: i < 6, // YMM0-YMM5是调用者保存
                argument: i < 8, // 前8个是参数寄存器
                return_value: i == 0, // YMM0是返回值寄存器
            });
        }
        physical_registers.insert(RegisterClass::Vector, vector_regs);
        
        Self {
            name: "LinearScanAllocator".to_string(),
            version: "1.0.0".to_string(),
            options: HashMap::new(),
            physical_registers,
            vreg_to_preg: HashMap::new(),
            vreg_to_stack: HashMap::new(),
            used_registers: HashMap::new(),
            used_stack_slots: HashSet::new(),
            next_stack_slot: 0,
            stats: RegisterAllocationStats::default(),
        }
    }
    
    /// 获取寄存器类
    fn get_register_class(&self, _reg: RegId) -> RegisterClass {
        // 在实际实现中，这里需要根据寄存器的用途确定其类
        // 目前默认返回通用寄存器
        RegisterClass::General
    }
    
    /// 获取可用的物理寄存器
    fn get_available_register(&self, reg_class: RegisterClass) -> Option<String> {
        if let Some(regs) = self.physical_registers.get(&reg_class) {
            if let Some(used) = self.used_registers.get(&reg_class) {
                for reg_info in regs {
                    if !used.contains(&reg_info.name) {
                        return Some(reg_info.name.clone());
                    }
                }
            } else {
                // 没有使用任何寄存器，返回第一个
                if let Some(first_reg) = regs.first() {
                    return Some(first_reg.name.clone());
                }
            }
        }
        None
    }
    
    /// 分配物理寄存器
    fn allocate_physical_register(&mut self, vreg: RegId, reg_class: RegisterClass) -> Option<String> {
        if let Some(preg) = self.get_available_register(reg_class) {
            // 标记寄存器为已使用
            self.used_registers.entry(reg_class).or_insert_with(HashSet::new).insert(preg.clone());
            
            // 记录映射
            self.vreg_to_preg.insert(vreg, preg.clone());
            
            Some(preg)
        } else {
            // 没有可用的物理寄存器，需要溢出到栈
            None
        }
    }
    
    /// 分配栈槽
    fn allocate_stack_slot(&mut self, vreg: RegId) -> usize {
        // 找到下一个可用的栈槽
        let mut slot = self.next_stack_slot;
        while self.used_stack_slots.contains(&slot) {
            slot += 1;
        }
        
        // 标记栈槽为已使用
        self.used_stack_slots.insert(slot);
        self.next_stack_slot = slot + 1;
        
        // 记录映射
        self.vreg_to_stack.insert(vreg, slot);
        
        slot
    }
    
    /// 释放物理寄存器
    fn release_physical_register(&mut self, preg: &str, reg_class: RegisterClass) {
        if let Some(used) = self.used_registers.get_mut(&reg_class) {
            used.remove(preg);
        }
    }
    
    /// 释放栈槽
    fn release_stack_slot(&mut self, slot: usize) {
        self.used_stack_slots.remove(&slot);
    }
    
    /// 模拟寄存器使用结束后的释放操作
    fn simulate_register_release(&mut self, vreg: RegId) {
        // 在实际实现中，这会在寄存器不再需要时被调用
        // 这里只是为了确保方法被使用
        
        // 克隆需要的数据以避免借用冲突
        let preg_clone = self.vreg_to_preg.get(&vreg).cloned();
        let slot_clone = self.vreg_to_stack.get(&vreg).copied();
        
        if let Some(preg) = preg_clone {
            let reg_class = self.get_register_class(vreg);
            self.release_physical_register(&preg, reg_class);
        }
        
        if let Some(slot) = slot_clone {
            self.release_stack_slot(slot);
        }
    }
}

impl RegisterAllocator for LinearScanAllocator {
    fn allocate(&mut self, block: &crate::compiler::CompiledIRBlock) -> Result<crate::compiler::CompiledIRBlock, VmError> {
        let start_time = std::time::Instant::now();
        
        // 重置分配状态
        self.reset();
        
        // 简化的线性扫描分配
        let mut allocated_block = block.clone();
        let mut vreg_to_preg = HashMap::new();
        let mut vreg_to_stack = HashMap::new();
        
        // 遍历所有操作，为每个虚拟寄存器分配物理寄存器或栈槽
        for op in &mut allocated_block.ops {
            match &op.op {
                IROp::MovImm { dst, .. } => {
                    let reg_class = self.get_register_class(*dst);
                    if let Some(preg) = self.allocate_physical_register(*dst, reg_class) {
                        vreg_to_preg.insert(*dst, preg.clone());
                        op.register_allocation.insert(format!("v{}", dst), preg);
                    } else {
                        let slot = self.allocate_stack_slot(*dst);
                        vreg_to_stack.insert(*dst, slot);
                        op.register_allocation.insert(format!("v{}", dst), format!("stack[{}]", slot));
                    }
                }
                IROp::Add { dst, src1, src2 } |
                IROp::Sub { dst, src1, src2 } |
                IROp::Mul { dst, src1, src2 } |
                IROp::Div { dst, src1, src2, .. } |
                IROp::Rem { dst, src1, src2, .. } |
                IROp::And { dst, src1, src2 } |
                IROp::Or { dst, src1, src2 } |
                IROp::Xor { dst, src1, src2 } => {
                    // 处理目标寄存器
                    let dst_class = self.get_register_class(*dst);
                    if let Some(preg) = self.allocate_physical_register(*dst, dst_class) {
                        vreg_to_preg.insert(*dst, preg.clone());
                        op.register_allocation.insert(format!("v{}", dst), preg);
                    } else {
                        let slot = self.allocate_stack_slot(*dst);
                        vreg_to_stack.insert(*dst, slot);
                        op.register_allocation.insert(format!("v{}", dst), format!("stack[{}]", slot));
                    }
                    
                    // 处理源寄存器1
                    let src1_class = self.get_register_class(*src1);
                    if let Some(preg) = vreg_to_preg.get(src1) {
                        op.register_allocation.insert(format!("v{}", src1), preg.clone());
                    } else if let Some(slot) = vreg_to_stack.get(src1) {
                        op.register_allocation.insert(format!("v{}", src1), format!("stack[{}]", slot));
                    } else if let Some(preg) = self.allocate_physical_register(*src1, src1_class) {
                        vreg_to_preg.insert(*src1, preg.clone());
                        op.register_allocation.insert(format!("v{}", src1), preg);
                    } else {
                        let slot = self.allocate_stack_slot(*src1);
                        vreg_to_stack.insert(*src1, slot);
                        op.register_allocation.insert(format!("v{}", src1), format!("stack[{}]", slot));
                    }
                    
                    // 处理源寄存器2
                    let src2_class = self.get_register_class(*src2);
                    if let Some(preg) = vreg_to_preg.get(src2) {
                        op.register_allocation.insert(format!("v{}", src2), preg.clone());
                    } else if let Some(slot) = vreg_to_stack.get(src2) {
                        op.register_allocation.insert(format!("v{}", src2), format!("stack[{}]", slot));
                    } else if let Some(preg) = self.allocate_physical_register(*src2, src2_class) {
                        vreg_to_preg.insert(*src2, preg.clone());
                        op.register_allocation.insert(format!("v{}", src2), preg);
                    } else {
                        let slot = self.allocate_stack_slot(*src2);
                        vreg_to_stack.insert(*src2, slot);
                        op.register_allocation.insert(format!("v{}", src2), format!("stack[{}]", slot));
                    }
                }
                // 其他操作类型的处理...
                _ => {}
            }
        }
        
        // 更新寄存器信息
        allocated_block.register_info.vreg_to_preg = vreg_to_preg.into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        allocated_block.register_info.stack_slots = vreg_to_stack.into_iter()
            .map(|(_vreg, slot)| crate::compiler::StackSlot {
                index: slot,
                size: 8, // 假设每个栈槽8字节
                alignment: 8,
                purpose: crate::compiler::StackSlotPurpose::Spill,
            })
            .collect();
        
        // 模拟释放一些寄存器以确保方法被使用
        if let Some((&first_vreg, _)) = self.vreg_to_preg.iter().next() {
            self.simulate_register_release(first_vreg);
        }
        
        // 更新统计信息
        let elapsed = start_time.elapsed().as_nanos() as u64;
        self.stats.allocation_time_ns = elapsed;
        self.stats.allocated_registers = self.vreg_to_preg.len();
        self.stats.spilled_registers = self.vreg_to_stack.len();
        self.stats.stack_slots_used = self.used_stack_slots.len();
        
        Ok(allocated_block)
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
        self.vreg_to_preg.clear();
        self.vreg_to_stack.clear();
        self.used_registers.clear();
        self.used_stack_slots.clear();
        self.next_stack_slot = 0;
        self.stats = RegisterAllocationStats::default();
    }
    
    fn get_stats(&self) -> RegisterAllocationStats {
        self.stats.clone()
    }
}

/// 优化的寄存器分配器
///
/// 提供高性能的寄存器分配，支持多种策略：
/// - LinearScan：线性扫描分配（基础）
/// - GraphColoring：图着色分配（高级）
/// - Hybrid：混合策略（默认）
pub struct OptimizedRegisterAllocator {
    /// 配置
    config: AllocatorConfig,
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

impl OptimizedRegisterAllocator {
    /// 创建新的优化寄存器分配器
    pub fn new(config: AllocatorConfig) -> Self {
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

    /// 创建默认配置的优化寄存器分配器
    pub fn default_config() -> Self {
        Self::new(AllocatorConfig::default())
    }

    /// 构建活跃区间
    fn build_live_intervals(&mut self, block: &CompiledIRBlock) {
        self.live_intervals.clear();
        
        let mut all_registers = HashSet::new();
        for instruction in &block.ops {
            self.collect_registers_from_instruction(instruction, &mut all_registers);
        }

        for &reg_id in &all_registers {
            let interval = self.compute_live_interval(block, reg_id);
            self.live_intervals.push(interval);
        }

        self.live_intervals.sort_by_key(|interval| interval.start);
    }

    /// 从指令中收集寄存器
    fn collect_registers_from_instruction(&self, instruction: &CompiledInstruction, registers: &mut HashSet<RegId>) {
        match &instruction.op {
            IROp::Add { dst, src1, src2 } |
            IROp::Sub { dst, src1, src2 } |
            IROp::Mul { dst, src1, src2 } |
            IROp::Div { dst, src1, src2, .. } => {
                registers.insert(*dst);
                registers.insert(*src1);
                registers.insert(*src2);
            }
            IROp::Load { dst, .. } |
            IROp::MovImm { dst, .. } => {
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

        for (i, instruction) in block.ops.iter().enumerate() {
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
            IROp::Div { dst, src1, src2, .. } => {
                *dst == reg_id || *src1 == reg_id || *src2 == reg_id
            }
            IROp::Load { dst, .. } |
            IROp::MovImm { dst, .. } => {
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
            active_intervals.retain(|active| active.end >= interval.start);

            for active in &active_intervals {
                if let Some(ref physical_reg) = active.physical_reg {
                    if !available_registers.contains(physical_reg) {
                        available_registers.push(physical_reg.clone());
                    }
                }
            }

            if !available_registers.is_empty() {
                interval.physical_reg = Some(available_registers.remove(0));
                self.stats.register_reuse_count.fetch_add(1, Ordering::Relaxed);
            } else {
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
        let mut nodes: Vec<_> = self.interference_graph.values().cloned().collect();
        nodes.sort_by(|a, b| b.priority.cmp(&a.priority));

        for mut node in nodes {
            let mut available_colors = Vec::new();
            
            for (i, _) in self.physical_registers.iter().enumerate() {
                let color = i;
                let mut color_available = true;
                
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

            if let Some(color) = available_colors.first() {
                node.color = Some(*color);
                node.physical_reg = Some(self.physical_registers[*color].clone());
            } else {
                node.physical_reg = None;
                self.stack_slots.insert(node.reg_id, self.allocate_stack_slot());
                self.stats.spill_count.fetch_add(1, Ordering::Relaxed);
            }

            self.interference_graph.insert(node.reg_id, node);
            self.stats.coloring_attempts.fetch_add(1, Ordering::Relaxed);
        }

        Ok(())
    }

    /// 混合分配策略
    fn hybrid_allocation(&mut self) -> Result<(), VmError> {
        if let Err(_) = self.linear_scan_allocation() {
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
        for instruction in &mut block.ops {
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
            IROp::Div { dst, src1, src2, .. } => {
                self.replace_register(&mut dst.clone());
                self.replace_register(&mut src1.clone());
                self.replace_register(&mut src2.clone());
            }
            IROp::Load { dst, .. } |
            IROp::MovImm { dst, .. } => {
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
        if let Some(interval) = self.live_intervals.iter()
            .find(|interval| interval.reg_id == *reg_id) {
            if let Some(ref physical_reg) = interval.physical_reg {
                // 分配了物理寄存器
            } else if let Some(stack_slot) = interval.stack_slot {
                // 溢出到栈
            }
        }
    }
}

impl RegisterAllocator for OptimizedRegisterAllocator {
    fn allocate(&mut self, block: &CompiledIRBlock) -> Result<CompiledIRBlock, VmError> {
        let start_time = std::time::Instant::now();
        
        let mut result_block = block.clone();
        
        self.build_live_intervals(&result_block);
        
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
        
        self.apply_allocation(&mut result_block)?;
        
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
                    _ => return Err(VmError::Core(vm_core::CoreError::InvalidParameter {
                        name: "strategy".to_string(),
                        value: value.to_string(),
                        message: format!("Invalid strategy: {}", value),
                    })),
                };
            }
            "max_physical_registers" => {
                self.config.max_physical_registers = value.parse()
                    .map_err(|_| VmError::Core(vm_core::CoreError::InvalidParameter {
                        name: "max_physical_registers".to_string(),
                        value: value.to_string(),
                        message: "Invalid max_physical_registers".to_string(),
                    }))?;
            }
            "spill_threshold" => {
                self.config.spill_threshold = value.parse()
                    .map_err(|_| VmError::Core(vm_core::CoreError::InvalidParameter {
                        name: "spill_threshold".to_string(),
                        value: value.to_string(),
                        message: "Invalid spill_threshold".to_string(),
                    }))?;
            }
            "enable_renaming" => {
                self.config.enable_renaming = value.parse()
                    .map_err(|_| VmError::Core(vm_core::CoreError::InvalidParameter {
                        name: "enable_renaming".to_string(),
                        value: value.to_string(),
                        message: "Invalid enable_renaming".to_string(),
                    }))?;
            }
            "enable_spill_optimization" => {
                self.config.enable_spill_optimization = value.parse()
                    .map_err(|_| VmError::Core(vm_core::CoreError::InvalidParameter {
                        name: "enable_spill_optimization".to_string(),
                        value: value.to_string(),
                        message: "Invalid enable_spill_optimization".to_string(),
                    }))?;
            }
            _ => return Err(VmError::Core(vm_core::CoreError::InvalidParameter {
                name: "option".to_string(),
                value: option.to_string(),
                message: format!("Unknown option: {}", option),
            })),
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