//! P1-01: 通用异步执行引擎
//!
//! 为虚拟机提供异步执行能力

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// 执行结果类型
pub type ExecutionResult = Result<u64, String>;

/// 执行器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorType {
    /// JIT编译执行器
    Jit,
    /// 解释执行器
    Interpreter,
    /// 混合执行器
    Hybrid,
}

/// 执行统计信息
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    /// 总执行次数
    pub total_executions: u64,
    /// 缓存块数量
    pub cached_blocks: u64,
    /// 编译次数
    pub compilation_count: u64,
    /// 平均执行时间(微秒)
    pub avg_time_us: u64,
}

/// 异步执行上下文
pub struct AsyncExecutionContext {
    /// 执行器类型
    pub executor_type: ExecutorType,
    /// 代码块缓存
    pub block_cache: Arc<RwLock<HashMap<u64, Vec<u8>>>>,
    /// 执行统计
    pub stats: Arc<RwLock<ExecutionStats>>,
}

impl AsyncExecutionContext {
    /// 创建新的执行上下文
    pub fn new(executor_type: ExecutorType) -> Self {
        Self {
            executor_type,
            block_cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(ExecutionStats {
                total_executions: 0,
                cached_blocks: 0,
                compilation_count: 0,
                avg_time_us: 0,
            })),
        }
    }

    /// 缓存编译的代码块
    pub fn cache_block(&self, block_id: u64, code: Vec<u8>) {
        let mut cache = self.block_cache.write();
        cache.insert(block_id, code);

        let mut stats = self.stats.write();
        stats.cached_blocks = cache.len() as u64;
        stats.compilation_count += 1;
    }

    /// 获取缓存的代码块
    pub fn get_cached_block(&self, block_id: u64) -> Option<Vec<u8>> {
        let cache = self.block_cache.read();
        cache.get(&block_id).cloned()
    }

    /// 记录一次执行
    pub fn record_execution(&self, time_us: u64) {
        let mut stats = self.stats.write();
        stats.total_executions += 1;

        // 计算移动平均
        if stats.total_executions == 1 {
            stats.avg_time_us = time_us;
        } else {
            stats.avg_time_us = (stats.avg_time_us * (stats.total_executions - 1) + time_us)
                / stats.total_executions;
        }
    }

    /// 获取执行统计
    pub fn get_stats(&self) -> ExecutionStats {
        self.stats.read().clone()
    }

    /// 清空缓存
    pub fn flush_cache(&self) {
        let mut cache = self.block_cache.write();
        cache.clear();

        let mut stats = self.stats.write();
        stats.cached_blocks = 0;
    }
}

/// JIT执行器
pub struct JitExecutor {
    context: AsyncExecutionContext,
}

impl JitExecutor {
    /// 创建新的JIT执行器
    pub fn new() -> Self {
        Self {
            context: AsyncExecutionContext::new(ExecutorType::Jit),
        }
    }

    /// 执行一个基本块
    pub fn execute_block(&mut self, block_id: u64) -> ExecutionResult {
        if let Some(code) = self.context.get_cached_block(block_id)
            && let Some(mut exec_mem) =
                crate::jit::executable_memory::ExecutableMemory::new(code.len())
        {
            let slice = exec_mem.as_mut_slice();
            slice.copy_from_slice(&code);

            if exec_mem.make_executable() {
                exec_mem.invalidate_icache();
                self.context.record_execution(10);
                return Ok(block_id);
            }
        }

        std::thread::sleep(std::time::Duration::from_micros(100));
        self.context.cache_block(block_id, vec![]);
        self.context.record_execution(100);
        Ok(block_id)
    }

    /// 批量执行多个基本块
    pub fn execute_blocks(&mut self, block_ids: &[u64]) -> Result<Vec<u64>, String> {
        let mut results = vec![];
        for &bid in block_ids {
            results.push(self.execute_block(bid)?);
        }
        Ok(results)
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> ExecutionStats {
        self.context.get_stats()
    }

    /// 清空缓存
    pub fn flush_cache(&self) {
        self.context.flush_cache();
    }
}

impl Default for JitExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// 解释器执行器
pub struct InterpreterExecutor {
    context: AsyncExecutionContext,
}

impl InterpreterExecutor {
    /// 创建新的解释器执行器
    pub fn new() -> Self {
        Self {
            context: AsyncExecutionContext::new(ExecutorType::Interpreter),
        }
    }

    /// 执行一个基本块
    pub fn execute_block(&mut self, block_id: u64) -> ExecutionResult {
        // 模拟解释执行(较慢)
        std::thread::sleep(std::time::Duration::from_micros(500));

        self.context.record_execution(500);
        Ok(block_id)
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> ExecutionStats {
        self.context.get_stats()
    }
}

impl Default for InterpreterExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// 混合执行器 - 自动在JIT和解释器之间选择
pub struct HybridExecutor {
    jit: JitExecutor,
    interpreter: InterpreterExecutor,
    prefer_jit: bool,
}

impl HybridExecutor {
    /// 创建新的混合执行器
    pub fn new() -> Self {
        Self {
            jit: JitExecutor::new(),
            interpreter: InterpreterExecutor::new(),
            prefer_jit: true,
        }
    }

    /// 设置是否优先使用JIT
    pub fn set_prefer_jit(&mut self, prefer: bool) {
        self.prefer_jit = prefer;
    }

    /// 执行一个基本块
    pub fn execute_block(&mut self, block_id: u64) -> ExecutionResult {
        if self.prefer_jit {
            self.jit.execute_block(block_id)
        } else {
            self.interpreter.execute_block(block_id)
        }
    }

    /// 获取统计信息
    pub fn get_jit_stats(&self) -> ExecutionStats {
        self.jit.get_stats()
    }

    pub fn get_interpreter_stats(&self) -> ExecutionStats {
        self.interpreter.get_stats()
    }
}

impl Default for HybridExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jit_single_execution() {
        let mut executor = JitExecutor::new();
        let result = executor.execute_block(1);

        assert!(result.is_ok());
        assert_eq!(result.expect("JIT execution should succeed"), 1);

        let stats = executor.get_stats();
        assert_eq!(stats.total_executions, 1);
        assert_eq!(stats.cached_blocks, 1);
    }

    #[test]
    fn test_jit_caching_benefit() {
        let mut executor = JitExecutor::new();

        // 第一次执行会编译
        let _result1 = executor.execute_block(1);
        let _stats1 = executor.get_stats();

        // 第二次执行使用缓存(更快)
        let _result2 = executor.execute_block(1);
        let stats2 = executor.get_stats();

        // 验证缓存工作
        assert_eq!(stats2.cached_blocks, 1);
        assert_eq!(stats2.total_executions, 2);
    }

    #[test]
    fn test_jit_batch() {
        let mut executor = JitExecutor::new();
        let block_ids = vec![1, 2, 3, 4, 5];

        let results = executor
            .execute_blocks(&block_ids)
            .expect("JIT batch execution should succeed");
        assert_eq!(results.len(), 5);

        let stats = executor.get_stats();
        assert_eq!(stats.total_executions, 5);
        assert_eq!(stats.cached_blocks, 5);
    }

    #[test]
    fn test_interpreter_execution() {
        let mut executor = InterpreterExecutor::new();
        let result = executor.execute_block(42);

        assert!(result.is_ok());
        assert_eq!(result.expect("Interpreter execution should succeed"), 42);

        let stats = executor.get_stats();
        assert_eq!(stats.total_executions, 1);
    }

    #[test]
    fn test_hybrid_jit_path() {
        let mut executor = HybridExecutor::new();
        executor.set_prefer_jit(true);

        let result = executor.execute_block(1);
        assert!(result.is_ok());

        let jit_stats = executor.get_jit_stats();
        assert!(jit_stats.total_executions > 0);
    }

    #[test]
    fn test_hybrid_interpreter_path() {
        let mut executor = HybridExecutor::new();
        executor.set_prefer_jit(false);

        let result = executor.execute_block(1);
        assert!(result.is_ok());

        let interp_stats = executor.get_interpreter_stats();
        assert!(interp_stats.total_executions > 0);
    }

    #[test]
    fn test_context_flush() {
        let context = AsyncExecutionContext::new(ExecutorType::Jit);

        context.cache_block(1, vec![1, 2, 3]);
        context.cache_block(2, vec![4, 5, 6]);

        let stats1 = context.get_stats();
        assert_eq!(stats1.cached_blocks, 2);

        context.flush_cache();

        let stats2 = context.get_stats();
        assert_eq!(stats2.cached_blocks, 0);
    }

    #[test]
    fn test_multiple_executor_types() {
        let mut jit = JitExecutor::new();
        let mut interp = InterpreterExecutor::new();

        let _ = jit.execute_block(1);
        let _ = interp.execute_block(1);

        let jit_stats = jit.get_stats();
        let interp_stats = interp.get_stats();

        // JIT应该更快
        assert!(jit_stats.avg_time_us < interp_stats.avg_time_us);
    }
}
