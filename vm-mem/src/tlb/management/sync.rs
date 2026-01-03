//! TLB同步机制
//!
//! 实现Per-CPU TLB之间的同步，确保一致性

use crate::GuestAddr;
use crate::tlb::core::per_cpu::PerCpuTlbManager;

/// Type alias for dedup window key to reduce type complexity
type DedupKey = (GuestAddr, u16, SyncEventType);

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use vm_core::VmError;

/// 同步事件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncEventType {
    /// 页表更新
    PageTableUpdate,
    /// ASID切换
    AsidSwitch,
    /// TLB刷新
    TlbFlush,
    /// 全局映射更新
    GlobalMappingUpdate,
    /// 页面权限变更
    PermissionChange,
}

/// 同步事件
#[derive(Debug, Clone)]
pub struct SyncEvent {
    /// 事件类型
    pub event_type: SyncEventType,
    /// Guest虚拟地址
    pub gva: GuestAddr,
    /// ASID
    pub asid: u16,
    /// 页面大小
    pub page_size: u64,
    /// 事件时间戳
    pub timestamp: Instant,
    /// 事件ID（用于去重）
    pub event_id: u64,
    /// 是否为全局事件
    pub global: bool,
    /// 源CPU ID
    pub source_cpu: usize,
}

impl SyncEvent {
    /// 创建新的同步事件
    pub fn new(
        event_type: SyncEventType,
        gva: GuestAddr,
        asid: u16,
        page_size: u64,
        source_cpu: usize,
    ) -> Self {
        static EVENT_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

        Self {
            event_type,
            gva,
            asid,
            page_size,
            timestamp: Instant::now(),
            event_id: EVENT_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
            global: matches!(event_type, SyncEventType::GlobalMappingUpdate),
            source_cpu,
        }
    }

    /// 检查事件是否影响特定地址
    pub fn affects_address(&self, gva: GuestAddr, asid: u16) -> bool {
        if self.global || self.asid == asid {
            let page_base = self.gva & !(self.page_size - 1);
            let gva_base = gva & !(self.page_size - 1);
            page_base == gva_base
        } else {
            false
        }
    }
}

/// 同步策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStrategy {
    /// 立即同步
    Immediate,
    /// 批量同步
    Batched,
    /// 延迟同步
    Delayed,
    /// 自适应同步
    Adaptive,
}

/// 同步配置
#[derive(Debug, Clone)]
pub struct TlbSyncConfig {
    /// 同步策略
    pub strategy: SyncStrategy,
    /// 批量同步大小
    pub batch_size: usize,
    /// 批量同步超时（毫秒）
    pub batch_timeout_ms: u64,
    /// 延迟同步延迟（毫秒）
    pub delay_ms: u64,
    /// 最大同步队列大小
    pub max_queue_size: usize,
    /// 是否启用事件去重
    pub enable_deduplication: bool,
    /// 去重窗口大小（毫秒）
    pub dedup_window_ms: u64,
    /// 是否启用统计信息
    pub enable_stats: bool,
}

impl Default for TlbSyncConfig {
    fn default() -> Self {
        Self {
            strategy: SyncStrategy::Adaptive,
            batch_size: 16,
            batch_timeout_ms: 10,
            delay_ms: 5,
            max_queue_size: 1024,
            enable_deduplication: true,
            dedup_window_ms: 100,
            enable_stats: true,
        }
    }
}

/// 同步统计信息
#[derive(Debug, Default)]
pub struct TlbSyncStats {
    /// 总同步事件数
    pub total_events: AtomicU64,
    /// 立即同步次数
    pub immediate_syncs: AtomicU64,
    /// 批量同步次数
    pub batched_syncs: AtomicU64,
    /// 延迟同步次数
    pub delayed_syncs: AtomicU64,
    /// 去重事件数
    pub deduplicated_events: AtomicU64,
    /// 同步失败次数
    pub failed_syncs: AtomicU64,
    /// 平均同步时间（纳秒）
    pub avg_sync_time_ns: AtomicU64,
    /// 最大同步时间（纳秒）
    pub max_sync_time_ns: AtomicU64,
    /// 当前队列大小
    pub current_queue_size: AtomicUsize,
    /// 最大队列大小
    pub max_queue_size: AtomicUsize,
}

impl TlbSyncStats {
    /// 获取统计信息快照
    pub fn snapshot(&self) -> TlbSyncStatsSnapshot {
        let total = self.total_events.load(Ordering::Relaxed);
        TlbSyncStatsSnapshot {
            total_events: total,
            immediate_syncs: self.immediate_syncs.load(Ordering::Relaxed),
            batched_syncs: self.batched_syncs.load(Ordering::Relaxed),
            delayed_syncs: self.delayed_syncs.load(Ordering::Relaxed),
            deduplicated_events: self.deduplicated_events.load(Ordering::Relaxed),
            failed_syncs: self.failed_syncs.load(Ordering::Relaxed),
            avg_sync_time_ns: self.avg_sync_time_ns.load(Ordering::Relaxed),
            max_sync_time_ns: self.max_sync_time_ns.load(Ordering::Relaxed),
            current_queue_size: self.current_queue_size.load(Ordering::Relaxed),
            max_queue_size: self.max_queue_size.load(Ordering::Relaxed),
            deduplication_rate: if total > 0 {
                self.deduplicated_events.load(Ordering::Relaxed) as f64 / total as f64
            } else {
                0.0
            },
        }
    }

    /// 重置统计信息
    pub fn reset(&self) {
        self.total_events.store(0, Ordering::Relaxed);
        self.immediate_syncs.store(0, Ordering::Relaxed);
        self.batched_syncs.store(0, Ordering::Relaxed);
        self.delayed_syncs.store(0, Ordering::Relaxed);
        self.deduplicated_events.store(0, Ordering::Relaxed);
        self.failed_syncs.store(0, Ordering::Relaxed);
        self.avg_sync_time_ns.store(0, Ordering::Relaxed);
        self.max_sync_time_ns.store(0, Ordering::Relaxed);
        self.max_queue_size.store(
            self.current_queue_size.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
    }
}

/// 同步统计信息快照
#[derive(Debug, Clone)]
pub struct TlbSyncStatsSnapshot {
    pub total_events: u64,
    pub immediate_syncs: u64,
    pub batched_syncs: u64,
    pub delayed_syncs: u64,
    pub deduplicated_events: u64,
    pub failed_syncs: u64,
    pub avg_sync_time_ns: u64,
    pub max_sync_time_ns: u64,
    pub current_queue_size: usize,
    pub max_queue_size: usize,
    pub deduplication_rate: f64,
}

/// TLB同步器
pub struct TlbSynchronizer {
    /// 配置
    config: TlbSyncConfig,
    /// Per-CPU TLB管理器
    tlb_manager: Arc<PerCpuTlbManager>,
    /// 同步事件队列
    sync_queue: Arc<Mutex<VecDeque<SyncEvent>>>,
    /// 去重窗口
    dedup_window: Arc<RwLock<HashMap<DedupKey, Instant>>>,
    /// 统计信息
    stats: Arc<TlbSyncStats>,
    /// 最后批量同步时间
    last_batch_sync: Arc<Mutex<Instant>>,
}

impl TlbSynchronizer {
    /// 创建新的TLB同步器
    pub fn new(config: TlbSyncConfig, tlb_manager: Arc<PerCpuTlbManager>) -> Self {
        Self {
            config,
            tlb_manager,
            sync_queue: Arc::new(Mutex::new(VecDeque::new())),
            dedup_window: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(TlbSyncStats::default()),
            last_batch_sync: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// 使用默认配置创建TLB同步器
    pub fn with_default_config(tlb_manager: Arc<PerCpuTlbManager>) -> Self {
        Self::new(TlbSyncConfig::default(), tlb_manager)
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> TlbSyncStatsSnapshot {
        self.stats.snapshot()
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        self.stats.reset();
    }

    /// 添加同步事件
    pub fn add_sync_event(&self, event: SyncEvent) -> Result<(), VmError> {
        // 更新统计信息
        self.stats.total_events.fetch_add(1, Ordering::Relaxed);

        // 去重检查
        if self.config.enable_deduplication {
            if self.is_duplicate_event(&event)? {
                self.stats
                    .deduplicated_events
                    .fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
            self.add_to_dedup_window(&event);
        }

        // 根据同步策略处理事件
        match self.config.strategy {
            SyncStrategy::Immediate => self.sync_immediate(&event),
            SyncStrategy::Batched => self.sync_batched(&event),
            SyncStrategy::Delayed => self.sync_delayed(&event),
            SyncStrategy::Adaptive => self.sync_adaptive(&event),
        }
    }

    // Helper methods for lock operations
    fn lock_dedup_window_read(
        &self,
    ) -> Result<std::sync::RwLockReadGuard<'_, HashMap<DedupKey, Instant>>, VmError> {
        self.dedup_window.read().map_err(|_| {
            VmError::Memory(vm_core::MemoryError::MmuLockFailed {
                message: format!(
                    "Failed to acquire dedup_window read lock: {}",
                    "dedup_window"
                ),
            })
        })
    }

    fn lock_dedup_window_write(
        &self,
    ) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<DedupKey, Instant>>, VmError> {
        self.dedup_window.write().map_err(|_| {
            VmError::Memory(vm_core::MemoryError::MmuLockFailed {
                message: format!(
                    "Failed to acquire dedup_window write lock: {}",
                    "dedup_window"
                ),
            })
        })
    }

    fn lock_sync_queue(&self) -> Result<std::sync::MutexGuard<'_, VecDeque<SyncEvent>>, VmError> {
        self.sync_queue.lock().map_err(|_| {
            VmError::Memory(vm_core::MemoryError::MmuLockFailed {
                message: format!("Failed to acquire sync_queue lock: {}", "sync_queue"),
            })
        })
    }

    fn lock_last_batch_sync(&self) -> Result<std::sync::MutexGuard<'_, Instant>, VmError> {
        self.last_batch_sync.lock().map_err(|_| {
            VmError::Memory(vm_core::MemoryError::MmuLockFailed {
                message: format!(
                    "Failed to acquire last_batch_sync lock: {}",
                    "last_batch_sync"
                ),
            })
        })
    }

    /// 检查是否为重复事件
    fn is_duplicate_event(&self, event: &SyncEvent) -> Result<bool, VmError> {
        let dedup_window = self.lock_dedup_window_read()?;
        let key = (event.gva, event.asid, event.event_type);

        Ok(if let Some(&timestamp) = dedup_window.get(&key) {
            let elapsed = timestamp.elapsed();
            elapsed < Duration::from_millis(self.config.dedup_window_ms)
        } else {
            false
        })
    }

    /// 添加事件到去重窗口
    fn add_to_dedup_window(&self, event: &SyncEvent) {
        if let Ok(mut dedup_window) = self.lock_dedup_window_write() {
            let key = (event.gva, event.asid, event.event_type);
            dedup_window.insert(key, event.timestamp);

            // 清理过期条目
            let now = Instant::now();
            dedup_window.retain(|_, &mut timestamp| {
                now.duration_since(timestamp) < Duration::from_millis(self.config.dedup_window_ms)
            });
        }
        // Silently fail if lock is poisoned
    }

    /// 立即同步
    fn sync_immediate(&self, event: &SyncEvent) -> Result<(), VmError> {
        let start_time = Instant::now();

        match event.event_type {
            SyncEventType::PageTableUpdate | SyncEventType::PermissionChange => {
                // 刷新特定页面
                self.tlb_manager.flush_page(event.gva, event.asid);
            }
            SyncEventType::AsidSwitch => {
                // 刷新特定ASID
                self.tlb_manager.flush_asid(event.asid);
            }
            SyncEventType::TlbFlush | SyncEventType::GlobalMappingUpdate => {
                // 刷新所有TLB
                self.tlb_manager.flush_all();
            }
        }

        // 更新统计信息
        self.stats.immediate_syncs.fetch_add(1, Ordering::Relaxed);
        self.update_sync_time_stats(start_time.elapsed().as_nanos() as u64);

        Ok(())
    }

    /// 批量同步
    fn sync_batched(&self, event: &SyncEvent) -> Result<(), VmError> {
        // 添加到队列
        {
            let mut queue = self.lock_sync_queue()?;

            // 检查队列大小
            if queue.len() >= self.config.max_queue_size {
                // 队列已满，强制同步
                let events = queue.drain(..).collect::<Vec<_>>();
                self.process_batch_sync(&events)?;
            }

            queue.push_back(event.clone());

            // 更新队列大小统计
            let current_size = queue.len();
            self.stats
                .current_queue_size
                .store(current_size, Ordering::Relaxed);

            let max_size = self.stats.max_queue_size.load(Ordering::Relaxed);
            if current_size > max_size {
                self.stats
                    .max_queue_size
                    .store(current_size, Ordering::Relaxed);
            }
        }

        // 检查是否需要立即处理
        let should_process = {
            let queue = self.lock_sync_queue()?;
            let queue_len = queue.len();
            queue_len >= self.config.batch_size || self.should_process_batch_timeout()?
        };

        if should_process {
            self.process_batch_queue()?;
        }

        Ok(())
    }

    /// 延迟同步
    fn sync_delayed(&self, event: &SyncEvent) -> Result<(), VmError> {
        // 添加到队列
        {
            let mut queue = self.lock_sync_queue()?;

            // 检查队列大小
            if queue.len() >= self.config.max_queue_size {
                // 队列已满，强制同步
                let events = queue.drain(..).collect::<Vec<_>>();
                self.process_batch_sync(&events)?;
            }

            queue.push_back(event.clone());

            // 更新队列大小统计
            let current_size = queue.len();
            self.stats
                .current_queue_size
                .store(current_size, Ordering::Relaxed);

            let max_size = self.stats.max_queue_size.load(Ordering::Relaxed);
            if current_size > max_size {
                self.stats
                    .max_queue_size
                    .store(current_size, Ordering::Relaxed);
            }
        }

        // 延迟处理
        std::thread::spawn({
            let sync_queue = self.sync_queue.clone();
            let stats = self.stats.clone();
            let delay_ms = self.config.delay_ms;
            let tlb_manager = self.tlb_manager.clone();

            move || {
                std::thread::sleep(Duration::from_millis(delay_ms));

                let events = {
                    // Use try_lock in spawned thread to avoid blocking
                    if let Ok(mut queue) = sync_queue.try_lock() {
                        let events = queue.drain(..).collect::<Vec<_>>();
                        stats.current_queue_size.store(0, Ordering::Relaxed);
                        events
                    } else {
                        // If lock is not available, skip this batch
                        return;
                    }
                };

                if !events.is_empty() {
                    Self::process_batch_sync_static(&events, &tlb_manager, &stats);
                }
            }
        });

        self.stats.delayed_syncs.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// 自适应同步
    fn sync_adaptive(&self, event: &SyncEvent) -> Result<(), VmError> {
        // 根据系统负载和事件类型选择策略
        let stats = self.stats.snapshot();

        // 如果队列很大或事件是全局的，使用立即同步
        if stats.current_queue_size > self.config.batch_size || event.global {
            return self.sync_immediate(event);
        }

        // 如果同步失败率高，使用批量同步
        if stats.total_events > 0 && stats.failed_syncs as f64 / stats.total_events as f64 > 0.1 {
            return self.sync_batched(event);
        }

        // 默认使用批量同步
        self.sync_batched(event)
    }

    /// 检查是否应该处理批量同步（基于超时）
    fn should_process_batch_timeout(&self) -> Result<bool, VmError> {
        let last_sync = self.lock_last_batch_sync()?;
        Ok(last_sync.elapsed() >= Duration::from_millis(self.config.batch_timeout_ms))
    }

    /// 处理批量同步队列
    fn process_batch_queue(&self) -> Result<(), VmError> {
        let events = {
            let mut queue = self.lock_sync_queue()?;
            let events = queue.drain(..).collect::<Vec<_>>();
            self.stats.current_queue_size.store(0, Ordering::Relaxed);

            // 更新最后批量同步时间
            let mut last_sync = self.lock_last_batch_sync()?;
            *last_sync = Instant::now();

            events
        };

        if !events.is_empty() {
            self.process_batch_sync(&events)?;
        }

        Ok(())
    }

    /// 处理批量同步
    fn process_batch_sync(&self, events: &[SyncEvent]) -> Result<(), VmError> {
        let start_time = Instant::now();

        // 按事件类型分组
        let mut page_updates = HashSet::new();
        let mut asid_switches = HashSet::new();
        let mut global_flushes = false;

        for event in events {
            match event.event_type {
                SyncEventType::PageTableUpdate | SyncEventType::PermissionChange => {
                    page_updates.insert((event.gva, event.asid));
                }
                SyncEventType::AsidSwitch => {
                    asid_switches.insert(event.asid);
                }
                SyncEventType::TlbFlush | SyncEventType::GlobalMappingUpdate => {
                    global_flushes = true;
                }
            }
        }

        // 执行同步操作
        if global_flushes {
            self.tlb_manager.flush_all();
        } else {
            // 先处理ASID切换
            for asid in asid_switches {
                self.tlb_manager.flush_asid(asid);
            }

            // 再处理页面更新
            for (gva, asid) in page_updates {
                self.tlb_manager.flush_page(gva, asid);
            }
        }

        // 更新统计信息
        self.stats.batched_syncs.fetch_add(1, Ordering::Relaxed);
        self.update_sync_time_stats(start_time.elapsed().as_nanos() as u64);

        Ok(())
    }

    /// 静态方法：处理批量同步（用于线程中）
    fn process_batch_sync_static(
        events: &[SyncEvent],
        tlb_manager: &PerCpuTlbManager,
        stats: &TlbSyncStats,
    ) {
        let start_time = Instant::now();

        // 按事件类型分组
        let mut page_updates = HashSet::new();
        let mut asid_switches = HashSet::new();
        let mut global_flushes = false;

        for event in events {
            match event.event_type {
                SyncEventType::PageTableUpdate | SyncEventType::PermissionChange => {
                    page_updates.insert((event.gva, event.asid));
                }
                SyncEventType::AsidSwitch => {
                    asid_switches.insert(event.asid);
                }
                SyncEventType::TlbFlush | SyncEventType::GlobalMappingUpdate => {
                    global_flushes = true;
                }
            }
        }

        // 执行同步操作
        if global_flushes {
            tlb_manager.flush_all();
        } else {
            // 先处理ASID切换
            for asid in asid_switches {
                tlb_manager.flush_asid(asid);
            }

            // 再处理页面更新
            for (gva, asid) in page_updates {
                tlb_manager.flush_page(gva, asid);
            }
        }

        // 更新统计信息
        stats.batched_syncs.fetch_add(1, Ordering::Relaxed);
        let sync_time = start_time.elapsed().as_nanos() as u64;
        Self::update_sync_time_stats_static(stats, sync_time);
    }

    /// 更新同步时间统计信息
    fn update_sync_time_stats(&self, sync_time_ns: u64) {
        let total = self.stats.total_events.load(Ordering::Relaxed);
        let current_avg = self.stats.avg_sync_time_ns.load(Ordering::Relaxed);

        // 计算新的平均值
        let new_avg = if total > 1 {
            (current_avg * (total - 1) + sync_time_ns) / total
        } else {
            sync_time_ns
        };

        self.stats
            .avg_sync_time_ns
            .store(new_avg, Ordering::Relaxed);

        // 更新最大值
        let current_max = self.stats.max_sync_time_ns.load(Ordering::Relaxed);
        if sync_time_ns > current_max {
            self.stats
                .max_sync_time_ns
                .store(sync_time_ns, Ordering::Relaxed);
        }
    }

    /// 静态方法：更新同步时间统计信息
    fn update_sync_time_stats_static(stats: &TlbSyncStats, sync_time_ns: u64) {
        let total = stats.total_events.load(Ordering::Relaxed);
        let current_avg = stats.avg_sync_time_ns.load(Ordering::Relaxed);

        // 计算新的平均值
        let new_avg = if total > 1 {
            (current_avg * (total - 1) + sync_time_ns) / total
        } else {
            sync_time_ns
        };

        stats.avg_sync_time_ns.store(new_avg, Ordering::Relaxed);

        // 更新最大值
        let current_max = stats.max_sync_time_ns.load(Ordering::Relaxed);
        if sync_time_ns > current_max {
            stats
                .max_sync_time_ns
                .store(sync_time_ns, Ordering::Relaxed);
        }
    }

    /// 强制处理所有待同步事件
    pub fn flush_sync_queue(&self) -> Result<(), VmError> {
        self.process_batch_queue()
    }

    /// 获取当前同步队列大小
    pub fn get_queue_size(&self) -> usize {
        self.stats.current_queue_size.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tlb::core::per_cpu::PerCpuTlbManager;

    #[test]
    fn test_sync_event_creation() {
        let event = SyncEvent::new(
            SyncEventType::PageTableUpdate,
            GuestAddr(0x1000),
            0,
            4096,
            0,
        );

        assert_eq!(event.event_type, SyncEventType::PageTableUpdate);
        assert_eq!(event.gva, GuestAddr(0x1000));
        assert_eq!(event.asid, 0);
        assert_eq!(event.page_size, 4096);
        assert_eq!(event.source_cpu, 0);
        assert!(!event.global);
    }

    #[test]
    fn test_sync_event_affects_address() {
        let event = SyncEvent::new(
            SyncEventType::PageTableUpdate,
            GuestAddr(0x1000),
            0,
            4096,
            0,
        );

        // 同一页面
        assert!(event.affects_address(GuestAddr(0x1000), 0));
        assert!(event.affects_address(GuestAddr(0x1FFF), 0));

        // 不同页面
        assert!(!event.affects_address(GuestAddr(0x2000), 0));

        // 不同ASID
        assert!(!event.affects_address(GuestAddr(0x1000), 1));
    }

    #[test]
    fn test_global_sync_event() {
        let event = SyncEvent::new(
            SyncEventType::GlobalMappingUpdate,
            GuestAddr(0x1000),
            0,
            4096,
            0,
        );

        assert!(event.global);

        // 全局事件影响所有ASID
        assert!(event.affects_address(GuestAddr(0x1000), 0));
        assert!(event.affects_address(GuestAddr(0x1000), 1));
    }

    #[test]
    fn test_tlb_synchronizer_creation() {
        let tlb_manager = Arc::new(PerCpuTlbManager::with_default_config());
        let synchronizer = TlbSynchronizer::with_default_config(tlb_manager);

        let stats = synchronizer.get_stats();
        assert_eq!(stats.total_events, 0);
    }

    #[test]
    fn test_immediate_sync() {
        let tlb_manager = Arc::new(PerCpuTlbManager::with_default_config());
        let config = TlbSyncConfig {
            strategy: SyncStrategy::Immediate,
            ..Default::default()
        };
        let synchronizer = TlbSynchronizer::new(config, tlb_manager.clone());

        let event = SyncEvent::new(
            SyncEventType::PageTableUpdate,
            GuestAddr(0x1000),
            0,
            4096,
            0,
        );

        let result = synchronizer.add_sync_event(event);
        assert!(result.is_ok());

        let stats = synchronizer.get_stats();
        assert_eq!(stats.total_events, 1);
        assert_eq!(stats.immediate_syncs, 1);
    }

    #[test]
    fn test_deduplication() {
        let tlb_manager = Arc::new(PerCpuTlbManager::with_default_config());
        let config = TlbSyncConfig {
            enable_deduplication: true,
            dedup_window_ms: 100,
            ..Default::default()
        };
        let synchronizer = TlbSynchronizer::new(config, tlb_manager.clone());

        let event1 = SyncEvent::new(
            SyncEventType::PageTableUpdate,
            GuestAddr(0x1000),
            0,
            4096,
            0,
        );

        let event2 = SyncEvent::new(
            SyncEventType::PageTableUpdate,
            GuestAddr(0x1000),
            0,
            4096,
            1,
        );

        let result1 = synchronizer.add_sync_event(event1);
        let result2 = synchronizer.add_sync_event(event2);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let stats = synchronizer.get_stats();
        assert_eq!(stats.total_events, 2);
        assert_eq!(stats.deduplicated_events, 1);
    }
}
