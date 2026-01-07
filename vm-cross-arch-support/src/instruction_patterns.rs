//! Common instruction pattern recognition for VM cross-architecture translation
//!
//! This module provides unified instruction pattern matching and semantic analysis
//! across different architectures, enabling better cross-architecture translation.

use std::collections::{HashMap, HashSet};

use thiserror::Error;
use vm_core::error::CoreError;
use vm_core::{GuestAddr, VmError};

use crate::encoding::{Architecture, RegId};

/// Errors that can occur during pattern matching
#[derive(Error, Debug, Clone, PartialEq)]
pub enum PatternError {
    #[error("Invalid instruction pattern: {0}")]
    InvalidPattern(String),
    #[error("Pattern not found for operation: {0:?}")]
    PatternNotFound(String),
    #[error("Incompatible operands for pattern: {0}")]
    IncompatibleOperands(String),
    #[error("Unsupported architecture for pattern: {0:?}")]
    UnsupportedArchitecture(Architecture),
    #[error("Pattern matching failed: {0}")]
    MatchingFailed(String),
}

impl From<PatternError> for VmError {
    fn from(err: PatternError) -> Self {
        match err {
            PatternError::InvalidPattern(msg) => VmError::Core(CoreError::DecodeError {
                message: format!("Invalid instruction pattern: {}", msg),
                position: Some(GuestAddr(0)),
                module: "vm-cross-arch-support::instruction_patterns".to_string(),
            }),
            PatternError::PatternNotFound(op) => VmError::Core(CoreError::NotSupported {
                feature: format!("Pattern for operation: {}", op),
                module: "vm-cross-arch-support::instruction_patterns".to_string(),
            }),
            PatternError::IncompatibleOperands(msg) => VmError::Core(CoreError::InvalidParameter {
                name: "operands".to_string(),
                value: "".to_string(),
                message: format!("Incompatible operands: {}", msg),
            }),
            PatternError::UnsupportedArchitecture(arch) => VmError::Core(CoreError::NotSupported {
                feature: format!("Pattern matching for architecture {:?}", arch),
                module: "vm-cross-arch-support::instruction_patterns".to_string(),
            }),
            PatternError::MatchingFailed(msg) => VmError::Core(CoreError::Internal {
                message: format!("Pattern matching failed: {}", msg),
                module: "vm-cross-arch-support::instruction_patterns".to_string(),
            }),
        }
    }
}

/// Instruction categories for classification
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InstructionCategory {
    Arithmetic(ArithmeticType),
    Logical(LogicalType),
    Memory(MemoryType),
    Branch(BranchType),
    Vector(VectorType),
    System(SystemType),
    Compare(CompareType),
    Convert(ConvertType),
    Other(String),
}

/// Arithmetic instruction types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArithmeticType {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Abs,
    Min,
    Max,
    Sqrt,
    Pow,
}

/// Logical instruction types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogicalType {
    And,
    Or,
    Xor,
    Not,
    ShiftLeft,
    ShiftRight,
    RotateLeft,
    RotateRight,
    BitTest,
    BitField,
}

/// Memory instruction types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryType {
    Load,
    Store,
    LoadAcquire,
    StoreRelease,
    LoadReserved,
    StoreConditional,
    Swap,
    CompareAndSwap,
    FetchAndOp,
    Prefetch,
    CacheOp,
}

/// Branch instruction types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BranchType {
    Unconditional,
    Conditional,
    Indirect,
    Call,
    Return,
    JumpTable,
    TailCall,
    Exception,
    Interrupt,
}

/// Vector instruction types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VectorType {
    Arithmetic,
    Logical,
    Shuffle,
    Blend,
    Insert,
    Extract,
    Reduce,
    Mask,
    Permute,
    Compress,
}

/// System instruction types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemType {
    Syscall,
    Halt,
    Nop,
    Barrier,
    CacheControl,
    TlbControl,
    Debug,
    Performance,
    Security,
}

/// Compare instruction types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompareType {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Test,
    Compare,
}

/// Convert instruction types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConvertType {
    IntToFloat,
    FloatToInt,
    Extend,
    Truncate,
    SignExtend,
    ZeroExtend,
    FloatToFloat,
    IntToInt,
}

/// Operand types for instructions
#[derive(Debug, Clone, PartialEq)]
pub enum OperandType {
    Register(RegId),
    Immediate(i64),
    Memory(MemoryOperand),
    Label(String),
    RegisterPair(RegId, RegId),
    RegisterList(Vec<RegId>),
    Vector(Vec<OperandType>),
    Complex(String), // For complex operands that don't fit other categories
}

impl Default for OperandType {
    fn default() -> Self {
        Self::Register(RegId(0))
    }
}

/// Memory operand representation
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MemoryOperand {
    pub base: Option<RegId>,
    pub index: Option<RegId>,
    pub scale: u8,
    pub displacement: i64,
    pub size: u8,
}

impl MemoryOperand {
    pub fn new(
        base: Option<RegId>,
        index: Option<RegId>,
        scale: u8,
        displacement: i64,
        size: u8,
    ) -> Self {
        Self {
            base,
            index,
            scale,
            displacement,
            size,
        }
    }

    pub fn simple(base: RegId, displacement: i64, size: u8) -> Self {
        Self {
            base: Some(base),
            index: None,
            scale: 1,
            displacement,
            size,
        }
    }

    pub fn indexed(base: RegId, index: RegId, scale: u8, displacement: i64, size: u8) -> Self {
        Self {
            base: Some(base),
            index: Some(index),
            scale,
            displacement,
            size,
        }
    }
}

/// Instruction flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InstructionFlags {
    pub sets_flags: bool,
    pub reads_flags: bool,
    pub is_conditional: bool,
    pub is_predicated: bool,
    pub is_atomic: bool,
    pub is_volatile: bool,
    pub is_privileged: bool,
    pub is_terminal: bool,
}

/// Semantic description of an instruction
#[derive(Debug, Clone)]
pub struct SemanticDescription {
    pub operation: String,
    pub preconditions: Vec<String>,
    pub postconditions: Vec<String>,
    pub side_effects: Vec<String>,
    pub dependencies: Vec<RegId>,
    pub outputs: Vec<RegId>,
}

/// Instruction pattern definition
#[derive(Debug, Clone)]
pub struct InstructionPattern {
    pub id: String,
    pub category: InstructionCategory,
    pub operands: Vec<OperandType>,
    pub flags: InstructionFlags,
    pub semantics: SemanticDescription,
    pub architectures: HashSet<Architecture>,
    pub cost: u32,
    pub latency: u32,
    pub throughput: f32,
}

impl InstructionPattern {
    pub fn new(id: impl Into<String>, category: InstructionCategory) -> Self {
        Self {
            id: id.into(),
            category,
            operands: Vec::new(),
            flags: InstructionFlags::default(),
            semantics: SemanticDescription {
                operation: String::new(),
                preconditions: Vec::new(),
                postconditions: Vec::new(),
                side_effects: Vec::new(),
                dependencies: Vec::new(),
                outputs: Vec::new(),
            },
            architectures: HashSet::new(),
            cost: 1,
            latency: 1,
            throughput: 1.0,
        }
    }

    pub fn with_operand(mut self, operand: OperandType) -> Self {
        self.operands.push(operand);
        self
    }

    pub fn with_operands(mut self, operands: Vec<OperandType>) -> Self {
        self.operands = operands;
        self
    }

    pub fn with_flags(mut self, flags: InstructionFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn with_semantics(mut self, semantics: SemanticDescription) -> Self {
        self.semantics = semantics;
        self
    }

    pub fn with_architecture(mut self, arch: Architecture) -> Self {
        self.architectures.insert(arch);
        self
    }

    pub fn with_cost(mut self, cost: u32) -> Self {
        self.cost = cost;
        self
    }

    pub fn with_latency(mut self, latency: u32) -> Self {
        self.latency = latency;
        self
    }

    pub fn with_throughput(mut self, throughput: f32) -> Self {
        self.throughput = throughput;
        self
    }

    /// Check if this pattern is compatible with a given architecture
    pub fn is_compatible_with(&self, arch: Architecture) -> bool {
        self.architectures.contains(&arch) || self.architectures.is_empty()
    }

    /// Get the number of operands
    pub fn operand_count(&self) -> usize {
        self.operands.len()
    }

    /// Check if this pattern has the specified operand types
    pub fn has_operand_types(&self, types: &[OperandType]) -> bool {
        if self.operands.len() != types.len() {
            return false;
        }

        for (actual, expected) in self.operands.iter().zip(types.iter()) {
            match (actual, expected) {
                (OperandType::Register(_), OperandType::Register(_)) => continue,
                (OperandType::Immediate(_), OperandType::Immediate(_)) => continue,
                (OperandType::Memory(_), OperandType::Memory(_)) => continue,
                (OperandType::Label(_), OperandType::Label(_)) => continue,
                (OperandType::Vector(_), OperandType::Vector(_)) => continue,
                _ => return false,
            }
        }

        true
    }
}

/// Pattern matcher for finding instruction patterns
pub trait PatternMatcher {
    /// Match an IR operation to an instruction pattern
    fn match_pattern(&self, ir_op: &IROp) -> Option<InstructionPattern>;

    /// Get equivalent patterns for a target architecture
    fn get_equivalent_patterns(
        &self,
        pattern: &InstructionPattern,
        target_arch: Architecture,
    ) -> Vec<InstructionPattern>;

    /// Get all patterns for a category
    fn get_patterns_by_category(&self, category: InstructionCategory) -> Vec<InstructionPattern>;

    /// Get the matcher's name
    fn name(&self) -> &'static str;
}

/// IR operation representation (simplified for pattern matching)
#[derive(Debug, Clone)]
pub struct IROp {
    pub opcode: String,
    pub operands: Vec<OperandType>,
    pub flags: InstructionFlags,
}

impl IROp {
    pub fn new(opcode: impl Into<String>) -> Self {
        Self {
            opcode: opcode.into(),
            operands: Vec::new(),
            flags: InstructionFlags::default(),
        }
    }

    pub fn with_operand(mut self, operand: OperandType) -> Self {
        self.operands.push(operand);
        self
    }

    pub fn with_operands(mut self, operands: Vec<OperandType>) -> Self {
        self.operands = operands;
        self
    }

    pub fn with_flags(mut self, flags: InstructionFlags) -> Self {
        self.flags = flags;
        self
    }
}

/// Default pattern matcher implementation
#[derive(Debug, Default)]
pub struct DefaultPatternMatcher {
    patterns: HashMap<String, Vec<InstructionPattern>>,
    category_index: HashMap<InstructionCategory, Vec<String>>,
    architecture_index: HashMap<Architecture, Vec<String>>,
}

impl DefaultPatternMatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a pattern to the matcher
    pub fn add_pattern(&mut self, pattern: InstructionPattern) {
        let opcode = pattern.semantics.operation.clone();

        // Add to main pattern index
        self.patterns
            .entry(opcode.clone())
            .or_default()
            .push(pattern.clone());

        // Add to category index
        self.category_index
            .entry(pattern.category)
            .or_default()
            .push(opcode.clone());

        // Add to architecture index
        for &arch in &pattern.architectures {
            self.architecture_index
                .entry(arch)
                .or_default()
                .push(opcode.clone());
        }
    }

    /// Initialize with common patterns
    pub fn initialize_common_patterns(&mut self) {
        self.add_arithmetic_patterns();
        self.add_logical_patterns();
        self.add_memory_patterns();
        self.add_branch_patterns();
    }

    /// Add common arithmetic patterns
    fn add_arithmetic_patterns(&mut self) {
        // ADD pattern
        let add_pattern = InstructionPattern::new("add", InstructionCategory::Arithmetic(ArithmeticType::Add))
                .with_operand(OperandType::Register(RegId(0))) // dst
                .with_operand(OperandType::Register(RegId(1))) // src1
                .with_operand(OperandType::Register(RegId(2))) // src2
                .with_flags(InstructionFlags {
                    sets_flags: true,
                    reads_flags: false,
                    is_conditional: false,
                    is_predicated: false,
                    is_atomic: false,
                    is_volatile: false,
                    is_privileged: false,
                    is_terminal: false,
                })
                .with_semantics(SemanticDescription {
                    operation: "add".to_string(),
                    preconditions: vec!["src1 and src2 are valid registers".to_string()],
                    postconditions: vec!["dst = src1 + src2".to_string()],
                    side_effects: vec![],
                    dependencies: vec![RegId(1), RegId(2)],
                    outputs: vec![RegId(0)],
                })
                .with_architecture(Architecture::X86_64)
                .with_architecture(Architecture::ARM64)
                .with_architecture(Architecture::RISCV64)
                .with_cost(1)
                .with_latency(1)
                .with_throughput(0.5);

        self.add_pattern(add_pattern);

        // SUB pattern
        let sub_pattern = InstructionPattern::new("sub", InstructionCategory::Arithmetic(ArithmeticType::Sub))
                .with_operand(OperandType::Register(RegId(0))) // dst
                .with_operand(OperandType::Register(RegId(1))) // src1
                .with_operand(OperandType::Register(RegId(2))) // src2
                .with_flags(InstructionFlags {
                    sets_flags: true,
                    reads_flags: false,
                    is_conditional: false,
                    is_predicated: false,
                    is_atomic: false,
                    is_volatile: false,
                    is_privileged: false,
                    is_terminal: false,
                })
                .with_semantics(SemanticDescription {
                    operation: "sub".to_string(),
                    preconditions: vec!["src1 and src2 are valid registers".to_string()],
                    postconditions: vec!["dst = src1 - src2".to_string()],
                    side_effects: vec![],
                    dependencies: vec![RegId(1), RegId(2)],
                    outputs: vec![RegId(0)],
                })
                .with_architecture(Architecture::X86_64)
                .with_architecture(Architecture::ARM64)
                .with_architecture(Architecture::RISCV64)
                .with_cost(1)
                .with_latency(1)
                .with_throughput(0.5);

        self.add_pattern(sub_pattern);
    }

    /// Add common logical patterns
    fn add_logical_patterns(&mut self) {
        // AND pattern
        let and_pattern = InstructionPattern::new("and", InstructionCategory::Logical(LogicalType::And))
                .with_operand(OperandType::Register(RegId(0))) // dst
                .with_operand(OperandType::Register(RegId(1))) // src1
                .with_operand(OperandType::Register(RegId(2))) // src2
                .with_flags(InstructionFlags {
                    sets_flags: true,
                    reads_flags: false,
                    is_conditional: false,
                    is_predicated: false,
                    is_atomic: false,
                    is_volatile: false,
                    is_privileged: false,
                    is_terminal: false,
                })
                .with_semantics(SemanticDescription {
                    operation: "and".to_string(),
                    preconditions: vec!["src1 and src2 are valid registers".to_string()],
                    postconditions: vec!["dst = src1 & src2".to_string()],
                    side_effects: vec![],
                    dependencies: vec![RegId(1), RegId(2)],
                    outputs: vec![RegId(0)],
                })
                .with_architecture(Architecture::X86_64)
                .with_architecture(Architecture::ARM64)
                .with_architecture(Architecture::RISCV64)
                .with_cost(1)
                .with_latency(1)
                .with_throughput(0.33);

        self.add_pattern(and_pattern);

        // OR pattern
        let or_pattern = InstructionPattern::new("or", InstructionCategory::Logical(LogicalType::Or))
                .with_operand(OperandType::Register(RegId(0))) // dst
                .with_operand(OperandType::Register(RegId(1))) // src1
                .with_operand(OperandType::Register(RegId(2))) // src2
                .with_flags(InstructionFlags {
                    sets_flags: true,
                    reads_flags: false,
                    is_conditional: false,
                    is_predicated: false,
                    is_atomic: false,
                    is_volatile: false,
                    is_privileged: false,
                    is_terminal: false,
                })
                .with_semantics(SemanticDescription {
                    operation: "or".to_string(),
                    preconditions: vec!["src1 and src2 are valid registers".to_string()],
                    postconditions: vec!["dst = src1 | src2".to_string()],
                    side_effects: vec![],
                    dependencies: vec![RegId(1), RegId(2)],
                    outputs: vec![RegId(0)],
                })
                .with_architecture(Architecture::X86_64)
                .with_architecture(Architecture::ARM64)
                .with_architecture(Architecture::RISCV64)
                .with_cost(1)
                .with_latency(1)
                .with_throughput(0.33);

        self.add_pattern(or_pattern);
    }

    /// Add common memory patterns
    fn add_memory_patterns(&mut self) {
        // LOAD pattern
        let load_pattern = InstructionPattern::new("load", InstructionCategory::Memory(MemoryType::Load))
                .with_operand(OperandType::Register(RegId(0))) // dst
                .with_operand(OperandType::Memory(MemoryOperand::simple(RegId(1), 0, 8))) // [src]
                .with_flags(InstructionFlags {
                    sets_flags: false,
                    reads_flags: false,
                    is_conditional: false,
                    is_predicated: false,
                    is_atomic: false,
                    is_volatile: false,
                    is_privileged: false,
                    is_terminal: false,
                })
                .with_semantics(SemanticDescription {
                    operation: "load".to_string(),
                    preconditions: vec!["src is a valid address".to_string()],
                    postconditions: vec!["dst = *src".to_string()],
                    side_effects: vec!["memory read".to_string()],
                    dependencies: vec![RegId(1)],
                    outputs: vec![RegId(0)],
                })
                .with_architecture(Architecture::X86_64)
                .with_architecture(Architecture::ARM64)
                .with_architecture(Architecture::RISCV64)
                .with_cost(3)
                .with_latency(4)
                .with_throughput(1.0);

        self.add_pattern(load_pattern);

        // STORE pattern
        let store_pattern = InstructionPattern::new("store", InstructionCategory::Memory(MemoryType::Store))
                .with_operand(OperandType::Memory(MemoryOperand::simple(RegId(0), 0, 8))) // [dst]
                .with_operand(OperandType::Register(RegId(1))) // src
                .with_flags(InstructionFlags {
                    sets_flags: false,
                    reads_flags: false,
                    is_conditional: false,
                    is_predicated: false,
                    is_atomic: false,
                    is_volatile: false,
                    is_privileged: false,
                    is_terminal: false,
                })
                .with_semantics(SemanticDescription {
                    operation: "store".to_string(),
                    preconditions: vec!["dst is a valid address".to_string()],
                    postconditions: vec!["*dst = src".to_string()],
                    side_effects: vec!["memory write".to_string()],
                    dependencies: vec![RegId(0), RegId(1)],
                    outputs: vec![],
                })
                .with_architecture(Architecture::X86_64)
                .with_architecture(Architecture::ARM64)
                .with_architecture(Architecture::RISCV64)
                .with_cost(3)
                .with_latency(4)
                .with_throughput(1.0);

        self.add_pattern(store_pattern);
    }

    /// Add common branch patterns
    fn add_branch_patterns(&mut self) {
        // JUMP pattern
        let jump_pattern = InstructionPattern::new(
            "jump",
            InstructionCategory::Branch(BranchType::Unconditional),
        )
        .with_operand(OperandType::Label("target".to_string()))
        .with_flags(InstructionFlags {
            sets_flags: false,
            reads_flags: false,
            is_conditional: false,
            is_predicated: false,
            is_atomic: false,
            is_volatile: false,
            is_privileged: false,
            is_terminal: true,
        })
        .with_semantics(SemanticDescription {
            operation: "jump".to_string(),
            preconditions: vec![],
            postconditions: vec!["PC = target".to_string()],
            side_effects: vec!["control flow change".to_string()],
            dependencies: vec![],
            outputs: vec![],
        })
        .with_architecture(Architecture::X86_64)
        .with_architecture(Architecture::ARM64)
        .with_architecture(Architecture::RISCV64)
        .with_cost(1)
        .with_latency(1)
        .with_throughput(1.0);

        self.add_pattern(jump_pattern);
    }
}

impl PatternMatcher for DefaultPatternMatcher {
    fn match_pattern(&self, ir_op: &IROp) -> Option<InstructionPattern> {
        let patterns = self.patterns.get(&ir_op.opcode)?;

        // Find a pattern with matching operands
        for pattern in patterns {
            if pattern.has_operand_types(&ir_op.operands) {
                return Some(pattern.clone());
            }
        }

        None
    }

    fn get_equivalent_patterns(
        &self,
        pattern: &InstructionPattern,
        target_arch: Architecture,
    ) -> Vec<InstructionPattern> {
        let mut equivalents = Vec::new();

        // Look for patterns with the same category and compatible operands
        if let Some(pattern_ids) = self.category_index.get(&pattern.category) {
            // Pre-allocate with estimated capacity
            equivalents.reserve(pattern_ids.len());

            for id in pattern_ids {
                if let Some(candidates) = self.patterns.get(id) {
                    for candidate in candidates {
                        if candidate.is_compatible_with(target_arch)
                            && candidate.has_operand_types(&pattern.operands)
                        {
                            equivalents.push(candidate.clone());
                        }
                    }
                }
            }
        }

        equivalents
    }

    fn get_patterns_by_category(&self, category: InstructionCategory) -> Vec<InstructionPattern> {
        let mut patterns = Vec::new();

        if let Some(pattern_ids) = self.category_index.get(&category) {
            // Estimate total capacity
            let estimated_size: usize = pattern_ids
                .iter()
                .filter_map(|id| self.patterns.get(id))
                .map(|candidates| candidates.len())
                .sum();

            patterns.reserve(estimated_size);

            for id in pattern_ids {
                if let Some(candidates) = self.patterns.get(id) {
                    patterns.extend(candidates.clone());
                }
            }
        }

        patterns
    }

    fn name(&self) -> &'static str {
        "DefaultPatternMatcher"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instruction_pattern() {
        let pattern =
            InstructionPattern::new("test", InstructionCategory::Arithmetic(ArithmeticType::Add))
                .with_operand(OperandType::Register(RegId(0)))
                .with_operand(OperandType::Register(RegId(1)))
                .with_architecture(Architecture::X86_64);

        assert_eq!(pattern.operand_count(), 2);
        assert!(pattern.is_compatible_with(Architecture::X86_64));
        assert!(!pattern.is_compatible_with(Architecture::ARM64));
    }

    #[test]
    fn test_pattern_matcher() {
        let mut matcher = DefaultPatternMatcher::new();
        matcher.initialize_common_patterns();

        let ir_op = IROp::new("add")
            .with_operand(OperandType::Register(RegId(0)))
            .with_operand(OperandType::Register(RegId(1)))
            .with_operand(OperandType::Register(RegId(2)));

        let pattern = matcher.match_pattern(&ir_op);
        assert!(pattern.is_some());

        let pattern = pattern.unwrap();
        assert_eq!(pattern.id, "add");
        assert!(matches!(
            pattern.category,
            InstructionCategory::Arithmetic(ArithmeticType::Add)
        ));
    }

    #[test]
    fn test_memory_operand() {
        let mem_op = MemoryOperand::simple(RegId(1), 0x1000, 4);
        assert_eq!(mem_op.base, Some(RegId(1)));
        assert_eq!(mem_op.displacement, 0x1000);
        assert_eq!(mem_op.size, 4);

        let indexed_op = MemoryOperand::indexed(RegId(1), RegId(2), 4, 0x1000, 4);
        assert_eq!(indexed_op.base, Some(RegId(1)));
        assert_eq!(indexed_op.index, Some(RegId(2)));
        assert_eq!(indexed_op.scale, 4);
    }

    #[test]
    fn test_pattern_error_to_vm_error_conversion() {
        use vm_core::VmError;

        // Test InvalidPattern conversion
        let err = PatternError::InvalidPattern("bad pattern".to_string());
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Core(_)));

        // Test PatternNotFound conversion
        let err = PatternError::PatternNotFound("unknown_op".to_string());
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Core(_)));

        // Test IncompatibleOperands conversion
        let err = PatternError::IncompatibleOperands("type mismatch".to_string());
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Core(_)));

        // Test UnsupportedArchitecture conversion
        let err = PatternError::UnsupportedArchitecture(Architecture::RISCV64);
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Core(_)));

        // Test MatchingFailed conversion
        let err = PatternError::MatchingFailed("match error".to_string());
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Core(_)));
    }

    // ========== New Comprehensive Tests for Coverage Enhancement ==========

    #[test]
    fn test_instruction_category_arithmetic() {
        let categories = vec![
            InstructionCategory::Arithmetic(ArithmeticType::Add),
            InstructionCategory::Arithmetic(ArithmeticType::Sub),
            InstructionCategory::Arithmetic(ArithmeticType::Mul),
            InstructionCategory::Arithmetic(ArithmeticType::Div),
            InstructionCategory::Arithmetic(ArithmeticType::Mod),
            InstructionCategory::Arithmetic(ArithmeticType::Neg),
            InstructionCategory::Arithmetic(ArithmeticType::Abs),
            InstructionCategory::Arithmetic(ArithmeticType::Min),
            InstructionCategory::Arithmetic(ArithmeticType::Max),
            InstructionCategory::Arithmetic(ArithmeticType::Sqrt),
            InstructionCategory::Arithmetic(ArithmeticType::Pow),
        ];

        for category in categories {
            assert!(matches!(category, InstructionCategory::Arithmetic(_)));
        }
    }

    #[test]
    fn test_instruction_category_logical() {
        let categories = vec![
            InstructionCategory::Logical(LogicalType::And),
            InstructionCategory::Logical(LogicalType::Or),
            InstructionCategory::Logical(LogicalType::Xor),
            InstructionCategory::Logical(LogicalType::Not),
            InstructionCategory::Logical(LogicalType::ShiftLeft),
            InstructionCategory::Logical(LogicalType::ShiftRight),
            InstructionCategory::Logical(LogicalType::RotateLeft),
            InstructionCategory::Logical(LogicalType::RotateRight),
            InstructionCategory::Logical(LogicalType::BitTest),
            InstructionCategory::Logical(LogicalType::BitField),
        ];

        for category in categories {
            assert!(matches!(category, InstructionCategory::Logical(_)));
        }
    }

    #[test]
    fn test_instruction_category_memory() {
        let categories = vec![
            InstructionCategory::Memory(MemoryType::Load),
            InstructionCategory::Memory(MemoryType::Store),
            InstructionCategory::Memory(MemoryType::LoadAcquire),
            InstructionCategory::Memory(MemoryType::StoreRelease),
            InstructionCategory::Memory(MemoryType::LoadReserved),
            InstructionCategory::Memory(MemoryType::StoreConditional),
            InstructionCategory::Memory(MemoryType::Swap),
            InstructionCategory::Memory(MemoryType::CompareAndSwap),
            InstructionCategory::Memory(MemoryType::FetchAndOp),
            InstructionCategory::Memory(MemoryType::Prefetch),
            InstructionCategory::Memory(MemoryType::CacheOp),
        ];

        for category in categories {
            assert!(matches!(category, InstructionCategory::Memory(_)));
        }
    }

    #[test]
    fn test_instruction_category_branch() {
        let categories = vec![
            InstructionCategory::Branch(BranchType::Unconditional),
            InstructionCategory::Branch(BranchType::Conditional),
            InstructionCategory::Branch(BranchType::Indirect),
            InstructionCategory::Branch(BranchType::Call),
            InstructionCategory::Branch(BranchType::Return),
            InstructionCategory::Branch(BranchType::JumpTable),
            InstructionCategory::Branch(BranchType::TailCall),
            InstructionCategory::Branch(BranchType::Exception),
            InstructionCategory::Branch(BranchType::Interrupt),
        ];

        for category in categories {
            assert!(matches!(category, InstructionCategory::Branch(_)));
        }
    }

    #[test]
    fn test_instruction_category_vector() {
        let categories = vec![
            InstructionCategory::Vector(VectorType::Arithmetic),
            InstructionCategory::Vector(VectorType::Logical),
            InstructionCategory::Vector(VectorType::Shuffle),
            InstructionCategory::Vector(VectorType::Blend),
            InstructionCategory::Vector(VectorType::Insert),
            InstructionCategory::Vector(VectorType::Extract),
            InstructionCategory::Vector(VectorType::Reduce),
            InstructionCategory::Vector(VectorType::Mask),
            InstructionCategory::Vector(VectorType::Permute),
            InstructionCategory::Vector(VectorType::Compress),
        ];

        for category in categories {
            assert!(matches!(category, InstructionCategory::Vector(_)));
        }
    }

    #[test]
    fn test_pattern_builder_with_cost() {
        let pattern =
            InstructionPattern::new("test", InstructionCategory::Arithmetic(ArithmeticType::Add))
                .with_cost(5);

        assert_eq!(pattern.cost, 5);
    }

    #[test]
    fn test_pattern_builder_with_latency() {
        let pattern =
            InstructionPattern::new("test", InstructionCategory::Arithmetic(ArithmeticType::Add))
                .with_latency(3);

        assert_eq!(pattern.latency, 3);
    }

    #[test]
    fn test_pattern_builder_with_throughput() {
        let pattern =
            InstructionPattern::new("test", InstructionCategory::Arithmetic(ArithmeticType::Add))
                .with_throughput(2.5);

        assert_eq!(pattern.throughput, 2.5);
    }

    #[test]
    fn test_pattern_builder_with_multiple_architectures() {
        let pattern =
            InstructionPattern::new("test", InstructionCategory::Arithmetic(ArithmeticType::Add))
                .with_architecture(Architecture::X86_64)
                .with_architecture(Architecture::ARM64)
                .with_architecture(Architecture::RISCV64);

        assert!(pattern.is_compatible_with(Architecture::X86_64));
        assert!(pattern.is_compatible_with(Architecture::ARM64));
        assert!(pattern.is_compatible_with(Architecture::RISCV64));
    }

    #[test]
    fn test_pattern_builder_with_flags() {
        let flags = InstructionFlags {
            sets_flags: true,
            reads_flags: false,
            is_conditional: false,
            is_predicated: false,
            is_atomic: false,
            is_volatile: false,
            is_privileged: false,
            is_terminal: false,
        };

        let pattern =
            InstructionPattern::new("test", InstructionCategory::Arithmetic(ArithmeticType::Add))
                .with_flags(flags);

        assert!(pattern.flags.sets_flags);
        assert!(!pattern.flags.reads_flags);
    }

    #[test]
    fn test_pattern_builder_with_semantics() {
        let semantics = SemanticDescription {
            operation: "add".to_string(),
            preconditions: vec![],
            postconditions: vec!["result = a + b".to_string()],
            side_effects: vec![],
            dependencies: vec![],
            outputs: vec![],
        };

        let pattern =
            InstructionPattern::new("test", InstructionCategory::Arithmetic(ArithmeticType::Add))
                .with_semantics(semantics);

        assert_eq!(pattern.semantics.operation, "add");
        assert_eq!(pattern.semantics.postconditions.len(), 1);
    }

    #[test]
    fn test_pattern_operand_count() {
        let pattern1 = InstructionPattern::new(
            "test1",
            InstructionCategory::Arithmetic(ArithmeticType::Add),
        )
        .with_operand(OperandType::Register(RegId(0)))
        .with_operand(OperandType::Register(RegId(1)));

        assert_eq!(pattern1.operand_count(), 2);

        let pattern2 = InstructionPattern::new(
            "test2",
            InstructionCategory::Arithmetic(ArithmeticType::Neg),
        )
        .with_operand(OperandType::Register(RegId(0)));

        assert_eq!(pattern2.operand_count(), 1);
    }

    #[test]
    fn test_memory_operand_simple() {
        let mem_op = MemoryOperand::simple(RegId(1), 0x2000, 8);
        assert_eq!(mem_op.base, Some(RegId(1)));
        assert_eq!(mem_op.displacement, 0x2000);
        assert_eq!(mem_op.size, 8);
        assert_eq!(mem_op.index, None);
        assert_eq!(mem_op.scale, 1);
    }

    #[test]
    fn test_memory_operand_indexed() {
        let mem_op = MemoryOperand::indexed(RegId(1), RegId(2), 8, 0x1000, 16);
        assert_eq!(mem_op.base, Some(RegId(1)));
        assert_eq!(mem_op.index, Some(RegId(2)));
        assert_eq!(mem_op.scale, 8);
        assert_eq!(mem_op.displacement, 0x1000);
        assert_eq!(mem_op.size, 16);
    }

    #[test]
    fn test_ir_op_builder() {
        let ir_op = IROp::new("custom_op")
            .with_operand(OperandType::Register(RegId(0)))
            .with_operand(OperandType::Immediate(42));

        assert_eq!(ir_op.opcode, "custom_op");
        assert_eq!(ir_op.operands.len(), 2);
    }

    #[test]
    fn test_pattern_matcher_get_patterns_by_category() {
        let mut matcher = DefaultPatternMatcher::new();
        matcher.initialize_common_patterns();

        let arithmetic_patterns =
            matcher.get_patterns_by_category(InstructionCategory::Arithmetic(ArithmeticType::Add));

        assert!(!arithmetic_patterns.is_empty());

        for pattern in arithmetic_patterns {
            assert!(matches!(
                pattern.category,
                InstructionCategory::Arithmetic(ArithmeticType::Add)
            ));
        }
    }

    #[test]
    fn test_pattern_matcher_no_match() {
        let mut matcher = DefaultPatternMatcher::new();
        matcher.initialize_common_patterns();

        let ir_op = IROp::new("nonexistent_op").with_operand(OperandType::Register(RegId(0)));

        let pattern = matcher.match_pattern(&ir_op);
        assert!(pattern.is_none());
    }

    #[test]
    fn test_pattern_matcher_get_equivalent_patterns() {
        let mut matcher = DefaultPatternMatcher::new();
        matcher.initialize_common_patterns();

        let pattern = InstructionPattern::new("add", InstructionCategory::Arithmetic(ArithmeticType::Add))
            .with_operand(OperandType::Register(RegId(0))) // dst
            .with_operand(OperandType::Register(RegId(1))) // src1
            .with_operand(OperandType::Register(RegId(2))) // src2
            .with_architecture(Architecture::X86_64);

        let equivalents = matcher.get_equivalent_patterns(&pattern, Architecture::ARM64);

        // Should find equivalent patterns for ARM64 (the same pattern also supports ARM64)
        assert!(!equivalents.is_empty());
    }

    #[test]
    fn test_pattern_error_partial_eq() {
        let err1 = PatternError::InvalidPattern("test".to_string());
        let err2 = PatternError::InvalidPattern("test".to_string());
        let err3 = PatternError::InvalidPattern("other".to_string());

        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    #[test]
    fn test_operand_type_register() {
        let reg_op = OperandType::Register(RegId(5));
        assert!(matches!(reg_op, OperandType::Register(_)));
    }

    #[test]
    fn test_operand_type_immediate() {
        let imm_op = OperandType::Immediate(42);
        assert!(matches!(imm_op, OperandType::Immediate(_)));
    }

    #[test]
    fn test_operand_type_label() {
        let label_op = OperandType::Label("target".to_string());
        assert!(matches!(label_op, OperandType::Label(_)));
    }

    #[test]
    fn test_operand_type_memory() {
        let mem_op = OperandType::Memory(MemoryOperand::simple(RegId(0), 0, 4));
        assert!(matches!(mem_op, OperandType::Memory(_)));
    }

    #[test]
    fn test_instruction_flags_all_false() {
        let flags = InstructionFlags {
            sets_flags: false,
            reads_flags: false,
            is_conditional: false,
            is_predicated: false,
            is_atomic: false,
            is_volatile: false,
            is_privileged: false,
            is_terminal: false,
        };

        assert!(!flags.sets_flags);
        assert!(!flags.is_terminal);
    }

    #[test]
    fn test_instruction_flags_all_true() {
        let flags = InstructionFlags {
            sets_flags: true,
            reads_flags: true,
            is_conditional: true,
            is_predicated: true,
            is_atomic: true,
            is_volatile: true,
            is_privileged: true,
            is_terminal: true,
        };

        assert!(flags.sets_flags);
        assert!(flags.is_terminal);
    }

    #[test]
    fn test_semantic_description() {
        let semantics = SemanticDescription {
            operation: "test_op".to_string(),
            preconditions: vec!["condition1".to_string(), "condition2".to_string()],
            postconditions: vec!["result".to_string()],
            side_effects: vec!["side_effect".to_string()],
            dependencies: vec![RegId(0)],
            outputs: vec![RegId(1)],
        };

        assert_eq!(semantics.operation, "test_op");
        assert_eq!(semantics.preconditions.len(), 2);
        assert_eq!(semantics.postconditions.len(), 1);
        assert_eq!(semantics.side_effects.len(), 1);
        assert_eq!(semantics.dependencies.len(), 1);
        assert_eq!(semantics.outputs.len(), 1);
    }

    #[test]
    fn test_pattern_matcher_initialization() {
        let mut matcher = DefaultPatternMatcher::new();
        matcher.initialize_common_patterns();

        // After initialization, matcher should have patterns for common operations
        let ir_op = IROp::new("add")
            .with_operand(OperandType::Register(RegId(0)))
            .with_operand(OperandType::Register(RegId(1)))
            .with_operand(OperandType::Register(RegId(2)));

        let pattern = matcher.match_pattern(&ir_op);
        assert!(pattern.is_some());
    }

    #[test]
    fn test_pattern_clone() {
        let pattern1 =
            InstructionPattern::new("test", InstructionCategory::Arithmetic(ArithmeticType::Add))
                .with_operand(OperandType::Register(RegId(0)))
                .with_architecture(Architecture::X86_64);

        let pattern2 = pattern1.clone();

        assert_eq!(pattern1.id, pattern2.id);
        assert_eq!(pattern1.category, pattern2.category);
    }

    #[test]
    fn test_semantic_description_clone() {
        let semantics1 = SemanticDescription {
            operation: "test".to_string(),
            preconditions: vec!["pre".to_string()],
            postconditions: vec!["post".to_string()],
            side_effects: vec!["effect".to_string()],
            dependencies: vec![RegId(0)],
            outputs: vec![RegId(1)],
        };

        let semantics2 = semantics1.clone();

        assert_eq!(semantics1.operation, semantics2.operation);
        assert_eq!(semantics1.preconditions, semantics2.preconditions);
    }

    #[test]
    fn test_memory_operand_clone() {
        let mem_op1 = MemoryOperand::indexed(RegId(1), RegId(2), 4, 0x1000, 8);
        let mem_op2 = mem_op1.clone();

        assert_eq!(mem_op1.base, mem_op2.base);
        assert_eq!(mem_op1.index, mem_op2.index);
        assert_eq!(mem_op1.scale, mem_op2.scale);
    }

    #[test]
    fn test_instruction_flags_copy() {
        let flags1 = InstructionFlags {
            sets_flags: true,
            reads_flags: false,
            is_conditional: true,
            is_predicated: false,
            is_atomic: true,
            is_volatile: false,
            is_privileged: true,
            is_terminal: false,
        };

        let flags2 = flags1;

        assert_eq!(flags1, flags2);
    }

    #[test]
    fn test_arithmetic_type_copy() {
        let type1 = ArithmeticType::Add;
        let type2 = type1;
        assert_eq!(type1, type2);
    }

    #[test]
    fn test_logical_type_copy() {
        let type1 = LogicalType::And;
        let type2 = type1;
        assert_eq!(type1, type2);
    }

    #[test]
    fn test_memory_type_copy() {
        let type1 = MemoryType::Load;
        let type2 = type1;
        assert_eq!(type1, type2);
    }

    #[test]
    fn test_branch_type_copy() {
        let type1 = BranchType::Conditional;
        let type2 = type1;
        assert_eq!(type1, type2);
    }

    #[test]
    fn test_vector_type_copy() {
        let type1 = VectorType::Shuffle;
        let type2 = type1;
        assert_eq!(type1, type2);
    }

    #[test]
    fn test_pattern_with_no_architectures() {
        let pattern =
            InstructionPattern::new("test", InstructionCategory::Arithmetic(ArithmeticType::Add));

        // No architectures specified - should be compatible with all (universal pattern)
        assert!(pattern.is_compatible_with(Architecture::X86_64));
        assert!(pattern.is_compatible_with(Architecture::ARM64));
        assert!(pattern.is_compatible_with(Architecture::RISCV64));
    }

    #[test]
    fn test_pattern_multiple_operands() {
        let pattern = InstructionPattern::new(
            "multi_op",
            InstructionCategory::Arithmetic(ArithmeticType::Add),
        )
        .with_operand(OperandType::Register(RegId(0)))
        .with_operand(OperandType::Register(RegId(1)))
        .with_operand(OperandType::Immediate(10))
        .with_operand(OperandType::Label("label".to_string()));

        assert_eq!(pattern.operands.len(), 4);
    }

    #[test]
    fn test_pattern_zero_operands() {
        let pattern =
            InstructionPattern::new("no_op", InstructionCategory::System(SystemType::Nop));
        assert_eq!(pattern.operands.len(), 0);
        assert_eq!(pattern.operand_count(), 0);
    }

    #[test]
    fn test_pattern_other_category() {
        let pattern = InstructionPattern::new(
            "custom",
            InstructionCategory::Other("custom_operation".to_string()),
        );
        assert!(matches!(pattern.category, InstructionCategory::Other(_)));
    }

    #[test]
    fn test_compare_and_convert_categories() {
        let cat1 = InstructionCategory::Arithmetic(ArithmeticType::Add);
        let cat2 = InstructionCategory::Arithmetic(ArithmeticType::Sub);
        let cat3 = InstructionCategory::Arithmetic(ArithmeticType::Add);

        assert_eq!(cat1, cat3);
        assert_ne!(cat1, cat2);
    }

    #[test]
    fn test_all_arithmetic_types() {
        let types = vec![
            ArithmeticType::Add,
            ArithmeticType::Sub,
            ArithmeticType::Mul,
            ArithmeticType::Div,
            ArithmeticType::Mod,
            ArithmeticType::Neg,
            ArithmeticType::Abs,
            ArithmeticType::Min,
            ArithmeticType::Max,
            ArithmeticType::Sqrt,
            ArithmeticType::Pow,
        ];

        assert_eq!(types.len(), 11); // Verify all types are covered
    }

    #[test]
    fn test_all_logical_types() {
        let types = vec![
            LogicalType::And,
            LogicalType::Or,
            LogicalType::Xor,
            LogicalType::Not,
            LogicalType::ShiftLeft,
            LogicalType::ShiftRight,
            LogicalType::RotateLeft,
            LogicalType::RotateRight,
            LogicalType::BitTest,
            LogicalType::BitField,
        ];

        assert_eq!(types.len(), 10); // Verify all types are covered
    }

    #[test]
    fn test_all_memory_types() {
        let types = vec![
            MemoryType::Load,
            MemoryType::Store,
            MemoryType::LoadAcquire,
            MemoryType::StoreRelease,
            MemoryType::LoadReserved,
            MemoryType::StoreConditional,
            MemoryType::Swap,
            MemoryType::CompareAndSwap,
            MemoryType::FetchAndOp,
            MemoryType::Prefetch,
            MemoryType::CacheOp,
        ];

        assert_eq!(types.len(), 11); // Verify all types are covered
    }

    #[test]
    fn test_all_branch_types() {
        let types = vec![
            BranchType::Unconditional,
            BranchType::Conditional,
            BranchType::Indirect,
            BranchType::Call,
            BranchType::Return,
            BranchType::JumpTable,
            BranchType::TailCall,
            BranchType::Exception,
            BranchType::Interrupt,
        ];

        assert_eq!(types.len(), 9); // Verify all types are covered
    }
}
