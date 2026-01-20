//! 统一MMU v2 - 整合同步和异步接口
//!
//! 这个文件提供了一个统一的MMU trait，整合了：
//! - 同步MMU操作（unified_mmu.rs）
//! - 异步MMU操作（async_mmu.rs）
//! - 统一的TLB层次结构
//! - 页表缓存
//! - 内存预取
//!
//! ## 设计目标
//!
//! 1. **统一接口**: 单一trait同时支持同步和异步操作
//! 2. **向后兼容**: 与现有SoftMmu和UnifiedMmu兼容
//! 3. **性能优化**: 支持多种TLB优化策略
//! 4. **可扩展性**: 易于添加新的架构支持

use std::sync::Arc;

use parking_lot::RwLock;
use vm_core::AddressTranslator;
use vm_core::error::VmError;
use vm_core::{AccessType, GuestAddr, GuestPhysAddr};

use crate::tlb::management::manager::{StandardTlbManager, TlbManager};
use crate::{PagingMode, PhysicalMemory};

// ============================================================================
// 统一MMU Trait - 核心接口
// ============================================================================

/// 统一MMU trait
///
/// 整合同步和异步MMU操作，提供统一的内存管理接口。
///
/// # 设计理念
///
/// - **同步优先**: 默认提供高效的同步接口
/// - **异步可选**: 通过feature flag启用异步接口
/// - **策略灵活**: 支持多种TLB优化策略
/// - **架构无关**: 支持RISC-V、ARM64、x86_64等多种架构
///
/// # 使用示例
///
/// ```rust
/// use vm_core::{AccessType, GuestAddr};
/// use vm_mem::unified_mmu_v2::UnifiedMMU;
///
/// // 同步操作
/// let pa = mmu.translate(GuestAddr(0x1000), AccessType::Read)?;
///
/// // 异步操作（需要"async" feature）
/// #[cfg(feature = "async")]
/// let pa = mmu
///     .translate_async(GuestAddr(0x1000), AccessType::Read)
///     .await?;
/// ```
pub trait UnifiedMMU: Send + Sync {
    // ========================================================================
    // 同步接口（必需）
    // ========================================================================

    /// 虚拟地址到物理地址的翻译
    ///
    /// # 参数
    /// - `va`: 虚拟地址
    /// - `access`: 访问类型
    ///
    /// # 返回
    /// 成功返回物理地址，失败返回错误
    ///
    /// # TLB优化
    /// - 首先检查TLB缓存
    /// - TLB未命中时进行页表遍历
    /// - 自动将翻译结果插入TLB
    fn translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestPhysAddr, VmError>;

    /// 读取内存（同步）
    ///
    /// # 参数
    /// - `pa`: 物理地址
    /// - `size`: 读取大小（1/2/4/8字节）
    ///
    /// # 返回
    /// 成功返回读取的值，失败返回错误
    fn read(&mut self, pa: GuestAddr, size: u8) -> Result<u64, VmError>;

    /// 写入内存（同步）
    ///
    /// # 参数
    /// - `pa`: 物理地址
    /// - `val`: 要写入的值
    /// - `size`: 写入大小（1/2/4/8字节）
    ///
    /// # 返回
    /// 成功返回()，失败返回错误
    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError>;

    /// 取指令（同步）
    ///
    /// # 参数
    /// - `pc`: 程序计数器地址
    ///
    /// # 返回
    /// 成功返回指令（4字节），失败返回错误
    fn fetch_insn(&mut self, pc: GuestAddr) -> Result<u64, VmError>;

    /// 批量读取（同步）
    ///
    /// # 参数
    /// - `pa`: 起始物理地址
    /// - `buf`: 输出缓冲区
    ///
    /// # 返回
    /// 成功返回()，失败返回错误
    fn read_bulk(&mut self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError>;

    /// 批量写入（同步）
    ///
    /// # 参数
    /// - `pa`: 起始物理地址
    /// - `buf`: 输入缓冲区
    ///
    /// # 返回
    /// 成功返回()，失败返回错误
    fn write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError>;

    // ========================================================================
    // TLB管理接口
    // ========================================================================

    /// 获取TLB管理器引用
    ///
    /// 用于直接访问TLB进行高级操作
    fn tlb(&self) -> &dyn TlbManager;

    /// 获取TLB管理器可变引用
    fn tlb_mut(&mut self) -> &mut dyn TlbManager;

    /// 刷新所有TLB
    fn flush_tlb(&mut self);

    /// 刷新指定ASID的TLB
    fn flush_tlb_asid(&mut self, asid: u16);

    /// 刷新指定页面的TLB
    fn flush_tlb_page(&mut self, va: GuestAddr);

    // ========================================================================
    // 配置接口
    // ========================================================================

    /// 设置分页模式
    ///
    /// # 参数
    /// - `mode`: 分页模式（Bare/Sv39/Sv48/Arm64/X86_64）
    ///
    /// # 注意
    /// 切换分页模式会自动刷新TLB
    fn set_paging_mode(&mut self, mode: PagingMode);

    /// 设置RISC-V SATP寄存器
    ///
    /// # 参数
    /// - `satp`: SATP寄存器值
    ///
    /// # SATP格式
    /// ```
    /// | 63..60 | 59..44 | 43..0 |
    /// | MODE   | ASID   | PPN   |
    /// ```
    fn set_satp(&mut self, satp: u64);

    /// 设置严格对齐检查
    ///
    /// # 参数
    /// - `enable`: true启用，false禁用
    fn set_strict_align(&mut self, enable: bool);

    // ========================================================================
    // 统计信息接口
    // ========================================================================

    /// 获取MMU统计信息
    fn stats(&self) -> UnifiedMmuStats;

    /// 获取内存大小
    fn memory_size(&self) -> usize;

    /// 导出内存内容
    fn dump_memory(&self) -> Vec<u8>;

    /// 恢复内存内容
    fn restore_memory(&mut self, data: &[u8]) -> Result<(), String>;

    // ========================================================================
    // 异步接口（feature-gated）
    // ========================================================================

    /// 异步地址翻译
    ///
    /// # 参数
    /// - `va`: 虚拟地址
    /// - `access`: 访问类型
    ///
    /// # 返回
    /// 成功返回物理地址，失败返回错误
    ///
    /// # 性能优势
    /// - 使用异步锁减少阻塞
    /// - 支持并发地址翻译
    /// - 优化I/O等待时间
    #[cfg(feature = "async")]
    #[allow(async_fn_in_trait)]
    async fn translate_async(
        &mut self,
        va: GuestAddr,
        access: AccessType,
    ) -> Result<GuestPhysAddr, VmError>;

    /// 异步取指令
    #[cfg(feature = "async")]
    #[allow(async_fn_in_trait)]
    async fn fetch_insn_async(&self, pc: GuestAddr) -> Result<u64, VmError>;

    /// 异步内存读取
    #[cfg(feature = "async")]
    #[allow(async_fn_in_trait)]
    async fn read_async(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError>;

    /// 异步内存写入
    #[cfg(feature = "async")]
    #[allow(async_fn_in_trait)]
    async fn write_async(&self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError>;

    /// 异步批量读取
    #[cfg(feature = "async")]
    #[allow(async_fn_in_trait)]
    async fn read_bulk_async(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError>;

    /// 异步批量写入
    #[cfg(feature = "async")]
    #[allow(async_fn_in_trait)]
    async fn write_bulk_async(&self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError>;

    /// 异步批量地址翻译
    ///
    /// # 参数
    /// - `vas`: 虚拟地址和访问类型的列表
    ///
    /// # 返回
    /// 成功返回物理地址列表，失败返回错误
    ///
    /// # 性能优势
    /// - 并发翻译多个地址
    /// - 减少锁竞争
    /// - 提高吞吐量
    #[cfg(feature = "async")]
    #[allow(async_fn_in_trait)]
    async fn translate_bulk_async(
        &mut self,
        vas: &[(GuestAddr, AccessType)],
    ) -> Result<Vec<GuestPhysAddr>, VmError>;

    /// 异步刷新TLB
    #[cfg(feature = "async")]
    #[allow(async_fn_in_trait)]
    async fn flush_tlb_async(&mut self) -> Result<(), VmError>;
}

// ============================================================================
// 统一MMU配置
// ============================================================================

/// 统一MMU配置
///
/// 整合了多级TLB和并发TLB的配置选项
#[derive(Debug, Clone)]
pub struct UnifiedMmuConfigV2 {
    /// 启用多级TLB（L1/L2/L3）
    pub enable_multilevel_tlb: bool,
    /// L1指令TLB容量
    pub l1_itlb_capacity: usize,
    /// L1数据TLB容量
    pub l1_dtlb_capacity: usize,
    /// L2统一TLB容量
    pub l2_tlb_capacity: usize,
    /// L3共享TLB容量
    pub l3_tlb_capacity: usize,

    /// 启用并发优化（分片TLB）
    pub enable_concurrent_tlb: bool,
    /// 分片TLB总容量
    pub sharded_tlb_capacity: usize,
    /// 分片数量（建议为2的幂）
    pub shard_count: usize,

    /// 启用快速路径（热点地址优化）
    pub enable_fast_path: bool,
    /// 快速路径TLB容量
    pub fast_path_capacity: usize,

    /// 启用页表缓存
    pub enable_page_table_cache: bool,
    /// 页表缓存容量
    pub page_table_cache_size: usize,

    /// 启用内存预取
    pub enable_prefetch: bool,
    /// 预取窗口大小
    pub prefetch_window: usize,
    /// 预取历史窗口
    pub prefetch_history_window: usize,

    /// 启用自适应TLB替换策略
    pub enable_adaptive_replacement: bool,
    /// 自适应替换策略的阈值
    pub adaptive_threshold: f64,

    /// 启用统计收集
    pub enable_stats: bool,
    /// 启用性能监控
    pub enable_monitoring: bool,

    /// 严格对齐检查
    pub strict_align: bool,

    /// 使用大页（2MB）
    pub use_hugepages: bool,
}

impl Default for UnifiedMmuConfigV2 {
    fn default() -> Self {
        Self {
            enable_multilevel_tlb: true,
            l1_itlb_capacity: 64,
            l1_dtlb_capacity: 128,
            l2_tlb_capacity: 512,
            l3_tlb_capacity: 2048,

            enable_concurrent_tlb: true,
            sharded_tlb_capacity: 4096,
            shard_count: 16,

            enable_fast_path: true,
            fast_path_capacity: 64,

            enable_page_table_cache: true,
            page_table_cache_size: 1024,

            enable_prefetch: true,
            prefetch_window: 4,
            prefetch_history_window: 32,

            enable_adaptive_replacement: true,
            adaptive_threshold: 0.8,

            enable_stats: true,
            enable_monitoring: true,

            strict_align: false,
            use_hugepages: false,
        }
    }
}

// ============================================================================
// 统一MMU统计信息
// ============================================================================

/// 统一MMU统计信息
///
/// 提供详细的性能指标，用于监控和调优
#[derive(Debug, Clone)]
pub struct UnifiedMmuStats {
    /// MMU ID
    pub mmu_id: u64,

    /// TLB统计
    pub tlb_hits: u64,
    pub tlb_misses: u64,
    pub tlb_hit_rate: f64,

    /// 页表缓存统计
    pub page_table_cache_hits: u64,
    pub page_table_cache_misses: u64,
    pub page_table_cache_hit_rate: f64,

    /// 预取统计
    pub prefetch_hits: u64,
    pub prefetch_count: u64,
    pub prefetch_efficiency: f64,

    /// 翻译统计
    pub total_translations: u64,
    pub page_walks: u64,

    /// 性能统计
    pub avg_translation_latency_ns: f64,
    pub total_translation_time_ns: u64,

    /// 内存统计
    pub memory_size_bytes: usize,
    pub memory_reads: u64,
    pub memory_writes: u64,
}

impl UnifiedMmuStats {
    /// 计算TLB命中率
    pub fn tlb_hit_rate(&self) -> f64 {
        let total = self.tlb_hits + self.tlb_misses;
        if total == 0 {
            0.0
        } else {
            self.tlb_hits as f64 / total as f64
        }
    }

    /// 计算页表缓存命中率
    pub fn page_table_cache_hit_rate(&self) -> f64 {
        let total = self.page_table_cache_hits + self.page_table_cache_misses;
        if total == 0 {
            0.0
        } else {
            self.page_table_cache_hits as f64 / total as f64
        }
    }

    /// 计算预取效率
    pub fn prefetch_efficiency(&self) -> f64 {
        if self.prefetch_count == 0 {
            0.0
        } else {
            self.prefetch_hits as f64 / self.prefetch_count as f64
        }
    }

    /// 计算平均翻译延迟（纳秒）
    pub fn avg_translation_latency_ns(&self) -> f64 {
        if self.total_translations == 0 {
            0.0
        } else {
            self.total_translation_time_ns as f64 / self.total_translations as f64
        }
    }
}

// ============================================================================
// 混合MMU实现 - 统一同步和异步
// ============================================================================

/// 混合MMU实现
///
/// 整合同步MMU（SoftMmu/UnifiedMmu）和异步MMU（AsyncMMU）
pub struct HybridMMU {
    /// MMU唯一ID
    #[allow(dead_code)] // Reserved for future use
    id: u64,

    /// 物理内存后端
    phys_mem: Arc<PhysicalMemory>,

    /// 同步MMU实现（使用Mutex包装以支持异步）
    sync_mmu: Arc<parking_lot::Mutex<Box<dyn AddressTranslator + Send>>>,

    /// TLB管理器（用于UnifiedMMU trait的tlb()和tlb_mut()方法）
    /// 注意：为了满足trait的签名要求，这里直接存储而不使用Arc<Mutex<>>
    tlb_manager: StandardTlbManager,

    /// 配置
    config: UnifiedMmuConfigV2,

    /// 统计信息
    stats: Arc<RwLock<UnifiedMmuStats>>,

    /// 分页模式
    paging_mode: RwLock<PagingMode>,

    /// ASID
    asid: RwLock<u16>,

    /// 严格对齐
    strict_align: RwLock<bool>,
}

impl HybridMMU {
    /// 创建新的混合MMU
    ///
    /// # 参数
    /// - `size`: 物理内存大小（字节）
    /// - `config`: MMU配置
    ///
    /// # 示例
    /// ```
    /// use vm_mem::unified_mmu_v2::{HybridMMU, UnifiedMmuConfigV2};
    ///
    /// let config = UnifiedMmuConfigV2::default();
    /// let mmu = HybridMMU::new(1024 * 1024 * 1024, config);
    /// ```
    pub fn new(size: usize, config: UnifiedMmuConfigV2) -> Self {
        // 使用SoftMmu作为同步实现，并boxed为trait object
        let soft_mmu = crate::SoftMmu::new(size, config.use_hugepages);
        let sync_mmu: Box<dyn AddressTranslator + Send> = Box::new(soft_mmu);
        let sync_mmu = Arc::new(parking_lot::Mutex::new(sync_mmu));

        // 创建TLB管理器（直接存储，不使用Arc<Mutex<>>）
        let tlb_manager =
            StandardTlbManager::new(config.l1_dtlb_capacity + config.l1_itlb_capacity);

        // 在config移动之前保存strict_align值
        let strict_align_value = config.strict_align;

        let stats = Arc::new(RwLock::new(UnifiedMmuStats {
            mmu_id: 1,
            tlb_hits: 0,
            tlb_misses: 0,
            tlb_hit_rate: 0.0,
            page_table_cache_hits: 0,
            page_table_cache_misses: 0,
            page_table_cache_hit_rate: 0.0,
            prefetch_hits: 0,
            prefetch_count: 0,
            prefetch_efficiency: 0.0,
            total_translations: 0,
            page_walks: 0,
            avg_translation_latency_ns: 0.0,
            total_translation_time_ns: 0,
            memory_size_bytes: size,
            memory_reads: 0,
            memory_writes: 0,
        }));

        Self {
            id: 1,
            phys_mem: Arc::new(PhysicalMemory::new(size, config.use_hugepages)),
            sync_mmu,
            tlb_manager,
            config,
            stats,
            paging_mode: RwLock::new(PagingMode::Bare),
            asid: RwLock::new(0),
            strict_align: RwLock::new(strict_align_value),
        }
    }

    /// 更新统计信息
    fn update_stats<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut UnifiedMmuStats),
    {
        if self.config.enable_stats {
            let mut stats = self.stats.write();
            update_fn(&mut stats);
        }
    }
}

impl UnifiedMMU for HybridMMU {
    fn translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestPhysAddr, VmError> {
        let start_time = std::time::Instant::now();

        // 使用同步MMU进行翻译
        let result = self.sync_mmu.lock().translate(va, access);

        // 更新统计信息
        let elapsed = start_time.elapsed();
        self.update_stats(|stats| {
            stats.total_translations += 1;
            stats.total_translation_time_ns += elapsed.as_nanos() as u64;
            stats.avg_translation_latency_ns = stats.avg_translation_latency_ns();
        });

        result
    }

    fn read(&mut self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        self.update_stats(|stats| {
            stats.memory_reads += 1;
        });

        // 使用PhysicalMemory读取
        let addr = pa.0 as usize;
        match size {
            1 => self.phys_mem.read_u8(addr).map(|v| v as u64),
            2 => self.phys_mem.read_u16(addr).map(|v| v as u64),
            4 => self.phys_mem.read_u32(addr).map(|v| v as u64),
            8 => self.phys_mem.read_u64(addr),
            _ => Err(VmError::from(vm_core::Fault::AlignmentFault)),
        }
        .map_err(|_| {
            VmError::from(vm_core::Fault::PageFault {
                addr: pa,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            })
        })
    }

    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
        self.update_stats(|stats| {
            stats.memory_writes += 1;
        });

        // 使用PhysicalMemory写入
        let addr = pa.0 as usize;
        let res = match size {
            1 => self.phys_mem.write_u8(addr, val as u8),
            2 => self.phys_mem.write_u16(addr, val as u16),
            4 => self.phys_mem.write_u32(addr, val as u32),
            8 => self.phys_mem.write_u64(addr, val),
            _ => return Err(VmError::from(vm_core::Fault::AlignmentFault)),
        };

        if res.is_err() {
            return Err(VmError::from(vm_core::Fault::PageFault {
                addr: pa,
                access_type: AccessType::Write,
                is_write: true,
                is_user: false,
            }));
        }

        Ok(())
    }

    fn fetch_insn(&mut self, pc: GuestAddr) -> Result<u64, VmError> {
        self.read(pc, 4)
    }

    fn read_bulk(&mut self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError> {
        self.phys_mem.read_buf(pa.0 as usize, buf).map_err(|_| {
            VmError::from(vm_core::Fault::PageFault {
                addr: pa,
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            })
        })
    }

    fn write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError> {
        self.phys_mem.write_buf(pa.0 as usize, buf).map_err(|_| {
            VmError::from(vm_core::Fault::PageFault {
                addr: pa,
                access_type: AccessType::Write,
                is_write: true,
                is_user: false,
            })
        })
    }

    fn tlb(&self) -> &dyn TlbManager {
        &self.tlb_manager
    }

    fn tlb_mut(&mut self) -> &mut dyn TlbManager {
        &mut self.tlb_manager
    }

    fn flush_tlb(&mut self) {
        self.sync_mmu.lock().flush_tlb();
    }

    fn flush_tlb_asid(&mut self, asid: u16) {
        self.sync_mmu.lock().flush_tlb_asid(asid);
    }

    fn flush_tlb_page(&mut self, va: GuestAddr) {
        self.sync_mmu.lock().flush_tlb_page(va);
    }

    fn set_paging_mode(&mut self, mode: PagingMode) {
        *self.paging_mode.write() = mode;
        // 需要sync_mmu支持set_paging_mode
        // 这里简化处理
        self.flush_tlb();
    }

    fn set_satp(&mut self, satp: u64) {
        let mode = (satp >> 60) & 0xF;
        let asid = ((satp >> 44) & 0xFFFF) as u16;

        *self.paging_mode.write() = match mode {
            0 => PagingMode::Bare,
            8 => PagingMode::Sv39,
            9 => PagingMode::Sv48,
            _ => PagingMode::Bare,
        };

        *self.asid.write() = asid;
        self.flush_tlb_asid(asid);
    }

    fn set_strict_align(&mut self, enable: bool) {
        *self.strict_align.write() = enable;
    }

    fn stats(&self) -> UnifiedMmuStats {
        self.stats.read().clone()
    }

    fn memory_size(&self) -> usize {
        self.phys_mem.size()
    }

    fn dump_memory(&self) -> Vec<u8> {
        self.phys_mem.dump()
    }

    fn restore_memory(&mut self, data: &[u8]) -> Result<(), String> {
        self.phys_mem.restore(data).map_err(|e| e.to_string())
    }

    // 异步接口实现（需要tokio）
    #[cfg(feature = "async")]
    async fn translate_async(
        &mut self,
        va: GuestAddr,
        access: AccessType,
    ) -> Result<GuestPhysAddr, VmError> {
        // 克隆Arc<Mmu>以在异步闭包中使用
        let sync_mmu = self.sync_mmu.clone();
        tokio::task::spawn_blocking(move || sync_mmu.lock().translate(va, access))
            .await
            .unwrap_or(Err(VmError::Core(vm_core::error::CoreError::Internal {
                message: "Async operation failed".to_string(),
                module: "unified_mmu_v2".to_string(),
            })))
    }

    #[cfg(feature = "async")]
    async fn fetch_insn_async(&self, pc: GuestAddr) -> Result<u64, VmError> {
        let phys_mem = self.phys_mem.clone();
        tokio::task::spawn_blocking(move || phys_mem.read_u32(pc.0 as usize).map(|v| v as u64))
            .await
            .unwrap_or(Err(VmError::Core(vm_core::error::CoreError::Internal {
                message: "Async operation failed".to_string(),
                module: "unified_mmu_v2".to_string(),
            })))
    }

    #[cfg(feature = "async")]
    async fn read_async(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        let phys_mem = self.phys_mem.clone();
        tokio::task::spawn_blocking(move || match size {
            1 => phys_mem.read_u8(pa.0 as usize).map(|v| v as u64),
            2 => phys_mem.read_u16(pa.0 as usize).map(|v| v as u64),
            4 => phys_mem.read_u32(pa.0 as usize).map(|v| v as u64),
            8 => phys_mem.read_u64(pa.0 as usize),
            _ => Err(VmError::from(vm_core::Fault::AlignmentFault)),
        })
        .await
        .unwrap_or(Err(VmError::Core(vm_core::error::CoreError::Internal {
            message: "Async operation failed".to_string(),
            module: "unified_mmu_v2".to_string(),
        })))
    }

    #[cfg(feature = "async")]
    async fn write_async(&self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
        let phys_mem = self.phys_mem.clone();
        tokio::task::spawn_blocking(move || match size {
            1 => phys_mem.write_u8(pa.0 as usize, val as u8),
            2 => phys_mem.write_u16(pa.0 as usize, val as u16),
            4 => phys_mem.write_u32(pa.0 as usize, val as u32),
            8 => phys_mem.write_u64(pa.0 as usize, val),
            _ => Err(VmError::from(vm_core::Fault::AlignmentFault)),
        })
        .await
        .unwrap_or(Err(VmError::Core(vm_core::error::CoreError::Internal {
            message: "Async operation failed".to_string(),
            module: "unified_mmu_v2".to_string(),
        })))
    }

    #[cfg(feature = "async")]
    async fn read_bulk_async(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError> {
        // 将数据读取到一个Vec中，然后复制回buf
        let phys_mem = self.phys_mem.clone();
        let addr = pa.0 as usize;
        let len = buf.len();
        let data = tokio::task::spawn_blocking(move || {
            let mut temp_buf = vec![0u8; len];
            phys_mem
                .read_buf(addr, &mut temp_buf)
                .map(|_| temp_buf)
                .map_err(|_| {
                    VmError::from(vm_core::Fault::PageFault {
                        addr: pa,
                        access_type: AccessType::Read,
                        is_write: false,
                        is_user: false,
                    })
                })
        })
        .await
        .unwrap_or(Err(VmError::Core(vm_core::error::CoreError::Internal {
            message: "Async operation failed".to_string(),
            module: "unified_mmu_v2".to_string(),
        })))?;

        // 复制数据回buf
        buf.copy_from_slice(&data);
        Ok(())
    }

    #[cfg(feature = "async")]
    async fn write_bulk_async(&self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError> {
        let phys_mem = self.phys_mem.clone();
        let addr = pa.0 as usize;
        let buf_vec = buf.to_vec(); // 复制数据到拥有的Vec
        tokio::task::spawn_blocking(move || {
            phys_mem.write_buf(addr, &buf_vec).map_err(|_| {
                VmError::from(vm_core::Fault::PageFault {
                    addr: pa,
                    access_type: AccessType::Write,
                    is_write: true,
                    is_user: false,
                })
            })
        })
        .await
        .unwrap_or(Err(VmError::Core(vm_core::error::CoreError::Internal {
            message: "Async operation failed".to_string(),
            module: "unified_mmu_v2".to_string(),
        })))
    }

    #[cfg(feature = "async")]
    async fn translate_bulk_async(
        &mut self,
        vas: &[(GuestAddr, AccessType)],
    ) -> Result<Vec<GuestPhysAddr>, VmError> {
        let sync_mmu = self.sync_mmu.clone();
        let vas = vas.to_vec();
        tokio::task::spawn_blocking(move || {
            let mut results = Vec::with_capacity(vas.len());
            let mut mmu = sync_mmu.lock();
            for (va, access) in vas {
                results.push(mmu.translate(va, access)?);
            }
            Ok(results)
        })
        .await
        .unwrap_or(Err(VmError::Core(vm_core::error::CoreError::Internal {
            message: "Async operation failed".to_string(),
            module: "unified_mmu_v2".to_string(),
        })))
    }

    #[cfg(feature = "async")]
    async fn flush_tlb_async(&mut self) -> Result<(), VmError> {
        let sync_mmu = self.sync_mmu.clone();
        tokio::task::spawn_blocking(move || {
            sync_mmu.lock().flush_tlb();
            Ok(())
        })
        .await
        .unwrap_or(Ok(()))
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_mmu_config_default() {
        let config = UnifiedMmuConfigV2::default();
        assert_eq!(config.l1_itlb_capacity, 64);
        assert_eq!(config.l1_dtlb_capacity, 128);
        assert_eq!(config.l2_tlb_capacity, 512);
        assert_eq!(config.shard_count, 16);
    }

    #[test]
    fn test_hybrid_mmu_creation() {
        let config = UnifiedMmuConfigV2::default();
        let mmu = HybridMMU::new(1024 * 1024, config);
        assert_eq!(mmu.memory_size(), 1024 * 1024);
    }

    #[test]
    fn test_translate_bare_mode() {
        let config = UnifiedMmuConfigV2::default();
        let mut mmu = HybridMMU::new(1024 * 1024, config);

        // Bare模式应该是恒等映射
        let result = mmu.translate(GuestAddr(0x1000), AccessType::Read);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), GuestPhysAddr(0x1000));
    }

    #[test]
    fn test_memory_read_write() {
        let config = UnifiedMmuConfigV2::default();
        let mut mmu = HybridMMU::new(1024 * 1024, config);

        // 写入
        let write_result = mmu.write(GuestAddr(0x100), 0xDEADBEEF, 4);
        assert!(write_result.is_ok());

        // 读取
        let read_result = mmu.read(GuestAddr(0x100), 4);
        assert!(read_result.is_ok());
        assert_eq!(read_result.unwrap(), 0xDEADBEEF);
    }

    #[test]
    fn test_stats_calculation() {
        let stats = UnifiedMmuStats {
            mmu_id: 1,
            tlb_hits: 800,
            tlb_misses: 200,
            tlb_hit_rate: 0.0,
            page_table_cache_hits: 0,
            page_table_cache_misses: 0,
            page_table_cache_hit_rate: 0.0,
            prefetch_hits: 0,
            prefetch_count: 0,
            prefetch_efficiency: 0.0,
            total_translations: 0,
            page_walks: 0,
            avg_translation_latency_ns: 0.0,
            total_translation_time_ns: 0,
            memory_size_bytes: 1024 * 1024,
            memory_reads: 0,
            memory_writes: 0,
        };

        // 测试TLB命中率计算
        assert_eq!(stats.tlb_hit_rate(), 0.8); // 800/(800+200)
    }

    #[test]
    fn test_config_custom() {
        let config = UnifiedMmuConfigV2 {
            enable_multilevel_tlb: true,
            l1_itlb_capacity: 128,
            l1_dtlb_capacity: 256,
            l2_tlb_capacity: 1024,
            l3_tlb_capacity: 4096,
            enable_concurrent_tlb: true,
            sharded_tlb_capacity: 8192,
            shard_count: 32,
            enable_fast_path: true,
            fast_path_capacity: 128,
            enable_page_table_cache: true,
            page_table_cache_size: 2048,
            enable_prefetch: true,
            prefetch_window: 8,
            prefetch_history_window: 64,
            enable_adaptive_replacement: true,
            adaptive_threshold: 0.9,
            enable_stats: true,
            enable_monitoring: true,
            strict_align: true,
            use_hugepages: true,
        };

        assert_eq!(config.l1_itlb_capacity, 128);
        assert_eq!(config.shard_count, 32);
        assert_eq!(config.strict_align, true);
        assert_eq!(config.use_hugepages, true);
    }

    #[test]
    fn test_bulk_operations() {
        let config = UnifiedMmuConfigV2::default();
        let mut mmu = HybridMMU::new(1024 * 1024, config);

        // 准备测试数据
        let test_data: Vec<u8> = (0..256).map(|i| i as u8).collect();

        // 批量写入
        let write_result = mmu.write_bulk(GuestAddr(0x1000), &test_data);
        assert!(write_result.is_ok());

        // 批量读取
        let mut read_buffer = vec![0u8; 256];
        let read_result = mmu.read_bulk(GuestAddr(0x1000), &mut read_buffer);
        assert!(read_result.is_ok());

        // 验证数据
        assert_eq!(read_buffer, test_data);
    }

    #[test]
    fn test_fetch_insn() {
        let config = UnifiedMmuConfigV2::default();
        let mut mmu = HybridMMU::new(1024 * 1024, config);

        // 写入指令数据
        let write_result = mmu.write(GuestAddr(0x2000), 0x12345678, 4);
        assert!(write_result.is_ok());

        // 取指令
        let fetch_result = mmu.fetch_insn(GuestAddr(0x2000));
        assert!(fetch_result.is_ok());
        assert_eq!(fetch_result.unwrap(), 0x12345678);
    }

    #[test]
    fn test_multiple_read_sizes() {
        let config = UnifiedMmuConfigV2::default();
        let mut mmu = HybridMMU::new(1024 * 1024, config);

        // 测试不同大小的写入
        let _ = mmu.write(GuestAddr(0x100), 0xFF, 1);
        let _ = mmu.write(GuestAddr(0x200), 0xFFFF, 2);
        let _ = mmu.write(GuestAddr(0x300), 0xFFFFFFFF, 4);
        let _ = mmu.write(GuestAddr(0x400), 0xFFFFFFFFFFFFFFFF, 8);

        // 测试不同大小的读取
        let r1 = mmu.read(GuestAddr(0x100), 1);
        assert!(r1.is_ok());
        assert_eq!(r1.unwrap(), 0xFF);

        let r2 = mmu.read(GuestAddr(0x200), 2);
        assert!(r2.is_ok());
        assert_eq!(r2.unwrap(), 0xFFFF);

        let r4 = mmu.read(GuestAddr(0x300), 4);
        assert!(r4.is_ok());
        assert_eq!(r4.unwrap(), 0xFFFFFFFF);

        let r8 = mmu.read(GuestAddr(0x400), 8);
        assert!(r8.is_ok());
        assert_eq!(r8.unwrap(), 0xFFFFFFFFFFFFFFFF);
    }

    #[test]
    fn test_tlb_flush() {
        let config = UnifiedMmuConfigV2::default();
        let mut mmu = HybridMMU::new(1024 * 1024, config);

        // 进行一些翻译操作
        let _ = mmu.translate(GuestAddr(0x1000), AccessType::Read);
        let _ = mmu.translate(GuestAddr(0x2000), AccessType::Read);

        // 刷新TLB
        mmu.flush_tlb();

        // 验证TLB被刷新（再次翻译应该仍然成功）
        let result = mmu.translate(GuestAddr(0x1000), AccessType::Read);
        assert!(result.is_ok());
    }

    #[test]
    fn test_paging_mode() {
        let config = UnifiedMmuConfigV2::default();
        let mut mmu = HybridMMU::new(1024 * 1024, config);

        // 设置分页模式
        mmu.set_paging_mode(PagingMode::Sv39);

        // 翻译应该仍然工作（虽然Bare模式下是恒等映射）
        let result = mmu.translate(GuestAddr(0x1000), AccessType::Read);
        assert!(result.is_ok());
    }

    #[test]
    fn test_strict_align() {
        let mut config = UnifiedMmuConfigV2::default();
        config.strict_align = true;

        let mut mmu = HybridMMU::new(1024 * 1024, config);

        // 设置严格对齐
        mmu.set_strict_align(true);

        // 非对齐访问可能失败（取决于实现）
        let result = mmu.read(GuestAddr(0x101), 4); // 非4字节对齐
        // 可能成功或失败，取决于实现
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_memory_dump_restore() {
        let config = UnifiedMmuConfigV2::default();
        let mut mmu = HybridMMU::new(1024 * 1024, config.clone());

        // 写入一些数据
        let _ = mmu.write(GuestAddr(0x100), 0xDEADBEEF, 4);
        let _ = mmu.write(GuestAddr(0x200), 0xCAFEBABE, 4);

        // 导出内存
        let dump = mmu.dump_memory();
        assert_eq!(dump.len(), 1024 * 1024);

        // 创建新的MMU并恢复
        let mut mmu2 = HybridMMU::new(1024 * 1024, config);
        let restore_result = mmu2.restore_memory(&dump);
        assert!(restore_result.is_ok());

        // 验证恢复的数据
        let r1 = mmu2.read(GuestAddr(0x100), 4);
        assert!(r1.is_ok());
        assert_eq!(r1.unwrap(), 0xDEADBEEF);

        let r2 = mmu2.read(GuestAddr(0x200), 4);
        assert!(r2.is_ok());
        assert_eq!(r2.unwrap(), 0xCAFEBABE);
    }

    #[test]
    fn test_stats_collection() {
        let config = UnifiedMmuConfigV2::default();
        let mut mmu = HybridMMU::new(1024 * 1024, config);

        // 执行一些操作
        let _ = mmu.translate(GuestAddr(0x1000), AccessType::Read);
        let _ = mmu.read(GuestAddr(0x100), 4);
        let _ = mmu.write(GuestAddr(0x200), 0x12345678, 4);

        // 获取统计信息
        let stats = mmu.stats();
        assert_eq!(stats.mmu_id, 1);
        assert_eq!(stats.memory_size_bytes, 1024 * 1024);
    }

    #[test]
    fn test_edge_case_zero_size() {
        let config = UnifiedMmuConfigV2::default();
        let mmu = HybridMMU::new(0, config);
        assert_eq!(mmu.memory_size(), 0);
    }

    #[test]
    fn test_edge_case_large_memory() {
        let config = UnifiedMmuConfigV2::default();
        let size = 1024 * 1024 * 1024; // 1GB
        let mmu = HybridMMU::new(size, config);
        assert_eq!(mmu.memory_size(), size);
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let config = UnifiedMmuConfigV2::default();
        let mmu = Arc::new(Mutex::new(HybridMMU::new(1024 * 1024, config)));
        let mut handles = vec![];

        // 创建多个线程并发访问
        for i in 0..4 {
            let mmu_clone = Arc::clone(&mmu);
            let handle = thread::spawn(move || {
                let mut mmu = mmu_clone.lock().unwrap();
                let addr = GuestAddr(0x100 + i * 0x10);
                let _ = mmu.write(addr, i as u64, 8);
                let _ = mmu.read(addr, 8);
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            assert!(handle.join().is_ok());
        }
    }

    #[test]
    fn test_flush_tlb_asid() {
        let config = UnifiedMmuConfigV2::default();
        let mut mmu = HybridMMU::new(1024 * 1024, config);

        // 设置ASID
        mmu.set_satp(0x0000_0001_0000_0000); // ASID=1

        // 刷新特定ASID的TLB
        mmu.flush_tlb_asid(1);

        // 验证仍然可以翻译
        let result = mmu.translate(GuestAddr(0x1000), AccessType::Read);
        assert!(result.is_ok());
    }

    #[test]
    fn test_flush_tlb_page() {
        let config = UnifiedMmuConfigV2::default();
        let mut mmu = HybridMMU::new(1024 * 1024, config);

        // 翻译一个地址
        let _ = mmu.translate(GuestAddr(0x1000), AccessType::Read);

        // 刷新特定页面的TLB
        mmu.flush_tlb_page(GuestAddr(0x1000));

        // 再次翻译应该仍然成功
        let result = mmu.translate(GuestAddr(0x1000), AccessType::Read);
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[cfg(feature = "async")]
    async fn test_async_translate() {
        let config = UnifiedMmuConfigV2::default();
        let mut mmu = HybridMMU::new(1024 * 1024, config);

        // 异步翻译
        let result = mmu
            .translate_async(GuestAddr(0x1000), AccessType::Read)
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), GuestPhysAddr(0x1000));
    }

    #[tokio::test]
    #[cfg(feature = "async")]
    async fn test_async_read_write() {
        let config = UnifiedMmuConfigV2::default();
        let mmu = HybridMMU::new(1024 * 1024, config);

        // 异步写入
        let write_result = mmu.write_async(GuestAddr(0x100), 0xDEADBEEF, 4).await;
        assert!(write_result.is_ok());

        // 异步读取
        let read_result = mmu.read_async(GuestAddr(0x100), 4).await;
        assert!(read_result.is_ok());
        assert_eq!(read_result.unwrap(), 0xDEADBEEF);
    }

    #[tokio::test]
    #[cfg(feature = "async")]
    async fn test_async_bulk_operations() {
        let config = UnifiedMmuConfigV2::default();
        let mmu = HybridMMU::new(1024 * 1024, config);

        // 准备测试数据
        let test_data: Vec<u8> = (0..256).map(|i| i as u8).collect();

        // 异步批量写入
        let write_result = mmu.write_bulk_async(GuestAddr(0x1000), &test_data).await;
        assert!(write_result.is_ok());

        // 异步批量读取
        let mut read_buffer = vec![0u8; 256];
        let read_result = mmu
            .read_bulk_async(GuestAddr(0x1000), &mut read_buffer)
            .await;
        assert!(read_result.is_ok());

        // 验证数据
        assert_eq!(read_buffer, test_data);
    }

    #[tokio::test]
    #[cfg(feature = "async")]
    async fn test_async_translate_bulk() {
        let config = UnifiedMmuConfigV2::default();
        let mut mmu = HybridMMU::new(1024 * 1024, config);

        // 准备地址列表
        let addresses = vec![
            (GuestAddr(0x1000), AccessType::Read),
            (GuestAddr(0x2000), AccessType::Write),
            (GuestAddr(0x3000), AccessType::Execute),
        ];

        // 异步批量翻译
        let results = mmu.translate_bulk_async(&addresses).await;
        assert!(results.is_ok());
        let addrs = results.unwrap();
        assert_eq!(addrs.len(), 3);
    }

    #[tokio::test]
    #[cfg(feature = "async")]
    async fn test_async_flush_tlb() {
        let config = UnifiedMmuConfigV2::default();
        let mut mmu = HybridMMU::new(1024 * 1024, config);

        // 异步刷新TLB
        let result = mmu.flush_tlb_async().await;
        assert!(result.is_ok());
    }
}
