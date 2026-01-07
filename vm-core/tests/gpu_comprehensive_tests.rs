//! GPU模块基础测试
//!
//! 测试GPU计算抽象层的基础功能

use vm_core::gpu::{
    device::{GpuArg, GpuDeviceInfo, GpuDeviceManager, GpuDeviceType},
    error::GpuError,
    executor::{GpuExecutionConfig, GpuExecutor, GpuExecutorConfig, GpuExecutorStats},
};

#[cfg(test)]
mod gpu_basic_tests {
    use super::*;

    // Test 1: GPU设备类型枚举
    #[test]
    fn test_gpu_device_type() {
        let cuda = GpuDeviceType::Cuda;
        let rocm = GpuDeviceType::Rocm;
        let other = GpuDeviceType::Other;

        // 验证设备类型可以复制和比较
        assert_eq!(cuda, cuda);
        assert_ne!(cuda, rocm);
        assert_ne!(cuda, other);
    }

    // Test 2: GPU设备信息创建
    #[test]
    fn test_gpu_device_info_creation() {
        let info = GpuDeviceInfo {
            device_type: GpuDeviceType::Cuda,
            name: "Test GPU".to_string(),
            device_id: 0,
            compute_capability: (8, 0),
            total_memory_mb: 8192,
            free_memory_mb: 8192,
            multiprocessor_count: 80,
            clock_rate_khz: 1695000,
            l2_cache_size: 4194304,
            supports_unified_memory: true,
            supports_shared_memory: true,
        };

        assert_eq!(info.name, "Test GPU");
        assert_eq!(info.total_memory_mb, 8192);
        assert!(info.supports_unified_memory);
        assert!(info.supports_shared_memory);
    }

    // Test 3: GPU执行配置创建
    #[test]
    fn test_gpu_execution_config() {
        let config = GpuExecutionConfig {
            kernel_source: "__global__ void test() {}".to_string(),
            kernel_name: "test".to_string(),
            grid_dim: (1, 1, 1),
            block_dim: (256, 1, 1),
            args: vec![],
            shared_memory_size: 0,
        };

        assert_eq!(config.kernel_name, "test");
        assert_eq!(config.grid_dim, (1, 1, 1));
        assert_eq!(config.block_dim, (256, 1, 1));
    }

    // Test 4: GPU执行器统计初始化
    #[test]
    fn test_gpu_executor_stats_initial() {
        let stats = GpuExecutorStats::default();

        assert_eq!(stats.total_executions, 0);
        assert_eq!(stats.gpu_success_count, 0);
        assert_eq!(stats.gpu_failure_count, 0);
        assert_eq!(stats.cpu_fallback_count, 0);
        assert_eq!(stats.kernel_compilation_count, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.cache_misses, 0);
    }

    // Test 5: GPU执行器统计计算
    #[test]
    fn test_gpu_executor_stats_calculation() {
        let mut stats = GpuExecutorStats::default();

        // 模拟一些执行
        stats.total_executions = 100;
        stats.gpu_success_count = 80;
        stats.gpu_failure_count = 20;
        stats.cpu_fallback_count = 20;

        // GPU成功率应该是 80%
        let success_rate = stats.gpu_success_rate();
        assert!((success_rate - 0.8).abs() < 0.01);

        // 缓存命中率
        stats.cache_hits = 60;
        stats.cache_misses = 40;
        let hit_rate = stats.cache_hit_rate();
        assert!((hit_rate - 0.6).abs() < 0.01);
    }

    // Test 6: GPU参数类型
    #[test]
    fn test_gpu_arg_variants() {
        let args = vec![
            GpuArg::U8(255),
            GpuArg::U32(123456),
            GpuArg::U64(9876543210),
            GpuArg::I32(-123),
            GpuArg::I64(-456),
            GpuArg::F32(3.14),
        ];

        assert_eq!(args.len(), 6);
        // 验证参数可以正确存储
    }

    // Test 7: GPU错误创建
    #[test]
    fn test_gpu_error_creation() {
        let errors = vec![
            GpuError::NoDeviceAvailable,
            GpuError::DeviceInitializationFailed {
                device_type: "CUDA".to_string(),
                reason: "Driver not found".to_string(),
            },
        ];

        // 验证错误可以正确显示
        for err in errors {
            let msg = format!("{}", err);
            assert!(!msg.is_empty());
        }
    }

    // Test 8: GPU执行器配置默认值
    #[test]
    fn test_gpu_executor_config_default() {
        let config = GpuExecutorConfig::default();

        assert!(config.enable_kernel_cache);
        assert_eq!(config.max_cache_size, 100);
        assert!(config.enable_performance_monitoring);
        assert!(config.enable_cpu_fallback);
        assert_eq!(config.execution_timeout_secs, 30);
    }

    // Test 9: GPU执行器克隆
    #[test]
    fn test_gpu_executor_config_clone() {
        let config1 = GpuExecutorConfig::default();
        let config2 = config1.clone();

        assert_eq!(config1.enable_kernel_cache, config2.enable_kernel_cache);
        assert_eq!(config1.max_cache_size, config2.max_cache_size);
    }

    // Test 10: GPU设备信息边界值
    #[test]
    fn test_gpu_device_info_boundaries() {
        let min_info = GpuDeviceInfo {
            device_type: GpuDeviceType::Other,
            name: "".to_string(),
            device_id: -1,
            compute_capability: (0, 0),
            total_memory_mb: 0,
            free_memory_mb: 0,
            multiprocessor_count: 0,
            clock_rate_khz: 0,
            l2_cache_size: 0,
            supports_unified_memory: false,
            supports_shared_memory: false,
        };

        assert_eq!(min_info.name, "");
        assert_eq!(min_info.total_memory_mb, 0);
    }

    // Test 11: GPU执行配置网格维度
    #[test]
    fn test_gpu_execution_config_grid_dimensions() {
        let config = GpuExecutionConfig {
            kernel_source: "test".to_string(),
            kernel_name: "test_kernel".to_string(),
            grid_dim: (256, 2, 1),
            block_dim: (512, 1, 1),
            args: vec![],
            shared_memory_size: 1024,
        };

        assert_eq!(config.grid_dim.0, 256);
        assert_eq!(config.grid_dim.1, 2);
        assert_eq!(config.grid_dim.2, 1);
        assert_eq!(config.shared_memory_size, 1024);
    }

    // Test 12: GPU执行器配置最大值
    #[test]
    fn test_gpu_execution_config_max_values() {
        let config = GpuExecutionConfig {
            kernel_source: "max".to_string(),
            kernel_name: "max_kernel".to_string(),
            grid_dim: (65535, 65535, 65535),
            block_dim: (1024, 1024, 64),
            args: vec![],
            shared_memory_size: 48 * 1024,
        };

        assert_eq!(config.grid_dim.0, 65535);
        assert_eq!(config.block_dim.2, 64);
        assert_eq!(config.shared_memory_size, 48 * 1024);
    }

    // Test 13: GPU执行器统计字段验证
    #[test]
    fn test_gpu_executor_stats_all_fields() {
        let mut stats = GpuExecutorStats::default();

        stats.total_executions = 1000;
        stats.gpu_success_count = 900;
        stats.gpu_failure_count = 100;
        stats.cpu_fallback_count = 50;
        stats.kernel_compilation_count = 25;
        stats.cache_hits = 150;
        stats.cache_misses = 75;
        stats.total_gpu_time_ns = 5_000_000_000;
        stats.total_cpu_time_ns = 500_000_000;

        assert_eq!(stats.total_executions, 1000);
        assert_eq!(stats.gpu_success_count, 900);
        assert!(stats.total_gpu_time_ns > 0);
        assert!(stats.total_cpu_time_ns > 0);
    }

    // Test 14: GPU缓存命中率计算
    #[test]
    fn test_cache_hit_rate_calculation() {
        let mut stats = GpuExecutorStats::default();

        // 测试零命中率
        stats.cache_hits = 0;
        stats.cache_misses = 100;
        assert_eq!(stats.cache_hit_rate(), 0.0);

        // 测试100%命中率
        stats.cache_hits = 100;
        stats.cache_misses = 0;
        assert_eq!(stats.cache_hit_rate(), 1.0);

        // 测试50%命中率
        stats.cache_hits = 50;
        stats.cache_misses = 50;
        assert_eq!(stats.cache_hit_rate(), 0.5);
    }

    // Test 15: GPU设备类型Debug和Clone
    #[test]
    fn test_gpu_device_type_traits() {
        let cuda = GpuDeviceType::Cuda;
        let cloned = cuda;

        assert_eq!(cuda, cloned);

        let debug_str = format!("{:?}", cuda);
        assert!(debug_str.contains("Cuda") || debug_str.contains("cuda"));
    }

    // Test 16: GPU执行配置Clone验证
    #[test]
    fn test_gpu_execution_config_clone() {
        let config1 = GpuExecutionConfig {
            kernel_source: "test kernel source".to_string(),
            kernel_name: "test".to_string(),
            grid_dim: (10, 20, 30),
            block_dim: (256, 1, 1),
            args: vec![GpuArg::U32(42)],
            shared_memory_size: 512,
        };

        let config2 = config1.clone();

        assert_eq!(config1.kernel_name, config2.kernel_name);
        assert_eq!(config1.grid_dim, config2.grid_dim);
        assert_eq!(config1.args.len(), config2.args.len());
        assert_eq!(config1.shared_memory_size, config2.shared_memory_size);
    }

    // Test 17: GPU执行器默认创建
    #[test]
    fn test_gpu_executor_default_creation() {
        let executor = GpuExecutor::default();
        let stats = executor.stats();

        // 新创建的执行器统计应该全部为0
        assert_eq!(stats.total_executions, 0);
        assert_eq!(stats.gpu_success_count, 0);
        assert_eq!(stats.gpu_failure_count, 0);
    }

    // Test 18: GPU设备管理器API存在性
    #[test]
    fn test_gpu_device_manager_api() {
        let manager = GpuDeviceManager::new();

        // 验证方法可调用（即使没有GPU硬件）
        let has_gpu = manager.has_gpu();
        // 结果可以是true或false，取决于硬件

        // 如果有GPU，测试获取默认设备
        if has_gpu {
            if let Some(device) = manager.default_device() {
                let info = device.device_info();
                assert!(!info.name.is_empty());
            }
        }
    }

    // Test 19: GPU执行器配置范围验证
    #[test]
    fn test_gpu_executor_config_validation() {
        let configs = vec![
            GpuExecutionConfig {
                kernel_source: "min".to_string(),
                kernel_name: "min".to_string(),
                grid_dim: (1, 1, 1),
                block_dim: (1, 1, 1),
                args: vec![],
                shared_memory_size: 0,
            },
            GpuExecutionConfig {
                kernel_source: "mid".to_string(),
                kernel_name: "mid".to_string(),
                grid_dim: (100, 100, 100),
                block_dim: (256, 2, 2),
                args: vec![GpuArg::U32(1), GpuArg::F32(2.0)],
                shared_memory_size: 2048,
            },
        ];

        assert_eq!(configs.len(), 2);
        for config in configs {
            assert!(!config.kernel_name.is_empty());
            assert!(config.shared_memory_size <= 48 * 1024); // 合理的上限
        }
    }

    // Test 20: GPU参数类型覆盖
    #[test]
    fn test_gpu_arg_all_types() {
        let all_args = vec![
            GpuArg::U8(255),
            GpuArg::U32(0xFFFFFFFF),
            GpuArg::U64(0xFFFFFFFFFFFFFFFF),
            GpuArg::I32(-2147483648),
            GpuArg::I64(-9223372036854775808),
            GpuArg::F32(3.14159),
            GpuArg::F64(2.718281828),
        ];

        assert_eq!(all_args.len(), 7);

        // 验证所有参数类型都可以创建
        for arg in all_args {
            let _ = arg; // 避免unused警告
        }
    }
}
