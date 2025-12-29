//! Common instruction pattern recognition for VM cross-architecture translation
//!
//! This module provides unified instruction pattern matching and semantic analysis
//! across different architectures, enabling better cross-architecture translation.

use crate::encoding::{Architecture, RegId};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

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
        let add_pattern =
            InstructionPattern::new("add", InstructionCategory::Arithmetic(ArithmeticType::Add))
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
        let sub_pattern =
            InstructionPattern::new("sub", InstructionCategory::Arithmetic(ArithmeticType::Sub))
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
        let and_pattern =
            InstructionPattern::new("and", InstructionCategory::Logical(LogicalType::And))
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
        let or_pattern =
            InstructionPattern::new("or", InstructionCategory::Logical(LogicalType::Or))
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
        let load_pattern =
            InstructionPattern::new("load", InstructionCategory::Memory(MemoryType::Load))
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
        let store_pattern =
            InstructionPattern::new("store", InstructionCategory::Memory(MemoryType::Store))
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
}
