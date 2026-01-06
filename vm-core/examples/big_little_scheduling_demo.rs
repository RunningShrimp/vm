//! Round 38: macOS大小核调度演示
//!
//! 展示如何使用调度API优化P-core/E-core任务分配

use std::thread;
use std::time::Duration;

use vm_core::scheduling::{
    with_background_cleanup, with_latency_sensitive, with_performance_critical,
    with_task_category, BigLittleScheduler, TaskCategory,
};

fn main() {
    println!("=== Round 38: macOS大小核调度演示 ===\n");

    // 1. 显示当前QoS
    println!("📊 当前QoS信息:");
    let current_qos = vm_core::scheduling::get_current_thread_qos();
    println!("  当前QoS类: {:?}", current_qos);
    println!("  优先级分数: {}", current_qos.priority_score());
    println!("  偏好P-core: {}", current_qos.prefers_performance_core());
    println!("  偏好E-core: {}", current_qos.prefers_efficiency_core());
    println!();

    // 2. 演示性能关键任务 (P-core)
    println!("🚀 性能关键任务 (P-core):");
    with_performance_critical(|| {
        let current = vm_core::scheduling::get_current_thread_qos();
        println!("  当前QoS: {:?} ({})", current, current.name());
        println!("  用途: JIT编译、热点代码生成");
        println!("  核心类型: P-core (4.5GHz)");
        println!("  模拟工作负载...");
        let start = std::time::Instant::now();
        simulate_compute_workload(100_000);
        let elapsed = start.elapsed();
        println!("  耗时: {:?}", elapsed);
    });
    println!();

    // 3. 演示延迟敏感任务 (P-core)
    println!("⚡ 延迟敏感任务 (P-core):");
    with_latency_sensitive(|| {
        let current = vm_core::scheduling::get_current_thread_qos();
        println!("  当前QoS: {:?} ({})", current, current.name());
        println!("  用途: 同步操作、事件处理");
        println!("  核心类型: P-core (4.5GHz)");
        println!("  模拟延迟敏感操作...");
        let start = std::time::Instant::now();
        simulate_latency_sensitive_workload();
        let elapsed = start.elapsed();
        println!("  耗时: {:?}", elapsed);
    });
    println!();

    // 4. 演示批处理任务 (E-core)
    println!("📦 批处理任务 (E-core):");
    with_task_category(TaskCategory::BatchProcessing, || {
        let current = vm_core::scheduling::get_current_thread_qos();
        println!("  当前QoS: {:?} ({})", current, current.name());
        println!("  用途: 垃圾回收、批量优化");
        println!("  核心类型: E-core (2.5GHz)");
        println!("  模拟批处理工作负载...");
        let start = std::time::Instant::now();
        simulate_batch_workload();
        let elapsed = start.elapsed();
        println!("  耗时: {:?}", elapsed);
    });
    println!();

    // 5. 演示后台清理任务 (E-core)
    println!("🧹 后台清理任务 (E-core):");
    with_background_cleanup(|| {
        let current = vm_core::scheduling::get_current_thread_qos();
        println!("  当前QoS: {:?} ({})", current, current.name());
        println!("  用途: 缓存清理、日志归档");
        println!("  核心类型: E-core (2.5GHz)");
        println!("  模拟后台清理工作负载...");
        let start = std::time::Instant::now();
        simulate_cleanup_workload();
        let elapsed = start.elapsed();
        println!("  耗时: {:?}", elapsed);
    });
    println!();

    // 6. 演示BigLittleScheduler自动调度
    println!("🤖 BigLittleScheduler自动调度:");
    let scheduler = BigLittleScheduler::new();
    println!("  调度策略: {:?}", scheduler.policy());

    let categories = vec![
        TaskCategory::PerformanceCritical,
        TaskCategory::LatencySensitive,
        TaskCategory::Normal,
        TaskCategory::BatchProcessing,
        TaskCategory::BackgroundCleanup,
    ];

    for category in categories {
        scheduler.schedule_task(category, || {
            let qos = vm_core::scheduling::get_current_thread_qos();
            let core_type = category.recommended_core_type();
            println!(
                "  {:?}: QoS={:?}, 核心={}",
                category, qos, core_type
            );
        });
    }
    println!();

    // 7. 实际应用场景示例
    println!("💡 实际应用场景:");
    println!();

    println!("场景1: JIT编译器 → PerformanceCritical");
    println!("  代码示例:");
    println!("  ```rust");
    println!("  fn compile_jit_code(&self, bytecode: &[u8]) {");
    println!("      with_performance_critical(|| {");
    println!("          // JIT编译逻辑");
    println!("          // 在P-core上运行以获得最快编译速度");
    println!("      });");
    println!("  }");
    println!("  ```");
    println!();

    println!("场景2: 垃圾回收 → BatchProcessing");
    println!("  代码示例:");
    println!("  ```rust");
    println!("  fn run_gc_cycle(&mut self) {");
    println!("      with_task_category(TaskCategory::BatchProcessing, || {");
    println!("          // GC逻辑");
    println!("          // 在E-core上运行以降低对前台任务的影响");
    println!("      });");
    println!("  }");
    println!("  ```");
    println!();

    println!("场景3: 用户交互事件 → LatencySensitive");
    println!("  代码示例:");
    println!("  ```rust");
    println!("  fn handle_user_event(&self, event: Event) {");
    println!("      with_latency_sensitive(|| {");
    println!("          // 事件处理逻辑");
    println!("          // 在P-core上运行以获得快速响应");
    println!("      });");
    println!("  }");
    println!("  ```");
    println!();

    println!("场景4: 后台优化 → BackgroundCleanup");
    println!("  代码示例:");
    println!("  ```rust");
    println!("  fn optimize_background(&self, code: &CompiledCode) {");
    println!("      with_background_cleanup(|| {");
    println!("          // 后台优化逻辑");
    println!("          // 在E-core上运行,不影响性能");
    println!("      });");
    println!("  }");
    println!("  ```");
    println!();

    // 8. 集成到vm-engine-jit
    println!("🔗 vm-engine-jit集成示例:");
    println!("  文件: vm-engine-jit/src/compiler.rs");
    println!("  ```rust");
    println!("  use vm_core::scheduling::with_performance_critical;");
    println!();
    println!("  impl JITCompiler {");
    println!("      pub fn compile(&self, bytecode: &[u8]) -> CompiledCode {");
    println!("          with_performance_critical(|| {");
    println!("              // 编译逻辑");
    println!("              // ...");
    println!("              compiled_code");
    println!("          })");
    println!("      }");
    println!("  }");
    println!("  ```");
    println!();

    // 9. 集成到vm-gc
    println!("🔗 vm-gc集成示例:");
    println!("  文件: vm-gc/src/gc.rs");
    println!("  ```rust");
    println!("  use vm_core::scheduling::with_task_category;");
    println!("  use vm_core::scheduling::TaskCategory;");
    println!();
    println!("  impl GarbageCollector {");
    println!("      pub fn collect(&mut self) {");
    println!("          with_task_category(TaskCategory::BatchProcessing, || {");
    println!("              // GC逻辑");
    println!("              // ...");
    println!("          });");
    println!("      }");
    println!("  }");
    println!("  ```");
    println!();

    println!("✅ Round 38调度演示完成!");
}

/// 模拟计算密集型工作负载
fn simulate_compute_workload(iterations: u64) {
    let mut result = 0u64;
    for i in 0..iterations {
        result = result.wrapping_add(i);
        result = result.wrapping_mul(3);
        result ^= result >> 32;
    }
    // 防止编译器优化掉
    std::hint::black_box(result);
}

/// 模拟延迟敏感工作负载
fn simulate_latency_sensitive_workload() {
    // 模拟需要快速响应的操作
    thread::sleep(Duration::from_micros(100));
    let mut data = vec![0u8; 1024];
    for i in 0..data.len() {
        data[i] = i as u8;
    }
    std::hint::black_box(data);
}

/// 模拟批处理工作负载
fn simulate_batch_workload() {
    // 模拟可以延后的大量数据处理
    let mut data = vec![0u64; 10_000];
    for i in 0..data.len() {
        data[i] = (i as u64).pow(3);
    }
    let sum: u64 = data.iter().sum();
    std::hint::black_box(sum);
}

/// 模拟清理工作负载
fn simulate_cleanup_workload() {
    // 模拟后台清理任务
    let mut data = vec![String::new(); 1000];
    for i in 0..data.len() {
        data[i] = format!("item_{}", i);
    }
    data.clear(); // 清理
    data.shrink_to_fit();
    std::hint::black_box(data);
}
