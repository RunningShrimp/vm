//! Common memory access abstraction for VM cross-architecture translation
//!
//! This module provides unified memory access patterns, alignment handling,
//! and endianness conversion across different architectures.

use crate::encoding::{Architecture, RegId};
use std::collections::HashMap;
use thiserror::Error;

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

/// Memory access types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    Read,
    Write,
    ReadWrite,
    Execute,
    AtomicRead,
    AtomicWrite,
    AtomicReadWrite,
}

/// Memory access width
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessWidth {
    Byte,
    HalfWord,
    Word,
    DoubleWord,
    QuadWord,
    Vector(u8), // Vector with specified lane count
}

/// Memory alignment requirements
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Unaligned,
    Aligned1,
    Aligned2,
    Aligned4,
    Aligned8,
    Aligned16,
    Aligned32,
    Aligned64,
    Natural, // Natural alignment for the access width
    Strict,  // Strict alignment (must be naturally aligned)
}

/// Memory access flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl Default for MemoryFlags {
    fn default() -> Self {
        Self {
            is_volatile: false,
            is_atomic: false,
            is_acquire: false,
            is_release: false,
            is_locked: false,
            is_cacheable: true,
            is_privileged: false,
            is_endian_aware: false,
        }
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationType {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSeverity {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixType {
    AlignAddress,
    AdjustOffset,
    ChangeAccessWidth,
    InsertPadding,
    UseUnalignedAccess,
    RestructureAccess,
}

/// Cost estimate for applying a fix
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixCost {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    Little,
    Big,
}

/// Conversion strategies for endianness
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionStrategy {
    Direct,      // Direct byte swapping
    Optimized,   // Optimized for common patterns
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
}
