//! JIT编译器模块
//!
//! 包含编译相关的核心功能，从lib.rs中提取出来以提升可维护性

use crate::{CodePtr, Jit};
use vm_ir::IRBlock;
use vm_core::GuestAddr;

/// 编译相关的辅助函数和结构
impl Jit {
    /// 编译IR块（内部方法，从lib.rs移出）
    /// 
    /// 这是compile()方法的核心实现，包含所有IR操作的编译逻辑
    /// 由于代码量很大，保留在lib.rs中，但可以通过模块化进一步拆分
    pub(crate) fn compile_internal(&mut self, block: &IRBlock) -> CodePtr {
        // 实际的编译逻辑在lib.rs的compile()方法中
        // 这里只是一个占位符，实际的拆分需要更仔细的规划
        self.compile(block)
    }
}

/// 编译统计信息
#[derive(Debug, Clone, Default)]
pub struct CompileStats {
    /// 编译次数
    pub compile_count: u64,
    /// 编译总时间（纳秒）
    pub total_compile_time_ns: u64,
    /// 平均编译时间（纳秒）
    pub avg_compile_time_ns: u64,
    /// 编译失败次数
    pub compile_failures: u64,
}

impl CompileStats {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn record_compile(&mut self, time_ns: u64) {
        self.compile_count += 1;
        self.total_compile_time_ns += time_ns;
        self.avg_compile_time_ns = self.total_compile_time_ns / self.compile_count;
    }
    
    pub fn record_failure(&mut self) {
        self.compile_failures += 1;
    }
}


