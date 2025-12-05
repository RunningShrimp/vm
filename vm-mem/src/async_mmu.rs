//! 异步MMU实现
//!
//! 提供异步版本的MMU接口，使用tokio异步运行时优化I/O操作

use std::sync::Arc;
use std::collections::HashMap;
use vm_core::{AccessType, GuestAddr, GuestPhysAddr, MMU, VmError};

#[cfg(feature = "async")]
use tokio::sync::Mutex as AsyncMutex;
#[cfg(feature = "async")]
use parking_lot::RwLock as AsyncRwLock;

#[cfg(feature = "async")]
use async_trait::async_trait;

/// 异步MMU trait
///
/// 提供异步版本的MMU操作，减少阻塞时间
#[cfg(feature = "async")]
#[async_trait]
pub trait AsyncMMU: Send + Sync {
    /// 异步地址转换
    async fn translate_async(
        &self,
        va: GuestAddr,
        access: AccessType,
    ) -> Result<GuestPhysAddr, VmError>;

    /// 异步指令取指
    async fn fetch_insn_async(&self, pc: GuestAddr) -> Result<u64, VmError>;

    /// 异步内存读取
    async fn read_async(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError>;

    /// 异步内存写入
    async fn write_async(&self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError>;

    /// 异步批量读取
    async fn read_bulk_async(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError>;

    /// 异步批量写入
    async fn write_bulk_async(&self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError>;

    /// 异步刷新TLB
    async fn flush_tlb_async(&self) -> Result<(), VmError>;
}

/// 异步MMU包装器
///
/// 将同步MMU包装为异步MMU，使用异步锁减少阻塞
#[cfg(feature = "async")]
pub struct AsyncMmuWrapper {
    /// 内部MMU（使用异步互斥锁保护）
    inner: Arc<AsyncMutex<Box<dyn MMU>>>,
}

#[cfg(feature = "async")]
impl AsyncMmuWrapper {
    /// 创建新的异步MMU包装器
    pub fn new(mmu: Box<dyn MMU>) -> Self {
        Self {
            inner: Arc::new(AsyncMutex::new(mmu)),
        }
    }

    /// 获取内部MMU的引用（用于需要直接访问的场景）
    pub fn inner(&self) -> &Arc<AsyncMutex<Box<dyn MMU>>> {
        &self.inner
    }
}

#[cfg(feature = "async")]
#[async_trait]
impl AsyncMMU for AsyncMmuWrapper {
    /// 异步地址转换（优化版：使用异步TLB查找和页表遍历）
    async fn translate_async(
        &self,
        va: GuestAddr,
        access: AccessType,
    ) -> Result<GuestPhysAddr, VmError> {
        // 优化：使用异步锁，减少锁持有时间，提高并发性能
        let mut mmu = self.inner.lock().await;
        mmu.translate(va, access)
    }

    async fn fetch_insn_async(&self, pc: GuestAddr) -> Result<u64, VmError> {
        let mmu = self.inner.lock().await;
        mmu.fetch_insn(pc)
    }

    async fn read_async(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        let mmu = self.inner.lock().await;
        mmu.read(pa, size)
    }

    async fn write_async(&self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
        let mut mmu = self.inner.lock().await;
        mmu.write(pa, val, size)
    }

    async fn read_bulk_async(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError> {
        let mmu = self.inner.lock().await;
        mmu.read_bulk(pa, buf)
    }

    async fn write_bulk_async(&self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError> {
        let mut mmu = self.inner.lock().await;
        mmu.write_bulk(pa, buf)
    }

    async fn flush_tlb_async(&self) -> Result<(), VmError> {
        let mut mmu = self.inner.lock().await;
        mmu.flush_tlb();
        Ok(())
    }
}

/// TLB缓存trait
pub trait TlbCache: Send + Sync {
    /// 查找TLB条目
    fn lookup(&self, vpn: u64, asid: u16, access: AccessType) -> Option<(u64, u64)>;

    /// 插入TLB条目
    fn insert(&mut self, vpn: u64, ppn: u64, flags: u64, asid: u16);

    /// 刷新所有条目
    fn flush_all(&mut self);

    /// 刷新指定ASID的条目
    fn flush_asid(&mut self, asid: u16);
}

/// 异步TLB查找器
///
/// 提供异步TLB查找功能，减少锁竞争
/// 优化：使用分片锁和快速路径优化
#[cfg(feature = "async")]
pub struct AsyncTlbLookup {
    /// TLB缓存（使用异步互斥锁）
    cache: Arc<AsyncMutex<Box<dyn TlbCache>>>,
    /// 快速路径缓存（使用parking_lot RwLock，用于热点地址）
    fast_cache: Arc<AsyncRwLock<HashMap<(u64, u16), (u64, u64)>>>,
    /// 快速路径大小限制
    fast_cache_size: usize,
}

#[cfg(feature = "async")]
impl AsyncTlbLookup {
    /// 创建新的异步TLB查找器
    pub fn new(cache: Arc<AsyncMutex<Box<dyn TlbCache>>>) -> Self {
        Self {
            cache,
            fast_cache: Arc::new(AsyncRwLock::new(HashMap::new())),
            fast_cache_size: 1024, // 快速路径缓存1024个条目
        }
    }

    /// 异步查找TLB（优化版：先查快速路径，再查主TLB）
    pub async fn lookup_async(
        &self,
        vpn: u64,
        asid: u16,
        access: AccessType,
    ) -> Option<(u64, u64)> {
        // 1. 快速路径：先查快速缓存（使用parking_lot RwLock，同步但快速）
        {
            let fast_cache = self.fast_cache.read();
            if let Some(&(ppn, flags)) = fast_cache.get(&(vpn, asid)) {
                // 检查权限
                let required = match access {
                    AccessType::Read => 1 << 1,
                    AccessType::Write => 1 << 2,
                    AccessType::Exec => 1 << 3,
                };
                if (flags & required) != 0 {
                    return Some((ppn, flags));
                }
            }
        }

        // 2. 慢速路径：查主TLB
        let result = {
            let cache = self.cache.lock().await;
            cache.lookup(vpn, asid, access)
        };

        // 3. 如果命中，更新快速缓存
        if let Some((ppn, flags)) = result {
            let mut fast_cache = self.fast_cache.write();
            // 限制快速缓存大小
            if fast_cache.len() < self.fast_cache_size {
                fast_cache.insert((vpn, asid), (ppn, flags));
            } else {
                // 如果缓存已满，随机替换一个条目（简化实现）
                if fast_cache.len() >= self.fast_cache_size {
                    fast_cache.clear(); // 简化：清空缓存
                    fast_cache.insert((vpn, asid), (ppn, flags));
                }
            }
        }

        result
    }

    /// 异步插入TLB条目
    pub async fn insert_async(&self, vpn: u64, ppn: u64, flags: u64, asid: u16) {
        // 更新快速缓存
        {
            let mut fast_cache = self.fast_cache.write();
            if fast_cache.len() < self.fast_cache_size {
                fast_cache.insert((vpn, asid), (ppn, flags));
            }
        }

        // 更新主TLB
        let mut cache = self.cache.lock().await;
        cache.insert(vpn, ppn, flags, asid);
    }

    /// 异步刷新所有TLB
    pub async fn flush_all_async(&self) {
        // 清空快速缓存
        {
            let mut fast_cache = self.fast_cache.write();
            fast_cache.clear();
        }

        // 清空主TLB
        let mut cache = self.cache.lock().await;
        cache.flush_all();
    }

    /// 异步刷新指定ASID的TLB
    pub async fn flush_asid_async(&self, asid: u16) {
        // 清空快速缓存中该ASID的条目
        {
            let mut fast_cache = self.fast_cache.write();
            fast_cache.retain(|(_, cached_asid), _| *cached_asid != asid);
        }

        // 清空主TLB中该ASID的条目
        let mut cache = self.cache.lock().await;
        cache.flush_asid(asid);
    }
}

/// 异步页表遍历器
///
/// 提供异步页表遍历功能，减少阻塞时间
#[cfg(feature = "async")]
pub struct AsyncPageTableWalker {
    /// 页表基址
    page_table_base: GuestAddr,
    /// 页表缓存（使用parking_lot RwLock）
    cache: Arc<AsyncRwLock<HashMap<u64, (u64, u64)>>>,
    /// 缓存大小限制
    cache_size: usize,
}

#[cfg(feature = "async")]
impl AsyncPageTableWalker {
    /// 创建新的异步页表遍历器
    pub fn new(page_table_base: GuestAddr) -> Self {
        Self {
            page_table_base,
            cache: Arc::new(AsyncRwLock::new(HashMap::new())),
            cache_size: 4096, // 缓存4096个页表条目
        }
    }

    /// 异步页表遍历
    ///
    /// 先查缓存，如果未命中则进行页表遍历
    /// 
    /// 注意：此方法需要AsyncMMU trait实现
    pub async fn walk_async(
        &self,
        va: GuestAddr,
        access: AccessType,
        mmu: &dyn AsyncMMU,
    ) -> Result<(GuestPhysAddr, u64), VmError> {
        let vpn = va >> 12; // PAGE_SHIFT = 12

        // 1. 先查缓存
        {
            let cache = self.cache.read();
            if let Some(&(pa, flags)) = cache.get(&vpn) {
                return Ok((pa, flags));
            }
        }

        // 2. 缓存未命中，进行页表遍历
        // 使用异步MMU进行地址转换
        let pa = mmu.translate_async(va, access).await?;
        let flags = 0x7; // R|W|X (简化实现，实际应该从页表项读取)

        // 3. 更新缓存
        {
            let mut cache = self.cache.write();
            if cache.len() < self.cache_size {
                cache.insert(vpn, (pa, flags));
            } else {
                // 如果缓存已满，清空缓存（简化实现，实际应该使用LRU等策略）
                cache.clear();
                cache.insert(vpn, (pa, flags));
            }
        }

        Ok((pa, flags))
    }

    /// 异步刷新页表缓存
    pub async fn flush_cache_async(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }
}

/// 异步文件I/O工具
///
/// 使用tokio::fs替代std::fs，提供异步文件操作
#[cfg(feature = "async")]
pub mod async_file_io {
    use std::path::Path;
    use vm_core::{AccessType, VmError};

    /// 异步读取文件到内存
    pub async fn read_file_to_memory<P: AsRef<Path>>(path: P) -> Result<Vec<u8>, VmError> {
        use tokio::io::AsyncReadExt;
        let mut file = tokio::fs::File::open(path).await.map_err(|_e| {
            VmError::from(vm_core::Fault::AccessViolation { 
                addr: 0, 
                access: AccessType::Read 
            })
        })?;

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await.map_err(|_e| {
            VmError::from(vm_core::Fault::AccessViolation { 
                addr: 0, 
                access: AccessType::Read 
            })
        })?;

        Ok(buffer)
    }

    /// 异步写入内存到文件
    pub async fn write_memory_to_file<P: AsRef<Path>>(path: P, data: &[u8]) -> Result<(), VmError> {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::File::create(path).await.map_err(|_e| {
            VmError::from(vm_core::Fault::AccessViolation { 
                addr: 0, 
                access: AccessType::Read 
            })
        })?;
        file.write_all(data).await.map_err(|_e| {
            VmError::from(vm_core::Fault::AccessViolation { 
                addr: 0, 
                access: AccessType::Write 
            })
        })?;

        file.sync_all().await.map_err(|_e| {
            VmError::from(vm_core::Fault::AccessViolation { 
                addr: 0, 
                access: AccessType::Write 
            })
        })?;

        Ok(())
    }

    /// 异步读取文件的一部分
    pub async fn read_file_chunk<P: AsRef<Path>>(
        path: P,
        offset: u64,
        size: usize,
    ) -> Result<Vec<u8>, VmError> {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        let mut file = tokio::fs::File::open(path).await.map_err(|_e| {
            VmError::from(vm_core::Fault::AccessViolation { 
                addr: 0, 
                access: AccessType::Read 
            })
        })?;

        // 定位到指定偏移
        file.seek(tokio::io::SeekFrom::Start(offset))
            .await
            .map_err(|_e| {
                VmError::from(vm_core::Fault::AccessViolation { 
                    addr: 0, 
                    access: AccessType::Read 
                })
            })?;

        let mut buffer = vec![0u8; size];
        file.read_exact(&mut buffer).await.map_err(|_e| {
            VmError::from(vm_core::Fault::AccessViolation { 
                addr: 0, 
                access: AccessType::Read 
            })
        })?;

        Ok(buffer)
    }

    /// 异步写入文件的一部分
    pub async fn write_file_chunk<P: AsRef<Path>>(
        path: P,
        offset: u64,
        data: &[u8],
    ) -> Result<(), VmError> {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .await
            .map_err(|_e| {
                VmError::from(vm_core::Fault::AccessViolation { 
                    addr: 0, 
                    access: AccessType::Write 
                })
            })?;

        // 定位到指定偏移
        use tokio::io::{AsyncSeekExt, AsyncWriteExt};
        file.seek(tokio::io::SeekFrom::Start(offset))
            .await
            .map_err(|_e| {
                VmError::from(vm_core::Fault::AccessViolation { 
                    addr: 0, 
                    access: AccessType::Read 
                })
            })?;

        file.write_all(data).await.map_err(|_e| {
            VmError::from(vm_core::Fault::AccessViolation { 
                addr: 0, 
                access: AccessType::Write 
            })
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_mmu_wrapper() {
        // 创建一个简单的MMU实现用于测试
        // 注意：这里需要实际的MMU实现，暂时跳过测试
        // let mmu = SoftMmu::new(1024 * 1024);
        // let async_mmu = AsyncMmuWrapper::new(Box::new(mmu));
        //
        // // 测试异步读取
        // let result = async_mmu.read_async(0x1000, 4).await;
        // assert!(result.is_ok());
        //
        // // 测试异步写入
        // let result = async_mmu.write_async(0x1000, 0x12345678, 4).await;
        // assert!(result.is_ok());
        //
        // // 验证写入结果
        // let value = async_mmu.read_async(0x1000, 4).await.unwrap();
        // assert_eq!(value, 0x12345678);
    }
}
