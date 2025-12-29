use std::fmt;

/// 编译层级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CompilationTier {
    /// Tier 1: 解释执行
    #[default]
    Interpreter,
    /// Tier 2: 基础JIT编译
    Baseline,
    /// Tier 3: 优化JIT编译
    Optimized,
}

impl CompilationTier {
    /// 获取层级编号
    pub fn level(&self) -> u8 {
        match self {
            CompilationTier::Interpreter => 1,
            CompilationTier::Baseline => 2,
            CompilationTier::Optimized => 3,
        }
    }

    /// 获取下一层级
    pub fn next(&self) -> Option<CompilationTier> {
        match self {
            CompilationTier::Interpreter => Some(CompilationTier::Baseline),
            CompilationTier::Baseline => Some(CompilationTier::Optimized),
            CompilationTier::Optimized => None,
        }
    }

    /// 是否可以升级到目标层级
    pub fn can_upgrade_to(&self, target: CompilationTier) -> bool {
        self.level() < target.level()
    }
}

impl fmt::Display for CompilationTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompilationTier::Interpreter => write!(f, "Interpreter"),
            CompilationTier::Baseline => write!(f, "Baseline JIT"),
            CompilationTier::Optimized => write!(f, "Optimized JIT"),
        }
    }
}

/// 编译决策
#[derive(Debug, Clone)]
pub struct CompilationDecision {
    /// 当前层级
    pub current_tier: CompilationTier,
    /// 目标层级
    pub target_tier: CompilationTier,
    /// 决策原因
    pub reason: DecisionReason,
    /// 执行计数
    pub execution_count: u32,
}

/// 决策原因
#[derive(Debug, Clone)]
pub enum DecisionReason {
    /// 首次执行
    FirstExecution,
    /// 达到阈值
    ThresholdReached,
    /// 热点检测
    HotspotDetected,
    /// 手动触发
    ManualTrigger,
    /// 性能回退
    PerformanceFallback,
    /// 其他原因
    Other(String),
}

impl CompilationDecision {
    /// 创建新的编译决策
    pub fn new(
        current_tier: CompilationTier,
        target_tier: CompilationTier,
        reason: DecisionReason,
        execution_count: u32,
    ) -> Self {
        Self {
            current_tier,
            target_tier,
            reason,
            execution_count,
        }
    }

    /// 是否需要重新编译
    pub fn needs_recompilation(&self) -> bool {
        self.current_tier != self.target_tier
    }

    /// 是否是升级
    pub fn is_upgrade(&self) -> bool {
        self.current_tier.can_upgrade_to(self.target_tier)
    }

    /// 是否是降级
    pub fn is_downgrade(&self) -> bool {
        self.target_tier.can_upgrade_to(self.current_tier)
    }
}

/// 层级性能统计
#[derive(Debug, Clone, Default)]
pub struct TierPerformanceStats {
    /// 解释执行统计
    pub interpreter: InterpreterStats,
    /// 基础JIT统计
    pub baseline: JITStats,
    /// 优化JIT统计
    pub optimized: JITStats,
}

/// 解释器统计
#[derive(Debug, Clone, Default)]
pub struct InterpreterStats {
    /// 执行指令数
    pub executed_instructions: u64,
    /// 执行周期数
    pub executed_cycles: u64,
    /// 平均每指令周期数
    pub avg_cycles_per_instruction: f64,
}

/// JIT统计
#[derive(Debug, Clone, Default)]
pub struct JITStats {
    /// 编译次数
    pub compilation_count: u64,
    /// 执行次数
    pub execution_count: u64,
    /// 总编译时间（纳秒）
    pub total_compilation_time_ns: u64,
    /// 平均编译时间（纳秒）
    pub avg_compilation_time_ns: u64,
    /// 总执行时间（纳秒）
    pub total_execution_time_ns: u64,
    /// 平均执行时间（纳秒）
    pub avg_execution_time_ns: u64,
}

impl TierPerformanceStats {
    /// 更新解释执行统计
    pub fn update_interpreter(&mut self, instructions: u64, cycles: u64) {
        self.interpreter.executed_instructions += instructions;
        self.interpreter.executed_cycles += cycles;
        self.interpreter.avg_cycles_per_instruction =
            self.interpreter.executed_cycles as f64 / self.interpreter.executed_instructions as f64;
    }

    /// 更新JIT编译统计
    pub fn update_jit_compilation(&mut self, tier: CompilationTier, time_ns: u64) {
        match tier {
            CompilationTier::Baseline => {
                self.baseline.compilation_count += 1;
                self.baseline.total_compilation_time_ns += time_ns;
                self.baseline.avg_compilation_time_ns =
                    self.baseline.total_compilation_time_ns / self.baseline.compilation_count;
            }
            CompilationTier::Optimized => {
                self.optimized.compilation_count += 1;
                self.optimized.total_compilation_time_ns += time_ns;
                self.optimized.avg_compilation_time_ns =
                    self.optimized.total_compilation_time_ns / self.optimized.compilation_count;
            }
            CompilationTier::Interpreter => {}
        }
    }

    /// 更新JIT执行统计
    pub fn update_jit_execution(&mut self, tier: CompilationTier, time_ns: u64) {
        match tier {
            CompilationTier::Baseline => {
                self.baseline.execution_count += 1;
                self.baseline.total_execution_time_ns += time_ns;
                self.baseline.avg_execution_time_ns =
                    self.baseline.total_execution_time_ns / self.baseline.execution_count;
            }
            CompilationTier::Optimized => {
                self.optimized.execution_count += 1;
                self.optimized.total_execution_time_ns += time_ns;
                self.optimized.avg_execution_time_ns =
                    self.optimized.total_execution_time_ns / self.optimized.execution_count;
            }
            CompilationTier::Interpreter => {}
        }
    }

    /// 获取编译总时间
    pub fn total_compilation_time(&self) -> u64 {
        self.baseline.total_compilation_time_ns + self.optimized.total_compilation_time_ns
    }

    /// 获取执行总时间
    pub fn total_execution_time(&self) -> u64 {
        self.baseline.total_execution_time_ns + self.optimized.total_execution_time_ns
    }
}
