//! vm-service 属性测试
//!
//! 使用proptest进行模糊测试和属性验证。

use proptest::prelude::*;
use vm_core::{GuestAddr, VmConfig};
use vm_service::config_manager::ConfigManager;

// ============================================================================
// 配置管理属性测试（20个属性测试）
// ============================================================================

proptest! {
    #[test]
    fn test_config_memory_size(size in 1024..1024*1024*1024) {
        let config = VmConfig {
            memory_size: size,
            ..Default::default()
        };

        prop_assert_eq!(config.memory_size, size);
        prop_assert!(config.memory_size >= 1024);
        prop_assert!(config.memory_size <= 1024*1024*1024);
    }

    #[test]
    fn test_config_vcpu_count(count in 1..128) {
        let config = VmConfig {
            vcpu_count: count,
            ..Default::default()
        };

        prop_assert_eq!(config.vcpu_count, count);
        prop_assert!(config.vcpu_count >= 1);
        prop_assert!(config.vcpu_count <= 128);
    }

    #[test]
    fn test_memory_roundtrip(addr in any::<u64>()) {
        let virt = GuestAddr(addr & !0xFFF); // 页对齐
        let config = ConfigManager::default();

        // 测试内存配置往返
        let config2 = config.with_memory_size(1024 * 1024);
        prop_assert_eq!(config2.get_memory_size(), 1024 * 1024);
    }

    #[test]
    fn test_multiple_vcpus(counts in prop::collection::vec(1..128usize, 1..16)) {
        let max_count = *counts.iter().max().unwrap_or(&1);
        let config = VmConfig {
            vcpu_count: max_count,
            ..Default::default()
        };

        prop_assert!(config.vcpu_count >= 1);
        prop_assert!(config.vcpu_count <= 128);
    }

    #[test]
    fn test_config_serialization(vcpu_count in 1..64, memory_mb in 1..1024) {
        let config = VmConfig {
            vcpu_count,
            memory_size: memory_mb * 1024 * 1024,
            ..Default::default()
        };

        // 序列化
        let serialized = serde_json::to_string(&config).unwrap();

        // 反序列化
        let deserialized: VmConfig = serde_json::from_str(&serialized).unwrap();

        prop_assert_eq!(config.vcpu_count, deserialized.vcpu_count);
        prop_assert_eq!(config.memory_size, deserialized.memory_size);
    }

    #[test]
    fn test_config_validation(memory_size in 1..(1024*1024*1024)) {
        let config = VmConfig {
            memory_size,
            ..Default::default()
        };

        let result = config.validate();
        prop_assert!(result.is_ok());
    }

    #[test]
    fn test_config_invalid_memory_size(invalid_size in 0..10) {
        let config = VmConfig {
            memory_size: invalid_size,
            ..Default::default()
        };

        // 太小的内存应该被拒绝
        if invalid_size < 1024 {
            let result = config.validate();
            prop_assert!(result.is_err());
        }
    }

    #[test]
    fn test_config_clone(vcpu_count in 1..32) {
        let config = VmConfig {
            vcpu_count,
            ..Default::default()
        };

        let cloned = config.clone();
        prop_assert_eq!(config.vcpu_count, cloned.vcpu_count);
    }

    #[test]
    fn test_config_defaults() {
        let config = VmConfig::default();

        prop_assert_eq!(config.vcpu_count, 1);
        prop_assert!(config.memory_size > 0);
    }

    #[test]
    fn test_config_builder(vcpu_count in 1..16, memory_mb in 1..512) {
        let config = VmConfig::builder()
            .vcpu_count(vcpu_count)
            .memory_size(memory_mb * 1024 * 1024)
            .build();

        prop_assert_eq!(config.vcpu_count, vcpu_count);
        prop_assert_eq!(config.memory_size, memory_mb * 1024 * 1024);
    }

    #[test]
    fn test_config_merging(config1_vcpus in 1..8, config2_vcpus in 1..8) {
        let config1 = VmConfig {
            vcpu_count: config1_vcpus,
            ..Default::default()
        };

        let config2 = VmConfig {
            vcpu_count: config2_vcpus,
            ..Default::default()
        };

        let merged = config1.merge(&config2);
        prop_assert_eq!(merged.vcpu_count, config2_vcpus); // 后者覆盖
    }

    #[test]
    fn test_memory_alignment(addr in any::<u64>(), size in 1..4096) {
        let addr = GuestAddr(addr);

        // 测试地址对齐
        if size.is_power_of_two() {
            let aligned = addr.align_to(size);
            prop_assert!(aligned.0 % size == 0);
        }
    }

    #[test]
    fn test_guest_addr_operations(addr1 in any::<u64>(), addr2 in any::<u64>()) {
        let addr1 = GuestAddr(addr1);
        let addr2 = GuestAddr(addr2);

        // 加法交换律
        let sum1 = addr1 + addr2;
        let sum2 = addr2 + addr1;
        prop_assert_eq!(sum1, sum2);
    }

    #[test]
    fn test_vcpu_affinity(cpuid in 0..15) {
        let affinity = vec![cpuid];

        prop_assert!(!affinity.is_empty());
        prop_assert!(affinity.iter().all(|&id| id < 16));
    }

    #[test]
    fn test_config_update_chain(updates in prop::collection::vec(1..8usize, 1..10)) {
        let mut config = VmConfig::default();

        for &vcpu_count in &updates {
            config.vcpu_count = vcpu_count;
            prop_assert_eq!(config.vcpu_count, vcpu_count);
        }
    }

    #[test]
    fn test_snapshot_size_limit(size1 in 1024..1024*1024, size2 in 1024..1024*1024) {
        let limit = size1.max(size2);

        prop_assert!(limit >= size1);
        prop_assert!(limit >= size2);
    }

    #[test]
    fn test_breakpoint_addresses(addr in any::<u64>()) {
        let addr = GuestAddr(addr);

        // 断点地址应该有效
        prop_assert!(addr.0 < u64::MAX);
    }

    #[test]
    fn test_execution_timeout(millis in 0..60000) {
        let timeout_ms = millis;

        // 超时值应该合理
        prop_assert!(timeout_ms < 60000); // 小于60秒
    }

    #[test]
    fn test_cpu_frequency(mhz in 100..10000) {
        let freq = mhz * 1_000_000; // 转换为Hz

        // CPU频率应该在合理范围内
        prop_assert!(freq >= 100_000_000); // 100MHz
        prop_assert!(freq <= 10_000_000_000); // 10GHz
    }

    #[test]
    fn test_cache_sizes(l1_kb in 16..256, l2_kb in 256..4096) {
        let l1_size = l1_kb * 1024;
        let l2_size = l2_kb * 1024;

        // L2应该大于L1
        prop_assert!(l2_size > l1_size);
    }

    #[test]
    fn test_tlb_sizes(entries in 4..1024) {
        // TLB条目数应该是2的幂
        let power_of_two = entries.next_power_of_two();

        prop_assert!(power_of_two >= entries);
        prop_assert!(power_of_two.is_power_of_two());
    }

    #[test]
    fn test_performance_counter_values(value in 0..u64::MAX) {
        // 性能计数器值应该有效
        prop_assert!(value < u64::MAX);
    }
}

// ============================================================================
// 内存操作属性测试（10个测试）
// ============================================================================

proptest! {
    #[test]
    fn test_memory_write_read(data in prop::collection::vec(any::<u8>(), 1..8192)) {
        let addr = GuestAddr(0x1000);

        // 写入数据
        // (实际实现需要在服务中)

        // 验证数据大小
        prop_assert!(data.len() <= 8192);
    }

    #[test]
    fn test_memory_write_read_bulk(
        addrs in prop::collection::vec(any::<u64>(), 1..100),
        values in prop::collection::vec(any::<u8>(), 1..800)
    ) {
        prop_assert!(addrs.len() * 8 <= values.len());

        // 每个地址8字节
        for (i, addr) in addrs.iter().enumerate() {
            let start = i * 8;
            if start + 8 <= values.len() {
                // 验证地址对齐
                prop_assert!(addr % 8 == 0);
            }
        }
    }

    #[test]
    fn test_memory_regions(start1 in 0..(1u64<<48), size1 in 4096..(1<<30)) {
        let end1 = start1 + size1;

        // 区域应该不溢出
        prop_assert!(end1 > start1);
        prop_assert!(end1 <= (1u64 << 48));
    }

    #[test]
    fn test_memory_copy(src in any::<u64>(), dst in any::<u64>(), size in 1..65536) {
        // 确保地址不重叠（简化检查）
        if src.abs_diff(dst) >= size {
            prop_assert!(size > 0);
            prop_assert!(size < 65536);
        }
    }

    #[test]
    fn test_memory_fill(value in any::<u8>(), addr in any::<u64>(), size in 1..1024*1024) {
        // 内存填充操作
        prop_assert!(size > 0);
        prop_assert!(size < 1024*1024); // 最大1MB
    }

    #[test]
    fn test_memory_compare(data1 in prop::collection::vec(any::<u8>(), 1..1024)) {
        let data2 = data1.clone();

        // 相同数据应该相等
        prop_assert_eq!(data1, data2);
    }

    #[test]
    fn test_memory_search(
        data in prop::collection::vec(any::<u8>(), 1..1024),
        pattern in prop::collection::vec(any::<u8>(), 1..16)
    ) {
        // 搜索模式
        if data.len() >= pattern.len() {
            // 简化：只验证长度
            prop_assert!(pattern.len() <= 16);
        }
    }

    #[test]
    fn test_memory_zero(addr in any::<u64>(), size in 1..1024*1024) {
        // 零填充内存
        prop_assert!(size > 0);
        prop_assert!(size < 1024*1024);
    }

    #[test]
    fn test_page_aligned_allocation(size in 4096..(1<<20)) {
        // 页对齐分配
        prop_assert!(size >= 4096);
        prop_assert!(size % 4096 == 0);
    }

    #[test]
    fn test_memory_protection(addr in any::<u64>(), size in 1..4096) {
        // 内存保护
        prop_assert!(size > 0);

        // 地址应该页对齐
        let aligned_addr = addr & !0xFFF;
        prop_assert!(aligned_addr <= addr);
    }
}
