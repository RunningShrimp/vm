//! WGPU 后端重导出
//!
//! 从 gpu_virt 模块重导出 WGPU 相关类型
pub use crate::gpu_virt::{GpuBackend as GpuBackendTrait, GpuStats, WgpuBackend};
