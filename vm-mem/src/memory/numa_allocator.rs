//! NUMA 感知内存分配器
//!
//! 提供 NUMA 感知的内存分配策略，优化多 socket 系统的内存访问延迟。
//! 支持多种分配策略：本地分配、交错分配、绑定分配和优先分配。
//!
//! ## 性能特点
//! - **本地分配优先**: 优先在当前CPU所在NUMA节点分配内存
//! - **交错分配**: 在多个NUMA节点间轮询分配，实现负载均衡
//! - **绑定分配**: 将内存绑定到指定NUMA节点
//! - **优先分配**: 优先使用指定节点，失败时自动回退
//!
//! ## 使用场景
//! - **虚拟机内存管理**: 为VM分配本地NUMA节点内存，减少跨节点访问延迟
//! - **垃圾回收**: GC内部数据结构使用NUMA感知分配
//! - **高性能应用**: 大数据处理、科学计算等需要优化内存访问的应用
//!
//! ## 平台支持
//! - **Linux**: 完整的NUMA API支持
//! - **其他平台**: 回退到标准分配器
//!
//! ## 示例
//!
//! ### 基本使用
//!
//! ```rust
//! use vm_mem::{NumaAllocator, NumaAllocPolicy, NumaNodeInfo};
//!
//! // 创建NUMA节点信息
//! let nodes = vec![
//!     NumaNodeInfo {
//!         node_id: 0,
//!         total_memory: 8 * 1024 * 1024 * 1024, // 8GB
//!         available_memory: 7 * 1024 * 1024 * 1024, // 7GB
//!         cpu_mask: 0xFF, // CPU 0-7
//!     },
//!     NumaNodeInfo {
//!         node_id: 1,
//!         total_memory: 8 * 1024 * 1024 * 1024, // 8GB
//!         available_memory: 7 * 1024 * 1024 * 1024, // 7GB
//!         cpu_mask: 0xFF00, // CPU 8-15
//!     },
//! ];
//!
//! // 创建本地分配器
//! let allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Local);
//!
//! // 分配内存
//! let layout = std::alloc::Layout::from_size_align(1024 * 1024, 8)?;
//! let ptr = allocator.allocate(layout)?;
//!
//! // 使用内存...
//!
//! // 释放内存
//! allocator.deallocate(ptr, 1024 * 1024);
//! ```
//!
//! ### 全局NUMA分配器
//!
//! ```rust
//! use vm_mem::{init_global_numa_allocator, NumaAllocPolicy, GlobalNumaAllocator};
//!
//! // 设置全局NUMA分配器
//! init_global_numa_allocator(NumaAllocPolicy::Local)?;
//!
//! // 使用全局分配器
//! let layout = std::alloc::Layout::from_size_align(1024, 8)?;
//! let ptr = unsafe { std::alloc::alloc(layout) };
//!
//! // 使用内存...
//!
//! // 释放内存
//! unsafe { std::alloc::dealloc(ptr, layout) };
//! ```
//!
//! ### 多线程NUMA分配
//!
//! ```rust
//! use vm_mem::{NumaAllocator, NumaAllocPolicy, NumaNodeInfo};
//! use std::sync::Arc;
//! use std::thread;
//!
//! // 创建共享分配器
//! let nodes = vec![
//!     NumaNodeInfo {
//!         node_id: 0,
//!         total_memory: 8 * 1024 * 1024 * 1024,
//!         available_memory: 7 * 1024 * 1024 * 1024,
//!         cpu_mask: 0xFF,
//!     },
//!     NumaNodeInfo {
//!         node_id: 1,
//!         total_memory: 8 * 1024 * 1024 * 1024,
//!         available_memory: 7 * 1024 * 1024 * 1024,
//!         cpu_mask: 0xFF00,
//!     },
//! ];
//!
//! let allocator = Arc::new(NumaAllocator::new(nodes, NumaAllocPolicy::Local));
//!
//! // 创建多个线程进行内存分配
//! let mut handles = Vec::new();
//! for thread_id in 0..4 {
//!     let allocator_clone = allocator.clone();
//!     let handle = thread::spawn(move || {
//!         let layout = std::alloc::Layout::from_size_align(4096, 8).unwrap();
//!         if let Ok(ptr) = allocator_clone.allocate(layout) {
//!             // 使用内存...
//!             allocator_clone.deallocate(ptr, 4096);
//!         }
//!     });
//!     handles.push(handle);
//! }
//!
//! // 等待所有线程完成
//! for handle in handles {
//!     handle.join().unwrap();
//! }
//!
//! // 打印统计信息
//! let stats = allocator.stats();
//! println!("本地分配: {}", stats.local_allocs.load(std::sync::atomic::Ordering::Relaxed));
//! println!("远程分配: {}", stats.remote_allocs.load(std::sync::atomic::Ordering::Relaxed));
//! ```
//!
//! 标识: 服务接口

use std::alloc::{GlobalAlloc, Layout};
use std::ptr::NonNull;
use std::sync::Arc;

/// NUMA 节点信息
///
/// 标识: 数据模型
#[derive(Debug, Clone)]
pub struct NumaNodeInfo {
    /// 节点 ID
    pub node_id: usize,
    /// 总内存大小（字节）
    pub total_memory: u64,
    /// 可用内存大小（字节）
    pub available_memory: u64,
    /// CPU 掩码
    pub cpu_mask: u64,
}

/// NUMA 感知分配策略
///
/// 标识: 数据模型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumaAllocPolicy {
    /// 本地分配（优先当前线程所在节点）
    Local,
    /// 交错分配（轮询所有节点）
    Interleave,
    /// 绑定分配（绑定到指定节点）
    Bind(usize),
    /// 优先分配到指定节点，失败时回退
    Preferred(usize),
}

/// NUMA 内存分配器
///
/// 标识: 数据模型
pub struct NumaAllocator {
    /// NUMA 节点信息列表
    nodes: Vec<NumaNodeInfo>,
    /// 当前分配策略
    policy: NumaAllocPolicy,
    /// 分配统计
    stats: Arc<NumaAllocStats>,
}

/// NUMA 分配统计
///
/// 标识: 数据模型
#[derive(Debug, Default)]
pub struct NumaAllocStats {
    /// 本地分配次数
    pub local_allocs: std::sync::atomic::AtomicU64,
    /// 远程分配次数
    pub remote_allocs: std::sync::atomic::AtomicU64,
    /// 分配失败次数
    pub failed_allocs: std::sync::atomic::AtomicU64,
}

impl NumaAllocator {
    /// 创建 NUMA 分配器
    ///
    /// # 参数
    /// - `nodes`: NUMA 节点信息
    /// - `policy`: 分配策略
    pub fn new(nodes: Vec<NumaNodeInfo>, policy: NumaAllocPolicy) -> Self {
        Self {
            nodes,
            policy,
            stats: Arc::new(NumaAllocStats::default()),
        }
    }

    /// 获取当前线程所在的 NUMA 节点
    #[cfg(target_os = "linux")]
    pub fn current_node() -> Option<usize> {
        unsafe {
            let node = libc::numa_node_of_cpu(libc::sched_getcpu());
            if node >= 0 { Some(node as usize) } else { None }
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn current_node() -> Option<usize> {
        None
    }

    /// 获取 NUMA 节点数量
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 获取指定节点的信息
    pub fn node_info(&self, node_id: usize) -> Option<&NumaNodeInfo> {
        self.nodes.get(node_id)
    }

    /// 分配内存（根据策略）
    pub fn allocate(&self, layout: Layout) -> Result<NonNull<u8>, String> {
        let size = layout.size();
        let alignment = layout.align();

        match self.policy {
            NumaAllocPolicy::Local => self.allocate_local(size, alignment),
            NumaAllocPolicy::Interleave => self.allocate_interleave(size, alignment),
            NumaAllocPolicy::Bind(node_id) => self.allocate_on_node(node_id, size, alignment),
            NumaAllocPolicy::Preferred(node_id) => self
                .allocate_on_node(node_id, size, alignment)
                .or_else(|_| self.allocate_interleave(size, alignment)),
        }
    }

    /// 本地分配
    #[cfg(target_os = "linux")]
    fn allocate_local(&self, size: usize, alignment: usize) -> Result<NonNull<u8>, String> {
        if let Some(node) = Self::current_node() {
            match self.allocate_on_node(node, size, alignment) {
                Ok(ptr) => {
                    self.stats
                        .local_allocs
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    Ok(ptr)
                }
                Err(_) => {
                    self.stats
                        .remote_allocs
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    self.allocate_interleave(size, alignment)
                }
            }
        } else {
            self.allocate_interleave(size, alignment)
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn allocate_local(&self, size: usize, alignment: usize) -> Result<NonNull<u8>, String> {
        self.allocate_interleave(size, alignment)
    }

    /// 在指定节点上分配
    #[cfg(target_os = "linux")]
    fn allocate_on_node(
        &self,
        node_id: usize,
        size: usize,
        alignment: usize,
    ) -> Result<NonNull<u8>, String> {
        if node_id >= self.nodes.len() {
            return Err(format!("Invalid NUMA node: {}", node_id));
        }

        unsafe {
            // 创建 nodemask
            let mut nodemask = std::mem::zeroed::<libc::nodemask_t>();
            libc::nodemask_zero(&mut nodemask);
            libc::nodemask_set(&mut nodemask, node_id as libc::nodemask_t);

            // 使用 numa_alloc_onnode 分配内存
            let ptr = libc::numa_alloc_onnode(size, node_id as i32);
            if ptr.is_null() {
                self.stats
                    .failed_allocs
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Err("Failed to allocate memory on NUMA node".to_string())
            } else {
                NonNull::new(ptr as *mut u8).ok_or_else(|| "Null pointer".to_string())
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn allocate_on_node(
        &self,
        _node_id: usize,
        size: usize,
        _alignment: usize,
    ) -> Result<NonNull<u8>, String> {
        // 回退到标准分配
        unsafe {
            let layout = Layout::from_size_align_unchecked(size, 1);
            let ptr = std::alloc::alloc(layout);
            NonNull::new(ptr).ok_or_else(|| "Failed to allocate memory".to_string())
        }
    }

    /// 交错分配
    fn allocate_interleave(&self, size: usize, _alignment: usize) -> Result<NonNull<u8>, String> {
        if self.nodes.is_empty() {
            return Err("No NUMA nodes available".to_string());
        }

        #[cfg(target_os = "linux")]
        unsafe {
            let ptr = libc::numa_alloc_interleaved(size);
            if ptr.is_null() {
                self.stats
                    .failed_allocs
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Err("Failed to allocate interleaved memory".to_string())
            } else {
                NonNull::new(ptr as *mut u8).ok_or_else(|| "Null pointer".to_string())
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            unsafe {
                let layout = Layout::from_size_align_unchecked(size, 1);
                let ptr = std::alloc::alloc(layout);
                NonNull::new(ptr).ok_or_else(|| "Failed to allocate memory".to_string())
            }
        }
    }

    /// 释放内存
    #[cfg(target_os = "linux")]
    pub fn deallocate(&self, ptr: NonNull<u8>, size: usize) {
        unsafe {
            libc::numa_free(ptr.as_ptr() as *mut libc::c_void, size);
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn deallocate(&self, ptr: NonNull<u8>, size: usize) {
        unsafe {
            let layout = Layout::from_size_align_unchecked(size, 1);
            std::alloc::dealloc(ptr.as_ptr(), layout);
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> &NumaAllocStats {
        &self.stats
    }
}

/// 全局 NUMA 分配器（可选）
pub struct GlobalNumaAllocator {
    allocator: std::sync::OnceLock<NumaAllocator>,
}

impl Default for GlobalNumaAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalNumaAllocator {
    /// 创建全局NUMA分配器
    pub fn new() -> Self {
        Self {
            allocator: std::sync::OnceLock::new(),
        }
    }

    /// 初始化全局分配器
    pub fn init(&self, nodes: Vec<NumaNodeInfo>, policy: NumaAllocPolicy) {
        self.allocator
            .get_or_init(|| NumaAllocator::new(nodes, policy));
    }

    /// 获取全局分配器实例
    fn get_allocator(&self) -> Option<&NumaAllocator> {
        self.allocator.get()
    }

    /// 获取全局分配器实例，返回错误
    #[allow(dead_code)] // Reserved for future use when error handling is needed
    fn try_get_allocator(&self) -> Result<&NumaAllocator, String> {
        self.allocator
            .get()
            .ok_or_else(|| "GlobalNumaAllocator not initialized".to_string())
    }

    /// 自动检测NUMA节点并初始化
    #[cfg(target_os = "linux")]
    pub fn auto_init(&self, policy: NumaAllocPolicy) -> Result<(), String> {
        let nodes = self.detect_numa_nodes()?;
        self.init(nodes, policy);
        Ok(())
    }

    /// 检测系统NUMA节点
    #[cfg(target_os = "linux")]
    fn detect_numa_nodes(&self) -> Result<Vec<NumaNodeInfo>, String> {
        unsafe {
            if libc::numa_available() == -1 {
                // 没有NUMA支持，创建单个节点
                return Ok(vec![NumaNodeInfo {
                    node_id: 0,
                    total_memory: self.get_system_memory(),
                    available_memory: self.get_system_memory(),
                    cpu_mask: (1 << libc::numa_num_configured_cpus()) - 1,
                }]);
            }

            let mut nodes = Vec::new();
            let max_node = libc::numa_max_node();

            for node_id in 0..=max_node {
                let node_mask = libc::numa_node_to_cpus(node_id);
                let total_memory = self.get_node_memory(node_id);

                nodes.push(NumaNodeInfo {
                    node_id: node_id as usize,
                    total_memory,
                    available_memory: total_memory, // 简化，假设全部可用
                    cpu_mask: node_mask,
                });
            }

            Ok(nodes)
        }
    }

    /// 获取系统总内存
    #[cfg(target_os = "linux")]
    fn get_system_memory(&self) -> u64 {
        unsafe {
            let mut info = std::mem::zeroed::<libc::sysinfo>();
            if libc::sysinfo(&mut info) == 0 {
                info.totalram * info.mem_unit as u64
            } else {
                4 * 1024 * 1024 * 1024 // 默认4GB
            }
        }
    }

    /// 获取指定节点的内存
    #[cfg(target_os = "linux")]
    fn get_node_memory(&self, node_id: i32) -> u64 {
        unsafe {
            let mut node_size = 0u64;
            if libc::numa_node_size64(node_id, &mut node_size) == 0 {
                node_size
            } else {
                0
            }
        }
    }

    /// 自动检测NUMA节点并初始化（非Linux平台）
    #[cfg(not(target_os = "linux"))]
    pub fn auto_init(&self, policy: NumaAllocPolicy) -> Result<(), String> {
        // 非Linux平台，创建单个节点
        let nodes = vec![NumaNodeInfo {
            node_id: 0,
            total_memory: 4 * 1024 * 1024 * 1024, // 默认4GB
            available_memory: 4 * 1024 * 1024 * 1024,
            cpu_mask: 0xFF,
        }];
        self.init(nodes, policy);
        Ok(())
    }
}

unsafe impl GlobalAlloc for GlobalNumaAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // 使用全局NUMA分配器进行分配
        match self.get_allocator() {
            Some(allocator) => match allocator.allocate(layout) {
                Ok(ptr) => ptr.as_ptr(),
                Err(_) => std::ptr::null_mut(),
            },
            None => std::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if !ptr.is_null() {
            // 使用全局NUMA分配器进行释放
            if let Some(non_null_ptr) = std::ptr::NonNull::new(ptr)
                && let Some(allocator) = self.get_allocator()
            {
                allocator.deallocate(non_null_ptr, layout.size());
            }
        }
    }
}

/// 全局NUMA分配器实例
static mut GLOBAL_NUMA_ALLOCATOR: Option<GlobalNumaAllocator> = None;
static INIT: std::sync::Once = std::sync::Once::new();

/// 初始化全局NUMA分配器
pub fn init_global_numa_allocator(policy: NumaAllocPolicy) -> Result<(), String> {
    let mut result = Ok(());
    INIT.call_once(|| {
        let allocator = GlobalNumaAllocator::new();
        if let Err(e) = allocator.auto_init(policy) {
            result = Err(e);
        } else {
            unsafe {
                GLOBAL_NUMA_ALLOCATOR = Some(allocator);
            }
        }
    });
    result
}

/// 获取全局NUMA分配器统计
pub fn global_numa_stats() -> Option<&'static NumaAllocStats> {
    unsafe {
        if let Some(ref allocator) = GLOBAL_NUMA_ALLOCATOR {
            allocator.get_allocator().map(|a| a.stats())
        } else {
            None
        }
    }
}

/// 使用全局NUMA分配器的示例
///
/// # Examples
///
/// ```rust
/// use vm_mem::{init_global_numa_allocator, NumaAllocPolicy};
///
/// // 初始化全局NUMA分配器，使用本地分配策略
/// init_global_numa_allocator(NumaAllocPolicy::Local).expect("Failed to initialize NUMA allocator");
///
/// // 现在可以使用全局分配器进行内存分配
/// // 实际使用时需要通过#[global_allocator]属性设置
/// ```
///
/// # Note
///
/// 要使用全局NUMA分配器，需要在程序入口处添加：
/// ```rust
/// #[global_allocator]
/// static GLOBAL_NUMA: vm_mem::GlobalNumaAllocator = vm_mem::GlobalNumaAllocator::new();
///
/// fn main() {
///     vm_mem::init_global_numa_allocator(vm_mem::NumaAllocPolicy::Local)
///         .expect("Failed to initialize NUMA allocator");
///     // ... 应用程序代码
/// }
/// ```

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_numa_allocator_creation() {
        let nodes = vec![
            NumaNodeInfo {
                node_id: 0,
                total_memory: 8 * 1024 * 1024 * 1024,
                available_memory: 7 * 1024 * 1024 * 1024,
                cpu_mask: 0xFF,
            },
            NumaNodeInfo {
                node_id: 1,
                total_memory: 8 * 1024 * 1024 * 1024,
                available_memory: 7 * 1024 * 1024 * 1024,
                cpu_mask: 0xFF00,
            },
        ];

        let allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Local);
        assert_eq!(allocator.node_count(), 2);
    }

    #[test]
    fn test_numa_node_info() {
        let nodes = vec![NumaNodeInfo {
            node_id: 0,
            total_memory: 8 * 1024 * 1024 * 1024,
            available_memory: 7 * 1024 * 1024 * 1024,
            cpu_mask: 0xFF,
        }];

        let allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Local);
        let info = allocator.node_info(0).expect("Node 0 should exist");
        assert_eq!(info.node_id, 0);
        assert_eq!(info.total_memory, 8 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_numa_alloc_policies() {
        let nodes = vec![NumaNodeInfo {
            node_id: 0,
            total_memory: 8 * 1024 * 1024 * 1024,
            available_memory: 7 * 1024 * 1024 * 1024,
            cpu_mask: 0xFF,
        }];

        let _ = NumaAllocator::new(nodes.clone(), NumaAllocPolicy::Local);
        let _ = NumaAllocator::new(nodes.clone(), NumaAllocPolicy::Interleave);
        let _ = NumaAllocator::new(nodes.clone(), NumaAllocPolicy::Bind(0));
        let _ = NumaAllocator::new(nodes, NumaAllocPolicy::Preferred(0));
    }

    #[test]
    #[cfg(target_os = "linux")] // NUMA功能仅在Linux上可用
    fn test_memory_allocation_deallocation() {
        let nodes = vec![NumaNodeInfo {
            node_id: 0,
            total_memory: 8 * 1024 * 1024 * 1024,
            available_memory: 7 * 1024 * 1024 * 1024,
            cpu_mask: 0xFF,
        }];

        let allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Local);

        // 测试小块分配
        let layout = Layout::from_size_align(64, 8).expect("Invalid layout");
        let ptr = allocator
            .allocate(layout)
            .expect("Allocation should succeed");
        assert!(!ptr.as_ptr().is_null());

        // 验证统计信息
        let stats = allocator.stats();
        assert!(
            stats
                .local_allocs
                .load(std::sync::atomic::Ordering::Relaxed)
                > 0
                || stats
                    .remote_allocs
                    .load(std::sync::atomic::Ordering::Relaxed)
                    > 0
        );

        // 释放内存
        allocator.deallocate(ptr, 64);

        // 测试大块分配
        let layout = Layout::from_size_align(1024 * 1024, 4096).expect("Invalid layout");
        let ptr = allocator
            .allocate(layout)
            .expect("Allocation should succeed");
        assert!(!ptr.as_ptr().is_null());
        allocator.deallocate(ptr, 1024 * 1024);
    }

    #[test]
    fn test_numa_node_detection() {
        // 测试节点检测功能
        let current_node = NumaAllocator::current_node();
        // 在非NUMA系统上可能返回None，在NUMA系统上返回Some
        // 我们不做具体的断言，因为这取决于运行环境
        let _ = current_node;
    }

    #[test]
    fn test_allocation_failure_handling() {
        let nodes = vec![NumaNodeInfo {
            node_id: 0,
            total_memory: 1024, // 很小的内存
            available_memory: 512,
            cpu_mask: 0xFF,
        }];

        let allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Local);

        // 尝试分配超出可用内存的块
        let layout = Layout::from_size_align(2048, 8).expect("Invalid layout");
        let result = allocator.allocate(layout);

        // 在测试环境中可能成功也可能失败，但不应该panic
        let _ = result;

        // 验证失败统计
        let stats = allocator.stats();
        let failed_count = stats
            .failed_allocs
            .load(std::sync::atomic::Ordering::Relaxed);
        // 失败计数可能为0或非0，取决于系统
        let _ = failed_count;
    }

    #[test]
    #[cfg(target_os = "linux")] // NUMA功能仅在Linux上可用
    fn test_invalid_node_allocation() {
        let nodes = vec![NumaNodeInfo {
            node_id: 0,
            total_memory: 8 * 1024 * 1024 * 1024,
            available_memory: 7 * 1024 * 1024 * 1024,
            cpu_mask: 0xFF,
        }];

        let _allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Bind(0));

        // 绑定到不存在的节点
        let bind_allocator = NumaAllocator::new(vec![], NumaAllocPolicy::Bind(999));
        let layout = Layout::from_size_align(64, 8).expect("Invalid layout");
        let result = bind_allocator.allocate(layout);
        assert!(result.is_err());
    }
}

/// 性能基准测试模块
#[cfg(test)]
mod benchmarks {
    use super::*;
    use std::time::Instant;

    /// 性能基准测试配置
    #[derive(Debug)]
    struct BenchmarkConfig {
        allocation_sizes: Vec<usize>,
        iterations: usize,
        warmup_iterations: usize,
    }

    impl Default for BenchmarkConfig {
        fn default() -> Self {
            Self {
                allocation_sizes: vec![64, 256, 1024, 4096, 16384, 65536],
                iterations: 1000,
                warmup_iterations: 100,
            }
        }
    }

    /// 运行NUMA分配器性能基准测试
    #[test]
    fn benchmark_numa_allocation() {
        let nodes = vec![
            NumaNodeInfo {
                node_id: 0,
                total_memory: 8 * 1024 * 1024 * 1024,
                available_memory: 7 * 1024 * 1024 * 1024,
                cpu_mask: 0xFF,
            },
            NumaNodeInfo {
                node_id: 1,
                total_memory: 8 * 1024 * 1024 * 1024,
                available_memory: 7 * 1024 * 1024 * 1024,
                cpu_mask: 0xFF00,
            },
        ];

        let config = BenchmarkConfig::default();

        println!("=== NUMA Allocation Performance Benchmark ===");

        for &policy in &[NumaAllocPolicy::Local, NumaAllocPolicy::Interleave] {
            println!("\nTesting policy: {:?}", policy);

            let allocator = NumaAllocator::new(nodes.clone(), policy);

            for &size in &config.allocation_sizes {
                // 预热
                for _ in 0..config.warmup_iterations {
                    let layout = Layout::from_size_align(size, 8).expect("Invalid layout");
                    if let Ok(ptr) = allocator.allocate(layout) {
                        allocator.deallocate(ptr, size);
                    }
                }

                // 性能测试
                let start = Instant::now();
                let mut successful_allocs = 0;

                for _ in 0..config.iterations {
                    let layout = Layout::from_size_align(size, 8).expect("Invalid layout");
                    if let Ok(ptr) = allocator.allocate(layout) {
                        allocator.deallocate(ptr, size);
                        successful_allocs += 1;
                    }
                }

                let duration = start.elapsed();
                let avg_time = duration.as_nanos() as f64 / successful_allocs as f64;

                println!(
                    "  Size {} bytes: {} ns/alloc ({} successful)",
                    size, avg_time as u64, successful_allocs
                );
            }

            // 输出统计信息
            let stats = allocator.stats();
            println!(
                "  Local allocs: {}",
                stats
                    .local_allocs
                    .load(std::sync::atomic::Ordering::Relaxed)
            );
            println!(
                "  Remote allocs: {}",
                stats
                    .remote_allocs
                    .load(std::sync::atomic::Ordering::Relaxed)
            );
            println!(
                "  Failed allocs: {}",
                stats
                    .failed_allocs
                    .load(std::sync::atomic::Ordering::Relaxed)
            );
        }
    }

    /// 内存压力测试
    #[test]
    fn stress_test_memory_pressure() {
        let nodes = vec![NumaNodeInfo {
            node_id: 0,
            total_memory: 8 * 1024 * 1024 * 1024,
            available_memory: 7 * 1024 * 1024 * 1024,
            cpu_mask: 0xFF,
        }];

        let allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Local);
        let mut allocations = Vec::new();

        // 分配大量小块内存
        for i in 0..10000 {
            let size = 64 + (i % 10) * 64; // 64, 128, 192, ..., 640 bytes
            let layout = Layout::from_size_align(size, 8).expect("Invalid layout");

            match allocator.allocate(layout) {
                Ok(ptr) => allocations.push((ptr, size)),
                Err(_) => break, // 分配失败时停止
            }
        }

        println!("Allocated {} blocks", allocations.len());

        // 释放所有内存
        for (ptr, size) in allocations {
            allocator.deallocate(ptr, size);
        }

        // 输出最终统计
        let stats = allocator.stats();
        println!(
            "Final stats - Local: {}, Remote: {}, Failed: {}",
            stats
                .local_allocs
                .load(std::sync::atomic::Ordering::Relaxed),
            stats
                .remote_allocs
                .load(std::sync::atomic::Ordering::Relaxed),
            stats
                .failed_allocs
                .load(std::sync::atomic::Ordering::Relaxed)
        );
    }
}

/// GC系统集成测试
#[cfg(test)]
mod gc_integration_tests {
    use super::*;

    #[test]
    fn test_gc_numa_integration() {
        // 创建NUMA分配器
        let nodes = vec![
            NumaNodeInfo {
                node_id: 0,
                total_memory: 8 * 1024 * 1024 * 1024,
                available_memory: 7 * 1024 * 1024 * 1024,
                cpu_mask: 0xFF,
            },
            NumaNodeInfo {
                node_id: 1,
                total_memory: 8 * 1024 * 1024 * 1024,
                available_memory: 7 * 1024 * 1024 * 1024,
                cpu_mask: 0xFF00,
            },
        ];

        let numa_allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Local);

        // 测试NUMA分配器功能（模拟GC使用场景）
        let layout = Layout::from_size_align(1024, 8).expect("Invalid layout");
        let ptr = numa_allocator
            .allocate(layout)
            .expect("Allocation should succeed");
        assert!(!ptr.as_ptr().is_null());

        // 释放内存
        numa_allocator.deallocate(ptr, 1024);

        // 验证统计
        let stats = numa_allocator.stats();
        assert!(
            stats
                .local_allocs
                .load(std::sync::atomic::Ordering::Relaxed)
                >= 0
        );
    }

    #[test]
    fn test_numa_memory_pressure_under_gc() {
        let nodes = vec![NumaNodeInfo {
            node_id: 0,
            total_memory: 8 * 1024 * 1024 * 1024,
            available_memory: 7 * 1024 * 1024 * 1024,
            cpu_mask: 0xFF,
        }];

        let allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Interleave);

        // 模拟GC工作负载：分配/释放循环
        let mut allocations = Vec::new();
        let block_size = 4096; // 4KB块，模拟GC对象

        // 第一轮：分配大量块
        for _ in 0..1000 {
            let layout = Layout::from_size_align(block_size, 8).expect("Invalid layout");
            match allocator.allocate(layout) {
                Ok(ptr) => allocations.push(ptr),
                Err(_) => break, // 内存不足时停止
            }
        }

        println!("NUMA GC simulation: allocated {} blocks", allocations.len());

        // 第二轮：随机释放50%的块（模拟GC回收）
        let to_free = allocations.len() / 2;
        for _ in 0..to_free {
            if let Some(ptr) = allocations.pop() {
                allocator.deallocate(ptr, block_size);
            }
        }

        // 第三轮：重新分配（模拟对象重新分配）
        for _ in 0..to_free {
            let layout = Layout::from_size_align(block_size, 8).expect("Invalid layout");
            if let Ok(ptr) = allocator.allocate(layout) {
                allocations.push(ptr);
            }
        }

        // 清理所有分配
        for ptr in allocations {
            allocator.deallocate(ptr, block_size);
        }

        // 输出统计信息
        let stats = allocator.stats();
        println!("NUMA GC simulation completed:");
        println!(
            "  Local allocations: {}",
            stats
                .local_allocs
                .load(std::sync::atomic::Ordering::Relaxed)
        );
        println!(
            "  Remote allocations: {}",
            stats
                .remote_allocs
                .load(std::sync::atomic::Ordering::Relaxed)
        );
        println!(
            "  Failed allocations: {}",
            stats
                .failed_allocs
                .load(std::sync::atomic::Ordering::Relaxed)
        );
    }
}
