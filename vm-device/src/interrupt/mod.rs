//! 中断控制器
//!
//! 本模块包含中断控制器的实现。
//!
//! ## 控制器类型
//!
//! - `clint`: Core Local Interruptor - 核心本地中断控制器
//!   - 定时器中断
//!   - 软件中断
//!
//! - `plic`: Platform Level Interrupt Controller - 平台级中断控制器
//!   - 外部设备中断路由
//!   - 中断优先级管理
//!   - 中断使能控制

// 重导出现有模块
pub use crate::clint::*;
pub use crate::plic::*;

