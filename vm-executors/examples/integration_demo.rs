//! vm-executors Integration Demo
//!
//! This example demonstrates the integration of all three executor modules:
//! - Async executor (JIT, Interpreter, Hybrid)
//! - Coroutine scheduler
//! - Distributed execution

use vm_executors::{
    FaultToleranceConfig, JitExecutor, LoadBalancingStrategy, coroutine::Scheduler,
    distributed::DistributedArchitectureConfig,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== vm-executors Integration Demo ===\n");

    // 1. Async Executor Demo
    println!("1. Async Executor Demo");
    println!("   Creating JIT executor...");
    let mut jit_executor = JitExecutor::new();

    println!("   Executing blocks...");
    for block_id in 1..=5 {
        let result = jit_executor.execute_block(block_id)?;
        println!("   - Block {} executed successfully", result);
    }

    let stats = jit_executor.get_stats();
    println!(
        "   Stats: {} executions, {} cached blocks, avg time: {}μs\n",
        stats.total_executions, stats.cached_blocks, stats.avg_time_us
    );

    // 2. Coroutine Scheduler Demo
    println!("2. Coroutine Scheduler Demo");
    println!("   Creating scheduler with 4 vCPUs...");
    let mut scheduler = Scheduler::new(4);

    println!("   Creating and submitting coroutines...");
    for i in 0..10 {
        let mut coroutine = scheduler.create_coroutine();
        coroutine.mark_ready();
        scheduler.submit_coroutine(coroutine);
        println!("   - Coroutine {} created and submitted", i);
    }

    println!("\n   Simulating execution on vCPU 0...");
    let mut executed = 0;
    while let Some(mut coro) = scheduler.next_coroutine(0) {
        coro.mark_running();
        coro.record_execution(100); // Simulate 100μs execution
        executed += 1;
        if executed >= 5 {
            break;
        }
    }

    let stats = scheduler.get_stats();
    println!("   Executed {} coroutines", executed);
    println!("   Load imbalance: {:.2}", stats.load_imbalance);
    println!("   Global queue length: {}\n", stats.global_queue_length);

    // 3. Work Stealing Demo
    println!("3. Work Stealing Demo");
    println!("   Assigning coroutines to vCPU 0...");
    for _ in 0..3 {
        let coro = scheduler.create_coroutine();
        scheduler.assign_to_vcpu(0, coro).unwrap();
    }

    println!(
        "   vCPU 0 queue length: {}",
        scheduler.get_vcpu_stats(0).unwrap().executions
    );

    println!("   vCPU 1 attempting to steal work...");
    if let Some(stolen) = scheduler.try_steal_work(1) {
        println!("   - Successfully stole coroutine {}", stolen.id);
    }

    // 4. Distributed Execution Demo (mock)
    println!("\n4. Distributed Execution Demo");
    println!("   Creating distributed configuration...");
    let config = DistributedArchitectureConfig {
        coordinator_addr: "127.0.0.1:8080".parse().unwrap(),
        initial_vm_addrs: vec![
            "127.0.0.1:8081".parse().unwrap(),
            "127.0.0.1:8082".parse().unwrap(),
        ],
        discovery_port: 8081,
        comm_port: 8082,
        load_balancing_strategy: LoadBalancingStrategy::RoundRobin,
        fault_tolerance: FaultToleranceConfig::default(),
    };

    println!(
        "   Config created with {} initial VMs",
        config.initial_vm_addrs.len()
    );
    println!("   Load balancing: {:?}", config.load_balancing_strategy);
    println!(
        "   Fault tolerance: max retries = {}",
        config.fault_tolerance.max_retries
    );

    println!("\n=== Demo Complete ===");
    println!("All executor modules integrated successfully!");

    Ok(())
}
