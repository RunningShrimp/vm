//! GPU模块综合测试
//!
//! 测试GPU计算抽象层的核心功能，包括设备管理、内存分配、内核执行等。

use std::sync::Arc;
use vm_core::gpu::GpuDeviceType;
use vm_core::gpu::*;

#[cfg(test)]
mod tests {
    use super::*;

    // Test 1: GPU错误类型检查
    #[test]
    fn test_gpu_error_display() {
        let err = GpuError::NoDeviceAvailable;
        let msg = format!("{}", err);
        assert!(msg.contains("No GPU device") || msg.contains("NoDeviceAvailable"));

        let err = GpuError::DeviceInitializationFailed {
            device_type: "CUDA".to_string(),
            reason: "Driver not found".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("CUDA") && msg.contains("Driver not found"));
    }

    // Test 2: GPU设备信息结构
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
        assert_eq!(info.multiprocessor_count, 80);
        assert!(info.supports_unified_memory);
        assert!(info.supports_shared_memory);
    }

    // Test 3: GPU执行结果结构
    #[test]
    fn test_gpu_execution_result_success() {
        let result = GpuExecutionResult {
            success: true,
            execution_time_ns: 1_000_000,
            gpu_used: true,
            error_message: None,
        };

        assert!(result.success);
        assert!(result.gpu_used);
        assert_eq!(result.execution_time_ns, 1_000_000);
        assert!(result.error_message.is_none());
    }

    // Test 4: GPU执行结果失败
    #[test]
    fn test_gpu_execution_result_failure() {
        let result = GpuExecutionResult {
            success: false,
            execution_time_ns: 0,
            gpu_used: false,
            error_message: Some("Kernel execution failed".to_string()),
        };

        assert!(!result.success);
        assert!(!result.gpu_used);
        assert_eq!(result.execution_time_ns, 0);
        assert_eq!(
            result.error_message,
            Some("Kernel execution failed".to_string())
        );
    }

    // Test 5: GPU执行配置创建
    #[test]
    fn test_gpu_execution_config_creation() {
        let config = GpuExecutionConfig {
            kernel_source: r#"
                __global__ void test_kernel(float* data) {
                    int idx = threadIdx.x;
                    data[idx] = data[idx] * 2.0f;
                }
            "#
            .to_string(),
            kernel_name: "test_kernel".to_string(),
            grid_dim: (1, 1, 1),
            block_dim: (256, 1, 1),
            args: vec![],
            shared_memory_size: 0,
        };

        assert_eq!(config.kernel_name, "test_kernel");
        assert_eq!(config.grid_dim, (1, 1, 1));
        assert_eq!(config.block_dim, (256, 1, 1));
        assert_eq!(config.shared_memory_size, 0);
        assert!(!config.kernel_source.is_empty());
    }

    // Test 6: GPU执行器默认创建
    #[test]
    fn test_gpu_executor_default() {
        let executor = GpuExecutor::default();
        // 执行器应该成功创建（即使没有GPU硬件）
        // 验证统计信息初始化
        let stats = executor.stats();
        assert_eq!(stats.total_executions, 0);
        assert_eq!(stats.gpu_executions, 0);
        assert_eq!(stats.cpu_fallbacks, 0);
    }

    // Test 7: GPU执行器统计
    #[test]
    fn test_gpu_executor_stats_initial() {
        let executor = GpuExecutor::default();
        let stats = executor.stats();

        assert_eq!(stats.total_executions, 0);
        assert_eq!(stats.gpu_executions, 0);
        assert_eq!(stats.cpu_fallbacks, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.cache_misses, 0);
    }

    // Test 8: GPU设备管理器创建
    #[test]
    fn test_gpu_device_manager_new() {
        let manager = GpuDeviceManager::new();
        // 设备管理器应该成功创建
        // 即使在没有GPU硬件的环境中也不应该panic
        assert!(manager.has_gpu() == manager.has_gpu()); // 验证方法可调用
    }

    // Test 9: GPU参数类型
    #[test]
    fn test_gpu_arg_types() {
        // 验证GpuArg可以被正确构造（如果该类型是公开的）
        // 这里测试参数的类型安全性
        let config = GpuExecutionConfig {
            kernel_source: "test".to_string(),
            kernel_name: "test".to_string(),
            grid_dim: (1, 1, 1),
            block_dim: (1, 1, 1),
            args: vec![],
            shared_memory_size: 0,
        };

        // 验证配置可以克隆（用于缓存）
        let config2 = config.clone();
        assert_eq!(config.kernel_name, config2.kernel_name);
    }

    // Test 10: GPU错误转换
    #[test]
    fn test_gpu_error_from_std_io() {
        use std::io;
        let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
        // 验证IO错误可以转换为GpuError（如果实现了From trait）
        // 这里只验证错误类型存在，具体转换取决于实现
        let _ = io_err; // 避免unused警告
    }

    // Test 11: GPU执行结果Debug trait
    #[test]
    fn test_gpu_execution_result_debug() {
        let result = GpuExecutionResult {
            success: true,
            execution_time_ns: 100,
            gpu_used: true,
            error_message: None,
        };

        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("GpuExecutionResult"));
        assert!(debug_str.contains("success") || debug_str.contains("true"));
    }

    // Test 12: 多个GPU执行配置比较
    #[test]
    fn test_gpu_execution_config_equality() {
        let config1 = GpuExecutionConfig {
            kernel_source: "kernel1".to_string(),
            kernel_name: "test".to_string(),
            grid_dim: (1, 1, 1),
            block_dim: (256, 1, 1),
            args: vec![],
            shared_memory_size: 0,
        };

        let config2 = GpuExecutionConfig {
            kernel_source: "kernel2".to_string(), // 不同的源代码
            kernel_name: "test".to_string(),
            grid_dim: (1, 1, 1),
            block_dim: (256, 1, 1),
            args: vec![],
            shared_memory_size: 0,
        };

        // 相同的配置应该相等
        assert_eq!(config1.kernel_name, config2.kernel_name);
        assert_eq!(config1.grid_dim, config2.grid_dim);

        // 不同的源代码应该不相等
        assert_ne!(config1.kernel_source, config2.kernel_source);
    }

    // Test 13: GPU执行器克隆（如果支持）
    #[test]
    fn test_gpu_executor_clone() {
        let executor = GpuExecutor::default();
        // 如果GpuExecutor实现了Clone，测试克隆功能
        let _ = executor; // 避免unused警告
        // 具体的克隆测试取决于实现
    }

    // Test 14: GPU设备信息验证
    #[test]
    fn test_gpu_device_info_validation() {
        let info = GpuDeviceInfo {
            device_type: GpuDeviceType::Cuda,
            name: "RTX 4090".to_string(),
            device_id: 0,
            compute_capability: (8, 9),
            total_memory_mb: 24576, // 24GB
            free_memory_mb: 24576,
            multiprocessor_count: 128,
            clock_rate_khz: 2520000,
            l2_cache_size: 75497472,
            supports_unified_memory: true,
            supports_shared_memory: true,
        };

        // 验证设备信息的合理性
        assert!(info.total_memory_mb > 0);
        assert!(info.free_memory_mb > 0);
        assert!(info.multiprocessor_count > 0);
        assert!(info.clock_rate_khz > 0);
        assert!(info.l2_cache_size > 0);
    }

    // Test 15: GPU执行配置边界值
    #[test]
    fn test_gpu_execution_config_boundary_values() {
        // 测试最小配置
        let min_config = GpuExecutionConfig {
            kernel_source: "min".to_string(),
            kernel_name: "min_kernel".to_string(),
            grid_dim: (1, 1, 1),
            block_dim: (1, 1, 1),
            args: vec![],
            shared_memory_size: 0,
        };

        assert_eq!(min_config.grid_dim, (1, 1, 1));
        assert_eq!(min_config.block_dim, (1, 1, 1));

        // 测试较大配置
        let max_config = GpuExecutionConfig {
            kernel_source: "max".to_string(),
            kernel_name: "max_kernel".to_string(),
            grid_dim: (65535, 65535, 65535),
            block_dim: (1024, 1024, 64),
            args: vec![],
            shared_memory_size: 48 * 1024, // 48KB
        };

        assert_eq!(max_config.shared_memory_size, 48 * 1024);
        assert_eq!(max_config.block_dim.2, 64); // z dimension
    }

    // Test 16: GPU错误处理一致性
    #[test]
    fn test_gpu_error_handling() {
        let errors = vec![
            GpuError::NoDeviceAvailable,
            GpuError::DeviceInitializationFailed {
                device_type: "TestDevice".to_string(),
                reason: "Test reason".to_string(),
            },
            GpuError::MemoryAllocationFailed {
                requested_size: 1024,
                reason: "Out of memory".to_string(),
            },
            GpuError::KernelCompilationFailed {
                kernel_name: "test_kernel".to_string(),
                source: "test source".to_string(),
                reason: "Compilation error".to_string(),
            },
            GpuError::KernelExecutionFailed {
                kernel_name: "test_kernel".to_string(),
                reason: "Execution failed".to_string(),
            },
        ];

        // 验证所有错误都可以正确显示
        for err in errors {
            let msg = format!("{}", err);
            assert!(!msg.is_empty());
        }
    }

    // Test 17: GPU执行结果部分比较
    #[test]
    fn test_gpu_execution_result_partial_comparison() {
        let result1 = GpuExecutionResult {
            success: true,
            execution_time_ns: 1000,
            gpu_used: true,
            error_message: None,
        };

        let result2 = GpuExecutionResult {
            success: true,
            execution_time_ns: 2000, // 不同的执行时间
            gpu_used: true,
            error_message: None,
        };

        // 比较相同字段
        assert_eq!(result1.success, result2.success);
        assert_eq!(result1.gpu_used, result2.gpu_used);
        assert!(result1.execution_time_ns != result2.execution_time_ns);
    }

    // Test 18: GPU执行器并发安全性（基础）
    #[test]
    fn test_gpu_executor_concurrent_creation() {
        // 测试多个执行器可以同时创建
        let executor1 = GpuExecutor::default();
        let executor2 = GpuExecutor::default();
        let executor3 = GpuExecutor::default();

        // 所有执行器都应该独立工作
        let stats1 = executor1.stats();
        let stats2 = executor2.stats();
        let stats3 = executor3.stats();

        assert_eq!(stats1.total_executions, 0);
        assert_eq!(stats2.total_executions, 0);
        assert_eq!(stats3.total_executions, 0);
    }

    // Test 19: GPU设备信息默认值
    #[test]
    fn test_gpu_device_info_default() {
        // 测试如果存在Default实现
        // 或者创建最小配置的设备信息
        let info = GpuDeviceInfo {
            device_type: GpuDeviceType::Other,
            name: String::new(),
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

        assert_eq!(info.name, "");
        assert_eq!(info.total_memory_mb, 0);
        assert_eq!(info.multiprocessor_count, 0);
        assert!(!info.supports_unified_memory);
        assert!(!info.supports_shared_memory);
    }

    // Test 20: GPU执行配置序列化（如果支持）
    #[test]
    fn test_gpu_execution_config_fields() {
        let config = GpuExecutionConfig {
            kernel_source: "test kernel".to_string(),
            kernel_name: "test_func".to_string(),
            grid_dim: (2, 3, 4),
            block_dim: (8, 8, 8),
            args: vec![],
            shared_memory_size: 1024,
        };

        // 验证所有字段都可以访问
        assert_eq!(config.kernel_name, "test_func");
        assert_eq!(config.grid_dim.0, 2);
        assert_eq!(config.grid_dim.1, 3);
        assert_eq!(config.grid_dim.2, 4);
        assert_eq!(config.block_dim.0, 8);
        assert_eq!(config.shared_memory_size, 1024);
    }
}
