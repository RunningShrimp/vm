//! # VM Engine - 统一虚拟机执行引擎
//!
//! 本模块统一了所有执行引擎，包括：
//! - JIT 编译引擎 ([`jit`])
//! - 解释器引擎 ([`interpreter`])
//! - 执行器 ([`executor`])
//!
//! ## 模块结构
//!
//! - [`jit`]: Just-In-Time 编译引擎，提供高性能的代码生成和执行
//! - [`interpreter`]: 解释器引擎，提供直接的指令解释执行
//! - [`executor`]: 执行器抽象和通用执行框架
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use vm_engine::jit::Jit;
//! use vm_engine::interpreter::Interpreter;
//!
//! // 使用 JIT 引擎
//! let jit = Jit::new(config);
//! jit.execute(&mut vm_state)?;
//!
//! // 使用解释器引擎
//! let interpreter = Interpreter::new(config);
//! interpreter.execute(&mut vm_state)?;
//! ```

pub mod jit;
pub mod interpreter;
pub mod executor;

// Common engine types
pub type EngineResult<T> = Result<T, EngineError>;

/// Unified engine error type
pub use vm_core::foundation::VmError as EngineError;
