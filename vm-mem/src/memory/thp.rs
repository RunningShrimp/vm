//! 透明大页(THP)支持
//!
//! 提供透明大页(THP)支持，减少TLB压力，提高内存访问性能。
//! THP是Linux内核的一种特性，可以自动将多个常规页面合并为大页，
//! 对应用程序透明，无需修改应用程序代码。
//!
//! ## 性能特点
//! - **减少TLB压力**: 大页可以覆盖更多内存，减少TLB未命中
//! - **提高内存访问速度**: 减少页表遍历开销
//! - **降低页表占用**: 减少页表项数量
//! - **对应用透明**: 无需修改应用程序代码
//!
//! ## 使用场景
//! - **虚拟机内存管理**: 为VM内存启用THP，提高虚拟机性能
//! - **大数据处理**: 处理大量数据的应用，如数据库、数据分析
//! - **科学计算**: 需要大量内存访问的计算密集型应用
//!
//! ## 平台支持
//! - **Linux**: 完整的THP支持
//! - **其他平台**: 回退到常规页面
//!
//! ## 示例
//!
//! ### 基本使用
//!
//! ```rust
//! use vm_mem::{init_global_thp_manager, allocate_with_thp, deallocate_with_thp, ThpPolicy};
//!
//! // 初始化THP管理器
//! init_global_thp_manager(ThpPolicy::Transparent)?;
//!
//! // 使用THP分配内存
//! let size = 2 * 1024 * 1024; // 2MB
//! let ptr = allocate_with_thp(size)?;
//!
//! if !ptr.is_null() {
//!     // 使用内存...
//!     unsafe {
//!         std::ptr::write_bytes(ptr, 0xCC, size);
//!     }
//!     
//!     // 释放内存
//!     deallocate_with_thp(ptr, size);
//! }
//! ```
//!
//! ### THP管理器使用
//!
//! ```rust
//! use vm_mem::{TransparentHugePageManager, ThpPolicy};
//!
//! // 创建THP管理器
//! let manager = TransparentHugePageManager::new(ThpPolicy::Transparent)?;
//!
//! // 检查THP可用性
//! if manager.is_thp_available() {
//!     println!("THP可用");
//! } else {
//!     println!("THP不可用");
//! }
//!
//! // 获取THP配置
//! let config = manager.get_thp_config()?;
//! println!("THP策略: {:?}", config.enabled);
//! println!("碎片整理: {}", config.defrag);
//! println!("使用零页: {}", config.use_zero_page);
//!
//! // 分配内存
//! let size = 4 * 1024 * 1024; // 4MB
//! let ptr = manager.allocate_with_thp(size)?;
//!
//! if !ptr.is_null() {
//!     // 检查是否使用了THP
//!     let is_thp = manager.is_thp_address(ptr);
//!     println!("使用THP: {}", is_thp);
//!     
//!     // 使用内存...
//!     
//!     // 释放内存
//!     manager.deallocate_thp(ptr, size);
//! }
//! ```
//!
//! ### 不同THP策略比较
//!
//! ```rust
//! use vm_mem::{TransparentHugePageManager, ThpPolicy};
//!
//! let policies = [
//!     (ThpPolicy::Always, "Always"),
//!     (ThpPolicy::Never, "Never"),
//!     (ThpPolicy::Madvise, "Madvise"),
//!     (ThpPolicy::Transparent, "Transparent"),
//! ];
//!
//! let test_size = 4 * 1024 * 1024; // 4MB
//!
//! for (policy, name) in &policies {
//!     println!("\n测试策略: {}", name);
//!     
//!     // 创建THP管理器
//!     match TransparentHugePageManager::new(*policy) {
//!         Ok(manager) => {
//!             // 测试分配
//!             match manager.allocate_with_thp(test_size) {
//!                 Ok(ptr) if !ptr.is_null() => {
//!                     println!("  分配成功: {:p}", ptr);
//!                     
//!                     // 检查是否使用了THP
//!                     let is_thp = manager.is_thp_address(ptr);
//!                     println!("  使用THP: {}", is_thp);
//!                     
//!                     // 释放内存
//!                     manager.deallocate_thp(ptr, test_size);
//!                 }
//!                 Ok(_) => {
//!                     println!("  分配失败: 返回空指针");
//!                 }
//!                 Err(e) => {
//!                     println!("  分配失败: {}", e);
//!                 }
//!             }
//!             
//!             // 打印统计信息
//!             let stats = manager.stats();
//!             println!("  THP分配: {}",
//!                 stats.thp_allocations.load(std::sync::atomic::Ordering::Relaxed));
//!             println!("  常规分配: {}",
//!                 stats.normal_allocations.load(std::sync::atomic::Ordering::Relaxed));
//!         }
//!         Err(e) => {
//!             println!("  创建THP管理器失败: {}", e);
//!         }
//!     }
//! }
//! ```
//!
//! ### THP性能测试
//!
//! ```rust
//! use vm_mem::{allocate_with_thp, deallocate_with_thp, init_global_thp_manager, ThpPolicy};
//! use std::time::Instant;
//!
//! // 初始化THP管理器
//! init_global_thp_manager(ThpPolicy::Transparent)?;
//!
//! let block_size = 2 * 1024 * 1024; // 2MB
//! let block_count = 100;
//!
//! // 测试THP分配性能
//! println!("测试THP分配性能...");
//! let thp_start = Instant::now();
//! let mut thp_ptrs = Vec::new();
//!
//! for _ in 0..block_count {
//!     match allocate_with_thp(block_size) {
//!         Ok(ptr) if !ptr.is_null() => {
//!             thp_ptrs.push(ptr);
//!         }
//!         _ => {}
//!     }
//! }
//!
//! let thp_alloc_time = thp_start.elapsed();
//!
//! // 使用内存
//! for &ptr in &thp_ptrs {
//!     unsafe {
//!         std::ptr::write_bytes(ptr, 0xDD, block_size);
//!     }
//! }
//!
//! // 释放内存
//! for ptr in thp_ptrs {
//!     deallocate_with_thp(ptr, block_size);
//! }
//!
//! // 测试常规分配性能
//! println!("测试常规分配性能...");
//! let regular_start = Instant::now();
//! let mut regular_ptrs = Vec::new();
//!
//! for _ in 0..block_count {
//!     let layout = std::alloc::Layout::from_size_align(block_size, 8)?;
//!     let ptr = unsafe { std::alloc::alloc(layout) };
//!     if !ptr.is_null() {
//!         regular_ptrs.push(ptr);
//!     }
//! }
//!
//! let regular_alloc_time = regular_start.elapsed();
//!
//! // 使用内存
//! for &ptr in &regular_ptrs {
//!     unsafe {
//!         std::ptr::write_bytes(ptr, 0xEE, block_size);
//!     }
//! }
//!
//! // 释放内存
//! for ptr in regular_ptrs {
//!     unsafe {
//!         let layout = match std::alloc::Layout::from_size_align(block_size, 8) {
//!             Ok(l) => l,
//!             Err(_) => return, // Invalid layout, skip deallocation
//!         };
//!         std::alloc::dealloc(ptr, layout);
//!     }
//! }
//!
//! // 打印性能比较结果
//! println!("性能比较结果:");
//! println!("  THP分配时间: {:?}", thp_alloc_time);
//! println!("  常规分配时间: {:?}", regular_alloc_time);
//!
//! if thp_alloc_time < regular_alloc_time {
//!     let speedup = regular_alloc_time.as_nanos() as f64 / thp_alloc_time.as_nanos() as f64;
//!     println!("  THP加速比: {:.2}x", speedup);
//! } else {
//!     println!("  THP未显示性能优势");
//! }
//! ```

use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::sync::OnceLock;

/// THP配置选项
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThpPolicy {
    /// 总是使用THP
    Always,
    /// 从不使用THP
    Never,
    /// 在内存压力下使用THP
    Madvise,
    /// 透明大页(默认)
    Transparent,
}

/// THP统计信息
#[derive(Debug, Default)]
pub struct ThpStats {
    /// THP命中次数
    pub thp_hits: std::sync::atomic::AtomicU64,
    /// THP未命中次数
    pub thp_misses: std::sync::atomic::AtomicU64,
    /// 分配的THP数量
    pub thp_allocations: std::sync::atomic::AtomicU64,
    /// 分配的常规页面数量
    pub normal_allocations: std::sync::atomic::AtomicU64,
}

impl Clone for ThpStats {
    fn clone(&self) -> Self {
        Self {
            thp_hits: std::sync::atomic::AtomicU64::new(
                self.thp_hits.load(std::sync::atomic::Ordering::Relaxed),
            ),
            thp_misses: std::sync::atomic::AtomicU64::new(
                self.thp_misses.load(std::sync::atomic::Ordering::Relaxed),
            ),
            thp_allocations: std::sync::atomic::AtomicU64::new(
                self.thp_allocations
                    .load(std::sync::atomic::Ordering::Relaxed),
            ),
            normal_allocations: std::sync::atomic::AtomicU64::new(
                self.normal_allocations
                    .load(std::sync::atomic::Ordering::Relaxed),
            ),
        }
    }
}

/// 透明大页管理器
pub struct TransparentHugePageManager {
    /// THP策略
    policy: ThpPolicy,
    /// THP统计
    stats: std::sync::Arc<ThpStats>,
    /// THP是否可用
    thp_available: bool,
}

impl TransparentHugePageManager {
    /// 创建THP管理器
    pub fn new(policy: ThpPolicy) -> io::Result<Self> {
        let thp_available = Self::check_thp_availability()?;

        Ok(Self {
            policy,
            stats: std::sync::Arc::new(ThpStats::default()),
            thp_available,
        })
    }

    /// 检查THP可用性
    pub fn check_thp_availability() -> io::Result<bool> {
        // 检查/sys/kernel/mm/transparent_hugepage/enabled
        let enabled_path = "/sys/kernel/mm/transparent_hugepage/enabled";
        if Path::new(enabled_path).exists() {
            let mut file = fs::File::open(enabled_path)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            Ok(contents.trim() == "[always]" || contents.trim() == "[madvise]")
        } else {
            // 如果文件不存在，假设THP不可用
            Ok(false)
        }
    }

    /// 获取THP配置信息
    pub fn get_thp_config() -> io::Result<ThpConfig> {
        let enabled_path = "/sys/kernel/mm/transparent_hugepage/enabled";
        let defrag_path = "/sys/kernel/mm/transparent_hugepage/defrag";
        let use_zero_page_path = "/sys/kernel/mm/transparent_hugepage/use_zero_page";

        let enabled = if Path::new(enabled_path).exists() {
            let mut file = fs::File::open(enabled_path)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            match contents.trim() {
                "[always]" => ThpPolicy::Always,
                "[never]" => ThpPolicy::Never,
                "[madvise]" => ThpPolicy::Madvise,
                _ => ThpPolicy::Transparent,
            }
        } else {
            ThpPolicy::Never
        };

        let defrag = if Path::new(defrag_path).exists() {
            let mut file = fs::File::open(defrag_path)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            contents.trim() == "1" || contents.trim() == "[always]"
        } else {
            false
        };

        let use_zero_page = if Path::new(use_zero_page_path).exists() {
            let mut file = fs::File::open(use_zero_page_path)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            contents.trim() == "1"
        } else {
            false
        };

        Ok(ThpConfig {
            enabled,
            defrag,
            use_zero_page,
        })
    }

    /// 启用THP的内存分配
    #[cfg(target_os = "linux")]
    pub fn allocate_with_thp(&self, size: usize) -> io::Result<*mut u8> {
        use std::ptr;

        if !self.thp_available {
            // THP不可用，回退到常规分配
            self.stats
                .normal_allocations
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return unsafe {
                Ok(std::alloc::alloc(
                    std::alloc::Layout::from_size_align_unchecked(size, 4096),
                ))
            };
        }

        match self.policy {
            ThpPolicy::Never => {
                self.stats
                    .normal_allocations
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                unsafe {
                    Ok(std::alloc::alloc(
                        std::alloc::Layout::from_size_align_unchecked(size, 4096),
                    ))
                }
            }
            ThpPolicy::Always | ThpPolicy::Transparent => {
                // 使用mmap分配内存，让内核自动使用THP
                let flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS;
                let prot = libc::PROT_READ | libc::PROT_WRITE;
                let addr = unsafe { libc::mmap(ptr::null_mut(), size, prot, flags, -1, 0) };

                if addr == libc::MAP_FAILED {
                    self.stats
                        .normal_allocations
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    unsafe {
                        Ok(std::alloc::alloc(
                            std::alloc::Layout::from_size_align_unchecked(size, 4096),
                        ))
                    }
                } else {
                    self.stats
                        .thp_allocations
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    Ok(addr as *mut u8)
                }
            }
            ThpPolicy::Madvise => {
                // 先使用常规分配，然后使用madvise建议使用THP
                let flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS;
                let prot = libc::PROT_READ | libc::PROT_WRITE;
                let addr = unsafe { libc::mmap(ptr::null_mut(), size, prot, flags, -1, 0) };

                if addr == libc::MAP_FAILED {
                    self.stats
                        .normal_allocations
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    unsafe {
                        Ok(std::alloc::alloc(
                            std::alloc::Layout::from_size_align_unchecked(size, 4096),
                        ))
                    }
                } else {
                    // 建议内核使用THP
                    let result = unsafe {
                        libc::madvise(addr as *mut libc::c_void, size, libc::MADV_HUGEPAGE)
                    };
                    if result == 0 {
                        self.stats
                            .thp_allocations
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    } else {
                        self.stats
                            .normal_allocations
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    Ok(addr as *mut u8)
                }
            }
        }
    }

    /// 非Linux平台的THP分配
    #[cfg(not(target_os = "linux"))]
    pub fn allocate_with_thp(&self, size: usize) -> io::Result<*mut u8> {
        // 非Linux平台，回退到常规分配
        self.stats
            .normal_allocations
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        unsafe {
            Ok(std::alloc::alloc(
                std::alloc::Layout::from_size_align_unchecked(size, 4096),
            ))
        }
    }

    /// 释放THP内存
    #[cfg(target_os = "linux")]
    pub fn deallocate_thp(&self, ptr: *mut u8, size: usize) {
        if !ptr.is_null() {
            unsafe {
                libc::munmap(ptr as *mut libc::c_void, size);
            }
        }
    }

    /// 非Linux平台的THP释放
    ///
    /// # Safety
    ///
    /// Callers must ensure:
    /// - `ptr` must point to a memory region previously allocated by this allocator
    /// - `size` must match the size used for allocation
    /// - The memory region must not be freed twice
    #[cfg(not(target_os = "linux"))]
    pub unsafe fn deallocate_thp(&self, ptr: *mut u8, size: usize) {
        unsafe {
            if !ptr.is_null() {
                let layout = std::alloc::Layout::from_size_align_unchecked(size, 4096);
                std::alloc::dealloc(ptr, layout);
            }
        }
    }

    /// 获取THP统计信息
    pub fn stats(&self) -> &ThpStats {
        &self.stats
    }

    /// 获取THP策略
    pub fn policy(&self) -> ThpPolicy {
        self.policy
    }

    /// 检查THP是否可用
    pub fn is_thp_available(&self) -> bool {
        self.thp_available
    }

    /// 检查地址是否使用THP
    #[cfg(target_os = "linux")]
    pub fn is_thp_address(addr: *const u8) -> bool {
        use std::fs;
        use std::io::{self, Read};

        // 读取/proc/self/pagemap来检查页面大小
        let pagemap_path = "/proc/self/pagemap";
        if !Path::new(pagemap_path).exists() {
            return false;
        }

        let addr_val = addr as usize;
        let page_index = addr_val / 4096;
        let offset = page_index * 8; // 每个条目8字节

        match fs::File::open(pagemap_path) {
            Ok(mut file) => {
                if file.seek(io::SeekFrom::Start(offset as u64)).is_ok() {
                    let mut buffer = [0u8; 8];
                    if file.read_exact(&mut buffer).is_ok() {
                        let pagemap_entry = u64::from_le_bytes(buffer);
                        // 检查页面大小位(第61-63位)
                        let page_size = (pagemap_entry >> 61) & 0x3;
                        return page_size != 0; // 非零表示使用了大页
                    }
                }
            }
            Err(_) => {}
        }

        false
    }

    /// 非Linux平台的THP地址检查
    #[cfg(not(target_os = "linux"))]
    pub fn is_thp_address(_addr: *const u8) -> bool {
        false
    }

    /// 获取THP使用统计
    #[cfg(target_os = "linux")]
    pub fn get_thp_usage_stats() -> io::Result<ThpUsageStats> {
        let stats_path = "/proc/meminfo";
        if !Path::new(stats_path).exists() {
            return Ok(ThpUsageStats::default());
        }

        let mut file = fs::File::open(stats_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let mut stats = ThpUsageStats::default();

        for line in contents.lines() {
            if line.starts_with("AnonHugePages:") {
                stats.anon_huge_pages = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            } else if line.starts_with("ShmemHugePages:") {
                stats.shmem_huge_pages = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            } else if line.starts_with("HugePages_Total:") {
                stats.huge_pages_total = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            } else if line.starts_with("HugePages_Free:") {
                stats.huge_pages_free = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            } else if line.starts_with("HugePages_Rsvd:") {
                stats.huge_pages_reserved = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            } else if line.starts_with("Hugepagesize:") {
                stats.huge_page_size = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(2048); // 默认2MB
            }
        }

        Ok(stats)
    }

    /// 非Linux平台的THP使用统计
    #[cfg(not(target_os = "linux"))]
    pub fn get_thp_usage_stats() -> io::Result<ThpUsageStats> {
        Ok(ThpUsageStats::default())
    }
}

/// THP配置信息
#[derive(Debug, Clone)]
pub struct ThpConfig {
    /// THP策略
    pub enabled: ThpPolicy,
    /// 是否启用碎片整理
    pub defrag: bool,
    /// 是否使用零页
    pub use_zero_page: bool,
}

/// THP使用统计
#[derive(Debug, Default, Clone)]
pub struct ThpUsageStats {
    /// 匿名大页数量
    pub anon_huge_pages: u64,
    /// 共享内存大页数量
    pub shmem_huge_pages: u64,
    /// 总大页数量
    pub huge_pages_total: u64,
    /// 空闲大页数量
    pub huge_pages_free: u64,
    /// 保留大页数量
    pub huge_pages_reserved: u64,
    /// 大页大小(kB)
    pub huge_page_size: u64,
}

/// 全局THP管理器
static GLOBAL_THP_MANAGER: OnceLock<TransparentHugePageManager> = OnceLock::new();

/// 初始化全局THP管理器
pub fn init_global_thp_manager(policy: ThpPolicy) -> io::Result<()> {
    // 检查是否已经初始化
    if GLOBAL_THP_MANAGER.get().is_some() {
        // 已经初始化，直接返回
        return Ok(());
    }

    // 创建THP管理器
    let manager = TransparentHugePageManager::new(policy)?;

    // 设置全局THP管理器
    GLOBAL_THP_MANAGER.set(manager).map_err(|_| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "THP manager already initialized",
        )
    })
}

/// 获取全局THP管理器
pub fn get_global_thp_manager() -> Option<&'static TransparentHugePageManager> {
    GLOBAL_THP_MANAGER.get()
}

/// 使用THP分配内存的便利函数
pub fn allocate_with_thp(size: usize) -> io::Result<*mut u8> {
    if let Some(manager) = get_global_thp_manager() {
        manager.allocate_with_thp(size)
    } else {
        // THP管理器未初始化，使用常规分配
        unsafe {
            Ok(std::alloc::alloc(
                std::alloc::Layout::from_size_align_unchecked(size, 4096),
            ))
        }
    }
}

/// 使用THP释放内存的便利函数
///
/// # Safety
///
/// Callers must ensure:
/// - `ptr` must point to a memory region previously allocated by this allocator
/// - `size` must match the size used for allocation
/// - The memory region must not be freed twice
pub unsafe fn deallocate_with_thp(ptr: *mut u8, size: usize) {
    unsafe {
        if let Some(manager) = get_global_thp_manager() {
            manager.deallocate_thp(ptr, size);
        } else {
            // THP管理器未初始化，使用常规释放
            if !ptr.is_null() {
                let layout = std::alloc::Layout::from_size_align_unchecked(size, 4096);
                std::alloc::dealloc(ptr, layout);
            }
        }
    }
}

/// 检查地址是否使用THP的便利函数
pub fn is_thp_address(addr: *const u8) -> bool {
    TransparentHugePageManager::is_thp_address(addr)
}

/// 获取THP使用统计的便利函数
pub fn get_thp_usage_stats() -> io::Result<ThpUsageStats> {
    TransparentHugePageManager::get_thp_usage_stats()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thp_availability() {
        // 测试THP可用性检查
        let available = TransparentHugePageManager::check_thp_availability().unwrap_or(false);
        println!("THP available: {}", available);
    }

    #[test]
    fn test_thp_config() {
        // 测试THP配置获取
        let config = TransparentHugePageManager::get_thp_config().unwrap_or(ThpConfig {
            enabled: ThpPolicy::Never,
            defrag: false,
            use_zero_page: false,
        });

        println!("THP config: {:?}", config);
    }

    #[test]
    fn test_thp_allocation() {
        // 测试THP分配
        let manager = match TransparentHugePageManager::new(ThpPolicy::Transparent) {
            Ok(mgr) => mgr,
            Err(e) => {
                println!("Failed to create THP manager: {}, skipping test", e);
                return;
            }
        };

        let sizes = [4096, 65536, 1048576]; // 4KB, 64KB, 1MB

        for &size in &sizes {
            match manager.allocate_with_thp(size) {
                Ok(ptr) if !ptr.is_null() => {
                    println!("Allocated {} bytes with THP: {:p}", size, ptr);

                    // 检查是否使用了THP
                    let is_thp = TransparentHugePageManager::is_thp_address(ptr);
                    println!("  Uses THP: {}", is_thp);

                    // 释放内存
                    unsafe {
                        manager.deallocate_thp(ptr, size);
                    }
                }
                Ok(_) => {
                    println!("Failed to allocate {} bytes with THP", size);
                }
                Err(e) => {
                    println!("Error allocating {} bytes: {}", size, e);
                }
            }
        }

        // 打印统计信息
        let stats = manager.stats();
        println!("THP stats: {:?}", stats);
    }
}
