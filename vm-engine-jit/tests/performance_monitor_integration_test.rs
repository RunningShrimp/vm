//! JIT性能监控集成测试
//!
//! 测试EventBasedJitMonitor与Jit编译器的集成功能

use vm_core::GuestAddr;
use vm_engine_jit::Jit;
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};

/// 创建测试IR块
fn create_test_block(addr: GuestAddr, size: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);

    for i in 0..size {
        let dst = (i % 16) as u32;
        let src1 = ((i + 1) % 16) as u32;
        let src2 = ((i + 2) % 16) as u32;

        // 混合不同类型的操作
        match i % 5 {
            0 => builder.push(IROp::MovImm { dst, imm: i as u64 }),
            1 => builder.push(IROp::Add { dst, src1, src2 }),
            2 => builder.push(IROp::Mul { dst, src1, src2 }),
            3 => builder.push(IROp::Load {
                dst,
                base: src1,
                offset: (i * 8) as i64,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            }),
            _ => builder.push(IROp::Store {
                src: dst,
                base: src1,
                offset: (i * 8) as i64,
                size: 8,
                flags: vm_ir::MemFlags::default(),
            }),
        }
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

#[test]
fn test_performance_monitor_basic() {
    // 创建JIT编译器并启用性能监控（禁用ML引导，确保编译）
    let mut jit = Jit::with_ml_guidance(false);
    jit.enable_performance_monitor();

    // 编译几个测试块
    let block1 = create_test_block(GuestAddr(0x1000), 50);
    let block2 = create_test_block(GuestAddr(0x2000), 100);
    let block3 = create_test_block(GuestAddr(0x3000), 150);

    jit.compile_only(&block1);
    jit.compile_only(&block2);
    jit.compile_only(&block3);

    // 获取性能监控器并生成报告
    let monitor = jit.get_performance_monitor().expect("Monitor should be enabled");
    let report = monitor.generate_report();

    // 验证报告
    assert_eq!(report.global_metrics.total_compilations, 3);
    assert_eq!(report.blocks_reported, 3);

    // 打印报告供人工检查
    println!("\n=== Performance Monitor Test ===");
    report.print();
}

#[test]
fn test_performance_monitor_hotspot_tracking() {
    let mut jit = Jit::new();
    jit.enable_performance_monitor();

    // 创建一个块并多次执行以触发热点检测
    let block = create_test_block(GuestAddr(0x1000), 50);

    // 编译块
    jit.compile_only(&block);

    // 模拟多次执行（通过直接调用hot_counts）
    // 注意：这需要访问内部API，实际使用中通过run()会自动触发
    // 这里我们只是验证监控器可以记录热点

    let monitor = jit.get_performance_monitor().expect("Monitor should be enabled");

    // 手动记录一些热点事件
    monitor.record_hotspot(0x1000);
    monitor.record_hotspot(0x1000);
    monitor.record_hotspot(0x1000);

    let report = monitor.generate_report();

    // 验证热点被记录
    assert!(report.global_metrics.hotspot_detections >= 3);

    println!("\n=== Hotspot Tracking Test ===");
    report.print();
}

#[test]
fn test_performance_monitor_disabled_by_default() {
    // 不启用监控
    let mut jit = Jit::new();

    // 编译一些块
    let block1 = create_test_block(GuestAddr(0x1000), 50);
    jit.compile_only(&block1);

    // 验证监控器未启用
    assert!(jit.get_performance_monitor().is_none());
}

#[test]
fn test_performance_monitor_enable_disable() {
    let mut jit = Jit::with_ml_guidance(false);

    // 初始状态：未启用
    assert!(jit.get_performance_monitor().is_none());

    // 启用监控
    jit.enable_performance_monitor();
    assert!(jit.get_performance_monitor().is_some());

    // 编译一些块
    let block1 = create_test_block(GuestAddr(0x1000), 50);
    jit.compile_only(&block1);

    // 禁用监控并获取监控器
    let monitor = jit.disable_performance_monitor().expect("Monitor should be returned");

    // 验证监控器包含数据
    let report = monitor.generate_report();
    assert_eq!(report.global_metrics.total_compilations, 1);

    // 验证JIT中不再有监控器
    assert!(jit.get_performance_monitor().is_none());
}

#[test]
fn test_performance_monitor_multiple_blocks() {
    let mut jit = Jit::with_ml_guidance(false);
    jit.enable_performance_monitor();

    // 编译多个块，大小和地址不同
    let blocks = vec![
        create_test_block(GuestAddr(0x1000), 10),
        create_test_block(GuestAddr(0x2000), 50),
        create_test_block(GuestAddr(0x3000), 100),
        create_test_block(GuestAddr(0x4000), 200),
        create_test_block(GuestAddr(0x5000), 500),
    ];

    for block in &blocks {
        jit.compile_only(block);
    }

    let monitor = jit.get_performance_monitor().expect("Monitor should be enabled");
    let report = monitor.generate_report();

    // 验证所有块都被记录
    assert_eq!(report.global_metrics.total_compilations, 5);
    assert_eq!(report.blocks_reported, 5);

    // 验证编译时间统计
    assert!(report.global_metrics.avg_compile_time_ns > 0);
    assert!(report.global_metrics.min_compile_time_ns > 0);
    assert!(report.global_metrics.max_compile_time_ns >= report.global_metrics.min_compile_time_ns);

    // 验证最慢和最热的块列表
    assert!(!report.slowest_blocks.is_empty());
    assert!(!report.most_hot_blocks.is_empty() || report.global_metrics.hotspot_detections == 0);

    println!("\n=== Multiple Blocks Test ===");
    report.print();
}
