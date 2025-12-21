//! 自动跨架构执行器
//!
//! 提供自动检测host/guest架构并选择合适的解码器和执行引擎的功能

use super::{CrossArchConfig, CrossArchStrategy};
use std::fmt;
use tracing::{info, warn};
use vm_core::{ExecMode, ExecutionEngine, GuestAddr, GuestArch, MMU, VmError};
use vm_engine_interpreter::Interpreter;
use vm_engine_jit::Jit;
use vm_ir::IRBlock;

/// 统一解码器trait（统一不同架构的解码器接口）
pub trait UnifiedDecoder: Send + Sync {
    /// 解码指令为IR块
    fn decode(&mut self, mmu: &mut dyn MMU, pc: GuestAddr) -> Result<IRBlock, VmError>;

    /// 获取支持的guest架构
    fn guest_arch(&self) -> GuestArch;
}

/// 自动执行器
///
/// 自动检测host和guest架构，选择合适的解码器和执行引擎
pub struct AutoExecutor {
    /// 跨架构配置
    config: CrossArchConfig,
    /// 解码器（根据guest架构选择）
    decoder: Box<dyn UnifiedDecoder>,
    /// 执行引擎（根据策略选择）
    engine: Box<dyn ExecutionEngine<IRBlock>>,
}

impl AutoExecutor {
    /// 自动创建执行器
    ///
    /// 根据guest架构自动检测host架构并配置
    pub fn auto_create(
        guest_arch: GuestArch,
        exec_mode: Option<ExecMode>,
    ) -> Result<Self, VmError> {
        // 1. 自动检测并创建跨架构配置
        let config = CrossArchConfig::auto_detect(guest_arch)?;

        info!("🔍 架构检测结果: {}, 策略: {:?}", config, config.strategy);

        // 2. 根据guest架构创建解码器
        let decoder: Box<dyn UnifiedDecoder> = match guest_arch {
            GuestArch::X86_64 => Box::new(X86_64DecoderWrapper::new()),
            GuestArch::Arm64 => Box::new(ARM64DecoderWrapper::new()),
            GuestArch::Riscv64 => Box::new(Riscv64DecoderWrapper::new()),
        };

        // 3. 根据策略和执行模式创建执行引擎
        let exec_mode = exec_mode.unwrap_or_else(|| config.recommended_exec_mode());
        let engine: Box<dyn ExecutionEngine<IRBlock>> = match exec_mode {
            ExecMode::Interpreter => {
                info!("Using interpreter execution engine");
                Box::new(Interpreter::new())
            }
            ExecMode::JIT => {
                // vm-engine-jit 是强依赖，可以直接使用
                info!("Using JIT execution engine");
                // 如果 JIT 初始化失败，会回退到解释器
                // 这里我们直接创建，让 Jit::new() 处理初始化
                Box::new(Jit::new())
            }
            ExecMode::HardwareAssisted => {
                if config.strategy == CrossArchStrategy::Native {
                    // 同架构可以使用硬件加速
                    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
                    {
                        use vm_accel::{select, AccelKind};
                        let (kind, _accel) = select();
                        match kind {
                            AccelKind::Kvm | AccelKind::Hvf | AccelKind::Whpx => {
                                info!("Hardware acceleration available: {:?}", kind);
                                // 尝试创建硬件加速执行引擎
                                match super::hardware_accel_engine::HardwareAccelEngine::new() {
                                    Ok(engine) => {
                                        info!("Hardware acceleration engine created successfully");
                                        Box::new(engine)
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to create hardware acceleration engine: {:?}, falling back to interpreter",
                                            e
                                        );
                                        Box::new(Interpreter::new())
                                    }
                                }
                            }
                            AccelKind::None => {
                                warn!("Hardware acceleration not available, falling back to interpreter");
                                Box::new(Interpreter::new())
                            }
                        }
                    }
                    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
                    {
                        warn!("HardwareAssisted mode not supported on this platform, falling back to interpreter");
                        Box::new(Interpreter::new())
                    }
                } else {
                    warn!("HardwareAssisted mode not supported for cross-architecture execution, falling back to interpreter");
                    Box::new(Interpreter::new())
                }
            }
        };

        Ok(Self {
            config,
            decoder,
            engine,
        })
    }

    /// 执行单个基本块
    pub fn execute_block(
        &mut self,
        mmu: &mut dyn MMU,
        pc: GuestAddr,
    ) -> Result<vm_core::ExecResult, VmError> {
        // 1. 解码指令
        let ir_block = self.decoder.decode(mmu, pc)?;

        // 2. 执行IR
        Ok(self.engine.run(mmu, &ir_block))
    }

    /// 获取配置信息
    pub fn config(&self) -> &CrossArchConfig {
        &self.config
    }

    /// 获取执行引擎
    pub fn engine_mut(&mut self) -> &mut dyn ExecutionEngine<IRBlock> {
        self.engine.as_mut()
    }

    /// 解码指令为IR块（不执行）
    ///
    /// 这个方法允许在不执行代码的情况下获取IR块，
    /// 用于JIT编译、AOT编译等场景
    pub fn decode_block(&mut self, mmu: &mut dyn MMU, pc: GuestAddr) -> Result<IRBlock, VmError> {
        // 使用解码器解码指令为IR块
        self.decoder.decode(mmu, pc)
    }
}

impl fmt::Display for AutoExecutor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AutoExecutor({})", self.config)
    }
}

// ============================================================================
// 解码器包装器（统一不同架构的解码器接口）
// ============================================================================

/// x86-64解码器包装器
struct X86_64DecoderWrapper {
    decoder: vm_frontend_x86_64::X86Decoder,
}

impl X86_64DecoderWrapper {
    fn new() -> Self {
        Self {
            decoder: vm_frontend_x86_64::X86Decoder::new(),
        }
    }
}

impl UnifiedDecoder for X86_64DecoderWrapper {
    fn decode(&mut self, mmu: &mut dyn MMU, pc: GuestAddr) -> Result<IRBlock, VmError> {
        // X86Decoder实现了vm_core::Decoder trait，调用decode方法解码基本块
        vm_core::Decoder::decode(&mut self.decoder, mmu, pc).map_err(|e| {
            VmError::Core(vm_core::CoreError::DecodeError {
                message: format!("x86-64 decode error: {:?}", e),
                position: Some(pc),
                module: "X86_64Decoder".to_string(),
            })
        })
    }

    fn guest_arch(&self) -> GuestArch {
        GuestArch::X86_64
    }
}

/// ARM64解码器包装器
struct ARM64DecoderWrapper {
    decoder: vm_frontend_arm64::Arm64Decoder,
}

impl ARM64DecoderWrapper {
    fn new() -> Self {
        Self {
            decoder: vm_frontend_arm64::Arm64Decoder::new(),
        }
    }
}

impl UnifiedDecoder for ARM64DecoderWrapper {
    fn decode(&mut self, mmu: &mut dyn MMU, pc: GuestAddr) -> Result<IRBlock, VmError> {
        vm_core::Decoder::decode(&mut self.decoder, mmu, pc).map_err(|e| {
            VmError::Core(vm_core::CoreError::DecodeError {
                message: format!("ARM64 decode error: {:?}", e),
                position: Some(pc),
                module: "ARM64Decoder".to_string(),
            })
        })
    }

    fn guest_arch(&self) -> GuestArch {
        GuestArch::Arm64
    }
}

/// RISC-V64解码器包装器
struct Riscv64DecoderWrapper {
    decoder: vm_frontend_riscv64::RiscvDecoder,
}

impl Riscv64DecoderWrapper {
    fn new() -> Self {
        Self {
            decoder: vm_frontend_riscv64::RiscvDecoder,
        }
    }
}

impl UnifiedDecoder for Riscv64DecoderWrapper {
    fn decode(&mut self, mmu: &mut dyn MMU, pc: GuestAddr) -> Result<IRBlock, VmError> {
        vm_core::Decoder::decode(&mut self.decoder, mmu, pc).map_err(|e| {
            VmError::Core(vm_core::CoreError::DecodeError {
                message: format!("RISC-V64 decode error: {:?}", e),
                position: Some(pc),
                module: "Riscv64Decoder".to_string(),
            })
        })
    }

    fn guest_arch(&self) -> GuestArch {
        GuestArch::Riscv64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_executor_creation() {
        // 测试自动创建执行器
        let executor = AutoExecutor::auto_create(GuestArch::X86_64, None);
        assert!(executor.is_ok());

        let executor = executor.unwrap();
        info!("Created executor: {}", executor);
        assert!(executor.config().is_supported());
    }
}
