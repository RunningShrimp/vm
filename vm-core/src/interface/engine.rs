//! 执行引擎接口定义

use super::{Configurable, HotStats, Observable, VmComponent};
use crate::{ExecResult, GuestAddr, VcpuStateContainer, VmError};

// 注意：IRBlock 是泛型参数，避免循环依赖 vm-ir
// 实际使用时通过 ExecutionEngine<vm_ir::IRBlock> 具体化

/// 执行引擎配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionEngineConfig {
    /// 引擎类型
    pub engine_type: ExecutionEngineType,
    /// 热点阈值
    pub hot_threshold: u64,
    /// 最大编译队列大小
    pub max_compile_queue: usize,
    /// 启用性能监控
    pub enable_monitoring: bool,
    /// 异步执行支持
    pub async_support: bool,
}

impl Default for ExecutionEngineConfig {
    fn default() -> Self {
        Self {
            engine_type: ExecutionEngineType::Hybrid,
            hot_threshold: 100,
            max_compile_queue: 1000,
            enable_monitoring: true,
            async_support: true,
        }
    }
}

/// 执行引擎类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExecutionEngineType {
    Interpreter,
    Jit,
    Accelerated,
    Hybrid,
}

/// 执行引擎状态
#[derive(Debug, Clone)]
pub struct ExecutionEngineState {
    pub pc: GuestAddr,
    pub registers: [u64; 32],
    pub float_registers: [f64; 32],
    pub execution_count: u64,
    pub compiled_blocks: usize,
    pub hot_blocks: usize,
}

/// 执行引擎统计
#[derive(Debug, Clone, Default)]
pub struct ExecutionEngineStats {
    pub total_instructions: u64,
    pub total_cycles: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub compilation_time_ns: u64,
    pub execution_time_ns: u64,
}

/// 扩展的执行引擎trait
pub trait ExecutionEngine<I>: VmComponent {
    type State;
    type Stats;

    /// 执行IR块
    fn execute<M: super::super::mmu_traits::MMU>(&mut self, mmu: &mut M, block: &I) -> ExecResult;

    /// 获取寄存器值
    fn get_register(&self, index: usize) -> u64;

    /// 设置寄存器值
    fn set_register(&mut self, index: usize, value: u64);

    /// 获取程序计数器
    fn get_pc(&self) -> GuestAddr;

    /// 设置程序计数器
    fn set_pc(&mut self, pc: GuestAddr);

    /// 获取执行状态
    fn get_execution_state(&self) -> &Self::State;

    /// 获取执行统计
    fn get_execution_stats(&self) -> &Self::Stats;

    /// 重置执行状态
    fn reset(&mut self);

    /// 获取vCPU状态
    fn get_vcpu_state(&self) -> VcpuStateContainer;

    /// 设置vCPU状态
    fn set_vcpu_state(&mut self, state: &VcpuStateContainer);

    /// 异步执行版本
    fn execute_async<M: super::super::mmu_traits::MMU + Send>(
        &mut self,
        mmu: &mut M,
        block: &I,
    ) -> impl std::future::Future<Output = ExecResult> + Send;
}

/// 热编译管理trait（用于JIT和Hybrid引擎）
pub trait HotCompilationManager {
    /// 设置热点阈值
    fn set_hot_threshold(&mut self, min: u64, max: u64);

    /// 获取热点统计
    fn get_hot_stats(&self) -> &HotStats;

    /// 清除热点缓存
    fn clear_hot_cache(&mut self);

    /// 预编译块
    fn precompile_block(&mut self, address: GuestAddr) -> Result<(), VmError>;
}

// Note: StateSynchronizer trait removed to avoid circular dependency with vm-ir.
// This should be implemented in crates that depend on both vm-core and vm-ir.

/// 解释器引擎实现
pub struct InterpreterEngine {
    config: ExecutionEngineConfig,
    state: ExecutionEngineState,
    stats: ExecutionEngineStats,
}

impl InterpreterEngine {
    pub fn new(config: ExecutionEngineConfig) -> Self {
        Self {
            config,
            state: ExecutionEngineState {
                pc: crate::GuestAddr(0),
                registers: [0; 32],
                float_registers: [0.0; 32],
                execution_count: 0,
                compiled_blocks: 0,
                hot_blocks: 0,
            },
            stats: ExecutionEngineStats::default(),
        }
    }

    /// 获取执行统计信息
    pub fn get_stats(&self) -> &ExecutionEngineStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.stats = ExecutionEngineStats::default();
    }

    /// 更新执行统计信息
    pub fn update_stats<F>(&mut self, updater: F)
    where
        F: FnOnce(&mut ExecutionEngineStats),
    {
        updater(&mut self.stats);
    }
}

impl VmComponent for InterpreterEngine {
    type Config = ExecutionEngineConfig;
    type Error = VmError;

    fn init(config: Self::Config) -> Result<Self, Self::Error> {
        Ok(Self::new(config))
    }

    fn start(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn status(&self) -> super::ComponentStatus {
        super::ComponentStatus::Running
    }

    fn name(&self) -> &str {
        "InterpreterEngine"
    }
}

impl Configurable for InterpreterEngine {
    type Config = ExecutionEngineConfig;

    fn update_config(&mut self, config: &Self::Config) -> Result<(), VmError> {
        self.config = config.clone();
        Ok(())
    }

    fn get_config(&self) -> &Self::Config {
        &self.config
    }

    fn validate_config(config: &Self::Config) -> Result<(), VmError> {
        if config.hot_threshold == 0 {
            return Err(VmError::Core(crate::CoreError::Config {
                message: "hot_threshold must be greater than 0".to_string(),
                path: Some("hot_threshold".to_string()),
            }));
        }
        Ok(())
    }
}

impl Observable for InterpreterEngine {
    type State = ExecutionEngineState;
    type Event = crate::VmEvent;

    fn get_state(&self) -> &Self::State {
        &self.state
    }

    fn subscribe(
        &mut self,
        _callback: Box<dyn Fn(&Self::State, &Self::Event) + Send + Sync>,
    ) -> super::SubscriptionId {
        // 简化实现
        0
    }

    fn unsubscribe(&mut self, _id: super::SubscriptionId) -> Result<(), VmError> {
        Ok(())
    }
}

// Note: ExecutionEngine<vm_ir::IRBlock> implementation for InterpreterEngine removed
// to avoid circular dependency with vm-ir. Concrete implementations should be
// provided in crates that depend on both vm-core and vm-ir.
