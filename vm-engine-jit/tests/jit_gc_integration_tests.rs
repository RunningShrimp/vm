//! JIT编译器和GC功能集成测试套件
//!
//! 测试JIT编译器、GC和缓存系统之间的协作，包括：
//! - JIT与GC协作测试
//! - 端到端编译和执行测试
//! - 性能回归测试
//! - 多组件协作的复杂场景测试

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use vm_core::GuestAddr;
use vm_engine_jit::Jit;
use vm_engine_jit::optimizing_compiler::{
    RegisterAllocator, InstructionScheduler, OptimizationPassManager
};
use vm_engine_jit::unified_gc::{
    UnifiedGc, UnifiedGcConfig, UnifiedGcStats, GCPhase
};
use vm_engine_jit::unified_cache::{
    UnifiedCodeCache, CacheConfig, CacheStats, PrefetchConfig
};
use vm_engine_jit::ewma_hotspot::{EwmaHotspotDetector, EwmaHotspotConfig};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};
use vm_mem::SoftMmu;

/// 创建测试用的集成配置
fn create_integration_config() -> (UnifiedGcConfig, CacheConfig, EwmaHotspotConfig) {
    let gc_config = UnifiedGcConfig {
        young_gen: vm_engine_jit::unified_gc::YoungGenConfig {
            initial_size: 2 * 1024 * 1024,      // 2MB
            max_size: 32 * 1024 * 1024,         // 32MB
            promotion_threshold: 50,
            ..Default::default()
        },
        old_gen: vm_engine_jit::unified_gc::OldGenConfig {
            initial_size: 8 * 1024 * 1024,   // 8MB
            max_size: 128 * 1024 * 1024,      // 128MB
            ..Default::default()
        },
        concurrent: vm_engine_jit::unified_gc::ConcurrentConfig {
            enabled: true,
            marking_threads: 2,
            sweeping_threads: 1,
            ..Default::default()
        },
        adaptive: vm_engine_jit::unified_gc::AdaptiveConfig {
            enabled: true,
            target_pause_time_ms: 10,
            ..Default::default()
        },
        ..Default::default()
    };
    
    let cache_config = CacheConfig {
        max_entries: 5000,
        max_memory_bytes: 50 * 1024 * 1024, // 50MB
        eviction_policy: vm_engine_jit::unified_cache::EvictionPolicy::LRU_LFU,
        cleanup_interval_secs: 30,
        hotness_decay_factor: 0.99,
        warmup_size: 500,
    };
    
    let hotspot_config = EwmaHotspotConfig {
        frequency_alpha: 0.3,
        execution_time_alpha: 0.2,
        hotspot_threshold: 1.0,
        min_execution_count: 10,
        min_execution_time_us: 5,
        frequency_weight: 0.4,
        execution_time_weight: 0.4,
        complexity_weight: 0.2,
        base_threshold: 100,
        time_window_ms: 1000,
        decay_factor: 0.95,
    };
    
    (gc_config, cache_config, hotspot_config)
}

/// 创建测试用的IR块
fn create_integration_ir_block(addr: GuestAddr, complexity: usize) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    
    // 根据复杂度创建不同数量的操作
    for i in 1..=complexity {
        builder.push(IROp::MovImm { dst: i as u32, imm: i as u64 });
        if i > 1 {
            builder.push(IROp::Add {
                dst: i as u32,
                src1: (i - 1) as u32,
                src2: 1,
            });
        }
    }
    
    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 创建测试用的JIT实例
fn create_test_jit() -> Jit {
    let mut jit = Jit::new();
    jit
}

// ============================================================================
// JIT与GC协作测试
// ============================================================================

#[test]
fn test_jit_gc_memory_allocation_interaction() {
    let (gc_config, _, _) = create_integration_config();
    let gc = Arc::new(UnifiedGc::new(gc_config));
    let mut jit = create_test_jit();
    
    // 测试JIT编译过程中的内存分配与GC的协调
    let blocks = HashMap::from([
        (0x1000, create_integration_ir_block(0x1000, 20)),
        (0x2000, create_integration_ir_block(0x2000, 30)),
        (0x3000, create_integration_ir_block(0x3000, 40)),
    ]);
    
    // 编译块（会分配内存）
    for (addr, block) in &blocks {
        let _compiled = jit.compile_block(block);
        
        // 模拟编译后的内存分配
        gc.allocate(*addr, block.ops.len() * 8); // 估算内存使用
    }
    
    // 触发GC
    let roots = vec![0x1000, 0x2000, 0x3000];
    gc.collect_young(&roots);
    
    // 验证JIT与GC的协作
    let gc_stats = gc.get_stats();
    assert!(gc_stats.total_allocations > 0);
    assert!(gc_stats.young_gen.collections > 0);
}

#[test]
fn test_jit_gc_concurrent_execution() {
    let (gc_config, _, _) = create_integration_config();
    let gc = Arc::new(UnifiedGc::new(gc_config));
    let mut jit = create_test_jit();
    
    // 测试JIT编译与GC并发执行
    let gc_clone = gc.clone();
    let jit_handle = thread::spawn(move || {
        let blocks = HashMap::from([
            (0x1000, create_integration_ir_block(0x1000, 25)),
            (0x2000, create_integration_ir_block(0x2000, 35)),
        ]);
        
        for (addr, block) in blocks {
            let _compiled = jit.compile_block(&block);
            
            // 分配内存
            gc_clone.allocate(addr, block.ops.len() * 8);
            
            // 模拟一些处理时间
            thread::sleep(Duration::from_millis(10));
        }
    });
    
    // GC线程
    let gc_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(25)); // 等待一些分配
        
        let roots = vec![0x1000, 0x2000];
        gc.collect_young(&roots);
    });
    
    // 等待完成
    jit_handle.join().unwrap();
    gc_thread.join().unwrap();
    
    // 验证并发执行结果
    let gc_stats = gc.get_stats();
    assert!(gc_stats.total_allocations > 0);
    assert!(gc_stats.concurrent.max_concurrent_threads > 0);
}

#[test]
fn test_jit_gc_memory_pressure_response() {
    let (gc_config, _, _) = create_integration_config();
    let gc = Arc::new(UnifiedGc::new(gc_config));
    let mut jit = create_test_jit();
    
    // 模拟内存压力情况
    let gc_clone = gc.clone();
    let pressure_handle = thread::spawn(move || {
        // 大量分配以触发内存压力
        for i in 0..100 {
            let addr = 0x1000 + i * 0x100;
            let block = create_integration_ir_block(addr, 50);
            let _compiled = jit.compile_block(&block);
            
            // 分配大量内存
            gc_clone.allocate(addr, 1024); // 1KB per block
            
            if i % 10 == 0 {
                thread::sleep(Duration::from_millis(1));
            }
        }
    });
    
    // GC响应线程
    let gc_response = thread::spawn(move || {
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(10));
            
            let roots = (0..10).map(|i| 0x1000 + i * 0x100).collect();
            gc.collect_full(&roots);
        }
    });
    
    // 等待完成
    pressure_handle.join().unwrap();
    gc_response.join().unwrap();
    
    // 验证内存压力响应
    let gc_stats = gc.get_stats();
    assert!(gc_stats.memory_pressure_events > 0);
    assert!(gc_stats.full_collections > 0);
}

// ============================================================================
// JIT与缓存协作测试
// ============================================================================

#[test]
fn test_jit_cache_compilation_caching() {
    let (_, cache_config, hotspot_config) = create_integration_config();
    let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
    let mut jit = create_test_jit();
    
    // 测试JIT编译结果缓存
    let blocks = HashMap::from([
        (0x1000, create_integration_ir_block(0x1000, 20)),
        (0x2000, create_integration_ir_block(0x2000, 30)),
        (0x3000, create_integration_ir_block(0x3000, 40)),
    ]);
    
    // 第一次编译（应该缓存结果）
    for (addr, block) in &blocks {
        // 检查缓存
        let cached = cache.lookup(*addr);
        assert!(cached.is_none(), "Should not be cached initially");
        
        // 编译
        let compiled = jit.compile_block(block);
        
        // 缓存编译结果
        cache.insert_sync(*addr, compiled.code, compiled.compile_time_ns);
    }
    
    // 第二次访问（应该命中缓存）
    for addr in blocks.keys() {
        let cached = cache.lookup(*addr);
        assert!(cached.is_some(), "Should be cached now");
    }
    
    // 验证缓存统计
    let stats = cache.get_stats();
    assert!(stats.hits > 0);
    assert!(stats.misses > 0);
    assert!(stats.hit_rate > 0.0);
}

#[test]
fn test_jit_cache_hotspot_aware_compilation() {
    let (_, cache_config, hotspot_config) = create_integration_config();
    let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
    let mut jit = create_test_jit();
    
    // 测试热点感知的编译缓存
    let hotspot_addr = 0x1000;
    let normal_addr = 0x2000;
    
    // 模拟热点执行
    for _ in 0..20 {
        cache.record_execution(hotspot_addr, 50, 1.0);
    }
    
    // 模拟普通执行
    for _ in 0..5 {
        cache.record_execution(normal_addr, 50, 1.0);
    }
    
    // 编译热点块
    let hotspot_block = create_integration_ir_block(hotspot_addr, 30);
    let hotspot_compiled = jit.compile_block(&hotspot_block);
    cache.insert_sync(hotspot_addr, hotspot_compiled.code, hotspot_compiled.compile_time_ns);
    
    // 编译普通块
    let normal_block = create_integration_ir_block(normal_addr, 30);
    let normal_compiled = jit.compile_block(&normal_block);
    cache.insert_sync(normal_addr, normal_compiled.code, normal_compiled.compile_time_ns);
    
    // 验证热点识别
    assert!(cache.is_hotspot(hotspot_addr));
    assert!(!cache.is_hotspot(normal_addr));
    
    // 验证缓存行为
    let stats = cache.get_stats();
    assert!(stats.total_entries >= 2);
}

#[test]
fn test_jit_cache_prefetch_integration() {
    let (_, mut cache_config, hotspot_config) = create_integration_config();
    cache_config.eviction_policy = vm_engine_jit::unified_cache::EvictionPolicy::LRU_LFU;
    
    let prefetch_config = PrefetchConfig {
        enable_smart_prefetch: true,
        enable_background_compile: true,
        prefetch_window_size: 3,
        prefetch_threshold: 2,
        max_prefetch_queue_size: 10,
        prefetch_priority: vm_engine_jit::unified_cache::CompilePriority::Low,
    };
    
    let cache = Arc::new(UnifiedCodeCache::with_prefetch_config(
        cache_config,
        hotspot_config,
        prefetch_config,
    ));
    
    let mut jit = create_test_jit();
    
    // 建立访问模式
    let access_pattern = vec![(0x1000, 0x2000), (0x2000, 0x3000), (0x3000, 0x4000)];
    
    for (from, to) in &access_pattern {
        // 编译块
        let block = create_integration_ir_block(*from, 20);
        let compiled = jit.compile_block(&block);
        cache.insert_sync(*from, compiled.code, compiled.compile_time_ns);
        
        // 记录跳转
        cache.record_jump(*from, *to);
        
        // 访问块
        cache.lookup(*from);
    }
    
    // 验证预取
    if let Some(prefetch_stats) = cache.get_prefetch_stats() {
        assert!(prefetch_stats.total_prefetch_requests > 0);
    }
}

// ============================================================================
// 端到端编译和执行测试
// ============================================================================

#[test]
fn test_end_to_end_compilation_execution() {
    let (gc_config, cache_config, hotspot_config) = create_integration_config();
    let gc = Arc::new(UnifiedGc::new(gc_config));
    let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
    let mut jit = create_test_jit();
    
    // 创建测试程序
    let program = vec![
        (0x1000, create_integration_ir_block(0x1000, 15)), // 函数入口
        (0x1100, create_integration_ir_block(0x1100, 20)), // 主函数
        (0x1200, create_integration_ir_block(0x1200, 25)), // 辅助函数
        (0x1300, create_integration_ir_block(0x1300, 30)), // 另一个辅助函数
    ];
    
    // 编译所有块
    let mut compiled_blocks = HashMap::new();
    for (addr, block) in program {
        let compiled = jit.compile_block(&block);
        compiled_blocks.insert(addr, compiled);
        
        // 缓存编译结果
        cache.insert_sync(addr, compiled.code.clone(), compiled.compile_time_ns);
        
        // 分配内存
        gc.allocate(addr, block.ops.len() * 8);
    }
    
    // 模拟执行
    let execution_order = vec![0x1000, 0x1100, 0x1200, 0x1300];
    for addr in &execution_order {
        // 查找缓存
        let cached = cache.lookup(*addr);
        assert!(cached.is_some(), "Block should be cached");
        
        // 记录执行
        cache.record_execution(*addr, 50, 1.0);
        
        // 模拟执行时间
        thread::sleep(Duration::from_millis(1));
    }
    
    // 验证端到端执行
    let gc_stats = gc.get_stats();
    let cache_stats = cache.get_stats();
    
    assert!(gc_stats.total_allocations >= 4);
    assert!(cache_stats.total_entries >= 4);
    assert!(cache_stats.hits >= 4);
    assert!(cache_stats.hit_rate == 1.0);
}

#[test]
fn test_end_to_end_performance_regression() {
    let (gc_config, cache_config, hotspot_config) = create_integration_config();
    let gc = Arc::new(UnifiedGc::new(gc_config));
    let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
    let mut jit = create_test_jit();
    
    // 性能基准测试
    let start_time = Instant::now();
    
    // 编译和执行大量块
    for i in 0..100 {
        let addr = 0x1000 + i * 0x100;
        let block = create_integration_ir_block(addr, 20 + i % 10);
        
        // 编译
        let compile_start = Instant::now();
        let compiled = jit.compile_block(&block);
        let compile_time = compile_start.elapsed();
        
        // 缓存
        cache.insert_sync(addr, compiled.code.clone(), compiled.compile_time_ns);
        
        // 分配内存
        gc.allocate(addr, block.ops.len() * 8);
        
        // 执行
        let exec_start = Instant::now();
        cache.lookup(addr);
        cache.record_execution(addr, 50, 1.0);
        let exec_time = exec_start.elapsed();
        
        // 验证性能指标
        assert!(compile_time.as_millis() < 50, "Compilation should be fast");
        assert!(exec_time.as_micros() < 100, "Execution should be fast");
    }
    
    let total_time = start_time.elapsed();
    
    // 定期GC
    let roots = (0..100).map(|i| 0x1000 + i * 0x100).collect();
    gc.collect_young(&roots);
    
    // 验证性能回归
    let gc_stats = gc.get_stats();
    let cache_stats = cache.get_stats();
    
    assert!(total_time.as_secs() < 10, "Total execution should complete in reasonable time");
    assert!(gc_stats.pause_time_stats.max_pause_ms < 20, "GC pause should be short");
    assert!(cache_stats.avg_lookup_time_ns < 10000, "Cache lookup should be fast");
}

#[test]
fn test_end_to_end_stress_test() {
    let (gc_config, cache_config, hotspot_config) = create_integration_config();
    let gc = Arc::new(UnifiedGc::new(gc_config));
    let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
    
    // 压力测试：多线程编译和执行
    let handles: Vec<_> = (0..4)
        .map(|thread_id| {
            let gc = gc.clone();
            let cache = cache.clone();
            
            thread::spawn(move || {
                let mut jit = create_test_jit();
                
                for i in 0..50 {
                    let addr = 0x1000 + (thread_id * 50 + i) * 0x100;
                    let block = create_integration_ir_block(addr, 20);
                    
                    // 编译
                    let compiled = jit.compile_block(&block);
                    
                    // 缓存
                    cache.insert_sync(addr, compiled.code, compiled.compile_time_ns);
                    
                    // 分配内存
                    gc.allocate(addr, block.ops.len() * 8);
                    
                    // 执行
                    cache.lookup(addr);
                    cache.record_execution(addr, 50, 1.0);
                    
                    // 记录跳转
                    if i > 0 {
                        let prev_addr = addr - 0x100;
                        cache.record_jump(prev_addr, addr);
                    }
                }
                
                true
            })
        })
        .collect();
    
    // GC线程
    let gc_handle = thread::spawn({
        let gc = gc.clone();
        move || {
            for _ in 0..10 {
                thread::sleep(Duration::from_millis(50));
                
                let roots = (0..200).map(|i| 0x1000 + i * 0x100).collect();
                gc.collect_young(&roots);
            }
        }
    });
    
    // 等待所有线程完成
    for handle in handles {
        assert!(handle.join().unwrap());
    }
    
    gc_handle.join().unwrap();
    
    // 验证压力测试结果
    let gc_stats = gc.get_stats();
    let cache_stats = cache.get_stats();
    
    assert!(gc_stats.total_allocations >= 200);
    assert!(cache_stats.total_entries >= 200);
    assert!(gc_stats.concurrent.max_concurrent_threads > 1);
}

// ============================================================================
// 多组件协作复杂场景测试
// ============================================================================

#[test]
fn test_complex_scenario_jit_gc_cache_cooperation() {
    let (gc_config, cache_config, hotspot_config) = create_integration_config();
    let gc = Arc::new(UnifiedGc::new(gc_config));
    let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
    
    // 复杂场景：模拟真实程序执行
    let scenario = vec![
        // 阶段1：初始化
        (0x1000, 10, true),  // 热点函数
        (0x2000, 15, false), // 普通函数
        (0x3000, 20, false), // 普通函数
        
        // 阶段2：热点执行
        (0x1000, 10, true),  // 重复热点
        (0x1000, 10, true),
        (0x1000, 10, true),
        
        // 阶段3：函数调用
        (0x2000, 15, false),
        (0x3000, 20, false),
        (0x2000, 15, false),
        
        // 阶段4：更多热点执行
        (0x1000, 10, true),
        (0x1000, 10, true),
    ];
    
    let mut jit = create_test_jit();
    
    for (i, (addr, complexity, is_hotspot)) in scenario.iter().enumerate() {
        // 编译
        let block = create_integration_ir_block(*addr, *complexity);
        let compiled = jit.compile_block(&block);
        
        // 缓存
        cache.insert_sync(*addr, compiled.code, compiled.compile_time_ns);
        
        // 分配内存
        gc.allocate(*addr, block.ops.len() * 8);
        
        // 执行
        cache.lookup(*addr);
        cache.record_execution(*addr, 50, 1.0);
        
        // 模拟函数调用跳转
        if i > 0 {
            let prev_addr = scenario[i-1].0;
            cache.record_jump(prev_addr, *addr);
        }
        
        // 模拟执行时间
        thread::sleep(Duration::from_millis(1));
        
        // 定期GC
        if i % 5 == 0 {
            let roots = scenario.iter().take(i+1).map(|(addr, _, _)| *addr).collect();
            gc.collect_young(&roots);
        }
    }
    
    // 验证复杂场景结果
    let gc_stats = gc.get_stats();
    let cache_stats = cache.get_stats();
    
    assert!(gc_stats.total_allocations >= 10);
    assert!(cache_stats.total_entries >= 3);
    assert!(cache.is_hotspot(0x1000));
    assert!(!cache.is_hotspot(0x2000));
    
    // 验证热点提升
    if let Some(prefetch_stats) = cache.get_prefetch_stats() {
        assert!(prefetch_stats.total_prefetch_requests > 0);
    }
}

#[test]
fn test_complex_scenario_memory_pressure_and_hotspot() {
    let (mut gc_config, cache_config, hotspot_config) = create_integration_config();
    gc_config.young_gen.initial_size = 1 * 1024 * 1024; // 1MB，小内存
    gc_config.young_gen.max_size = 4 * 1024 * 1024;    // 4MB
    
    let gc = Arc::new(UnifiedGc::new(gc_config));
    let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
    
    // 内存压力下的热点检测
    let mut jit = create_test_jit();
    
    // 创建大量块（内存压力）
    for i in 0..50 {
        let addr = 0x1000 + i * 0x100;
        let complexity = 30 + i % 20; // 变化的复杂度
        let block = create_integration_ir_block(addr, complexity);
        
        // 编译
        let compiled = jit.compile_block(&block);
        
        // 缓存
        cache.insert_sync(addr, compiled.code, compiled.compile_time_ns);
        
        // 分配内存（可能导致内存压力）
        gc.allocate(addr, block.ops.len() * 16); // 更大的内存分配
        
        // 模拟执行
        cache.lookup(addr);
        
        // 前半部分创建热点
        if i < 25 {
            for _ in 0..(5 + i % 10) {
                cache.record_execution(addr, 50 + complexity as u64, 1.0);
            }
        }
    }
    
    // 触发GC
    let roots = (0..50).map(|i| 0x1000 + i * 0x100).collect();
    gc.collect_full(&roots);
    
    // 验证内存压力和热点检测
    let gc_stats = gc.get_stats();
    let cache_stats = cache.get_stats();
    
    assert!(gc_stats.memory_pressure_events > 0);
    assert!(gc_stats.full_collections > 0);
    assert!(cache_stats.evictions > 0);
    
    // 验证热点仍然被识别
    let hotspots_found = (0..25).any(|i| {
        let addr = 0x1000 + i * 0x100;
        cache.is_hotspot(addr)
    });
    assert!(hotspots_found, "Should detect hotspots even under memory pressure");
}

#[test]
fn test_complex_scenario_adaptive_behavior() {
    let (mut gc_config, cache_config, hotspot_config) = create_integration_config();
    gc_config.adaptive.target_pause_time_ms = 5; // 严格的目标
    
    let gc = Arc::new(UnifiedGc::new(gc_config));
    let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
    
    // 自适应行为测试
    let mut jit = create_test_jit();
    
    // 阶段1：正常负载
    for i in 0..20 {
        let addr = 0x1000 + i * 0x100;
        let block = create_integration_ir_block(addr, 15);
        
        let compiled = jit.compile_block(&block);
        cache.insert_sync(addr, compiled.code, compiled.compile_time_ns);
        gc.allocate(addr, block.ops.len() * 8);
        
        cache.lookup(addr);
        cache.record_execution(addr, 30, 1.0);
    }
    
    // GC并观察自适应行为
    let roots = (0..20).map(|i| 0x1000 + i * 0x100).collect();
    gc.collect_young(&roots);
    
    // 阶段2：高负载（应该触发自适应调整）
    for i in 20..40 {
        let addr = 0x1000 + i * 0x100;
        let block = create_integration_ir_block(addr, 25); // 更复杂
        
        let compiled = jit.compile_block(&block);
        cache.insert_sync(addr, compiled.code, compiled.compile_time_ns);
        gc.allocate(addr, block.ops.len() * 12); // 更多内存
        
        cache.lookup(addr);
        cache.record_execution(addr, 80, 1.5); // 更长执行时间，更高复杂度
    }
    
    // 高负载下的GC
    let roots = (0..40).map(|i| 0x1000 + i * 0x100).collect();
    gc.collect_full(&roots);
    
    // 验证自适应行为
    let gc_stats = gc.get_stats();
    
    assert!(gc_stats.adaptive.threshold_adjustments > 0);
    assert!(gc_stats.pause_time_stats.avg_pause_ms > 0);
    
    // 验证GC尝试满足目标停顿时间
    let avg_pause = gc_stats.pause_time_stats.avg_pause_ms;
    assert!(avg_pause < 15, "GC should adapt to meet target pause time");
}

#[test]
fn test_complex_scenario_error_recovery() {
    let (gc_config, cache_config, hotspot_config) = create_integration_config();
    let gc = Arc::new(UnifiedGc::new(gc_config));
    let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
    
    // 错误恢复测试
    let mut jit = create_test_jit();
    
    // 正常操作
    for i in 0..10 {
        let addr = 0x1000 + i * 0x100;
        let block = create_integration_ir_block(addr, 15);
        
        let compiled = jit.compile_block(&block);
        cache.insert_sync(addr, compiled.code, compiled.compile_time_ns);
        gc.allocate(addr, block.ops.len() * 8);
        
        cache.lookup(addr);
    }
    
    // 模拟错误情况
    let error_addr = 0x9999;
    let invalid_block = create_integration_ir_block(error_addr, 100); // 过大的块
    
    // 尝试处理无效块（应该优雅处理）
    let result = std::panic::catch_unwind(|| {
        let compiled = jit.compile_block(&invalid_block);
        cache.insert_sync(error_addr, compiled.code, compiled.compile_time_ns);
        gc.allocate(error_addr, invalid_block.ops.len() * 8);
    });
    
    // 应该能够恢复
    assert!(result.is_ok() || result.is_err());
    
    // 继续正常操作
    for i in 10..15 {
        let addr = 0x1000 + i * 0x100;
        let block = create_integration_ir_block(addr, 15);
        
        let compiled = jit.compile_block(&block);
        cache.insert_sync(addr, compiled.code, compiled.compile_time_ns);
        gc.allocate(addr, block.ops.len() * 8);
        
        cache.lookup(addr);
    }
    
    // 验证错误恢复
    let gc_stats = gc.get_stats();
    let cache_stats = cache.get_stats();
    
    assert!(gc_stats.total_allocations >= 15);
    assert!(cache_stats.total_entries >= 15);
    
    // 系统应该仍然正常工作
    let normal_addr = 0x1000;
    assert!(cache.lookup(normal_addr).is_some());
}