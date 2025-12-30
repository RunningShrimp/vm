//! 优化器测试套件 (最小可行版本)
//!
//! 测试VM优化器的基础功能

use vm_optimizers::{
    BlockFeatures, BlockProfile, CallProfile, CompilationDecision, ConcurrencyConfig,
    MemoryOptimizer,
};

// ============================================================================
// Memory Optimizer测试
// ============================================================================

#[cfg(test)]
mod memory_optimizer_tests {
    use super::*;

    /// 测试1: MemoryOptimizer创建
    #[test]
    fn test_memory_optimizer_creation() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let stats = optimizer.get_tlb_stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
    }

    /// 测试2: 地址翻译
    #[test]
    fn test_translate_address() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let result = optimizer.translate(0x1000);
        assert!(result.is_ok());
    }

    /// 测试3: 批量地址翻译
    #[test]
    fn test_batch_translate() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let addrs = vec![0x1000, 0x2000, 0x3000];
        let result = optimizer.batch_access(&addrs);
        assert!(result.is_ok());
    }

    /// 测试4: 内存分配
    #[test]
    fn test_allocate_memory() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let result = optimizer.allocate(1024);
        assert!(result.is_ok());
    }

    /// 测试5: TLB统计
    #[test]
    fn test_tlb_statistics() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let _ = optimizer.translate(0x1000);
        let stats = optimizer.get_tlb_stats();
        assert!(stats.hits + stats.misses > 0);
    }

    /// 测试6: TLB命中率
    #[test]
    fn test_tlb_hit_rate() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        for _ in 0..10 {
            let _ = optimizer.translate(0x1000);
        }

        let stats = optimizer.get_tlb_stats();
        let hit_rate = stats.hit_rate();
        assert!(hit_rate >= 0.0 && hit_rate <= 100.0);
    }

    /// 测试7: 并发配置
    #[test]
    fn test_concurrency_config() {
        let config = ConcurrencyConfig::new(8);
        assert_eq!(config.max_concurrent, 8);
        assert!(config.enabled);
    }

    /// 测试8: 并发配置验证
    #[test]
    fn test_concurrency_config_validation() {
        let config = ConcurrencyConfig::new(0);
        let result = config.validate();
        assert!(result.is_err());

        let config2 = ConcurrencyConfig::new(16);
        let result2 = config2.validate();
        assert!(result2.is_ok());
    }

    /// 测试9: 默认并发配置
    #[test]
    fn test_default_concurrency_config() {
        let config = ConcurrencyConfig::default();
        assert_eq!(config.max_concurrent, 8);
    }

    /// 测试10: 使用自定义并发
    #[test]
    fn test_memory_optimizer_with_concurrency() {
        let numa_config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let concurrency = ConcurrencyConfig::new(16);
        let optimizer = MemoryOptimizer::with_concurrency(numa_config, concurrency);

        let stats = optimizer.get_tlb_stats();
        assert_eq!(stats.hits, 0);
    }
}

// ============================================================================
// 数据结构测试
// ============================================================================

#[cfg(test)]
mod data_structure_tests {
    use super::*;

    /// 测试11: BlockFeatures
    #[test]
    fn test_block_features() {
        let features = BlockFeatures {
            size_bytes: 100,
            branch_count: 5,
            loop_count: 2,
            call_count: 3,
            memory_ops: 20,
            execution_count: 1000,
            execution_time_us: 5000,
        };

        assert_eq!(features.size_bytes, 100);
        assert_eq!(features.branch_count, 5);
    }

    /// 测试12: BlockProfile
    #[test]
    fn test_block_profile() {
        let profile = BlockProfile {
            block_id: 1,
            execution_count: 100,
            total_time_us: 10000,
            branch_hits: 50,
            branch_misses: 10,
            cache_hits: 80,
            cache_misses: 20,
        };

        assert_eq!(profile.block_id, 1);
    }

    /// 测试13: CallProfile
    #[test]
    fn test_call_profile() {
        let profile = CallProfile {
            caller_id: 1,
            callee_id: 2,
            call_count: 50,
            total_time_us: 5000,
        };

        assert_eq!(profile.caller_id, 1);
        assert_eq!(profile.callee_id, 2);
    }

    /// 测试14: CompilationDecision - Tier0
    #[test]
    fn test_compilation_decision_tier0() {
        let decision = CompilationDecision::Tier0;
        assert!(matches!(decision, CompilationDecision::Tier0));
    }

    /// 测试15: CompilationDecision - Tier1
    #[test]
    fn test_compilation_decision_tier1() {
        let decision = CompilationDecision::Tier1;
        assert!(matches!(decision, CompilationDecision::Tier1));
    }

    /// 测试16: CompilationDecision - Tier2
    #[test]
    fn test_compilation_decision_tier2() {
        let decision = CompilationDecision::Tier2;
        assert!(matches!(decision, CompilationDecision::Tier2));
    }
}

// ============================================================================
// 基础集成测试
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// 测试17: 多次地址翻译
    #[test]
    fn test_multiple_translations() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        for i in 0..10 {
            let addr = 0x1000 + (i * 0x1000);
            let result = optimizer.translate(addr);
            assert!(result.is_ok());
        }
    }

    /// 测试18: 多次内存分配
    #[test]
    fn test_multiple_allocations() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        for _ in 0..10 {
            let result = optimizer.allocate(1024);
            assert!(result.is_ok());
        }
    }

    /// 测试19: 大批量地址翻译
    #[test]
    fn test_large_batch_translation() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let addrs: Vec<u64> = (0..50).map(|i| 0x1000 + i * 0x1000).collect();
        let result = optimizer.batch_access(&addrs);
        assert!(result.is_ok());
    }

    /// 测试20: TLB缓存效果
    #[test]
    fn test_tlb_caching() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        // 首次翻译
        let _ = optimizer.translate(0x1000);
        let stats1 = optimizer.get_tlb_stats();

        // 再次翻译相同地址
        let _ = optimizer.translate(0x1000);
        let stats2 = optimizer.get_tlb_stats();

        // 命中或未命中都应该增加
        assert!(stats2.hits + stats2.misses > stats1.hits + stats1.misses);
    }
}

// ============================================================================
// 错误处理测试
// ============================================================================

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    /// 测试21: 无效地址处理
    #[test]
    fn test_invalid_address_handling() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        // 测试0地址 (可能有效也可能无效)
        let result = optimizer.translate(0x0);
        // 只验证不panic
        let _ = result;
    }

    /// 测试22: 空批量处理
    #[test]
    fn test_empty_batch_access() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let addrs: Vec<u64> = vec![];
        let result = optimizer.batch_access(&addrs);
        assert!(result.is_ok());
    }

    /// 测试23: 零大小分配
    #[test]
    fn test_zero_size_allocation() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let result = optimizer.allocate(0);
        // 验证不panic
        let _ = result;
    }

    /// 测试24: 大小分配
    #[test]
    fn test_large_allocation() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let result = optimizer.allocate(1024 * 1024);
        // 验证不panic
        let _ = result;
    }
}

// ============================================================================
// 性能测试基础
// ============================================================================

#[cfg(test)]
mod performance_tests {
    use super::*;

    /// 测试25: 批量操作性能
    #[test]
    fn test_batch_performance() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let addrs: Vec<u64> = (0..100).map(|i| 0x1000 + i * 0x1000).collect();
        let start = std::time::Instant::now();
        let result = optimizer.batch_access(&addrs);
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(elapsed.as_millis() < 1000); // 应该在1秒内完成
    }

    /// 测试26: TLB缓存效率
    #[test]
    fn test_cache_efficiency() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        // 翻译相同地址100次
        for _ in 0..100 {
            let _ = optimizer.translate(0x1000);
        }

        let stats = optimizer.get_tlb_stats();
        let hit_rate = stats.hit_rate();
        assert!(hit_rate > 90.0); // 应该有很高的命中率
    }
}

// ============================================================================
// 并发测试基础
// ============================================================================

#[cfg(test)]
mod concurrency_tests {
    use super::*;

    /// 测试27: 并发配置范围
    #[test]
    fn test_concurrency_config_range() {
        let config1 = ConcurrencyConfig::new(1);
        assert_eq!(config1.max_concurrent, 1);

        let config2 = ConcurrencyConfig::new(512);
        assert_eq!(config2.max_concurrent, 512);
    }

    /// 测试28: 顺序模式
    #[test]
    fn test_sequential_mode() {
        let config = ConcurrencyConfig::sequential();
        assert_eq!(config.max_concurrent, 1);
        assert!(!config.enabled);
    }

    /// 测试29: 并发模式
    #[test]
    fn test_concurrent_mode() {
        let config = ConcurrencyConfig::new(4);
        assert_eq!(config.max_concurrent, 4);
        assert!(config.enabled);
    }

    /// 测试30: 默认配置合理
    #[test]
    fn test_default_config_is_reasonable() {
        let config = ConcurrencyConfig::default();
        assert!(config.max_concurrent > 0);
        assert!(config.max_concurrent <= 512);
    }
}

// ============================================================================
// 统计信息测试
// ============================================================================

#[cfg(test)]
mod statistics_tests {
    use super::*;

    /// 测试31: TLB统计初始值
    #[test]
    fn test_initial_tlb_stats() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let stats = optimizer.get_tlb_stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
    }

    /// 测试32: 统计信息累积
    #[test]
    fn test_stats_accumulation() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        for _ in 0..10 {
            let _ = optimizer.translate(0x1000);
        }

        let stats = optimizer.get_tlb_stats();
        assert!(stats.hits + stats.misses > 0);
    }

    /// 测试33: 命中率计算
    #[test]
    fn test_hit_rate_calculation() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        // 无操作
        let stats = optimizer.get_tlb_stats();
        let hit_rate = stats.hit_rate();
        assert_eq!(hit_rate, 0.0); // 无操作时命中率为0
    }

    /// 测试34: 统计信息不减少
    #[test]
    fn test_stats_never_decrease() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let stats1 = optimizer.get_tlb_stats();
        let _ = optimizer.translate(0x1000);
        let stats2 = optimizer.get_tlb_stats();

        assert!(stats2.hits + stats2.misses >= stats1.hits + stats1.misses);
    }
}

// ============================================================================
// 边界条件测试
// ============================================================================

#[cfg(test)]
mod boundary_tests {
    use super::*;

    /// 测试35: 最小并发数
    #[test]
    fn test_min_concurrency() {
        let config = ConcurrencyConfig::new(1);
        assert_eq!(config.max_concurrent, 1);
    }

    /// 测试36: 最大并发数
    #[test]
    fn test_max_concurrency() {
        let config = ConcurrencyConfig::new(512);
        assert_eq!(config.max_concurrent, 512);
    }

    /// 测试37: 地址范围
    #[test]
    fn test_address_range() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        // 测试不同范围的地址
        let addrs = vec![0x1000, 0x1000000, 0x100000000];
        for addr in addrs {
            let result = optimizer.translate(addr);
            // 只验证不panic
            let _ = result;
        }
    }

    /// 测试38: 分配大小范围
    #[test]
    fn test_allocation_size_range() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        for size in [1, 1024, 1024 * 1024].iter() {
            let result = optimizer.allocate(*size);
            // 只验证不panic
            let _ = result;
        }
    }
}

// ============================================================================
// 枚举测试
// ============================================================================

#[cfg(test)]
mod enum_tests {
    use super::*;

    /// 测试39: CompilationDecision所有变体
    #[test]
    fn test_all_compilation_decisions() {
        let decisions = [
            CompilationDecision::Tier0,
            CompilationDecision::Tier1,
            CompilationDecision::Tier2,
            CompilationDecision::Tier3,
        ];

        for decision in decisions {
            match decision {
                CompilationDecision::Tier0 => {}
                CompilationDecision::Tier1 => {}
                CompilationDecision::Tier2 => {}
                CompilationDecision::Tier3 => {}
            }
        }
    }

    /// 测试40: BlockFeatures字段完整性
    #[test]
    fn test_block_features_fields_complete() {
        let features = BlockFeatures {
            size_bytes: 100,
            branch_count: 10,
            loop_count: 2,
            call_count: 3,
            memory_ops: 20,
            execution_count: 1000,
            execution_time_us: 5000,
        };

        assert!(features.size_bytes > 0);
        assert!(features.branch_count > 0);
        assert!(features.memory_ops > 0);
        assert!(features.execution_count > 0);
        assert!(features.execution_time_us > 0);
    }

    /// 测试41: BlockProfile字段验证
    #[test]
    fn test_block_profile_fields_valid() {
        let profile = BlockProfile {
            block_id: 42,
            execution_count: 100,
            total_time_us: 10000,
            branch_hits: 50,
            branch_misses: 10,
            cache_hits: 80,
            cache_misses: 20,
        };

        assert!(profile.block_id > 0);
        assert!(profile.execution_count > 0);
        assert!(profile.total_time_us > 0);
    }

    /// 测试42: CallProfile字段验证
    #[test]
    fn test_call_profile_fields_valid() {
        let profile = CallProfile {
            caller_id: 1,
            callee_id: 2,
            call_count: 100,
            total_time_us: 5000,
        };

        assert!(profile.caller_id > 0);
        assert!(profile.callee_id > 0);
        assert!(profile.call_count > 0);
        assert!(profile.total_time_us > 0);
    }
}

// ============================================================================
// 组合测试
// ============================================================================

#[cfg(test)]
mod combined_tests {
    use super::*;

    /// 测试43: 分配和翻译
    #[test]
    fn test_allocate_and_translate() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let alloc_result = optimizer.allocate(1024);
        assert!(alloc_result.is_ok());

        let translate_result = optimizer.translate(0x2000);
        assert!(translate_result.is_ok());
    }

    /// 测试44: 批量操作后统计
    #[test]
    fn test_stats_after_batch() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let addrs = vec![0x1000, 0x2000, 0x3000];
        let _ = optimizer.batch_access(&addrs);

        let stats = optimizer.get_tlb_stats();
        assert!(stats.hits + stats.misses > 0);
    }

    /// 测试45: 多次创建optimizer
    #[test]
    fn test_multiple_optimizers() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };

        for _ in 0..5 {
            let optimizer = MemoryOptimizer::new(config);
            let stats = optimizer.get_tlb_stats();
            assert_eq!(stats.hits, 0);
        }
    }
}

// ============================================================================
// 额外验证测试
// ============================================================================

#[cfg(test)]
mod validation_tests {
    use super::*;

    /// 测试46: ConcurrencyConfig验证逻辑
    #[test]
    fn test_config_validation_logic() {
        // 有效配置
        let valid_configs = [1, 8, 16, 64, 256];
        for &max_concurrent in &valid_configs {
            let config = ConcurrencyConfig::new(max_concurrent);
            assert!(config.validate().is_ok());
        }
    }

    /// 测试47: 内存分配一致性
    #[test]
    fn test_allocation_consistency() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let size = 1024;
        let addr1 = optimizer.allocate(size);
        let addr2 = optimizer.allocate(size);

        // 两次分配应该返回不同的地址
        if addr1.is_ok() && addr2.is_ok() {
            assert_ne!(addr1.unwrap(), addr2.unwrap());
        }
    }

    /// 测试48: 地址翻译一致性
    #[test]
    fn test_translation_consistency() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let addr = 0x1000;
        let result1 = optimizer.translate(addr);
        let result2 = optimizer.translate(addr);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        // 相同地址应该翻译到相同物理地址
        assert_eq!(result1.unwrap(), result2.unwrap());
    }

    /// 测试49: TLB缓存一致性
    #[test]
    fn test_cache_consistency() {
        let config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let optimizer = MemoryOptimizer::new(config);

        let stats1 = optimizer.get_tlb_stats();
        let _ = optimizer.translate(0x1000);
        let stats2 = optimizer.get_tlb_stats();

        // 统计信息应该更新
        assert!(stats2.hits + stats2.misses >= stats1.hits + stats1.misses);
    }

    /// 测试50: 默认配置可用性
    #[test]
    fn test_default_configs_are_usable() {
        let numa_config = vm_optimizers::NumaConfig {
            num_nodes: 4,
            mem_per_node: 1024 * 1024,
        };
        let concurrency = ConcurrencyConfig::default();

        let optimizer = MemoryOptimizer::with_concurrency(numa_config, concurrency);
        let result = optimizer.translate(0x1000);

        assert!(result.is_ok());
    }
}
