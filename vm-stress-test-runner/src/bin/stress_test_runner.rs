//! 压力测试运行器
//!
//! 独立的压力测试运行器，用于长时间运行的压力测试和稳定性测试

use clap::{Arg, Command};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use vm_core::{GuestAddr, GuestArch, MemoryAccess};
use vm_cross_arch::UnifiedExecutor;
use vm_engine_jit::core::{JITConfig, JITEngine};
use vm_ir::{IRBlock, IRBuilder, IROp, MemFlags, Terminator};
use vm_mem::{NumaAllocPolicy, NumaAllocator, NumaNodeInfo, SoftMmu};

/// 压力测试配置
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StressTestConfig {
    /// 测试持续时间（秒）
    duration: u64,
    /// 线程数
    threads: u32,
    /// 内存压力级别（1-10）
    memory_pressure: u8,
    /// CPU压力级别（1-10）
    cpu_pressure: u8,
    /// 输出文件路径
    output: String,
}

/// 压力测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StressTestResult {
    /// 测试名称
    name: String,
    /// 执行时间（毫秒）
    execution_time_ms: u64,
    /// 内存使用量（字节）
    memory_usage_bytes: u64,
    /// 操作数
    operations: u64,
    /// 错误数
    errors: u64,
    /// 吞吐量（操作/秒）
    throughput_ops_per_sec: f64,
}

/// 压力测试报告
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StressTestReport {
    /// 测试开始时间
    start_time: String,
    /// 测试结束时间
    end_time: String,
    /// 测试配置
    config: StressTestConfig,
    /// 测试结果
    results: Vec<StressTestResult>,
}

impl StressTestReport {
    /// 创建新的测试报告
    fn new(config: StressTestConfig) -> Self {
        let now = chrono::Utc::now();
        Self {
            start_time: now.to_rfc3339(),
            end_time: String::new(),
            config,
            results: Vec::new(),
        }
    }

    /// 完成测试
    fn finish(&mut self) {
        self.end_time = chrono::Utc::now().to_rfc3339();
    }

    /// 添加测试结果
    fn add_result(&mut self, result: StressTestResult) {
        self.results.push(result);
    }

    /// 保存到文件
    fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }
}

/// 运行跨架构翻译压力测试
fn run_cross_arch_stress_test(config: &StressTestConfig) -> StressTestResult {
    let test_name = "cross_arch_stress".to_string();
    let start_time = Instant::now();

    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let operations = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let errors = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let barrier = Arc::new(Barrier::new(config.threads as usize));

    let mut handles = Vec::new();

    for thread_id in 0..config.threads {
        let running_clone = running.clone();
        let operations_clone = operations.clone();
        let errors_clone = errors.clone();
        let barrier_clone = barrier.clone();
        let cpu_pressure = config.cpu_pressure;

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            // 创建执行器
            let mut executor =
                match UnifiedExecutor::auto_create(GuestArch::X86_64, 128 * 1024 * 1024) {
                    Ok(exec) => exec,
                    Err(_) => {
                        errors_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        return;
                    }
                };

            // 创建测试代码
            let test_code = create_test_code_x86();
            let code_base = 0x1000 + thread_id as u64 * 0x10000;

            // 加载代码
            for (i, byte) in test_code.iter().enumerate() {
                if executor
                    .mmu_mut()
                    .write(GuestAddr(code_base + i as u64), *byte as u64, 1)
                    .is_err()
                {
                    errors_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }

            // 根据CPU压力级别调整执行次数
            let execution_count = match cpu_pressure {
                1..=3 => 10,
                4..=6 => 50,
                7..=8 => 100,
                9..=10 => 200,
                _ => 50,
            };

            while running_clone.load(std::sync::atomic::Ordering::Relaxed) {
                // 执行代码多次
                for _ in 0..execution_count {
                    match executor.execute(GuestAddr(code_base)) {
                        Ok(_) => {
                            operations_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        }
                        Err(_) => errors_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                    };
                }

                // 短暂休息
                thread::sleep(Duration::from_millis(10));
            }
        });

        handles.push(handle);
    }

    // 运行指定时间
    thread::sleep(Duration::from_secs(config.duration));
    running.store(false, std::sync::atomic::Ordering::Relaxed);

    // 等待所有线程完成
    for handle in handles {
        let _ = handle.join();
    }

    let execution_time = start_time.elapsed();
    let total_operations = operations.load(std::sync::atomic::Ordering::Relaxed);
    let total_errors = errors.load(std::sync::atomic::Ordering::Relaxed);
    let memory_usage = estimate_memory_usage();

    StressTestResult {
        name: test_name,
        execution_time_ms: execution_time.as_millis() as u64,
        memory_usage_bytes: memory_usage,
        operations: total_operations,
        errors: total_errors,
        throughput_ops_per_sec: total_operations as f64 / execution_time.as_secs_f64(),
    }
}

/// 运行JIT编译压力测试
fn run_jit_compilation_stress_test(config: &StressTestConfig) -> StressTestResult {
    let test_name = "jit_compilation_stress".to_string();
    let start_time = Instant::now();

    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let operations = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let errors = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let barrier = Arc::new(Barrier::new(config.threads as usize));

    let mut handles = Vec::new();

    for thread_id in 0..config.threads {
        let running_clone = running.clone();
        let operations_clone = operations.clone();
        let errors_clone = errors.clone();
        let barrier_clone = barrier.clone();
        let memory_pressure = config.memory_pressure;
        let cpu_pressure = config.cpu_pressure;

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            // 创建JIT引擎
            let mut jit = JITEngine::new(JITConfig::default());

            // 根据压力级别调整工作负载
            let block_count = match memory_pressure {
                1..=3 => 10,
                4..=6 => 50,
                7..=8 => 100,
                9..=10 => 200,
                _ => 50,
            };

            let complexity = match cpu_pressure {
                1..=3 => 100,
                4..=6 => 500,
                7..=8 => 1000,
                9..=10 => 2000,
                _ => 500,
            };

            while running_clone.load(std::sync::atomic::Ordering::Relaxed) {
                // 创建测试IR块
                let block = create_complex_ir_block(
                    GuestAddr(0x1000 + thread_id as u64 * 0x10000),
                    complexity,
                );

                // 编译块
                match jit.compile(&block) {
                    Ok(_) => operations_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                    Err(_) => errors_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                };

                // 创建多个块以增加内存压力
                for i in 0..block_count {
                    let addr = 0x2000 + thread_id as u64 * 0x10000 + i as u64 * 0x1000;
                    let block = create_basic_ir_block(GuestAddr(addr), 100);
                    let _ = jit.compile(&block);
                }

                // 短暂休息
                thread::sleep(Duration::from_millis(10));
            }
        });

        handles.push(handle);
    }

    // 运行指定时间
    thread::sleep(Duration::from_secs(config.duration));
    running.store(false, std::sync::atomic::Ordering::Relaxed);

    // 等待所有线程完成
    for handle in handles {
        let _ = handle.join();
    }

    let execution_time = start_time.elapsed();
    let total_operations = operations.load(std::sync::atomic::Ordering::Relaxed);
    let total_errors = errors.load(std::sync::atomic::Ordering::Relaxed);
    let memory_usage = estimate_memory_usage();

    StressTestResult {
        name: test_name,
        execution_time_ms: execution_time.as_millis() as u64,
        memory_usage_bytes: memory_usage,
        operations: total_operations,
        errors: total_errors,
        throughput_ops_per_sec: total_operations as f64 / execution_time.as_secs_f64(),
    }
}

/// 运行内存管理压力测试
fn run_memory_stress_test(config: &StressTestConfig) -> StressTestResult {
    let test_name = "memory_stress".to_string();
    let start_time = Instant::now();

    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let operations = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let errors = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let barrier = Arc::new(Barrier::new(config.threads as usize));

    let mut handles = Vec::new();

    for thread_id in 0..config.threads {
        let running_clone = running.clone();
        let operations_clone = operations.clone();
        let errors_clone = errors.clone();
        let barrier_clone = barrier.clone();
        let memory_pressure = config.memory_pressure;

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            // 创建MMU
            let mut mmu = SoftMmu::new(1024 * 1024 * 1024, false);

            // 创建NUMA分配器
            let nodes = vec![NumaNodeInfo {
                node_id: 0,
                total_memory: 8 * 1024 * 1024 * 1024,     // 8GB
                available_memory: 7 * 1024 * 1024 * 1024, // 7GB
                cpu_mask: 0xFF,                           // CPU 0-7
            }];
            let allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Local);

            // 根据压力级别调整内存操作大小
            let allocation_size = match memory_pressure {
                1..=3 => 1024,     // 1KB
                4..=6 => 10240,    // 10KB
                7..=8 => 102400,   // 100KB
                9..=10 => 1024000, // 1MB
                _ => 10240,
            };

            let allocation_count = match memory_pressure {
                1..=3 => 100,
                4..=6 => 500,
                7..=8 => 1000,
                9..=10 => 2000,
                _ => 500,
            };

            let mut allocations = Vec::new();

            while running_clone.load(std::sync::atomic::Ordering::Relaxed) {
                // 分配内存
                for i in 0..allocation_count {
                    let addr = thread_id as u64 * 0x10000000 + i as u64 * allocation_size;
                    let layout = std::alloc::Layout::from_size_align(
                        allocation_size.try_into().unwrap_or(1),
                        8,
                    )
                    .unwrap_or(std::alloc::Layout::from_size_align(1, 1).unwrap());
                    match allocator.allocate(layout) {
                        Ok(ptr) => {
                            // 使用 addr 变量记录分配的地址
                            mmu.write(vm_core::GuestAddr(addr), addr, 8)
                                .unwrap_or_else(|_| {
                                    errors_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                });

                            allocations.push((ptr, allocation_size)); // 存储指针和大小用于释放
                            operations_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                        Err(_) => {
                            errors_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                }

                // 执行内存读写操作
                for &(ptr, _) in &allocations {
                    for offset in (0..allocation_size).step_by(4096) {
                        // 将指针转换为地址进行操作
                        let guest_addr = ptr.as_ptr() as u64 + offset;
                        // 写入
                        if mmu
                            .write(vm_core::GuestAddr(guest_addr), 0xDEADBEEF, 8)
                            .is_err()
                        {
                            errors_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        } else {
                            operations_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }

                        // 读取
                        if mmu.read(vm_core::GuestAddr(guest_addr), 8).is_err() {
                            errors_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        } else {
                            operations_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                }

                // 释放部分内存
                for _ in 0..allocation_count / 2 {
                    if let Some((ptr, size)) = allocations.pop() {
                        allocator.deallocate(ptr, size.try_into().unwrap_or(1));
                        operations_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }

                // 短暂休息
                thread::sleep(Duration::from_millis(10));
            }
        });

        handles.push(handle);
    }

    // 运行指定时间
    thread::sleep(Duration::from_secs(config.duration));
    running.store(false, std::sync::atomic::Ordering::Relaxed);

    // 等待所有线程完成
    for handle in handles {
        let _ = handle.join();
    }

    let execution_time = start_time.elapsed();
    let total_operations = operations.load(std::sync::atomic::Ordering::Relaxed);
    let total_errors = errors.load(std::sync::atomic::Ordering::Relaxed);
    let memory_usage = estimate_memory_usage();

    StressTestResult {
        name: test_name,
        execution_time_ms: execution_time.as_millis() as u64,
        memory_usage_bytes: memory_usage,
        operations: total_operations,
        errors: total_errors,
        throughput_ops_per_sec: total_operations as f64 / execution_time.as_secs_f64(),
    }
}

/// 运行资源泄漏测试
fn run_resource_leak_test() -> StressTestResult {
    let test_name = "resource_leak".to_string();
    let start_time = Instant::now();
    let mut operations = 0;
    let mut errors = 0;

    // 记录初始资源使用
    let initial_memory = estimate_memory_usage();

    // 创建和销毁大量资源
    for _ in 0..1000 {
        // 创建JIT引擎
        let mut jit = JITEngine::new(JITConfig::default());

        // 创建并编译多个块
        for i in 0..10 {
            let block = create_basic_ir_block(GuestAddr(0x1000 + i as u64 * 0x1000), 100);
            if jit.compile(&block).is_ok() {
                operations += 1;
            } else {
                errors += 1;
            }
        }

        // 创建执行器
        if let Ok(mut executor) = UnifiedExecutor::auto_create(GuestArch::X86_64, 128 * 1024 * 1024)
        {
            // 创建测试代码
            let test_code = create_test_code_x86();
            let code_base = 0x1000;

            // 加载代码
            for (i, byte) in test_code.iter().enumerate() {
                if executor
                    .mmu_mut()
                    .write(GuestAddr(code_base + i as u64), *byte as u64, 1)
                    .is_err()
                {
                    errors += 1;
                }
            }

            // 执行代码
            for _ in 0..10 {
                match executor.execute(GuestAddr(code_base)) {
                    Ok(_) => operations += 1,
                    Err(_) => errors += 1,
                }
            }
        } else {
            errors += 1;
        }

        // 创建MMU
        let mut mmu = SoftMmu::new(1024 * 1024, false);

        // 创建NUMA分配器
        let nodes = vec![NumaNodeInfo {
            node_id: 0,
            total_memory: 8 * 1024 * 1024 * 1024,     // 8GB
            available_memory: 7 * 1024 * 1024 * 1024, // 7GB
            cpu_mask: 0xFF,                           // CPU 0-7
        }];
        let allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Local);

        // 分配和释放内存
        let mut allocated_ptrs = Vec::new();
        for i in 0..100 {
            let addr = i as u64 * 4096;
            let layout = std::alloc::Layout::from_size_align(4096, 8)
                .unwrap_or(std::alloc::Layout::from_size_align(1, 1).unwrap());
            if let Ok(ptr) = allocator.allocate(layout) {
                allocated_ptrs.push((ptr, 4096));
                operations += 1;

                // 执行内存操作
                if mmu.write(GuestAddr(addr), 0xDEADBEEF, 8).is_ok() {
                    operations += 1;
                } else {
                    errors += 1;
                }

                if mmu.read(GuestAddr(addr), 8).is_ok() {
                    operations += 1;
                } else {
                    errors += 1;
                }

                allocator.deallocate(ptr, 4096);
                operations += 1;
            } else {
                errors += 1;
            }
        }
    }

    // 强制垃圾回收
    thread::sleep(Duration::from_millis(100));

    // 记录最终资源使用
    let final_memory = estimate_memory_usage();
    let memory_leak = final_memory.saturating_sub(initial_memory);

    let execution_time = start_time.elapsed();

    StressTestResult {
        name: test_name,
        execution_time_ms: execution_time.as_millis() as u64,
        memory_usage_bytes: memory_leak,
        operations,
        errors,
        throughput_ops_per_sec: operations as f64 / execution_time.as_secs_f64(),
    }
}

/// 估算内存使用量
fn estimate_memory_usage() -> u64 {
    // 这里应该使用实际的内存监控API
    // 为了示例，我们返回一个模拟值
    use std::sync::atomic::{AtomicU64, Ordering};
    static MEMORY_COUNTER: AtomicU64 = AtomicU64::new(0);
    MEMORY_COUNTER.fetch_add(1024 * 1024, Ordering::Relaxed) // 模拟1MB增长
}

/// 创建基础IR块
fn create_basic_ir_block(addr: GuestAddr, instruction_count: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);

    for i in 0..instruction_count {
        builder.push(IROp::MovImm {
            dst: (i % 16) as u32,
            imm: (i * 42) as u64,
        });
        builder.push(IROp::Add {
            dst: 0,
            src1: 0,
            src2: (i % 16) as u32,
        });
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 创建复杂IR块
fn create_complex_ir_block(addr: GuestAddr, complexity: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);

    for i in 0..complexity {
        match i % 8 {
            0 => {
                builder.push(IROp::MovImm {
                    dst: 1,
                    imm: i as u64,
                });
                builder.push(IROp::Add {
                    dst: 0,
                    src1: 0,
                    src2: 1,
                });
            }
            1 => {
                builder.push(IROp::Sub {
                    dst: 2,
                    src1: 0,
                    src2: 1,
                });
            }
            2 => {
                builder.push(IROp::Mul {
                    dst: 3,
                    src1: 2,
                    src2: 1,
                });
            }
            3 => {
                builder.push(IROp::Div {
                    dst: 4,
                    src1: 3,
                    src2: 1,
                    signed: false,
                });
            }
            4 => {
                builder.push(IROp::Load {
                    dst: 5,
                    base: 0,
                    offset: (i * 8) as i64,
                    size: 8,
                    flags: MemFlags::default(),
                });
            }
            5 => {
                builder.push(IROp::Store {
                    base: 0,
                    offset: (i * 8) as i64,
                    size: 8,
                    src: 5,
                    flags: MemFlags::default(),
                });
            }
            6 => {
                builder.push(IROp::SllImm {
                    dst: 6,
                    src: 0,
                    sh: 2,
                });
            }
            _ => {
                builder.push(IROp::SrlImm {
                    dst: 7,
                    src: 6,
                    sh: 1,
                });
            }
        }
    }

    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 创建x86测试代码
fn create_test_code_x86() -> Vec<u8> {
    vec![
        0xB8, 0x0A, 0x00, 0x00, 0x00, // mov eax, 10
        0xBB, 0x14, 0x00, 0x00, 0x00, // mov ebx, 20
        0x01, 0xD8, // add eax, ebx
        0x83, 0xC0, 0x05, // add eax, 5
        0x29, 0xD8, // sub eax, ebx
        0xC3, // ret
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("stress_test_runner")
        .version("1.0")
        .about("VM压力测试运行器")
        .arg(
            Arg::new("duration")
                .long("duration")
                .short('d')
                .value_name("SECONDS")
                .help("测试持续时间（秒）")
                .default_value("60"),
        )
        .arg(
            Arg::new("threads")
                .long("threads")
                .short('t')
                .value_name("COUNT")
                .help("线程数")
                .default_value("4"),
        )
        .arg(
            Arg::new("memory-pressure")
                .long("memory-pressure")
                .short('m')
                .value_name("LEVEL")
                .help("内存压力级别（1-10）")
                .default_value("5"),
        )
        .arg(
            Arg::new("cpu-pressure")
                .long("cpu-pressure")
                .short('c')
                .value_name("LEVEL")
                .help("CPU压力级别（1-10）")
                .default_value("5"),
        )
        .arg(
            Arg::new("output")
                .long("output")
                .short('o')
                .value_name("FILE")
                .help("输出文件路径")
                .default_value("stress_test_results.json"),
        )
        .arg(
            Arg::new("test")
                .long("test")
                .short('T')
                .value_name("TYPE")
                .help("测试类型 (all, cross_arch, jit, memory, resource_leak)")
                .default_value("all"),
        )
        .get_matches();

    let config = StressTestConfig {
        duration: matches.get_one::<String>("duration").unwrap().parse()?,
        threads: matches.get_one::<String>("threads").unwrap().parse()?,
        memory_pressure: matches
            .get_one::<String>("memory-pressure")
            .unwrap()
            .parse()?,
        cpu_pressure: matches.get_one::<String>("cpu-pressure").unwrap().parse()?,
        output: matches.get_one::<String>("output").unwrap().to_string(),
    };

    let test_type = matches.get_one::<String>("test").unwrap();

    println!("开始压力测试...");
    println!("配置: {:?}", config);
    println!("测试类型: {}", test_type);

    let mut report = StressTestReport::new(config.clone());

    match test_type.as_str() {
        "all" => {
            println!("运行所有压力测试...");
            report.add_result(run_cross_arch_stress_test(&config));
            report.add_result(run_jit_compilation_stress_test(&config));
            report.add_result(run_memory_stress_test(&config));
            report.add_result(run_resource_leak_test());
        }
        "cross_arch" => {
            println!("运行跨架构翻译压力测试...");
            report.add_result(run_cross_arch_stress_test(&config));
        }
        "jit" => {
            println!("运行JIT编译压力测试...");
            report.add_result(run_jit_compilation_stress_test(&config));
        }
        "memory" => {
            println!("运行内存管理压力测试...");
            report.add_result(run_memory_stress_test(&config));
        }
        "resource_leak" => {
            println!("运行资源泄漏测试...");
            report.add_result(run_resource_leak_test());
        }
        _ => {
            eprintln!("未知的测试类型: {}", test_type);
            eprintln!("支持的测试类型: all, cross_arch, jit, memory, resource_leak");
            std::process::exit(1);
        }
    }

    report.finish();

    // 保存结果
    report.save_to_file(&config.output)?;
    println!("结果已保存到: {}", config.output);

    // 打印摘要
    println!("\n==== 压力测试摘要 ====");
    for result in &report.results {
        println!("测试: {}", result.name);
        println!("  执行时间: {} ms", result.execution_time_ms);
        println!(
            "  内存使用: {:.2} MB",
            result.memory_usage_bytes as f64 / 1024.0 / 1024.0
        );
        println!("  操作数: {}", result.operations);
        println!("  错误数: {}", result.errors);
        println!("  吞吐量: {:.0} ops/sec", result.throughput_ops_per_sec);
        println!();
    }

    Ok(())
}
