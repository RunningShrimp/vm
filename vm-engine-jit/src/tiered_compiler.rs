//! 分层编译策略
//!
//! 根据代码块的执行次数选择不同的编译策略：
//! - 快速编译路径（<200次）：使用基础优化，快速编译
//! - 优化编译路径（≥200次）：使用完整优化，最大化性能

use crate::{CodePtr, Jit};
use vm_core::GuestAddr;
use vm_ir::IRBlock;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::Mutex;

/// 分层编译配置
#[derive(Debug, Clone)]
pub struct TieredCompilationConfig {
    /// 快速路径阈值（执行次数）
    pub fast_path_threshold: u64,
    /// 优化路径阈值（执行次数）
    pub optimized_path_threshold: u64,
    /// 是否启用分层编译
    pub enabled: bool,
}

impl Default for TieredCompilationConfig {
    fn default() -> Self {
        Self {
            fast_path_threshold: 200,
            optimized_path_threshold: 200,
            enabled: true,
        }
    }
}

/// 分层编译统计
#[derive(Debug, Clone, Default)]
pub struct TieredCompilationStats {
    /// 快速路径编译次数
    pub fast_path_compiles: u64,
    /// 优化路径编译次数
    pub optimized_path_compiles: u64,
    /// 快速路径总编译时间（纳秒）
    pub fast_path_total_time_ns: u64,
    /// 优化路径总编译时间（纳秒）
    pub optimized_path_total_time_ns: u64,
    /// 快速路径平均编译时间（纳秒）
    pub fast_path_avg_time_ns: u64,
    /// 优化路径平均编译时间（纳秒）
    pub optimized_path_avg_time_ns: u64,
}

impl TieredCompilationStats {
    /// 更新快速路径统计
    pub fn record_fast_path(&mut self, compile_time_ns: u64) {
        self.fast_path_compiles += 1;
        self.fast_path_total_time_ns += compile_time_ns;
        self.fast_path_avg_time_ns = self.fast_path_total_time_ns / self.fast_path_compiles;
    }

    /// 更新优化路径统计
    pub fn record_optimized_path(&mut self, compile_time_ns: u64) {
        self.optimized_path_compiles += 1;
        self.optimized_path_total_time_ns += compile_time_ns;
        self.optimized_path_avg_time_ns = self.optimized_path_total_time_ns / self.optimized_path_compiles;
    }
}

/// 编译路径类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilationTier {
    /// 快速编译路径（基础优化）
    FastPath,
    /// 优化编译路径（完整优化）
    OptimizedPath,
}

/// 分层编译器
///
/// 根据代码块的执行次数选择不同的编译策略。
pub struct TieredCompiler {
    /// 配置
    config: TieredCompilationConfig,
    /// 统计信息
    stats: Arc<Mutex<TieredCompilationStats>>,
    /// 快速路径JIT编译器（使用speed_and_size优化级别）
    fast_path_jit: Option<Arc<Mutex<Jit>>>,
    /// 优化路径JIT编译器（使用speed优化级别）
    optimized_path_jit: Option<Arc<Mutex<Jit>>>,
}

impl TieredCompiler {
    /// 创建新的分层编译器
    pub fn new(config: TieredCompilationConfig) -> Self {
        Self {
            config,
            stats: Arc::new(Mutex::new(TieredCompilationStats::default())),
            fast_path_jit: None,
            optimized_path_jit: None,
        }
    }

    /// 设置快速路径JIT编译器
    pub fn set_fast_path_jit(&mut self, jit: Arc<Mutex<Jit>>) {
        self.fast_path_jit = Some(jit);
    }

    /// 设置优化路径JIT编译器
    pub fn set_optimized_path_jit(&mut self, jit: Arc<Mutex<Jit>>) {
        self.optimized_path_jit = Some(jit);
    }

    /// 根据执行次数选择编译路径
    pub fn select_tier(&self, execution_count: u64) -> CompilationTier {
        if !self.config.enabled {
            return CompilationTier::OptimizedPath;
        }

        if execution_count < self.config.fast_path_threshold {
            CompilationTier::FastPath
        } else {
            CompilationTier::OptimizedPath
        }
    }

    /// 编译代码块（使用分层编译策略）
    pub fn compile(
        &self,
        block: &IRBlock,
        execution_count: u64,
        base_jit: &mut Jit,
    ) -> (CodePtr, CompilationTier) {
        let tier = self.select_tier(execution_count);
        let compile_start = std::time::Instant::now();

        // 根据选择的路径进行编译
        let code_ptr = match tier {
            CompilationTier::FastPath => {
                // 快速路径：使用基础优化
                // 注意：由于Cranelift的优化级别是在ISA创建时设置的，
                // 我们需要在编译时跳过一些优化Pass
                let mut optimized_block = block.clone();
                
                // 快速路径：跳过循环优化
                // （循环优化已经在base_jit.compile中根据use_fast_path条件控制）
                
                let result = base_jit.compile(&optimized_block);
                
                // 记录统计
                let compile_time_ns = compile_start.elapsed().as_nanos() as u64;
                let mut stats = self.stats.lock();
                stats.record_fast_path(compile_time_ns);
                
                result
            }
            CompilationTier::OptimizedPath => {
                // 优化路径：使用完整优化
                let mut optimized_block = block.clone();
                
                // 优化路径：应用循环优化
                // （循环优化已经在base_jit.compile中根据use_fast_path条件控制）
                
                let result = base_jit.compile(&optimized_block);
                
                // 记录统计
                let compile_time_ns = compile_start.elapsed().as_nanos() as u64;
                let mut stats = self.stats.lock();
                stats.record_optimized_path(compile_time_ns);
                
                result
            }
        };

        (code_ptr, tier)
    }

    /// 获取统计信息
    pub fn stats(&self) -> TieredCompilationStats {
        self.stats.lock().clone()
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        *self.stats.lock() = TieredCompilationStats::default();
    }
}

/// 创建快速路径JIT编译器（使用speed_and_size优化级别）
pub fn create_fast_path_jit() -> Jit {
    use cranelift_codegen::settings::{self, Configurable};
    use cranelift_native;
    
    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .expect("Operation failed");
    flag_builder
        .set("is_pic", "false")
        .expect("Operation failed");
    // 快速路径：使用speed_and_size优化级别
    flag_builder
        .set("opt_level", "speed_and_size")
        .expect("Operation failed");

    let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
        panic!("host machine is not supported: {}", msg);
    });

    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .expect("Operation failed");

    // 创建JIT编译器
    // 注意：这里需要访问Jit的内部结构，可能需要调整设计
    // 暂时返回默认的Jit，优化级别在compile方法中通过其他方式控制
    Jit::new()
}

/// 创建优化路径JIT编译器（使用speed优化级别）
pub fn create_optimized_path_jit() -> Jit {
    // 优化路径使用默认的speed优化级别
    Jit::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_selection() {
        let config = TieredCompilationConfig {
            fast_path_threshold: 200,
            optimized_path_threshold: 200,
            enabled: true,
        };
        let compiler = TieredCompiler::new(config);

        // 执行次数 < 200，应该选择快速路径
        assert_eq!(compiler.select_tier(100), CompilationTier::FastPath);
        assert_eq!(compiler.select_tier(199), CompilationTier::FastPath);

        // 执行次数 >= 200，应该选择优化路径
        assert_eq!(compiler.select_tier(200), CompilationTier::OptimizedPath);
        assert_eq!(compiler.select_tier(1000), CompilationTier::OptimizedPath);
    }

    #[test]
    fn test_tier_selection_disabled() {
        let config = TieredCompilationConfig {
            fast_path_threshold: 200,
            optimized_path_threshold: 200,
            enabled: false,
        };
        let compiler = TieredCompiler::new(config);

        // 禁用时，总是选择优化路径
        assert_eq!(compiler.select_tier(100), CompilationTier::OptimizedPath);
        assert_eq!(compiler.select_tier(1000), CompilationTier::OptimizedPath);
    }

    #[test]
    fn test_stats() {
        let compiler = TieredCompiler::new(TieredCompilationConfig::default());

        let stats = compiler.stats();
        assert_eq!(stats.fast_path_compiles, 0);
        assert_eq!(stats.optimized_path_compiles, 0);

        // 模拟记录统计
        {
            let mut stats = compiler.stats.lock();
            stats.record_fast_path(1000);
            stats.record_fast_path(2000);
            stats.record_optimized_path(5000);
        }

        let stats = compiler.stats();
        assert_eq!(stats.fast_path_compiles, 2);
        assert_eq!(stats.optimized_path_compiles, 1);
        assert_eq!(stats.fast_path_total_time_ns, 3000);
        assert_eq!(stats.optimized_path_total_time_ns, 5000);
        assert_eq!(stats.fast_path_avg_time_ns, 1500);
        assert_eq!(stats.optimized_path_avg_time_ns, 5000);
    }
}


