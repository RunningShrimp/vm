// Virtual Machine Common Services
//
// 本模块提供VM的通用服务，包括：
// - 事件系统（Event System）
// - 日志系统（Logging）
// - 配置管理（Configuration Management）
// - 错误处理（Error Handling）

pub mod lockfree;

// Placeholder for future modules (event, logging, config, error)
// These will be created when needed during development
// pub mod event;
// pub mod logging;
// pub mod config;
// pub mod error;

// 重新导出主要类型
// pub use event::{VmEvent, EventBus};
// pub use logging::{VmLogger, LogLevel};
// pub use config::{VmConfig, ConfigManager};
// pub use error::{VmError, VmResult};
pub use lockfree::{hash_table::*, queue::*, state_management::*};

/// VM通用库版本
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// VM通用库描述
pub const DESCRIPTION: &str =
    "Virtual Machine Common Services - Event System, Logging, Configuration, and Error Handling";

/// VM通用库构建信息
#[derive(Debug)]
pub struct BuildInfo {
    pub version: &'static str,
    pub build_time: &'static str,
    pub git_commit: Option<&'static str>,
    pub rust_version: &'static str,
}

impl Default for BuildInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildInfo {
    pub fn new() -> Self {
        Self {
            version: VERSION,
            build_time: option_env!("VERGEN_BUILD_TIME").unwrap_or("unknown"),
            git_commit: option_env!("VERGEN_GIT_COMMIT"),
            rust_version: env!("CARGO_PKG_VERSION"),
        }
    }
}

/// 获取构建信息
pub fn get_build_info() -> BuildInfo {
    BuildInfo::new()
}

// /// VM通用库初始化
// /// This function will be enabled when required modules are implemented
// pub fn init() -> VmResult<()> {
//     // 初始化日志系统
//     logging::init();
//
//     // 初始化配置管理器
//     config::init()?;
//
//     // 初始化事件总线
//     event::init();
//
//     Ok(())
// }
// pub fn init() -> VmResult<()> {
//     // 初始化日志系统
//     logging::init();
//
//     // 初始化配置管理器
//     config::init()?;
//
//     // 初始化事件总线
//     event::init();
//
//     Ok(())
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
        println!("VM Common version: {}", VERSION);
    }

    #[test]
    fn test_build_info() {
        let info = BuildInfo::new();
        assert!(!info.version.is_empty());
        println!("Build version: {}", info.version);
    }
}
