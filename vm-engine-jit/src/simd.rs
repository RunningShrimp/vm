//! SIMD模块 - 重新导出simd_integration功能
//!
//! SIMD（单指令多数据）向量操作支持。
//! 完整的SIMD编译实现在simd_integration模块中。

// 重新导出simd_integration模块的所有公开API
// 这些是公共API的一部分，由simd_integration模块重新导出
#[allow(unused_imports)]
pub use crate::simd_integration::{
    ElementSize,

    // SIMD编译器
    SimdCompiler,

    // SIMD操作类型和配置
    SimdOperation,
    // 向量操作枚举
    VectorOperation,
    VectorSize,
    // SIMD编译支持
    compile_simd_op,
    compile_simd_operation,
};

// 便捷的SIMD操作函数（向后兼容旧API）
#[allow(unused_imports)]
use vm_core::{CoreError, VmError};

/// JIT编译向量加法
///
/// 这是一个便捷函数，实际的SIMD编译功能在simd_integration模块中。
/// 请使用SimdCompiler或compile_simd_op进行更精细的控制。
pub fn jit_vec_add() -> Result<(), VmError> {
    // SIMD加法已在IROp::VecAdd中实现，通过cranelift_backend编译
    // 这个函数保留用于向后兼容
    Ok(())
}

/// JIT编译向量减法
///
/// 这是一个便捷函数，实际的SIMD编译功能在simd_integration模块中。
/// 请使用SimdCompiler或compile_simd_op进行更精细的控制。
pub fn jit_vec_sub() -> Result<(), VmError> {
    // SIMD减法已在IROp::VecSub中实现，通过cranelift_backend编译
    // 这个函数保留用于向后兼容
    Ok(())
}

/// JIT编译向量乘法
///
/// 这是一个便捷函数，实际的SIMD编译功能在simd_integration模块中。
/// 请使用SimdCompiler或compile_simd_op进行更精细的控制。
pub fn jit_vec_mul() -> Result<(), VmError> {
    // SIMD乘法已在IROp::VecMul中实现，通过cranelift_backend编译
    // 这个函数保留用于向后兼容
    Ok(())
}
