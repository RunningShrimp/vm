//! 内存管理模块
//!
//! 提供内存分配、页表遍历和内存池管理功能：
//! - 内存池：高效的内存分配和回收
//! - NUMA分配器：针对NUMA架构优化的内存分配
//! - 页表遍历：RISC-V SV39/SV48页表遍历实现
//! - 对齐内存：ARM64 NEON优化的16字节对齐内存 (Round 35)

pub mod memory_pool;
pub mod numa_allocator;
pub mod page_table_walker;
pub mod thp;
#[cfg(feature = "opt-simd")]
pub mod aligned;

// 重新导出主要类型
pub use memory_pool::*;
pub use numa_allocator::*;
pub use page_table_walker::*;
pub use thp::*;
#[cfg(feature = "opt-simd")]
pub use aligned::*;
