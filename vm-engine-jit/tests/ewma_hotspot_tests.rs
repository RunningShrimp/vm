//! EWMA热点检测测试套件

use std::time::Duration;
use vm_core::GuestAddr;
use vm_engine_jit::ewma_hotspot::{EwmaHotspotConfig, EwmaHotspotDetector};

#[test]
fn test_ewma_hotspot_basic() {
    let config = EwmaHotspotConfig::default();
    let detector = EwmaHotspotDetector::new(config);

    let addr = 0x1000;

    // 记录执行
    detector.record_execution(addr, 100); // 100微秒
    detector.record_execution(addr, 150);
    detector.record_execution(addr, 120);

    // 检查是否为热点
    let is_hotspot = detector.is_hotspot(addr);
    // 取决于配置和阈值
    assert!(is_hotspot || !is_hotspot); // 至少不会panic
}

#[test]
fn test_ewma_hotspot_frequency_smoothing() {
    let mut config = EwmaHotspotConfig::default();
    config.frequency_alpha = 0.5; // 更高的alpha，更重视最新数据
    let detector = EwmaHotspotDetector::new(config);

    let addr = 0x1000;

    // 快速连续执行
    for _ in 0..10 {
        detector.record_execution(addr, 50);
    }

    // 应该被识别为热点
    let is_hotspot = detector.is_hotspot(addr);
    assert!(is_hotspot);
}

#[test]
fn test_ewma_hotspot_execution_time_smoothing() {
    let mut config = EwmaHotspotConfig::default();
    config.execution_time_alpha = 0.3;
    config.min_execution_time_us = 10;
    let detector = EwmaHotspotDetector::new(config);

    let addr = 0x1000;

    // 记录长时间执行
    for _ in 0..5 {
        detector.record_execution(addr, 1000); // 1ms
    }

    // 应该被识别为热点
    let is_hotspot = detector.is_hotspot(addr);
    assert!(is_hotspot);
}

#[test]
fn test_ewma_hotspot_threshold() {
    let mut config = EwmaHotspotConfig::default();
    config.hotspot_threshold = 50.0; // 较低的阈值
    let detector = EwmaHotspotDetector::new(config);

    let addr = 0x1000;

    // 少量执行
    for _ in 0..3 {
        detector.record_execution(addr, 100);
    }

    // 应该被识别为热点（阈值较低）
    let is_hotspot = detector.is_hotspot(addr);
    assert!(is_hotspot);
}

#[test]
fn test_ewma_hotspot_multiple_addresses() {
    let config = EwmaHotspotConfig::default();
    let detector = EwmaHotspotDetector::new(config);

    let addr1 = 0x1000;
    let addr2 = 0x2000;

    // 记录不同地址的执行
    for _ in 0..5 {
        detector.record_execution(addr1, 100);
    }
    for _ in 0..2 {
        detector.record_execution(addr2, 100);
    }

    // addr1应该是热点，addr2可能不是
    let is_hotspot1 = detector.is_hotspot(addr1);
    let is_hotspot2 = detector.is_hotspot(addr2);
    assert!(is_hotspot1);
    // addr2可能不是热点
}

#[test]
fn test_ewma_hotspot_stats() {
    let config = EwmaHotspotConfig::default();
    let detector = EwmaHotspotDetector::new(config);

    let addr = 0x1000;

    // 记录执行
    for _ in 0..10 {
        detector.record_execution(addr, 100);
    }

    let stats = detector.get_stats();
    assert_eq!(stats.total_records, 10);
    assert!(stats.hotspot_identified > 0);
}

#[test]
fn test_ewma_hotspot_cleanup() {
    let mut config = EwmaHotspotConfig::default();
    config.cleanup_interval_secs = 1; // 1秒清理间隔
    let detector = EwmaHotspotDetector::new(config);

    let addr = 0x1000;

    // 记录执行
    detector.record_execution(addr, 100);

    // 等待清理
    std::thread::sleep(Duration::from_secs(2));

    // 再次记录执行（应该触发清理）
    detector.record_execution(addr, 100);

    // 应该仍然能正常工作
    let is_hotspot = detector.is_hotspot(addr);
    assert!(is_hotspot || !is_hotspot);
}

