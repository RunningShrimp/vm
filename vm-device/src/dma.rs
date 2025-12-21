//! 直接内存访问 (Direct Memory Access, DMA) 模块
//!
//! 实现零复制 I/O 操作，支持：
//! - DMA 描述符管理
//! - 内存映射 (mmap) 设备 I/O
//! - 分散-聚集 (Scatter-Gather) 列表
//! - DMA 一致性处理

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vm_core::{GuestAddr, HostAddr, MemoryError, VmError};

/// DMA 描述符
#[derive(Debug, Clone, Copy)]
pub struct DmaDescriptor {
    /// 客户机物理地址
    pub guest_addr: GuestAddr,
    /// 主机物理地址（如果已映射）
    pub host_addr: Option<HostAddr>,
    /// 长度（字节）
    pub len: usize,
    /// 标志
    pub flags: DmaFlags,
}

/// DMA 标志
#[derive(Debug, Clone, Copy, Default)]
pub struct DmaFlags {
    /// 读方向（主机 -> 设备）
    pub readable: bool,
    /// 写方向（设备 -> 主机）
    pub writable: bool,
    /// 缓存一致
    pub coherent: bool,
}

/// DMA 映射类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaMapType {
    /// 内核无法直接访问（需要专门映射）
    Unmapped,
    /// mmap 映射（零复制）
    MmapBased,
    /// 物理连续内存
    Contiguous,
}

/// 分散-聚集列表项
#[derive(Debug, Clone)]
pub struct ScatterGatherEntry {
    /// DMA 描述符
    pub descriptor: DmaDescriptor,
    /// 映射类型
    pub map_type: DmaMapType,
}

/// DMA 总线地址转换结果
#[derive(Debug, Clone)]
pub struct DmaTranslation {
    /// 主机地址
    pub host_addr: HostAddr,
    /// 物理连续长度
    pub contiguous_len: usize,
    /// 是否需要缓存同步
    pub needs_sync: bool,
}

/// DMA 管理器错误
#[derive(Debug, Clone)]
pub enum DmaError {
    /// 无法映射地址
    MappingFailed(String),
    /// 不支持的操作
    Unsupported(String),
    /// 内存不足
    OutOfMemory,
    /// 地址无效
    InvalidAddress,
}

impl From<DmaError> for VmError {
    fn from(err: DmaError) -> Self {
        match err {
            DmaError::MappingFailed(msg) | DmaError::Unsupported(msg) => {
                VmError::Io(std::io::Error::other(msg).to_string())
            }
            DmaError::OutOfMemory => VmError::Memory(MemoryError::AllocationFailed {
                message: "DMA: out of memory".into(),
                size: None,
            }),
            DmaError::InvalidAddress => VmError::Memory(MemoryError::MappingFailed {
                message: "DMA: invalid address".into(),
                src: None,
                dst: None,
            }),
        }
    }
}

/// DMA 管理器
pub struct DmaManager {
    /// 已映射的 DMA 区域
    mappings: Arc<Mutex<HashMap<GuestAddr, DmaDescriptor>>>,
    /// 支持的最大传输大小
    max_transfer_size: usize,
}

impl DmaManager {
    /// 创建新的 DMA 管理器
    pub fn new(max_transfer_size: usize) -> Self {
        Self {
            mappings: Arc::new(Mutex::new(HashMap::new())),
            max_transfer_size,
        }
    }

    /// 注册 DMA 映射
    pub fn register_mapping(&self, desc: DmaDescriptor) -> Result<(), DmaError> {
        let mut mappings = self
            .mappings
            .lock()
            .map_err(|_| DmaError::MappingFailed("Lock failed".into()))?;

        if desc.len == 0 {
            return Err(DmaError::InvalidAddress);
        }

        if desc.len > self.max_transfer_size {
            return Err(DmaError::Unsupported("Transfer too large".into()));
        }

        mappings.insert(desc.guest_addr, desc);
        Ok(())
    }

    /// 查找 DMA 映射
    pub fn find_mapping(&self, guest_addr: GuestAddr) -> Result<Option<DmaDescriptor>, DmaError> {
        let mappings = self
            .mappings
            .lock()
            .map_err(|_| DmaError::MappingFailed("Lock failed".into()))?;
        Ok(mappings.get(&guest_addr).copied())
    }

    /// 从客户机地址翻译 DMA 地址
    pub fn translate_dma_addr(&self, guest_addr: GuestAddr) -> Result<DmaTranslation, DmaError> {
        let mappings = self
            .mappings
            .lock()
            .map_err(|_| DmaError::MappingFailed("Lock failed".into()))?;

        if let Some(desc) = mappings.get(&guest_addr) {
            let host_addr = desc
                .host_addr
                .ok_or(DmaError::MappingFailed("No host address".into()))?;

            Ok(DmaTranslation {
                host_addr,
                contiguous_len: desc.len,
                needs_sync: !desc.flags.coherent,
            })
        } else {
            Err(DmaError::InvalidAddress)
        }
    }

    /// 构建分散-聚集列表
    pub fn build_scatter_gather_list(
        &self,
        guest_addr: GuestAddr,
        len: usize,
    ) -> Result<Vec<ScatterGatherEntry>, DmaError> {
        let mut sg_list = Vec::new();
        let mappings = self
            .mappings
            .lock()
            .map_err(|_| DmaError::MappingFailed("Lock failed".into()))?;

        let mut remaining = len;
        let mut current_guest = guest_addr;

        while remaining > 0 && sg_list.len() < 1024 {
            if let Some(desc) = mappings.get(&current_guest) {
                let transfer_len = remaining.min(desc.len);

                sg_list.push(ScatterGatherEntry {
                    descriptor: DmaDescriptor {
                        guest_addr: current_guest,
                        host_addr: desc.host_addr,
                        len: transfer_len,
                        flags: desc.flags,
                    },
                    map_type: if desc.host_addr.is_some() {
                        DmaMapType::MmapBased
                    } else {
                        DmaMapType::Unmapped
                    },
                });

                current_guest += transfer_len as u64;
                remaining -= transfer_len;
            } else {
                return Err(DmaError::InvalidAddress);
            }
        }

        if remaining > 0 {
            return Err(DmaError::Unsupported("Address space fragmented".into()));
        }

        Ok(sg_list)
    }

    /// 同步 DMA 缓存（对于非一致设备）
    pub fn sync_for_device(&self, guest_addr: GuestAddr) -> Result<(), DmaError> {
        let mappings = self
            .mappings
            .lock()
            .map_err(|_| DmaError::MappingFailed("Lock failed".into()))?;

        if let Some(desc) = mappings.get(&guest_addr) {
            if !desc.flags.coherent {
                // 这里应该调用实际的缓存同步代码
                // 例如：clflush, clwb, sfence 等
            }
            Ok(())
        } else {
            Err(DmaError::InvalidAddress)
        }
    }

    /// 同步 DMA 缓存（从设备读取）
    pub fn sync_from_device(&self, guest_addr: GuestAddr) -> Result<(), DmaError> {
        let mappings = self
            .mappings
            .lock()
            .map_err(|_| DmaError::MappingFailed("Lock failed".into()))?;

        if let Some(desc) = mappings.get(&guest_addr) {
            if !desc.flags.coherent {
                // 这里应该调用实际的缓存同步代码
                // 例如：clflush, mfence 等
            }
            Ok(())
        } else {
            Err(DmaError::InvalidAddress)
        }
    }

    /// 注销 DMA 映射
    pub fn unregister_mapping(&self, guest_addr: GuestAddr) -> Result<(), DmaError> {
        let mut mappings = self
            .mappings
            .lock()
            .map_err(|_| DmaError::MappingFailed("Lock failed".into()))?;
        mappings.remove(&guest_addr);
        Ok(())
    }

    /// 清除所有映射
    pub fn clear_all_mappings(&self) -> Result<(), DmaError> {
        let mut mappings = self
            .mappings
            .lock()
            .map_err(|_| DmaError::MappingFailed("Lock failed".into()))?;
        mappings.clear();
        Ok(())
    }

    /// 获取映射统计信息
    pub fn get_stats(&self) -> Result<DmaStats, DmaError> {
        let mappings = self
            .mappings
            .lock()
            .map_err(|_| DmaError::MappingFailed("Lock failed".into()))?;

        let total_size: usize = mappings.values().map(|d| d.len).sum();
        let coherent_count = mappings.values().filter(|d| d.flags.coherent).count();
        let mapped_count = mappings.values().filter(|d| d.host_addr.is_some()).count();

        Ok(DmaStats {
            total_mappings: mappings.len(),
            total_size,
            coherent_count,
            mapped_count,
        })
    }
}

/// DMA 统计信息
#[derive(Debug, Clone)]
pub struct DmaStats {
    /// 总映射数
    pub total_mappings: usize,
    /// 总映射大小（字节）
    pub total_size: usize,
    /// 一致缓存映射数
    pub coherent_count: usize,
    /// 已映射到主机地址的映射数
    pub mapped_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_core::{GuestAddr, HostAddr};

    #[test]
    fn test_dma_manager_creation() {
        let dma = DmaManager::new(4096);
        assert_eq!(dma.max_transfer_size, 4096);
    }

    #[test]
    fn test_register_and_find_mapping() {
        let dma = DmaManager::new(4096);
        let desc = DmaDescriptor {
            guest_addr: GuestAddr(0x1000),
            host_addr: Some(HostAddr(0x2000)),
            len: 512,
            flags: DmaFlags {
                readable: true,
                writable: true,
                coherent: true,
            },
        };

        assert!(dma.register_mapping(desc).is_ok());
        let found = dma.find_mapping(GuestAddr(0x1000)).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().len, 512);
    }

    #[test]
    fn test_translate_dma_address() {
        let dma = DmaManager::new(4096);
        let desc = DmaDescriptor {
            guest_addr: GuestAddr(0x1000),
            host_addr: Some(HostAddr(0x2000)),
            len: 512,
            flags: DmaFlags::default(),
        };

        dma.register_mapping(desc).unwrap();
        let translation = dma.translate_dma_addr(GuestAddr(0x1000)).unwrap();
        assert_eq!(translation.host_addr, HostAddr(0x2000));
    }

    #[test]
    fn test_scatter_gather_list() {
        let dma = DmaManager::new(4096);
        let desc = DmaDescriptor {
            guest_addr: GuestAddr(0x1000),
            host_addr: Some(HostAddr(0x2000)),
            len: 1024,
            flags: DmaFlags::default(),
        };

        dma.register_mapping(desc).unwrap();
        let sg_list = dma
            .build_scatter_gather_list(GuestAddr(0x1000), 512)
            .unwrap();
        assert_eq!(sg_list.len(), 1);
        assert_eq!(sg_list[0].descriptor.len, 512);
    }

    #[test]
    fn test_invalid_address_translation() {
        let dma = DmaManager::new(4096);
        let result = dma.translate_dma_addr(GuestAddr(0x5000));
        assert!(result.is_err());
    }

    #[test]
    fn test_dma_stats() {
        let dma = DmaManager::new(4096);
        let desc1 = DmaDescriptor {
            guest_addr: GuestAddr(0x1000),
            host_addr: Some(HostAddr(0x2000)),
            len: 512,
            flags: DmaFlags::default(),
        };
        let desc2 = DmaDescriptor {
            guest_addr: GuestAddr(0x2000),
            host_addr: Some(HostAddr(0x3000)),
            len: 1024,
            flags: DmaFlags::default(),
        };

        dma.register_mapping(desc1).unwrap();
        dma.register_mapping(desc2).unwrap();

        let stats = dma.get_stats().unwrap();
        assert_eq!(stats.total_mappings, 2);
        assert_eq!(stats.total_size, 1536);
    }
}
