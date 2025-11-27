//! 并行编译器
//!
//! 使用rayon并行编译IR块，提高编译效率。

use crate::compiler_backend::{CompilerBackend, CompilerError, CompilerStats};
use vm_ir::IRBlock;
use rayon::prelude::*;
use std::sync::{Arc, Mutex};

/// 并行JIT编译器
pub struct ParallelJITCompiler {
    /// 编译器后端
    backend: Box<dyn CompilerBackend>,
    /// 统计信息
    stats: Arc<Mutex<CompilerStats>>,
}

impl ParallelJITCompiler {
    /// 创建新的并行编译器
    pub fn new(backend: Box<dyn CompilerBackend>) -> Self {
        Self {
            backend,
            stats: Arc::new(Mutex::new(CompilerStats::new())),
        }
    }
    
    /// 并行编译多个IR块
    pub fn compile_blocks(&mut self, blocks: &[IRBlock]) -> Vec<Result<Vec<u8>, CompilerError>> {
        let stats = Arc::clone(&self.stats);
        
        // 注意：这里有一个已知的限制 - 为了实现真正的并行编译，
        // 我们需要为每个线程创建独立的后端实例。
        // 但当前设计中，后端无法轻松克隆，所以我们退回到串行编译。
        // 这是一个需要在后续迭代中解决的架构限制。
        
        // 临时使用串行编译实现，保持API一致性
        blocks
            .iter()
            .map(|block| {
                let start_time = std::time::Instant::now();
                let result = self.backend.compile(block);
                
                // 更新统计信息
                if let Ok(ref code) = result {
                    let compile_time = start_time.elapsed().as_nanos() as u64;
                    stats.lock().unwrap().update_compile(compile_time, code.len());
                }
                
                result
            })
            .collect()
    }
    
    /// 获取统计信息
    pub fn get_stats(&self) -> CompilerStats {
        self.stats.lock().unwrap().clone()
    }
    
    /// 重置统计信息
    pub fn reset_stats(&self) {
        *self.stats.lock().unwrap() = CompilerStats::new();
    }
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
                name: "test_block_1".to_string(),
                ops: vec![],
                terminator: vm_ir::Terminator::Ret { value: None },
            },
            IRBlock {
                name: "test_block_2".to_string(),
                ops: vec![],
                terminator: vm_ir::Terminator::Ret { value: None },
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
}
