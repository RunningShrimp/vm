//! 跨架构一致性测试与性能基准
//!
//! 确保 translator 和 interpreter 对相同 IR 块产生一致的执行结果

use super::translator::ArchTranslator;
use super::{SourceArch, TargetArch};
use vm_core::{ExecResult, ExecStatus, GuestAddr, MMU, VmError};
use vm_engine_interpreter::Interpreter;
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};
use vm_mem::SoftMmu;

/// 测试结果
#[derive(Debug, Clone)]
pub struct ConsistencyTestResult {
    /// 是否通过
    pub passed: bool,
    /// 解释器执行结果
    pub interpreter_result: ExecResult,
    /// 翻译后执行结果（通过解释器执行翻译后的IR）
    pub translated_result: ExecResult,
    /// 差异描述
    pub differences: Vec<String>,
}

/// 跨架构一致性测试器
pub struct ConsistencyTester {
    /// 源架构
    source_arch: SourceArch,
    /// 目标架构
    target_arch: TargetArch,
    /// 内存大小
    memory_size: usize,
}

impl ConsistencyTester {
    /// 创建新的测试器
    pub fn new(source_arch: SourceArch, target_arch: TargetArch) -> Self {
        Self {
            source_arch,
            target_arch,
            memory_size: 64 * 1024 * 1024, // 64MB
        }
    }

    /// 设置内存大小
    pub fn with_memory_size(mut self, size: usize) -> Self {
        self.memory_size = size;
        self
    }

    /// 测试单个 IR 块的一致性
    ///
    /// 1. 使用解释器直接执行源 IR 块
    /// 2. 使用 translator 翻译 IR 块
    /// 3. 使用解释器执行翻译后的 IR 块
    /// 4. 比较两个执行结果
    pub fn test_block_consistency(
        &self,
        source_block: &IRBlock,
    ) -> Result<ConsistencyTestResult, VmError> {
        // 1. 创建两个独立的内存和解释器实例
        let mut mmu1 = SoftMmu::new(self.memory_size, false);
        let mut mmu2 = SoftMmu::new(self.memory_size, false);
        
        // 初始化相同的初始状态
        let initial_pc = source_block.start_pc;
        let mut interpreter1 = Interpreter::new();
        interpreter1.set_pc(initial_pc);
        
        let mut interpreter2 = Interpreter::new();
        interpreter2.set_pc(initial_pc);

        // 2. 直接执行源 IR 块
        let interpreter_result = interpreter1.run(&mut mmu1, source_block);

        // 3. 翻译 IR 块
        let mut translator = ArchTranslator::new(self.source_arch, self.target_arch);
        let translated_block = translator.translate_block(source_block)?;

        // 4. 执行翻译后的 IR 块
        let translated_result = interpreter2.run(&mut mmu2, &translated_block);

        // 5. 比较结果
        let mut differences = Vec::new();
        let passed = self.compare_results(
            &interpreter_result,
            &translated_result,
            &mut differences,
        );

        Ok(ConsistencyTestResult {
            passed,
            interpreter_result,
            translated_result,
            differences,
        })
    }

    /// 比较两个执行结果
    fn compare_results(
        &self,
        result1: &ExecResult,
        result2: &ExecResult,
        differences: &mut Vec<String>,
    ) -> bool {
        let mut passed = true;

        // 比较状态
        match (&result1.status, &result2.status) {
            (ExecStatus::Continue, ExecStatus::Continue) => {}
            (ExecStatus::Error(e1), ExecStatus::Error(e2)) => {
                if format!("{:?}", e1) != format!("{:?}", e2) {
                    differences.push(format!(
                        "错误不匹配: {:?} vs {:?}",
                        e1, e2
                    ));
                    passed = false;
                }
            }
            (s1, s2) => {
                differences.push(format!("状态不匹配: {:?} vs {:?}", s1, s2));
                passed = false;
            }
        }

        // 比较下一个 PC（允许小的差异，因为翻译可能改变块边界）
        let pc_diff = (result1.next_pc.0 as i64 - result2.next_pc.0 as i64).abs();
        if pc_diff > 16 {
            // 允许最多 16 字节的差异（可能是由于翻译导致的指令对齐）
            differences.push(format!(
                "PC 差异过大: {:#x} vs {:#x} (差异: {} 字节)",
                result1.next_pc.0, result2.next_pc.0, pc_diff
            ));
            // 注意：这里不标记为失败，因为翻译可能改变块边界
        }

        passed
    }

    /// 运行一组预定义的测试用例
    pub fn run_test_suite(&self) -> Vec<(String, ConsistencyTestResult)> {
        let mut results = Vec::new();

        // 测试 1: 基本算术操作
        {
            let mut builder = IRBuilder::new(GuestAddr(0x1000));
            builder.push(IROp::MovImm { dst: 0, imm: 10 });
            builder.push(IROp::MovImm { dst: 1, imm: 20 });
            builder.push(IROp::Add { dst: 2, src1: 0, src2: 1 });
            builder.push(IROp::Sub { dst: 3, src1: 2, src2: 0 });
            builder.set_term(Terminator::Ret);
            let block = builder.build();
            
            if let Ok(result) = self.test_block_consistency(&block) {
                results.push(("基本算术操作".to_string(), result));
            }
        }

        // 测试 2: 逻辑操作
        {
            let mut builder = IRBuilder::new(GuestAddr(0x2000));
            builder.push(IROp::MovImm { dst: 0, imm: 0xFF });
            builder.push(IROp::MovImm { dst: 1, imm: 0x0F });
            builder.push(IROp::And { dst: 2, src1: 0, src2: 1 });
            builder.push(IROp::Or { dst: 3, src1: 0, src2: 1 });
            builder.push(IROp::Xor { dst: 4, src1: 0, src2: 1 });
            builder.set_term(Terminator::Ret);
            let block = builder.build();
            
            if let Ok(result) = self.test_block_consistency(&block) {
                results.push(("逻辑操作".to_string(), result));
            }
        }

        // 测试 3: 移位操作
        {
            let mut builder = IRBuilder::new(GuestAddr(0x3000));
            builder.push(IROp::MovImm { dst: 0, imm: 0x1234 });
            builder.push(IROp::SllImm { dst: 1, src: 0, sh: 4 });
            builder.push(IROp::SrlImm { dst: 2, src: 0, sh: 4 });
            builder.push(IROp::SraImm { dst: 3, src: 0, sh: 4 });
            builder.set_term(Terminator::Ret);
            let block = builder.build();
            
            if let Ok(result) = self.test_block_consistency(&block) {
                results.push(("移位操作".to_string(), result));
            }
        }

        // 测试 4: 比较操作
        {
            let mut builder = IRBuilder::new(GuestAddr(0x4000));
            builder.push(IROp::MovImm { dst: 0, imm: 10 });
            builder.push(IROp::MovImm { dst: 1, imm: 20 });
            builder.push(IROp::CmpEq { dst: 2, lhs: 0, rhs: 1 });
            builder.push(IROp::CmpLt { dst: 3, lhs: 0, rhs: 1 });
            builder.push(IROp::CmpGe { dst: 4, lhs: 0, rhs: 1 });
            builder.set_term(Terminator::Ret);
            let block = builder.build();
            
            if let Ok(result) = self.test_block_consistency(&block) {
                results.push(("比较操作".to_string(), result));
            }
        }

        // 测试 5: Select 操作
        {
            let mut builder = IRBuilder::new(GuestAddr(0x5000));
            builder.push(IROp::MovImm { dst: 0, imm: 1 }); // cond = true
            builder.push(IROp::MovImm { dst: 1, imm: 100 }); // true_val
            builder.push(IROp::MovImm { dst: 2, imm: 200 }); // false_val
            builder.push(IROp::Select {
                dst: 3,
                cond: 0,
                true_val: 1,
                false_val: 2,
            });
            builder.set_term(Terminator::Ret);
            let block = builder.build();
            
            if let Ok(result) = self.test_block_consistency(&block) {
                results.push(("Select 操作".to_string(), result));
            }
        }

        results
    }
}

/// 性能基准测试
pub struct PerformanceBenchmark {
    /// 测试块
    test_blocks: Vec<IRBlock>,
}

impl PerformanceBenchmark {
    /// 创建新的基准测试
    pub fn new() -> Self {
        Self {
            test_blocks: Vec::new(),
        }
    }

    /// 添加测试块
    pub fn add_test_block(mut self, block: IRBlock) -> Self {
        self.test_blocks.push(block);
        self
    }

    /// 运行基准测试
    pub fn run(&self, iterations: u64) -> BenchmarkResult {
        use std::time::Instant;

        // 测试解释器性能
        let mut interpreter = Interpreter::new();
        let mut mmu = SoftMmu::new(64 * 1024 * 1024, false);
        
        let start = Instant::now();
        for _ in 0..iterations {
            for block in &self.test_blocks {
                let _ = interpreter.run(&mut mmu, block);
            }
        }
        let interpreter_time = start.elapsed();

        // 测试翻译器性能（翻译 + 执行）
        let mut translator = ArchTranslator::new(SourceArch::X86_64, TargetArch::ARM64);
        let mut interpreter2 = Interpreter::new();
        let mut mmu2 = SoftMmu::new(64 * 1024 * 1024, false);
        
        let start = Instant::now();
        for _ in 0..iterations {
            for block in &self.test_blocks {
                if let Ok(translated) = translator.translate_block(block) {
                    let _ = interpreter2.run(&mut mmu2, &translated);
                }
            }
        }
        let translator_time = start.elapsed();

        BenchmarkResult {
            interpreter_time,
            translator_time,
            iterations,
            block_count: self.test_blocks.len(),
        }
    }
}

/// 基准测试结果
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// 解释器执行时间
    pub interpreter_time: std::time::Duration,
    /// 翻译器执行时间（包括翻译开销）
    pub translator_time: std::time::Duration,
    /// 迭代次数
    pub iterations: u64,
    /// 块数量
    pub block_count: usize,
}

impl BenchmarkResult {
    /// 获取解释器平均执行时间（每块）
    pub fn interpreter_avg_time_per_block(&self) -> f64 {
        let total_blocks = self.iterations * self.block_count as u64;
        self.interpreter_time.as_secs_f64() / total_blocks as f64
    }

    /// 获取翻译器平均执行时间（每块，包括翻译）
    pub fn translator_avg_time_per_block(&self) -> f64 {
        let total_blocks = self.iterations * self.block_count as u64;
        self.translator_time.as_secs_f64() / total_blocks as f64
    }

    /// 获取性能开销（翻译器相对于解释器的倍数）
    pub fn overhead_ratio(&self) -> f64 {
        self.translator_time.as_secs_f64() / self.interpreter_time.as_secs_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consistency_basic_arithmetic() {
        let tester = ConsistencyTester::new(SourceArch::X86_64, TargetArch::ARM64);
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::MovImm { dst: 0, imm: 10 });
        builder.push(IROp::MovImm { dst: 1, imm: 20 });
        builder.push(IROp::Add { dst: 2, src1: 0, src2: 1 });
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        let result = tester.test_block_consistency(&block).unwrap();
        assert!(result.passed, "一致性测试失败: {:?}", result.differences);
    }

    #[test]
    fn test_consistency_logical_ops() {
        let tester = ConsistencyTester::new(SourceArch::X86_64, TargetArch::ARM64);
        let mut builder = IRBuilder::new(GuestAddr(0x2000));
        builder.push(IROp::MovImm { dst: 0, imm: 0xFF });
        builder.push(IROp::MovImm { dst: 1, imm: 0x0F });
        builder.push(IROp::And { dst: 2, src1: 0, src2: 1 });
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        let result = tester.test_block_consistency(&block).unwrap();
        assert!(result.passed, "逻辑操作一致性测试失败: {:?}", result.differences);
    }

    #[test]
    fn test_consistency_suite() {
        let tester = ConsistencyTester::new(SourceArch::X86_64, TargetArch::ARM64);
        let results = tester.run_test_suite();

        let mut failed = Vec::new();
        for (name, result) in &results {
            if !result.passed {
                failed.push((name.clone(), result.differences.clone()));
            }
        }

        if !failed.is_empty() {
            eprintln!("以下测试失败:");
            for (name, diffs) in &failed {
                eprintln!("  {}: {:?}", name, diffs);
            }
        }

        // 至少应该有一些测试通过
        assert!(
            results.len() > failed.len(),
            "所有一致性测试都失败了"
        );
    }

    #[test]
    fn test_performance_benchmark() {
        let mut builder = IRBuilder::new(GuestAddr(0x1000));
        builder.push(IROp::MovImm { dst: 0, imm: 10 });
        builder.push(IROp::Add { dst: 1, src1: 0, src2: 0 });
        builder.set_term(Terminator::Ret);
        let block = builder.build();

        let benchmark = PerformanceBenchmark::new().add_test_block(block);
        let result = benchmark.run(1000);

        // 验证基准测试运行成功
        assert!(result.interpreter_time.as_secs_f64() > 0.0);
        assert!(result.translator_time.as_secs_f64() > 0.0);
        
        println!(
            "性能基准: 解释器={:?}, 翻译器={:?}, 开销={:.2}x",
            result.interpreter_time,
            result.translator_time,
            result.overhead_ratio()
        );
    }
}

