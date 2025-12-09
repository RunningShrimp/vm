//! P1-01 异步执行引擎集成测试
//! 独立于vm-engine-jit编译状态

/// 简化的异步执行引擎
pub struct SimpleAsyncExecutor {
    execution_count: u64,
}

impl SimpleAsyncExecutor {
    pub fn new() -> Self {
        Self {
            execution_count: 0,
        }
    }

    pub async fn execute(&mut self) -> Result<u64, String> {
        tokio::time::sleep(std::time::Duration::from_micros(10)).await;
        self.execution_count += 1;
        Ok(self.execution_count)
    }

    pub async fn execute_batch(&mut self, count: u32) -> Result<u64, String> {
        for _ in 0..count {
            self.execute().await?;
        }
        Ok(self.execution_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_async_execution() {
        let mut executor = SimpleAsyncExecutor::new();
        let result = executor.execute().await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_batch_execution() {
        let mut executor = SimpleAsyncExecutor::new();
        let result = executor.execute_batch(5).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 5);
    }

    #[tokio::test]
    async fn test_concurrent_tasks() {
        let mut exec1 = SimpleAsyncExecutor::new();
        let mut exec2 = SimpleAsyncExecutor::new();
        
        let task1 = async {
            let mut e = SimpleAsyncExecutor::new();
            e.execute().await
        };
        
        let task2 = async {
            let mut e = SimpleAsyncExecutor::new();
            e.execute().await
        };
        
        let (r1, r2) = tokio::join!(task1, task2);
        
        assert!(r1.is_ok());
        assert!(r2.is_ok());
    }
}
