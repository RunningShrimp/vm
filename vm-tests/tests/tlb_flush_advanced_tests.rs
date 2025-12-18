//! TLB刷新策略高级优化测试
//!
//! 验证预测性刷新、选择性刷新和自适应优化的效果

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use vm_core::{AccessType, MMU};
use vm_mem::{
    AdvancedTlbFlushConfig, AdaptiveFlushConfig, PerCpuTlbManager, PredictiveFlushConfig,
    SelectiveFlushConfig, TlbFlushConfig, FlushStrategy, FlushScope, FlushRequest,
    AdvancedTlbFlushManager, AccessPredictor, PageImportanceEvaluator,
};

/// 测试预测性刷新功能
#[test]
fn test_predictive_flush() {
    println!("=== 预测性刷新测试 ===");
    
    let tlb_manager = Arc::new(PerCpuTlbManager::with_default_config());
    let config = AdvancedTlbFlushConfig {
        predictive_config: PredictiveFlushConfig {
            enabled: true,
            prediction_window: 4,
            accuracy_threshold: 0.5,
            max_predictive_flushes: 2,
            history_size: 64,
        },
        ..Default::default()
    };
    
    let flush_manager = AdvancedTlbFlushManager::new(config, tlb_manager.clone());
    
    // 模拟顺序访问模式
    let base_addr = 0x1000;
    for i in 0..10 {
        flush_manager.record_access(base_addr + i * 4096, 0, AccessType::Read);
    }
    
    // 请求刷新当前页面
    let request = FlushRequest::new(
        FlushScope::SinglePage,
        base_addr + 9 * 4096,
        base_addr + 9 * 4096,
        0,
        10,
        0,
    );
    
    let result = flush_manager.request_flush(request);
    assert!(result.is_ok());
    
    // 检查预测性刷新统计
    let predictive_stats = flush_manager.get_predictive_stats();
    println!("预测性刷新次数: {}", predictive_stats.predictive_flushes);
    println!("预测准确率: {:.2}%", predictive_stats.prediction_accuracy * 100.0);
    
    // 验证预测性刷新是否被触发
    assert!(predictive_stats.predictive_flushes > 0, "应该触发预测性刷新");
}

/// 测试选择性刷新功能
#[test]
fn test_selective_flush() {
    println!("=== 选择性刷新测试 ===");
    
    let tlb_manager = Arc::new(PerCpuTlbManager::with_default_config());
    let config = AdvancedTlbFlushConfig {
        selective_config: SelectiveFlushConfig {
            enabled: true,
            hot_page_threshold: 5,
            cold_page_threshold: 2,
            frequency_weight: 0.6,
            recency_weight: 0.3,
            size_weight: 0.1,
        },
        ..Default::default()
    };
    
    let flush_manager = AdvancedTlbFlushManager::new(config, tlb_manager.clone());
    
    // 创建热点页面和冷页面
    let hot_addr = 0x2000;
    let cold_addr = 0x3000;
    
    // 热点页面：频繁访问
    for _ in 0..10 {
        flush_manager.record_access(hot_addr, 0, AccessType::Read);
    }
    
    // 冷页面：少量访问
    for _ in 0..2 {
        flush_manager.record_access(cold_addr, 0, AccessType::Read);
    }
    
    // 请求刷新包含热点和冷页面的范围
    let request = FlushRequest::new(
        FlushScope::PageRange,
        hot_addr,
        cold_addr,
        0,
        10,
        0,
    );
    
    let result = flush_manager.request_flush(request);
    assert!(result.is_ok());
    
    // 检查选择性刷新统计
    let predictive_stats = flush_manager.get_predictive_stats();
    println!("选择性刷新次数: {}", predictive_stats.selective_flushes);
    println!("跳过的热点页面数: {}", predictive_stats.skipped_hot_pages);
    
    // 验证选择性刷新是否被触发
    assert!(predictive_stats.selective_flushes > 0, "应该触发选择性刷新");
}

/// 测试自适应刷新功能
#[test]
fn test_adaptive_flush() {
    println!("=== 自适应刷新测试 ===");
    
    let tlb_manager = Arc::new(PerCpuTlbManager::with_default_config());
    let config = AdvancedTlbFlushConfig {
        adaptive_config: AdaptiveFlushConfig {
            enabled: true,
            monitoring_window: 20,
            performance_threshold: 0.1,
            strategy_switch_interval: 1,
            min_samples: 5,
        },
        ..Default::default()
    };
    
    let flush_manager = AdvancedTlbFlushManager::new(config, tlb_manager.clone());
    
    // 模拟不同性能场景
    for i in 0..30 {
        let addr = 0x4000 + i * 4096;
        flush_manager.record_access(addr, 0, AccessType::Read);
        
        let request = FlushRequest::new(
            FlushScope::SinglePage,
            addr,
            addr,
            0,
            10,
            0,
        );
        
        let _ = flush_manager.request_flush(request);
        
        // 短暂延迟以模拟不同性能
        thread::sleep(Duration::from_millis(1));
    }
    
    // 检查基础刷新统计
    let stats = flush_manager.get_stats();
    println!("总刷新请求数: {}", stats.total_requests);
    println!("自适应刷新次数: {}", stats.adaptive_flushes);
    
    // 验证自适应刷新是否被触发
    assert!(stats.adaptive_flushes > 0, "应该触发自适应刷新");
}

/// 测试页面重要性评估
#[test]
fn test_page_importance_evaluation() {
    println!("=== 页面重要性评估测试 ===");
    
    let mut evaluator = PageImportanceEvaluator::new(SelectiveFlushConfig::default());
    
    // 创建不同访问模式的页面
    let frequent_addr = 0x5000;
    let recent_addr = 0x6000;
    let large_addr = 0x7000;
    
    // 频繁访问页面
    for _ in 0..20 {
        evaluator.record_access(frequent_addr, 0, AccessType::Read);
    }
    
    // 最近访问页面
    evaluator.record_access(recent_addr, 0, AccessType::Read);
    
    // 大页面（模拟）
    evaluator.record_access(large_addr, 0, AccessType::Read);
    
    // 评估页面重要性
    let frequent_importance = evaluator.evaluate_importance(frequent_addr, 0);
    let recent_importance = evaluator.evaluate_importance(recent_addr, 0);
    let large_importance = evaluator.evaluate_importance(large_addr, 0);
    
    println!("频繁访问页面重要性: {:.2}", frequent_importance);
    println!("最近访问页面重要性: {:.2}", recent_importance);
    println!("大页面重要性: {:.2}", large_importance);
    
    // 验证评估结果
    assert!(frequent_importance > recent_importance, "频繁访问页面应该更重要");
    
    // 获取热点页面
    let hot_pages = evaluator.get_hot_pages(0);
    assert!(!hot_pages.is_empty(), "应该有热点页面");
    assert_eq!(hot_pages[0].0, frequent_addr, "最热页面应该是频繁访问页面");
}

/// 测试访问预测器
#[test]
fn test_access_predictor() {
    println!("=== 访问预测器测试 ===");
    
    let mut predictor = AccessPredictor::new(64, 4);
    
    // 创建顺序访问模式
    for i in 0..8 {
        predictor.record_access(i * 4096, 0);
    }
    
    // 预测下一个访问
    let predicted = predictor.predict_next_accesses(0);
    println!("预测的下一个访问: {:?}", predicted);
    
    // 验证预测结果
    assert!(!predicted.is_empty(), "应该有预测结果");
    
    // 验证预测
    let is_correct = predictor.validate_prediction(&predicted, 8 * 4096);
    assert!(is_correct, "预测应该正确");
    
    // 检查预测准确率
    let accuracy = predictor.get_accuracy();
    println!("预测准确率: {:.2}%", accuracy * 100.0);
    assert!(accuracy > 0.0, "预测准确率应该大于0");
}

/// 测试高级刷新管理器综合性能
#[test]
fn test_advanced_flush_manager_performance() {
    println!("=== 高级刷新管理器性能测试 ===");
    
    let tlb_manager = Arc::new(PerCpuTlbManager::with_default_config());
    let config = AdvancedTlbFlushConfig::default();
    let flush_manager = AdvancedTlbFlushManager::new(config, tlb_manager.clone());
    
    // 模拟复杂访问模式
    let test_addresses: Vec<u64> = (0..100).map(|i| 0x10000 + i * 4096).collect();
    let iterations = 1000;
    
    // 预热阶段
    for &addr in &test_addresses {
        flush_manager.record_access(addr, 0, AccessType::Read);
    }
    
    // 测试阶段
    let start = Instant::now();
    
    for i in 0..iterations {
        let addr = test_addresses[i % test_addresses.len()];
        
        // 记录访问
        flush_manager.record_access(addr, 0, AccessType::Read);
        
        // 定期请求刷新
        if i % 10 == 0 {
            let request = FlushRequest::new(
                FlushScope::PageRange,
                addr,
                addr + 4096 * 5,
                0,
                10,
                0,
            );
            
            let _ = flush_manager.request_flush(request);
        }
    }
    
    let duration = start.elapsed();
    
    // 获取统计信息
    let stats = flush_manager.get_stats();
    let predictive_stats = flush_manager.get_predictive_stats();
    
    println!("测试时间: {:?}", duration);
    println!("总刷新请求数: {}", stats.total_requests);
    println!("平均刷新时间: {} ns", stats.avg_flush_time_ns);
    println!("优化率: {:.2}%", stats.optimization_rate * 100.0);
    println!("预测性刷新次数: {}", predictive_stats.predictive_flushes);
    println!("选择性刷新次数: {}", predictive_stats.selective_flushes);
    println!("预测准确率: {:.2}%", predictive_stats.prediction_accuracy * 100.0);
    
    // 性能断言
    assert!(stats.optimization_rate > 0.1, "优化率应该超过10%");
    assert!(predictive_stats.prediction_accuracy > 0.5, "预测准确率应该超过50%");
}

/// 测试多线程环境下的高级刷新管理器
#[test]
fn test_advanced_flush_manager_concurrent() {
    println!("=== 高级刷新管理器并发测试 ===");
    
    let tlb_manager = Arc::new(PerCpuTlbManager::with_default_config());
    let config = AdvancedTlbFlushConfig::default();
    let flush_manager = Arc::new(AdvancedTlbFlushManager::new(config, tlb_manager.clone()));
    
    let thread_count = 4;
    let operations_per_thread = 100;
    
    let mut handles = Vec::new();
    
    for thread_id in 0..thread_count {
        let flush_manager = flush_manager.clone();
        
        let handle = thread::spawn(move || {
            for i in 0..operations_per_thread {
                let addr = 0x20000 + thread_id * 0x10000 + i * 4096;
                
                // 记录访问
                flush_manager.record_access(addr, thread_id as u16, AccessType::Read);
                
                // 请求刷新
                let request = FlushRequest::new(
                    FlushScope::SinglePage,
                    addr,
                    addr,
                    thread_id as u16,
                    10,
                    thread_id,
                );
                
                let _ = flush_manager.request_flush(request);
            }
        });
        
        handles.push(handle);
    }
    
    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }
    
    // 获取统计信息
    let stats = flush_manager.get_stats();
    let predictive_stats = flush_manager.get_predictive_stats();
    
    println!("总刷新请求数: {}", stats.total_requests);
    println!("优化率: {:.2}%", stats.optimization_rate * 100.0);
    println!("预测性刷新次数: {}", predictive_stats.predictive_flushes);
    println!("选择性刷新次数: {}", predictive_stats.selective_flushes);
    
    // 验证并发安全性
    assert_eq!(stats.total_requests, (thread_count * operations_per_thread) as u64);
}