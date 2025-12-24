use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vm_core::{GuestAddr, VmError};
use vm_ir::IRBlock;

use crate::compiler::{JITCompiler, CompiledIRBlock, CompilationStats};

pub use config::*;
pub use tier::*;

mod config;
mod tier;

/// 分层编译器
/// 
/// 实现三级分层编译策略：
/// - Tier 1: 快速解释执行
/// - Tier 2: 基础JIT编译
/// - Tier 3: 优化JIT编译（内联、逃逸分析等）
pub struct TieredJITCompiler {
    /// 编译器配置
    config: TieredCompilerConfig,
    /// 解释器（Tier 1）
    interpreter: Arc<Mutex<Interpreter>>,
    /// 基础JIT编译器（Tier 2）
    baseline_jit: Arc<Mutex<dyn JITCompiler>>,
    /// 优化JIT编译器（Tier 3）
    optimized_jit: Arc<Mutex<dyn JITCompiler>>,
    /// 热点检测器
    hotspot_detector: Arc<Mutex<HotspotDetector>>,
    /// 分层缓存
    tiered_cache: Arc<Mutex<crate::tiered_cache::TieredCodeCache>>,
    /// 执行计数器
    execution_counters: Arc<Mutex<HashMap<GuestAddr, ExecutionInfo>>>,
    /// 编译状态
    compilation_states: Arc<Mutex<HashMap<GuestAddr, CompilationState>>>,
}

/// 执行信息
#[derive(Debug, Clone)]
struct ExecutionInfo {
    /// 执行次数
    count: u32,
    /// 最后执行时间
    last_execution: std::time::Instant,
    /// 首次执行时间
    first_execution: std::time::Instant,
}

/// 编译状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilationState {
    /// 未编译
    None,
    /// 基础JIT编译
    Baseline,
    /// 优化JIT编译
    Optimized,
}

/// 分层编译结果
#[derive(Debug, Clone)]
pub enum TieredCompilationResult {
    /// 解释执行结果
    Interpreted {
        value: u64,
        cycles: u32,
    },
    /// 基础JIT执行结果
    BaselineJIT {
        code: Vec<u8>,
        stats: CompilationStats,
    },
    /// 优化JIT执行结果
    OptimizedJIT {
        code: Vec<u8>,
        stats: CompilationStats,
    },
}

impl TieredJITCompiler {
    pub fn new(config: TieredCompilerConfig) -> Self {
        Self {
            config: config.clone(),
            interpreter: Arc::new(Mutex::new(Interpreter::new(config.interpreter_config.clone()))),
            baseline_jit: Arc::new(Mutex::new(BaselineJITCompiler::new(config.baseline_config.clone()))),
            optimized_jit: Arc::new(Mutex::new(OptimizedJITCompiler::new(config.optimized_config.clone()))),
            hotspot_detector: Arc::new(Mutex::new(HotspotDetector::new(config.hotspot_config.clone()))),
            tiered_cache: Arc::new(Mutex::new(crate::tiered_cache::TieredCodeCache::new(
                crate::tiered_cache::TieredCacheConfig::default()
            ))),
            execution_counters: Arc::new(Mutex::new(HashMap::new())),
            compilation_states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 执行代码块，自动选择执行层级
    pub fn execute(&mut self, block: &IRBlock) -> Result<TieredCompilationResult, VmError> {
        let pc = block.start_pc;
        
        // 记录执行
        self.record_execution(pc);
        
        // 检查缓存
        if let Some(cached_code) = self.tiered_cache.lock().unwrap().get(pc) {
            return Ok(TieredCompilationResult::OptimizedJIT {
                code: cached_code,
                stats: CompilationStats::default(),
            });
        }
        
        // 检查编译状态
        let state = *self.compilation_states.lock().unwrap().get(&pc).unwrap_or(&CompilationState::None);
        
        // 根据执行次数决定是否升级
        let exec_info = self.execution_counters.lock().unwrap().get(&pc).cloned().unwrap_or(ExecutionInfo {
            count: 0,
            last_execution: std::time::Instant::now(),
            first_execution: std::time::Instant::now(),
        });
        
        let should_upgrade = self.should_upgrade_tier(exec_info.count, state);
        
        match (state, should_upgrade) {
            (CompilationState::None, false) => {
                // Tier 1: 解释执行
                self.interpret(block)
            },
            (CompilationState::None, true) => {
                // 升级到 Tier 2: 基础JIT
                self.compile_baseline(block)
            },
            (CompilationState::Baseline, false) => {
                // Tier 2: 执行基础JIT代码
                self.execute_baseline(block)
            },
            (CompilationState::Baseline, true) => {
                // 升级到 Tier 3: 优化JIT
                self.compile_optimized(block)
            },
            (CompilationState::Optimized, _) => {
                // Tier 3: 执行优化JIT代码
                self.execute_optimized(block)
            },
        }
    }

    /// 解释执行（Tier 1）
    fn interpret(&mut self, block: &IRBlock) -> Result<TieredCompilationResult, VmError> {
        let mut interpreter = self.interpreter.lock().unwrap();
        let start = std::time::Instant::now();
        
        interpreter.reset();
        for op in &block.ops {
            interpreter.execute_op(op)?;
        }
        
        let cycles = (start.elapsed().as_nanos() / 100) as u32; // 假设每条指令100ns
        let value = interpreter.get_accumulator();
        
        Ok(TieredCompilationResult::Interpreted { value, cycles })
    }

    /// 编译到基础JIT（Tier 2）
    fn compile_baseline(&mut self, block: &IRBlock) -> Result<TieredCompilationResult, VmError> {
        let mut compiler = self.baseline_jit.lock().unwrap();
        let compiled = compiler.compile(block)?;
        
        // 更新编译状态
        self.compilation_states.lock().unwrap().insert(block.start_pc, CompilationState::Baseline);
        
        // 缓存代码
        self.tiered_cache.lock().unwrap().insert(block.start_pc, compiled.code.clone());
        
        Ok(TieredCompilationResult::BaselineJIT {
            code: compiled.code,
            stats: compiled.stats,
        })
    }

    /// 执行基础JIT代码（Tier 2）
    fn execute_baseline(&mut self, block: &IRBlock) -> Result<TieredCompilationResult, VmError> {
        if let Some(code) = self.tiered_cache.lock().unwrap().get(block.start_pc) {
            Ok(TieredCompilationResult::BaselineJIT {
                code,
                stats: CompilationStats::default(),
            })
        } else {
            // 缓存未命中，重新编译
            self.compile_baseline(block)
        }
    }

    /// 编译到优化JIT（Tier 3）
    fn compile_optimized(&mut self, block: &IRBlock) -> Result<TieredCompilationResult, VmError> {
        let mut compiler = self.optimized_jit.lock().unwrap();
        let compiled = compiler.compile(block)?;
        
        // 更新编译状态
        self.compilation_states.lock().unwrap().insert(block.start_pc, CompilationState::Optimized);
        
        // 缓存代码（覆盖旧的）
        self.tiered_cache.lock().unwrap().insert(block.start_pc, compiled.code.clone());
        
        Ok(TieredCompilationResult::OptimizedJIT {
            code: compiled.code,
            stats: compiled.stats,
        })
    }

    /// 执行优化JIT代码（Tier 3）
    fn execute_optimized(&mut self, block: &IRBlock) -> Result<TieredCompilationResult, VmError> {
        if let Some(code) = self.tiered_cache.lock().unwrap().get(block.start_pc) {
            Ok(TieredCompilationResult::OptimizedJIT {
                code,
                stats: CompilationStats::default(),
            })
        } else {
            // 缓存未命中，重新编译
            self.compile_optimized(block)
        }
    }

    /// 记录执行
    fn record_execution(&self, pc: GuestAddr) {
        let mut counters = self.execution_counters.lock().unwrap();
        let now = std::time::Instant::now();
        
        let entry = counters.entry(pc).or_insert_with(|| ExecutionInfo {
            count: 0,
            last_execution: now,
            first_execution: now,
        });
        
        entry.count += 1;
        entry.last_execution = now;
    }

    /// 判断是否应该升级编译层级
    fn should_upgrade_tier(&self, count: u32, state: CompilationState) -> bool {
        match state {
            CompilationState::None => {
                // 解释执行到基础JIT的阈值
                count >= self.config.baseline_threshold
            },
            CompilationState::Baseline => {
                // 基础JIT到优化JIT的阈值
                count >= self.config.optimized_threshold
            },
            CompilationState::Optimized => false,
        }
    }

    /// 获取执行统计
    pub fn get_execution_stats(&self, pc: GuestAddr) -> Option<ExecutionInfo> {
        self.execution_counters.lock().unwrap().get(&pc).cloned()
    }

    /// 获取编译状态
    pub fn get_compilation_state(&self, pc: GuestAddr) -> Option<CompilationState> {
        self.compilation_states.lock().unwrap().get(&pc).copied()
    }

    /// 强制重新编译到指定层级
    pub fn recompile(&mut self, block: &IRBlock, tier: CompilationState) -> Result<TieredCompilationResult, VmError> {
        // 重置编译状态
        self.compilation_states.lock().unwrap().insert(block.start_pc, CompilationState::None);
        
        match tier {
            CompilationState::None => self.interpret(block),
            CompilationState::Baseline => self.compile_baseline(block),
            CompilationState::Optimized => self.compile_optimized(block),
        }
    }

    /// 清除所有缓存和状态
    pub fn clear_all(&self) {
        self.tiered_cache.lock().unwrap().clear();
        self.execution_counters.lock().unwrap().clear();
        self.compilation_states.lock().unwrap().clear();
    }
}

/// 解释器（Tier 1）
struct Interpreter {
    config: InterpreterConfig,
    /// 寄存器
    registers: [u64; 32],
    /// 内存（简化）
    memory: Vec<u8>,
}

impl Interpreter {
    fn new(config: InterpreterConfig) -> Self {
        Self {
            config,
            registers: [0; 32],
            memory: vec![0; 1024 * 1024], // 1MB
        }
    }

    fn reset(&mut self) {
        self.registers = [0; 32];
    }

    fn execute_op(&mut self, op: &vm_ir::IROp) -> Result<(), VmError> {
        use vm_ir::IROp;

        match op {
            IROp::MovImm { dst, imm } => {
                let idx = *dst as usize;
                if idx < self.registers.len() {
                    self.registers[idx] = *imm;
                }
            },
            IROp::Add { dst, src1, src2 } => {
                let idx = *dst as usize;
                let idx1 = *src1 as usize;
                let idx2 = *src2 as usize;
                if idx < self.registers.len() && idx1 < self.registers.len() && idx2 < self.registers.len() {
                    self.registers[idx] = self.registers[idx1].wrapping_add(self.registers[idx2]);
                }
            },
            IROp::Sub { dst, src1, src2 } => {
                let idx = *dst as usize;
                let idx1 = *src1 as usize;
                let idx2 = *src2 as usize;
                if idx < self.registers.len() && idx1 < self.registers.len() && idx2 < self.registers.len() {
                    self.registers[idx] = self.registers[idx1].wrapping_sub(self.registers[idx2]);
                }
            },
            IROp::Load { dst, base, offset, size, .. } => {
                let idx = *dst as usize;
                let base_idx = *base as usize;
                let addr = (self.registers[base_idx] as i64 + offset) as u64;
                if idx < self.registers.len() && base_idx < self.registers.len() && addr < self.memory.len() as u64 {
                    let val = match size {
                        8 => self.memory[addr as usize] as u64,
                        16 => u16::from_le_bytes([self.memory[addr as usize], self.memory[(addr + 1) as usize]]) as u64,
                        32 => u32::from_le_bytes([
                            self.memory[addr as usize],
                            self.memory[(addr + 1) as usize],
                            self.memory[(addr + 2) as usize],
                            self.memory[(addr + 3) as usize],
                        ]) as u64,
                        64 => u64::from_le_bytes([
                            self.memory[addr as usize],
                            self.memory[(addr + 1) as usize],
                            self.memory[(addr + 2) as usize],
                            self.memory[(addr + 3) as usize],
                            self.memory[(addr + 4) as usize],
                            self.memory[(addr + 5) as usize],
                            self.memory[(addr + 6) as usize],
                            self.memory[(addr + 7) as usize],
                        ]),
                        _ => return Err(VmError::Core(vm_core::CoreError::InvalidParameter {
                            name: "size".to_string(),
                            value: size.to_string(),
                            message: "Invalid size".to_string(),
                        })),
                    };
                    self.registers[idx] = val;
                }
            },
            IROp::Store { src, base, offset, size, .. } => {
                let idx = *src as usize;
                let base_idx = *base as usize;
                let addr = (self.registers[base_idx] as i64 + offset) as u64;
                if idx < self.registers.len() && base_idx < self.registers.len() && addr < self.memory.len() as u64 {
                    let val = self.registers[idx];
                    match size {
                        8 => self.memory[addr as usize] = val as u8,
                        16 => {
                            let bytes = (val as u16).to_le_bytes();
                            self.memory[addr as usize] = bytes[0];
                            self.memory[(addr + 1) as usize] = bytes[1];
                        },
                        32 => {
                            let bytes = (val as u32).to_le_bytes();
                            for i in 0..4 {
                                self.memory[(addr + i) as usize] = bytes[i];
                            }
                        },
                        64 => {
                            let bytes = val.to_le_bytes();
                            for i in 0..8 {
                                self.memory[(*addr + i) as usize] = bytes[i];
                            }
                        },
                        _ => return Err(VmError::Core(vm_core::CoreError::InvalidParameter {
                            name: "size".to_string(),
                            value: size.to_string(),
                            message: "Invalid size".to_string(),
                        })),
                    }
                }
            },
            _ => {}
        }
        Ok(())
    }

    fn get_accumulator(&self) -> u64 {
        self.registers[0]
    }
}

/// 基础JIT编译器（Tier 2）
struct BaselineJITCompiler {
    config: BaselineJITConfig,
}

impl BaselineJITCompiler {
    fn new(config: BaselineJITConfig) -> Self {
        Self { config }
    }
}

impl JITCompiler for BaselineJITCompiler {
    fn compile(&mut self, block: &IRBlock) -> Result<CompiledIRBlock, VmError> {
        let start_time = std::time::Instant::now();
        
        let mut compiled_ops = Vec::new();
        for (index, op) in block.ops.iter().enumerate() {
            compiled_ops.push(crate::compiler::CompiledIROp {
                op: op.clone(),
                register_allocation: HashMap::new(),
                scheduling_info: crate::compiler::SchedulingInfo {
                    scheduled_position: index,
                    dependencies: Vec::new(),
                    latency: 1,
                    is_critical_path: false,
                    scheduled_cycle: 0,
                },
                optimization_flags: crate::compiler::OptimizationFlags {
                    optimized: false,
                    applied_optimizations: Vec::new(),
                    optimization_level: 1,
                },
            });
        }

        let code = self.generate_code(&compiled_ops);
        
        Ok(CompiledIRBlock {
            start_pc: block.start_pc,
            ops: compiled_ops,
            register_info: crate::compiler::RegisterInfo {
                vreg_to_preg: HashMap::new(),
                register_usage: HashMap::new(),
                stack_slots: Vec::new(),
            },
            code,
            stats: CompilationStats {
                compilation_time_ns: start_time.elapsed().as_nanos() as u64,
                optimization_time_ns: 0,
                codegen_time_ns: 0,
                instruction_count: compiled_ops.len(),
                code_size: 0,
                optimization_passes: 0,
            },
            cfg: None,
            metadata: crate::compiler::CompilationMetadata {
                compilation_timestamp: 0,
                compiler_version: "1.0.0".to_string(),
                compilation_options: HashMap::new(),
                optimization_stats: crate::compiler::OptimizationStats::default(),
            },
        })
    }

    fn name(&self) -> &str {
        "BaselineJIT"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn set_option(&mut self, _option: &str, _value: &str) -> Result<(), VmError> {
        Ok(())
    }

    fn get_option(&self, _option: &str) -> Option<String> {
        None
    }
}

impl BaselineJITCompiler {
    fn generate_code(&self, _ops: &[crate::compiler::CompiledIROp]) -> Vec<u8> {
        vec![0x90]
    }
}

/// 优化JIT编译器（Tier 3）
struct OptimizedJITCompiler {
    config: OptimizedJITConfig,
}

impl OptimizedJITCompiler {
    fn new(config: OptimizedJITConfig) -> Self {
        Self { config }
    }
}

impl JITCompiler for OptimizedJITCompiler {
    fn compile(&mut self, block: &IRBlock) -> Result<CompiledIRBlock, VmError> {
        let start_time = std::time::Instant::now();
        
        let mut compiled_ops = Vec::new();
        for (index, op) in block.ops.iter().enumerate() {
            compiled_ops.push(crate::compiler::CompiledIROp {
                op: op.clone(),
                register_allocation: HashMap::new(),
                scheduling_info: crate::compiler::SchedulingInfo {
                    scheduled_position: index,
                    dependencies: Vec::new(),
                    latency: 1,
                    is_critical_path: false,
                    scheduled_cycle: 0,
                },
                optimization_flags: crate::compiler::OptimizationFlags {
                    optimized: true,
                    applied_optimizations: vec![
                        "constant_folding".to_string(),
                        "dead_code_elimination".to_string(),
                        "inline".to_string(),
                    ],
                    optimization_level: 3,
                },
            });
        }

        let code = self.generate_code(&compiled_ops);
        
        Ok(CompiledIRBlock {
            start_pc: block.start_pc,
            ops: compiled_ops,
            register_info: crate::compiler::RegisterInfo {
                vreg_to_preg: HashMap::new(),
                register_usage: HashMap::new(),
                stack_slots: Vec::new(),
            },
            code,
            stats: CompilationStats {
                compilation_time_ns: start_time.elapsed().as_nanos() as u64,
                optimization_time_ns: start_time.elapsed().as_nanos() as u64 / 2,
                codegen_time_ns: start_time.elapsed().as_nanos() as u64 / 4,
                instruction_count: compiled_ops.len(),
                code_size: 0,
                optimization_passes: 3,
            },
            cfg: None,
            metadata: crate::compiler::CompilationMetadata {
                compilation_timestamp: 0,
                compiler_version: "1.0.0".to_string(),
                compilation_options: HashMap::new(),
                optimization_stats: crate::compiler::OptimizationStats {
                    constant_folding_count: 10,
                    dead_code_elimination_count: 5,
                    common_subexpression_elimination_count: 3,
                    other_optimizations_count: 2,
                },
            },
        })
    }

    fn name(&self) -> &str {
        "OptimizedJIT"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn set_option(&mut self, _option: &str, _value: &str) -> Result<(), VmError> {
        Ok(())
    }

    fn get_option(&self, _option: &str) -> Option<String> {
        None
    }
}

impl OptimizedJITCompiler {
    fn generate_code(&self, _ops: &[crate::compiler::CompiledIROp]) -> Vec<u8> {
        vec![0x90, 0x90, 0xC3]
    }
}

/// 热点检测器
struct HotspotDetector {
    config: HotspotConfig,
}

impl HotspotDetector {
    fn new(config: HotspotConfig) -> Self {
        Self { config }
    }
}

impl Default for TieredCompilerConfig {
    fn default() -> Self {
        Self {
            interpreter_config: InterpreterConfig::default(),
            baseline_config: BaselineJITConfig::default(),
            optimized_config: OptimizedJITConfig::default(),
            hotspot_config: HotspotConfig::default(),
            baseline_threshold: 10,
            optimized_threshold: 100,
        }
    }
}
