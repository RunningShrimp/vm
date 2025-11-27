//! 原子操作和并发优化模块
//!
//! 实现原子操作的优化、内存屏障分析和并发访问优化

use std::collections::HashSet;
use vm_ir::{RegId, IROp, AtomicOp};

/// 内存屏障类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryBarrier {
    /// 完整内存屏障 (全同步)
    Full,
    /// 加载屏障 (防止后续加载越过此点)
    Load,
    /// 存储屏障 (防止之前存储越过此点)
    Store,
    /// 无屏障 (原子操作，无额外同步)
    Acquire,
    /// 释放屏障
    Release,
}

/// 原子操作上下文
#[derive(Debug, Clone)]
pub struct AtomicContext {
    /// 原子操作的内存地址
    pub addr: u64,
    /// 操作类型
    pub op_type: AtomicOp,
    /// 内存屏障类型
    pub barrier: MemoryBarrier,
    /// 是否可以被其他线程观察到
    pub is_observable: bool,
}

/// 原子操作分析器
pub struct AtomicAnalyzer {
    /// 已分析的地址
    analyzed_addrs: HashSet<u64>,
}

impl AtomicAnalyzer {
    /// 创建新分析器
    pub fn new() -> Self {
        Self {
            analyzed_addrs: HashSet::new(),
        }
    }

    /// 分析原子操作的依赖性
    pub fn analyze_dependencies(ops: &[IROp]) -> AtomicDependencyGraph {
        let mut graph = AtomicDependencyGraph::new();

        for (idx, op) in ops.iter().enumerate() {
            if let Some(atomic_info) = Self::extract_atomic_info(op) {
                graph.add_operation(idx, atomic_info);
            }
        }

        graph.compute_dependencies();
        graph
    }

    /// 提取原子操作信息
    fn extract_atomic_info(op: &IROp) -> Option<AtomicContext> {
        match op {
            IROp::AtomicRMW {
                dst: _,
                addr,
                value: _,
                op_type,
            } => Some(AtomicContext {
                addr: *addr as u64,
                op_type: *op_type,
                barrier: MemoryBarrier::Full,
                is_observable: true,
            }),
            _ => None,
        }
    }

    /// 判断两个原子操作是否有冲突
    pub fn has_conflict(ctx1: &AtomicContext, ctx2: &AtomicContext) -> bool {
        // 同一地址的原子操作有冲突
        ctx1.addr == ctx2.addr
            || (ctx1.is_observable && ctx2.is_observable && Self::may_alias(ctx1.addr, ctx2.addr))
    }

    /// 判断两个地址是否可能别名
    fn may_alias(addr1: u64, addr2: u64) -> bool {
        // 简化：直接比较地址（实际应该做更复杂的分析）
        addr1 == addr2
    }

    /// 优化原子操作的屏障
    /// 
    /// 如果分析表明某些屏障不必要，可以降级为更轻量的屏障
    pub fn optimize_barriers(ops: &mut [IROp]) {
        let dependencies = Self::analyze_dependencies(ops);
        
        for (idx, op) in ops.iter_mut().enumerate() {
            if let IROp::AtomicRMW { .. } = op {
                let can_weaken = !dependencies.depends_on_previous(idx)
                    && !dependencies.has_dependent_after(idx);
                
                if can_weaken {
                    // 此原子操作可以使用更轻量的屏障
                    // (实现细节由具体编译器决定)
                }
            }
        }
    }
}

/// 原子依赖图
pub struct AtomicDependencyGraph {
    /// 操作: (索引, 上下文)
    operations: Vec<(usize, AtomicContext)>,
    /// 依赖关系: (from, to)
    dependencies: Vec<(usize, usize)>,
}

impl AtomicDependencyGraph {
    /// 创建新图
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    /// 添加操作
    fn add_operation(&mut self, idx: usize, ctx: AtomicContext) {
        self.operations.push((idx, ctx));
    }

    /// 计算依赖关系
    fn compute_dependencies(&mut self) {
        for i in 0..self.operations.len() {
            for j in (i + 1)..self.operations.len() {
                let (idx_i, ctx_i) = &self.operations[i];
                let (idx_j, ctx_j) = &self.operations[j];

                if AtomicAnalyzer::has_conflict(ctx_i, ctx_j) {
                    // j 依赖于 i 的结果
                    self.dependencies.push((*idx_i, *idx_j));
                }
            }
        }
    }

    /// 检查是否依赖于之前的操作
    fn depends_on_previous(&self, idx: usize) -> bool {
        self.dependencies.iter().any(|(from, to)| *to == idx && *from < idx)
    }

    /// 检查是否有后续操作依赖此操作
    fn has_dependent_after(&self, idx: usize) -> bool {
        self.dependencies.iter().any(|(from, _to)| *from == idx)
    }
}

/// 并发访问模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcurrencyPattern {
    /// 无同步
    Unsynchronized,
    /// 只读访问
    ReadOnly,
    /// 单线程访问
    SingleThreaded,
    /// 多线程读，单线程写
    MultiReaderSingleWriter,
    /// 多线程竞争
    MultiThreadedRace,
}

/// 并发优化器
pub struct ConcurrencyOptimizer;

impl ConcurrencyOptimizer {
    /// 分析并发访问模式
    pub fn analyze_pattern(accessed_regs: &[(RegId, AccessType)]) -> ConcurrencyPattern {
        let mut write_count = 0;
        let mut read_count = 0;

        for (_reg, access_type) in accessed_regs {
            match access_type {
                AccessType::Read => read_count += 1,
                AccessType::Write => write_count += 1,
                AccessType::RMW => {
                    read_count += 1;
                    write_count += 1;
                }
            }
        }

        match (read_count, write_count) {
            (_, 0) => ConcurrencyPattern::ReadOnly,
            (0, _) => ConcurrencyPattern::SingleThreaded,
            (r, 1) if r > 1 => ConcurrencyPattern::MultiReaderSingleWriter,
            _ => ConcurrencyPattern::MultiThreadedRace,
        }
    }

    /// 生成适应并发模式的代码
    pub fn generate_optimized_code(pattern: ConcurrencyPattern) -> CodeGenStrategy {
        match pattern {
            ConcurrencyPattern::Unsynchronized => CodeGenStrategy {
                use_atomic: false,
                use_barrier: false,
                inline_level: 3,
            },
            ConcurrencyPattern::ReadOnly => CodeGenStrategy {
                use_atomic: false,
                use_barrier: false,
                inline_level: 3,
            },
            ConcurrencyPattern::SingleThreaded => CodeGenStrategy {
                use_atomic: false,
                use_barrier: false,
                inline_level: 2,
            },
            ConcurrencyPattern::MultiReaderSingleWriter => CodeGenStrategy {
                use_atomic: true,
                use_barrier: true,
                inline_level: 1,
            },
            ConcurrencyPattern::MultiThreadedRace => CodeGenStrategy {
                use_atomic: true,
                use_barrier: true,
                inline_level: 0,
            },
        }
    }
}

/// 访问类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    /// 读访问
    Read,
    /// 写访问
    Write,
    /// 读-修改-写
    RMW,
}

/// 代码生成策略
#[derive(Debug)]
pub struct CodeGenStrategy {
    /// 是否使用原子操作
    pub use_atomic: bool,
    /// 是否使用内存屏障
    pub use_barrier: bool,
    /// 内联优化级别 (0-3)
    pub inline_level: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_context_creation() {
        let ctx = AtomicContext {
            addr: 0x1000,
            op_type: AtomicOp::Add,
            barrier: MemoryBarrier::Full,
            is_observable: true,
        };

        assert_eq!(ctx.addr, 0x1000);
        assert_eq!(ctx.barrier, MemoryBarrier::Full);
    }

    #[test]
    fn test_concurrency_pattern_analysis() {
        let accesses = vec![(RegId(1), AccessType::Read), (RegId(1), AccessType::Read)];
        let pattern = ConcurrencyOptimizer::analyze_pattern(&accesses);
        assert_eq!(pattern, ConcurrencyPattern::ReadOnly);
    }

    #[test]
    fn test_code_gen_strategy() {
        let strategy = ConcurrencyOptimizer::generate_optimized_code(
            ConcurrencyPattern::MultiThreadedRace,
        );
        assert!(strategy.use_atomic);
        assert!(strategy.use_barrier);
    }
}
