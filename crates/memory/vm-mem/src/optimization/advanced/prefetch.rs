//! 地址翻译预取优化
//!
//! 实现高效的地址翻译预取和缓存机制

use crate::mmu::{PageTableFlags, PageWalkResult};
use crate::{GuestAddr, VmError};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use vm_core::AccessType;
use vm_core::error::MemoryError;

/// 预取策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefetchStrategy {
    /// 不预取
    None,
    /// 固定距离预取
    FixedDistance,
    /// 自适应预取
    Adaptive,
    /// 基于历史模式的预取
    PatternBased,
    /// 流式预取
    StreamBased,
}

/// 访问模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessPattern {
    /// 随机访问
    Random,
    /// 顺序访问
    Sequential,
    /// 步长访问
    Strided { stride: u64 },
    /// 未知模式
    Unknown,
}

/// 地址翻译请求
#[derive(Debug, Clone)]
pub struct TranslationRequest {
    /// Guest虚拟地址
    pub gva: GuestAddr,
    /// ASID
    pub asid: u16,
    /// 访问类型
    pub access_type: AccessType,
    /// 请求时间戳
    pub timestamp: Instant,
    /// 是否为预取请求
    pub is_prefetch: bool,
}

/// 地址翻译结果
#[derive(Debug, Clone)]
pub struct TranslationResult {
    /// Guest虚拟地址
    pub gva: GuestAddr,
    /// Guest物理地址
    pub gpa: GuestAddr,
    /// 页面大小
    pub page_size: u64,
    /// 页表标志
    pub flags: PageTableFlags,
    /// 是否来自缓存
    pub from_cache: bool,
    /// 翻译时间（纳秒）
    pub translation_time_ns: u64,
}

/// 访问历史记录
#[derive(Debug, Clone)]
pub struct AccessHistory {
    /// 地址序列
    pub addresses: VecDeque<GuestAddr>,
    /// 时间戳序列
    pub timestamps: VecDeque<Instant>,
    /// 访问类型序列
    pub access_types: VecDeque<AccessType>,
    /// 最大历史记录数
    pub max_history: usize,
}

impl AccessHistory {
    /// 创建新的访问历史
    pub fn new(max_history: usize) -> Self {
        Self {
            addresses: VecDeque::with_capacity(max_history),
            timestamps: VecDeque::with_capacity(max_history),
            access_types: VecDeque::with_capacity(max_history),
            max_history,
        }
    }

    /// 添加访问记录
    pub fn add_access(&mut self, gva: GuestAddr, access_type: AccessType) {
        self.addresses.push_back(gva);
        self.timestamps.push_back(Instant::now());
        self.access_types.push_back(access_type);

        // 保持历史记录大小
        if self.addresses.len() > self.max_history {
            self.addresses.pop_front();
            self.timestamps.pop_front();
            self.access_types.pop_front();
        }
    }

    /// 分析访问模式
    pub fn analyze_pattern(&self) -> AccessPattern {
        if self.addresses.len() < 3 {
            return AccessPattern::Unknown;
        }

        let addresses: Vec<_> = self.addresses.iter().cloned().collect();
        let mut differences = Vec::with_capacity(addresses.len() - 1);

        for i in 1..addresses.len() {
            differences.push(addresses[i].0.wrapping_sub(addresses[i - 1].0));
        }

        // 检查是否为顺序访问
        let sequential_count = differences
            .iter()
            .filter(|&&diff| diff == 4096 || diff == 1)
            .count();
        if sequential_count as f64 / differences.len() as f64 > 0.8 {
            return AccessPattern::Sequential;
        }

        // 检查是否为步长访问
        if differences.len() >= 2 {
            let first_diff = differences[0];
            let stride_count = differences
                .iter()
                .filter(|&&diff| diff == first_diff)
                .count();
            if stride_count as f64 / differences.len() as f64 > 0.8 {
                return AccessPattern::Strided { stride: first_diff };
            }
        }

        // 默认为随机访问
        AccessPattern::Random
    }

    /// 预测下一个访问地址
    pub fn predict_next_address(&self) -> Option<GuestAddr> {
        if self.addresses.is_empty() {
            return None;
        }

        let pattern = self.analyze_pattern();
        let last_addr = match self.addresses.back() {
            Some(addr) => *addr,
            None => return None,
        };

        match pattern {
            AccessPattern::Sequential => Some(last_addr + 4096), // 下一页
            AccessPattern::Strided { stride } => Some(last_addr + stride),
            _ => None,
        }
    }
}

/// 预取配置
#[derive(Debug, Clone)]
pub struct PrefetchConfig {
    /// 预取策略
    pub strategy: PrefetchStrategy,
    /// 固定预取距离（页数）
    pub fixed_distance: usize,
    /// 最大预取距离（页数）
    pub max_distance: usize,
    /// 预取队列大小
    pub prefetch_queue_size: usize,
    /// 预取超时时间（毫秒）
    pub prefetch_timeout_ms: u64,
    /// 访问历史大小
    pub access_history_size: usize,
    /// 预取命中率阈值
    pub prefetch_hit_threshold: f64,
    /// 是否启用自适应预取
    pub enable_adaptive: bool,
}

impl Default for PrefetchConfig {
    fn default() -> Self {
        Self {
            strategy: PrefetchStrategy::Adaptive,
            fixed_distance: 2,
            max_distance: 8,
            prefetch_queue_size: 64,
            prefetch_timeout_ms: 100,
            access_history_size: 32,
            prefetch_hit_threshold: 0.7,
            enable_adaptive: true,
        }
    }
}

/// 预取统计信息
#[derive(Debug, Default)]
pub struct PrefetchStats {
    /// 总翻译次数
    pub total_translations: AtomicU64,
    /// 缓存命中次数
    pub cache_hits: AtomicU64,
    /// 预取次数
    pub prefetch_count: AtomicU64,
    /// 预取命中次数
    pub prefetch_hits: AtomicU64,
    /// 预取未命中次数
    pub prefetch_misses: AtomicU64,
    /// 总翻译时间（纳秒）
    pub total_translation_time_ns: AtomicU64,
    /// 平均翻译时间（纳秒）
    pub avg_translation_time_ns: AtomicU64,
}

impl Clone for PrefetchStats {
    fn clone(&self) -> Self {
        Self {
            total_translations: AtomicU64::new(self.total_translations.load(Ordering::Relaxed)),
            cache_hits: AtomicU64::new(self.cache_hits.load(Ordering::Relaxed)),
            prefetch_count: AtomicU64::new(self.prefetch_count.load(Ordering::Relaxed)),
            prefetch_hits: AtomicU64::new(self.prefetch_hits.load(Ordering::Relaxed)),
            prefetch_misses: AtomicU64::new(self.prefetch_misses.load(Ordering::Relaxed)),
            total_translation_time_ns: AtomicU64::new(
                self.total_translation_time_ns.load(Ordering::Relaxed),
            ),
            avg_translation_time_ns: AtomicU64::new(
                self.avg_translation_time_ns.load(Ordering::Relaxed),
            ),
        }
    }
}

impl PrefetchStats {
    /// 获取统计信息快照
    pub fn snapshot(&self) -> PrefetchStatsSnapshot {
        let total = self.total_translations.load(Ordering::Relaxed);
        PrefetchStatsSnapshot {
            total_translations: total,
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            prefetch_count: self.prefetch_count.load(Ordering::Relaxed),
            prefetch_hits: self.prefetch_hits.load(Ordering::Relaxed),
            prefetch_misses: self.prefetch_misses.load(Ordering::Relaxed),
            total_translation_time_ns: self.total_translation_time_ns.load(Ordering::Relaxed),
            avg_translation_time_ns: if total > 0 {
                self.total_translation_time_ns.load(Ordering::Relaxed) / total
            } else {
                0
            },
        }
    }

    /// 重置统计信息
    pub fn reset(&self) {
        self.total_translations.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.prefetch_count.store(0, Ordering::Relaxed);
        self.prefetch_hits.store(0, Ordering::Relaxed);
        self.prefetch_misses.store(0, Ordering::Relaxed);
        self.total_translation_time_ns.store(0, Ordering::Relaxed);
        self.avg_translation_time_ns.store(0, Ordering::Relaxed);
    }
}

/// 预取统计信息快照
#[derive(Debug, Clone)]
pub struct PrefetchStatsSnapshot {
    pub total_translations: u64,
    pub cache_hits: u64,
    pub prefetch_count: u64,
    pub prefetch_hits: u64,
    pub prefetch_misses: u64,
    pub total_translation_time_ns: u64,
    pub avg_translation_time_ns: u64,
}

impl PrefetchStatsSnapshot {
    /// 计算缓存命中率
    pub fn cache_hit_rate(&self) -> f64 {
        if self.total_translations == 0 {
            0.0
        } else {
            self.cache_hits as f64 / self.total_translations as f64
        }
    }

    /// 计算预取命中率
    pub fn prefetch_hit_rate(&self) -> f64 {
        let total_prefetches = self.prefetch_hits + self.prefetch_misses;
        if total_prefetches == 0 {
            0.0
        } else {
            self.prefetch_hits as f64 / total_prefetches as f64
        }
    }
}

/// 地址翻译预取器
pub struct TranslationPrefetcher {
    /// 配置
    config: PrefetchConfig,
    /// 翻译缓存
    translation_cache: Arc<Mutex<HashMap<(GuestAddr, u16), TranslationResult>>>,
    /// 预取队列
    prefetch_queue: Arc<Mutex<VecDeque<TranslationRequest>>>,
    /// 访问历史
    access_history: Arc<Mutex<HashMap<u16, AccessHistory>>>, // 按ASID分组
    /// 统计信息
    stats: Arc<PrefetchStats>,
    /// 地址翻译函数
    translate_fn:
        Box<dyn Fn(GuestAddr, u16, AccessType) -> Result<PageWalkResult, VmError> + Send + Sync>,
}

/// Type alias for translation cache to reduce complexity
type TranslationCacheGuard<'a> =
    std::sync::MutexGuard<'a, HashMap<(GuestAddr, u16), TranslationResult>>;

impl TranslationPrefetcher {
    /// Helper: Lock translation cache
    fn lock_translation_cache(&self) -> Result<TranslationCacheGuard<'_>, VmError> {
        self.translation_cache.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "translation_cache".to_string(),
            })
        })
    }

    /// Helper: Lock prefetch queue
    fn lock_prefetch_queue(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, VecDeque<TranslationRequest>>, VmError> {
        self.prefetch_queue.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "prefetch_queue".to_string(),
            })
        })
    }

    /// Helper: Lock access history
    fn lock_access_history(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<u16, AccessHistory>>, VmError> {
        self.access_history.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "access_history".to_string(),
            })
        })
    }
    /// 创建新的预取器
    pub fn new<F>(config: PrefetchConfig, translate_fn: F) -> Self
    where
        F: Fn(GuestAddr, u16, AccessType) -> Result<PageWalkResult, VmError>
            + Send
            + Sync
            + 'static,
    {
        Self {
            config,
            translation_cache: Arc::new(Mutex::new(HashMap::new())),
            prefetch_queue: Arc::new(Mutex::new(VecDeque::new())),
            access_history: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(PrefetchStats::default()),
            translate_fn: Box::new(translate_fn),
        }
    }

    /// 使用默认配置创建预取器
    pub fn with_default_config<F>(translate_fn: F) -> Self
    where
        F: Fn(GuestAddr, u16, AccessType) -> Result<PageWalkResult, VmError>
            + Send
            + Sync
            + 'static,
    {
        Self::new(PrefetchConfig::default(), translate_fn)
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> PrefetchStatsSnapshot {
        self.stats.snapshot()
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        self.stats.reset();
    }

    /// 翻译地址（带预取）
    pub fn translate(
        &self,
        gva: GuestAddr,
        asid: u16,
        access_type: AccessType,
    ) -> Result<TranslationResult, VmError> {
        let start_time = Instant::now();

        // 更新统计信息
        self.stats
            .total_translations
            .fetch_add(1, Ordering::Relaxed);

        // 检查缓存
        let page_base = GuestAddr(gva.0 & !(4096 - 1)); // 页对齐
        let cache_key = (page_base, asid);

        {
            let cache = self.lock_translation_cache()?;
            if let Some(cached_result) = cache.get(&cache_key) {
                self.stats.cache_hits.fetch_add(1, Ordering::Relaxed);

                // 更新访问历史
                self.update_access_history(gva, asid, access_type);

                // 触发预取
                self.trigger_prefetch(gva, asid, access_type);

                return Ok(TranslationResult {
                    gva,
                    gpa: cached_result.gpa + (gva - cached_result.gva),
                    page_size: cached_result.page_size,
                    flags: cached_result.flags,
                    from_cache: true,
                    translation_time_ns: start_time.elapsed().as_nanos() as u64,
                });
            }
        }

        // 缓存未命中，进行翻译
        let page_walk_result = (self.translate_fn)(gva, asid, access_type)?;
        let translation_time = start_time.elapsed().as_nanos() as u64;

        // 更新统计信息
        self.stats
            .total_translation_time_ns
            .fetch_add(translation_time, Ordering::Relaxed);

        // 更新缓存
        {
            let mut cache = self.lock_translation_cache()?;
            cache.insert(
                cache_key,
                TranslationResult {
                    gva: page_base,
                    gpa: GuestAddr(page_walk_result.gpa & !(4096 - 1)),
                    page_size: page_walk_result.page_size,
                    flags: page_walk_result.flags,
                    from_cache: false,
                    translation_time_ns: translation_time,
                },
            );
        }

        // 更新访问历史
        self.update_access_history(gva, asid, access_type);

        // 触发预取
        self.trigger_prefetch(gva, asid, access_type);

        Ok(TranslationResult {
            gva,
            gpa: page_walk_result.gpa,
            page_size: page_walk_result.page_size,
            flags: page_walk_result.flags,
            from_cache: false,
            translation_time_ns: translation_time,
        })
    }

    /// 更新访问历史
    fn update_access_history(&self, gva: GuestAddr, asid: u16, access_type: AccessType) {
        if let Ok(mut histories) = self.lock_access_history() {
            let history = histories
                .entry(asid)
                .or_insert_with(|| AccessHistory::new(self.config.access_history_size));
            history.add_access(gva, access_type);
        }
    }

    /// 触发预取
    fn trigger_prefetch(&self, gva: GuestAddr, asid: u16, access_type: AccessType) {
        match self.config.strategy {
            PrefetchStrategy::None => (),
            PrefetchStrategy::FixedDistance => self.prefetch_fixed_distance(gva, asid, access_type),
            PrefetchStrategy::Adaptive => self.prefetch_adaptive(gva, asid, access_type),
            PrefetchStrategy::PatternBased => self.prefetch_pattern_based(gva, asid, access_type),
            PrefetchStrategy::StreamBased => self.prefetch_stream_based(gva, asid, access_type),
        }
    }

    /// 固定距离预取
    fn prefetch_fixed_distance(&self, gva: GuestAddr, asid: u16, access_type: AccessType) {
        let page_size = 4096;
        let current_page = gva.0 & !(page_size - 1);

        for i in 1..=self.config.fixed_distance {
            let prefetch_gva = GuestAddr(current_page + i as u64 * page_size);
            self.enqueue_prefetch_request(prefetch_gva, asid, access_type);
        }
    }

    /// 自适应预取
    fn prefetch_adaptive(&self, gva: GuestAddr, asid: u16, access_type: AccessType) {
        if let Ok(histories) = self.lock_access_history()
            && let Some(history) = histories.get(&asid)
        {
            let pattern = history.analyze_pattern();

            match pattern {
                AccessPattern::Sequential => {
                    // 顺序访问，预取后续几页
                    let page_size = 4096;
                    let current_page = gva.0 & !(page_size - 1);

                    // 根据历史命中率调整预取距离
                    let stats = self.stats.snapshot();
                    let prefetch_distance =
                        if stats.prefetch_hit_rate() > self.config.prefetch_hit_threshold {
                            self.config.max_distance.min(self.config.fixed_distance * 2)
                        } else {
                            self.config.fixed_distance
                        };

                    for i in 1..=prefetch_distance {
                        let prefetch_gva = GuestAddr(current_page + i as u64 * page_size);
                        self.enqueue_prefetch_request(prefetch_gva, asid, access_type);
                    }
                }
                AccessPattern::Strided { stride } => {
                    // 步长访问，预取下一个步长位置
                    let prefetch_gva = GuestAddr(gva.0 + stride);
                    self.enqueue_prefetch_request(prefetch_gva, asid, access_type);
                }
                _ => {
                    // 随机访问，不预取
                }
            }
        }
    }

    /// 基于模式的预取
    fn prefetch_pattern_based(&self, _gva: GuestAddr, asid: u16, access_type: AccessType) {
        if let Ok(histories) = self.lock_access_history()
            && let Some(history) = histories.get(&asid)
            && let Some(next_addr) = history.predict_next_address()
        {
            self.enqueue_prefetch_request(next_addr, asid, access_type);
        }
    }

    /// 流式预取
    fn prefetch_stream_based(&self, gva: GuestAddr, asid: u16, access_type: AccessType) {
        // 检查是否为连续访问
        if let Ok(histories) = self.lock_access_history()
            && let Some(history) = histories.get(&asid)
            && history.addresses.len() >= 2
        {
            let last_addr = match history.addresses.back() {
                Some(addr) => *addr,
                None => return,
            };
            let second_last_addr = history.addresses[history.addresses.len() - 2];

            // 如果是连续页访问，预取后续流
            if (last_addr.0 & !(4096 - 1)) == (second_last_addr.0 & !(4096 - 1)) + 4096 {
                let page_size = 4096;
                let current_page = gva.0 & !(page_size - 1);

                for i in 1..=self.config.fixed_distance {
                    let prefetch_gva = GuestAddr(current_page + i as u64 * page_size);
                    self.enqueue_prefetch_request(prefetch_gva, asid, access_type);
                }
            }
        }
    }

    /// 将预取请求加入队列
    fn enqueue_prefetch_request(&self, gva: GuestAddr, asid: u16, access_type: AccessType) {
        if let Ok(mut queue) = self.lock_prefetch_queue() {
            // 检查队列大小
            if queue.len() >= self.config.prefetch_queue_size {
                return;
            }

            // 检查是否已在队列中
            for request in queue.iter() {
                if request.gva == gva && request.asid == asid {
                    return;
                }
            }

            // 添加到队列
            queue.push_back(TranslationRequest {
                gva,
                asid,
                access_type,
                timestamp: Instant::now(),
                is_prefetch: true,
            });

            self.stats.prefetch_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 处理预取队列
    pub fn process_prefetch_queue(&self) {
        if let Ok(mut queue) = self.lock_prefetch_queue() {
            let now = Instant::now();
            let timeout = Duration::from_millis(self.config.prefetch_timeout_ms);

            // 收集超时的请求
            let mut expired_requests = Vec::new();
            let mut remaining_requests = VecDeque::new();

            while let Some(request) = queue.pop_front() {
                if now.duration_since(request.timestamp) > timeout {
                    expired_requests.push(request);
                } else {
                    remaining_requests.push_back(request);
                }
            }

            // 更新队列
            *queue = remaining_requests;

            // 处理超时的请求
            drop(queue); // 释放锁

            for request in expired_requests {
                self.process_prefetch_request(request);
            }
        }
    }

    /// 处理单个预取请求
    fn process_prefetch_request(&self, request: TranslationRequest) {
        let page_base = GuestAddr(request.gva.0 & !(4096 - 1)); // 页对齐
        let cache_key = (page_base, request.asid);

        // 检查是否已在缓存中
        {
            if let Ok(cache) = self.lock_translation_cache()
                && cache.contains_key(&cache_key)
            {
                self.stats.prefetch_hits.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }

        // 执行预取翻译
        match (self.translate_fn)(request.gva, request.asid, request.access_type) {
            Ok(page_walk_result) => {
                // 更新缓存
                if let Ok(mut cache) = self.lock_translation_cache() {
                    cache.insert(
                        cache_key,
                        TranslationResult {
                            gva: page_base,
                            gpa: GuestAddr(page_walk_result.gpa & !(4096 - 1)),
                            page_size: page_walk_result.page_size,
                            flags: page_walk_result.flags,
                            from_cache: false,
                            translation_time_ns: 0,
                        },
                    );

                    self.stats.prefetch_hits.fetch_add(1, Ordering::Relaxed);
                }
            }
            Err(_) => {
                self.stats.prefetch_misses.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// 清理翻译缓存
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.lock_translation_cache() {
            cache.clear();
        }
    }

    /// 清理访问历史
    pub fn clear_history(&self) {
        if let Ok(mut histories) = self.lock_access_history() {
            histories.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmu::PageTableFlags;

    fn mock_translate_fn(
        gva: GuestAddr,
        _asid: u16,
        _access_type: AccessType,
    ) -> Result<PageWalkResult, VmError> {
        Ok(PageWalkResult {
            gpa: gva + 0x1000_0000,
            page_size: 4096,
            flags: PageTableFlags::default(),
        })
    }

    #[test]
    fn test_access_history() {
        let mut history = AccessHistory::new(5);

        history.add_access(GuestAddr(0x1000), AccessType::Read);
        history.add_access(GuestAddr(0x2000), AccessType::Read);
        history.add_access(GuestAddr(0x3000), AccessType::Read);

        assert_eq!(history.addresses.len(), 3);
        assert_eq!(history.addresses[0], GuestAddr(0x1000));
        assert_eq!(history.addresses[2], GuestAddr(0x3000));

        let pattern = history.analyze_pattern();
        assert_eq!(pattern, AccessPattern::Sequential);

        let next_addr = history.predict_next_address();
        assert_eq!(next_addr, Some(GuestAddr(0x4000)));
    }

    #[test]
    fn test_prefetcher_creation() {
        let prefetcher = TranslationPrefetcher::with_default_config(mock_translate_fn);
        let stats = prefetcher.get_stats();
        assert_eq!(stats.total_translations, 0);
    }

    #[test]
    fn test_translation() {
        let prefetcher = TranslationPrefetcher::with_default_config(mock_translate_fn);

        let result = prefetcher.translate(GuestAddr(0x1000), 0, AccessType::Read);
        assert!(result.is_ok());
        let result = match result {
            Ok(r) => r,
            Err(e) => {
                panic!("Failed to translate: {:?}", e);
            }
        };
        assert_eq!(result.gva, GuestAddr(0x1000));
        assert_eq!(result.gpa, GuestAddr(0x1000_1000));
        assert!(!result.from_cache);

        // 第二次翻译应该命中缓存
        let result = prefetcher.translate(GuestAddr(0x1000), 0, AccessType::Read);
        assert!(result.is_ok());
        let result = match result {
            Ok(r) => r,
            Err(e) => {
                panic!("Failed to translate: {:?}", e);
            }
        };
        assert_eq!(result.gva, GuestAddr(0x1000));
        assert_eq!(result.gpa, GuestAddr(0x1000_1000));
        assert!(result.from_cache);

        let stats = prefetcher.get_stats();
        assert_eq!(stats.total_translations, 2);
        assert_eq!(stats.cache_hits, 1);
    }

    #[test]
    fn test_prefetch_config() {
        let config = PrefetchConfig {
            strategy: PrefetchStrategy::FixedDistance,
            fixed_distance: 3,
            ..Default::default()
        };

        assert_eq!(config.strategy, PrefetchStrategy::FixedDistance);
        assert_eq!(config.fixed_distance, 3);
    }
}
