//! 内存模型映射
//!
//! 处理不同架构之间的内存模型差异：
//! - 字节序（大端/小端）
//! - 内存对齐要求
//! - 地址空间布局
//! - 页表格式差异

use vm_core::{GuestAddr, GuestArch, GuestPhysAddr};
use tracing::debug;

/// 字节序
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    /// 小端序（最低有效字节在前）
    LittleEndian,
    /// 大端序（最高有效字节在前）
    BigEndian,
}

/// 内存对齐要求
#[derive(Debug, Clone, Copy)]
pub struct Alignment {
    /// 最小对齐（字节）
    pub min: usize,
    /// 推荐对齐（字节）
    pub recommended: usize,
}

/// 内存模型配置
pub struct MemoryModel {
    /// 架构
    arch: GuestArch,
    /// 字节序
    endianness: Endianness,
    /// 对齐要求
    alignment: Alignment,
    /// 页大小
    page_size: u64,
    /// 地址空间大小（位）
    address_space_bits: u8,
}

impl MemoryModel {
    /// 创建指定架构的内存模型
    pub fn new(arch: GuestArch) -> Self {
        match arch {
            GuestArch::X86_64 => Self {
                arch,
                endianness: Endianness::LittleEndian,
                alignment: Alignment {
                    min: 1,
                    recommended: 8,
                },
                page_size: 4096,
                address_space_bits: 48, // x86_64 使用 48 位虚拟地址
            },
            GuestArch::Arm64 => Self {
                arch,
                endianness: Endianness::LittleEndian,
                alignment: Alignment {
                    min: 1,
                    recommended: 8,
                },
                page_size: 4096,
                address_space_bits: 48, // ARM64 使用 48 位虚拟地址
            },
            GuestArch::Riscv64 => Self {
                arch,
                endianness: Endianness::LittleEndian,
                alignment: Alignment {
                    min: 1,
                    recommended: 8,
                },
                page_size: 4096,
                address_space_bits: 39, // RISC-V64 SV39 使用 39 位虚拟地址
            },
        }
    }

    /// 获取字节序
    pub fn endianness(&self) -> Endianness {
        self.endianness
    }

    /// 获取对齐要求
    pub fn alignment(&self) -> Alignment {
        self.alignment
    }

    /// 获取页大小
    pub fn page_size(&self) -> u64 {
        self.page_size
    }

    /// 获取地址空间大小（位）
    pub fn address_space_bits(&self) -> u8 {
        self.address_space_bits
    }

    /// 检查地址是否对齐
    pub fn is_aligned(&self, addr: u64, alignment: usize) -> bool {
        addr % (alignment as u64) == 0
    }

    /// 对齐地址
    pub fn align_address(&self, addr: u64, alignment: usize) -> u64 {
        let align = alignment as u64;
        (addr + align - 1) & !(align - 1)
    }

    /// 转换地址（在不同架构之间）
    /// 
    /// 当前实现：直接返回，因为地址空间布局相似
    /// 实际实现可能需要处理：
    /// - 地址空间布局差异
    /// - 页表格式差异
    /// - 内存映射区域差异
    pub fn translate_address(&self, _from_arch: GuestArch, addr: GuestAddr) -> GuestAddr {
        // TODO: 实现地址转换逻辑
        // 当前实现：直接返回
        addr
    }
}

/// 内存模型映射器
pub struct MemoryModelMapper {
    /// Guest 内存模型
    guest_model: MemoryModel,
    /// Host 内存模型
    host_model: MemoryModel,
}

impl MemoryModelMapper {
    /// 创建新的内存模型映射器
    pub fn new(guest_arch: GuestArch, host_arch: GuestArch) -> Self {
        Self {
            guest_model: MemoryModel::new(guest_arch),
            host_model: MemoryModel::new(host_arch),
        }
    }

    /// 检查字节序兼容性
    pub fn is_endianness_compatible(&self) -> bool {
        self.guest_model.endianness() == self.host_model.endianness()
    }

    /// 转换地址
    pub fn translate_address(&self, addr: GuestAddr) -> GuestAddr {
        self.guest_model.translate_address(
            self.guest_model.arch,
            addr,
        )
    }

    /// 对齐地址（使用 guest 架构的对齐要求）
    pub fn align_guest_address(&self, addr: u64) -> u64 {
        let alignment = self.guest_model.alignment().recommended;
        self.guest_model.align_address(addr, alignment)
    }

    /// 获取 guest 页大小
    pub fn guest_page_size(&self) -> u64 {
        self.guest_model.page_size()
    }

    /// 获取 host 页大小
    pub fn host_page_size(&self) -> u64 {
        self.host_model.page_size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_model_creation() {
        let model = MemoryModel::new(GuestArch::X86_64);
        assert_eq!(model.endianness(), Endianness::LittleEndian);
        assert_eq!(model.page_size(), 4096);
        assert_eq!(model.address_space_bits(), 48);
    }

    #[test]
    fn test_address_alignment() {
        let model = MemoryModel::new(GuestArch::X86_64);
        
        assert!(model.is_aligned(0x1000, 8));
        assert!(!model.is_aligned(0x1001, 8));
        
        let aligned = model.align_address(0x1001, 8);
        assert_eq!(aligned, 0x1008);
    }

    #[test]
    fn test_memory_model_mapper() {
        let mapper = MemoryModelMapper::new(GuestArch::Arm64, GuestArch::X86_64);
        
        // 两个架构都使用小端序，应该兼容
        assert!(mapper.is_endianness_compatible());
        
        // 页大小应该相同
        assert_eq!(mapper.guest_page_size(), mapper.host_page_size());
    }
}

