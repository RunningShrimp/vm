// 测试文件：验证图着色寄存器分配器的功能

use vm_engine_jit::optimizing_compiler::{
    RegisterAllocatorTrait, GraphColoringAllocator,
    RegisterAllocatorStats, RegisterAllocation
};
use vm_ir::{IROp, RegId};
use std::collections::HashMap;

/// 创建简单的IR操作序列用于测试
fn create_simple_ir_ops() -> Vec<IROp> {
    vec![
        IROp::MovImm { dst: 1, imm: 10 },
        IROp::MovImm { dst: 2, imm: 20 },
        IROp::Add { dst: 3, src1: 1, src2: 2 },
        IROp::Mov { dst: 0, src: 3 },
    ]
}

/// 创建复杂的IR操作序列（需要寄存器溢出）
fn create_complex_ir_ops() -> Vec<IROp> {
    let mut ops = Vec::new();
    // 创建大量寄存器使用，超过可用寄存器数量
    for i in 1..=35 { // 超过32个可用寄存器
        ops.push(IROp::MovImm { dst: i as RegId, imm: i as u64 });
        if i > 1 {
            ops.push(IROp::Add {
                dst: (i + 35) as RegId,
                src1: (i - 1) as RegId,
                src2: i as RegId,
            });
        }
    }
    ops
}

/// 测试图着色分配器的基本功能
#[test]
fn test_graph_coloring_allocator_basic_functionality() {
    let mut allocator = GraphColoringAllocator::new();
    let ops = create_simple_ir_ops();
    
    // 分析寄存器生命周期
    allocator.analyze_lifetimes(&ops);
    
    // 分配寄存器
    let allocations = allocator.allocate_registers(&ops);
    
    // 验证所有使用的寄存器都被分配
    assert!(!allocations.is_empty(), "Should allocate registers for all used registers");
    
    // 验证分配结果的有效性
    for (reg, allocation) in &allocations {
        assert!(*reg > 0, "Register ID should be valid");
        match allocation {
            RegisterAllocation::Register(phys_reg) => {
                assert!(*phys_reg < 32, "Physical register should be in valid range (0-31)");
            }
            RegisterAllocation::Stack(offset) => {
                assert!(*offset >= 0, "Stack offset should be non-negative");
            }
        }
    }
    
    // 检查统计信息
    let stats = allocator.get_stats();
    assert_eq!(stats.total_allocations, allocations.len() as u64);
    assert_eq!(stats.spills, 0); // 简单测试用例不应有溢出
}

/// 测试图着色分配器的溢出处理能力
#[test]
fn test_graph_coloring_allocator_spill_handling() {
    let mut allocator = GraphColoringAllocator::new();
    let ops = create_complex_ir_ops();
    
    allocator.analyze_lifetimes(&ops);
    let allocations = allocator.allocate_registers(&ops);
    
    // 验证当寄存器数量超过可用寄存器时，会有溢出
    assert!(!allocations.is_empty(), "Should allocate registers even with spill");
    
    // 检查是否有寄存器被溢出到内存
    let stats = allocator.get_stats();
    assert!(
        stats.spills > 0,
        "Should have spilled registers when exceeding physical register count"
    );
}

/// 测试函数入口
fn main() {
    let ops = create_simple_ir_ops();
    let mut allocator = GraphColoringAllocator::new();
    allocator.analyze_lifetimes(&ops);
    let result = allocator.allocate_registers(&ops);
    
    println!("寄存器分配结果:");
    for (reg, allocation) in result {
        println!("  虚拟寄存器 {}: {:?}", reg, allocation);
    }
    
    let stats = allocator.get_stats();
    println!("\n统计信息:");
    println!("  总分配次数: {}", stats.total_allocations);
    println!("  溢出次数: {}", stats.spills);
    println!("  使用的物理寄存器数量: {}", stats.physical_regs_used);
    println!("  平均分配时间 (ns): {}", stats.avg_allocation_time_ns);
}
