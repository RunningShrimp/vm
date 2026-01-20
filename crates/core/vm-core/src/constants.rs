//! VM系统常量
//!
//! 定义虚拟机和内存管理相关的常量，避免硬编码和重复。

/// 默认内存大小：64 MB
pub const DEFAULT_MEMORY_SIZE: usize = 64 * 1024 * 1024;

/// 页面大小：4 KB（标准x86和RISC-V页面大小）
pub const PAGE_SIZE: usize = 4096;

/// 页面大小掩码（用于地址对齐检查）
pub const PAGE_OFFSET_MASK: u64 = PAGE_SIZE as u64 - 1;

/// 最大访客内存：4 GB
pub const MAX_GUEST_MEMORY: usize = 4 * 1024 * 1024 * 1024;

/// TLB默认大小：256条目
pub const DEFAULT_TLB_SIZE: usize = 256;

/// TLB最大大小：1024条目
pub const MAX_TLB_SIZE: usize = 1024;

/// 最小TLB大小：16条目
pub const MIN_TLB_SIZE: usize = 16;

/// 默认TLB关联度：4路组相联
pub const DEFAULT_TLB_ASSOCIATIVITY: usize = 4;

/// JIT代码缓存默认大小：32 MB
pub const DEFAULT_CODE_CACHE_SIZE: usize = 32 * 1024 * 1024;

/// 最大JIT代码缓存大小：256 MB
pub const MAX_CODE_CACHE_SIZE: usize = 256 * 1024 * 1024;

/// 最小JIT代码缓存大小：1 MB
pub const MIN_CODE_CACHE_SIZE: usize = 1024 * 1024;

/// 快速TLB（L0 TLB）大小：4条目
pub const FAST_TLB_SIZE: usize = 4;

/// L1指令缓存大小：32 KB
pub const L1_INSTRUCTION_CACHE_SIZE: usize = 32 * 1024;

/// L1数据缓存大小：32 KB
pub const L1_DATA_CACHE_SIZE: usize = 32 * 1024;

/// L2统一缓存大小：256 KB
pub const L2_CACHE_SIZE: usize = 256 * 1024;

/// L3缓存大小：8 MB
pub const L3_CACHE_SIZE: usize = 8 * 1024 * 1024;

/// 默认栈大小：1 MB
pub const DEFAULT_STACK_SIZE: usize = 1024 * 1024;

/// 最大栈大小：16 MB
pub const MAX_STACK_SIZE: usize = 16 * 1024 * 1024;

/// 最小栈大小：4 KB
pub const MIN_STACK_SIZE: usize = 4 * 1024;

/// 默认堆大小：16 MB
pub const DEFAULT_HEAP_SIZE: usize = 16 * 1024 * 1024;

/// 最大堆大小：1 GB
pub const MAX_HEAP_SIZE: usize = 1024 * 1024 * 1024;

/// 最小堆大小：1 MB
pub const MIN_HEAP_SIZE: usize = 1024 * 1024;

/// MMAP区域基地址（RISC-V标准）
pub const MMAP_BASE_ADDR: u64 = 0x1000_0000;

/// 栈基地址（RISC-V标准）
pub const STACK_BASE_ADDR: u64 = 0x7FFF_FFFF_F000;

/// 动态链接器基地址
pub const LD_BASE_ADDR: u64 = 0x3000_0000;

/// 默认时间片长度：10000个指令
pub const DEFAULT_TIME_SLICE: u64 = 10_000;

/// 最小时间片长度：1000个指令
pub const MIN_TIME_SLICE: u64 = 1_000;

/// 最大时间片长度：100000个指令
pub const MAX_TIME_SLICE: u64 = 100_000;

/// 内存对齐要求：8字节
pub const MEMORY_ALIGNMENT: usize = 8;

/// 指令对齐要求：2或4字节（取决于架构）
pub const INSTRUCTION_ALIGNMENT: usize = 2;

/// 最大中断向量数：256
pub const MAX_INTERRUPT_VECTORS: usize = 256;

/// 默认中断优先级：中等级别
pub const DEFAULT_INTERRUPT_PRIORITY: u8 = 128;

/// VCPU热插超时：5秒
pub const VCPU_HOTPLUG_TIMEOUT_SECS: u64 = 5;

/// 设备热插超时：10秒
pub const DEVICE_HOTPLUG_TIMEOUT_SECS: u64 = 10;

/// 快照压缩级别：默认中等压缩
pub const SNAPSHOT_COMPRESSION_LEVEL: u32 = 6;

/// 快照版本：1
pub const SNAPSHOT_VERSION: u32 = 1;

/// AOT缓存版本：1
pub const AOT_CACHE_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_size_is_power_of_two() {
        assert!(PAGE_SIZE.is_power_of_two());
    }

    #[test]
    fn test_default_memory_size_is_aligned() {
        assert_eq!(DEFAULT_MEMORY_SIZE % PAGE_SIZE, 0);
    }

    #[test]
    fn test_max_guest_memory_is_aligned() {
        assert_eq!(MAX_GUEST_MEMORY % PAGE_SIZE, 0);
    }

    #[test]
    fn test_tlb_sizes_are_valid() {
        const { assert!(MIN_TLB_SIZE <= DEFAULT_TLB_SIZE) }
        const { assert!(DEFAULT_TLB_SIZE <= MAX_TLB_SIZE) }
        assert!(DEFAULT_TLB_SIZE.is_power_of_two());
    }

    #[test]
    fn test_cache_sizes_are_aligned() {
        assert_eq!(L1_INSTRUCTION_CACHE_SIZE % PAGE_SIZE, 0);
        assert_eq!(L1_DATA_CACHE_SIZE % PAGE_SIZE, 0);
        assert_eq!(L2_CACHE_SIZE % PAGE_SIZE, 0);
    }

    #[test]
    fn test_stack_sizes_are_valid() {
        const { assert!(MIN_STACK_SIZE <= DEFAULT_STACK_SIZE) }
        const { assert!(DEFAULT_STACK_SIZE <= MAX_STACK_SIZE) }
        assert_eq!(DEFAULT_STACK_SIZE % PAGE_SIZE, 0);
    }

    #[test]
    fn test_heap_sizes_are_valid() {
        const { assert!(MIN_HEAP_SIZE <= DEFAULT_HEAP_SIZE) }
        const { assert!(DEFAULT_HEAP_SIZE <= MAX_HEAP_SIZE) }
        assert_eq!(DEFAULT_HEAP_SIZE % PAGE_SIZE, 0);
    }

    #[test]
    fn test_code_cache_sizes_are_valid() {
        const { assert!(MIN_CODE_CACHE_SIZE <= DEFAULT_CODE_CACHE_SIZE) }
        const { assert!(DEFAULT_CODE_CACHE_SIZE <= MAX_CODE_CACHE_SIZE) }
        assert_eq!(DEFAULT_CODE_CACHE_SIZE % PAGE_SIZE, 0);
    }

    #[test]
    fn test_time_slice_range() {
        const { assert!(MIN_TIME_SLICE <= DEFAULT_TIME_SLICE) }
        const { assert!(DEFAULT_TIME_SLICE <= MAX_TIME_SLICE) }
    }

    #[test]
    fn test_memory_alignment() {
        assert!(MEMORY_ALIGNMENT.is_power_of_two());
        assert_eq!(MEMORY_ALIGNMENT, 8);
    }
}
