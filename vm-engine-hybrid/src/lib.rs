//! Hybrid Execution Engine
//!
//! 混合执行引擎，结合解释器和JIT编译器，实现热点追踪和自适应优化

use vm_core::{ExecutionEngine, MMU, Fault};
use vm_ir::IRBlock;
use vm_engine_interpreter::Interpreter;
use vm_engine_jit::Jit;
use std::collections::HashMap;

/// 热点阈值：执行次数超过此值时触发JIT编译
const HOT_THRESHOLD: u64 = 100;

/// 块执行统计
#[derive(Debug, Clone, Default)]
pub struct BlockStats {
    /// 执行次数
    pub exec_count: u64,
    /// 是否已编译
    pub is_compiled: bool,
    /// 累计执行时间（微秒）
    pub total_time_us: u64,
}

/// 混合执行引擎
pub struct HybridEngine {
    /// 解释器
    interpreter: Interpreter,
    /// JIT编译器
    jit: Jit,
    /// 执行统计
    stats: HashMap<u64, BlockStats>,
    /// 总解释执行次数
    pub total_interpreted: u64,
    /// 总JIT执行次数
    pub total_jit: u64,
    /// 总编译次数
    pub total_compiled: u64,
}

impl HybridEngine {
    /// 创建新的混合执行引擎
    pub fn new() -> Self {
        Self {
            interpreter: Interpreter::new(),
            jit: Jit::new(),
            stats: HashMap::new(),
            total_interpreted: 0,
            total_jit: 0,
            total_compiled: 0,
        }
    }

    /// 记录块执行并判断是否需要编译
    fn should_compile(&mut self, pc: u64) -> bool {
        let stats = self.stats.entry(pc).or_default();
        stats.exec_count += 1;

        // 如果已经编译过，直接返回false
        if stats.is_compiled {
            return false;
        }

        // 如果执行次数达到阈值，触发编译
        if stats.exec_count >= HOT_THRESHOLD {
            stats.is_compiled = true;
            true
        } else {
            false
        }
    }

    /// 获取执行统计
    pub fn get_stats(&self, pc: u64) -> Option<&BlockStats> {
        self.stats.get(&pc)
    }

    /// 获取所有统计信息
    pub fn get_all_stats(&self) -> &HashMap<u64, BlockStats> {
        &self.stats
    }

    /// 获取热点块（执行次数最多的前N个）
    pub fn get_hot_blocks(&self, top_n: usize) -> Vec<(u64, &BlockStats)> {
        let mut blocks: Vec<_> = self.stats.iter()
            .map(|(pc, stats)| (*pc, stats))
            .collect();
        
        blocks.sort_by(|a, b| b.1.exec_count.cmp(&a.1.exec_count));
        blocks.truncate(top_n);
        blocks
    }

    /// 打印执行统计
    pub fn print_stats(&self) {
        println!("\n=== Hybrid Engine Statistics ===");
        println!("Total Interpreted: {}", self.total_interpreted);
        println!("Total JIT: {}", self.total_jit);
        println!("Total Compiled: {}", self.total_compiled);
        println!("JIT Ratio: {:.2}%", 
            if self.total_interpreted + self.total_jit > 0 {
                (self.total_jit as f64 / (self.total_interpreted + self.total_jit) as f64) * 100.0
            } else {
                0.0
            }
        );

        println!("\nTop 10 Hot Blocks:");
        for (i, (pc, stats)) in self.get_hot_blocks(10).iter().enumerate() {
            println!("  {}. PC={:#x}: exec_count={}, compiled={}", 
                i + 1, pc, stats.exec_count, stats.is_compiled);
        }
    }

    /// 设置寄存器值
    pub fn set_reg(&mut self, idx: u32, val: u64) {
        self.interpreter.set_reg(idx, val);
        if (idx as usize) < self.jit.regs.len() {
            self.jit.regs[idx as usize] = val;
        }
    }

    /// 获取寄存器值
    pub fn get_reg(&self, idx: u32) -> u64 {
        self.interpreter.get_reg(idx)
    }

    /// 同步寄存器状态（从解释器到JIT）
    fn sync_regs_to_jit(&mut self) {
        for i in 0..32 {
            self.jit.regs[i] = self.interpreter.get_reg(i as u32);
        }
    }

    /// 同步寄存器状态（从JIT到解释器）
    fn sync_regs_from_jit(&mut self) {
        for i in 0..32 {
            self.interpreter.set_reg(i as u32, self.jit.regs[i]);
        }
    }
}

impl Default for HybridEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionEngine for HybridEngine {
    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock) -> Result<(), Fault> {
        let pc = block.start_pc;

        // 检查是否需要编译
        if self.should_compile(pc) {
            self.total_compiled += 1;
            // 触发JIT编译
            self.sync_regs_to_jit();
            // 注意：这里只是标记为已编译，实际编译在JIT引擎内部完成
        }

        // 检查是否已经有编译好的代码
        let stats = self.stats.get(&pc).unwrap();
        if stats.is_compiled && self.jit.is_hot(pc) {
            // 使用JIT执行
            self.sync_regs_to_jit();
            self.jit.run(mmu, block)?;
            self.sync_regs_from_jit();
            self.total_jit += 1;
        } else {
            // 使用解释器执行
            self.interpreter.run(mmu, block)?;
            self.total_interpreted += 1;
        }

        Ok(())
    }

    fn set_reg(&mut self, idx: u32, val: u64) {
        self.set_reg(idx, val);
    }

    fn get_reg(&self, idx: u32) -> u64 {
        self.get_reg(idx)
    }

    fn set_pc(&mut self, pc: u64) {
        self.interpreter.set_pc(pc);
        self.jit.pc = pc;
    }

    fn get_pc(&self) -> u64 {
        self.interpreter.get_pc()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_ir::{IROp, Terminator, IRBuilder};
    use vm_mem::SoftMmu;

    #[test]
    fn test_hybrid_engine_basic() {
        let mut engine = HybridEngine::new();
        let mut mmu = SoftMmu::new(1024 * 1024);

        // 创建一个简单的IR块
        let mut builder = IRBuilder::new(0x1000);
        builder.push(IROp::MovImm { dst: 1, imm: 42 });
        builder.set_term(Terminator::Jmp { target: 0x1004 });
        let block = builder.build();

        // 执行多次以触发JIT编译
        for _ in 0..HOT_THRESHOLD + 10 {
            engine.run(&mut mmu, &block).unwrap();
        }

        assert_eq!(engine.get_reg(1), 42);
        assert!(engine.total_compiled > 0);
    }

    #[test]
    fn test_hot_block_tracking() {
        let mut engine = HybridEngine::new();

        // 模拟执行
        for _ in 0..50 {
            let _ = engine.should_compile(0x1000);
        }
        for _ in 0..200 {
            let _ = engine.should_compile(0x2000);
        }

        let hot_blocks = engine.get_hot_blocks(2);
        assert_eq!(hot_blocks.len(), 2);
        assert_eq!(hot_blocks[0].0, 0x2000); // 最热的块
        assert_eq!(hot_blocks[1].0, 0x1000);
    }
}
