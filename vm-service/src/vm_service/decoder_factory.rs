use vm_core::{Decoder, GuestAddr, GuestArch, MMU, VmError};
use vm_ir::IRBlock;

/// Decoder factory module with performance features enabled
#[cfg(feature = "performance")]
pub mod decoder_factory {
    use super::*;
    use vm_frontend::arm64::{Arm64Decoder, Arm64Instruction};
    use vm_frontend::riscv64::{RiscvDecoder, RiscvInstruction};
    use vm_frontend::x86_64::{X86Decoder, X86Instruction};

    /// 统一的服务解码器枚举
    pub enum ServiceDecoder {
        Riscv64(RiscvDecoder),
        Arm64(Arm64Decoder),
        X86_64(X86Decoder),
        /// PowerPC64 decoder is not yet implemented in frontend
        PowerPC64Unsupported,
    }

    /// 服务指令包装器，适配前端解码器的指令类型
    pub struct ServiceInstruction {
        /// 内部指令指针，使用动态类型存储不同架构的指令
        inner: Box<dyn InstructionTrait>,
    }

    /// 指令trait，定义前端解码器指令需要实现的方法
    pub trait InstructionTrait {
        fn next_pc(&self) -> GuestAddr;
        fn size(&self) -> u8;
        fn operand_count(&self) -> usize;
        fn mnemonic(&self) -> &str;
        fn is_control_flow(&self) -> bool;
        fn is_memory_access(&self) -> bool;
    }

    impl InstructionTrait for Arm64Instruction {
        fn next_pc(&self) -> GuestAddr {
            self.next_pc()
        }
        fn size(&self) -> u8 {
            self.size()
        }
        fn operand_count(&self) -> usize {
            self.operand_count()
        }
        fn mnemonic(&self) -> &str {
            self.mnemonic()
        }
        fn is_control_flow(&self) -> bool {
            self.is_branch
        }
        fn is_memory_access(&self) -> bool {
            self.has_memory_op
        }
    }

    impl InstructionTrait for RiscvInstruction {
        fn next_pc(&self) -> GuestAddr {
            self.next_pc()
        }
        fn size(&self) -> u8 {
            self.size()
        }
        fn operand_count(&self) -> usize {
            self.operand_count()
        }
        fn mnemonic(&self) -> &str {
            self.mnemonic()
        }
        fn is_control_flow(&self) -> bool {
            self.is_branch
        }
        fn is_memory_access(&self) -> bool {
            self.has_memory_op
        }
    }

    impl InstructionTrait for X86Instruction {
        fn next_pc(&self) -> GuestAddr {
            self.next_pc()
        }
        fn size(&self) -> u8 {
            self.size()
        }
        fn operand_count(&self) -> usize {
            self.operand_count()
        }
        fn mnemonic(&self) -> &str {
            self.mnemonic()
        }
        fn is_control_flow(&self) -> bool {
            self.is_control_flow()
        }
        fn is_memory_access(&self) -> bool {
            self.is_memory_access()
        }
    }

    impl ServiceInstruction {
        /// 创建新的服务指令
        pub fn new<I: InstructionTrait + 'static>(instruction: I) -> Self {
            ServiceInstruction {
                inner: Box::new(instruction),
            }
        }

        /// 获取下一条指令的地址
        pub fn next_pc(&self) -> GuestAddr {
            self.inner.next_pc()
        }

        /// 获取指令大小
        pub fn size(&self) -> u8 {
            self.inner.size()
        }

        /// 获取操作数数量
        pub fn operand_count(&self) -> usize {
            self.inner.operand_count()
        }

        /// 获取指令助记符
        pub fn mnemonic(&self) -> &str {
            self.inner.mnemonic()
        }

        /// 检查是否是控制流指令
        pub fn is_control_flow(&self) -> bool {
            self.inner.is_control_flow()
        }

        /// 检查是否是内存访问指令
        pub fn is_memory_access(&self) -> bool {
            self.inner.is_memory_access()
        }
    }

    impl Decoder for ServiceDecoder {
        type Instruction = ServiceInstruction;
        type Block = IRBlock;

        fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Instruction, VmError> {
            match self {
                ServiceDecoder::Riscv64(d) => {
                    let insn = d.decode_insn(mmu, pc)?;
                    Ok(ServiceInstruction::new(insn))
                }
                ServiceDecoder::Arm64(d) => {
                    let insn = d.decode_insn(mmu, pc)?;
                    Ok(ServiceInstruction::new(insn))
                }
                ServiceDecoder::X86_64(d) => {
                    let insn = d.decode_insn(mmu, pc)?;
                    Ok(ServiceInstruction::new(insn))
                }
                ServiceDecoder::PowerPC64Unsupported => {
                    Err(VmError::Core(vm_core::CoreError::NotImplemented {
                        feature: "PowerPC64 frontend decoder".to_string(),
                        module: "vm-service".to_string(),
                    }))
                }
            }
        }

        fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, VmError> {
            match self {
                ServiceDecoder::Riscv64(d) => d.decode(mmu, pc),
                ServiceDecoder::Arm64(d) => d.decode(mmu, pc),
                ServiceDecoder::X86_64(d) => d.decode(mmu, pc),
                ServiceDecoder::PowerPC64Unsupported => {
                    Err(VmError::Core(vm_core::CoreError::NotImplemented {
                        feature: "PowerPC64 frontend decoder".to_string(),
                        module: "vm-service".to_string(),
                    }))
                }
            }
        }
    }

    /// 解码器工厂
    pub struct DecoderFactory;

    impl DecoderFactory {
        pub fn create(arch: GuestArch) -> ServiceDecoder {
            match arch {
                GuestArch::Riscv64 => ServiceDecoder::Riscv64(RiscvDecoder),
                GuestArch::Arm64 => ServiceDecoder::Arm64(Arm64Decoder::new()),
                GuestArch::X86_64 => ServiceDecoder::X86_64(X86Decoder::new()),
                GuestArch::PowerPC64 => {
                    // PowerPC64 frontend decoder is not yet implemented
                    //
                    // The PowerPC64 decoder exists in vm-cross-arch module but doesn't implement
                    // the Decoder trait expected by vm-service. To fully support PowerPC64:
                    //
                    // 1. Add PowerPC64 module to vm-frontend crate
                    // 2. Implement the Decoder trait for PowerPC64 frontend
                    // 3. Add PowerPC64Instruction wrapper implementing InstructionTrait
                    // 4. Export from vm-frontend/src/lib.rs
                    //
                    // For now, we return a placeholder that produces a clear error message
                    // when someone attempts to decode PowerPC64 instructions.
                    ServiceDecoder::PowerPC64Unsupported
                }
            }
        }
    }
}

/// Decoder factory module without performance features (minimal implementation)
#[cfg(not(feature = "performance"))]
pub mod decoder_factory {
    use super::*;

    /// Placeholder decoder when frontend support is disabled
    pub enum ServiceDecoder {
        Unavailable,
    }

    impl Decoder for ServiceDecoder {
        type Instruction = vm_core::Instruction;
        type Block = IRBlock;

        fn decode_insn(
            &mut self,
            _mmu: &dyn MMU,
            _pc: GuestAddr,
        ) -> Result<Self::Instruction, VmError> {
            Err(VmError::Core(vm_core::CoreError::NotImplemented {
                feature: "Frontend decoder (enable 'performance' feature)".to_string(),
                module: "vm-service".to_string(),
            }))
        }

        fn decode(&mut self, _mmu: &dyn MMU, _pc: GuestAddr) -> Result<Self::Block, VmError> {
            Err(VmError::Core(vm_core::CoreError::NotImplemented {
                feature: "Frontend decoder (enable 'performance' feature)".to_string(),
                module: "vm-service".to_string(),
            }))
        }
    }

    /// 解码器工厂
    pub struct DecoderFactory;

    impl DecoderFactory {
        pub fn create(_arch: GuestArch) -> ServiceDecoder {
            ServiceDecoder::Unavailable
        }
    }
}
