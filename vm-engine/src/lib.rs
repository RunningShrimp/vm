#![allow(dead_code)] // P2: 允许未使用的代码（待后续清理）
#![allow(unused_variables)] // P2: 允许未使用的变量（待后续清理）
#![allow(unreachable_patterns)] // P2: 允许不可达模式（待后续重构）

pub mod jit;
pub mod executor;
pub mod interpreter;

// 重新导出常用类型
pub use jit::{JITCompiler, JITConfig};
pub use executor::{AsyncExecutionContext, ExecutorType};
