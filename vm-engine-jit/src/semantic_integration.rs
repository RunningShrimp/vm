//! 语义库与混合执行引擎的集成
//!
//! 展示如何使用 vm-ir-lift 的语义库来优化 HybridExecutor

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use vm_ir_lift::decoder::{ISA, Instruction, OperandType};
use vm_ir_lift::{LiftingContext, semantics::create_semantics};

/// 语义缓存 - 缓存已提升的指令 IR
#[derive(Clone)]
pub struct SemanticCache {
    /// 指令字节 -> 生成的 LLVM IR 映射
    cache: Arc<RwLock<HashMap<Vec<u8>, String>>>,
    /// 命中统计
    hits: Arc<RwLock<u64>>,
    /// 未命中统计
    misses: Arc<RwLock<u64>>,
}

impl SemanticCache {
    /// 创建新的语义缓存
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            hits: Arc::new(RwLock::new(0)),
            misses: Arc::new(RwLock::new(0)),
        }
    }

    /// 查询缓存，如果未命中则返回 None
    pub fn lookup(&self, instr_bytes: &[u8]) -> Option<String> {
        let cache = self.cache.read();
        if let Some(ir) = cache.get(instr_bytes) {
            *self.hits.write() += 1;
            return Some(ir.clone());
        }
        *self.misses.write() += 1;
        None
    }

    /// 插入缓存
    pub fn insert(&self, instr_bytes: Vec<u8>, ir: String) {
        self.cache.write().insert(instr_bytes, ir);
    }

    /// 获取缓存命中率
    pub fn hit_rate(&self) -> f64 {
        let hits = *self.hits.read() as f64;
        let misses = *self.misses.read() as f64;
        let total = hits + misses;
        if total > 0.0 { hits / total } else { 0.0 }
    }

    /// 获取统计信息
    pub fn stats(&self) -> (u64, u64, f64) {
        let hits = *self.hits.read();
        let misses = *self.misses.read();
        let rate = self.hit_rate();
        (hits, misses, rate)
    }

    /// 清空缓存
    pub fn clear(&self) {
        self.cache.write().clear();
        *self.hits.write() = 0;
        *self.misses.write() = 0;
    }
}

/// 语义分析器 - 提升指令为 LLVM IR
pub struct SemanticAnalyzer {
    isa: ISA,
    cache: SemanticCache,
}

impl SemanticAnalyzer {
    /// 创建新的语义分析器
    pub fn new(isa: ISA) -> Self {
        Self {
            isa,
            cache: SemanticCache::new(),
        }
    }

    /// 将指令字节提升为 LLVM IR
    pub fn lift_instruction(&self, instr_bytes: &[u8]) -> Result<String, String> {
        // 首先检查缓存
        if let Some(ir) = self.cache.lookup(instr_bytes) {
            return Ok(ir);
        }

        // 构建一个简单的指令对象进行演示
        // 在实际使用中，需要使用真实的解码器
        let mnemonic = format!("instr_{}", instr_bytes.len());
        let instr = Instruction::new(mnemonic, vec![], instr_bytes.len());

        // 获取语义库
        let semantics = create_semantics(self.isa);

        // 生成 LLVM IR
        let mut ctx = LiftingContext::new(self.isa);
        let ir = semantics
            .lift(&instr, &mut ctx)
            .map_err(|e| format!("Lift error: {:?}", e))?;

        // 缓存结果
        self.cache.insert(instr_bytes.to_vec(), ir.clone());

        Ok(ir)
    }

    /// 批量提升指令块
    pub fn lift_block(&self, block_bytes: &[u8]) -> Result<Vec<String>, String> {
        let mut instructions = Vec::new();
        let mut offset = 0;

        // 简化版本：按固定大小切分（实际需要真实的指令长度）
        while offset < block_bytes.len() {
            let chunk_size = std::cmp::min(15, block_bytes.len() - offset);
            let chunk = &block_bytes[offset..offset + chunk_size];

            match self.lift_instruction(chunk) {
                Ok(ir) => instructions.push(ir),
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to lift instruction at offset {}: {}",
                        offset, e
                    );
                }
            }

            offset += chunk_size;
        }

        Ok(instructions)
    }

    /// 获取缓存统计
    pub fn cache_stats(&self) -> (u64, u64, f64) {
        self.cache.stats()
    }

    /// 清空缓存
    pub fn clear_cache(&self) {
        self.cache.clear();
    }
}

/// 优化的混合执行器 - 集成语义库
pub struct OptimizedHybridExecutor {
    analyzer: SemanticAnalyzer,
    /// IR 代码块缓存
    ir_cache: Arc<RwLock<HashMap<u64, Vec<String>>>>,
    /// 优化统计
    optimization_count: Arc<RwLock<u64>>,
}

impl OptimizedHybridExecutor {
    /// 创建新的优化混合执行器
    pub fn new(isa: ISA) -> Self {
        Self {
            analyzer: SemanticAnalyzer::new(isa),
            ir_cache: Arc::new(RwLock::new(HashMap::new())),
            optimization_count: Arc::new(RwLock::new(0)),
        }
    }

    /// 分析和优化块
    pub fn optimize_block(&self, pc: u64, block_bytes: &[u8]) -> Result<Vec<String>, String> {
        // 检查 IR 缓存
        {
            let cache = self.ir_cache.read();
            if let Some(ir) = cache.get(&pc) {
                return Ok(ir.clone());
            }
        }

        // 提升指令为 LLVM IR
        let ir = self.analyzer.lift_block(block_bytes)?;

        // 缓存结果
        {
            self.ir_cache.write().insert(pc, ir.clone());
            *self.optimization_count.write() += 1;
        }

        Ok(ir)
    }

    /// 获取优化统计
    pub fn stats(&self) -> OptimizationStats {
        let (hits, misses, hit_rate) = self.analyzer.cache_stats();
        let opt_count = *self.optimization_count.read();

        OptimizationStats {
            semantic_cache_hits: hits,
            semantic_cache_misses: misses,
            semantic_hit_rate: hit_rate,
            blocks_optimized: opt_count,
        }
    }

    /// 清空缓存
    pub fn clear_caches(&self) {
        self.analyzer.clear_cache();
        self.ir_cache.write().clear();
        *self.optimization_count.write() = 0;
    }
}

/// 优化统计信息
#[derive(Debug, Clone)]
pub struct OptimizationStats {
    pub semantic_cache_hits: u64,
    pub semantic_cache_misses: u64,
    pub semantic_hit_rate: f64,
    pub blocks_optimized: u64,
}

impl OptimizationStats {
    pub fn print(&self) {
        println!("=== Semantic Lifting Optimization Stats ===");
        println!("Semantic Cache Hits: {}", self.semantic_cache_hits);
        println!("Semantic Cache Misses: {}", self.semantic_cache_misses);
        println!("Semantic Hit Rate: {:.1}%", self.semantic_hit_rate * 100.0);
        println!("Blocks Optimized: {}", self.blocks_optimized);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_cache_hit_miss() {
        let cache = SemanticCache::new();
        let instr = vec![0x48, 0x89, 0xC3]; // MOV RBX, RAX

        // 第一次查询 - 未命中
        assert!(cache.lookup(&instr).is_none());

        // 插入缓存
        cache.insert(instr.clone(), "mov_ir".to_string());

        // 第二次查询 - 命中
        assert_eq!(cache.lookup(&instr), Some("mov_ir".to_string()));

        let (hits, misses, rate) = cache.stats();
        assert_eq!(hits, 1);
        assert_eq!(misses, 1);
        assert!(rate > 0.4 && rate < 0.6); // 50%
    }

    #[test]
    fn test_semantic_analyzer_creation() {
        let analyzer = SemanticAnalyzer::new(ISA::X86_64);
        assert_eq!(analyzer.isa, ISA::X86_64);
    }

    #[test]
    fn test_optimized_hybrid_executor_creation() {
        let executor = OptimizedHybridExecutor::new(ISA::X86_64);
        let stats = executor.stats();
        assert_eq!(stats.blocks_optimized, 0);
        assert_eq!(stats.semantic_cache_hits, 0);
    }

    #[test]
    fn test_optimization_stats_print() {
        let stats = OptimizationStats {
            semantic_cache_hits: 100,
            semantic_cache_misses: 20,
            semantic_hit_rate: 0.833,
            blocks_optimized: 42,
        };
        // 这个测试主要是为了确保 print 不会 panic
        stats.print();
    }
}
