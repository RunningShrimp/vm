//! 异步混合执行器
//! 
//! 结合JIT和解释器的优势，根据热点检测动态选择执行方式

use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;

/// 执行器选择策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// 解释器模式（启动快，编译开销小）
    Interpreter,
    /// JIT模式（执行快，编译开销大）
    Jit,
}

/// 执行器统计信息
#[derive(Debug, Clone, Copy)]
pub struct ExecutionStats {
    pub total_blocks: u64,
    pub interpreter_blocks: u64,
    pub jit_blocks: u64,
    pub total_cycles: u64,
    pub total_latency_us: u64,
}

impl ExecutionStats {
    pub fn new() -> Self {
        Self {
            total_blocks: 0,
            interpreter_blocks: 0,
            jit_blocks: 0,
            total_cycles: 0,
            total_latency_us: 0,
        }
    }

    pub fn average_latency_us(&self) -> f64 {
        if self.total_blocks == 0 {
            0.0
        } else {
            self.total_latency_us as f64 / self.total_blocks as f64
        }
    }
}

/// 块执行信息
#[derive(Debug, Clone)]
pub struct BlockStats {
    pub address: u64,
    pub execution_count: u32,
    pub total_cycles: u64,
    pub mode: ExecutionMode,
}

/// 异步混合执行器配置
#[derive(Debug, Clone)]
pub struct AsyncHybridConfig {
    /// 热点阈值 (执行次数)
    pub hotspot_threshold: u32,
    /// JIT编译超时 (毫秒)
    pub jit_compile_timeout_ms: u64,
    /// 最大并发编译任务
    pub max_concurrent_compilations: usize,
    /// 是否启用预热
    pub enable_warmup: bool,
    /// 预热阈值
    pub warmup_threshold: u32,
}

impl Default for AsyncHybridConfig {
    fn default() -> Self {
        Self {
            hotspot_threshold: 100,
            jit_compile_timeout_ms: 500,
            max_concurrent_compilations: 4,
            enable_warmup: true,
            warmup_threshold: 50,
        }
    }
}

/// 异步混合执行器
pub struct AsyncHybridExecutor {
    config: AsyncHybridConfig,
    /// 块执行统计 (地址 -> BlockStats)
    block_stats: Arc<DashMap<u64, BlockStats>>,
    /// 当前执行模式 (地址 -> ExecutionMode)
    execution_modes: Arc<DashMap<u64, ExecutionMode>>,
    /// 全局统计
    global_stats: Arc<tokio::sync::Mutex<ExecutionStats>>,
    /// 后台编译任务数
    active_compilations: Arc<tokio::sync::Semaphore>,
}

impl AsyncHybridExecutor {
    /// 创建新的混合执行器
    pub fn new(config: AsyncHybridConfig) -> Self {
        Self {
            config: config.clone(),
            block_stats: Arc::new(DashMap::new()),
            execution_modes: Arc::new(DashMap::new()),
            global_stats: Arc::new(tokio::sync::Mutex::new(ExecutionStats::new())),
            active_compilations: Arc::new(tokio::sync::Semaphore::new(config.max_concurrent_compilations)),
        }
    }

    /// 确定最佳执行模式
    pub async fn decide_execution_mode(&self, block_addr: u64) -> ExecutionMode {
        // 检查是否已有决定
        if let Some(mode) = self.execution_modes.get(&block_addr) {
            return *mode;
        }

        // 检查执行计数
        let stats = self.block_stats
            .entry(block_addr)
            .or_insert_with(|| BlockStats {
                address: block_addr,
                execution_count: 0,
                total_cycles: 0,
                mode: ExecutionMode::Interpreter,
            });

        // 递增执行计数
        let new_count = stats.execution_count + 1;
        drop(stats);
        
        self.block_stats.alter(&block_addr, |_, mut bs| {
            bs.execution_count = new_count;
            bs
        });

        // 基于阈值选择模式
        if new_count >= self.config.hotspot_threshold {
            // 热点块，使用JIT
            self.execution_modes.insert(block_addr, ExecutionMode::Jit);
            
            // 异步启动后台编译任务
            let executor = Arc::new(self.clone_for_background_task());
            tokio::spawn(async move {
                executor.compile_block_async(block_addr).await;
            });
            
            ExecutionMode::Jit
        } else {
            ExecutionMode::Interpreter
        }
    }

    /// 记录块执行结果
    pub async fn record_execution(
        &self,
        block_addr: u64,
        mode: ExecutionMode,
        cycles: u64,
        latency_us: u64,
    ) {
        // 更新块统计
        if let Some(mut stats) = self.block_stats.get_mut(&block_addr) {
            stats.total_cycles += cycles;
            stats.mode = mode;
        }

        // 更新全局统计
        let mut global = self.global_stats.lock().await;
        global.total_blocks += 1;
        global.total_cycles += cycles;
        global.total_latency_us += latency_us;
        
        match mode {
            ExecutionMode::Interpreter => global.interpreter_blocks += 1,
            ExecutionMode::Jit => global.jit_blocks += 1,
        }
    }

    /// 异步编译块
    async fn compile_block_async(&self, _block_addr: u64) {
        let _permit = self.active_compilations.acquire().await;
        
        // 模拟JIT编译
        tokio::time::sleep(tokio::time::Duration::from_millis(
            self.config.jit_compile_timeout_ms / 2,
        )).await;
    }

    /// 获取全局统计信息
    pub async fn get_stats(&self) -> ExecutionStats {
        *self.global_stats.lock().await
    }

    /// 获取块的执行统计
    pub fn get_block_stats(&self, block_addr: u64) -> Option<BlockStats> {
        self.block_stats.get(&block_addr).map(|entry| entry.clone())
    }

    /// 预热（批量执行以收集数据）
    pub async fn warmup(&self, blocks: &[(u64, u32)]) {
        for (addr, _) in blocks {
            let mode = self.decide_execution_mode(*addr).await;
            self.record_execution(*addr, mode, 10, 1).await;
        }
    }

    /// 为后台任务克隆执行器
    fn clone_for_background_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            block_stats: Arc::clone(&self.block_stats),
            execution_modes: Arc::clone(&self.execution_modes),
            global_stats: Arc::clone(&self.global_stats),
            active_compilations: Arc::clone(&self.active_compilations),
        }
    }

    /// 重置统计信息
    pub async fn reset_stats(&self) {
        *self.global_stats.lock().await = ExecutionStats::new();
        self.block_stats.clear();
        self.execution_modes.clear();
    }

    /// 获取热点块列表
    pub fn get_hotspots(&self) -> Vec<(u64, BlockStats)> {
        self.block_stats
            .iter()
            .filter(|entry| entry.value().execution_count >= self.config.hotspot_threshold)
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }
}

impl Clone for AsyncHybridExecutor {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            block_stats: Arc::clone(&self.block_stats),
            execution_modes: Arc::clone(&self.execution_modes),
            global_stats: Arc::clone(&self.global_stats),
            active_compilations: Arc::clone(&self.active_compilations),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execution_mode_selection() {
        let config = AsyncHybridConfig {
            hotspot_threshold: 3,
            ..Default::default()
        };
        let executor = AsyncHybridExecutor::new(config);

        // 前3次应该使用解释器
        let mode1 = executor.decide_execution_mode(0x1000).await;
        assert_eq!(mode1, ExecutionMode::Interpreter);
        
        let mode2 = executor.decide_execution_mode(0x1000).await;
        assert_eq!(mode2, ExecutionMode::Interpreter);
        
        let mode3 = executor.decide_execution_mode(0x1000).await;
        assert_eq!(mode3, ExecutionMode::Interpreter);
        
        // 第4次应该切换到JIT
        let mode4 = executor.decide_execution_mode(0x1000).await;
        assert_eq!(mode4, ExecutionMode::Jit);
    }

    #[tokio::test]
    async fn test_execution_recording() {
        let executor = AsyncHybridExecutor::new(AsyncHybridConfig::default());

        executor.record_execution(0x1000, ExecutionMode::Interpreter, 100, 10).await;
        executor.record_execution(0x2000, ExecutionMode::Jit, 50, 5).await;

        let stats = executor.get_stats().await;
        assert_eq!(stats.total_blocks, 2);
        assert_eq!(stats.interpreter_blocks, 1);
        assert_eq!(stats.jit_blocks, 1);
        assert_eq!(stats.total_cycles, 150);
    }

    #[tokio::test]
    async fn test_hotspot_detection() {
        let config = AsyncHybridConfig {
            hotspot_threshold: 2,
            ..Default::default()
        };
        let executor = AsyncHybridExecutor::new(config);

        // 执行热点块
        for _ in 0..3 {
            let _ = executor.decide_execution_mode(0x1000).await;
        }

        let hotspots = executor.get_hotspots();
        assert_eq!(hotspots.len(), 1);
        assert_eq!(hotspots[0].0, 0x1000);
    }

    #[tokio::test]
    async fn test_concurrent_execution_decisions() {
        let executor = Arc::new(AsyncHybridExecutor::new(AsyncHybridConfig::default()));
        let mut handles = vec![];

        // 并发做出执行决定
        for i in 0..10 {
            let exec = Arc::clone(&executor);
            let handle = tokio::spawn(async move {
                exec.decide_execution_mode(i as u64).await
            });
            handles.push(handle);
        }

        // 等待所有任务完成
        for handle in handles {
            let _ = handle.await;
        }

        let stats = executor.get_stats().await;
        assert!(stats.total_blocks > 0);
    }

    #[tokio::test]
    async fn test_stats_calculation() {
        let executor = AsyncHybridExecutor::new(AsyncHybridConfig::default());

        executor.record_execution(0x1000, ExecutionMode::Interpreter, 100, 20).await;
        executor.record_execution(0x1000, ExecutionMode::Interpreter, 100, 20).await;

        let stats = executor.get_stats().await;
        assert_eq!(stats.average_latency_us(), 20.0);
    }

    #[tokio::test]
    async fn test_stats_reset() {
        let executor = AsyncHybridExecutor::new(AsyncHybridConfig::default());

        executor.record_execution(0x1000, ExecutionMode::Interpreter, 100, 10).await;
        let stats_before = executor.get_stats().await;
        assert!(stats_before.total_blocks > 0);

        executor.reset_stats().await;
        let stats_after = executor.get_stats().await;
        assert_eq!(stats_after.total_blocks, 0);
    }
}
