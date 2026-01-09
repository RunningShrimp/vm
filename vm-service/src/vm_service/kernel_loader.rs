//! 内核加载模块

use std::sync::{Arc, Mutex};

use super::x86_boot::BootParams;
use vm_core::MMU;
use vm_core::{GuestAddr, MemoryError, VmError, VmResult};

/// 加载内核镜像到内存（同步版本，保留用于向后兼容）
pub fn load_kernel(
    mmu: Arc<Mutex<Box<dyn MMU>>>,
    data: &[u8],
    load_addr: GuestAddr,
) -> VmResult<()> {
    log::info!("=== Kernel Loading Debug ===");
    log::info!(
        "Loading kernel: load_addr={:#x}, size={} bytes",
        load_addr.0,
        data.len()
    );

    // Specifically log bytes at 0x44 (where corruption was detected)
    let debug_offset = 0x44usize;
    if data.len() > debug_offset + 4 {
        log::info!("Critical bytes at file offset 0x{:x}:", debug_offset);
        log::info!(
            "  File: {:02x} {:02x} {:02x} {:02x}",
            data[debug_offset],
            data[debug_offset + 1],
            data[debug_offset + 2],
            data[debug_offset + 3]
        );
    }

    let mut mmu_guard = mmu.lock().map_err(|_| {
        VmError::Memory(MemoryError::MmuLockFailed {
            message: "Failed to acquire MMU lock".to_string(),
        })
    })?;

    log::info!("About to call write_bulk with addr={:#x}", load_addr.0);
    let write_result = mmu_guard.write_bulk(load_addr, data);
    log::info!("write_bulk completed: {:?}", write_result);

    // Try to verify by reading back a few bytes
    if data.len() > debug_offset + 4 {
        let target = GuestAddr(load_addr.0 + debug_offset as u64);
        log::info!("Attempting to verify written data at {:#x}...", target.0);

        // Use read_bulk to read back
        let mut verify_buf = [0u8; 4];
        let read_result = mmu_guard.read_bulk(target, &mut verify_buf);

        match read_result {
            Ok(_) => {
                log::info!(
                    "  Read back: {:02x} {:02x} {:02x} {:02x}",
                    verify_buf[0],
                    verify_buf[1],
                    verify_buf[2],
                    verify_buf[3]
                );
                log::info!(
                    "  Expected:  {:02x} {:02x} {:02x} {:02x}",
                    data[debug_offset],
                    data[debug_offset + 1],
                    data[debug_offset + 2],
                    data[debug_offset + 3]
                );

                if verify_buf[0] != data[debug_offset]
                    || verify_buf[1] != data[debug_offset + 1]
                    || verify_buf[2] != data[debug_offset + 2]
                    || verify_buf[3] != data[debug_offset + 3]
                {
                    log::error!("❌ MEMORY CORRUPTION! Read bytes don't match file!");
                } else {
                    log::info!("✅ Verification passed!");
                }
            }
            Err(e) => {
                log::error!("Failed to read back for verification: {:?}", e);
            }
        }
    }

    write_result?;

    log::info!("=== Kernel Loading Complete ===");

    Ok(())
}

/// Load bzImage kernel and return entry point
///
/// This function handles Linux bzImage format by:
/// 1. Loading the kernel at the specified address
/// 2. Parsing the boot protocol header
/// 3. Returning the appropriate entry point (32-bit or 64-bit)
pub fn load_bzimage_kernel(
    mmu: Arc<Mutex<Box<dyn MMU>>>,
    data: &[u8],
    load_addr: GuestAddr,
) -> VmResult<GuestAddr> {
    let mut mmu_guard = mmu.lock().map_err(|_| {
        VmError::Memory(MemoryError::MmuLockFailed {
            message: "Failed to acquire MMU lock".to_string(),
        })
    })?;

    // Load kernel to memory
    mmu_guard.write_bulk(load_addr, data)?;

    // Try to parse as bzImage
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        BootParams::from_bzimage(&mut **mmu_guard, load_addr)
    }));

    let boot_params = match result {
        Ok(Ok(params)) => {
            log::info!("Successfully parsed bzImage boot protocol");
            Some(params)
        }
        _ => {
            log::warn!("Could not parse bzImage boot header, treating as raw kernel");
            None
        }
    };

    // Determine entry point
    let entry_point = if let Some(params) = boot_params {
        // Use bzImage entry point
        let code32_start = params.code32_start();

        // For modern 64-bit kernels, entry is usually at 0x100000 relative to load
        if code32_start == 0x100000 {
            log::info!("bzImage: 64-bit kernel detected, entry at load_addr + 0x100000");
            GuestAddr(load_addr.0 + 0x100000)
        } else {
            log::info!("bzImage: using entry point 0x{:x}", code32_start);
            GuestAddr(code32_start as u64)
        }
    } else {
        // Use load_addr as entry point
        log::info!("Raw kernel: using load_addr as entry point");
        load_addr
    };

    Ok(entry_point)
}

/// Load bzImage kernel properly with setup code and protected mode separation
///
/// bzImage format:
/// - Setup code (real mode): First (setup_sects + 1) * 512 bytes
/// - Protected mode code: Rest of the kernel
///
/// Loading addresses:
/// - Setup code -> 0x10000
/// - Protected mode -> 0x100000
pub fn load_bzimage_kernel_properly(
    mmu: Arc<Mutex<Box<dyn MMU>>>,
    data: &[u8],
    setup_load_addr: GuestAddr,
    pm_load_addr: GuestAddr,
) -> VmResult<GuestAddr> {
    log::info!("=== Loading bzImage Kernel Properly ===");
    log::info!("Total kernel size: {} bytes", data.len());
    log::info!("Setup load address: 0x{:08X}", setup_load_addr.0);
    log::info!("Protected mode load address: 0x{:08X}", pm_load_addr.0);

    // Read setup_sects from offset 0x1F1
    if data.len() < 0x1F1 + 1 {
        return Err(VmError::Memory(MemoryError::InvalidAddress(GuestAddr(
            0x1F1,
        ))));
    }

    let setup_sects = data[0x1F1] as usize;
    let setup_size = (setup_sects + 1) * 512; // +1 for the boot sector
    log::info!("Setup sectors: {} ({} bytes)", setup_sects, setup_size);

    // Validate setup size
    if setup_size > data.len() {
        log::warn!(
            "Setup size ({}) exceeds kernel size ({}), using entire kernel as setup",
            setup_size,
            data.len()
        );
        let adjusted_setup_size = data.len();
        let mut mmu_guard = mmu.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire MMU lock".to_string(),
            })
        })?;

        mmu_guard.write_bulk(setup_load_addr, data)?;
        log::info!(
            "Loaded entire kernel as setup code at 0x{:08X}",
            setup_load_addr.0
        );
        return Ok(setup_load_addr);
    }

    let protected_mode_size = data.len() - setup_size;
    log::info!("Protected mode code: {} bytes", protected_mode_size);

    let mut mmu_guard = mmu.lock().map_err(|_| {
        VmError::Memory(MemoryError::MmuLockFailed {
            message: "Failed to acquire MMU lock".to_string(),
        })
    })?;

    // Load setup code at 0x10000
    log::info!(
        "Loading setup code ({} bytes) at 0x{:08X}...",
        setup_size,
        setup_load_addr.0
    );
    mmu_guard.write_bulk(setup_load_addr, &data[..setup_size])?;
    log::info!("✓ Setup code loaded");

    // Load protected mode code at 0x100000
    log::info!(
        "Loading protected mode code ({} bytes) at 0x{:08X}...",
        protected_mode_size,
        pm_load_addr.0
    );
    mmu_guard.write_bulk(pm_load_addr, &data[setup_size..])?;
    log::info!("✓ Protected mode code loaded");

    log::info!("=== bzImage Loading Complete ===");
    log::info!("Entry point: 0x{:08X} (setup code)", setup_load_addr.0);

    Ok(setup_load_addr)
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
