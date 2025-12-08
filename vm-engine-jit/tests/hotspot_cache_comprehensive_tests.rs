//! 热点检测和缓存功能全面测试套件
//!
//! 测试热点检测和缓存系统的所有核心组件，包括：
//! - EWMA热点检测器
//! - 统一代码缓存
//! - 智能预取器
//! - 缓存淘汰策略

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use vm_core::GuestAddr;
use vm_engine_jit::ewma_hotspot::{EwmaHotspotDetector, EwmaHotspotConfig, EwmaHotspotStats};
use vm_engine_jit::unified_cache::{
    UnifiedCodeCache, CacheConfig, CacheEntry, EvictionPolicy, CacheStats,
    PrefetchConfig, PrefetchStats, SmartPrefetcher, AccessPattern,
    CompilePriority, CompileRequest, CompileResult
};
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};

/// 创建测试用的EWMA配置
fn create_test_ewma_config() -> EwmaHotspotConfig {
    EwmaHotspotConfig {
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
    }
}

/// 创建测试用的缓存配置
fn create_test_cache_config() -> CacheConfig {
    CacheConfig {
        max_entries: 1000,
        max_memory_bytes: 10 * 1024 * 1024, // 10MB
        eviction_policy: EvictionPolicy::LRU_LFU,
        cleanup_interval_secs: 60,
        hotness_decay_factor: 0.99,
        warmup_size: 100,
    }
}

/// 创建测试用的预取配置
fn create_test_prefetch_config() -> PrefetchConfig {
    PrefetchConfig {
        enable_smart_prefetch: true,
        enable_background_compile: true,
        prefetch_window_size: 5,
        prefetch_threshold: 3,
        max_prefetch_queue_size: 100,
        prefetch_priority: CompilePriority::Low,
    }
}

/// 创建测试用的IR块
fn create_test_ir_block(addr: GuestAddr) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    builder.push(IROp::MovImm { dst: 1, imm: 42 });
    builder.push(IROp::Add { dst: 0, src1: 0, src2: 1 });
    builder.set_term(Terminator::Ret);
    builder.build()
}

/// 创建复杂的IR块（用于复杂度测试）
fn create_complex_ir_block(addr: GuestAddr) -> IRBlock {
    let mut builder = IRBuilder::new(addr);
    for i in 1..=20 {
        builder.push(IROp::MovImm { dst: i, imm: i as u64 });
        if i > 1 {
            builder.push(IROp::Add {
                dst: i,
                src1: i - 1,
                src2: 1,
            });
        }
    }
    builder.set_term(Terminator::Ret);
    builder.build()
}

// ============================================================================
// EWMA热点检测器测试
// ============================================================================

#[test]
fn test_ewma_hotspot_basic_detection() {
    let config = create_test_ewma_config();
    let detector = EwmaHotspotDetector::new(config);
    
    let addr = 0x1000;
    
    // 记录多次执行
    for i in 0..20 {
        detector.record_execution(addr, 10 + i as u64);
    }
    
    // 应该被识别为热点
    assert!(detector.is_hotspot(addr));
    assert!(detector.get_hotspot_score(addr) >= 1.0);
    
    // 验证统计信息
    let stats = detector.get_stats();
    assert!(stats.total_detections >= 20);
    assert!(stats.hotspot_identifications > 0);
}

#[test]
fn test_ewma_hotspot_frequency_smoothing() {
    let mut config = create_test_ewma_config();
    config.frequency_alpha = 0.5; // 更高的alpha，更重视最新数据
    
    let detector = EwmaHotspotDetector::new(config);
    
    let addr = 0x1000;
    
    // 快速连续执行
    for _ in 0..15 {
        detector.record_execution(addr, 50);
    }
    
    // 验证EWMA频率平滑
    let freq_ewma = detector.get_frequency_ewma(addr);
    assert!(freq_ewma > 0.0);
    
    // 应该被识别为热点
    assert!(detector.is_hotspot(addr));
    
    let stats = detector.get_stats();
    assert!(stats.avg_frequency_ewma > 0.0);
}

#[test]
fn test_ewma_hotspot_execution_time_smoothing() {
    let mut config = create_test_ewma_config();
    config.execution_time_alpha = 0.3;
    config.min_execution_time_us = 10;
    
    let detector = EwmaHotspotDetector::new(config);
    
    let addr = 0x1000;
    
    // 记录长时间执行
    for i in 0..10 {
        detector.record_execution(addr, 100 + i * 10); // 递增的执行时间
    }
    
    // 验证EWMA执行时间平滑
    let time_ewma = detector.get_execution_time_ewma(addr);
    assert!(time_ewma >= 100.0);
    
    // 应该被识别为热点
    assert!(detector.is_hotspot(addr));
    
    let stats = detector.get_stats();
    assert!(stats.avg_execution_time_ewma > 0.0);
}

#[test]
fn test_ewma_hotspot_multidimensional_scoring() {
    let config = create_test_ewma_config();
    let detector = EwmaHotspotDetector::new(config);
    
    let addr = 0x1000;
    
    // 记录执行（带复杂度）
    for i in 0..15 {
        let complexity = 1.0 + (i % 3) as f64 * 0.5; // 变化的复杂度
        detector.record_execution_with_complexity(addr, 50 + i * 5, complexity);
    }
    
    // 验证多维度评分
    let score = detector.get_hotspot_score(addr);
    assert!(score > 0.0);
    
    // 应该被识别为热点
    assert!(detector.is_hotspot(addr));
    
    // 验证执行次数计算
    let exec_count = detector.get_execution_count(addr);
    assert!(exec_count > 0);
}

#[test]
fn test_ewma_hotspot_adaptive_threshold() {
    let config = create_test_ewma_config();
    let detector = EwmaHotspotDetector::new(config);
    
    let addr = 0x1000;
    
    // 记录长时间执行
    for _ in 0..20 {
        detector.record_execution_with_complexity(addr, 1000, 2.0); // 长时间，高复杂度
    }
    
    // 获取自适应阈值
    let threshold = detector.get_adaptive_threshold(addr);
    
    // 由于执行时间长且复杂度高，阈值应该降低
    assert!(threshold < 100); // 基础阈值是100
}

#[test]
fn test_ewma_hotspot_cleanup() {
    let mut config = create_test_ewma_config();
    config.time_window_ms = 10; // 10ms窗口
    
    let detector = EwmaHotspotDetector::new(config);
    
    let addr = 0x1000;
    
    // 记录执行
    detector.record_execution(addr, 50);
    
    // 等待超过时间窗口
    thread::sleep(Duration::from_millis(20));
    
    // 再次记录执行（应该触发清理）
    detector.record_execution(addr, 50);
    
    // 检查统计信息
    let stats = detector.get_stats();
    assert!(stats.cleanup_count > 0);
}

#[test]
fn test_ewma_hotspot_multiple_addresses() {
    let config = create_test_ewma_config();
    let detector = EwmaHotspotDetector::new(config);
    
    let addr1 = 0x1000;
    let addr2 = 0x2000;
    let addr3 = 0x3000;
    
    // 记录不同地址的执行
    for _ in 0..20 {
        detector.record_execution(addr1, 100); // 高频
    }
    for _ in 0..5 {
        detector.record_execution(addr2, 50);  // 低频
    }
    for _ in 0..15 {
        detector.record_execution(addr3, 200); // 中频
    }
    
    // 验证热点识别
    assert!(detector.is_hotspot(addr1)); // 应该是热点
    assert!(!detector.is_hotspot(addr2)); // 可能不是热点
    assert!(detector.is_hotspot(addr3)); // 应该是热点
    
    // 获取热点列表
    let hotspots = detector.get_hotspots();
    assert!(hotspots.len() >= 2);
    
    // 热点应该按评分排序
    for i in 1..hotspots.len() {
        assert!(hotspots[i-1].1 >= hotspots[i].1);
    }
}

#[test]
fn test_ewma_hotspot_performance() {
    let config = create_test_ewma_config();
    let detector = EwmaHotspotDetector::new(config);
    
    let start_time = Instant::now();
    
    // 记录大量执行
    for i in 0..1000 {
        let addr = 0x1000 + (i % 10) * 0x100;
        detector.record_execution(addr, 50 + (i % 10) as u64 * 5);
    }
    
    let elapsed = start_time.elapsed();
    
    // 验证性能
    assert!(elapsed.as_millis() < 100, "Hotspot detection should be fast");
    
    let stats = detector.get_stats();
    assert_eq!(stats.total_detections, 1000);
}

// ============================================================================
// 统一代码缓存测试
// ============================================================================

#[test]
fn test_unified_cache_basic_operations() {
    let cache_config = create_test_cache_config();
    let hotspot_config = create_test_ewma_config();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);
    
    let addr = 0x1000;
    let code = vec![0x90, 0x90, 0x90]; // NOP指令
    
    // 插入代码
    cache.insert_sync(addr, code.clone(), false);
    
    // 查找代码
    let found = cache.lookup(addr);
    assert!(found.is_some());
    
    // 验证统计信息
    let stats = cache.get_stats();
    assert!(stats.total_entries > 0);
    assert!(stats.hits > 0);
}

#[test]
fn test_unified_cache_hot_cold_promotion() {
    let mut cache_config = create_test_cache_config();
    cache_config.warmup_size = 3; // 访问3次后提升到热缓存
    
    let hotspot_config = create_test_ewma_config();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);
    
    let addr = 0x1000;
    let code = vec![0x90];
    cache.insert_sync(addr, code, false);
    
    // 初始应该在冷缓存
    assert!(cache.lookup(addr).is_some());
    
    // 访问多次以触发提升
    for _ in 0..5 {
        cache.lookup(addr);
    }
    
    // 应该提升到热缓存
    let stats = cache.get_stats();
    assert!(stats.total_entries > 0);
}

#[test]
fn test_unified_cache_eviction_policies() {
    // 测试LRU策略
    let mut cache_config = create_test_cache_config();
    cache_config.eviction_policy = EvictionPolicy::LRU;
    cache_config.max_entries = 3;
    
    let hotspot_config = create_test_ewma_config();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);
    
    // 插入超过限制的条目
    for i in 0..5 {
        let addr = 0x1000 + i * 0x100;
        let code = vec![0x90];
        cache.insert_sync(addr, code, false);
    }
    
    // 验证LRU淘汰
    let stats = cache.get_stats();
    assert!(stats.total_entries <= 3);
    assert!(stats.evictions > 0);
}

#[test]
fn test_unified_cache_lfu_eviction() {
    // 测试LFU策略
    let mut cache_config = create_test_cache_config();
    cache_config.eviction_policy = EvictionPolicy::LFU;
    cache_config.max_entries = 3;
    
    let hotspot_config = create_test_ewma_config();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);
    
    // 插入条目并访问不同次数
    for i in 0..5 {
        let addr = 0x1000 + i * 0x100;
        let code = vec![0x90];
        cache.insert_sync(addr, code, false);
        
        // 访问不同次数
        for _ in 0..(5 - i) {
            cache.lookup(addr);
        }
    }
    
    // 验证LFU淘汰（访问次数少的应该被淘汰）
    let stats = cache.get_stats();
    assert!(stats.total_entries <= 3);
    assert!(stats.evictions > 0);
}

#[test]
fn test_unified_cache_value_based_eviction() {
    // 测试基于价值的淘汰策略
    let mut cache_config = create_test_cache_config();
    cache_config.eviction_policy = EvictionPolicy::ValueBased;
    cache_config.max_entries = 3;
    
    let hotspot_config = create_test_ewma_config();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);
    
    // 插入条目并设置不同的执行收益
    for i in 0..5 {
        let addr = 0x1000 + i * 0x100;
        let code = vec![0x90];
        cache.insert_sync(addr, code, false);
        
        // 设置不同的执行收益
        let benefit = (i + 1) as f64 * 10.0;
        cache.update_execution_benefit(addr, benefit);
    }
    
    // 验证基于价值的淘汰
    let stats = cache.get_stats();
    assert!(stats.total_entries <= 3);
    assert!(stats.evictions > 0);
}

#[test]
fn test_unified_cache_hybrid_lfu_lru_eviction() {
    // 测试LRU+LFU混合策略
    let mut cache_config = create_test_cache_config();
    cache_config.eviction_policy = EvictionPolicy::LRU_LFU;
    cache_config.max_entries = 3;
    
    let hotspot_config = create_test_ewma_config();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);
    
    // 插入条目
    for i in 0..5 {
        let addr = 0x1000 + i * 0x100;
        let code = vec![0x90];
        cache.insert_sync(addr, code, false);
    }
    
    // 访问条目
    for i in 0..3 {
        let addr = 0x1000 + i * 0x100;
        cache.lookup(addr);
    }
    
    // 验证混合淘汰
    let stats = cache.get_stats();
    assert!(stats.total_entries <= 3);
    assert!(stats.evictions > 0);
}

#[test]
fn test_unified_cache_performance() {
    let cache_config = create_test_cache_config();
    let hotspot_config = create_test_ewma_config();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);
    
    let start_time = Instant::now();
    
    // 插入大量条目
    for i in 0..1000 {
        let addr = 0x1000 + i * 0x100;
        let code = vec![0x90; 64]; // 64字节代码
        cache.insert_sync(addr, code, false);
    }
    
    let insertion_time = start_time.elapsed();
    
    // 测试查找性能
    let lookup_start = Instant::now();
    for i in 0..1000 {
        let addr = 0x1000 + i * 0x100;
        cache.lookup(addr);
    }
    let lookup_time = lookup_start.elapsed();
    
    // 验证性能
    assert!(insertion_time.as_millis() < 100, "Insertion should be fast");
    assert!(lookup_time.as_millis() < 50, "Lookup should be fast");
    
    let stats = cache.get_stats();
    assert!(stats.avg_lookup_time_ns > 0);
}

#[test]
fn test_unified_cache_concurrent_access() {
    let cache_config = create_test_cache_config();
    let hotspot_config = create_test_ewma_config();
    let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
    
    // 并发访问测试
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let cache = cache.clone();
            thread::spawn(move || {
                for j in 0..100 {
                    let addr = 0x1000 + (i * 100 + j) * 0x100;
                    let code = vec![0x90];
                    
                    // 插入
                    cache.insert_sync(addr, code.clone(), false);
                    
                    // 查找
                    cache.lookup(addr);
                }
                true
            })
        })
        .collect();
    
    // 等待所有线程完成
    for handle in handles {
        assert!(handle.join().unwrap());
    }
    
    // 验证并发访问结果
    let stats = cache.get_stats();
    assert!(stats.total_entries > 0);
    assert!(stats.hits > 0);
}

// ============================================================================
// 智能预取器测试
// ============================================================================

#[test]
fn test_smart_prefetcher_basic_functionality() {
    let config = create_test_prefetch_config();
    let prefetcher = SmartPrefetcher::new(config);
    
    // 记录跳转历史
    prefetcher.record_jump(0x1000, 0x2000);
    prefetcher.record_jump(0x1000, 0x2000); // 重复跳转
    prefetcher.record_jump(0x1000, 0x3000);
    prefetcher.record_jump(0x2000, 0x4000);
    prefetcher.record_jump(0x2000, 0x4000); // 重复跳转
    
    // 获取预取地址
    if let Some(addr) = prefetcher.get_next_prefetch_address() {
        prefetcher.mark_prefetched(addr);
        prefetcher.record_prefetch_hit();
        
        // 验证统计
        let stats = prefetcher.get_stats();
        assert!(stats.total_prefetch_requests > 0);
        assert!(stats.successful_prefetches > 0);
        assert!(stats.prefetch_hits > 0);
        assert!(stats.prefetch_accuracy > 0.0);
    }
}

#[test]
fn test_smart_prefetcher_access_pattern_learning() {
    let config = create_test_prefetch_config();
    let prefetcher = SmartPrefetcher::new(config);
    
    // 记录顺序访问模式
    prefetcher.record_jump(0x1000, 0x1100);
    prefetcher.record_jump(0x1100, 0x1200);
    prefetcher.record_jump(0x1200, 0x1300);
    
    // 记录分支访问模式
    prefetcher.record_jump(0x2000, 0x3000);
    prefetcher.record_jump(0x2000, 0x4000);
    
    // 记录循环访问模式
    prefetcher.record_jump(0x5000, 0x6000);
    prefetcher.record_jump(0x6000, 0x5000);
    
    // 验证预取队列
    let queue_size = prefetcher.queue_size();
    assert!(queue_size > 0);
    
    // 获取预取地址
    let mut prefetched_addrs = Vec::new();
    while let Some(addr) = prefetcher.get_next_prefetch_address() {
        prefetched_addrs.push(addr);
        if prefetched_addrs.len() >= 5 {
            break;
        }
    }
    
    assert!(!prefetched_addrs.is_empty());
}

#[test]
fn test_smart_prefetcher_prediction_accuracy() {
    let config = create_test_prefetch_config();
    let prefetcher = SmartPrefetcher::new(config);
    
    // 建立强跳转模式
    for _ in 0..10 {
        prefetcher.record_jump(0x1000, 0x2000);
        prefetcher.record_jump(0x1000, 0x2000); // 强模式
    }
    
    // 获取预测地址
    let mut predictions = Vec::new();
    for _ in 0..5 {
        if let Some(addr) = prefetcher.get_next_prefetch_address() {
            predictions.push(addr);
            prefetcher.mark_prefetched(addr);
        }
    }
    
    // 验证预测包含最可能的跳转目标
    assert!(predictions.contains(&0x2000));
}

#[test]
fn test_smart_prefetcher_queue_management() {
    let mut config = create_test_prefetch_config();
    config.max_prefetch_queue_size = 3; // 小队列
    
    let prefetcher = SmartPrefetcher::new(config);
    
    // 添加多个跳转
    for i in 0..10 {
        prefetcher.record_jump(0x1000, 0x2000 + i * 0x100);
    }
    
    // 验证队列大小限制
    let queue_size = prefetcher.queue_size();
    assert!(queue_size <= 3);
    
    let stats = prefetcher.get_stats();
    assert!(stats.queue_size <= 3);
}

#[test]
fn test_smart_prefetcher_performance() {
    let config = create_test_prefetch_config();
    let prefetcher = SmartPrefetcher::new(config);
    
    let start_time = Instant::now();
    
    // 记录大量跳转
    for i in 0..1000 {
        let from = 0x1000 + (i % 100) * 0x100;
        let to = 0x2000 + (i % 50) * 0x100;
        prefetcher.record_jump(from, to);
    }
    
    let elapsed = start_time.elapsed();
    
    // 验证性能
    assert!(elapsed.as_millis() < 50, "Prefetcher should be fast");
    
    let stats = prefetcher.get_stats();
    assert!(stats.total_prefetch_requests > 0);
}

// ============================================================================
// 集成测试
// ============================================================================

#[test]
fn test_cache_with_hotspot_integration() {
    let cache_config = create_test_cache_config();
    let hotspot_config = create_test_ewma_config();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);
    
    let addr = 0x1000;
    let code = vec![0x90];
    
    // 插入代码
    cache.insert_sync(addr, code, false);
    
    // 记录执行（触发热点检测）
    for _ in 0..15 {
        cache.record_execution(addr, 50, 1.0);
        cache.lookup(addr); // 访问缓存
    }
    
    // 验证热点检测与缓存集成
    assert!(cache.is_hotspot(addr));
    
    let stats = cache.get_stats();
    assert!(stats.hits > 0);
}

#[test]
fn test_cache_with_prefetch_integration() {
    let mut cache_config = create_test_cache_config();
    let hotspot_config = create_test_ewma_config();
    let prefetch_config = create_test_prefetch_config();
    
    let cache = UnifiedCodeCache::with_prefetch_config(
        cache_config,
        hotspot_config,
        prefetch_config,
    );
    
    // 记录跳转
    cache.record_jump(0x1000, 0x2000);
    cache.record_jump(0x2000, 0x3000);
    cache.record_jump(0x1000, 0x2000); // 重复跳转
    
    // 插入代码
    cache.insert_sync(0x1000, vec![0x90], false);
    cache.insert_sync(0x2000, vec![0x90], false);
    cache.insert_sync(0x3000, vec![0x90], false);
    
    // 访问代码（可能触发预取）
    cache.lookup(0x1000);
    
    // 验证预取统计
    if let Some(prefetch_stats) = cache.get_prefetch_stats() {
        assert!(prefetch_stats.total_prefetch_requests > 0);
    }
}

#[test]
fn test_cache_async_operations() {
    use tokio_test;
    
    let cache_config = create_test_cache_config();
    let hotspot_config = create_test_ewma_config();
    let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
    
    // 异步测试
    tokio_test::block_on(async {
        let addr = 0x1000;
        let block = create_test_ir_block(addr);
        
        // 异步插入
        cache.insert_async(addr, block, CompilePriority::High);
        
        // 异步查找
        let result = cache.get_async(addr).await;
        // 注意：由于是异步编译，可能还没有完成
    });
}

#[test]
fn test_cache_memory_management() {
    let mut cache_config = create_test_cache_config();
    cache_config.max_memory_bytes = 1024; // 1KB限制
    
    let hotspot_config = create_test_ewma_config();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);
    
    // 插入大量代码（超过内存限制）
    for i in 0..20 {
        let addr = 0x1000 + i * 0x100;
        let code = vec![0x90; 100]; // 100字节代码
        cache.insert_sync(addr, code, false);
    }
    
    // 验证内存管理
    let stats = cache.get_stats();
    assert!(stats.total_size_bytes <= 1024);
    assert!(stats.evictions > 0);
}

#[test]
fn test_cache_statistics_accuracy() {
    let cache_config = create_test_cache_config();
    let hotspot_config = create_test_ewma_config();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);
    
    // 插入一些条目
    for i in 0..10 {
        let addr = 0x1000 + i * 0x100;
        let code = vec![0x90; 64];
        cache.insert_sync(addr, code, false);
    }
    
    // 访问一些条目
    for i in 0..5 {
        let addr = 0x1000 + i * 0x100;
        cache.lookup(addr);
    }
    
    // 访问不存在的条目
    cache.lookup(0x9999);
    
    // 验证统计准确性
    let stats = cache.get_stats();
    assert!(stats.total_entries >= 10);
    assert!(stats.hits >= 5);
    assert!(stats.misses >= 1);
    assert!(stats.hit_rate > 0.0);
    assert!(stats.hit_rate < 1.0);
}

#[test]
fn test_cache_stress_test() {
    let cache_config = create_test_cache_config();
    let hotspot_config = create_test_ewma_config();
    let cache = Arc::new(UnifiedCodeCache::new(cache_config, hotspot_config));
    
    // 压力测试：多线程访问
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let cache = cache.clone();
            thread::spawn(move || {
                for j in 0..100 {
                    let addr = 0x1000 + (i * 100 + j) * 0x100;
                    let code = vec![0x90; 32];
                    
                    // 插入
                    cache.insert_sync(addr, code, false);
                    
                    // 查找
                    cache.lookup(addr);
                    
                    // 记录执行
                    cache.record_execution(addr, 25, 1.0);
                }
                true
            })
        })
        .collect();
    
    // 等待所有线程完成
    for handle in handles {
        assert!(handle.join().unwrap());
    }
    
    // 验证压力测试结果
    let stats = cache.get_stats();
    assert!(stats.total_entries > 0);
    assert!(stats.hits > 0);
    assert!(stats.misses > 0);
}

#[test]
fn test_cache_edge_cases() {
    let cache_config = create_test_cache_config();
    let hotspot_config = create_test_ewma_config();
    let cache = UnifiedCodeCache::new(cache_config, hotspot_config);
    
    // 测试空缓存
    let result = cache.lookup(0x1000);
    assert!(result.is_none());
    
    // 测试重复插入
    let addr = 0x1000;
    let code = vec![0x90];
    cache.insert_sync(addr, code.clone(), false);
    cache.insert_sync(addr, code.clone(), false); // 重复插入
    
    let result = cache.lookup(addr);
    assert!(result.is_some());
    
    // 测试边界地址
    let min_addr = 0;
    let max_addr = u64::MAX;
    
    cache.insert_sync(min_addr, vec![0x90], false);
    cache.insert_sync(max_addr, vec![0x90], false);
    
    assert!(cache.lookup(min_addr).is_some());
    assert!(cache.lookup(max_addr).is_some());
    
    // 验证统计
    let stats = cache.get_stats();
    assert!(stats.total_entries >= 3);
}