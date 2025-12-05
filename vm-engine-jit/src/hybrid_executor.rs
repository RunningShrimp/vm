//! 混合执行引擎：AOT + JIT 混合执行策略
//!
//! 实现查找顺序：AOT → JIT(即时编译) → Cold(解释执行)
//!
//! 回退机制：
//! - AOT代码执行失败 → 回退到JIT编译
//! - JIT编译失败或未命中 → 回退到解释执行
//! - 支持AOT代码的延迟加载和链接

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use vm_core::{ExecResult, ExecStats, ExecStatus, ExecutionEngine, GuestAddr, MMU, VmError};
use vm_ir::IRBlock;

use crate::aot_loader::{AotCodeBlock, AotLoader};

/// 混合执行引擎的代码块来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodeSource {
    /// 来自 AOT 镜像
    AotImage,
    /// 来自 JIT 即时编译
    JitCompiled,
    /// 解释执行（回退）
    Interpreted,
}

/// 执行统计信息
#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    /// 各来源的执行次数
    pub source_hits: Arc<RwLock<HashMap<CodeSource, u64>>>,
}

impl ExecutionStats {
    /// 创建新的执行统计
    pub fn new() -> Self {
        Self {
            source_hits: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 更新执行来源计数
    pub fn update(&self, source: CodeSource) {
        let mut hits = self.source_hits.write();
        *hits.entry(source).or_insert(0) += 1;
    }

    /// 获取指定来源的命中次数
    pub fn get_hits(&self, source: CodeSource) -> u64 {
        self.source_hits.read().get(&source).copied().unwrap_or(0)
    }

    /// 打印执行统计信息
    pub fn print_stats(&self) {
        let hits = self.source_hits.read();
        let total: u64 = hits.values().sum();
        if total > 0 {
            for source in &[
                CodeSource::AotImage,
                CodeSource::JitCompiled,
                CodeSource::Interpreted,
            ] {
                let count = hits.get(source).copied().unwrap_or(0);
                let pct = (count as f64 / total as f64) * 100.0;
                println!("  {:?}: {} ({:.1}%)", source, count, pct);
            }
        }
    }
}

/// AOT执行失败原因
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AotFailureReason {
    /// 代码块未找到
    BlockNotFound,
    /// 代码块未解析/链接
    BlockNotResolved,
    /// 执行失败
    ExecutionFailed,
    /// 依赖未满足
    DependencyMissing,
}

/// 混合执行器
#[derive(Clone)]
pub struct HybridExecutor {
    /// AOT 加载器（可选）
    aot_loader: Option<Arc<AotLoader>>,
    /// 执行统计
    stats: Arc<ExecutionStats>,
    /// 块访问计数（用于热点检测）
    access_counts: Arc<RwLock<HashMap<GuestAddr, u64>>>,
    /// AOT失败计数（用于回退决策）
    aot_failures: Arc<RwLock<HashMap<GuestAddr, (u64, AotFailureReason)>>>,
    /// 是否启用AOT延迟加载
    enable_lazy_loading: bool,
    /// AOT执行失败阈值（超过此值后回退到JIT）
    aot_failure_threshold: u64,
}

impl HybridExecutor {
    /// 创建新的混合执行器
    pub fn new(aot_loader: Option<Arc<AotLoader>>) -> Self {
        Self {
            aot_loader,
            stats: Arc::new(ExecutionStats::new()),
            access_counts: Arc::new(RwLock::new(HashMap::new())),
            aot_failures: Arc::new(RwLock::new(HashMap::new())),
            enable_lazy_loading: true,
            aot_failure_threshold: 3, // 默认3次失败后回退
        }
    }

    /// 创建带配置的混合执行器
    pub fn with_config(
        aot_loader: Option<Arc<AotLoader>>,
        enable_lazy_loading: bool,
        aot_failure_threshold: u64,
    ) -> Self {
        Self {
            aot_loader,
            stats: Arc::new(ExecutionStats::new()),
            access_counts: Arc::new(RwLock::new(HashMap::new())),
            aot_failures: Arc::new(RwLock::new(HashMap::new())),
            enable_lazy_loading,
            aot_failure_threshold,
        }
    }

    /// 获取执行统计
    pub fn stats(&self) -> Arc<ExecutionStats> {
        self.stats.clone()
    }

    /// 执行 IR 块，返回 (ExecResult, 代码来源)
    pub fn lookup_and_execute(
        &self,
        pc: GuestAddr,
        block: &IRBlock,
        mmu: &mut dyn MMU,
        jit: &mut crate::Jit,
    ) -> (ExecResult, CodeSource) {
        // 记录访问
        {
            let mut counts = self.access_counts.write();
            let count = counts.entry(pc).or_insert(0);
            *count += 1;
        }

        // 检查是否应该跳过AOT（因为失败次数过多）
        let should_skip_aot = {
            let failures = self.aot_failures.read();
            if let Some((count, _)) = failures.get(&pc) {
                *count >= self.aot_failure_threshold
            } else {
                false
            }
        };

        // 1. 优先检查 AOT 镜像（如果未超过失败阈值）
        if !should_skip_aot {
            if let Some(aot_loader) = &self.aot_loader {
                match self.try_execute_aot(pc, block, mmu, aot_loader) {
                    Ok(result) => {
                        // 清除失败计数（成功执行）
                        {
                            let mut failures = self.aot_failures.write();
                            failures.remove(&pc);
                        }
                        self.stats.update(CodeSource::AotImage);
                        tracing::debug!("AOT execution successful for PC: {:#x}", pc);
                        return (result, CodeSource::AotImage);
                    }
                    Err(reason) => {
                        // 记录AOT失败
                        {
                            let mut failures = self.aot_failures.write();
                            let (count, _) = failures.entry(pc).or_insert((0, reason));
                            *count += 1;
                        }
                        tracing::debug!(
                            "AOT execution failed for PC: {:#x}, reason: {:?}, falling back to JIT",
                            pc,
                            reason
                        );
                        // 继续到JIT回退
                    }
                }
            }
        }

        // 2. JIT 编译（回退路径1）
        if jit.is_hot(pc) {
            match self.try_execute_jit(pc, block, mmu, jit) {
                Ok(result) => {
                    self.stats.update(CodeSource::JitCompiled);
                    tracing::debug!("JIT execution successful for PC: {:#x}", pc);
                    return (result, CodeSource::JitCompiled);
                }
                Err(_) => {
                    tracing::debug!(
                        "JIT execution failed for PC: {:#x}, falling back to interpreter",
                        pc
                    );
                    // 继续到解释器回退
                }
            }
        }

        // 3. 最终回退：解释执行
        self.stats.update(CodeSource::Interpreted);
        tracing::debug!("Fallback interpretation for PC: {:#x}", pc);

        let result = jit.run(mmu, block);
        (result, CodeSource::Interpreted)
    }

    /// 尝试执行AOT代码
    fn try_execute_aot(
        &self,
        pc: GuestAddr,
        block: &IRBlock,
        mmu: &mut dyn MMU,
        aot_loader: &AotLoader,
    ) -> Result<ExecResult, AotFailureReason> {
        // 1. 查找AOT代码块
        let aot_block = match aot_loader.lookup_block(pc) {
            Some(block) => block,
            None => {
                // 如果启用延迟加载，尝试加载
                if self.enable_lazy_loading {
                    match aot_loader.load_code_block(pc) {
                        Ok(Some(block)) => block,
                        Ok(None) => return Err(AotFailureReason::BlockNotFound),
                        Err(_) => return Err(AotFailureReason::BlockNotFound),
                    }
                } else {
                    return Err(AotFailureReason::BlockNotFound);
                }
            }
        };

        // 2. 验证代码块完整性（检查依赖）
        if let Err(_) = aot_loader.validate_block_integrity(pc) {
            // 尝试链接代码块
            if self.enable_lazy_loading {
                if let Err(_) = aot_loader.link_code_block(pc) {
                    return Err(AotFailureReason::DependencyMissing);
                }
            } else {
                return Err(AotFailureReason::BlockNotResolved);
            }
        }

        // 3. 执行AOT代码
        match self.execute_aot_code(&aot_block, mmu) {
            Ok(result) => Ok(result),
            Err(_) => Err(AotFailureReason::ExecutionFailed),
        }
    }

    /// 执行AOT编译的代码
    fn execute_aot_code(
        &self,
        aot_block: &AotCodeBlock,
        mmu: &mut dyn MMU,
    ) -> Result<ExecResult, VmError> {
        // 注意：这是一个简化的实现
        // 在实际实现中，需要：
        // 1. 设置执行上下文（寄存器、MMU等）
        // 2. 调用编译后的代码
        // 3. 处理返回值和异常

        // 验证代码块大小
        if aot_block.size == 0 {
            return Err(vm_core::VmError::Core(vm_core::CoreError::InvalidState {
                message: "Empty AOT block".to_string(),
                current: "size_zero".to_string(),
                expected: "size_nonzero".to_string(),
            }));
        }

        unsafe {
            // 将代码指针转换为函数指针
            // 函数签名: (regs: &mut [u64; 32], ctx: &mut JitContext, fregs: &mut [f64; 32]) -> u64
            // 这与JIT编译的代码使用相同的调用约定
            type AotCodeFn = fn(&mut [u64; 32], &mut crate::JitContext, &mut [f64; 32]) -> u64;
            let code_fn: AotCodeFn = std::mem::transmute(aot_block.host_addr);

            // 创建临时寄存器（实际应该从执行上下文获取）
            let mut regs = [0u64; 32];
            let mut fregs = [0.0f64; 32];
            let mut jit_ctx = crate::JitContext { mmu };

            // 执行代码
            let next_pc = code_fn(&mut regs, &mut jit_ctx, &mut fregs);

            Ok(ExecResult {
                status: ExecStatus::Ok,
                stats: ExecStats::default(),
                next_pc,
            })
        }
    }

    /// 尝试执行JIT编译的代码
    fn try_execute_jit(
        &self,
        pc: GuestAddr,
        block: &IRBlock,
        mmu: &mut dyn MMU,
        jit: &mut crate::Jit,
    ) -> Result<ExecResult, VmError> {
        // 首先检查异步编译是否已完成
        if let Some(code_ptr) = jit.check_async_compile(pc) {
            // 异步编译已完成，检查结果是否有效
            if !code_ptr.0.is_null() {
                // 编译成功，执行JIT编译的代码
                return Ok(jit.run(mmu, block));
            } else {
                // 编译失败，回退到解释器
                tracing::debug!(
                    "Async compilation failed for PC: {:#x}, falling back to interpreter",
                    pc
                );
                return Err(VmError::Core(vm_core::CoreError::CompilationFailed {
                    message: "Async compilation returned null pointer".to_string(),
                }));
            }
        }

        // 检查是否已编译（同步缓存）
        if jit.is_hot(pc) {
            // 已编译，直接执行
            return Ok(jit.run(mmu, block));
        }

        // 未编译，启动异步编译
        let block_clone = block.clone();
        let compile_timeout = std::time::Duration::from_millis(100); // 100ms超时
        let compile_start = std::time::Instant::now();
        
        // 启动异步编译
        let _handle = jit.compile_async(block_clone);
        
        // 等待编译完成（带超时）
        // 注意：由于我们在同步上下文中，我们需要使用block_on
        // 但为了不阻塞，我们设置一个短超时
        let compile_result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // 在异步上下文中，可以等待编译完成
            handle.block_on(async {
                tokio::time::timeout(compile_timeout, async {
                    // 轮询检查编译结果
                    loop {
                        if let Some(code_ptr) = jit.check_async_compile(pc) {
                            return Some(code_ptr);
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
                    }
                })
                .await
            })
        } else {
            // 不在异步上下文中，使用同步编译作为回退
            None
        };

        match compile_result {
            Some(Ok(Some(code_ptr))) if !code_ptr.0.is_null() => {
                // 异步编译成功
                tracing::debug!(
                    "Async compilation completed for PC: {:#x} in {:?}",
                    pc,
                    compile_start.elapsed()
                );
                Ok(jit.run(mmu, block))
            }
            Some(Ok(Some(_))) => {
                // 编译返回null指针（超时或失败）
                tracing::debug!(
                    "Async compilation failed (null pointer) for PC: {:#x}, falling back to interpreter",
                    pc
                );
                Err(VmError::Core(vm_core::CoreError::CompilationFailed {
                    message: "Async compilation returned null pointer".to_string(),
                }))
            }
            Some(Err(_)) | None => {
                // 编译超时或不在异步上下文，回退到同步编译或解释器
                tracing::debug!(
                    "Async compilation timeout or not in async context for PC: {:#x}, using fallback",
                    pc
                );
                
                // 尝试同步编译（快速路径）
                let sync_code_ptr = jit.compile(block);
                if !sync_code_ptr.0.is_null() {
                    Ok(jit.run(mmu, block))
                } else {
                    // 同步编译也失败，回退到解释器
                    Err(VmError::Core(vm_core::CoreError::CompilationFailed {
                        message: "Both async and sync compilation failed".to_string(),
                    }))
                }
            }
            _ => {
                // 其他情况，回退到解释器
                Err(VmError::Core(vm_core::CoreError::CompilationFailed {
                    message: "Compilation check failed".to_string(),
                }))
            }
        }
    }

    /// 获取块的访问计数
    pub fn get_access_count(&self, pc: GuestAddr) -> u64 {
        self.access_counts.read().get(&pc).copied().unwrap_or(0)
    }

    /// 获取AOT失败信息
    pub fn get_aot_failure_info(&self, pc: GuestAddr) -> Option<(u64, AotFailureReason)> {
        self.aot_failures.read().get(&pc).copied()
    }

    /// 重置AOT失败计数（用于重新尝试AOT执行）
    pub fn reset_aot_failures(&self, pc: Option<GuestAddr>) {
        let mut failures = self.aot_failures.write();
        if let Some(pc) = pc {
            failures.remove(&pc);
        } else {
            failures.clear();
        }
    }

    /// 清空统计信息
    pub fn reset_stats(&self) {
        self.stats.source_hits.write().clear();
        self.access_counts.write().clear();
        self.aot_failures.write().clear();
    }

    /// 设置AOT失败阈值
    pub fn set_aot_failure_threshold(&mut self, threshold: u64) {
        self.aot_failure_threshold = threshold;
    }

    /// 启用/禁用AOT延迟加载
    pub fn set_lazy_loading(&mut self, enable: bool) {
        self.enable_lazy_loading = enable;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_creation() {
        let executor = HybridExecutor::new(None);
        assert_eq!(executor.get_access_count(0x1000), 0);
    }

    #[test]
    fn test_statistics_tracking() {
        let stats = ExecutionStats::new();
        stats.update(CodeSource::Interpreted);
        stats.update(CodeSource::Interpreted);
        stats.update(CodeSource::AotImage);

        assert_eq!(stats.get_hits(CodeSource::Interpreted), 2);
        assert_eq!(stats.get_hits(CodeSource::AotImage), 1);
        assert_eq!(stats.get_hits(CodeSource::JitCompiled), 0);
    }

    #[test]
    fn test_code_source_equality() {
        let s1 = CodeSource::AotImage;
        let s2 = CodeSource::AotImage;
        let s3 = CodeSource::Interpreted;

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }
}
