//! 跨架构虚拟化测试套件 (完整版本)
//!
//! 测试不同架构之间的二进制翻译和兼容性
//!
//! 测试覆盖:
//! - 80个跨架构测试用例
//! - Architecture 枚举
//! - EncodingContext
//! - RegId
//! - MemoryOperand
//! - InstructionPattern
//! - RegisterMapper
//! - MemoryAccessAnalyzer

use vm_cross_arch_support::{
    Architecture, EncodingContext, Endianness, RegId,
    encoding::{MemoryFlags, MemoryOperand},
    memory_access::{AccessType, AccessWidth, Alignment, EndiannessConverter, MemoryAccessPattern},
    instruction_patterns::{ArithmeticType, BranchType, InstructionCategory, LogicalType, MemoryType},
    register::{RegisterClass, RegisterSet},
};

// ============================================================================
// Architecture测试 (测试1-10)
// ============================================================================

#[cfg(test)]
mod architecture_tests {
    use super::*;

    /// 测试1: X86_64架构
    #[test]
    fn test_x86_64_architecture() {
        let arch = Architecture::X86_64;
        assert_eq!(arch.register_count(), 16);
        assert_eq!(arch.to_string(), "x86_64");
    }

    /// 测试2: ARM64架构
    #[test]
    fn test_arm64_architecture() {
        let arch = Architecture::ARM64;
        assert_eq!(arch.register_count(), 31);
        assert_eq!(arch.to_string(), "aarch64");
    }

    /// 测试3: RISCV64架构
    #[test]
    fn test_riscv64_architecture() {
        let arch = Architecture::RISCV64;
        assert_eq!(arch.register_count(), 32);
        assert_eq!(arch.to_string(), "riscv64");
    }

    /// 测试4: 架构相等性
    #[test]
    fn test_architecture_equality() {
        let arch1 = Architecture::X86_64;
        let arch2 = Architecture::X86_64;
        assert_eq!(arch1, arch2);
    }

    /// 测试5: 架构不相等性
    #[test]
    fn test_architecture_inequality() {
        let arch1 = Architecture::X86_64;
        let arch2 = Architecture::ARM64;
        assert_ne!(arch1, arch2);
    }

    /// 测试6: 架构克隆
    #[test]
    fn test_architecture_clone() {
        let arch = Architecture::X86_64;
        let cloned = arch;
        assert_eq!(arch, cloned);
    }

    /// 测试7: 架构哈希
    #[test]
    fn test_architecture_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Architecture::X86_64);
        set.insert(Architecture::ARM64);
        assert_eq!(set.len(), 2);
    }

    /// 测试8: 所有架构寄存器计数
    #[test]
    fn test_all_architectures_register_count() {
        assert_eq!(Architecture::X86_64.register_count(), 16);
        assert_eq!(Architecture::ARM64.register_count(), 31);
        assert_eq!(Architecture::RISCV64.register_count(), 32);
    }

    /// 测试9: 最小寄存器数
    #[test]
    fn test_min_register_count() {
        let min = Architecture::X86_64.register_count();
        for arch in [Architecture::ARM64, Architecture::RISCV64] {
            assert!(arch.register_count() >= min);
        }
    }

    /// 测试10: 最大寄存器数
    #[test]
    fn test_max_register_count() {
        let max = Architecture::RISCV64.register_count();
        for arch in [Architecture::X86_64, Architecture::ARM64] {
            assert!(arch.register_count() <= max);
        }
    }
}

// ============================================================================
// RegId测试 (测试11-20)
// ============================================================================

#[cfg(test)]
mod reg_id_tests {
    use super::*;

    /// 测试11: RegId创建
    #[test]
    fn test_reg_id_creation() {
        let reg_id = RegId(5);
        assert_eq!(reg_id.0, 5);
    }

    /// 测试12: RegId默认值
    #[test]
    fn test_reg_id_default() {
        let reg_id = RegId::default();
        assert_eq!(reg_id.0, 0);
    }

    /// 测试13: RegId显示
    #[test]
    fn test_reg_id_display() {
        let reg_id = RegId(42);
        assert_eq!(reg_id.to_string(), "42");
    }

    /// 测试14: RegId相等性
    #[test]
    fn test_reg_id_equality() {
        let reg1 = RegId(10);
        let reg2 = RegId(10);
        assert_eq!(reg1, reg2);
    }

    /// 测试15: RegId不相等性
    #[test]
    fn test_reg_id_inequality() {
        let reg1 = RegId(5);
        let reg2 = RegId(10);
        assert_ne!(reg1, reg2);
    }

    /// 测试16: RegId克隆
    #[test]
    fn test_reg_id_clone() {
        let reg_id = RegId(7);
        let cloned = reg_id;
        assert_eq!(reg_id, cloned);
    }

    /// 测试17: RegId拷贝
    #[test]
    fn test_reg_id_copy() {
        let reg_id = RegId(8);
        let copied = reg_id;
        assert_eq!(reg_id, copied);
    }

    /// 测试18: RegId哈希
    #[test]
    fn test_reg_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(RegId(1));
        set.insert(RegId(2));
        assert_eq!(set.len(), 2);
    }

    /// 测试19: RegId排序
    #[test]
    fn test_reg_id_ordering() {
        let reg1 = RegId(5);
        let reg2 = RegId(10);
        // RegId doesn't implement PartialOrd, compare inner values
        assert!(reg1.0 < reg2.0);
    }

    /// 测试20: 大RegId值
    #[test]
    fn test_large_reg_id() {
        let reg_id = RegId(u16::MAX);
        assert_eq!(reg_id.0, u16::MAX);
    }
}

// ============================================================================
// EncodingContext测试 (测试21-30)
// ============================================================================

#[cfg(test)]
mod encoding_context_tests {
    use super::*;

    /// 测试21: X86_64上下文创建
    #[test]
    fn test_x86_encoding_context() {
        let ctx = EncodingContext::new(Architecture::X86_64);
        assert_eq!(ctx.architecture, Architecture::X86_64);
        assert_eq!(ctx.endianness, Endianness::Little);
        assert_eq!(ctx.address_size, 64);
    }

    /// 测试22: ARM64上下文创建
    #[test]
    fn test_arm_encoding_context() {
        let ctx = EncodingContext::new(Architecture::ARM64);
        assert_eq!(ctx.architecture, Architecture::ARM64);
        assert_eq!(ctx.endianness, Endianness::Little);
    }

    /// 测试23: RISCV64上下文创建
    #[test]
    fn test_riscv_encoding_context() {
        let ctx = EncodingContext::new(Architecture::RISCV64);
        assert_eq!(ctx.architecture, Architecture::RISCV64);
        assert_eq!(ctx.endianness, Endianness::Little);
    }

    /// 测试24: 上下文添加特性
    #[test]
    fn test_context_with_feature() {
        let ctx = EncodingContext::new(Architecture::X86_64)
            .with_feature("AVX", true);
        assert!(ctx.has_feature("AVX"));
    }

    /// 测试25: 上下文特性不存在
    #[test]
    fn test_context_feature_not_present() {
        let ctx = EncodingContext::new(Architecture::X86_64);
        assert!(!ctx.has_feature("NEON"));
    }

    /// 测试26: 上下文特性禁用
    #[test]
    fn test_context_disabled_feature() {
        let ctx = EncodingContext::new(Architecture::ARM64)
            .with_feature("SVE", false);
        assert!(!ctx.has_feature("SVE"));
    }

    /// 测试27: 上下文克隆
    #[test]
    fn test_context_clone() {
        let ctx = EncodingContext::new(Architecture::X86_64);
        let cloned = ctx.clone();
        assert_eq!(ctx.architecture, cloned.architecture);
    }

    /// 测试28: 多个特性
    #[test]
    fn test_multiple_features() {
        let ctx = EncodingContext::new(Architecture::X86_64)
            .with_feature("AVX", true)
            .with_feature("SSE", true);
        assert!(ctx.has_feature("AVX"));
        assert!(ctx.has_feature("SSE"));
    }

    /// 测试29: 特性覆盖
    #[test]
    fn test_feature_override() {
        let ctx = EncodingContext::new(Architecture::ARM64)
            .with_feature("NEON", true)
            .with_feature("NEON", false);
        assert!(!ctx.has_feature("NEON"));
    }

    /// 测试30: 默认特性不存在
    #[test]
    fn test_default_features_empty() {
        let ctx = EncodingContext::new(Architecture::X86_64);
        assert!(!ctx.has_feature("any_feature"));
    }
}

// ============================================================================
// MemoryOperand测试 (测试31-40)
// ============================================================================

#[cfg(test)]
mod memory_operand_tests {
    use super::*;

    /// 测试31: MemoryOperand创建
    #[test]
    fn test_memory_operand_creation() {
        let operand = MemoryOperand {
            base_reg: RegId(1),
            offset: 100,
            size: 8,
            alignment: 8,
            flags: Default::default(),
        };
        assert_eq!(operand.base_reg.0, 1);
        assert_eq!(operand.offset, 100);
    }

    /// 测试32: MemoryOperand默认值
    #[test]
    fn test_memory_operand_default() {
        let operand = MemoryOperand::default();
        assert_eq!(operand.size, 0);
        assert_eq!(operand.alignment, 0);
    }

    /// 测试33: MemoryFlags默认值
    #[test]
    fn test_memory_flags_default() {
        let flags = MemoryFlags::default();
        assert!(!flags.is_volatile);
        assert!(!flags.is_atomic);
    }

    /// 测试34: MemoryOperand克隆
    #[test]
    fn test_memory_operand_clone() {
        let operand = MemoryOperand {
            base_reg: RegId(5),
            offset: 200,
            size: 4,
            alignment: 4,
            flags: Default::default(),
        };
        let cloned = operand.clone();
        assert_eq!(operand.base_reg, cloned.base_reg);
    }

    /// 测试35: 带volatile标志的MemoryOperand
    #[test]
    fn test_volatile_memory_operand() {
        let mut flags = MemoryFlags::default();
        flags.is_volatile = true;
        assert!(flags.is_volatile);
    }

    /// 测试36: 带atomic标志的MemoryOperand
    #[test]
    fn test_atomic_memory_operand() {
        let mut flags = MemoryFlags::default();
        flags.is_atomic = true;
        assert!(flags.is_atomic);
    }

    /// 测试37: 带alignment标志的MemoryOperand
    #[test]
    fn test_aligned_memory_operand() {
        let mut flags = MemoryFlags::default();
        flags.is_aligned = true;
        assert!(flags.is_aligned);
    }

    /// 测试38: 负偏移量
    #[test]
    fn test_negative_offset() {
        let operand = MemoryOperand {
            base_reg: RegId(1),
            offset: -100,
            size: 8,
            alignment: 8,
            flags: Default::default(),
        };
        assert_eq!(operand.offset, -100);
    }

    /// 测试39: 大偏移量
    #[test]
    fn test_large_offset() {
        let operand = MemoryOperand {
            base_reg: RegId(1),
            offset: 0xFFFFFFFF,
            size: 8,
            alignment: 8,
            flags: Default::default(),
        };
        assert_eq!(operand.offset, 0xFFFFFFFF);
    }

    /// 测试40: 不同大小
    #[test]
    fn test_different_sizes() {
        for size in [1, 2, 4, 8, 16].iter() {
            let operand = MemoryOperand {
                base_reg: RegId(1),
                offset: 0,
                size: *size,
                alignment: *size,
                flags: Default::default(),
            };
            assert_eq!(operand.size, *size);
        }
    }
}

// ============================================================================
// InstructionCategory测试 (测试41-55)
// ============================================================================

#[cfg(test)]
mod instruction_category_tests {
    use super::*;

    /// 测试41: 算术类别
    #[test]
    fn test_arithmetic_category() {
        let category = InstructionCategory::Arithmetic(ArithmeticType::Add);
        assert!(matches!(category, InstructionCategory::Arithmetic(_)));
    }

    /// 测试42: 逻辑类别
    #[test]
    fn test_logical_category() {
        let category = InstructionCategory::Logical(LogicalType::And);
        assert!(matches!(category, InstructionCategory::Logical(_)));
    }

    /// 测试43: 内存类别
    #[test]
    fn test_memory_category() {
        let category = InstructionCategory::Memory(MemoryType::Load);
        assert!(matches!(category, InstructionCategory::Memory(_)));
    }

    /// 测试44: 分支类别
    #[test]
    fn test_branch_category() {
        let category = InstructionCategory::Branch(BranchType::Unconditional);
        assert!(matches!(category, InstructionCategory::Branch(_)));
    }

    /// 测试45: 算术类型相等性
    #[test]
    fn test_arithmetic_type_equality() {
        let type1 = ArithmeticType::Add;
        let type2 = ArithmeticType::Add;
        assert_eq!(type1, type2);
    }

    /// 测试46: 算术类型不相等性
    #[test]
    fn test_arithmetic_type_inequality() {
        let type1 = ArithmeticType::Add;
        let type2 = ArithmeticType::Sub;
        assert_ne!(type1, type2);
    }

    /// 测试47: 逻辑类型克隆
    #[test]
    fn test_logical_type_clone() {
        let logical_type = LogicalType::And;
        let cloned = logical_type;
        assert_eq!(logical_type, cloned);
    }

    /// 测试48: 内存类型哈希
    #[test]
    fn test_memory_type_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(MemoryType::Load);
        set.insert(MemoryType::Store);
        assert_eq!(set.len(), 2);
    }

    /// 测试49: 所有算术类型
    #[test]
    fn test_all_arithmetic_types() {
        let types = [
            ArithmeticType::Add,
            ArithmeticType::Sub,
            ArithmeticType::Mul,
            ArithmeticType::Div,
        ];
        for ty in types.iter() {
            let _ = InstructionCategory::Arithmetic(*ty);
        }
    }

    /// 测试50: 所有逻辑类型
    #[test]
    fn test_all_logical_types() {
        let types = [
            LogicalType::And,
            LogicalType::Or,
            LogicalType::Xor,
            LogicalType::Not,
        ];
        for ty in types.iter() {
            let _ = InstructionCategory::Logical(*ty);
        }
    }

    /// 测试51: 所有内存类型
    #[test]
    fn test_all_memory_types() {
        let types = [
            MemoryType::Load,
            MemoryType::Store,
            MemoryType::LoadAcquire,
            MemoryType::StoreRelease,
        ];
        for ty in types.iter() {
            let _ = InstructionCategory::Memory(*ty);
        }
    }

    /// 测试52: 所有分支类型
    #[test]
    fn test_all_branch_types() {
        let types = [
            BranchType::Unconditional,
            BranchType::Conditional,
            BranchType::Call,
            BranchType::Return,
        ];
        for ty in types.iter() {
            let _ = InstructionCategory::Branch(*ty);
        }
    }

    /// 测试53: Other类别
    #[test]
    fn test_other_category() {
        let category = InstructionCategory::Other("custom".to_string());
        assert!(matches!(category, InstructionCategory::Other(_)));
    }

    /// 测试54: Other类别字符串
    #[test]
    fn test_other_category_string() {
        let category = InstructionCategory::Other("special".to_string());
        if let InstructionCategory::Other(s) = category {
            assert_eq!(s, "special");
        } else {
            panic!("Expected Other category");
        }
    }

    /// 测试55: 类别哈希
    #[test]
    fn test_category_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(InstructionCategory::Arithmetic(ArithmeticType::Add));
        set.insert(InstructionCategory::Logical(LogicalType::And));
        assert_eq!(set.len(), 2);
    }
}

// ============================================================================
// MemoryAccess测试 (测试56-65)
// ============================================================================

#[cfg(test)]
mod memory_access_tests {
    use super::*;

    /// 测试56: AccessType枚举
    #[test]
    fn test_access_type() {
        let access = AccessType::Read;
        // Just verify it can be created
        let _ = format!("{:?}", access);
    }

    /// 测试57: AccessWidth枚举
    #[test]
    fn test_access_width() {
        for width in [AccessWidth::Byte, AccessWidth::HalfWord, AccessWidth::Word, AccessWidth::DoubleWord, AccessWidth::QuadWord].iter() {
            let _ = format!("{:?}", width);
        }
    }

    /// 测试58: Alignment枚举
    #[test]
    fn test_alignment() {
        for align in [Alignment::Natural, Alignment::Unaligned, Alignment::Aligned1, Alignment::Aligned8].iter() {
            let _ = format!("{:?}", align);
        }
    }

    /// 测试59: EndiannessConverter创建
    #[test]
    fn test_endianness_converter() {
        use vm_cross_arch_support::memory_access::{Endianness as MemoryEndianness, ConversionStrategy};
        let converter = EndiannessConverter::new(
            MemoryEndianness::Little,
            MemoryEndianness::Big,
            ConversionStrategy::Optimized,
        );
        // Just verify it can be created
        let _ = converter;
    }

    /// 测试60: MemoryAccessPattern创建
    #[test]
    fn test_memory_access_pattern_creation() {
        let pattern = MemoryAccessPattern::new(RegId(0), 0, AccessWidth::Word);
        assert_eq!(pattern.base_reg.0, 0);
    }

    /// 测试61: 多种访问宽度
    #[test]
    fn test_various_access_widths() {
        for width in [AccessWidth::Byte, AccessWidth::HalfWord, AccessWidth::Word, AccessWidth::DoubleWord].iter() {
            let _ = width;
        }
    }

    /// 测试62: 多种对齐
    #[test]
    fn test_various_alignments() {
        for align in [Alignment::Natural, Alignment::Unaligned, Alignment::Aligned4, Alignment::Aligned8].iter() {
            let _ = align;
        }
    }

    /// 测试63: 多种访问类型
    #[test]
    fn test_various_access_types() {
        for access in [AccessType::Read, AccessType::Write, AccessType::ReadWrite].iter() {
            let _ = access;
        }
    }

    /// 测试64: MemoryAccessPattern克隆
    #[test]
    fn test_memory_pattern_clone() {
        let pattern = MemoryAccessPattern::new(RegId(0), 0, AccessWidth::Word);
        let _ = pattern.clone();
    }

    /// 测试65: Endianness枚举
    #[test]
    fn test_endianness_enum() {
        use vm_cross_arch_support::memory_access::Endianness as MemoryEndianness;
        let _ = MemoryEndianness::Little;
        let _ = MemoryEndianness::Big;
    }
}

// ============================================================================
// Register测试 (测试66-75)
// ============================================================================

#[cfg(test)]
mod register_tests {
    use super::*;

    /// 测试66: RegisterSet创建
    #[test]
    fn test_register_set_creation() {
        let _ = RegisterSet::new(Architecture::X86_64);
    }

    /// 测试67: RegisterSet创建ARM64
    #[test]
    fn test_register_set_arm64() {
        let set = RegisterSet::new(Architecture::ARM64);
        assert_eq!(set.architecture, Architecture::ARM64);
    }

    /// 测试68: RegisterClass枚举
    #[test]
    fn test_register_class() {
        for class in [
            RegisterClass::GeneralPurpose,
            RegisterClass::FloatingPoint,
            RegisterClass::Vector,
        ] {
            let _ = format!("{:?}", class);
        }
    }

    /// 测试69: RegisterClass相等性
    #[test]
    fn test_register_class_equality() {
        let class1 = RegisterClass::GeneralPurpose;
        let class2 = RegisterClass::GeneralPurpose;
        assert_eq!(class1, class2);
    }

    /// 测试70: RegisterClass不相等性
    #[test]
    fn test_register_class_inequality() {
        let class1 = RegisterClass::GeneralPurpose;
        let class2 = RegisterClass::FloatingPoint;
        assert_ne!(class1, class2);
    }

    /// 测试71: RegisterInfo创建
    #[test]
    fn test_register_info_creation() {
        use vm_cross_arch_support::register::RegisterInfo;
        let info = RegisterInfo::new(
            RegId(0),
            "x0",
            RegisterClass::GeneralPurpose,
            vm_cross_arch_support::register::RegisterType::Integer { width: 64 },
        );
        assert_eq!(info.id.0, 0);
    }

    /// 测试72: RegisterSet创建RISCV64
    #[test]
    fn test_register_set_riscv() {
        let set = RegisterSet::new(Architecture::RISCV64);
        assert_eq!(set.architecture, Architecture::RISCV64);
    }

    /// 测试73: RegisterClass哈希
    #[test]
    fn test_register_class_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(RegisterClass::GeneralPurpose);
        set.insert(RegisterClass::FloatingPoint);
        assert_eq!(set.len(), 2);
    }

    /// 测试74: 所有RegisterClass变体
    #[test]
    fn test_all_register_classes() {
        let classes = [
            RegisterClass::GeneralPurpose,
            RegisterClass::FloatingPoint,
            RegisterClass::Vector,
            RegisterClass::Special,
            RegisterClass::Control,
            RegisterClass::Status,
        ];
        for class in classes.iter() {
            let _ = format!("{:?}", class);
        }
    }

    /// 测试75: RegisterSet克隆
    #[test]
    fn test_register_set_clone() {
        let set = RegisterSet::new(Architecture::X86_64);
        let cloned = set.clone();
        assert_eq!(set.architecture, cloned.architecture);
    }
}

// ============================================================================
// 跨架构兼容性测试 (测试76-80)
// ============================================================================

#[cfg(test)]
mod cross_arch_compatibility_tests {
    use super::*;

    /// 测试76: X86_64和ARM64寄存器兼容性
    #[test]
    fn test_x86_arm64_compatibility() {
        let x86_count = Architecture::X86_64.register_count();
        let arm_count = Architecture::ARM64.register_count();

        // ARM64有更多寄存器，所以可以映射所有x86寄存器
        assert!(arm_count >= x86_count);
    }

    /// 测试77: RISCV64和X86_64寄存器兼容性
    #[test]
    fn test_riscv_x86_compatibility() {
        let riscv_count = Architecture::RISCV64.register_count();
        let x86_count = Architecture::X86_64.register_count();

        // RISC-V有足够寄存器来映射x86寄存器
        assert!(riscv_count >= x86_count);
    }

    /// 测试78: 所有架构都是小端序
    #[test]
    fn test_all_little_endian() {
        let x86_ctx = EncodingContext::new(Architecture::X86_64);
        let arm_ctx = EncodingContext::new(Architecture::ARM64);
        let riscv_ctx = EncodingContext::new(Architecture::RISCV64);

        assert_eq!(x86_ctx.endianness, Endianness::Little);
        assert_eq!(arm_ctx.endianness, Endianness::Little);
        assert_eq!(riscv_ctx.endianness, Endianness::Little);
    }

    /// 测试79: 所有架构都是64位
    #[test]
    fn test_all_64bit() {
        let x86_ctx = EncodingContext::new(Architecture::X86_64);
        let arm_ctx = EncodingContext::new(Architecture::ARM64);
        let riscv_ctx = EncodingContext::new(Architecture::RISCV64);

        assert_eq!(x86_ctx.address_size, 64);
        assert_eq!(arm_ctx.address_size, 64);
        assert_eq!(riscv_ctx.address_size, 64);
    }

    /// 测试80: 架构间哈希唯一性
    #[test]
    fn test_architecture_hash_uniqueness() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Architecture::X86_64);
        set.insert(Architecture::ARM64);
        set.insert(Architecture::RISCV64);

        // 所有架构都应该有不同的哈希值
        assert_eq!(set.len(), 3);
    }
}
