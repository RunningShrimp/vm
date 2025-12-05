//! JIT统计信息模块
//!
//! 包含执行统计、编译统计等统计相关功能

use crate::BlockStats;
use vm_core::GuestAddr;
use std::collections::HashMap;

/// 执行统计信息
#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    /// 总编译次数
    pub total_compiled: u64,
    /// 总解释执行次数
    pub total_interpreted: u64,
    /// 块统计信息
    pub block_stats: HashMap<GuestAddr, BlockStats>,
}

impl ExecutionStats {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn record_compiled(&mut self) {
        self.total_compiled += 1;
    }
    
    pub fn record_interpreted(&mut self) {
        self.total_interpreted += 1;
    }
    
    pub fn get_block_stats(&self, pc: GuestAddr) -> Option<&BlockStats> {
        self.block_stats.get(&pc)
    }
    
    pub fn update_block_stats(&mut self, pc: GuestAddr, stats: BlockStats) {
        self.block_stats.insert(pc, stats);
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
        if self.compile_count > 0 {
            self.avg_compile_time_ns = self.total_compile_time_ns / self.compile_count;
        }
    }
    
    pub fn record_failure(&mut self) {
        self.compile_failures += 1;
    }
}


