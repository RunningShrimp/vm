//! 统一执行器 - 主流程集成核心
//!
//! 本模块实现了所有执行引擎（解释器、JIT、AOT）的统一编排层，
//! 根据策略自动选择最优执行方式，实现性能与灵活性的平衡。

use std::collections::HashMap;

use crate::{ExecResult, ExecStats, ExecutionEngine, GuestAddr, MMU, VmError};

// IR 类型定义 - 需要与 vm-irm 的类型对齐
pub struct IRBlock {
    pub start_pc: GuestAddr,
    // 简化版本，实际需要与 vm-ir 的 IRBlock 对齐
}

pub struct Instruction {
    // 占位符
}

/// 执行引擎类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineType {
    /// 解释器 - 启动快，性能较低
    Interpreter,
    /// JIT 编译 - 性能高，编译开销大
    JIT,
    /// AOT 缓存 - 最高性能，需要预编译
    AOT,
    /// 硬件加速 - 需要硬件支持
    Hardware,
}

/// 执行策略配置
#[derive(Debug, Clone)]
pub struct ExecutionPolicy {
    /// 热点阈值 - 执行次数超过此值触发 JIT
    pub hotspot_threshold: u32,
    /// 是否启用 JIT
    pub enable_jit: bool,
    /// 是否启用 AOT
    pub enable_aot: bool,
    /// 是否启用硬件加速
    pub enable_hardware: bool,
    /// 最大编译并发数
    pub max_concurrent_compilations: usize,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            hotspot_threshold: 100,
            enable_jit: true,
            enable_aot: true,
            enable_hardware: true,
            max_concurrent_compilations: 4,
        }
    }
}

/// 块执行统计
#[derive(Debug, Clone)]
struct BlockStats {
    /// 执行次数
    execution_count: u32,
    /// 最后执行时间戳
    last_executed: u64,
    /// 当前使用的引擎
    current_engine: EngineType,
    /// 是否已编译
    compiled: bool,
}

/// 统一执行器
pub struct UnifiedExecutor {
    /// 解释器引擎
    interpreter: Box<dyn ExecutionEngine<IRBlock>>,
    /// JIT 引擎 (可选)
    jit_engine: Option<Box<dyn ExecutionEngine<IRBlock>>>,
    /// AOT 缓存 (可选)
    aot_cache: Option<AotCache>,
    /// 执行策略
    policy: ExecutionPolicy,
    /// 块统计信息 (地址 -> 统计)
    block_stats: HashMap<GuestAddr, BlockStats>,
    /// 全局统计
    stats: ExecStats,
    /// 启动时间
    start_time: std::time::Instant,
}

/// AOT 缓存管理器
#[derive(Debug)]
pub struct AotCache {
    /// 缓存的编译代码
    cached_code: HashMap<GuestAddr, Vec<u8>>,
    /// 缓存命中次数
    cache_hits: u64,
    /// 缓存未命中次数
    cache_misses: u64,
}

impl AotCache {
    /// 创建新的 AOT 缓存
    pub fn new() -> Self {
        Self {
            cached_code: HashMap::new(),
            cache_hits: 0,
            cache_misses: 0,
        }
    }

    /// 查找缓存的代码
    pub fn lookup(&mut self, addr: GuestAddr) -> Option<&[u8]> {
        if let Some(code) = self.cached_code.get(&addr) {
            self.cache_hits += 1;
            Some(code)
        } else {
            self.cache_misses += 1;
            None
        }
    }

    /// 插入编译后的代码
    pub fn insert(&mut self, addr: GuestAddr, code: Vec<u8>) {
        self.cached_code.insert(addr, code);
    }

    /// 获取缓存统计
    pub fn stats(&self) -> (u64, u64) {
        (self.cache_hits, self.cache_misses)
    }
}

impl Default for AotCache {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedExecutor {
    /// 创建新的统一执行器
    ///
    /// # 参数
    /// - `interpreter`: 解释器引擎 (必需)
    /// - `jit_engine`: JIT 引擎 (可选)
    /// - `policy`: 执行策略
    pub fn new(
        interpreter: Box<dyn ExecutionEngine<IRBlock>>,
        jit_engine: Option<Box<dyn ExecutionEngine<IRBlock>>>,
        policy: ExecutionPolicy,
    ) -> Self {
        let aot_cache = if policy.enable_aot {
            Some(AotCache::new())
        } else {
            None
        };

        Self {
            interpreter,
            jit_engine,
            aot_cache,
            policy,
            block_stats: HashMap::new(),
            stats: ExecStats::default(),
            start_time: std::time::Instant::now(),
        }
    }

    /// 使用默认策略创建
    pub fn with_defaults(interpreter: Box<dyn ExecutionEngine<IRBlock>>) -> Self {
        Self::new(interpreter, None, ExecutionPolicy::default())
    }

    /// 选择执行引擎
    ///
    /// 根据块地址和执行统计，自动选择最优执行引擎
    fn select_engine(&mut self, block_addr: GuestAddr) -> EngineType {
        // 1. 检查 AOT 缓存
        if let Some(ref mut cache) = self.aot_cache {
            if cache.lookup(block_addr).is_some() {
                return EngineType::AOT;
            }
        }

        // 2. 检查块统计
        if let Some(stats) = self.block_stats.get(&block_addr) {
            // 热点检测：执行次数超过阈值，使用 JIT
            if stats.execution_count >= self.policy.hotspot_threshold {
                if self.policy.enable_jit && self.jit_engine.is_some() {
                    return EngineType::JIT;
                }
            }

            // 保持当前引擎
            return stats.current_engine;
        }

        // 3. 新块，使用解释器
        EngineType::Interpreter
    }

    /// 更新块统计
    fn update_block_stats(&mut self, block_addr: GuestAddr, engine: EngineType) {
        let stats = self
            .block_stats
            .entry(block_addr)
            .or_insert_with(|| BlockStats {
                execution_count: 0,
                last_executed: 0,
                current_engine: EngineType::Interpreter,
                compiled: false,
            });

        stats.execution_count += 1;
        stats.last_executed = self.start_time.elapsed().as_secs();
        stats.current_engine = engine;

        // 标记为已编译（如果使用 JIT 或 AOT）
        if matches!(engine, EngineType::JIT | EngineType::AOT) {
            stats.compiled = true;
        }
    }
}

/// 统一执行器实现 ExecutionEngine trait
impl ExecutionEngine<IRBlock> for UnifiedExecutor {
    fn execute_instruction(&mut self, instruction: &crate::Instruction) -> Result<(), VmError> {
        // 委托给解释器执行单条指令
        self.interpreter.execute_instruction(instruction)
    }

    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock) -> ExecResult {
        let block_addr = block.start_pc;

        // 选择执行引擎
        let engine_type = self.select_engine(block_addr);

        // 执行块
        let result = match engine_type {
            EngineType::Interpreter => {
                // 解释器执行
                self.interpreter.run(mmu, block)
            }
            EngineType::JIT => {
                // JIT 执行
                if let Some(ref mut jit) = self.jit_engine {
                    jit.run(mmu, block)
                } else {
                    // JIT 不可用，回退到解释器
                    self.interpreter.run(mmu, block)
                }
            }
            EngineType::AOT => {
                // AOT 执行（当前简化为解释器）
                // TODO: 实际实现需要从缓存加载并执行编译后的代码
                self.interpreter.run(mmu, block)
            }
            EngineType::Hardware => {
                // 硬件加速执行（当前未实现）
                // 回退到解释器
                self.interpreter.run(mmu, block)
            }
        };

        // 更新统计
        self.update_block_stats(block_addr, engine_type);

        // 合并执行统计
        result
    }

    fn get_reg(&self, idx: usize) -> u64 {
        self.interpreter.get_reg(idx)
    }

    fn set_reg(&mut self, idx: usize, val: u64) {
        self.interpreter.set_reg(idx, val);
    }

    fn get_pc(&self) -> GuestAddr {
        self.interpreter.get_pc()
    }

    fn set_pc(&mut self, pc: GuestAddr) {
        self.interpreter.set_pc(pc);
    }

    fn get_vcpu_state(&self) -> crate::VcpuStateContainer {
        self.interpreter.get_vcpu_state()
    }

    fn set_vcpu_state(&mut self, state: &crate::VcpuStateContainer) {
        self.interpreter.set_vcpu_state(state);
    }
}

/// 辅助函数：创建默认的统一执行器
pub fn create_default_executor() -> Result<UnifiedExecutor, VmError> {
    // 注意：这里需要实际的解释器实现
    // 当前返回错误，需要集成实际的解释器
    Err(VmError::Core(crate::CoreError::Internal {
        message: "需要集成实际的解释器实现".to_string(),
        module: "unified_executor".to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_policy_default() {
        let policy = ExecutionPolicy::default();
        assert_eq!(policy.hotspot_threshold, 100);
        assert!(policy.enable_jit);
        assert!(policy.enable_aot);
    }

    #[test]
    fn test_aot_cache_operations() {
        let mut cache = AotCache::new();
        let addr = GuestAddr(0x1000);
        let code = vec![0x90, 0x90, 0x90]; // NOP instructions

        // 初始状态：未命中
        assert!(cache.lookup(addr).is_none());
        assert_eq!(cache.stats().1, 1); // 1 次未命中

        // 插入代码
        cache.insert(addr, code.clone());

        // 查找成功
        let result = cache.lookup(addr);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &code[..]);
        assert_eq!(cache.stats().0, 1); // 1 次命中
    }

    #[test]
    fn test_engine_type_selection() {
        let mut cache = AotCache::new();
        let addr = GuestAddr(0x1000);

        // 无缓存时默认返回解释器
        let mut executor = create_test_executor();
        let engine = executor.select_engine(addr);
        assert_eq!(engine, EngineType::Interpreter);

        // 添加 AOT 缓存后应选择 AOT
        cache.insert(addr, vec![0x90]);
        executor.aot_cache = Some(cache);
        let engine = executor.select_engine(addr);
        assert_eq!(engine, EngineType::AOT);
    }

    #[test]
    fn test_block_stats_tracking() {
        let mut executor = create_test_executor();
        let addr = GuestAddr(0x1000);

        // 初始状态：无统计
        assert!(!executor.block_stats.contains_key(&addr));

        // 更新统计
        executor.update_block_stats(addr, EngineType::Interpreter);
        assert!(executor.block_stats.contains_key(&addr));

        {
            let stats = &executor.block_stats[&addr];
            assert_eq!(stats.execution_count, 1);
            assert_eq!(stats.current_engine, EngineType::Interpreter);
            assert!(!stats.compiled);
        }

        // 再次更新
        executor.update_block_stats(addr, EngineType::JIT);
        let stats = &executor.block_stats[&addr];
        assert_eq!(stats.execution_count, 2);
        assert_eq!(stats.current_engine, EngineType::JIT);
        assert!(stats.compiled);
    }

    // 辅助函数：创建测试执行器
    fn create_test_executor() -> UnifiedExecutor {
        // 注意：这里使用占位符，实际需要真实的解释器
        // 为了测试通过，我们只测试策略逻辑
        UnifiedExecutor {
            interpreter: Box::new(TestInterpreter),
            jit_engine: None,
            aot_cache: None,
            policy: ExecutionPolicy::default(),
            block_stats: HashMap::new(),
            stats: ExecStats::default(),
            start_time: std::time::Instant::now(),
        }
    }

    // 测试用的占位解释器
    struct TestInterpreter;

    impl ExecutionEngine<IRBlock> for TestInterpreter {
        fn execute_instruction(
            &mut self,
            _instruction: &crate::Instruction,
        ) -> Result<(), VmError> {
            Ok(())
        }

        fn run(&mut self, _mmu: &mut dyn MMU, _block: &IRBlock) -> ExecResult {
            ExecResult {
                status: crate::ExecStatus::Ok,
                stats: ExecStats::default(),
                next_pc: GuestAddr(0),
            }
        }

        fn get_reg(&self, _idx: usize) -> u64 {
            0
        }

        fn set_reg(&mut self, _idx: usize, _val: u64) {}

        fn get_pc(&self) -> GuestAddr {
            GuestAddr(0)
        }

        fn set_pc(&mut self, _pc: GuestAddr) {}

        fn get_vcpu_state(&self) -> crate::VcpuStateContainer {
            crate::VcpuStateContainer::default()
        }

        fn set_vcpu_state(&mut self, _state: &crate::VcpuStateContainer) {}
    }
}
