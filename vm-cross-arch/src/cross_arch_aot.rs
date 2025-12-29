//! 跨架构AOT编译器
//!
//! 支持三种架构（AMD64、ARM64、RISC-V64）两两之间的AOT编译

#[cfg(feature = "jit")]
use super::{Architecture, PerformanceConfig, PerformanceOptimizer};
use std::collections::HashMap;
use std::path::Path;
use vm_core::{GuestAddr, VmError};

#[cfg(feature = "jit")]
use vm_engine::jit::aot::{AotBuilder, CodegenMode, CompilationOptions};

use vm_ir::IRBlock;
use vm_ir::lift::ISA;

/// 跨架构AOT编译配置
#[derive(Debug, Clone)]
pub struct CrossArchAotConfig {
    /// 源架构
    pub source_arch: Architecture,
    /// 目标架构
    pub target_arch: Architecture,
    /// 优化级别 (0-3)
    pub optimization_level: u32,
    /// 是否启用跨架构优化
    pub enable_cross_arch_optimization: bool,
    /// 代码生成模式
    pub codegen_mode: CodegenMode,
}

impl Default for CrossArchAotConfig {
    fn default() -> Self {
        Self {
            source_arch: Architecture::X86_64,
            target_arch: Architecture::ARM64,
            optimization_level: 2,
            enable_cross_arch_optimization: true,
            codegen_mode: CodegenMode::LLVM,
        }
    }
}

/// 跨架构AOT编译器
pub struct CrossArchAotCompiler {
    /// AOT构建器
    builder: AotBuilder,
    /// 编译配置
    config: CrossArchAotConfig,
    /// 源架构到IR的映射
    source_to_ir: HashMap<GuestAddr, IRBlock>,
    /// 编译统计
    stats: CrossArchAotStats,
}

/// 跨架构AOT编译统计
#[derive(Debug, Clone, Default)]
pub struct CrossArchAotStats {
    /// 编译的块数
    pub blocks_compiled: usize,
    /// 跨架构转换次数
    pub cross_arch_translations: usize,
    /// 总编译时间（毫秒）
    pub total_compilation_time_ms: u64,
    /// 生成的代码大小（字节）
    pub generated_code_size: usize,
}

impl CrossArchAotCompiler {
    /// 创建新的跨架构AOT编译器
    pub fn new(config: CrossArchAotConfig) -> Result<Self, VmError> {
        // 将Architecture转换为ISA
        let target_isa = match config.target_arch {
            Architecture::X86_64 => ISA::X86_64,
            Architecture::ARM64 => ISA::ARM64,
            Architecture::RISCV64 => ISA::RISCV64,
        };

        let compilation_options = CompilationOptions {
            optimization_level: config.optimization_level,
            target_isa,
            enable_applicability_check: true,
            codegen_mode: config.codegen_mode,
            enable_parallel_compilation: false,
            parallel_threads: 1,
            respect_dependencies: true,
        };

        let builder = AotBuilder::with_options(compilation_options);

        Ok(Self {
            builder,
            config,
            source_to_ir: HashMap::new(),
            stats: CrossArchAotStats::default(),
        })
    }

    /// 从源架构代码编译到目标架构
    ///
    /// 流程：源架构代码 → IR → 目标架构代码
    pub fn compile_from_source(
        &mut self,
        pc: GuestAddr,
        source_code: &[u8],
        source_arch: Architecture,
    ) -> Result<(), VmError> {
        let start_time = std::time::Instant::now();

        // 1. 解码源架构代码为IR（使用缓存避免重复解码）
        let ir_block = if let Some(cached_ir) = self.source_to_ir.get(&pc) {
            cached_ir.clone()
        } else {
            let new_ir = self.decode_source_to_ir(source_code, source_arch, pc)?;
            self.source_to_ir.insert(pc, new_ir.clone());
            new_ir
        };

        // 2. 跨架构优化（如果需要）
        let optimized_ir = if self.config.enable_cross_arch_optimization {
            self.optimize_for_target_arch(&ir_block)?
        } else {
            ir_block.clone()
        };

        // 3. 编译IR到目标架构代码
        let target_code = self.compile_ir_to_target(&optimized_ir)?;
        let code_size = target_code.len();
        self.stats.generated_code_size += code_size;

        // 4. 添加到AOT镜像
        self.builder
            .add_compiled_block(pc, target_code, 0)
            .map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to add compiled block: {}", e),
                    module: "CrossArchAotCompiler".to_string(),
                })
            })?;

        // 更新统计
        let elapsed = start_time.elapsed();
        self.stats.blocks_compiled += 1;
        self.stats.cross_arch_translations += 1;
        self.stats.total_compilation_time_ms += elapsed.as_millis() as u64;

        Ok(())
    }

    /// 从IR块编译到目标架构
    pub fn compile_from_ir(&mut self, pc: GuestAddr, ir_block: &IRBlock) -> Result<(), VmError> {
        let start_time = std::time::Instant::now();

        // 跨架构优化
        let optimized_ir = if self.config.enable_cross_arch_optimization {
            self.optimize_for_target_arch(ir_block)?
        } else {
            ir_block.clone()
        };

        // 编译到目标架构
        let target_code = self.compile_ir_to_target(&optimized_ir)?;

        // 添加到AOT镜像
        let code_len = target_code.len();
        self.builder
            .add_compiled_block(pc, target_code, 0)
            .map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to add compiled block: {}", e),
                    module: "CrossArchAotCompiler".to_string(),
                })
            })?;

        // 更新统计
        let elapsed = start_time.elapsed();
        self.stats.blocks_compiled += 1;
        self.stats.total_compilation_time_ms += elapsed.as_millis() as u64;
        self.stats.generated_code_size += code_len;

        Ok(())
    }

    /// 解码源架构代码为IR
    fn decode_source_to_ir(
        &mut self,
        source_code: &[u8],
        source_arch: Architecture,
        pc: GuestAddr,
    ) -> Result<IRBlock, VmError> {
        // 使用vm-ir-lift解码
        use vm_ir::lift::create_decoder;

        let source_isa = match source_arch {
            Architecture::X86_64 => ISA::X86_64,
            Architecture::ARM64 => ISA::ARM64,
            Architecture::RISCV64 => ISA::RISCV64,
        };

        let decoder = create_decoder(source_isa);
        let _instructions = decoder
            .decode_sequence(source_code, source_code.len())
            .map_err(|e| {
                VmError::Core(vm_core::CoreError::DecodeError {
                    message: format!("Failed to decode source code: {}", e),
                    position: Some(pc),
                    module: "CrossArchAotCompiler".to_string(),
                })
            })?;

        // 将源指令提升为 IR 的逻辑暂未接入，先构建空块
        let mut ir_ops: Vec<vm_ir::IROp> = Vec::new();

        // 构建IRBlock
        use vm_ir::{IRBuilder, Terminator};
        let mut builder = IRBuilder::new(pc);
        for op in ir_ops.drain(..) {
            builder.push(op);
        }
        builder.set_term(Terminator::Ret);

        Ok(builder.build())
    }

    /// 为目标架构优化IR
    fn optimize_for_target_arch(&self, ir_block: &IRBlock) -> Result<IRBlock, VmError> {
        // 使用性能优化器进行优化
        let perf_config = PerformanceConfig::default();
        let mut optimizer = PerformanceOptimizer::new(perf_config);

        optimizer
            .optimize_ir_block(ir_block, self.config.target_arch)
            .map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to optimize IR: {}", e),
                    module: "CrossArchAotCompiler".to_string(),
                })
            })
    }

    /// 编译IR到目标架构代码
    fn compile_ir_to_target(&self, ir_block: &IRBlock) -> Result<Vec<u8>, VmError> {
        // 使用vm-cross-arch的编码器将IR编码为目标架构
        use super::encoder::{ArchEncoder, Arm64Encoder, Riscv64Encoder, X86_64Encoder};

        let mut target_code = Vec::new();

        let pc = ir_block.start_pc;
        match self.config.target_arch {
            Architecture::X86_64 => {
                let encoder = X86_64Encoder;
                for op in &ir_block.ops {
                    let instructions = encoder.encode_op(op, pc).map_err(|e| {
                        VmError::Core(vm_core::CoreError::Internal {
                            message: format!("Failed to encode to x86-64: {:?}", e),
                            module: "CrossArchAotCompiler".to_string(),
                        })
                    })?;
                    for inst in instructions {
                        target_code.extend_from_slice(&inst.bytes);
                    }
                }
            }
            Architecture::ARM64 => {
                let encoder = Arm64Encoder;
                for op in &ir_block.ops {
                    let instructions = encoder.encode_op(op, pc).map_err(|e| {
                        VmError::Core(vm_core::CoreError::Internal {
                            message: format!("Failed to encode to ARM64: {:?}", e),
                            module: "CrossArchAotCompiler".to_string(),
                        })
                    })?;
                    for inst in instructions {
                        target_code.extend_from_slice(&inst.bytes);
                    }
                }
            }
            Architecture::RISCV64 => {
                let encoder = Riscv64Encoder;
                for op in &ir_block.ops {
                    let instructions = encoder.encode_op(op, pc).map_err(|e| {
                        VmError::Core(vm_core::CoreError::Internal {
                            message: format!("Failed to encode to RISC-V64: {:?}", e),
                            module: "CrossArchAotCompiler".to_string(),
                        })
                    })?;
                    for inst in instructions {
                        target_code.extend_from_slice(&inst.bytes);
                    }
                }
            }
        }

        Ok(target_code)
    }

    /// 构建AOT镜像
    /// 构建并丢弃 AOT 镜像（该镜像类型在 aot-builder 内部，无法在此公开返回）
    pub fn build(self) {
        let _ = self.builder.build();
    }

    /// 保存AOT镜像到文件
    pub fn save_to_file<P: AsRef<Path>>(self, path: P) -> Result<(), VmError> {
        let image = self.builder.build();
        let mut file = std::fs::File::create(path).map_err(|e| VmError::Io(e.to_string()))?;
        image?
            .serialize(&mut file)
            .map_err(|e| VmError::Io(e.to_string()))?;
        Ok(())
    }

    /// 获取编译统计
    pub fn stats(&self) -> &CrossArchAotStats {
        &self.stats
    }

    /// 获取配置
    pub fn config(&self) -> &CrossArchAotConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_arch_aot_compiler() {
        let config = CrossArchAotConfig {
            source_arch: Architecture::X86_64,
            target_arch: Architecture::ARM64,
            optimization_level: 2,
            enable_cross_arch_optimization: true,
            codegen_mode: CodegenMode::LLVM,
        };

        let compiler = CrossArchAotCompiler::new(config);
        assert!(compiler.is_ok());
    }
}
