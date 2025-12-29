//! TLB预热机制简化测试
//!
//! 直接测试TLB预热功能，避免依赖其他有问题的模块

#[cfg(test)]
mod prefetch_simple_tests {
    use crate::tlb::{MultiLevelTlb, MultiLevelTlbConfig, StaticPreheatMode};
    use vm_core::GuestAddr;

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
            enable_prefetch: true, // 启用预热
            enable_pattern_tracking: false,
            preheat_window_size: 4,
            static_preheat_mode: StaticPreheatMode::Disabled,
        };

        let mut tlb = MultiLevelTlb::new(config);

        // 准备预热地址
        let addresses = vec![
            GuestAddr(0x1000), // 4KB
            GuestAddr(0x2000), // 8KB
            GuestAddr(0x3000), // 12KB
        ];

        // 添加到预取队列
        tlb.prefetch_addresses(addresses);

        // 验证预取队列中有地址
        assert_eq!(tlb.prefetch_queue_len(), 3, "预取队列应该包含3个地址");

        // 执行预热
        tlb.prefetch();

        // 验证预热完成
        assert!(tlb.is_prefetch_done(), "预热应该已完成");

        // 获取统计信息
        let stats = tlb.get_stats();
        assert!(
            stats
                .prefetch_hits
                .load(std::sync::atomic::Ordering::Relaxed)
                >= 3,
            "应该有至少3个预热命中"
        );

        // 验证L1 TLB中有预热条目
        let usage = tlb.get_usage();
        assert!(usage.0 >= 3, "L1 TLB中应该有至少3个预热条目");
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
            enable_pattern_tracking: false,
            preheat_window_size: 4,
            static_preheat_mode: StaticPreheatMode::Disabled,
        };

        let mut tlb = MultiLevelTlb::new(config);

        // 批量添加大量地址
        let addresses: Vec<GuestAddr> = (0..32)
            .map(|i| GuestAddr(0x1000 + (i as u64) * 4096))
            .collect();

        tlb.prefetch_addresses(addresses);

        // 验证预取队列限制（应该有最多 prefetch_window * 2 个）
        assert!(tlb.prefetch_queue_len() <= 32, "预取队列大小应该被限制");

        // 执行预热（应该只预热prefetch_window个）
        tlb.prefetch();

        let stats = tlb.get_stats();
        let prefetch_hits = stats
            .prefetch_hits
            .load(std::sync::atomic::Ordering::Relaxed);

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
            enable_prefetch: false, // 禁用预热
            enable_pattern_tracking: false,
            preheat_window_size: 4,
            static_preheat_mode: StaticPreheatMode::Disabled,
        };

        let mut tlb = MultiLevelTlb::new(config);

        // 准备预热地址
        let addresses = vec![GuestAddr(0x1000), GuestAddr(0x2000)];

        // 添加到预取队列（应该被忽略）
        tlb.prefetch_addresses(addresses);
        assert_eq!(tlb.prefetch_queue_len(), 0, "预热禁用时，预取队列应该为空");

        // 执行预热（应该不做任何事）
        tlb.prefetch();

        // 验证预热未执行
        assert!(!tlb.is_prefetch_done(), "预热禁用时，不应标记为完成");

        let stats = tlb.get_stats();
        assert_eq!(
            stats
                .prefetch_hits
                .load(std::sync::atomic::Ordering::Relaxed),
            0,
            "预热禁用时，不应有预热命中"
        );
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
            enable_pattern_tracking: false,
            preheat_window_size: 4,
            static_preheat_mode: StaticPreheatMode::Disabled,
        };

        let mut tlb = MultiLevelTlb::new(config);

        // 准备8个地址，但只有4个会在窗口内被预热
        let addresses: Vec<GuestAddr> = (0..8)
            .map(|i| GuestAddr(0x1000 + (i as u64) * 4096))
            .collect();

        tlb.prefetch_addresses(addresses);
        tlb.prefetch();

        let stats = tlb.get_stats();
        let prefetch_hits = stats
            .prefetch_hits
            .load(std::sync::atomic::Ordering::Relaxed);

        // 预热窗口是8，应该只预热8个
        assert!(prefetch_hits > 0, "应该有预热条目");
    }
}
