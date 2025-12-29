//! 执行引擎接口定义

use crate::{Configurable, ExecResult, GuestAddr, HotStats, Observable, VmComponent, VmError};
use vm_core::VcpuStateContainer;
use vm_ir::IRBlock;

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
    fn execute<M: crate::memory::MemoryManager>(&mut self, mmu: &mut M, block: &I) -> ExecResult;

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
    fn execute_async<M: crate::memory::MemoryManager + Send>(
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

/// 状态同步trait（用于Hybrid引擎）
pub trait StateSynchronizer {
    /// 从源同步状态到目标
    fn sync_state_to<E: ExecutionEngine<IRBlock>>(&mut self, target: &mut E)
    -> Result<(), VmError>;

    /// 从目标同步状态到源
    fn sync_state_from<E: ExecutionEngine<IRBlock>>(&mut self, source: &E) -> Result<(), VmError>;

    /// 检查状态是否需要同步
    fn needs_sync(&self) -> bool;
}

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
                pc: vm_core::GuestAddr(0),
                registers: [0; 32],
                float_registers: [0.0; 32],
                execution_count: 0,
                compiled_blocks: 0,
                hot_blocks: 0,
            },
            stats: ExecutionEngineStats::default(),
        }
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

    fn status(&self) -> crate::ComponentStatus {
        crate::ComponentStatus::Running
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
            return Err(VmError::Core(vm_core::CoreError::Config {
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
    ) -> crate::SubscriptionId {
        // 简化实现
        0
    }

    fn unsubscribe(&mut self, _id: crate::SubscriptionId) -> Result<(), VmError> {
        Ok(())
    }
}

impl ExecutionEngine<IRBlock> for InterpreterEngine {
    type State = ExecutionEngineState;
    type Stats = ExecutionEngineStats;

    fn execute<M: crate::memory::MemoryManager>(
        &mut self,
        _mmu: &mut M,
        _block: &IRBlock,
    ) -> ExecResult {
        // 简化实现
        self.state.execution_count += 1;
        vm_core::ExecResult {
            status: vm_core::ExecStatus::Continue,
            stats: vm_core::ExecStats::default(),
            next_pc: self.state.pc,
        }
    }

    fn get_register(&self, index: usize) -> u64 {
        self.state.registers[index]
    }

    fn set_register(&mut self, index: usize, value: u64) {
        if index < self.state.registers.len() {
            self.state.registers[index] = value;
        }
    }

    fn get_pc(&self) -> GuestAddr {
        self.state.pc
    }

    fn set_pc(&mut self, pc: GuestAddr) {
        self.state.pc = pc;
    }

    fn get_execution_state(&self) -> &<Self as ExecutionEngine<IRBlock>>::State {
        &self.state
    }

    fn get_execution_stats(&self) -> &Self::Stats {
        &self.stats
    }

    fn reset(&mut self) {
        self.state = ExecutionEngineState {
            pc: vm_core::GuestAddr(0),
            registers: [0; 32],
            float_registers: [0.0; 32],
            execution_count: 0,
            compiled_blocks: 0,
            hot_blocks: 0,
        };
        self.stats = ExecutionEngineStats::default();
    }

    fn get_vcpu_state(&self) -> VcpuStateContainer {
        VcpuStateContainer {
            vcpu_id: 0, // 默认VCPU ID
            state: vm_core::VmState {
                regs: vm_core::GuestRegs::default(),
                memory: vec![],
                pc: self.state.pc,
            },
            running: true,
        }
    }

    fn set_vcpu_state(&mut self, state: &VcpuStateContainer) {
        self.state.pc = state.state.pc;
        // 其他状态可以根据需要设置
    }

    async fn execute_async<M: crate::memory::MemoryManager + Send>(
        &mut self,
        mmu: &mut M,
        block: &IRBlock,
    ) -> ExecResult {
        // 对于解释器，异步执行就是同步执行
        self.execute(mmu, block)
    }
}
