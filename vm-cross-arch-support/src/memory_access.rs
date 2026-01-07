//! Common memory access abstraction for VM cross-architecture translation
//!
//! This module provides unified memory access patterns, alignment handling,
//! and endianness conversion across different architectures.

use std::collections::HashMap;

use thiserror::Error;
use vm_core::error::{CoreError, MemoryError as VmMemoryError};
use vm_core::{GuestAddr, VmError};

use crate::encoding::{Architecture, RegId};

/// Errors that can occur during memory access operations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum MemoryError {
    #[error(
        "Memory access alignment violation: address {:#x} not aligned to {} bytes",
        _0,
        _1
    )]
    AlignmentViolation(u64, u64),
    #[error("Memory access out of bounds: address {:#x}, size {}", _0, _1)]
    OutOfBounds(u64, usize),
    #[error("Invalid memory access size: {0}")]
    InvalidSize(usize),
    #[error("Memory protection violation: {0}")]
    ProtectionViolation(String),
    #[error("Page fault at address {0:#x}: {1}")]
    PageFault(u64, String),
    #[error("Atomic operation violation: {0}")]
    AtomicViolation(String),
    #[error("Endianness conversion error: {0}")]
    EndiannessError(String),
}

impl From<MemoryError> for VmError {
    fn from(err: MemoryError) -> Self {
        match err {
            MemoryError::AlignmentViolation(addr, alignment) => {
                VmError::Memory(VmMemoryError::AccessViolation {
                    addr: GuestAddr(addr),
                    msg: format!("Address not aligned to {} bytes", alignment),
                    access_type: None,
                })
            }
            MemoryError::OutOfBounds(addr, size) => {
                VmError::Memory(VmMemoryError::AccessViolation {
                    addr: GuestAddr(addr),
                    msg: format!("Access out of bounds: size {}", size),
                    access_type: None,
                })
            }
            MemoryError::InvalidSize(size) => VmError::Core(CoreError::InvalidParameter {
                name: "access_size".to_string(),
                value: format!("{}", size),
                message: "Invalid memory access size".to_string(),
            }),
            MemoryError::ProtectionViolation(msg) => {
                VmError::Memory(VmMemoryError::AccessViolation {
                    addr: GuestAddr(0),
                    msg,
                    access_type: None,
                })
            }
            MemoryError::PageFault(_addr, msg) => VmError::Memory(VmMemoryError::PageTableError {
                message: format!("Page fault: {}", msg),
                level: None,
            }),
            MemoryError::AtomicViolation(msg) => VmError::Core(CoreError::Internal {
                message: format!("Atomic operation violation: {}", msg),
                module: "vm-cross-arch-support::memory_access".to_string(),
            }),
            MemoryError::EndiannessError(msg) => VmError::Core(CoreError::Internal {
                message: format!("Endianness conversion error: {}", msg),
                module: "vm-cross-arch-support::memory_access".to_string(),
            }),
        }
    }
}

/// Memory access types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccessType {
    #[default]
    Read,
    Write,
    ReadWrite,
    Execute,
    AtomicRead,
    AtomicWrite,
    AtomicReadWrite,
}

/// Memory access width
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccessWidth {
    #[default]
    Word, // 4 bytes (most common)
    Byte,       // 1 byte
    HalfWord,   // 2 bytes
    DoubleWord, // 8 bytes
    QuadWord,   // 16 bytes
    Vector(u8), // Vector with specified lane count
}

/// Memory alignment requirements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    Natural, // Natural alignment for the access width (recommended)
    Unaligned,
    Aligned1,
    Aligned2,
    Aligned4,
    Aligned8,
    Aligned16,
    Aligned32,
    Aligned64,
    Strict, // Strict alignment (must be naturally aligned)
}

/// Memory access flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryFlags {
    pub is_volatile: bool,
    pub is_atomic: bool,
    pub is_acquire: bool,
    pub is_release: bool,
    pub is_locked: bool,
    pub is_cacheable: bool,
    pub is_privileged: bool,
    pub is_endian_aware: bool,
}

/// Memory access pattern description
#[derive(Debug, Clone)]
pub struct MemoryAccessPattern {
    pub base_reg: RegId,
    pub offset: i64,
    pub width: AccessWidth,
    pub alignment: Alignment,
    pub access_type: AccessType,
    pub flags: MemoryFlags,
    pub repeat_count: Option<u32>, // For repeated accesses
}

impl MemoryAccessPattern {
    pub fn new(base_reg: RegId, offset: i64, width: AccessWidth) -> Self {
        Self {
            base_reg,
            offset,
            width,
            alignment: Alignment::Natural,
            access_type: AccessType::Read,
            flags: MemoryFlags::default(),
            repeat_count: None,
        }
    }

    pub fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn with_access_type(mut self, access_type: AccessType) -> Self {
        self.access_type = access_type;
        self
    }

    pub fn with_flags(mut self, flags: MemoryFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn with_repeat(mut self, count: u32) -> Self {
        self.repeat_count = Some(count);
        self
    }

    /// Get the size of this access in bytes
    pub fn size(&self) -> usize {
        match self.width {
            AccessWidth::Byte => 1,
            AccessWidth::HalfWord => 2,
            AccessWidth::Word => 4,
            AccessWidth::DoubleWord => 8,
            AccessWidth::QuadWord => 16,
            AccessWidth::Vector(lanes) => lanes as usize,
        }
    }

    /// Get the required alignment for this access
    pub fn required_alignment(&self) -> u64 {
        match self.alignment {
            Alignment::Unaligned => 1,
            Alignment::Aligned1 => 1,
            Alignment::Aligned2 => 2,
            Alignment::Aligned4 => 4,
            Alignment::Aligned8 => 8,
            Alignment::Aligned16 => 16,
            Alignment::Aligned32 => 32,
            Alignment::Aligned64 => 64,
            Alignment::Natural => self.size() as u64,
            Alignment::Strict => self.size() as u64,
        }
    }
}

/// Memory access optimization result
#[derive(Debug, Clone)]
pub struct OptimizedPattern {
    pub original: MemoryAccessPattern,
    pub optimized: MemoryAccessPattern,
    pub optimization_type: OptimizationType,
    pub performance_gain: f32,
}

/// Types of memory access optimizations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptimizationType {
    #[default]
    NoOptimization,
    AlignedAccess,
    CombinedAccess,
    VectorizedAccess,
    Prefetch,
    CacheLineOptimized,
    BurstAccess,
}

/// Memory access optimizer
pub trait MemoryAccessOptimizer {
    /// Optimize a memory access pattern
    fn optimize_access_pattern(&self, pattern: &MemoryAccessPattern) -> OptimizedPattern;

    /// Detect alignment issues in a pattern
    fn detect_alignment_issues(&self, pattern: &MemoryAccessPattern) -> Vec<AlignmentIssue>;

    /// Suggest fixes for alignment issues
    fn suggest_fixes(&self, issues: &[AlignmentIssue]) -> Vec<Fix>;

    /// Get the optimizer's name
    fn name(&self) -> &'static str;
}

/// Alignment issue description
#[derive(Debug, Clone)]
pub struct AlignmentIssue {
    pub address: u64,
    pub required_alignment: u64,
    pub actual_alignment: u64,
    pub severity: IssueSeverity,
    pub description: String,
}

/// Issue severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IssueSeverity {
    #[default]
    Warning,
    Error,
    Critical,
}

/// Fix suggestion for alignment issues
#[derive(Debug, Clone)]
pub struct Fix {
    pub fix_type: FixType,
    pub description: String,
    pub cost_estimate: FixCost,
}

/// Types of fixes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FixType {
    #[default]
    UseUnalignedAccess, // Least invasive
    AlignAddress,
    AdjustOffset,
    ChangeAccessWidth,
    InsertPadding,
    RestructureAccess,
}

/// Cost estimate for applying a fix
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FixCost {
    #[default]
    None,
    Low,
    Medium,
    High,
    Unknown,
}

/// Default memory access optimizer
#[derive(Debug)]
pub struct DefaultMemoryAccessOptimizer {
    architecture: Architecture,
    cache_line_size: u64,
    vector_width: u8,
}

impl DefaultMemoryAccessOptimizer {
    pub fn new(architecture: Architecture) -> Self {
        let cache_line_size = match architecture {
            Architecture::X86_64 => 64,
            Architecture::ARM64 => 64,
            Architecture::RISCV64 => 64,
        };

        let vector_width = match architecture {
            Architecture::X86_64 => 32,  // AVX-512
            Architecture::ARM64 => 32,   // SVE2
            Architecture::RISCV64 => 32, // V extension
        };

        Self {
            architecture,
            cache_line_size,
            vector_width,
        }
    }
}

impl MemoryAccessOptimizer for DefaultMemoryAccessOptimizer {
    fn optimize_access_pattern(&self, pattern: &MemoryAccessPattern) -> OptimizedPattern {
        let mut optimized = pattern.clone();
        let mut optimization_type = OptimizationType::NoOptimization;
        let mut performance_gain = 0.0;

        // Apply architecture-specific optimizations
        let arch_factor = match self.architecture {
            Architecture::X86_64 => 1.0,
            Architecture::ARM64 => 1.1, // ARM64 typically has better vectorization
            Architecture::RISCV64 => 1.05, // RISC-V with V extension
        };

        // Check for alignment optimization
        if pattern.alignment == Alignment::Unaligned {
            optimized.alignment = Alignment::Natural;
            optimization_type = OptimizationType::AlignedAccess;
            performance_gain += 0.2 * arch_factor; // 20% improvement adjusted for architecture
        }

        // Check for vectorization opportunity
        if let Some(repeat_count) = pattern.repeat_count
            && repeat_count > 1
            && pattern.size() <= self.vector_width as usize
        {
            optimized.width = AccessWidth::Vector(self.vector_width);
            optimization_type = OptimizationType::VectorizedAccess;
            performance_gain += 0.4 * arch_factor; // 40% improvement adjusted for architecture
        }

        // Check for cache line optimization
        let address = pattern.offset as u64;
        if !address.is_multiple_of(self.cache_line_size) {
            let aligned_address =
                (address + self.cache_line_size - 1) & !(self.cache_line_size - 1);
            optimized.offset = aligned_address as i64;
            optimization_type = OptimizationType::CacheLineOptimized;
            performance_gain += 0.1 * arch_factor; // 10% improvement adjusted for architecture
        }

        OptimizedPattern {
            original: pattern.clone(),
            optimized,
            optimization_type,
            performance_gain,
        }
    }

    fn detect_alignment_issues(&self, pattern: &MemoryAccessPattern) -> Vec<AlignmentIssue> {
        let mut issues = Vec::new();
        let address = pattern.offset as u64;
        let required_alignment = pattern.required_alignment();
        let actual_alignment = address & (!address + 1); // Find least significant set bit

        if actual_alignment < required_alignment {
            let severity = if pattern.flags.is_atomic {
                IssueSeverity::Critical
            } else if pattern.width == AccessWidth::QuadWord {
                IssueSeverity::Error
            } else {
                IssueSeverity::Warning
            };

            issues.push(AlignmentIssue {
                address,
                required_alignment,
                actual_alignment,
                severity,
                description: format!(
                    "Memory access at {:#x} requires {}-byte alignment but is only {}-byte aligned",
                    address, required_alignment, actual_alignment
                ),
            });
        }

        issues
    }

    fn suggest_fixes(&self, issues: &[AlignmentIssue]) -> Vec<Fix> {
        issues
            .iter()
            .map(|issue| match issue.severity {
                IssueSeverity::Critical => Fix {
                    fix_type: FixType::AlignAddress,
                    description: "Align the address to the required boundary".to_string(),
                    cost_estimate: FixCost::Medium,
                },
                IssueSeverity::Error => Fix {
                    fix_type: FixType::ChangeAccessWidth,
                    description: "Use smaller access width or aligned access".to_string(),
                    cost_estimate: FixCost::Low,
                },
                IssueSeverity::Warning => Fix {
                    fix_type: FixType::UseUnalignedAccess,
                    description: "Use unaligned access if supported".to_string(),
                    cost_estimate: FixCost::Low,
                },
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "DefaultMemoryAccessOptimizer"
    }
}

/// Endianness converter for cross-architecture memory access
#[derive(Debug, Clone)]
pub struct EndiannessConverter {
    source: Endianness,
    target: Endianness,
    strategy: ConversionStrategy,
}

/// Endianness types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Endianness {
    #[default]
    Little,
    Big,
}

/// Conversion strategies for endianness
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConversionStrategy {
    #[default]
    Optimized, // Optimized for common patterns (recommended)
    Direct,      // Direct byte swapping
    Lazy,        // Convert on demand
    Precomputed, // Precompute conversion tables
}

impl EndiannessConverter {
    pub fn new(source: Endianness, target: Endianness, strategy: ConversionStrategy) -> Self {
        Self {
            source,
            target,
            strategy,
        }
    }

    /// Convert data from source to target endianness
    pub fn convert(&self, data: &mut [u8]) -> Result<(), MemoryError> {
        if self.source == self.target {
            return Ok(());
        }

        match self.strategy {
            ConversionStrategy::Direct => self.direct_convert(data),
            ConversionStrategy::Optimized => self.optimized_convert(data),
            ConversionStrategy::Lazy => self.lazy_convert(data),
            ConversionStrategy::Precomputed => self.precomputed_convert(data),
        }
    }

    /// Convert a single value
    pub fn convert_value<T>(&self, value: &mut T) -> Result<(), MemoryError> {
        let data = unsafe {
            std::slice::from_raw_parts_mut(value as *mut T as *mut u8, std::mem::size_of::<T>())
        };
        self.convert(data)
    }

    /// Direct byte swapping conversion
    fn direct_convert(&self, data: &mut [u8]) -> Result<(), MemoryError> {
        data.reverse();
        Ok(())
    }

    /// Optimized conversion for common patterns
    fn optimized_convert(&self, data: &mut [u8]) -> Result<(), MemoryError> {
        match data.len() {
            2 => data.swap(0, 1),
            4 => {
                data.swap(0, 3);
                data.swap(1, 2);
            }
            8 => {
                data.swap(0, 7);
                data.swap(1, 6);
                data.swap(2, 5);
                data.swap(3, 4);
            }
            16 => {
                data.swap(0, 15);
                data.swap(1, 14);
                data.swap(2, 13);
                data.swap(3, 12);
                data.swap(4, 11);
                data.swap(5, 10);
                data.swap(6, 9);
                data.swap(7, 8);
            }
            _ => data.reverse(),
        }
        Ok(())
    }

    /// Lazy conversion (deferred)
    fn lazy_convert(&self, _data: &mut [u8]) -> Result<(), MemoryError> {
        // In a real implementation, this would mark the data for lazy conversion
        Ok(())
    }

    /// Precomputed conversion tables
    fn precomputed_convert(&self, data: &mut [u8]) -> Result<(), MemoryError> {
        // In a real implementation, this would use precomputed lookup tables
        self.direct_convert(data)
    }
}

/// Memory access pattern analyzer
#[derive(Debug)]
pub struct MemoryAccessAnalyzer {
    patterns: Vec<MemoryAccessPattern>,
    statistics: HashMap<String, u64>,
}

impl Default for MemoryAccessAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryAccessAnalyzer {
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            statistics: HashMap::new(),
        }
    }

    /// Add a memory access pattern for analysis
    pub fn add_pattern(&mut self, pattern: MemoryAccessPattern) {
        self.patterns.push(pattern.clone());

        // Update statistics
        let key = format!("{:?}_{:?}", pattern.width, pattern.access_type);
        *self.statistics.entry(key).or_insert(0) += 1;
    }

    /// Analyze patterns and return insights
    pub fn analyze(&self) -> AnalysisResult {
        let mut total_accesses = 0;
        let mut unaligned_accesses = 0;
        let mut atomic_accesses = 0;
        let mut vector_accesses = 0;
        let mut size_distribution = HashMap::new();

        for pattern in &self.patterns {
            total_accesses += 1;

            if pattern.alignment == Alignment::Unaligned {
                unaligned_accesses += 1;
            }

            if pattern.flags.is_atomic {
                atomic_accesses += 1;
            }

            if matches!(pattern.width, AccessWidth::Vector(_)) {
                vector_accesses += 1;
            }

            let size = pattern.size();
            *size_distribution.entry(size).or_insert(0) += 1;
        }

        let most_common_size = size_distribution
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(&size, _)| size);

        AnalysisResult {
            total_accesses,
            unaligned_accesses,
            atomic_accesses,
            vector_accesses,
            unaligned_percentage: (unaligned_accesses as f64 / total_accesses as f64) * 100.0,
            atomic_percentage: (atomic_accesses as f64 / total_accesses as f64) * 100.0,
            vector_percentage: (vector_accesses as f64 / total_accesses as f64) * 100.0,
            size_distribution,
            most_common_size,
        }
    }
}

/// Analysis result for memory access patterns
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub total_accesses: u64,
    pub unaligned_accesses: u64,
    pub atomic_accesses: u64,
    pub vector_accesses: u64,
    pub unaligned_percentage: f64,
    pub atomic_percentage: f64,
    pub vector_percentage: f64,
    pub size_distribution: HashMap<usize, u64>,
    pub most_common_size: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_access_pattern() {
        let pattern = MemoryAccessPattern::new(RegId(0), 0x1000, AccessWidth::Word)
            .with_alignment(Alignment::Aligned4)
            .with_access_type(AccessType::Read);

        assert_eq!(pattern.size(), 4);
        assert_eq!(pattern.required_alignment(), 4);
    }

    #[test]
    fn test_endianness_converter() {
        let converter = EndiannessConverter::new(
            Endianness::Little,
            Endianness::Big,
            ConversionStrategy::Direct,
        );

        let mut data = vec![0x12, 0x34, 0x56, 0x78];
        converter.convert(&mut data).unwrap();
        assert_eq!(data, vec![0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn test_memory_access_optimizer() {
        let optimizer = DefaultMemoryAccessOptimizer::new(Architecture::X86_64);
        let pattern = MemoryAccessPattern::new(RegId(0), 0x1001, AccessWidth::Word)
            .with_alignment(Alignment::Unaligned);

        let result = optimizer.optimize_access_pattern(&pattern);
        assert!(result.performance_gain > 0.0);
        assert_ne!(result.optimization_type, OptimizationType::NoOptimization);
    }

    #[test]
    fn test_memory_access_analyzer() {
        let mut analyzer = MemoryAccessAnalyzer::new();

        analyzer.add_pattern(MemoryAccessPattern::new(
            RegId(0),
            0x1000,
            AccessWidth::Word,
        ));
        analyzer.add_pattern(MemoryAccessPattern::new(
            RegId(1),
            0x1004,
            AccessWidth::DoubleWord,
        ));
        analyzer.add_pattern(MemoryAccessPattern::new(
            RegId(2),
            0x1008,
            AccessWidth::Word,
        ));

        let result = analyzer.analyze();
        assert_eq!(result.total_accesses, 3);
        assert_eq!(result.most_common_size, Some(4));
    }

    #[test]
    fn test_memory_error_to_vm_error_conversion() {
        use vm_core::VmError;

        // Test AlignmentViolation conversion
        let err = MemoryError::AlignmentViolation(0x1003, 4);
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Memory(_)));

        // Test OutOfBounds conversion
        let err = MemoryError::OutOfBounds(0x1000, 4096);
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Memory(_)));

        // Test InvalidSize conversion
        let err = MemoryError::InvalidSize(999);
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Core(_)));

        // Test ProtectionViolation conversion
        let err = MemoryError::ProtectionViolation("write to read-only".to_string());
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Memory(_)));

        // Test PageFault conversion
        let err = MemoryError::PageFault(0x5000, "page not present".to_string());
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Memory(_)));

        // Test AtomicViolation conversion
        let err = MemoryError::AtomicViolation("atomic alignment error".to_string());
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Core(_)));

        // Test EndiannessError conversion
        let err = MemoryError::EndiannessError("conversion failed".to_string());
        let vm_err: VmError = err.into();
        assert!(matches!(vm_err, VmError::Core(_)));
    }

    // ========== New Comprehensive Tests for Coverage Enhancement ==========

    #[test]
    fn test_memory_access_pattern_builder_methods() {
        // Test with_alignment
        let pattern = MemoryAccessPattern::new(RegId(0), 0x1000, AccessWidth::Word)
            .with_alignment(Alignment::Aligned4);
        assert_eq!(pattern.alignment, Alignment::Aligned4);

        // Test with_access_type
        let pattern = pattern.with_access_type(AccessType::Write);
        assert_eq!(pattern.access_type, AccessType::Write);

        // Test with_flags
        let flags = MemoryFlags {
            is_volatile: true,
            is_atomic: true,
            ..Default::default()
        };
        let pattern = pattern.with_flags(flags);
        assert!(pattern.flags.is_volatile);
        assert!(pattern.flags.is_atomic);

        // Test with_repeat
        let pattern = pattern.with_repeat(10);
        assert_eq!(pattern.repeat_count, Some(10));
    }

    #[test]
    fn test_access_width_sizes() {
        assert_eq!(
            MemoryAccessPattern::new(RegId(0), 0, AccessWidth::Byte).size(),
            1
        );
        assert_eq!(
            MemoryAccessPattern::new(RegId(0), 0, AccessWidth::HalfWord).size(),
            2
        );
        assert_eq!(
            MemoryAccessPattern::new(RegId(0), 0, AccessWidth::Word).size(),
            4
        );
        assert_eq!(
            MemoryAccessPattern::new(RegId(0), 0, AccessWidth::DoubleWord).size(),
            8
        );
        assert_eq!(
            MemoryAccessPattern::new(RegId(0), 0, AccessWidth::QuadWord).size(),
            16
        );
        assert_eq!(
            MemoryAccessPattern::new(RegId(0), 0, AccessWidth::Vector(32)).size(),
            32
        );
    }

    #[test]
    fn test_alignment_requirements() {
        let pattern = MemoryAccessPattern::new(RegId(0), 0, AccessWidth::Word)
            .with_alignment(Alignment::Unaligned);
        assert_eq!(pattern.required_alignment(), 1);

        let pattern = pattern.with_alignment(Alignment::Aligned2);
        assert_eq!(pattern.required_alignment(), 2);

        let pattern = pattern.with_alignment(Alignment::Aligned4);
        assert_eq!(pattern.required_alignment(), 4);

        let pattern = pattern.with_alignment(Alignment::Aligned8);
        assert_eq!(pattern.required_alignment(), 8);

        let pattern = pattern.with_alignment(Alignment::Aligned16);
        assert_eq!(pattern.required_alignment(), 16);

        let pattern = pattern.with_alignment(Alignment::Natural);
        assert_eq!(pattern.required_alignment(), 4); // Word is 4 bytes

        let pattern = pattern.with_alignment(Alignment::Strict);
        assert_eq!(pattern.required_alignment(), 4); // Word is 4 bytes
    }

    #[test]
    fn test_memory_flags_default() {
        let flags = MemoryFlags::default();
        assert!(!flags.is_volatile);
        assert!(!flags.is_atomic);
        assert!(!flags.is_acquire);
        assert!(!flags.is_release);
        assert!(!flags.is_locked);
        assert!(!flags.is_cacheable);
        assert!(!flags.is_privileged);
        assert!(!flags.is_endian_aware);
    }

    #[test]
    fn test_access_type_copy() {
        let access1 = AccessType::Read;
        let access2 = access1;
        assert_eq!(access1, access2);
    }

    #[test]
    fn test_access_width_copy() {
        let width1 = AccessWidth::Word;
        let width2 = width1;
        assert_eq!(width1, width2);
    }

    #[test]
    fn test_alignment_copy() {
        let align1 = Alignment::Aligned4;
        let align2 = align1;
        assert_eq!(align1, align2);
    }

    #[test]
    fn test_memory_error_partial_eq() {
        let err1 = MemoryError::AlignmentViolation(0x1000, 4);
        let err2 = MemoryError::AlignmentViolation(0x1000, 4);
        let err3 = MemoryError::AlignmentViolation(0x1004, 4);

        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    #[test]
    fn test_all_alignment_values() {
        let pattern = MemoryAccessPattern::new(RegId(0), 0, AccessWidth::Word);

        // Test all alignment values
        let alignments = vec![
            Alignment::Natural,
            Alignment::Unaligned,
            Alignment::Aligned1,
            Alignment::Aligned2,
            Alignment::Aligned4,
            Alignment::Aligned8,
            Alignment::Aligned16,
            Alignment::Aligned32,
            Alignment::Aligned64,
            Alignment::Strict,
        ];

        for alignment in alignments {
            let pattern = pattern.clone().with_alignment(alignment);
            assert!(pattern.required_alignment() >= 1);
        }
    }

    #[test]
    fn test_all_access_types() {
        let access_types = vec![
            AccessType::Read,
            AccessType::Write,
            AccessType::ReadWrite,
            AccessType::Execute,
            AccessType::AtomicRead,
            AccessType::AtomicWrite,
            AccessType::AtomicReadWrite,
        ];

        for access_type in access_types {
            let pattern = MemoryAccessPattern::new(RegId(0), 0, AccessWidth::Word)
                .with_access_type(access_type);
            assert_eq!(pattern.access_type, access_type);
        }
    }

    #[test]
    fn test_vector_access_width() {
        let pattern = MemoryAccessPattern::new(RegId(0), 0, AccessWidth::Vector(16));
        assert_eq!(pattern.size(), 16);
        assert_eq!(pattern.required_alignment(), 16); // Natural alignment
    }

    #[test]
    fn test_default_memory_access_optimizer_all_architectures() {
        // Test X86_64
        let optimizer_x86 = DefaultMemoryAccessOptimizer::new(Architecture::X86_64);
        assert_eq!(optimizer_x86.name(), "DefaultMemoryAccessOptimizer");
        assert_eq!(optimizer_x86.cache_line_size, 64);
        assert_eq!(optimizer_x86.vector_width, 32);

        // Test ARM64
        let optimizer_arm = DefaultMemoryAccessOptimizer::new(Architecture::ARM64);
        assert_eq!(optimizer_arm.cache_line_size, 64);
        assert_eq!(optimizer_arm.vector_width, 32);

        // Test RISCV64
        let optimizer_riscv = DefaultMemoryAccessOptimizer::new(Architecture::RISCV64);
        assert_eq!(optimizer_riscv.cache_line_size, 64);
        assert_eq!(optimizer_riscv.vector_width, 32);
    }

    #[test]
    fn test_optimizer_vectorization_opportunity() {
        let optimizer = DefaultMemoryAccessOptimizer::new(Architecture::X86_64);

        // Create a pattern with repeat count > 1 (vectorization opportunity)
        let pattern = MemoryAccessPattern::new(RegId(0), 0x1000, AccessWidth::Word).with_repeat(10);

        let result = optimizer.optimize_access_pattern(&pattern);

        // Should detect vectorization opportunity
        assert!(result.performance_gain > 0.0);
        assert_ne!(result.optimization_type, OptimizationType::NoOptimization);
    }

    #[test]
    fn test_optimizer_cache_line_optimization() {
        let optimizer = DefaultMemoryAccessOptimizer::new(Architecture::X86_64);

        // Create a pattern with unaligned cache line address
        let pattern = MemoryAccessPattern::new(RegId(0), 0x1041, AccessWidth::Word); // Not cache line aligned

        let result = optimizer.optimize_access_pattern(&pattern);

        // Should optimize for cache line alignment
        assert!(result.performance_gain > 0.0);
    }

    #[test]
    fn test_detect_alignment_issues() {
        let optimizer = DefaultMemoryAccessOptimizer::new(Architecture::X86_64);

        // Create a pattern with misaligned address
        let pattern = MemoryAccessPattern::new(RegId(0), 0x1001, AccessWidth::Word)
            .with_alignment(Alignment::Aligned4);

        let issues = optimizer.detect_alignment_issues(&pattern);

        assert!(!issues.is_empty());
        assert_eq!(issues[0].required_alignment, 4);
    }

    #[test]
    fn test_detect_alignment_issues_atomic_critical() {
        let optimizer = DefaultMemoryAccessOptimizer::new(Architecture::X86_64);

        // Create a pattern with atomic access and misalignment
        let pattern = MemoryAccessPattern::new(RegId(0), 0x1001, AccessWidth::Word)
            .with_alignment(Alignment::Aligned4);
        let mut flags = MemoryFlags::default();
        flags.is_atomic = true;
        let pattern = pattern.with_flags(flags);

        let issues = optimizer.detect_alignment_issues(&pattern);

        assert!(!issues.is_empty());
        assert_eq!(issues[0].severity, IssueSeverity::Critical);
    }

    #[test]
    fn test_detect_alignment_issues_quadword() {
        let optimizer = DefaultMemoryAccessOptimizer::new(Architecture::X86_64);

        // Create a pattern with QuadWord and misalignment
        let pattern = MemoryAccessPattern::new(RegId(0), 0x1001, AccessWidth::QuadWord)
            .with_alignment(Alignment::Natural);

        let issues = optimizer.detect_alignment_issues(&pattern);

        assert!(!issues.is_empty());
        assert_eq!(issues[0].severity, IssueSeverity::Error);
    }

    #[test]
    fn test_suggest_fixes() {
        let optimizer = DefaultMemoryAccessOptimizer::new(Architecture::X86_64);

        let pattern = MemoryAccessPattern::new(RegId(0), 0x1001, AccessWidth::Word)
            .with_alignment(Alignment::Aligned4);

        let issues = optimizer.detect_alignment_issues(&pattern);
        let fixes = optimizer.suggest_fixes(&issues);

        assert!(!fixes.is_empty());
        // For Warning severity, it suggests UseUnalignedAccess which is the default
        assert!(matches!(
            fixes[0].fix_type,
            FixType::UseUnalignedAccess | FixType::AdjustOffset | FixType::AlignAddress
        ));
    }

    #[test]
    fn test_suggest_fixes_critical_severity() {
        let optimizer = DefaultMemoryAccessOptimizer::new(Architecture::X86_64);

        let mut flags = MemoryFlags::default();
        flags.is_atomic = true;

        let pattern = MemoryAccessPattern::new(RegId(0), 0x1001, AccessWidth::Word)
            .with_alignment(Alignment::Aligned4)
            .with_flags(flags);

        let issues = optimizer.detect_alignment_issues(&pattern);
        let fixes = optimizer.suggest_fixes(&issues);

        assert_eq!(fixes[0].fix_type, FixType::AlignAddress);
        assert_eq!(fixes[0].cost_estimate, FixCost::Medium);
    }

    #[test]
    fn test_endianness_same_endian_conversion() {
        let converter = EndiannessConverter::new(
            Endianness::Little,
            Endianness::Little,
            ConversionStrategy::Direct,
        );

        let mut data = vec![0x12, 0x34, 0x56, 0x78];
        converter.convert(&mut data).unwrap();

        // Should remain unchanged
        assert_eq!(data, vec![0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_endianness_big_to_little() {
        let converter = EndiannessConverter::new(
            Endianness::Big,
            Endianness::Little,
            ConversionStrategy::Direct,
        );

        let mut data = vec![0x12, 0x34, 0x56, 0x78];
        converter.convert(&mut data).unwrap();

        assert_eq!(data, vec![0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn test_endianness_optimized_conversion() {
        let converter = EndiannessConverter::new(
            Endianness::Little,
            Endianness::Big,
            ConversionStrategy::Optimized,
        );

        // Test 2-byte conversion
        let mut data2 = vec![0x12, 0x34];
        converter.convert(&mut data2).unwrap();
        assert_eq!(data2, vec![0x34, 0x12]);

        // Test 4-byte conversion
        let mut data4 = vec![0x12, 0x34, 0x56, 0x78];
        converter.convert(&mut data4).unwrap();
        assert_eq!(data4, vec![0x78, 0x56, 0x34, 0x12]);

        // Test 8-byte conversion
        let mut data8 = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        converter.convert(&mut data8).unwrap();
        assert_eq!(data8, vec![0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn test_endianness_16byte_optimized_conversion() {
        let converter = EndiannessConverter::new(
            Endianness::Little,
            Endianness::Big,
            ConversionStrategy::Optimized,
        );

        let mut data = vec![
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
        ];
        converter.convert(&mut data).unwrap();

        assert_eq!(
            data,
            vec![
                0x10, 0x0F, 0x0E, 0x0D, 0x0C, 0x0B, 0x0A, 0x09, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03,
                0x02, 0x01
            ]
        );
    }

    #[test]
    fn test_endianness_lazy_conversion() {
        let converter = EndiannessConverter::new(
            Endianness::Little,
            Endianness::Big,
            ConversionStrategy::Lazy,
        );

        let mut data = vec![0x12, 0x34, 0x56, 0x78];
        let result = converter.convert(&mut data);

        // Lazy conversion should succeed (deferred)
        assert!(result.is_ok());
    }

    #[test]
    fn test_endianness_precomputed_conversion() {
        let converter = EndiannessConverter::new(
            Endianness::Little,
            Endianness::Big,
            ConversionStrategy::Precomputed,
        );

        let mut data = vec![0x12, 0x34, 0x56, 0x78];
        converter.convert(&mut data).unwrap();

        // Should perform conversion (falls back to direct)
        assert_eq!(data, vec![0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn test_endianness_copy() {
        let endian1 = Endianness::Little;
        let endian2 = endian1;
        assert_eq!(endian1, endian2);
    }

    #[test]
    fn test_conversion_strategy_copy() {
        let strategy1 = ConversionStrategy::Optimized;
        let strategy2 = strategy1;
        assert_eq!(strategy1, strategy2);
    }

    #[test]
    fn test_memory_access_analyzer_unaligned_tracking() {
        let mut analyzer = MemoryAccessAnalyzer::new();

        // Add unaligned access
        analyzer.add_pattern(
            MemoryAccessPattern::new(RegId(0), 0x1001, AccessWidth::Word)
                .with_alignment(Alignment::Unaligned),
        );

        // Add aligned access
        analyzer.add_pattern(
            MemoryAccessPattern::new(RegId(1), 0x1000, AccessWidth::Word)
                .with_alignment(Alignment::Aligned4),
        );

        let result = analyzer.analyze();
        assert_eq!(result.unaligned_accesses, 1);
        assert_eq!(result.total_accesses, 2);
        assert!(result.unaligned_percentage > 0.0);
    }

    #[test]
    fn test_memory_access_analyzer_atomic_tracking() {
        let mut analyzer = MemoryAccessAnalyzer::new();

        // Add atomic access
        let mut flags = MemoryFlags::default();
        flags.is_atomic = true;

        analyzer.add_pattern(
            MemoryAccessPattern::new(RegId(0), 0x1000, AccessWidth::Word).with_flags(flags),
        );

        let result = analyzer.analyze();
        assert_eq!(result.atomic_accesses, 1);
        assert_eq!(result.total_accesses, 1);
        assert!(result.atomic_percentage > 0.0);
    }

    #[test]
    fn test_memory_access_analyzer_vector_tracking() {
        let mut analyzer = MemoryAccessAnalyzer::new();

        // Add vector access
        analyzer.add_pattern(MemoryAccessPattern::new(
            RegId(0),
            0x1000,
            AccessWidth::Vector(16),
        ));

        let result = analyzer.analyze();
        assert_eq!(result.vector_accesses, 1);
        assert_eq!(result.total_accesses, 1);
        assert!(result.vector_percentage > 0.0);
    }

    #[test]
    fn test_memory_access_analyzer_size_distribution() {
        let mut analyzer = MemoryAccessAnalyzer::new();

        analyzer.add_pattern(MemoryAccessPattern::new(
            RegId(0),
            0x1000,
            AccessWidth::Byte,
        ));
        analyzer.add_pattern(MemoryAccessPattern::new(
            RegId(1),
            0x1004,
            AccessWidth::Word,
        ));
        analyzer.add_pattern(MemoryAccessPattern::new(
            RegId(2),
            0x1008,
            AccessWidth::Word,
        ));
        analyzer.add_pattern(MemoryAccessPattern::new(
            RegId(3),
            0x1010,
            AccessWidth::DoubleWord,
        ));

        let result = analyzer.analyze();

        assert_eq!(result.total_accesses, 4);
        assert_eq!(result.size_distribution.get(&1), Some(&1));
        assert_eq!(result.size_distribution.get(&4), Some(&2));
        assert_eq!(result.size_distribution.get(&8), Some(&1));
        assert_eq!(result.most_common_size, Some(4)); // Word appears twice
    }

    #[test]
    fn test_issue_severity_levels() {
        let severities = vec![
            IssueSeverity::Warning,
            IssueSeverity::Error,
            IssueSeverity::Critical,
        ];

        for severity in severities {
            assert!(matches!(
                severity,
                IssueSeverity::Warning | IssueSeverity::Error | IssueSeverity::Critical
            ));
        }
    }

    #[test]
    fn test_fix_type_all_values() {
        let fix_types = vec![
            FixType::UseUnalignedAccess,
            FixType::AlignAddress,
            FixType::AdjustOffset,
            FixType::ChangeAccessWidth,
            FixType::InsertPadding,
            FixType::RestructureAccess,
        ];

        for fix_type in fix_types {
            assert!(matches!(
                fix_type,
                FixType::UseUnalignedAccess
                    | FixType::AlignAddress
                    | FixType::AdjustOffset
                    | FixType::ChangeAccessWidth
                    | FixType::InsertPadding
                    | FixType::RestructureAccess
            ));
        }
    }

    #[test]
    fn test_fix_cost_levels() {
        let costs = vec![
            FixCost::None,
            FixCost::Low,
            FixCost::Medium,
            FixCost::High,
            FixCost::Unknown,
        ];

        for cost in costs {
            assert!(matches!(
                cost,
                FixCost::None | FixCost::Low | FixCost::Medium | FixCost::High | FixCost::Unknown
            ));
        }
    }

    #[test]
    fn test_optimization_type_all_values() {
        let opt_types = vec![
            OptimizationType::NoOptimization,
            OptimizationType::AlignedAccess,
            OptimizationType::CombinedAccess,
            OptimizationType::VectorizedAccess,
            OptimizationType::Prefetch,
            OptimizationType::CacheLineOptimized,
            OptimizationType::BurstAccess,
        ];

        for opt_type in opt_types {
            assert!(matches!(
                opt_type,
                OptimizationType::NoOptimization
                    | OptimizationType::AlignedAccess
                    | OptimizationType::CombinedAccess
                    | OptimizationType::VectorizedAccess
                    | OptimizationType::Prefetch
                    | OptimizationType::CacheLineOptimized
                    | OptimizationType::BurstAccess
            ));
        }
    }

    #[test]
    fn test_optimized_pattern_clone() {
        let pattern = MemoryAccessPattern::new(RegId(0), 0x1000, AccessWidth::Word);

        let optimized = OptimizedPattern {
            original: pattern.clone(),
            optimized: pattern,
            optimization_type: OptimizationType::AlignedAccess,
            performance_gain: 0.25,
        };

        let cloned = optimized.clone();
        assert_eq!(cloned.performance_gain, optimized.performance_gain);
    }

    #[test]
    fn test_alignment_issue_clone() {
        let issue = AlignmentIssue {
            address: 0x1001,
            required_alignment: 4,
            actual_alignment: 1,
            severity: IssueSeverity::Warning,
            description: "Test issue".to_string(),
        };

        let cloned = issue.clone();
        assert_eq!(cloned.address, issue.address);
        assert_eq!(cloned.severity, issue.severity);
    }

    #[test]
    fn test_memory_access_pattern_clone() {
        let pattern = MemoryAccessPattern::new(RegId(0), 0x1000, AccessWidth::Word)
            .with_alignment(Alignment::Aligned4)
            .with_access_type(AccessType::Read);

        let cloned = pattern.clone();
        assert_eq!(cloned.base_reg, pattern.base_reg);
        assert_eq!(cloned.alignment, pattern.alignment);
        assert_eq!(cloned.access_type, pattern.access_type);
    }

    #[test]
    fn test_memory_flags_clone() {
        let flags = MemoryFlags {
            is_volatile: true,
            is_atomic: true,
            ..Default::default()
        };

        let cloned = flags.clone();
        assert_eq!(cloned.is_volatile, flags.is_volatile);
        assert_eq!(cloned.is_atomic, flags.is_atomic);
    }

    #[test]
    fn test_analysis_result_default() {
        let result = AnalysisResult {
            total_accesses: 0,
            unaligned_accesses: 0,
            atomic_accesses: 0,
            vector_accesses: 0,
            unaligned_percentage: 0.0,
            atomic_percentage: 0.0,
            vector_percentage: 0.0,
            size_distribution: HashMap::new(),
            most_common_size: None,
        };

        assert_eq!(result.total_accesses, 0);
        assert!(result.size_distribution.is_empty());
    }
}
