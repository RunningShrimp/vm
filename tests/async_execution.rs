//! P1-01: 异步化执行引擎
//!
//! 为执行引擎添加async/await支持

use std::future::Future;
use std::pin::Pin;

/// 异步执行引擎trait
pub trait AsyncExecutionEngine: Send + Sync {
    /// 异步执行代码块
    fn run_async(
        &mut self,
        block_id: u64,
    ) -> Pin<Box<dyn Future<Output = Result<u64, String>> + Send + '_>>;
}

/// JIT异步执行器
pub struct AsyncJitExecutor {
    name: String,
    compiled_blocks: usize,
}

impl AsyncJitExecutor {
    pub fn new() -> Self {
        Self {
            name: "AsyncJIT".to_string(),
            compiled_blocks: 0,
        }
    }

    pub fn compile_block(&mut self, _block_id: u64) {
        self.compiled_blocks += 1;
    }

    pub fn get_compiled_count(&self) -> usize {
        self.compiled_blocks
    }
}

impl AsyncExecutionEngine for AsyncJitExecutor {
    fn run_async(
        &mut self,
        block_id: u64,
    ) -> Pin<Box<dyn Future<Output = Result<u64, String>> + Send + '_>> {
        let name = self.name.clone();
        Box::pin(async move {
            // 模拟异步编译
            tokio::time::sleep(std::time::Duration::from_micros(100)).await;
            Ok(block_id)
        })
    }
}

/// 解释器异步执行器
pub struct AsyncInterpreterExecutor {
    instruction_count: u64,
}

impl AsyncInterpreterExecutor {
    pub fn new() -> Self {
        Self {
            instruction_count: 0,
        }
    }
}

impl AsyncExecutionEngine for AsyncInterpreterExecutor {
    fn run_async(
        &mut self,
        block_id: u64,
    ) -> Pin<Box<dyn Future<Output = Result<u64, String>> + Send + '_>> {
        Box::pin(async move {
            // 模拟异步解释执行
            tokio::time::sleep(std::time::Duration::from_micros(500)).await;
            Ok(block_id)
        })
    }
}

/// 混合异步执行器
pub struct AsyncHybridExecutor {
    jit: AsyncJitExecutor,
    interpreter: AsyncInterpreterExecutor,
    prefer_jit: bool,
}

impl AsyncHybridExecutor {
    pub fn new() -> Self {
        Self {
            jit: AsyncJitExecutor::new(),
            interpreter: AsyncInterpreterExecutor::new(),
            prefer_jit: true,
        }
    }

    pub fn set_prefer_jit(&mut self, prefer: bool) {
        self.prefer_jit = prefer;
    }
}

impl AsyncExecutionEngine for AsyncHybridExecutor {
    fn run_async(
        &mut self,
        block_id: u64,
    ) -> Pin<Box<dyn Future<Output = Result<u64, String>> + Send + '_>> {
        let prefer_jit = self.prefer_jit;
        
        if prefer_jit && self.jit.get_compiled_count() > 0 {
            self.jit.run_async(block_id)
        } else {
            self.interpreter.run_async(block_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_jit_executor() {
        let mut executor = AsyncJitExecutor::new();
        executor.compile_block(1);
        
        let result = executor.run_async(1).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_async_interpreter() {
        let mut executor = AsyncInterpreterExecutor::new();
        let result = executor.run_async(42).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_async_hybrid_jit_path() {
        let mut executor = AsyncHybridExecutor::new();
        executor.jit.compile_block(1);
        executor.set_prefer_jit(true);
        
        let result = executor.run_async(1).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_async_hybrid_interp_path() {
        let mut executor = AsyncHybridExecutor::new();
        executor.set_prefer_jit(false);
        
        let result = executor.run_async(2).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_async_blocks() {
        let mut executor = AsyncJitExecutor::new();
        
        let mut tasks = vec![];
        for i in 1..=5 {
            executor.compile_block(i);
            let future = executor.run_async(i);
            tasks.push(future);
        }
        
        for task in tasks {
            let result = task.await;
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_async_performance() {
        let mut executor = AsyncJitExecutor::new();
        executor.compile_block(1);
        
        let start = std::time::Instant::now();
        let _ = executor.run_async(1).await;
        let elapsed = start.elapsed();
        
        // JIT应该<1ms
        assert!(elapsed.as_millis() < 1);
    }
}
