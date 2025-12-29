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

    mmu_guard.write_bulk(load_addr, data)?;

    Ok(())
}

/// 异步加载内核镜像到内存
#[cfg(feature = "performance")]
pub async fn load_kernel_async(
    mmu: Arc<tokio::sync::Mutex<Box<dyn MMU + Send>>>,
    data: &[u8],
    load_addr: GuestAddr,
) -> VmResult<()> {
    // 直接在异步MMU上操作
    let mut mmu_guard = mmu.lock().await;
    mmu_guard.write_bulk(load_addr, data)?;

    Ok(())
}

/// 同步包装器：在runtime中执行异步加载
#[cfg(feature = "performance")]
pub fn load_kernel_async_sync(
    mmu: Arc<tokio::sync::Mutex<Box<dyn MMU + Send>>>,
    data: &[u8],
    load_addr: GuestAddr,
) -> VmResult<()> {
    block_on_async_helper(load_kernel_async(mmu, data, load_addr))
}

/// 从文件加载内核（同步版本）
///
/// 返回文件数据，由调用者决定如何加载到MMU
pub fn load_kernel_file(path: &str, _load_addr: GuestAddr) -> VmResult<Vec<u8>> {
    use std::fs;
    let data = fs::read(path).map_err(|e| VmError::Io(e.to_string()))?;

    // 验证数据不为空
    if data.is_empty() {
        return Err(VmError::Core(vm_core::CoreError::Config {
            message: "Kernel file is empty".to_string(),
            path: Some(path.to_string()),
        }));
    }

    Ok(data)
}

/// 异步从文件加载内核
#[cfg(feature = "performance")]
pub async fn load_kernel_file_async(
    mmu: Arc<tokio::sync::Mutex<Box<dyn MMU + Send>>>,
    path: &str,
    load_addr: GuestAddr,
) -> VmResult<()> {
    // 使用异步文件I/O读取文件
    let data = tokio::fs::read(path).await.map_err(|e| {
        VmError::Memory(MemoryError::MmuLockFailed {
            message: format!("Failed to read file: {}", e),
        })
    })?;

    // 使用异步MMU写入内存
    load_kernel_async(mmu, &data, load_addr).await
}

/// 同步包装器：在runtime中执行异步文件加载
#[cfg(feature = "performance")]
pub fn load_kernel_file_async_sync(
    mmu: Arc<tokio::sync::Mutex<Box<dyn MMU + Send>>>,
    path: &str,
    load_addr: GuestAddr,
) -> VmResult<()> {
    block_on_async_helper(load_kernel_file_async(mmu, path, load_addr))
}

/// Helper function to block on async operations, using Handle when available
#[cfg(feature = "performance")]
fn block_on_async_helper<F, R>(f: F) -> VmResult<R>
where
    F: std::future::Future<Output = VmResult<R>>,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(f),
        Err(_) => tokio::runtime::Runtime::new()
            .map_err(|e| VmError::Io(format!("Failed to create tokio runtime: {}", e)))?
            .block_on(f),
    }
}
