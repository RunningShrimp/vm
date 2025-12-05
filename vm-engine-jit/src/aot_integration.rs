//! AOT 编译集成辅助模块
//!
//! 提供 AOT 加载器初始化、配置验证等集成功能

use std::io;
use std::path::Path;
use std::sync::Arc;

use crate::aot_loader::AotLoader;
use vm_core::{AotConfig, VmConfig};

/// 根据配置初始化 AOT 加载器
///
/// # 参数
/// - `config`: 虚拟机配置，包含 AOT 配置信息
///
/// # 返回
/// - `Ok(Some(loader))`: 如果 AOT 已启用且镜像路径有效，返回加载器
/// - `Ok(None)`: 如果 AOT 未启用或未配置镜像路径
/// - `Err(e)`: 如果加载镜像时出错
pub fn init_aot_loader(config: &VmConfig) -> io::Result<Option<Arc<AotLoader>>> {
    if !config.aot.enable_aot {
        return Ok(None);
    }

    if let Some(ref path) = config.aot.aot_image_path {
        let loader = AotLoader::from_file(path)?;
        tracing::info!("AOT loader initialized from: {}", path);
        Ok(Some(Arc::new(loader)))
    } else {
        tracing::debug!("AOT enabled but no image path provided");
        Ok(None)
    }
}

/// 验证 AOT 配置的有效性
///
/// # 参数
/// - `config`: AOT 配置
///
/// # 返回
/// - `Ok(())`: 配置有效
/// - `Err(msg)`: 配置无效，返回错误消息
pub fn validate_aot_config(config: &AotConfig) -> Result<(), String> {
    if !config.enable_aot {
        return Ok(());
    }

    if let Some(ref path) = config.aot_image_path {
        if !Path::new(path).exists() {
            return Err(format!("AOT image file not found: {}", path));
        }
    }

    if config.aot_hotspot_threshold == 0 {
        return Err("AOT hotspot threshold must be greater than 0".to_string());
    }

    Ok(())
}

/// 创建测试用的 AOT 镜像
///
/// 用于测试和开发目的
pub fn create_test_aot_image() -> crate::aot_format::AotImage {
    use crate::aot_format::{AotImage, SymbolType};

    let mut image = AotImage::new();

    // 添加一些测试代码块
    for i in 0..5 {
        let pc = 0x1000 + i * 0x100;
        let code = vec![0x90; 8 + i * 4]; // NOP 指令
        image.add_code_block(pc.try_into().unwrap(), &code, 1);

        // 添加符号
        let symbol_name = format!("test_block_{}", i);
        image.add_symbol(
            symbol_name,
            image.code_section.len() as u64 - code.len() as u64,
            code.len() as u32,
            SymbolType::BlockLabel,
        );
    }

    image
}

/// 创建混合执行器（集成AOT和JIT）
///
/// # 参数
/// - `config`: 虚拟机配置
///
/// # 返回
/// - `Ok(executor)`: 混合执行器
/// - `Err(e)`: 如果初始化失败
pub fn create_hybrid_executor(
    config: &vm_core::VmConfig,
) -> std::io::Result<crate::hybrid_executor::HybridExecutor> {
    use crate::hybrid_executor::HybridExecutor;

    let aot_loader = init_aot_loader(config)?;

    // 使用默认配置创建执行器
    // 注意：如果需要自定义配置，可以在AotConfig中添加相应字段
    let executor = HybridExecutor::new(aot_loader);

    Ok(executor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::{ExecMode, GuestArch, VmConfig};

    #[test]
    fn test_validate_aot_config_disabled() {
        let config = AotConfig::default();
        assert!(validate_aot_config(&config).is_ok());
    }

    #[test]
    fn test_validate_aot_config_invalid_threshold() {
        let mut config = AotConfig::default();
        config.enable_aot = true;
        config.aot_hotspot_threshold = 0;

        assert!(validate_aot_config(&config).is_err());
    }

    #[test]
    fn test_create_test_aot_image() {
        let image = create_test_aot_image();
        assert_eq!(image.code_blocks.len(), 5);
        assert!(image.code_section.len() > 0);
    }

    #[test]
    fn test_init_aot_loader_disabled() {
        let mut config = VmConfig::default();
        config.aot.enable_aot = false;

        let result = init_aot_loader(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_init_aot_loader_no_path() {
        let mut config = VmConfig::default();
        config.aot.enable_aot = true;
        config.aot.aot_image_path = None;

        let result = init_aot_loader(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
