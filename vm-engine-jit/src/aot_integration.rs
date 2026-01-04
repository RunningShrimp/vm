//! AOT集成占位实现

use crate::aot_format::{AotError, AotHeader, AotImage};
use crate::aot_loader::{AotConfig, AotLoader};
use crate::hybrid_executor::{HybridConfig, HybridExecutor};
use std::time::SystemTime;

pub struct AotIntegration;

/// 创建混合执行器
///
/// 根据提供的配置创建混合执行器，如果未提供配置则使用默认配置。
pub fn create_hybrid_executor(config: Option<HybridConfig>) -> Result<HybridExecutor, AotError> {
    let config = config.unwrap_or_default();
    HybridExecutor::new(config)
}

/// 创建测试用AOT镜像
///
/// 创建一个简单的AOT镜像用于测试目的。
pub fn create_test_aot_image() -> Result<AotImage, AotError> {
    let header = AotHeader {
        magic: *b"AOT\0",
        version: 1,
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        entry_point: 0x1000,
        code_size: 4096,
        data_size: 0,
    };

    // 简单的RISC-V指令: addi x0, x0, 0
    let code = vec![0x13, 0x05, 0xa0, 0x00];

    Ok(AotImage::new(header, code, vec![]))
}

/// 初始化AOT加载器
///
/// 创建新的AOT加载器，可选地加载指定路径的缓存。
pub fn init_aot_loader(cache_path: Option<&str>) -> Result<AotLoader, AotError> {
    let loader = AotLoader::new();

    if let Some(path) = cache_path {
        loader.load_cache(path)?;
    }

    Ok(loader)
}

/// 验证AOT配置
///
/// 检查AOT配置的有效性，确保所有参数都在合理范围内。
pub fn validate_aot_config(config: &AotConfig) -> Result<(), AotError> {
    if config.cache_size == 0 {
        return Err(AotError::InvalidConfig("cache_size must be > 0".into()));
    }

    if config.optimization_level > 3 {
        return Err(AotError::InvalidConfig(
            "optimization_level must be <= 3".into(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_hybrid_executor_with_default_config() {
        let result = create_hybrid_executor(None);
        assert!(result.is_ok());

        let executor = result.unwrap();
        assert_eq!(executor.config.jit_threshold, 0);
        assert_eq!(executor.config.aot_threshold, 0);
    }

    #[test]
    fn test_create_hybrid_executor_with_custom_config() {
        let config = HybridConfig {
            jit_threshold: 100,
            aot_threshold: 1000,
            enable_adaptive: true,
        };

        let result = create_hybrid_executor(Some(config));
        assert!(result.is_ok());

        let executor = result.unwrap();
        assert_eq!(executor.config.jit_threshold, 100);
        assert_eq!(executor.config.aot_threshold, 1000);
        assert!(executor.config.enable_adaptive);
    }

    #[test]
    fn test_create_test_aot_image() {
        let result = create_test_aot_image();
        assert!(result.is_ok());

        let image = result.unwrap();
        assert_eq!(image.header.magic, *b"AOT\0");
        assert_eq!(image.header.version, 1);
        assert_eq!(image.header.entry_point, 0x1000);
        assert_eq!(image.header.code_size, 4096);
        assert_eq!(image.code.len(), 4);
        assert_eq!(image.code, vec![0x13, 0x05, 0xa0, 0x00]);
    }

    #[test]
    fn test_init_aot_loader_without_cache() {
        let result = init_aot_loader(None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_aot_loader_with_cache_path() {
        // 测试提供缓存路径的情况（即使路径不存在，函数应该返回Ok）
        let result = init_aot_loader(Some("/tmp/test_cache"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_aot_config_valid() {
        let config = AotConfig::default();
        let result = validate_aot_config(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_aot_config_zero_cache_size() {
        let mut config = AotConfig::default();
        config.cache_size = 0;

        let result = validate_aot_config(&config);
        assert!(result.is_err());

        if let Err(AotError::InvalidConfig(msg)) = result {
            assert!(msg.contains("cache_size must be > 0"));
        } else {
            panic!("Expected InvalidConfig error");
        }
    }

    #[test]
    fn test_validate_aot_config_invalid_optimization_level() {
        let mut config = AotConfig::default();
        config.optimization_level = 4; // 超过最大值3

        let result = validate_aot_config(&config);
        assert!(result.is_err());

        if let Err(AotError::InvalidConfig(msg)) = result {
            assert!(msg.contains("optimization_level must be <= 3"));
        } else {
            panic!("Expected InvalidConfig error");
        }
    }

    #[test]
    fn test_validate_aot_config_boundary_values() {
        // 测试边界值
        let mut config = AotConfig::default();

        // cache_size = 1 应该有效
        config.cache_size = 1;
        assert!(validate_aot_config(&config).is_ok());

        // optimization_level = 0 应该有效
        config.optimization_level = 0;
        assert!(validate_aot_config(&config).is_ok());

        // optimization_level = 3 应该有效（最大值）
        config.optimization_level = 3;
        assert!(validate_aot_config(&config).is_ok());
    }
}
