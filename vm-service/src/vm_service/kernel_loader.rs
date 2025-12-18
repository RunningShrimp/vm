//! 内核加载模块

use std::sync::{Arc, Mutex};
use vm_core::MMU;
use vm_core::{GuestAddr, MemoryError, VmError, VmResult};

/// 加载内核镜像到内存（同步版本，保留用于向后兼容）
pub fn load_kernel(
    mmu: Arc<Mutex<Box<dyn MMU>>>,
    data: &[u8],
    load_addr: GuestAddr,
) -> VmResult<()> {
    let mut mmu_guard = mmu.lock().map_err(|_| {
        VmError::Memory(MemoryError::MmuLockFailed {
            message: "Failed to acquire MMU lock".to_string(),
        })
    })?;

    mmu_guard
        .write_bulk(load_addr, data)
        .map_err(|f| VmError::from(f))?;

    Ok(())
}

/// 异步加载内核镜像到内存
#[cfg(feature = "async")]
pub async fn load_kernel_async(
    mmu: Arc<tokio::sync::Mutex<Box<dyn MMU + Send>>>,
    data: &[u8],
    load_addr: GuestAddr,
) -> VmResult<()> {
    use vm_mem::AsyncMMU;
    use vm_mem::AsyncMmuWrapper;

    // 将同步MMU包装为异步MMU
    let async_mmu = AsyncMmuWrapper { inner: mmu };

    async_mmu
        .write_bulk_async(load_addr, data)
        .await
        .map_err(|f| VmError::from(f))?;

    Ok(())
}

/// 从文件加载内核（同步版本）
#[cfg(not(feature = "no_std"))]
pub fn load_kernel_file(path: &str, _load_addr: GuestAddr) -> VmResult<()> {
    use std::fs;
    let _data = fs::read(path).map_err(|e| VmError::Io(e.to_string()))?;
    // 注意：这个函数需要MMU，但为了简化，我们返回错误
    // 实际使用时应该通过VirtualMachineService调用
    Err(VmError::Core(vm_core::CoreError::Config {
        message: "load_kernel_file should be called through VirtualMachineService".to_string(),
        path: None,
    }))
}

/// 异步从文件加载内核
#[cfg(all(feature = "async", not(feature = "no_std")))]
pub async fn load_kernel_file_async(
    mmu: Arc<tokio::sync::Mutex<Box<dyn MMU + Send>>>,
    path: &str,
    load_addr: GuestAddr,
) -> VmResult<()> {
    use vm_mem::async_mmu::async_file_io;

    // 使用异步文件I/O读取文件
    let data = async_file_io::read_file_to_memory(path).await?;

    // 使用异步MMU写入内存
    load_kernel_async(mmu, &data, load_addr).await
}


