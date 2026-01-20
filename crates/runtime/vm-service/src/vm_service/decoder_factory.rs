//! Decoder factory module
//!
//! Provides decoder creation utilities for different architectures.

use vm_core::{Decoder, GuestAddr, GuestArch, MMU, VmError, VmResult};
use vm_ir::IRBlock;

/// Unified decoder enum that wraps all architecture-specific decoders
#[cfg(feature = "performance")]
pub enum UnifiedDecoder {
    Riscv64(vm_frontend::riscv64::RiscvDecoder),
    Arm64(vm_frontend::arm64::Arm64Decoder),
    X86_64(vm_frontend::x86_64::X86Decoder),
}

#[cfg(feature = "performance")]
impl Decoder for UnifiedDecoder {
    type Instruction = vm_ir::IROp;
    type Block = IRBlock;

    fn decode_insn(
        &mut self,
        _mmu: &mut (dyn MMU + 'static),
        _pc: GuestAddr,
    ) -> VmResult<Self::Instruction> {
        // Single instruction decoding is not commonly used
        // Use decode() for block decoding instead
        Err(VmError::Core(vm_core::CoreError::Internal {
            message: "Use decode() for block decoding".to_string(),
            module: "UnifiedDecoder".to_string(),
        }))
    }

    fn decode(&mut self, mmu: &mut (dyn MMU + 'static), pc: GuestAddr) -> VmResult<Self::Block> {
        match self {
            UnifiedDecoder::Riscv64(decoder) => decoder.decode(mmu, pc),
            UnifiedDecoder::Arm64(decoder) => decoder.decode(mmu, pc),
            UnifiedDecoder::X86_64(decoder) => decoder.decode(mmu, pc),
        }
    }
}

/// Create a unified decoder for the specified architecture
#[cfg(feature = "performance")]
pub fn create_decoder(arch: GuestArch) -> UnifiedDecoder {
    match arch {
        GuestArch::Riscv64 => UnifiedDecoder::Riscv64(vm_frontend::riscv64::RiscvDecoder),
        GuestArch::Arm64 => UnifiedDecoder::Arm64(vm_frontend::arm64::Arm64Decoder::new()),
        GuestArch::X86_64 => UnifiedDecoder::X86_64(vm_frontend::x86_64::X86Decoder::new()),
        GuestArch::PowerPC64 => {
            // PowerPC64 decoder not yet implemented, fallback to RISC-V
            UnifiedDecoder::Riscv64(vm_frontend::riscv64::RiscvDecoder)
        }
    }
}
