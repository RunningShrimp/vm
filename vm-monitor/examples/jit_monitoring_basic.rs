//! JIT监控基础示例
//!
//! 本示例展示如何使用JitPerformanceMonitor进行JIT性能监控。
//!
//! 运行示例:
//! ```bash
//! cargo run --example jit_monitoring_basic --package vm-monitor
//! ```

use std::sync::Arc;
use vm_core::domain_services::{DomainEventBus, ExecutionEvent};
use vm_monitor::jit_monitor::JitPerformanceMonitor;

fn main() {
    println!("=== JIT性能监控基础示例 ===\n");

    // 1. 创建DomainEventBus（示例中暂未使用，但展示完整集成模式）
    let _event_bus = Arc::new(DomainEventBus::new());
    println!("✅ Created DomainEventBus");

    // 2. 创建JitPerformanceMonitor
    let monitor = Arc::new(JitPerformanceMonitor::new());
    println!("✅ Created JitPerformanceMonitor");

    // 3. 模拟JIT编译活动
    println!("\n📊 Simulating JIT compilation activity...\n");

    // 模拟代码块编译
    for i in 1..=10 {
        let pc = 0x1000 + (i as u64) * 0x100;
        let block_size = 50 + i * 10;

        let event = ExecutionEvent::CodeBlockCompiled {
            vm_id: "test-vm".to_string(),
            pc,
            block_size,
        };

        monitor.handle_code_block_compiled(&event);
        println!("Compiled block {}: PC=0x{:x}, size={} bytes",
                 i, pc, block_size);
    }

    // 模拟热点检测
    println!();
    for i in 1..=5 {
        let pc = 0x1000 + (i as u64) * 0x200;
        let exec_count = 100 * i as u64;

        let event = ExecutionEvent::HotspotDetected {
            vm_id: "test-vm".to_string(),
            pc,
            execution_count: exec_count,
        };

        monitor.handle_hotspot_detected(&event);
        println!("Hotspot detected: PC=0x{:x}, exec_count={}",
                 pc, exec_count);
    }

    // 4. 生成性能报告
    println!("\n📊 Generating performance report...\n");
    let report = monitor.generate_report();
    println!("{}", report);

    // 5. 显示统计信息
    let stats = monitor.get_statistics();
    println!("\n📈 Statistics Summary:");
    println!("  Total compilations: {}", stats.total_compilations);
    println!("  Total compiled bytes: {} bytes", stats.total_compiled_bytes);
    println!("  Average block size: {:.2} bytes", stats.avg_block_size);
    println!("  Total hotspots: {}", stats.total_hotspots);
    println!("  Average execution count: {:.2}", stats.avg_execution_count);

    // 6. 演示重置功能
    println!("\n🔄 Resetting monitor...");
    monitor.reset();

    let stats_after_reset = monitor.get_statistics();
    println!("After reset:");
    println!("  Total compilations: {}", stats_after_reset.total_compilations);
    println!("  Total hotspots: {}", stats_after_reset.total_hotspots);

    println!("\n✅ Example completed successfully!");
    println!("\n💡 Usage Notes:");
    println!("  - JitPerformanceMonitor可以独立使用，不依赖DomainEventBus");
    println!("  - 手动调用handle_code_block_compiled()和handle_hotspot_detected()");
    println!("  - 使用generate_report()获取详细性能报告");
    println!("  - 使用get_statistics()获取当前统计快照");
    println!("  - 使用reset()清空所有统计数据");
}
