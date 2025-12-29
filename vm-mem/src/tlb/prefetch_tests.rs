//! TLB预热机制测试
//!
//! 测试MultiLevelTlb的预热功能：
//! - 基本预热功能
//! - 批量预热功能
//! - 预热统计验证
//! - 预热效果评估

use super::{MultiLevelTlb, MultiLevelTlbConfig, OptimizedTlbEntry};
use std::collections::HashMap;

#[cfg(test)]
mod tlb_prefetch_tests {
    use super::*;

    /// 测试基本的预热功能
    #[test]
    fn test_basic_prefetch() {
        // 创建配置（启用预热）
        let config = MultiLevelTlbConfig {
            l1_capacity: 16,
            l2_capacity: 64,
            l3_capacity: 128,
            prefetch_window: 8,
            prefetch_threshold: 0.8,
            adaptive_replacement: true,
            concurrent_optimization: false,
            enable_stats: true,
            enable_prefetch: true,  // 启用预热
        };

        let mut tlb = MultiLevelTlb::new(config);

        // 准备预热地址
        let addresses = vec![
            GuestAddr(0x1000),  // 4KB
            GuestAddr(0x2000),  // 8KB
            GuestAddr(0x3000),  // 12KB
            GuestAddr(0x4000),  // 16KB
        ];

        // 添加到预取队列
        tlb.prefetch_addresses(addresses);

        // 验证预取队列中有地址
        assert!(tlb.prefetch_queue.len() == 4, "预取队列应该包含4个地址");

        // 执行预热
        tlb.prefetch();

        // 验证预热完成
        assert!(tlb.prefetch_done, "预热应该已完成");

        // 获取统计信息
        let stats = tlb.get_stats();
        assert!(stats.prefetch_hits.load(std::sync::atomic::Ordering::Relaxed) >= 4, 
                "应该有至少4个预热命中");

        // 验证L1 TLB中有预热条目
        let usage = tlb.get_usage();
        assert!(usage.0 >= 4, "L1 TLB中应该有至少4个预热条目");
    }

    /// 测试批量预热功能
    #[test]
    fn test_batch_prefetch() {
        let config = MultiLevelTlbConfig {
            l1_capacity: 32,
            l2_capacity: 128,
            l3_capacity: 512,
            prefetch_window: 16,
            prefetch_threshold: 0.75,
            adaptive_replacement: true,
            concurrent_optimization: false,
            enable_stats: true,
            enable_prefetch: true,
        };

        let mut tlb = MultiLevelTlb::new(config);

        // 批量添加大量地址
        let addresses: Vec<GuestAddr> = (0..32)
            .map(|i| GuestAddr(0x1000 + (i as u64) * 4096))
            .collect();

        tlb.prefetch_addresses(addresses);

        // 验证预取队列限制（应该有最多 prefetch_window * 2 个）
        assert!(tlb.prefetch_queue.len() <= 32, "预取队列大小应该被限制");

        // 执行预热（应该只预热prefetch_window个）
        tlb.prefetch();

        let stats = tlb.get_stats();
        let prefetch_hits = stats.prefetch_hits.load(std::sync::atomic::Ordering::Relaxed);
        
        // 验证预热了最多16个（prefetch_window）
        assert!(prefetch_hits <= 16, "预热数量应该限制在prefetch_window内");
        assert!(prefetch_hits > 0, "应该有至少1个预热命中");
    }

    /// 测试预热禁用状态
    #[test]
    fn test_prefetch_disabled() {
        let config = MultiLevelTlbConfig {
            l1_capacity: 16,
            l2_capacity: 64,
            l3_capacity: 128,
            prefetch_window: 4,
            prefetch_threshold: 0.8,
            adaptive_replacement: true,
            concurrent_optimization: false,
            enable_stats: true,
            enable_prefetch: false,  // 禁用预热
        };

        let mut tlb = MultiLevelTlb::new(config);

        // 准备预热地址
        let addresses = vec![
            GuestAddr(0x1000),
            GuestAddr(0x2000),
        ];

        // 添加到预取队列（应该被忽略）
        tlb.prefetch_addresses(addresses);
        assert_eq!(tlb.prefetch_queue.len(), 0, "预热禁用时，预取队列应该为空");

        // 执行预热（应该不做任何事）
        tlb.prefetch();

        // 验证预热未执行
        assert!(!tlb.prefetch_done, "预热禁用时，不应标记为完成");

        let stats = tlb.get_stats();
        assert_eq!(stats.prefetch_hits.load(std::sync::atomic::Ordering::Relaxed), 0, 
                   "预热禁用时，不应有预热命中");
    }

    /// 测试重复预热保护
    #[test]
    fn test_duplicate_prefetch_protection() {
        let config = MultiLevelTlbConfig {
            l1_capacity: 8,
            l2_capacity: 32,
            l3_capacity: 64,
            prefetch_window: 4,
            prefetch_threshold: 0.8,
            adaptive_replacement: true,
            concurrent_optimization: false,
            enable_stats: true,
            enable_prefetch: true,
        };

        let mut tlb = MultiLevelTlb::new(config);

        let addresses = vec![GuestAddr(0x1000), GuestAddr(0x2000)];

        // 第一次预热
        tlb.prefetch_addresses(addresses);
        tlb.prefetch();

        let stats1 = tlb.get_stats();
        let prefetch_hits1 = stats1.prefetch_hits.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(prefetch_hits1, 2, "第一次应该预热2个条目");

        // 重置prefetch_done标记，尝试第二次预热（应该被阻止）
        tlb.prefetch_done = false;
        tlb.prefetch();

        let stats2 = tlb.get_stats();
        let prefetch_hits2 = stats2.prefetch_hits.load(std::sync::atomic::Ordering::Relaxed);
        
        // 第二次应该没有额外的预热命中（因为prefetch_done保护）
        assert_eq!(prefetch_hits2, prefetch_hits1, "重复预热应该被阻止");
    }

    /// 测试预热后的访问性能
    #[test]
    fn test_prefetch_access_performance() {
        let config = MultiLevelTlbConfig {
            l1_capacity: 16,
            l2_capacity: 64,
            l3_capacity: 128,
            prefetch_window: 8,
            prefetch_threshold: 0.8,
            adaptive_replacement: true,
            concurrent_optimization: false,
            enable_stats: true,
            enable_prefetch: true,
        };

        let mut tlb = MultiLevelTlb::new(config);

        // 预热常用地址
        let hot_addresses = vec![
            GuestAddr(0x1000),
            GuestAddr(0x2000),
            GuestAddr(0x3000),
        ];

        tlb.prefetch_addresses(hot_addresses);
        tlb.prefetch();

        // 访问预热过的地址（应该命中L1 TLB）
        let _ = tlb.lookup(GuestAddr(0x1000), 0, AccessType::Read);
        let _ = tlb.lookup(GuestAddr(0x2000), 0, AccessType::Read);

        let stats = tlb.get_stats();
        let total_lookups = stats.total_lookups.load(std::sync::atomic::Ordering::Relaxed);
        let l1_hits = stats.l1_hits.load(std::sync::atomic::Ordering::Relaxed);

        // 应该有2次查找，都命中L1
        assert_eq!(total_lookups, 2, "应该有2次查找");
        assert!(l1_hits >= 2, "预热过的地址应该命中L1");
    }

    /// 测试预热条目标记
    #[test]
    fn test_prefetch_entry_marks() {
        let config = MultiLevelTlbConfig {
            l1_capacity: 8,
            l2_capacity: 32,
            l3_capacity: 64,
            prefetch_window: 4,
            prefetch_threshold: 0.8,
            adaptive_replacement: true,
            concurrent_optimization: false,
            enable_stats: true,
            enable_prefetch: true,
        };

        let mut tlb = MultiLevelTlb::new(config);

        // 预热地址
        tlb.prefetch_addresses(vec![GuestAddr(0x1000)]);
        tlb.prefetch();

        // 访问预热过的地址
        let vpn = 0x1000 >> 12;
        let key = (vpn, 0);

        // 直接检查L1 TLB中的条目
        if let Some(entry) = tlb.l1_tlb.entries.get(&key) {
            assert!(entry.prefetch_mark, "预热条目应该有prefetch_mark标记");
            assert!(entry.hot_mark, "预热条目应该有hot_mark标记");
        } else {
            panic!("预热过的地址应该在L1 TLB中");
        }
    }

    /// 测试预热统计准确性
    #[test]
    fn test_prefetch_stats_accuracy() {
        let config = MultiLevelTlbConfig {
            l1_capacity: 16,
            l2_capacity: 64,
            l3_capacity: 128,
            prefetch_window: 8,
            prefetch_threshold: 0.8,
            adaptive_replacement: true,
            concurrent_optimization: false,
            enable_stats: true,
            enable_prefetch: true,
        };

        let mut tlb = MultiLevelTlb::new(config);

        // 准备8个地址，但只有4个会在窗口内被预热
        let addresses: Vec<GuestAddr> = (0..8)
            .map(|i| GuestAddr(0x1000 + (i as u64) * 4096))
            .collect();

        tlb.prefetch_addresses(addresses);
        tlb.prefetch();

        let stats = tlb.get_stats();
        let prefetch_hits = stats.prefetch_hits.load(std::sync::atomic::Ordering::Relaxed);

        // 预热窗口是8，应该只预热8个
        assert_eq!(prefetch_hits, 8, "应该预热prefetch_window个条目");
    }

    /// 测试预热与正常访问混合
    #[test]
    fn test_mixed_prefetch_and_normal_access() {
        let config = MultiLevelTlbConfig {
            l1_capacity: 8,
            l2_capacity: 32,
            l3_capacity: 64,
            prefetch_window: 4,
            prefetch_threshold: 0.8,
            adaptive_replacement: true,
            concurrent_optimization: false,
            enable_stats: true,
            enable_prefetch: true,
        };

        let mut tlb = MultiLevelTlb::new(config);

        // 预热2个地址
        tlb.prefetch_addresses(vec![GuestAddr(0x1000), GuestAddr(0x2000)]);
        tlb.prefetch();

        // 访问预热过的地址（应该命中）
        let _ = tlb.lookup(GuestAddr(0x1000), 0, AccessType::Read);
        let stats1 = tlb.get_stats();
        let hits1 = stats1.l1_hits.load(std::sync::atomic::Ordering::Relaxed);

        // 访问未预热的地址（应该缺失）
        let _ = tlb.lookup(GuestAddr(0x4000), 0, AccessType::Read);
        let stats2 = tlb.get_stats();
        let misses = stats2.total_misses.load(std::sync::atomic::Ordering::Relaxed);

        // 应该有1次命中，至少1次缺失
        assert!(hits1 >= 1, "预热地址应该命中");
        assert!(misses >= 1, "未预热地址应该缺失");
    }
}

#[cfg(test)]
mod tlb_prefetch_integration_tests {
    use super::*;

    /// 集成测试：预热与翻译功能
    #[test]
    fn test_prefetch_integration_with_translation() {
        let config = MultiLevelTlbConfig {
            l1_capacity: 8,
            l2_capacity: 32,
            l3_capacity: 64,
            prefetch_window: 4,
            prefetch_threshold: 0.8,
            adaptive_replacement: true,
            concurrent_optimization: false,
            enable_stats: true,
            enable_prefetch: true,
        };

        let mut tlb = MultiLevelTlb::new(config);

        // 插入一个条目（模拟页表遍历）
        let vpn = 0x1000 >> 12;
        let ppn = vpn;  // 假设物理地址 = 虚拟地址
        let flags = 0x7;

        tlb.l1_tlb.insert_entry(vpn, ppn, flags, 0, 0);

        // 预热地址（应该被跳过，因为已经在L1中）
        tlb.prefetch_addresses(vec![GuestAddr(0x1000)]);
        tlb.prefetch();

        // 访问地址（应该命中L1）
        let result = tlb.lookup(GuestAddr(0x1000), 0, AccessType::Read);
        assert!(result.is_some(), "应该找到翻译结果");

        let stats = tlb.get_stats();
        let prefetch_hits = stats.prefetch_hits.load(std::sync::atomic::Ordering::Relaxed);

        // 预热应该被跳过（已在L1中），prefetch_hits应该为0
        assert_eq!(prefetch_hits, 0, "已存在的地址不应被重复预热");
    }

    /// 集成测试：预热队列管理
    #[test]
    fn test_prefetch_queue_management() {
        let config = MultiLevelTlbConfig {
            l1_capacity: 4,
            l2_capacity: 16,
            l3_capacity: 32,
            prefetch_window: 4,
            prefetch_threshold: 0.8,
            adaptive_replacement: true,
            concurrent_optimization: false,
            enable_stats: true,
            enable_prefetch: true,
        };

        let mut tlb = MultiLevelTlb::new(config);

        // 第一次添加4个地址
        let addresses1 = vec![
            GuestAddr(0x1000),
            GuestAddr(0x2000),
            GuestAddr(0x3000),
            GuestAddr(0x4000),
        ];

        tlb.prefetch_addresses(addresses1);
        assert_eq!(tlb.prefetch_queue.len(), 4, "队列应该有4个地址");

        // 第二次添加3个地址（队列应该限制到4 + 3 = 7）
        let addresses2 = vec![
            GuestAddr(0x5000),
            GuestAddr(0x6000),
            GuestAddr(0x7000),
        ];

        tlb.prefetch_addresses(addresses2);

        // 队列最多应该有7个（4 + 3）
        assert_eq!(tlb.prefetch_queue.len(), 7, "队列应该有7个地址");

        // 执行预热（应该预热4个）
        tlb.prefetch();

        let stats = tlb.get_stats();
        let prefetch_hits = stats.prefetch_hits.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(prefetch_hits, 4, "应该预热prefetch_window个");
    }
}

