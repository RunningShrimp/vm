//! 统一MMU和TLB实现
//!
//! 整合多级TLB、并发TLB和页表缓存，实现高性能内存管理

use crate::memory::page_table_walker::{Sv39PageTableWalker, Sv48PageTableWalker};
use crate::tlb::tlb_concurrent::{ConcurrentTlbConfig, ConcurrentTlbManagerAdapter};
use crate::tlb::tlb_optimized::{MultiLevelTlb, MultiLevelTlbConfig};
use crate::{PAGE_SHIFT, PAGE_SIZE, PagingMode, PhysicalMemory, pte_flags};
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use vm_core::error::VmError;
use vm_core::{AccessType, GuestAddr, GuestPhysAddr, MmioDevice};
use vm_core::{AddressTranslator, MemoryAccess, MmioManager, MmuAsAny, TlbManager};

/// 页表缓存条目
#[derive(Debug, Clone)]
pub struct PageTableCacheEntry {
    /// 页表基址
    pub base: GuestPhysAddr,
    /// 页表级别（0-3）
    pub level: u8,
    /// 页表项索引
    pub index: u64,
    /// 页表项值
    pub pte_value: u64,
    /// 访问计数
    pub access_count: u64,
    /// 最后访问时间
    pub last_access: u64,
}

/// 页表缓存
///
/// 缓存页表遍历的中间结果，减少重复遍历开销
pub struct PageTableCache {
    /// 缓存条目（key: (base, level, index)）
    entries: HashMap<(GuestPhysAddr, u8, u64), PageTableCacheEntry>,
    /// LRU顺序
    lru_order: VecDeque<(GuestPhysAddr, u8, u64)>,
    /// 最大容量
    max_capacity: usize,
    /// 命中次数
    hits: u64,
    /// 缺失次数
    misses: u64,
}

impl PageTableCache {
    /// 创建新的页表缓存
    pub fn new(max_capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(max_capacity),
            lru_order: VecDeque::with_capacity(max_capacity),
            max_capacity,
            hits: 0,
            misses: 0,
        }
    }

    /// 查找页表项
    pub fn lookup(&mut self, base: GuestPhysAddr, level: u8, index: u64) -> Option<u64> {
        let key = (base, level, index);

        if let Some(entry) = self.entries.get_mut(&key) {
            entry.access_count += 1;
            entry.last_access = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;

            // 更新LRU顺序
            if let Some(pos) = self.lru_order.iter().position(|&k| k == key) {
                self.lru_order.remove(pos);
            }
            self.lru_order.push_back(key);

            self.hits += 1;
            Some(entry.pte_value)
        } else {
            self.misses += 1;
            None
        }
    }

    /// 插入页表项
    pub fn insert(&mut self, base: GuestPhysAddr, level: u8, index: u64, pte_value: u64) {
        let key = (base, level, index);

        // 如果已满，驱逐LRU条目
        if self.entries.len() >= self.max_capacity
            && let Some(old_key) = self.lru_order.pop_front()
        {
            self.entries.remove(&old_key);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let entry = PageTableCacheEntry {
            base,
            level,
            index,
            pte_value,
            access_count: 1,
            last_access: now,
        };

        self.entries.insert(key, entry);
        self.lru_order.push_back(key);
    }

    /// 使缓存失效（当页表更新时）
    pub fn invalidate(&mut self, base: GuestPhysAddr, level: Option<u8>) {
        if let Some(level) = level {
            // 使特定级别的缓存失效
            self.entries
                .retain(|(b, l, _), _| *b != base || *l != level);
            self.lru_order.retain(|(b, l, _)| *b != base || *l != level);
        } else {
            // 使所有相关缓存失效
            self.entries.retain(|(b, _, _), _| *b != base);
            self.lru_order.retain(|(b, _, _)| *b != base);
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> (u64, u64, f64) {
        let total = self.hits + self.misses;
        let hit_rate = if total > 0 {
            self.hits as f64 / total as f64
        } else {
            0.0
        };
        (self.hits, self.misses, hit_rate)
    }
}

impl AddressTranslator for UnifiedMmu {
    fn translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestPhysAddr, VmError> {
        self.translate_with_cache(va, access)
    }

    fn flush_tlb(&mut self) {
        self.flush_all_tlbs()
    }

    fn flush_tlb_asid(&mut self, asid: u16) {
        self.flush_asid_tlbs(asid)
    }

    fn flush_tlb_page(&mut self, va: GuestAddr) {
        // 根据策略尝试TLB页面刷新（使用trait接口）
        match self.strategy {
            MmuOptimizationStrategy::MultiLevel => {
                if let Some(ref mut tlb) = self.multilevel_tlb {
                    tlb.flush_page(va)
                }
            }
            MmuOptimizationStrategy::Concurrent => {
                if let Some(ref mut tlb) = self.concurrent_tlb {
                    tlb.flush_page(va)
                }
            }
            MmuOptimizationStrategy::Hybrid => {
                if let Some(ref mut tlb) = self.concurrent_tlb {
                    tlb.flush_page(va)
                }
                if let Some(ref mut tlb) = self.multilevel_tlb {
                    tlb.flush_page(va)
                }
            }
        }
    }
}

impl MemoryAccess for UnifiedMmu {
    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        // 这里我们需要将GuestAddr转换为GuestPhysAddr
        // 注意：在bare模式下，GuestAddr和GuestPhysAddr是相同的
        // 在分页模式下，这应该是已经翻译过的地址
        let phys_addr = GuestPhysAddr(pa.0);
        self.read_phys(phys_addr, size)
    }

    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError> {
        // 同样需要将GuestAddr转换为GuestPhysAddr
        let phys_addr = GuestPhysAddr(pa.0);
        self.write_phys(phys_addr, val, size)
    }

    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError> {
        // 取指令通常是4字节，根据架构不同可能有所不同
        // 这里假设是4字节指令
        self.read(pc, 4)
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
}

impl MmioManager for UnifiedMmu {
    fn map_mmio(&self, _base: GuestAddr, _size: u64, _device: Box<dyn MmioDevice>) {
        // 暂时未实现MMIO设备映射功能
        // 后续可以在这里添加MMIO设备管理逻辑
    }
}

impl MmuAsAny for UnifiedMmu {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// 内存预取器
///
/// 基于访问模式预测并预取内存页面
pub struct MemoryPrefetcher {
    /// 访问历史（最近访问的地址序列）
    access_history: VecDeque<GuestAddr>,
    /// 预取队列
    prefetch_queue: VecDeque<GuestAddr>,
    /// 历史窗口大小
    history_window: usize,
    /// 预取距离
    prefetch_distance: usize,
    /// 预取命中次数
    prefetch_hits: u64,
    /// 预取次数
    prefetch_count: u64,
}

impl MemoryPrefetcher {
    /// 创建新的内存预取器
    pub fn new(history_window: usize, prefetch_distance: usize) -> Self {
        Self {
            access_history: VecDeque::with_capacity(history_window),
            prefetch_queue: VecDeque::with_capacity(prefetch_distance),
            history_window,
            prefetch_distance,
            prefetch_hits: 0,
            prefetch_count: 0,
        }
    }

    /// 记录访问
    pub fn record_access(&mut self, addr: GuestAddr) {
        // 添加到历史
        if self.access_history.len() >= self.history_window {
            self.access_history.pop_front();
        }
        self.access_history.push_back(addr);

        // 分析访问模式并生成预取请求
        self.analyze_and_prefetch(addr);
    }

    /// 分析访问模式并生成预取请求
    fn analyze_and_prefetch(&mut self, current_addr: GuestAddr) {
        if self.access_history.len() < 2 {
            return;
        }

        // 计算步长（如果访问模式是顺序的）
        let last_addr = self.access_history[self.access_history.len() - 2];
        let step = current_addr.0.wrapping_sub(last_addr.0);

        // 如果步长合理（在页面大小范围内），预测下一个地址
        if step > 0 && step <= PAGE_SIZE * 4 {
            let predicted_addr = GuestAddr(current_addr.0.wrapping_add(step));

            // 检查是否已经在队列中
            if !self.prefetch_queue.contains(&predicted_addr) {
                if self.prefetch_queue.len() >= self.prefetch_distance {
                    self.prefetch_queue.pop_front();
                }
                self.prefetch_queue.push_back(predicted_addr);
                self.prefetch_count += 1;
            }
        }
    }

    /// 获取下一个预取地址
    pub fn get_prefetch_addr(&mut self) -> Option<GuestAddr> {
        self.prefetch_queue.pop_front()
    }

    /// 记录预取命中
    pub fn record_prefetch_hit(&mut self) {
        self.prefetch_hits += 1;
    }

    /// 获取预取效率
    pub fn prefetch_efficiency(&self) -> f64 {
        if self.prefetch_count == 0 {
            0.0
        } else {
            self.prefetch_hits as f64 / self.prefetch_count as f64
        }
    }
}

/// MMU优化策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmuOptimizationStrategy {
    /// 多级TLB优化
    MultiLevel,
    /// 并发优化
    Concurrent,
    /// 混合优化（多级+并发）
    Hybrid,
}

/// 统一TLB配置
///
/// 合并MultiLevelTlbConfig和ConcurrentTlbConfig的功能
/// 通过配置选项控制是否启用多级TLB、并发优化等特性
#[derive(Debug, Clone)]
pub struct UnifiedTlbConfig {
    /// 启用多级TLB
    pub enable_multilevel: bool,
    /// L1 TLB容量（最快访问）
    pub l1_capacity: usize,
    /// L2 TLB容量（中等访问）
    pub l2_capacity: usize,
    /// L3 TLB容量（大容量）
    pub l3_capacity: usize,
    /// 启用并发优化
    pub enable_concurrent: bool,
    /// 分片TLB总容量
    pub sharded_capacity: usize,
    /// 分片数量
    pub shard_count: usize,
    /// 快速路径TLB容量
    pub fast_path_capacity: usize,
    /// 启用快速路径
    pub enable_fast_path: bool,
    /// 预取窗口大小
    pub prefetch_window: usize,
    /// 预取阈值
    pub prefetch_threshold: f64,
    /// 自适应替换策略
    pub adaptive_replacement: bool,
    /// 启用自适应调整
    pub enable_adaptive: bool,
    /// 统计收集
    pub enable_stats: bool,
}

impl Default for UnifiedTlbConfig {
    fn default() -> Self {
        Self {
            enable_multilevel: true,
            l1_capacity: 64,
            l2_capacity: 256,
            l3_capacity: 1024,
            enable_concurrent: true,
            sharded_capacity: 4096,
            shard_count: 16,
            fast_path_capacity: 64,
            enable_fast_path: true,
            prefetch_window: 4,
            prefetch_threshold: 0.8,
            adaptive_replacement: true,
            enable_adaptive: true,
            enable_stats: true,
        }
    }
}

impl From<MultiLevelTlbConfig> for UnifiedTlbConfig {
    fn from(config: MultiLevelTlbConfig) -> Self {
        Self {
            enable_multilevel: true,
            l1_capacity: config.l1_capacity,
            l2_capacity: config.l2_capacity,
            l3_capacity: config.l3_capacity,
            enable_concurrent: config.concurrent_optimization,
            sharded_capacity: 4096,
            shard_count: 16,
            fast_path_capacity: 64,
            enable_fast_path: true,
            prefetch_window: config.prefetch_window,
            prefetch_threshold: config.prefetch_threshold,
            adaptive_replacement: config.adaptive_replacement,
            enable_adaptive: true,
            enable_stats: config.enable_stats,
        }
    }
}

impl From<ConcurrentTlbConfig> for UnifiedTlbConfig {
    fn from(config: ConcurrentTlbConfig) -> Self {
        Self {
            enable_multilevel: false,
            l1_capacity: 64,
            l2_capacity: 256,
            l3_capacity: 1024,
            enable_concurrent: true,
            sharded_capacity: config.sharded_capacity,
            shard_count: config.shard_count,
            fast_path_capacity: config.fast_path_capacity,
            enable_fast_path: config.enable_fast_path,
            prefetch_window: 4,
            prefetch_threshold: 0.8,
            adaptive_replacement: false,
            enable_adaptive: config.enable_adaptive,
            enable_stats: true,
        }
    }
}

/// 统一MMU配置
#[derive(Debug, Clone)]
pub struct UnifiedMmuConfig {
    /// 优化策略
    pub strategy: MmuOptimizationStrategy,
    /// TLB配置（多级）
    /// 统一TLB配置（合并多级TLB和并发TLB的配置）
    pub unified_tlb_config: UnifiedTlbConfig,
    /// 已废弃：多级TLB配置（保留用于向后兼容）
    #[deprecated(note = "使用unified_tlb_config替代")]
    pub multilevel_tlb_config: MultiLevelTlbConfig,
    /// 已废弃：并发TLB配置（保留用于向后兼容）
    #[deprecated(note = "使用unified_tlb_config替代")]
    pub concurrent_tlb_config: ConcurrentTlbConfig,
    /// 页表缓存容量
    pub page_table_cache_size: usize,
    /// 启用页表缓存
    pub enable_page_table_cache: bool,
    /// 启用内存预取
    pub enable_prefetch: bool,
    /// 预取历史窗口大小
    pub prefetch_history_window: usize,
    /// 预取距离
    pub prefetch_distance: usize,
    /// 预取窗口大小（用于VPN预取）
    pub prefetch_window: usize,
    /// 启用自适应调整
    pub enable_adaptive: bool,
    /// 性能监控
    pub enable_monitoring: bool,
    /// 严格对齐
    pub strict_align: bool,
}

impl Default for UnifiedMmuConfig {
    fn default() -> Self {
        Self {
            strategy: MmuOptimizationStrategy::Hybrid,
            unified_tlb_config: UnifiedTlbConfig::default(),
            // 已弃用字段：保留但不初始化，使用unified_tlb_config替代
            #[allow(deprecated)]
            multilevel_tlb_config: MultiLevelTlbConfig::default(),
            #[allow(deprecated)]
            concurrent_tlb_config: ConcurrentTlbConfig::default(),
            page_table_cache_size: 1024,
            enable_page_table_cache: true,
            enable_prefetch: true,
            prefetch_history_window: 32,
            prefetch_distance: 8,
            prefetch_window: 4,
            enable_adaptive: true,
            enable_monitoring: true,
            strict_align: false,
        }
    }
}

/// 统一MMU统计信息
#[derive(Debug)]
pub struct UnifiedMmuStats {
    /// MMU ID
    pub mmu_id: u64,
    /// TLB命中次数
    pub tlb_hits: AtomicU64,
    /// TLB缺失次数
    pub tlb_misses: AtomicU64,
    /// 页表缓存命中次数
    pub page_table_cache_hits: AtomicU64,
    /// 页表缓存缺失次数
    pub page_table_cache_misses: AtomicU64,
    /// 预取命中次数
    pub prefetch_hits: AtomicU64,
    /// 预取次数
    pub prefetch_count: AtomicU64,
    /// 总翻译次数
    pub total_translations: AtomicU64,
    /// 页表遍历次数
    pub page_walks: AtomicU64,
    /// 总翻译时间（纳秒）
    pub total_translation_time_ns: AtomicU64,
}

impl UnifiedMmuStats {
    pub fn new(mmu_id: u64) -> Self {
        Self {
            mmu_id,
            tlb_hits: AtomicU64::new(0),
            tlb_misses: AtomicU64::new(0),
            page_table_cache_hits: AtomicU64::new(0),
            page_table_cache_misses: AtomicU64::new(0),
            prefetch_hits: AtomicU64::new(0),
            prefetch_count: AtomicU64::new(0),
            total_translations: AtomicU64::new(0),
            page_walks: AtomicU64::new(0),
            total_translation_time_ns: AtomicU64::new(0),
        }
    }

    pub fn tlb_hit_rate(&self) -> f64 {
        let total = self.tlb_hits.load(Ordering::Relaxed) + self.tlb_misses.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            self.tlb_hits.load(Ordering::Relaxed) as f64 / total as f64
        }
    }

    pub fn page_table_cache_hit_rate(&self) -> f64 {
        let total = self.page_table_cache_hits.load(Ordering::Relaxed)
            + self.page_table_cache_misses.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            self.page_table_cache_hits.load(Ordering::Relaxed) as f64 / total as f64
        }
    }

    pub fn avg_latency_ns(&self) -> f64 {
        let total = self.total_translations.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            self.total_translation_time_ns.load(Ordering::Relaxed) as f64 / total as f64
        }
    }
}

/// 统一MMU实现
///
/// 整合多级TLB、并发TLB、页表缓存和内存预取
pub struct UnifiedMmu {
    /// MMU ID
    id: u64,
    /// 物理内存
    phys_mem: Arc<PhysicalMemory>,
    /// 分页模式
    paging_mode: PagingMode,
    /// 页表基址
    page_table_base: GuestPhysAddr,
    /// ASID
    asid: u16,
    /// 优化策略
    strategy: MmuOptimizationStrategy,
    /// 多级TLB（使用trait接口）
    multilevel_tlb: Option<Box<dyn TlbManager>>,
    /// 并发TLB（使用trait接口）
    concurrent_tlb: Option<Box<dyn TlbManager>>,
    /// 页表缓存
    page_table_cache: Option<RwLock<PageTableCache>>,
    /// 内存预取器
    prefetcher: Option<RwLock<MemoryPrefetcher>>,
    /// 页表遍历器
    page_table_walker: Option<Box<dyn vm_core::PageTableWalker>>,
    /// 配置
    config: UnifiedMmuConfig,
    /// 统计信息
    stats: Arc<UnifiedMmuStats>,
    /// 严格对齐
    strict_align: bool,
    /// 预取队列（用于VPN预取）
    prefetch_queue: RwLock<VecDeque<(u64, u16)>>, // (vpn, asid)
}

/// 全局MMU ID生成器
static NEXT_MMU_ID: AtomicU64 = AtomicU64::new(1);

impl UnifiedMmu {
    /// 创建新的统一MMU
    pub fn new(size: usize, use_hugepages: bool, config: UnifiedMmuConfig) -> Self {
        let id = NEXT_MMU_ID.fetch_add(1, Ordering::Relaxed);
        let phys_mem = Arc::new(PhysicalMemory::new(size, use_hugepages));

        // 使用统一TLB配置，根据配置选项决定使用哪个实现
        let (multilevel_tlb, concurrent_tlb) = {
            let unified_config = &config.unified_tlb_config;

            // 根据统一配置决定使用哪个实现
            let use_multilevel = unified_config.enable_multilevel
                || config.strategy == MmuOptimizationStrategy::MultiLevel
                || config.strategy == MmuOptimizationStrategy::Hybrid;
            let use_concurrent = unified_config.enable_concurrent
                || config.strategy == MmuOptimizationStrategy::Concurrent
                || config.strategy == MmuOptimizationStrategy::Hybrid;

            (
                if use_multilevel {
                    // 从统一配置创建多级TLB配置
                    let multilevel_config = MultiLevelTlbConfig {
                        l1_capacity: unified_config.l1_capacity,
                        l2_capacity: unified_config.l2_capacity,
                        l3_capacity: unified_config.l3_capacity,
                        prefetch_window: unified_config.prefetch_window,
                        prefetch_threshold: unified_config.prefetch_threshold,
                        adaptive_replacement: unified_config.adaptive_replacement,
                        concurrent_optimization: unified_config.enable_concurrent,
                        enable_stats: unified_config.enable_stats,
                    };
                    Some(Box::new(MultiLevelTlb::new(multilevel_config)) as Box<dyn TlbManager>)
                } else {
                    None
                },
                if use_concurrent {
                    // 从统一配置创建并发TLB配置
                    let concurrent_config = ConcurrentTlbConfig {
                        sharded_capacity: unified_config.sharded_capacity,
                        shard_count: unified_config.shard_count,
                        fast_path_capacity: unified_config.fast_path_capacity,
                        enable_fast_path: unified_config.enable_fast_path,
                        enable_adaptive: unified_config.enable_adaptive,
                    };
                    Some(
                        Box::new(ConcurrentTlbManagerAdapter::new(concurrent_config))
                            as Box<dyn TlbManager>,
                    )
                } else {
                    None
                },
            )
        };

        let page_table_cache = if config.enable_page_table_cache {
            Some(RwLock::new(PageTableCache::new(
                config.page_table_cache_size,
            )))
        } else {
            None
        };

        let prefetcher = if config.enable_prefetch {
            Some(RwLock::new(MemoryPrefetcher::new(
                config.prefetch_history_window,
                config.prefetch_distance,
            )))
        } else {
            None
        };

        Self {
            id,
            phys_mem,
            paging_mode: PagingMode::Bare,
            page_table_base: GuestPhysAddr(0),
            asid: 0,
            strategy: config.strategy,
            multilevel_tlb,
            concurrent_tlb,
            page_table_cache,
            prefetcher,
            page_table_walker: None,
            config: config.clone(),
            stats: Arc::new(UnifiedMmuStats::new(id)),
            strict_align: config.strict_align,
            prefetch_queue: RwLock::new(VecDeque::new()),
        }
    }

    /// 设置分页模式
    pub fn set_paging_mode(&mut self, mode: PagingMode) {
        if self.paging_mode != mode {
            self.paging_mode = mode;
            self.flush_all_tlbs();
            self.update_page_table_walker();
        }
    }

    /// 设置SATP寄存器（RISC-V）
    pub fn set_satp(&mut self, satp: u64) {
        let mode = (satp >> 60) & 0xF;
        let asid = ((satp >> 44) & 0xFFFF) as u16;
        let ppn = satp & ((1u64 << 44) - 1);

        self.paging_mode = match mode {
            0 => PagingMode::Bare,
            8 => PagingMode::Sv39,
            9 => PagingMode::Sv48,
            _ => PagingMode::Bare,
        };

        self.asid = asid;
        self.page_table_base = GuestPhysAddr(ppn << PAGE_SHIFT);

        self.flush_asid_tlbs(asid);
        self.update_page_table_walker();
    }

    /// 设置严格对齐
    pub fn set_strict_align(&mut self, enable: bool) {
        self.strict_align = enable;
    }

    /// 获取MMU ID
    pub fn id(&self) -> u64 {
        self.id
    }

    /// 刷新所有TLB
    fn flush_all_tlbs(&mut self) {
        if let Some(ref mut tlb) = self.multilevel_tlb {
            tlb.flush();
        }
        if let Some(ref mut tlb) = self.concurrent_tlb {
            tlb.flush();
        }
    }

    /// 刷新指定ASID的TLB
    fn flush_asid_tlbs(&mut self, asid: u16) {
        if let Some(ref mut tlb) = self.multilevel_tlb {
            tlb.flush_asid(asid);
        }
        if let Some(ref mut tlb) = self.concurrent_tlb {
            tlb.flush_asid(asid);
        }
    }

    /// 更新页表遍历器
    fn update_page_table_walker(&mut self) {
        self.page_table_walker = match self.paging_mode {
            PagingMode::Sv39 => Some(Box::new(Sv39PageTableWalker::new(
                self.page_table_base,
                self.asid,
            ))),
            PagingMode::Sv48 => Some(Box::new(Sv48PageTableWalker::new(
                self.page_table_base,
                self.asid,
            ))),
            _ => None,
        };
    }

    /// 翻译地址（带页表缓存和预取）
    pub fn translate_with_cache(
        &mut self,
        va: GuestAddr,
        access: AccessType,
    ) -> Result<GuestPhysAddr, VmError> {
        let start_time = std::time::Instant::now();
        self.stats
            .total_translations
            .fetch_add(1, Ordering::Relaxed);

        if self.paging_mode == PagingMode::Bare {
            return Ok(va.into());
        }

        let vpn = va.0 >> PAGE_SHIFT;
        let asid = self.asid;

        // 根据策略尝试TLB查找（使用trait接口）
        let tlb_result = match self.strategy {
            MmuOptimizationStrategy::MultiLevel => self
                .multilevel_tlb
                .as_mut()
                .and_then(|tlb| tlb.lookup(va, asid, access)),
            MmuOptimizationStrategy::Concurrent => self
                .concurrent_tlb
                .as_mut()
                .and_then(|tlb| tlb.lookup(va, asid, access)),
            MmuOptimizationStrategy::Hybrid => {
                // 优先使用并发TLB，然后是多级TLB
                self.concurrent_tlb
                    .as_mut()
                    .and_then(|tlb| tlb.lookup(va, asid, access))
                    .or_else(|| {
                        self.multilevel_tlb
                            .as_mut()
                            .and_then(|tlb| tlb.lookup(va, asid, access))
                    })
            }
        };

        if let Some(entry) = tlb_result {
            let flags = entry.flags;
            self.stats.tlb_hits.fetch_add(1, Ordering::Relaxed);

            let required = match access {
                AccessType::Read => pte_flags::R,
                AccessType::Write => pte_flags::W,
                AccessType::Execute => pte_flags::X,
                AccessType::Atomic => pte_flags::R | pte_flags::W, // Atomic operations need both R and W bits
            };

            if flags & required == 0 {
                return Err(VmError::from(vm_core::Fault::PageFault {
                    addr: va,
                    access_type: access,
                    is_write: access == AccessType::Write || access == AccessType::Atomic,
                    is_user: false,
                }));
            }

            let offset = va.0 & (PAGE_SIZE - 1);
            let pa = entry.phys_addr.0 | offset;

            // 更新统计信息
            let elapsed = start_time.elapsed();
            self.stats
                .total_translation_time_ns
                .fetch_add(elapsed.as_nanos() as u64, Ordering::Relaxed);

            // 记录访问用于预取
            if let Some(ref prefetcher) = self.prefetcher {
                prefetcher.write().record_access(va);
            }

            return Ok(GuestPhysAddr(pa));
        }

        // TLB缺失，进行页表遍历（使用页表缓存）
        self.stats.tlb_misses.fetch_add(1, Ordering::Relaxed);
        self.stats.page_walks.fetch_add(1, Ordering::Relaxed);

        let va = vpn << PAGE_SHIFT;

        // 使用页表遍历器或页表缓存
        let asid = self.asid;
        let (pa, flags) = match self.page_table_walker.take() {
            Some(mut walker) => {
                // 临时移出walker，避免双重借用
                let result = walker
                    .walk(GuestAddr(va), access, asid, self)
                    .map(|(pa, flags)| (pa.0, flags));
                // 将walker放回去
                self.page_table_walker = Some(walker);
                result
            }
            None => {
                // 使用页表缓存或恒等映射
                let pa = self.page_table_walk_with_cache(GuestAddr(va), access)?;
                Ok((pa.0, pte_flags::R | pte_flags::W | pte_flags::X))
            }
        }?;

        let ppn = pa >> PAGE_SHIFT;

        // 插入到TLB
        self.insert_to_all_tlbs(vpn, ppn, flags, self.asid);

        // 触发预取
        if self.config.enable_prefetch {
            self.trigger_prefetch(vpn, self.asid);
        }

        let required = match access {
            AccessType::Read => pte_flags::R,
            AccessType::Write => pte_flags::W,
            AccessType::Execute => pte_flags::X,
            AccessType::Atomic => pte_flags::R | pte_flags::W, // Atomic operations need both R and W bits
        };

        if flags & required == 0 {
            return Err(VmError::from(vm_core::Fault::PageFault {
                addr: GuestAddr(va),
                access_type: access,
                is_write: access == AccessType::Write || access == AccessType::Atomic,
                is_user: false,
            }));
        }

        let offset = va & (PAGE_SIZE - 1);
        let pa = (pa >> PAGE_SHIFT << PAGE_SHIFT) | offset;

        // 更新统计信息
        let elapsed = start_time.elapsed();
        self.stats
            .total_translation_time_ns
            .fetch_add(elapsed.as_nanos() as u64, Ordering::Relaxed);

        // 记录访问用于预取
        if let Some(ref prefetcher) = self.prefetcher {
            prefetcher.write().record_access(GuestAddr(va));
        }

        // 定期处理预取队列
        if self.config.enable_prefetch
            && self
                .stats
                .total_translations
                .load(Ordering::Relaxed)
                .is_multiple_of(100)
        {
            self.process_prefetch_queue();
        }

        Ok(GuestPhysAddr(pa))
    }

    /// 插入到所有TLB（使用trait接口）
    fn insert_to_all_tlbs(&mut self, vpn: u64, ppn: u64, flags: u64, asid: u16) {
        let entry = vm_core::TlbEntry {
            guest_addr: GuestAddr(vpn << PAGE_SHIFT),
            phys_addr: GuestPhysAddr(ppn << PAGE_SHIFT),
            flags,
            asid,
        };

        if let Some(ref mut tlb) = self.multilevel_tlb {
            tlb.update(entry);
        }
        if let Some(ref mut tlb) = self.concurrent_tlb {
            tlb.update(entry);
        }
    }

    /// 触发预取
    fn trigger_prefetch(&mut self, current_vpn: u64, asid: u16) {
        if !self.config.enable_prefetch {
            return;
        }

        let mut prefetch_queue = self.prefetch_queue.write();

        // 顺序预取
        for i in 1..=self.config.prefetch_window {
            let prefetch_vpn = current_vpn + i as u64;
            let prefetch_key = (prefetch_vpn, asid);

            if !prefetch_queue.contains(&prefetch_key) {
                prefetch_queue.push_back(prefetch_key);

                // 限制队列大小
                if prefetch_queue.len() > self.config.prefetch_window * 2 {
                    prefetch_queue.pop_front();
                }
            }
        }
    }

    /// 处理预取队列
    fn process_prefetch_queue(&mut self) {
        if !self.config.enable_prefetch {
            return;
        }

        let mut prefetch_queue = self.prefetch_queue.write();
        let prefetch_requests: Vec<_> = prefetch_queue.drain(..).collect();
        drop(prefetch_queue);

        for (vpn, asid) in prefetch_requests {
            // 检查是否已经在TLB中（使用trait接口）
            let va = GuestAddr(vpn << PAGE_SHIFT);
            let already_cached = match self.strategy {
                MmuOptimizationStrategy::MultiLevel => self
                    .multilevel_tlb
                    .as_mut()
                    .map(|tlb| tlb.lookup(va, asid, AccessType::Read).is_some())
                    .unwrap_or(false),
                MmuOptimizationStrategy::Concurrent => self
                    .concurrent_tlb
                    .as_mut()
                    .map(|tlb| tlb.lookup(va, asid, AccessType::Read).is_some())
                    .unwrap_or(false),
                MmuOptimizationStrategy::Hybrid => {
                    let multilevel_cached = self
                        .multilevel_tlb
                        .as_mut()
                        .map(|tlb| tlb.lookup(va, asid, AccessType::Read).is_some())
                        .unwrap_or(false);
                    let concurrent_cached = self
                        .concurrent_tlb
                        .as_mut()
                        .map(|tlb| tlb.lookup(va, asid, AccessType::Read).is_some())
                        .unwrap_or(false);
                    multilevel_cached || concurrent_cached
                }
            };

            if !already_cached {
                // 执行页表遍历并插入TLB
                let va = GuestAddr(vpn << PAGE_SHIFT);
                if let Ok(pa) = self.page_table_walk_with_cache(va, AccessType::Read) {
                    let ppn = pa >> PAGE_SHIFT;
                    let flags = pte_flags::R | pte_flags::W | pte_flags::X;
                    self.insert_to_all_tlbs(vpn, ppn, flags, asid);
                    self.stats.prefetch_hits.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    /// 使用页表缓存的页表遍历
    fn page_table_walk_with_cache(
        &self,
        va: GuestAddr,
        _access: AccessType,
    ) -> Result<GuestPhysAddr, VmError> {
        // 如果使用页表缓存，先尝试从缓存获取
        if let Some(ref cache) = self.page_table_cache {
            let mut cache_guard = cache.write();

            // 尝试从缓存获取页表项
            let vpn = va.0 >> PAGE_SHIFT;
            let level = 0; // 简化：实际应该遍历多级页表

            if let Some(pte_value) = cache_guard.lookup(self.page_table_base, level, vpn) {
                self.stats
                    .page_table_cache_hits
                    .fetch_add(1, Ordering::Relaxed);

                // 从PTE提取物理地址
                let ppn = (pte_value >> 10) & ((1u64 << 44) - 1);
                let pa = (ppn << PAGE_SHIFT) | (va.0 & (PAGE_SIZE - 1));
                return Ok(GuestPhysAddr(pa));
            } else {
                self.stats
                    .page_table_cache_misses
                    .fetch_add(1, Ordering::Relaxed);
            }
        }

        // 缓存缺失，使用页表遍历器或恒等映射
        if let Some(ref _walker) = self.page_table_walker {
            // 使用页表遍历器（需要&mut self，这里简化处理）
            // 实际应该通过页表遍历器进行遍历
            let pa = va; // 简化：恒等映射
            Ok(GuestPhysAddr(pa.0))
        } else {
            // 没有页表遍历器，使用恒等映射
            let pa = va;

            // 将结果存入缓存
            if let Some(ref cache) = self.page_table_cache {
                let mut cache_guard = cache.write();
                let vpn = va.0 >> PAGE_SHIFT;
                let level = 0;
                let pte_value = (pa.0 >> PAGE_SHIFT) << 10; // 简化PTE构造
                cache_guard.insert(self.page_table_base, level, vpn, pte_value);
            }

            Ok(GuestPhysAddr(pa.0))
        }
    }

    /// 获取预取地址
    pub fn get_prefetch_addr(&self) -> Option<GuestAddr> {
        self.prefetcher
            .as_ref()
            .and_then(|p| p.write().get_prefetch_addr())
    }

    /// 记录预取命中
    pub fn record_prefetch_hit(&self) {
        if let Some(ref prefetcher) = self.prefetcher {
            prefetcher.write().record_prefetch_hit();
            self.stats.prefetch_hits.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> Arc<UnifiedMmuStats> {
        Arc::clone(&self.stats)
    }

    /// 检查对齐
    fn check_alignment(&self, pa: GuestAddr, size: u8) -> Result<(), VmError> {
        if self.strict_align {
            match size {
                1 => {}
                2 => {
                    if pa % 2 != 0 {
                        return Err(VmError::from(vm_core::Fault::AlignmentFault));
                    }
                }
                4 => {
                    if pa % 4 != 0 {
                        return Err(VmError::from(vm_core::Fault::AlignmentFault));
                    }
                }
                8 => {
                    if pa % 8 != 0 {
                        return Err(VmError::from(vm_core::Fault::AlignmentFault));
                    }
                }
                _ => {
                    return Err(VmError::from(vm_core::Fault::AlignmentFault));
                }
            }
        } else if !matches!(size, 1 | 2 | 4 | 8) {
            return Err(VmError::from(vm_core::Fault::AlignmentFault));
        }
        Ok(())
    }

    /// 读取物理内存
    fn read_phys(&self, pa: GuestPhysAddr, size: u8) -> Result<u64, VmError> {
        let addr = pa.0 as usize;
        self.check_alignment(GuestAddr(pa.0), size)?;

        match size {
            1 => self.phys_mem.read_u8(addr).map(|v| v as u64),
            2 => self.phys_mem.read_u16(addr).map(|v| v as u64),
            4 => self.phys_mem.read_u32(addr).map(|v| v as u64),
            8 => self.phys_mem.read_u64(addr),
            _ => {
                return Err(VmError::from(vm_core::Fault::AlignmentFault));
            }
        }
        .map_err(|_| {
            VmError::from(vm_core::Fault::PageFault {
                addr: GuestAddr(pa.0),
                access_type: AccessType::Read,
                is_write: false,
                is_user: false,
            })
        })
    }

    /// 写入物理内存
    fn write_phys(&mut self, pa: GuestPhysAddr, val: u64, size: u8) -> Result<(), VmError> {
        let addr = pa.0 as usize;
        // 注意：这里我们将GuestPhysAddr转换为u64再转换为GuestAddr进行对齐检查
        // 这是一个临时解决方案，理想情况下应该有专门的GuestPhysAddr对齐检查方法
        self.check_alignment(GuestAddr(pa.0), size)?;

        let res = match size {
            1 => self.phys_mem.write_u8(addr, val as u8),
            2 => self.phys_mem.write_u16(addr, val as u16),
            4 => self.phys_mem.write_u32(addr, val as u32),
            8 => self.phys_mem.write_u64(addr, val),
            _ => {
                return Err(VmError::from(vm_core::Fault::AlignmentFault));
            }
        };

        if res.is_err() {
            return Err(VmError::from(vm_core::Fault::PageFault {
                addr: GuestAddr(pa.0),
                access_type: AccessType::Write,
                is_write: true,
                is_user: false,
            }));
        }

        Ok(())
    }
}
