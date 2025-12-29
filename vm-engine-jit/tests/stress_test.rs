use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use vm_core::GuestAddr;
use vm_engine_jit::{
    TieredJITCompiler,
    code_cache::{CodeCache, LRUCache},
    inline_cache::InlineCache,
    tiered_compiler::TieredCompilerConfig,
};
use vm_ir::{IRBlock, IROp, Terminator};

/// 压力测试配置
#[derive(Debug, Clone)]
pub struct StressTestConfig {
    /// 测试持续时间（秒）
    pub duration_secs: u64,
    /// 并发线程数
    pub thread_count: usize,
    /// 每个 IR 块的指令数
    pub block_size: usize,
    /// IR 块数量
    pub block_count: usize,
    /// 缓存大小（字节）
    pub cache_size: usize,
    /// 是否启用分层编译
    pub enable_tiered_compilation: bool,
    /// 是否启用内联缓存
    pub enable_inline_cache: bool,
}

impl Default for StressTestConfig {
    fn default() -> Self {
        Self {
            duration_secs: 60,
            thread_count: num_cpus::get(),
            block_size: 100,
            block_count: 1000,
            cache_size: 1024 * 1024 * 64,
            enable_tiered_compilation: true,
            enable_inline_cache: true,
        }
    }
}

/// 压力测试结果
#[derive(Debug, Clone)]
pub struct StressTestResult {
    /// 总执行次数
    pub total_executions: u64,
    /// 平均执行时间（纳秒）
    pub avg_execution_time_ns: u64,
    /// 最小执行时间（纳秒）
    pub min_execution_time_ns: u64,
    /// 最大执行时间（纳秒）
    pub max_execution_time_ns: u64,
    /// 执行成功率
    pub success_rate: f64,
    /// 错误次数
    pub error_count: u64,
    /// 缓存命中率
    pub cache_hit_rate: f64,
    /// 内存使用（字节）
    pub memory_usage_bytes: usize,
}

/// 分层编译器压力测试
pub fn test_tiered_compiler_stress(config: StressTestConfig) -> StressTestResult {
    let tiered_config = TieredCompilerConfig::default();
    let mut compiler = TieredJITCompiler::new(tiered_config);

    let start_time = Instant::now();
    let duration = Duration::from_secs(config.duration_secs);

    let mut total_executions: u64 = 0;
    let mut total_time_ns: u64 = 0;
    let mut min_time_ns: u64 = u64::MAX;
    let mut max_time_ns: u64 = 0;
    let mut error_count: u64 = 0;

    let mut blocks = Vec::with_capacity(config.block_count);
    for i in 0..config.block_count {
        blocks.push(create_test_block(i as u64 * 0x1000, config.block_size));
    }

    while start_time.elapsed() < duration {
        for block in &blocks {
            let exec_start = Instant::now();

            match compiler.execute(block) {
                Ok(_) => {
                    let elapsed = exec_start.elapsed().as_nanos() as u64;
                    total_executions += 1;
                    total_time_ns += elapsed;
                    min_time_ns = min_time_ns.min(elapsed);
                    max_time_ns = max_time_ns.max(elapsed);
                }
                Err(_) => {
                    error_count += 1;
                }
            }
        }
    }

    let cache_stats = compiler.tiered_cache.lock().unwrap().stats();
    let cache_hit_rate = cache_stats.hit_rate();

    StressTestResult {
        total_executions,
        avg_execution_time_ns: if total_executions > 0 {
            total_time_ns / total_executions
        } else {
            0
        },
        min_execution_time_ns: if min_time_ns == u64::MAX {
            0
        } else {
            min_time_ns
        },
        max_execution_time_ns,
        success_rate: if total_executions + error_count > 0 {
            total_executions as f64 / (total_executions + error_count) as f64
        } else {
            0.0
        },
        error_count,
        cache_hit_rate,
        memory_usage_bytes: 0,
    }
}

/// 并发执行压力测试
pub fn test_concurrent_execution(config: StressTestConfig) -> StressTestResult {
    let tiered_config = TieredCompilerConfig::default();
    let compiler = Arc::new(Mutex::new(TieredJITCompiler::new(tiered_config)));

    let start_time = Instant::now();
    let duration = Duration::from_secs(config.duration_secs);

    let results = Arc::new(Mutex::new(Vec::new()));

    let mut handles = Vec::new();

    for thread_id in 0..config.thread_count {
        let compiler = Arc::clone(&compiler);
        let results = Arc::clone(&results);
        let block_start = thread_id * config.block_count / config.thread_count;
        let block_end = (thread_id + 1) * config.block_count / config.thread_count;
        let block_size = config.block_size;

        let handle = thread::spawn(move || {
            let mut total_executions: u64 = 0;
            let mut total_time_ns: u64 = 0;
            let mut min_time_ns: u64 = u64::MAX;
            let mut max_time_ns: u64 = 0;
            let mut error_count: u64 = 0;

            let mut blocks = Vec::new();
            for i in block_start..block_end {
                blocks.push(create_test_block(i as u64 * 0x1000, block_size));
            }

            while Instant::now().duration_since(start_time) < duration {
                for block in &blocks {
                    let exec_start = Instant::now();

                    match compiler.lock().unwrap().execute(block) {
                        Ok(_) => {
                            let elapsed = exec_start.elapsed().as_nanos() as u64;
                            total_executions += 1;
                            total_time_ns += elapsed;
                            min_time_ns = min_time_ns.min(elapsed);
                            max_time_ns = max_time_ns.max(elapsed);
                        }
                        Err(_) => {
                            error_count += 1;
                        }
                    }
                }
            }

            let mut results = results.lock().unwrap();
            results.push((
                total_executions,
                total_time_ns,
                min_time_ns,
                max_time_ns,
                error_count,
            ));
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let results = results.lock().unwrap();
    let mut total_executions: u64 = 0;
    let mut total_time_ns: u64 = 0;
    let mut min_time_ns: u64 = u64::MAX;
    let mut max_time_ns: u64 = 0;
    let mut error_count: u64 = 0;

    for (execs, time, min_t, max_t, errors) in results.iter() {
        total_executions += execs;
        total_time_ns += time;
        min_time_ns = min_time_ns.min(*min_t);
        max_time_ns = max_time_ns.max(*max_t);
        error_count += errors;
    }

    let cache_stats = compiler
        .lock()
        .unwrap()
        .tiered_cache
        .lock()
        .unwrap()
        .stats();
    let cache_hit_rate = cache_stats.hit_rate();

    StressTestResult {
        total_executions,
        avg_execution_time_ns: if total_executions > 0 {
            total_time_ns / total_executions
        } else {
            0
        },
        min_execution_time_ns: if min_time_ns == u64::MAX {
            0
        } else {
            min_time_ns
        },
        max_execution_time_ns,
        success_rate: if total_executions + error_count > 0 {
            total_executions as f64 / (total_executions + error_count) as f64
        } else {
            0.0
        },
        error_count,
        cache_hit_rate,
        memory_usage_bytes: 0,
    }
}

/// 内存压力测试
pub fn test_memory_pressure(config: StressTestConfig) -> StressTestResult {
    let tiered_config = TieredCompilerConfig::default();
    let mut compiler = TieredJITCompiler::new(tiered_config);

    let start_time = Instant::now();
    let duration = Duration::from_secs(config.duration_secs);

    let mut total_executions: u64 = 0;
    let mut total_time_ns: u64 = 0;
    let mut min_time_ns: u64 = u64::MAX;
    let mut max_time_ns: u64 = 0;
    let mut error_count: u64 = 0;

    let block_size = config.block_size;
    let cache = Arc::new(Mutex::new(LRUCache::new(config.cache_size)));

    while start_time.elapsed() < duration {
        for i in 0..config.block_count {
            let block = create_test_block(i as u64 * 0x1000, block_size);
            let pc = block.start_pc;

            if !cache.lock().unwrap().contains(pc) {
                let exec_start = Instant::now();

                match compiler.execute(&block) {
                    Ok(_) => {
                        let elapsed = exec_start.elapsed().as_nanos() as u64;
                        total_executions += 1;
                        total_time_ns += elapsed;
                        min_time_ns = min_time_ns.min(elapsed);
                        max_time_ns = max_time_ns.max(elapsed);

                        cache.lock().unwrap().insert(pc, vec![0x90; 100]);
                    }
                    Err(_) => {
                        error_count += 1;
                    }
                }
            } else {
                total_executions += 1;
                let _ = cache.lock().unwrap().get(pc);
            }
        }
    }

    let cache_stats = cache.lock().unwrap().stats();
    let cache_hit_rate = cache_stats.hit_rate();
    let current_size = cache_stats.current_size;

    StressTestResult {
        total_executions,
        avg_execution_time_ns: if total_executions > 0 {
            total_time_ns / total_executions
        } else {
            0
        },
        min_execution_time_ns: if min_time_ns == u64::MAX {
            0
        } else {
            min_time_ns
        },
        max_execution_time_ns,
        success_rate: if total_executions + error_count > 0 {
            total_executions as f64 / (total_executions + error_count) as f64
        } else {
            0.0
        },
        error_count,
        cache_hit_rate,
        memory_usage_bytes: current_size,
    }
}

/// 内联缓存压力测试
pub fn test_inline_cache_stress(config: StressTestConfig) -> StressTestResult {
    let cache = Arc::new(Mutex::new(InlineCache::default()));

    let start_time = Instant::now();
    let duration = Duration::from_secs(config.duration_secs);

    let mut total_executions: u64 = 0;
    let mut total_time_ns: u64 = 0;
    let mut min_time_ns: u64 = u64::MAX;
    let mut max_time_ns: u64 = 0;

    while start_time.elapsed() < duration {
        for i in 0..config.block_count {
            let call_site = GuestAddr(0x2000 + i as u64 * 0x10);
            let receiver = (i % 100) as u64;
            let code_ptr = GuestAddr(0x3000 + i as u64 * 0x10);

            let lookup_start = Instant::now();

            match cache.lock().unwrap().lookup(call_site, receiver) {
                Some(_) => {
                    let elapsed = lookup_start.elapsed().as_nanos() as u64;
                    total_executions += 1;
                    total_time_ns += elapsed;
                    min_time_ns = min_time_ns.min(elapsed);
                    max_time_ns = max_time_ns.max(elapsed);
                }
                None => {
                    cache.lock().unwrap().update(call_site, receiver, code_ptr);
                }
            }
        }
    }

    let cache_stats = cache
        .lock()
        .unwrap()
        .stats()
        .expect("Failed to get cache stats");
    let cache_hit_rate = cache_stats.hit_rate();

    StressTestResult {
        total_executions,
        avg_execution_time_ns: if total_executions > 0 {
            total_time_ns / total_executions
        } else {
            0
        },
        min_execution_time_ns: if min_time_ns == u64::MAX {
            0
        } else {
            min_time_ns
        },
        max_execution_time_ns,
        success_rate: 1.0,
        error_count: 0,
        cache_hit_rate,
        memory_usage_bytes: cache
            .lock()
            .unwrap()
            .size()
            .expect("Failed to get cache size"),
    }
}

/// 运行所有压力测试
pub fn run_all_stress_tests(config: StressTestConfig) -> Vec<(&'static str, StressTestResult)> {
    vec![
        (
            "Tiered Compiler Stress",
            test_tiered_compiler_stress(config.clone()),
        ),
        (
            "Concurrent Execution Stress",
            test_concurrent_execution(config.clone()),
        ),
        (
            "Memory Pressure Stress",
            test_memory_pressure(config.clone()),
        ),
        ("Inline Cache Stress", test_inline_cache_stress(config)),
    ]
}

/// 创建测试 IR 块
fn create_test_block(start_pc: u64, size: usize) -> IRBlock {
    let mut ops = Vec::with_capacity(size);
    for i in 0..size {
        ops.push(IROp::MovImm {
            dst: (i % 32) as u8,
            imm: (i as u64) % 1000,
        });
        ops.push(IROp::Add {
            dst: (i % 32) as u8,
            src1: (i % 32) as u8,
            src2: ((i + 1) % 32) as u8,
        });
    }
    IRBlock {
        start_pc: GuestAddr(start_pc),
        ops,
        term: Terminator::Ret,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tiered_compiler_basic_stress() {
        let config = StressTestConfig {
            duration_secs: 5,
            thread_count: 1,
            block_size: 50,
            block_count: 100,
            ..Default::default()
        };

        let result = test_tiered_compiler_stress(config);

        assert!(result.total_executions > 0);
        assert!(result.success_rate > 0.99);
        assert!(result.error_count == 0);
    }

    #[test]
    fn test_concurrent_execution_basic_stress() {
        let config = StressTestConfig {
            duration_secs: 5,
            thread_count: 2,
            block_size: 50,
            block_count: 100,
            ..Default::default()
        };

        let result = test_concurrent_execution(config);

        assert!(result.total_executions > 0);
        assert!(result.success_rate > 0.99);
    }

    #[test]
    fn test_inline_cache_basic_stress() {
        let config = StressTestConfig {
            duration_secs: 5,
            block_size: 10,
            block_count: 100,
            ..Default::default()
        };

        let result = test_inline_cache_stress(config);

        assert!(result.total_executions > 0);
        assert!(result.cache_hit_rate > 0.5);
    }
}
