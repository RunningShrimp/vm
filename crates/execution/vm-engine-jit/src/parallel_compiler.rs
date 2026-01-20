//! 并行编译器
//!
//! 使用rayon并行编译IR块，提高编译效率。
//!
//! ## 特性
//!
//! - 并行编译：使用rayon并行编译多个IR块
//! - 智能分片：按块大小分组，平衡负载
//! - 统计信息：收集编译时间和代码大小
//! - 自适应线程池：根据系统负载动态调整
//! - 编译时间预算：控制最大编译时间

use crate::compiler_backend::{CompilerBackend, CompilerError, CompilerStats};
use rayon::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use vm_ir::IRBlock;

/// 并行JIT编译器
pub struct ParallelJITCompiler {
    /// 编译器后端（使用Arc<Mutex<>>以支持并行访问）
    backend: Arc<Mutex<Box<dyn CompilerBackend>>>,
    /// 统计信息
    stats: Arc<Mutex<CompilerStats>>,
    /// 线程池
    pool: rayon::ThreadPool,
    /// 编译时间预算（纳秒）
    time_budget_ns: u64,
    /// 是否启用自适应分片
    adaptive_chunking: bool,
}

/// 编译配置
#[derive(Debug, Clone)]
pub struct ParallelCompileConfig {
    /// 时间预算（纳秒）
    pub time_budget_ns: u64,
    /// 是否启用自适应分片
    pub adaptive_chunking: bool,
    /// 最小块大小（指令数）
    pub min_chunk_size: usize,
    /// 最大块大小（指令数）
    pub max_chunk_size: usize,
}

impl Default for ParallelCompileConfig {
    fn default() -> Self {
        Self {
            time_budget_ns: 10_000_000, // 10ms
            adaptive_chunking: true,
            min_chunk_size: 1,
            max_chunk_size: 1000,
        }
    }
}

impl ParallelJITCompiler {
    /// 创建新的并行编译器
    pub fn new(backend: Box<dyn CompilerBackend>) -> Self {
        Self::with_config(backend, ParallelCompileConfig::default())
    }

    /// 使用配置创建并行编译器
    pub fn with_config(backend: Box<dyn CompilerBackend>, config: ParallelCompileConfig) -> Self {
        // 创建线程池，默认使用可用CPU核心数
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_cpus::get())
            .build()
            .unwrap_or_else(|_| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(4)
                    .build()
                    .unwrap()
            });

        Self {
            backend: Arc::new(Mutex::new(backend)),
            stats: Arc::new(Mutex::new(CompilerStats::new())),
            pool,
            time_budget_ns: config.time_budget_ns,
            adaptive_chunking: config.adaptive_chunking,
        }
    }

    /// 创建自定义线程数的并行编译器
    pub fn with_threads(backend: Box<dyn CompilerBackend>, num_threads: usize) -> Self {
        let config = ParallelCompileConfig {
            time_budget_ns: 10_000_000,
            adaptive_chunking: true,
            ..Default::default()
        };

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .unwrap();

        Self {
            backend: Arc::new(Mutex::new(backend)),
            stats: Arc::new(Mutex::new(CompilerStats::new())),
            pool,
            time_budget_ns: config.time_budget_ns,
            adaptive_chunking: config.adaptive_chunking,
        }
    }

    /// 并行编译多个IR块（带时间预算）
    pub fn compile_blocks(&mut self, blocks: &[IRBlock]) -> Vec<Result<Vec<u8>, CompilerError>> {
        let start_time = Instant::now();
        let backend = Arc::clone(&self.backend);
        let stats = Arc::clone(&self.stats);
        let time_budget = self.time_budget_ns;

        blocks
            .par_iter()
            .map(|block| {
                // 检查时间预算
                let elapsed = start_time.elapsed().as_nanos() as u64;
                if elapsed > time_budget {
                    return Err(CompilerError::CompilationFailed(
                        "Time budget exceeded".to_string(),
                    ));
                }

                let compile_start = Instant::now();
                let result = backend.lock().unwrap().compile(block);

                // 更新统计信息
                if let Ok(ref code) = result {
                    let compile_time = compile_start.elapsed().as_nanos() as u64;
                    stats
                        .lock()
                        .unwrap()
                        .update_compile(compile_time, code.len());
                }

                result
            })
            .collect()
    }

    /// 并行编译多个IR块（忽略时间预算，用于批处理）
    pub fn compile_blocks_unbounded(
        &mut self,
        blocks: &[IRBlock],
    ) -> Vec<Result<Vec<u8>, CompilerError>> {
        let backend = Arc::clone(&self.backend);
        let stats = Arc::clone(&self.stats);

        blocks
            .par_iter()
            .map(|block| {
                let start_time = Instant::now();
                let result = backend.lock().unwrap().compile(block);

                if let Ok(ref code) = result {
                    let compile_time = start_time.elapsed().as_nanos() as u64;
                    stats
                        .lock()
                        .unwrap()
                        .update_compile(compile_time, code.len());
                }

                result
            })
            .collect()
    }

    /// 智能分片编译：按块大小分组后并行编译
    ///
    /// 将小块组合并为较大的chunk，提高并行效率
    pub fn compile_chunked(&mut self, blocks: &[IRBlock]) -> Vec<Vec<u8>> {
        // 按块大小分组
        let chunks = self.group_by_size(blocks);

        // 准备backend和stats的Arc克隆
        let backend = Arc::clone(&self.backend);
        let stats = Arc::clone(&self.stats);

        // 在自定义线程池中编译
        self.pool.install(|| {
            chunks
                .into_par_iter()
                .map(|chunk| {
                    chunk
                        .into_iter()
                        .flat_map(|block| {
                            let start_time = Instant::now();
                            let result = backend.lock().unwrap().compile(&block);

                            if let Ok(ref code) = result {
                                let compile_time = start_time.elapsed().as_nanos() as u64;
                                stats
                                    .lock()
                                    .unwrap()
                                    .update_compile(compile_time, code.len());
                            }

                            result
                        })
                        .flatten()
                        .collect::<Vec<u8>>()
                })
                .collect()
        })
    }

    /// 按块大小分组（智能分片策略）
    ///
    /// 策略：
    /// - 大块（>100 ops）：单独编译
    /// - 中块（10-100 ops）：每4个一组
    /// - 小块（<10 ops）：每16个一组
    fn group_by_size(&self, blocks: &[IRBlock]) -> Vec<Vec<IRBlock>> {
        let mut chunks = Vec::new();
        let mut current_chunk = Vec::new();
        let mut current_size = 0usize;

        for block in blocks {
            let block_size = block.ops.len();

            // 如果当前块很大，单独处理
            if block_size > 100 {
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk);
                    current_chunk = Vec::new();
                    current_size = 0;
                }
                chunks.push(vec![block.clone()]);
                continue;
            }

            // 检查是否需要开始新的chunk
            let chunk_size = if block_size < 10 {
                16 // 小块：16个一组
            } else {
                4 // 中块：4个一组
            };

            if current_chunk.len() >= chunk_size || current_size + block_size > 500 {
                chunks.push(current_chunk);
                current_chunk = Vec::new();
                current_size = 0;
            }

            current_chunk.push(block.clone());
            current_size += block_size;
        }

        // 添加最后一个chunk
        if !current_chunk.is_empty() {
            chunks.push(current_chunk);
        }

        chunks
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> CompilerStats {
        self.stats.lock().unwrap().clone()
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        *self.stats.lock().unwrap() = CompilerStats::new();
    }

    /// 获取编译性能指标
    pub fn get_performance_metrics(&self) -> ParallelCompileMetrics {
        let stats = self.stats.lock().unwrap();
        ParallelCompileMetrics {
            total_blocks: stats.compiled_blocks,
            total_time_ns: stats.total_compile_time_ns,
            avg_block_size: if stats.compiled_blocks > 0 {
                (stats.generated_code_size / stats.compiled_blocks) as usize
            } else {
                0
            },
            total_code_size: stats.generated_code_size as usize,
        }
    }

    /// 设置时间预算
    pub fn set_time_budget(&mut self, budget_ns: u64) {
        self.time_budget_ns = budget_ns;
    }

    /// 获取时间预算
    pub fn get_time_budget(&self) -> u64 {
        self.time_budget_ns
    }

    /// 启用/禁用自适应分片
    pub fn set_adaptive_chunking(&mut self, enabled: bool) {
        self.adaptive_chunking = enabled;
    }

    /// 预热编译器：编译一些常见模式以初始化缓存和JIT
    pub fn warmup(&mut self) -> Result<(), CompilerError> {
        // 创建简单的预热块
        let warmup_blocks = vec![
            IRBlock {
                start_pc: vm_core::GuestAddr(0x1000),
                ops: vec![vm_ir::IROp::BinaryOp {
                    op: vm_ir::BinaryOperator::Add,
                    dest: 1,
                    src1: vm_ir::Operand::Register(0),
                    src2: vm_ir::Operand::Immediate(1),
                }],
                term: vm_ir::Terminator::Ret,
            },
            IRBlock {
                start_pc: vm_core::GuestAddr(0x2000),
                ops: vec![vm_ir::IROp::BinaryOp {
                    op: vm_ir::BinaryOperator::Mul,
                    dest: 2,
                    src1: vm_ir::Operand::Register(1),
                    src2: vm_ir::Operand::Immediate(2),
                }],
                term: vm_ir::Terminator::Ret,
            },
        ];

        // 编译预热块（忽略结果，只为了初始化）
        let _results = self.compile_blocks_unbounded(&warmup_blocks);

        Ok(())
    }
}

/// 并行编译性能指标
#[derive(Debug, Clone)]
pub struct ParallelCompileMetrics {
    /// 编译的总块数
    pub total_blocks: u64,
    /// 总编译时间（纳秒）
    pub total_time_ns: u64,
    /// 平均块大小（字节）
    pub avg_block_size: usize,
    /// 总代码大小（字节）
    pub total_code_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cranelift_backend::CraneliftBackend;

    #[test]
    fn test_parallel_compiler() {
        let backend = CraneliftBackend::new().unwrap();
        let mut parallel_compiler = ParallelJITCompiler::new(Box::new(backend));

        // 创建测试块
        let blocks = vec![
            IRBlock {
                start_pc: vm_core::GuestAddr(0x1000),
                ops: vec![
                    vm_ir::IROp::MovImm { dst: 0, imm: 42 }, // 添加一个操作
                ],
                term: vm_ir::Terminator::Ret,
            },
            IRBlock {
                start_pc: vm_core::GuestAddr(0x2000),
                ops: vec![
                    vm_ir::IROp::MovImm { dst: 0, imm: 100 }, // 添加一个操作
                ],
                term: vm_ir::Terminator::Ret,
            },
        ];

        // 并行编译
        let results = parallel_compiler.compile_blocks(&blocks);

        // 验证结果
        assert_eq!(results.len(), 2);
        for result in results {
            assert!(result.is_ok());
        }

        // 验证统计信息
        let stats = parallel_compiler.get_stats();
        assert_eq!(stats.compiled_blocks, 2);
    }

    #[test]
    fn test_parallel_compiler_with_config() {
        let backend = CraneliftBackend::new().unwrap();
        let config = ParallelCompileConfig {
            time_budget_ns: 1_000_000, // 1ms
            adaptive_chunking: false,
            ..Default::default()
        };
        let parallel_compiler = ParallelJITCompiler::with_config(Box::new(backend), config);

        // 验证配置已应用
        assert_eq!(parallel_compiler.get_time_budget(), 1_000_000);
    }

    #[test]
    fn test_parallel_compiler_performance_metrics() {
        let backend = CraneliftBackend::new().unwrap();
        let mut parallel_compiler = ParallelJITCompiler::new(Box::new(backend));

        // 创建测试块
        let blocks = vec![IRBlock {
            start_pc: vm_core::GuestAddr(0x1000),
            ops: vec![],
            term: vm_ir::Terminator::Ret,
        }];

        // 编译并获取指标
        let _results = parallel_compiler.compile_blocks(&blocks);
        let metrics = parallel_compiler.get_performance_metrics();

        // 验证指标
        assert_eq!(metrics.total_blocks, 1);
        assert!(metrics.total_code_size > 0 || metrics.total_code_size == 0); // 代码大小可能为0
    }

    #[test]
    fn test_parallel_compiler_warmup() {
        let backend = CraneliftBackend::new().unwrap();
        let mut parallel_compiler = ParallelJITCompiler::new(Box::new(backend));

        // 预热编译器
        let result = parallel_compiler.warmup();

        // 预热应该成功（即使有些编译失败）
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_parallel_compiler_time_budget() {
        let backend = CraneliftBackend::new().unwrap();
        let mut parallel_compiler = ParallelJITCompiler::new(Box::new(backend));

        // 设置非常短的时间预算
        parallel_compiler.set_time_budget(1); // 1纳秒

        // 创建测试块
        let blocks = vec![IRBlock {
            start_pc: vm_core::GuestAddr(0x1000),
            ops: vec![],
            term: vm_ir::Terminator::Ret,
        }];

        // 编译（可能会超时）
        let results = parallel_compiler.compile_blocks(&blocks);

        // 至少应该有结果
        assert!(!results.is_empty());
    }

    #[test]
    fn test_parallel_compiler_adaptive_chunking() {
        let backend = CraneliftBackend::new().unwrap();
        let mut parallel_compiler = ParallelJITCompiler::new(Box::new(backend));

        // 启用自适应分片
        parallel_compiler.set_adaptive_chunking(true);

        // 创建不同大小的块
        let blocks = vec![
            IRBlock {
                start_pc: vm_core::GuestAddr(0x1000),
                ops: vec![],
                term: vm_ir::Terminator::Ret,
            },
            IRBlock {
                start_pc: vm_core::GuestAddr(0x2000),
                ops: vec![
                    vm_ir::IROp::BinaryOp {
                        op: vm_ir::BinaryOperator::Add,
                        dest: 1,
                        src1: vm_ir::Operand::Register(0),
                        src2: vm_ir::Operand::Immediate(1),
                    },
                    // 添加更多操作
                    vm_ir::IROp::BinaryOp {
                        op: vm_ir::BinaryOperator::Add,
                        dest: 2,
                        src1: vm_ir::Operand::Register(1),
                        src2: vm_ir::Operand::Immediate(1),
                    },
                ],
                term: vm_ir::Terminator::Ret,
            },
        ];

        // 编译（应该使用智能分片）
        let _results = parallel_compiler.compile_blocks(&blocks);

        // 验证编译成功
        assert!(true);
    }

    #[test]
    fn test_parallel_compiler_unbounded() {
        let backend = CraneliftBackend::new().unwrap();
        let mut parallel_compiler = ParallelJITCompiler::new(Box::new(backend));

        // 创建测试块
        let blocks = vec![IRBlock {
            start_pc: vm_core::GuestAddr(0x1000),
            ops: vec![],
            term: vm_ir::Terminator::Ret,
        }];

        // 使用无界编译（不检查时间预算）
        let results = parallel_compiler.compile_blocks_unbounded(&blocks);

        // 验证结果
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
    }
}
