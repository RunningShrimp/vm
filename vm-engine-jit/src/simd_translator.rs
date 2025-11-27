//! SIMD 和高级指令转换模块
//!
//! 提供向量、原子、系统等高级指令的 Cranelift 转换。

use vm_ir::RegId;
use cranelift::prelude::*;
use std::collections::HashMap;

/// SIMD 转换辅助函数
pub struct SimdTranslator;

impl SimdTranslator {
    /// 为 128 位向量操作创建向量类型
    pub fn vector_type_128() -> Type {
        // Cranelift 使用 I8X16 作为基本 128 位向量
        types::I8X16
    }

    /// 为 256 位向量操作创建向量类型
    pub fn vector_type_256() -> Type {
        // Cranelift 不直接支持 256 位，通常用两个 128 位向量模拟
        types::I8X16
    }

    /// 创建向量按元素大小进行操作的类型
    pub fn vector_element_type(element_size: u8) -> Option<Type> {
        match element_size {
            1 => Some(types::I8X16),
            2 => Some(types::I16X8),
            4 => Some(types::I32X4),
            8 => Some(types::I64X2),
            _ => None,
        }
    }
}

/// 原子操作转换
pub struct AtomicTranslator;

impl AtomicTranslator {
    /// 获取原子操作对应的 Cranelift 原子操作码
    pub fn get_atomic_op(op: &vm_ir::AtomicOp) -> Option<AtomicRmwOp> {
        match op {
            vm_ir::AtomicOp::Add => Some(AtomicRmwOp::Add),
            vm_ir::AtomicOp::Sub => Some(AtomicRmwOp::Sub),
            vm_ir::AtomicOp::And => Some(AtomicRmwOp::BitwiseAnd),
            vm_ir::AtomicOp::Or => Some(AtomicRmwOp::BitwiseOr),
            vm_ir::AtomicOp::Xor => Some(AtomicRmwOp::BitwiseXor),
            vm_ir::AtomicOp::Xchg => Some(AtomicRmwOp::Xchg),
            _ => None, // CmpXchg, Min, Max 等需要特殊处理
        }
    }
}

/// CSR（Control and Status Register）访问转换
pub struct CsrTranslator {
    csr_map: HashMap<u16, String>,
}

impl CsrTranslator {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        // RISC-V CSR 编码
        map.insert(0x300, "mstatus".to_string());
        map.insert(0x301, "misa".to_string());
        map.insert(0x304, "mie".to_string());
        map.insert(0x305, "mtvec".to_string());
        map.insert(0x341, "mepc".to_string());
        map.insert(0x342, "mcause".to_string());
        map.insert(0x343, "mtval".to_string());

        Self { csr_map: map }
    }

    pub fn get_csr_name(&self, csr: u16) -> String {
        self.csr_map.get(&csr)
            .cloned()
            .unwrap_or_else(|| format!("csr_{:x}", csr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_type_selection() {
        assert_eq!(SimdTranslator::vector_element_type(4), Some(types::I32X4));
        assert_eq!(SimdTranslator::vector_element_type(8), Some(types::I64X2));
    }
}
