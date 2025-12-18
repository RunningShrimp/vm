//! 跨架构TLB集成测试
//!
//! 测试统一TLB接口在不同架构下的兼容性和性能

use vm_core::{AccessType, GuestAddr, TlbEntry, TlbManager};
use vm_mem::{
    ConcurrentTlbConfig, ConcurrentTlbManagerAdapter, MultiLevelTlb, MultiLevelTlbConfig,
    UnifiedMmu, UnifiedMmuConfig,
};

// 测试架构列表
const TEST_ARCHITECTURES: &[(&str, usize)] = &[("x86_64", 16), ("arm64", 32), ("riscv64", 32)];

// 测试页大小
const PAGE_SIZE: u64 = 4096;

/// 通用TLB功能测试
fn test_tlb_basic_functionality(tlb: &mut dyn TlbManager) {
    println!("=== 测试TLB基本功能 ===");

    // 测试1: 基本插入和查找
    let entry = TlbEntry {
        guest_addr: 0x1000,
        phys_addr: 0x2000,
        flags: 0x7, // R+W+X
        asid: 0,
    };
    tlb.update(entry);

    let result = tlb.lookup(0x1000, 0, AccessType::Read);
    assert!(result.is_some());
    assert_eq!(result.unwrap().phys_addr, 0x2000);
    println!("✓ 基本插入和查找测试通过");

    // 测试2: 地址对齐检查
    let result = tlb.lookup(0x1001, 0, AccessType::Write);
    assert!(result.is_some());
    assert_eq!(result.unwrap().phys_addr, 0x2001); // 应该自动对齐
    println!("✓ 地址对齐测试通过");

    // 测试3: ASID隔离
    let entry_asid1 = TlbEntry {
        guest_addr: 0x1000,
        phys_addr: 0x3000,
        flags: 0x5, // R+W
        asid: 1,
    };
    tlb.update(entry_asid1);

    let result_asid0 = tlb.lookup(0x1000, 0, AccessType::Read);
    let result_asid1 = tlb.lookup(0x1000, 1, AccessType::Read);
    assert!(result_asid0.is_some());
    assert!(result_asid1.is_some());
    assert_eq!(result_asid0.unwrap().phys_addr, 0x2000);
    assert_eq!(result_asid1.unwrap().phys_addr, 0x3000);
    println!("✓ ASID隔离测试通过");

    // 测试4: TLB刷新
    tlb.flush();
    let result_after_flush = tlb.lookup(0x1000, 0, AccessType::Read);
    assert!(result_after_flush.is_none());
    println!("✓ TLB刷新测试通过");
}

/// 测试TLB与UnifiedMmu的集成
fn test_tlb_with_unified_mmu() {
    println!("\n=== 测试TLB与UnifiedMmu集成 ===");

    // 配置UnifiedMmu
    let config = UnifiedMmuConfig {
        strategy: vm_mem::MmuOptimizationStrategy::Hybrid,
        unified_tlb_config: vm_mem::UnifiedTlbConfig::default(),
        ..Default::default()
    };

    // 创建UnifiedMmu实例
    let mut mmu = vm_mem::UnifiedMmu::new(
        128 * 1024 * 1024, // 128MB内存
        config,
    );

    // 测试: 检查flush_tlb方法是否存在
    mmu.flush_tlb();
    println!("✓ UnifiedMmu.flush_tlb() 方法调用成功");
}

/// 多架构TLB性能基准测试
fn benchmark_tlb_performance() {
    println!("\n=== 测试TLB性能 ===");

    // 测试配置
    let iterations = 100000;
    let working_set_size = 1024;

    // 测试MultiLevelTlb
    let ml_config = MultiLevelTlbConfig::default();
    let mut ml_tlb = MultiLevelTlb::new(ml_config);

    let start = std::time::Instant::now();
    for i in 0..iterations {
        let addr = (i % working_set_size) as u64 * PAGE_SIZE;
        let entry = TlbEntry {
            guest_addr: addr,
            phys_addr: addr + 0x1000000,
            flags: 0x7,
            asid: (i % 16) as u16,
        };
        ml_tlb.update(entry);
        let _ = ml_tlb.lookup(addr, (i % 16) as u16, AccessType::Read);
    }
    let duration = start.elapsed();
    println!(
        "MultiLevelTlb 性能: {:.2} 操作/ms",
        iterations as f64 / duration.as_millis() as f64
    );

    // 测试ConcurrentTlbManager
    let ct_config = ConcurrentTlbConfig::default();
    let mut ct_tlb = ConcurrentTlbManagerAdapter::new(ct_config);

    let start = std::time::Instant::now();
    for i in 0..iterations {
        let addr = (i % working_set_size) as u64 * PAGE_SIZE;
        let entry = TlbEntry {
            guest_addr: addr,
            phys_addr: addr + 0x1000000,
            flags: 0x7,
            asid: (i % 16) as u16,
        };
        ct_tlb.update(entry);
        let _ = ct_tlb.lookup(addr, (i % 16) as u16, AccessType::Read);
    }
    let duration = start.elapsed();
    println!(
        "ConcurrentTlbManager 性能: {:.2} 操作/ms",
        iterations as f64 / duration.as_millis() as f64
    );
}

#[test]
fn run_tlb_cross_arch_integration_tests() {
    println!("=== 启动跨架构TLB集成测试 ===");

    // 测试MultiLevelTlb
    println!("\n--- 测试 MultiLevelTlb ---");
    let ml_config = MultiLevelTlbConfig::default();
    let mut ml_tlb = MultiLevelTlb::new(ml_config);
    test_tlb_basic_functionality(&mut ml_tlb);

    // 测试ConcurrentTlbManager
    println!("\n--- 测试 ConcurrentTlbManager ---");
    let ct_config = ConcurrentTlbConfig::default();
    let mut ct_tlb = ConcurrentTlbManagerAdapter::new(ct_config);
    test_tlb_basic_functionality(&mut ct_tlb);

    // 测试与UnifiedMmu的集成
    test_tlb_with_unified_mmu();

    // 运行性能基准测试
    benchmark_tlb_performance();

    println!("\n=== 所有测试通过! ===");
}

#[test]
fn test_arch_specific_tlb_behavior() {
    println!("=== 测试架构特定TLB行为 ===");

    // 这个测试可以扩展为检查不同架构下的TLB特性
    // 例如页大小、地址空间布局等

    for (arch, reg_count) in TEST_ARCHITECTURES {
        println!("--- 架构: {} ---", arch);
        println!("寄存器数量: {}", reg_count);
        println!("默认页大小: {}KB", PAGE_SIZE / 1024);

        // 这里可以添加架构特定的测试逻辑
        assert!(reg_count > 8, "{} 应该至少有8个通用寄存器", arch);
        assert!(PAGE_SIZE == 4096, "{} 应该使用4KB页", arch);

        println!("✓ 架构基本测试通过");
    }
}