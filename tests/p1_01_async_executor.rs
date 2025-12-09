//! P1-01: 异步化执行引擎集成测试

use std::future::Future;
use std::pin::Pin;

/// 异步执行引擎trait
pub trait AsyncExecutionEngine: Send + Sync {
    fn run_async(
        &mut self,
        block_id: u64,
    ) -> Pin<Box<dyn Future<Output = Result<u64, String>> + Send + '_>>;
}

/// JIT异步执行器
pub struct AsyncJitExecutor {
    compiled_blocks: usize,
}

impl AsyncJitExecutor {
    pub fn new() -> Self {
        Self {
            compiled_blocks: 0,
        }
    }

    pub fn compile_block(&mut self, _block_id: u64) {
        self.compiled_blocks += 1;
    }
}

impl AsyncExecutionEngine for AsyncJitExecutor {
    fn run_async(
        &mut self,
        block_id: u64,
    ) -> Pin<Box<dyn Future<Output = Result<u64, String>> + Send + '_>> {
        Box::pin(async move {
            tokio::time::sleep(std::time::Duration::from_micros(100)).await;
            Ok(block_id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_jit() {
        let mut executor = AsyncJitExecutor::new();
        executor.compile_block(1);
        
        let result = executor.run_async(1).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_multiple_blocks() {
        let mut executor = AsyncJitExecutor::new();
        
        for i in 1..=3 {
            executor.compile_block(i);
        }
        
        let r1 = executor.run_async(1).await;
        let r2 = executor.run_async(2).await;
        
        assert!(r1.is_ok());
        assert!(r2.is_ok());
    }
}
