//! AOT构建器配置和统计

use vm_ir_lift::ISA;

/// 编译选项配置
#[derive(Clone, Debug)]
pub struct CompilationOptions {
    /// 优化级别 (0, 1, 2)
    pub optimization_level: u32,
    /// 目标 ISA
    pub target_isa: ISA,
    /// 是否启用应用性检测
    pub enable_applicability_check: bool,
    /// 代码生成模式
    pub codegen_mode: CodegenMode,
    /// 是否启用并行编译
    pub enable_parallel_compilation: bool,
    /// 并行编译线程数（0表示自动检测）
    pub parallel_threads: usize,
    /// 是否考虑依赖关系进行编译调度
    pub respect_dependencies: bool,
}

#[derive(Clone, Debug)]
pub enum CodegenMode {
    /// 直接生成机器码
    Direct,
    /// 通过 LLVM 生成
    LLVM,
    /// 通过 Cranelift 生成
    Cranelift,
}

impl Default for CompilationOptions {
    fn default() -> Self {
        Self {
            optimization_level: 2,
            target_isa: ISA::X86_64,
            enable_applicability_check: true,
            codegen_mode: CodegenMode::Cranelift,
            enable_parallel_compilation: true,
            parallel_threads: 0, // 自动检测
            respect_dependencies: true,
        }
    }
}

/// 编译管道统计
#[derive(Clone, Debug, Default)]
pub struct CompilationStats {
    /// 输入指令数
    pub input_instructions: u64,
    /// 解码后的指令数
    pub decoded_instructions: u64,
    /// 生成的 IR 指令数
    pub generated_ir_instructions: u64,
    /// 优化后的 IR 指令数
    pub optimized_ir_instructions: u64,
    /// 生成的机器码字节数
    pub output_code_size: u64,
    /// 处理耗时（毫秒）
    pub compilation_time_ms: u64,
    /// 去重的代码块数量
    pub deduplicated_blocks: u64,
    /// 去重节省的字节数
    pub deduplicated_size: u64,
}

impl CompilationStats {
    /// 计算代码膨胀比
    pub fn code_expansion_ratio(&self) -> f64 {
        if self.input_instructions == 0 {
            1.0
        } else {
            self.output_code_size as f64 / self.input_instructions as f64
        }
    }

    /// 计算优化效果
    pub fn optimization_reduction(&self) -> f64 {
        if self.generated_ir_instructions == 0 {
            0.0
        } else {
            let reduced =
                self.generated_ir_instructions as f64 - self.optimized_ir_instructions as f64;
            reduced / self.generated_ir_instructions as f64
        }
    }
}

