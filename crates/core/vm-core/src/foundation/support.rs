// Virtual Machine Support Utilities
//
// 本模块提供VM的辅助工具，包括：
// - 工具函数（Utilities）
// - 宏定义（Macros）
// - 测试辅助工具（Testing Helpers）

pub mod utils;
pub mod macros;
pub mod test_helpers;

// 重新导出主要类型
pub use utils::*;
pub use macros::*;
pub use test_helpers::*;

/// VM支持库版本
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// VM支持库描述
pub const DESCRIPTION: &str = "Virtual Machine Support Utilities - Helper Functions, Macros, and Testing Tools";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
        println!("VM Support version: {}", VERSION);
    }
}

