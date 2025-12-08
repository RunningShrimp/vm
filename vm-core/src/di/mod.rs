//! 依赖注入模块
//! 
//! 提供完整的依赖注入框架，包括服务注册、解析、生命周期管理等功能。

pub mod di_builder;
pub mod di_container;
pub mod di_injector;
pub mod di_migration;
pub mod di_mod;
pub mod di_optimization;
pub mod di_registry;
pub mod di_resolver;
pub mod di_service_descriptor;
pub mod di_state_management;

// 重新导出主要类型
pub use di_builder::*;
pub use di_container::*;
pub use di_injector::*;
pub use di_migration::*;
pub use di_mod::*;
pub use di_optimization::*;
pub use di_registry::*;
pub use di_resolver::*;
pub use di_service_descriptor::*;
pub use di_state_management::*;