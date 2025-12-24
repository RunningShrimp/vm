//! Refactored architecture encoder using common modules
//! 
//! This module demonstrates how to use the new common encoding framework
//! to reduce code duplication across different architectures.

use super::{Architecture, TargetInstruction};
use vm_error::Architecture as VmErrorArchitecture;
use vm_ir::RegId;
use vm_encoding::{
    EncodingContext, InstructionBuilder, ImmediateFormat, RegisterField, EncodingError, utils, InstructionFlag
};
use vm_register::{RegisterMapper, RegisterSet, MappingStrategy, RegisterClass};
use vm_memory_access::{MemoryAccessPattern, AccessType, AccessWidth, Alignment, DefaultMemoryAccessOptimizer, MemoryAccessOptimizer};
use vm_instruction_patterns::DefaultPatternMatcher;
use vm_optimization::{
    OptimizationPipeline, OptimizationContext, OptimizationLevel,
    DeadCodeEliminationPass, ConstantFoldingPass, CommonSubexpressionEliminationPass
};

/// Refactored architecture encoder using common modules
pub struct RefactoredArchEncoder {
    architecture: Architecture,
    encoding_context: EncodingContext,
    register_set: RegisterSet,
    register_mapper: RegisterMapper,
    pattern_matcher: DefaultPatternMatcher,
    optimization_pipeline: OptimizationPipeline,
}

impl RefactoredArchEncoder {
    /// Create a new encoder for the specified architecture
    pub fn new(architecture: Architecture) -> Result<Self, EncodingError> {
        // 将本地 Architecture 类型转换为 vm_error::Architecture 类型
        let error_arch = match architecture {
            Architecture::X86_64 => VmErrorArchitecture::X86_64,
            Architecture::ARM64 => VmErrorArchitecture::ARM64,
            Architecture::RISCV64 => VmErrorArchitecture::RISCV64,
        };
        let encoding_context = EncodingContext::new(error_arch);
        let register_set = Self::create_register_set(architecture)?;
        let register_mapper = RegisterMapper::new(
            register_set.clone(),
            register_set.clone(),
            MappingStrategy::Optimized,
        );
        
        let mut pattern_matcher = DefaultPatternMatcher::new();
        pattern_matcher.initialize_common_patterns();
        
        let optimization_context = OptimizationContext::new(error_arch, error_arch)
            .with_optimization_level(OptimizationLevel::Standard);
        let mut optimization_pipeline = OptimizationPipeline::new(optimization_context);
        optimization_pipeline.add_pass(Box::new(ConstantFoldingPass));
        optimization_pipeline.add_pass(Box::new(DeadCodeEliminationPass));
        optimization_pipeline.add_pass(Box::new(CommonSubexpressionEliminationPass));

        Ok(Self {
            architecture,
            encoding_context,
            register_set,
            register_mapper,
            pattern_matcher,
            optimization_pipeline,
        })
    }

    /// Create register set for the specified architecture
    fn create_register_set(architecture: Architecture) -> Result<RegisterSet, EncodingError> {
        // 将本地 Architecture 类型转换为 vm_error::Architecture 类型
        let error_arch = match architecture {
            Architecture::X86_64 => VmErrorArchitecture::X86_64,
            Architecture::ARM64 => VmErrorArchitecture::ARM64,
            Architecture::RISCV64 => VmErrorArchitecture::RISCV64,
        };
        let mut register_set = RegisterSet::new(error_arch);
        
        match architecture {
            Architecture::X86_64 => {
                Self::add_x86_registers(&mut register_set);
            }
            Architecture::ARM64 => {
                Self::add_arm64_registers(&mut register_set);
            }
            Architecture::RISCV64 => {
                Self::add_riscv_registers(&mut register_set);
            }
        }
        
        Ok(register_set)
    }

    /// Add x86-64 registers
    fn add_x86_registers(register_set: &mut RegisterSet) {
        use vm_register::{RegisterInfo, RegisterType};
        
        // General purpose registers
        for i in 0..16 {
            let name = match i {
                0 => "rax", 1 => "rcx", 2 => "rdx", 3 => "rbx",
                4 => "rsp", 5 => "rbp", 6 => "rsi", 7 => "rdi",
                8 => "r8", 9 => "r9", 10 => "r10", 11 => "r11",
                12 => "r12", 13 => "r13", 14 => "r14", 15 => "r15",
                _ => unreachable!(),
            };
            
            let info = RegisterInfo::new(
                i,
                name,
                RegisterClass::GeneralPurpose,
                RegisterType::Integer { width: 64 },
            ).with_caller_saved();
            
            register_set.add_register(info);
        }
    }

    /// Add ARM64 registers
    fn add_arm64_registers(register_set: &mut RegisterSet) {
        use vm_register::{RegisterInfo, RegisterType};
        
        // General purpose registers (x0-x30)
        for i in 0..31 {
            let name = if i == 29 { "fp" } else if i == 30 { "lr" } else { &format!("x{}", i) };
            
            let info = RegisterInfo::new(
                i,
                name,
                RegisterClass::GeneralPurpose,
                RegisterType::Integer { width: 64 },
            ).with_callee_saved();
            
            register_set.add_register(info);
        }
    }

    /// Add RISC-V registers
    fn add_riscv_registers(register_set: &mut RegisterSet) {
        use vm_register::{RegisterInfo, RegisterType};
        
        // General purpose registers (x0-x31)
        for i in 0..32 {
            let name = match i {
                0 => "zero", 1 => "ra", 2 => "sp", 3 => "gp",
                4 => "tp", 5 => "t0", 6 => "t1", 7 => "t2",
                8 => "s0", 9 => "s1", 10 => "a0", 11 => "a1",
                12 => "a2", 13 => "a3", 14 => "a4", 15 => "a5",
                16 => "a6", 17 => "a7", 18 => "s2", 19 => "s3",
                20 => "s4", 21 => "s5", 22 => "s6", 23 => "s7",
                24 => "s8", 25 => "s9", 26 => "s10", 27 => "s11",
                28 => "t3", 29 => "t4", 30 => "t5", 31 => "t6",
                _ => unreachable!(),
            };
            
            let info = RegisterInfo::new(
                i,
                name,
                RegisterClass::GeneralPurpose,
                RegisterType::Integer { width: 64 },
            );
            
            register_set.add_register(info);
        }
    }

    /// Encode an arithmetic operation using common framework
    fn encode_arithmetic_op(
        &mut self,
        dst: RegId,
        src1: RegId,
        src2: RegId,
        operation: &str,
    ) -> Result<Vec<TargetInstruction>, EncodingError> {
        let mut builder = SimpleInstructionBuilder::new(self.architecture);
        
        // Map registers to target architecture
        let target_dst = self.register_mapper.map_register(dst)
            .map_err(|_| EncodingError::InvalidFormat)?;
        let target_src1 = self.register_mapper.map_register(src1)
            .map_err(|_| EncodingError::InvalidFormat)?;
        let target_src2 = self.register_mapper.map_register(src2)
            .map_err(|_| EncodingError::InvalidFormat)?;
        
        // Add opcode based on architecture
        match self.architecture {
            Architecture::X86_64 => {
                builder.add_opcode(0x48); // REX.W prefix
                builder.add_opcode(match operation {
                    "add" => 0x01, // ADD r/m64, r64
                    "sub" => 0x29, // SUB r/m64, r64
                    "and" => 0x21, // AND r/m64, r64
                    "or" => 0x09,  // OR r/m64, r64
                    "xor" => 0x31, // XOR r/m64, r64
                    _ => return Err(EncodingError::InvalidFormat),
                });
                
                // Add ModR/M byte
                let modrm = 0xC0 | ((target_dst & 7) << 3) | (target_src2 & 7);
                builder.add_opcode(modrm as u8);
            }
            Architecture::ARM64 => {
                let base_opcode = match operation {
                    "add" => 0x8B000000,
                    "sub" => 0xCB000000,
                    "and" => 0x8A000000,
                    "or" => 0x8A200000,
                    "xor" => 0xCA000000,
                    _ => return Err(EncodingError::InvalidFormat),
                };
                
                let word = base_opcode
                    | ((target_dst & 0x1F) as u32)      // Rd
                    | (((target_src1 & 0x1F) as u32) << 5) // Rn
                    | (((target_src2 & 0x1F) as u32) << 16); // Rm
                
                builder.add_opcode((word & 0xFF) as u8);
                builder.add_opcode(((word >> 8) & 0xFF) as u8);
                builder.add_opcode(((word >> 16) & 0xFF) as u8);
                builder.add_opcode(((word >> 24) & 0xFF) as u8);
            }
            Architecture::RISCV64 => {
                let base_opcode = match operation {
                    "add" => 0x00000033,
                    "sub" => 0x40000033,
                    "and" => 0x70000033,
                    "or" => 0x60000033,
                    "xor" => 0x40003333,
                    _ => return Err(EncodingError::InvalidFormat),
                };
                
                let word = base_opcode
                    | (((target_dst & 0x1F) as u32) << 7)   // rd
                    | (((target_src1 & 0x1F) as u32) << 15)  // rs1
                    | (((target_src2 & 0x1F) as u32) << 20); // rs2
                
                builder.add_opcode((word & 0xFF) as u8);
                builder.add_opcode(((word >> 8) & 0xFF) as u8);
                builder.add_opcode(((word >> 16) & 0xFF) as u8);
                builder.add_opcode(((word >> 24) & 0xFF) as u8);
            }
        }
        
        let bytes = builder.build()?;
        let length = bytes.len();
        Ok(vec![TargetInstruction {
            bytes,
            length,
            mnemonic: operation.to_string(),
            is_control_flow: false,
            is_memory_op: false,
        }])
    }

    /// Encode a memory access operation using common framework
    fn encode_memory_op(
        &mut self,
        dst: RegId,
        base: RegId,
        offset: i64,
        width: AccessWidth,
        access_type: AccessType,
    ) -> Result<Vec<TargetInstruction>, EncodingError> {
        let mut builder = SimpleInstructionBuilder::new(self.architecture);
        
        // Map registers to target architecture
        let target_dst = self.register_mapper.map_register(dst)
            .map_err(|_| EncodingError::InvalidFormat)?;
        let target_base = self.register_mapper.map_register(base)
            .map_err(|_| EncodingError::InvalidFormat)?;
        
        // Create memory access pattern
        let pattern = MemoryAccessPattern::new(target_base, offset, width)
            .with_access_type(access_type)
            .with_alignment(Alignment::Natural);
        
        // Optimize memory access pattern
        let optimizer = DefaultMemoryAccessOptimizer::new(self.encoding_context.architecture);
        let optimized_pattern = optimizer.optimize_access_pattern(&pattern);
        
        // Use optimized pattern for encoding
        let effective_offset = optimized_pattern.optimized.offset;
        let effective_width = optimized_pattern.optimized.width;
        
        // Encode based on architecture
        match self.architecture {
            Architecture::X86_64 => {
                match access_type {
                    AccessType::Read => {
                        // Select appropriate opcode based on effective width
                        let (rex_prefix, opcode) = match effective_width {
                            AccessWidth::Byte => (Some(0x40), 0x8A), // MOV r8, r/m8
                            AccessWidth::HalfWord => (Some(0x66), 0x8B), // MOV r16, r/m16 with 0x66 prefix
                            AccessWidth::Word => (None, 0x8B), // MOV r32, r/m32
                            AccessWidth::DoubleWord | AccessWidth::QuadWord => (Some(0x48), 0x8B), // MOV r64, r/m64
                            AccessWidth::Vector(_) => (Some(0x48), 0x8B), // For vectorized access, use quadword
                        };
                        
                        // Add prefixes if needed
                        if let Some(prefix) = rex_prefix {
                            builder.add_opcode(prefix);
                        }
                        
                        builder.add_opcode(opcode);
                        
                        // Add ModR/M byte
                        let modrm = if effective_offset == 0 {
                            0x00 | ((target_dst & 7) << 3) | (target_base & 7)
                        } else if utils::immediate_fits(effective_offset, ImmediateFormat::Signed32) {
                            0x80 | ((target_dst & 7) << 3) | (target_base & 7)
                        } else {
                            0x80 | ((target_dst & 7) << 3) | (target_base & 7)
                        };
                        builder.add_opcode(modrm as u8);
                        
                        // Add displacement if needed
                        if effective_offset != 0 {
                            builder.add_immediate(effective_offset, ImmediateFormat::Signed32);
                        }
                    }
                    AccessType::Write => {
                        // Select appropriate opcode based on effective width
                        let (rex_prefix, opcode) = match effective_width {
                            AccessWidth::Byte => (Some(0x40), 0x88), // MOV r/m8, r8
                            AccessWidth::HalfWord => (Some(0x66), 0x89), // MOV r/m16, r16 with 0x66 prefix
                            AccessWidth::Word => (None, 0x89), // MOV r/m32, r32
                            AccessWidth::DoubleWord | AccessWidth::QuadWord => (Some(0x48), 0x89), // MOV r/m64, r64
                            AccessWidth::Vector(_) => (Some(0x48), 0x89), // For vectorized access, use quadword
                        };
                        
                        // Add prefixes if needed
                        if let Some(prefix) = rex_prefix {
                            builder.add_opcode(prefix);
                        }
                        
                        builder.add_opcode(opcode);
                        
                        // Add ModR/M byte
                        let modrm = if effective_offset == 0 {
                            0x00 | ((target_dst & 7) << 3) | (target_base & 7)
                        } else if utils::immediate_fits(effective_offset, ImmediateFormat::Signed32) {
                            0x80 | ((target_dst & 7) << 3) | (target_base & 7)
                        } else {
                            0x80 | ((target_dst & 7) << 3) | (target_base & 7)
                        };
                        builder.add_opcode(modrm as u8);
                        
                        // Add displacement if needed
                        if effective_offset != 0 {
                            builder.add_immediate(effective_offset, ImmediateFormat::Signed32);
                        }
                    }
                    _ => {
                        return Err(EncodingError::InvalidFormat);
                    }
                }
            }
            Architecture::ARM64 => {
                // Select appropriate opcode based on effective width
                let base_opcode = match (access_type, effective_width) {
                    (AccessType::Read, AccessWidth::Byte) => 0x39400000, // LDRB W[t], [X[n]]
                    (AccessType::Read, AccessWidth::HalfWord) => 0x79400000, // LDRH W[t], [X[n]]
                    (AccessType::Read, AccessWidth::Word) => 0xB9400000, // LDR W[t], [X[n]]
                    (AccessType::Read, AccessWidth::DoubleWord | AccessWidth::QuadWord) => 0xF9400000, // LDR X[t], [X[n]]
                    (AccessType::Read, AccessWidth::Vector(_)) => 0x3DC00000, // LDR Q[t], [X[n]] (128-bit vector)
                    (AccessType::Write, AccessWidth::Byte) => 0x39000000, // STRB W[t], [X[n]]
                    (AccessType::Write, AccessWidth::HalfWord) => 0x79000000, // STRH W[t], [X[n]]
                    (AccessType::Write, AccessWidth::Word) => 0xB9000000, // STR W[t], [X[n]]
                    (AccessType::Write, AccessWidth::DoubleWord | AccessWidth::QuadWord) => 0xF9000000, // STR X[t], [X[n]]
                    (AccessType::Write, AccessWidth::Vector(_)) => 0x3D800000, // STR Q[t], [X[n]] (128-bit vector)
                    _ => return Err(EncodingError::InvalidFormat),
                };
                
                let word = base_opcode
                    | ((target_dst & 0x1F) as u32)      // Rt
                    | (((target_base & 0x1F) as u32) << 5); // Rn
                
                builder.add_opcode((word & 0xFF) as u8);
                builder.add_opcode(((word >> 8) & 0xFF) as u8);
                builder.add_opcode(((word >> 16) & 0xFF) as u8);
                builder.add_opcode(((word >> 24) & 0xFF) as u8);
                
                // Add offset if needed
                if effective_offset != 0 {
                    builder.add_immediate(effective_offset, ImmediateFormat::Signed12);
                }
            }
            Architecture::RISCV64 => {
                // Select appropriate opcode based on effective width
                let (base_opcode, funct3) = match (access_type, effective_width) {
                    (AccessType::Read, AccessWidth::Byte) => (0x00000003, 0x0), // LB
                    (AccessType::Read, AccessWidth::HalfWord) => (0x00001003, 0x1), // LH
                    (AccessType::Read, AccessWidth::Word) => (0x00002003, 0x2), // LW
                    (AccessType::Read, AccessWidth::DoubleWord | AccessWidth::QuadWord) => (0x00003003, 0x3), // LD
                    (AccessType::Read, AccessWidth::Vector(_)) => (0x00003003, 0x3), // For vectorized access, use LD
                    (AccessType::Write, AccessWidth::Byte) => (0x00000023, 0x0), // SB
                    (AccessType::Write, AccessWidth::HalfWord) => (0x00001023, 0x1), // SH
                    (AccessType::Write, AccessWidth::Word) => (0x00002023, 0x2), // SW
                    (AccessType::Write, AccessWidth::DoubleWord | AccessWidth::QuadWord) => (0x00003023, 0x3), // SD
                    (AccessType::Write, AccessWidth::Vector(_)) => (0x00003023, 0x3), // For vectorized access, use SD
                    _ => return Err(EncodingError::InvalidFormat),
                };
                
                let word = base_opcode
                    | (((target_dst & 0x1F) as u32) << 7)   // rd
                    | (((target_base & 0x1F) as u32) << 15)  // rs1
                    | ((funct3 as u32) << 12); // funct3
                
                builder.add_opcode((word & 0xFF) as u8);
                builder.add_opcode(((word >> 8) & 0xFF) as u8);
                builder.add_opcode(((word >> 16) & 0xFF) as u8);
                builder.add_opcode(((word >> 24) & 0xFF) as u8);
                
                // Add offset if needed
                if effective_offset != 0 {
                    builder.add_immediate(effective_offset, ImmediateFormat::Signed12);
                }
            }
        }
        
        let bytes = builder.build()?;
        let length = bytes.len();
        Ok(vec![TargetInstruction {
            bytes,
            length,
            mnemonic: "mov".to_string(),
            is_control_flow: false,
            is_memory_op: true,
        }])
    }


}

/// Simple instruction builder implementation
struct SimpleInstructionBuilder {
    architecture: Architecture,
    bytes: Vec<u8>,
}

impl SimpleInstructionBuilder {
    fn new(architecture: Architecture) -> Self {
        Self {
            architecture,
            bytes: Vec::new(),
        }
    }
}

impl InstructionBuilder for SimpleInstructionBuilder {
    fn add_opcode(&mut self, opcode: u8) -> &mut Self {
        self.bytes.push(opcode);
        self
    }

    fn add_register(&mut self, _reg: RegId, _field: RegisterField) -> &mut Self {
        // Simplified implementation - would be architecture-specific
        self
    }

    fn add_immediate(&mut self, imm: i64, format: ImmediateFormat) -> &mut Self {
        match format {
            ImmediateFormat::Signed12 => {
                let imm = imm as u16;
                self.bytes.push((imm & 0xFF) as u8);
                self.bytes.push(((imm >> 8) & 0xFF) as u8);
            }
            ImmediateFormat::Signed32 => {
                let imm = imm as u32;
                self.bytes.push((imm & 0xFF) as u8);
                self.bytes.push(((imm >> 8) & 0xFF) as u8);
                self.bytes.push(((imm >> 16) & 0xFF) as u8);
                self.bytes.push(((imm >> 24) & 0xFF) as u8);
            }
            _ => {
                // Handle other formats as needed
            }
        }
        self
    }

    fn add_memory_operand(&mut self, _base: RegId, _offset: i64, _size: u8) -> &mut Self {
        // Simplified implementation
        self
    }

    fn set_flag(&mut self, _flag: InstructionFlag) -> &mut Self {
        // Simplified implementation
        self
    }

    fn build(self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.bytes)
    }

    fn size(&self) -> usize {
        self.bytes.len()
    }
}

/// Example usage of the refactored encoder
pub fn example_refactored_encoding() -> Result<(), EncodingError> {
    // Create encoder for x86-64
    let mut encoder = RefactoredArchEncoder::new(Architecture::X86_64)?;
    
    // Encode an ADD instruction
    let add_instructions = encoder.encode_arithmetic_op(0, 1, 2, "add")?;
    println!("ADD instruction encoded: {:?}", add_instructions);
    
    // Encode a memory load
    let load_instructions = encoder.encode_memory_op(
        3,  // dst register
        4,  // base register
        0x1000,  // offset
        AccessWidth::Word,
        AccessType::Read,
    )?;
    println!("LOAD instruction encoded: {:?}", load_instructions);
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refactored_encoder_creation() {
        let encoder = RefactoredArchEncoder::new(Architecture::X86_64);
        assert!(encoder.is_ok());
    }

    #[test]
    fn test_arithmetic_encoding() {
        let encoder = RefactoredArchEncoder::new(Architecture::X86_64).unwrap();
        let result = encoder.encode_arithmetic_op(0, 1, 2, "add");
        assert!(result.is_ok());
        
        let instructions = result.unwrap();
        assert!(!instructions.is_empty());
    }

    #[test]
    fn test_memory_encoding() {
        let encoder = RefactoredArchEncoder::new(Architecture::X86_64).unwrap();
        let result = encoder.encode_memory_op(
            3, 4, 0x1000, AccessWidth::Word, AccessType::Read
        );
        assert!(result.is_ok());
        
        let instructions = result.unwrap();
        assert!(!instructions.is_empty());
    }
}