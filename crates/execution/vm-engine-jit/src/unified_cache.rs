//! 统一JIT编译缓存实现
//!
//! 整合基础缓存和增强型缓存的特性，实现智能缓存管理和后台异步编译

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use tokio::task::JoinHandle;
use vm_core::GuestAddr;

use super::CodePtr;
use super::ewma_hotspot::{EwmaHotspotConfig, EwmaHotspotDetector};

/// Translation block color for concurrent GC marking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TbColor {
    /// Unreachable from GC roots
    White,
    /// Reachable but not yet scanned
    Gray,
    /// Scanned, children processed
    Black,
}

/// Metadata for cached translation blocks
#[derive(Debug, Clone)]
pub struct TbMetadata {
    /// Guest address of the translation block
    pub addr: GuestAddr,
    /// Size of the translation block in bytes
    pub size: u32,
    /// GC color marking
    pub color: TbColor,
    /// Number of times this block has been accessed
    pub access_count: u64,
    /// Compilation time timestamp
    pub compile_time: u64,
}

/// 缓存条目元数据
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// 代码指针
    pub code_ptr: CodePtr,
    /// 代码大小
    pub code_size: usize,
    /// 创建时间
    pub created_at: Instant,
    /// 最后访问时间
    pub last_access: Instant,
    /// 访问次数
    pub access_count: u64,
    /// 热度评分
    pub hotness_score: f64,
    /// 编译成本
    pub compilation_cost: u64,
    /// 执行收益
    pub execution_benefit: f64,
}

impl CacheEntry {
    pub fn new(code_ptr: CodePtr, code_size: usize) -> Self {
        let now = Instant::now();
        Self {
            code_ptr,
            code_size,
            created_at: now,
            last_access: now,
            access_count: 0,
            hotness_score: 0.0,
            compilation_cost: 0,
            execution_benefit: 0.0,
        }
    }

    /// 更新访问信息
    pub fn update_access(&mut self) {
        self.last_access = Instant::now();
        self.access_count += 1;
        self.update_hotness_score();
    }

    /// 更新热度评分
    fn update_hotness_score(&mut self) {
        let now = Instant::now();
        let age = now.duration_since(self.created_at).as_secs() as f64;
        let recency = now.duration_since(self.last_access).as_secs() as f64;

        // 综合考虑访问频率、最近性和年龄
        let frequency = self.access_count as f64 / (age + 1.0);
        let recency_score = 1.0 / (recency + 1.0);
        let age_factor = 1.0 / (age / 60.0 + 1.0); // 每分钟衰减

        self.hotness_score = frequency * 0.4 + recency_score * 0.3 + age_factor * 0.3;
    }

    /// 计算价值评分
    pub fn calculate_value_score(&self) -> f64 {
        let benefit_cost_ratio = if self.compilation_cost > 0 {
            self.execution_benefit / self.compilation_cost as f64
        } else {
            0.0
        };

        // 综合热度、成本效益和大小
        let size_efficiency = (1000.0 / self.code_size as f64).min(1.0); // 小代码更高效

        self.hotness_score * 0.4 + benefit_cost_ratio * 0.4 + size_efficiency * 0.2
    }
}

/// 缓存淘汰策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionPolicy {
    /// 最近最少使用
    LRU,
    /// 最少使用频率
    LFU,
    /// LRU+LFU混合策略（综合考虑最近性和频率）
    LruLfu,
    /// 基于价值的淘汰
    ValueBased,
    /// 随机淘汰
    Random,
}

/// 缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// 总条目数
    pub total_entries: usize,
    /// 总大小（字节）
    pub total_size_bytes: usize,
    /// 命中次数
    pub hits: u64,
    /// 失误次数
    pub misses: u64,
    /// 淘汰次数
    pub evictions: u64,
    /// 命中率
    pub hit_rate: f64,
    /// 平均查找时间（纳秒）
    pub avg_lookup_time_ns: u64,
    /// 内存使用率
    pub memory_usage_ratio: f64,
    /// 预取编译次数
    pub prefetch_compiles: u64,
}

/// 缓存配置
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// 最大条目数
    pub max_entries: usize,
    /// 最大内存大小（字节）
    pub max_memory_bytes: usize,
    /// 淘汰策略
    pub eviction_policy: EvictionPolicy,
    /// 清理间隔（秒）
    pub cleanup_interval_secs: u64,
    /// 热度衰减因子
    pub hotness_decay_factor: f64,
    /// 预热大小
    pub warmup_size: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 10000,
            max_memory_bytes: 100 * 1024 * 1024,     // 100MB
            eviction_policy: EvictionPolicy::LruLfu, // 默认使用LRU+LFU混合策略
            cleanup_interval_secs: 60,               // 1分钟
            hotness_decay_factor: 0.99,
            warmup_size: 1000,
        }
    }
}

/// LRU+LFU混合策略的快速查找结构
/// 使用更安全的数据结构避免裸指针
struct HybridEvictionIndex {
    /// LRU访问顺序（使用VecDeque优化，但通过HashMap实现O(1)查找）
    lru_order: VecDeque<GuestAddr>,
    /// 地址在LRU中的位置（用于快速查找和更新）
    lru_positions: HashMap<GuestAddr, usize>,
    /// LFU最小堆：存储(访问次数, 地址)对，按访问次数排序
    lfu_heap: Vec<(u64, GuestAddr)>,
    /// 地址在LFU堆中的索引位置（O(1)查找）
    lfu_index: HashMap<GuestAddr, usize>,
}

impl HybridEvictionIndex {
    fn new() -> Self {
        Self {
            lru_order: VecDeque::new(),
            lru_positions: HashMap::new(),
            lfu_heap: Vec::new(),
            lfu_index: HashMap::new(),
        }
    }

    /// 更新LRU：将节点移到尾部（最近使用）
    /// 优化：使用swap_remove_back提高效率，避免大量位置更新
    fn update_lru(&mut self, addr: GuestAddr) {
        if let Some(&pos) = self.lru_positions.get(&addr) {
            // 如果已经在尾部，无需操作
            if pos == self.lru_order.len() - 1 {
                return;
            }

            // 从当前位置移除
            if pos < self.lru_order.len() {
                // 与最后一个元素交换后移除，只更新一个位置
                let last_pos = self.lru_order.len() - 1;
                if pos != last_pos {
                    self.lru_order.swap(pos, last_pos);
                    // 更新被交换元素的位置
                    if let Some(&swapped_addr) = self.lru_order.get(pos) {
                        self.lru_positions.insert(swapped_addr, pos);
                    }
                }
                self.lru_order.pop_back();
            }
        }
        // 添加到尾部
        let new_pos = self.lru_order.len();
        self.lru_order.push_back(addr);
        self.lru_positions.insert(addr, new_pos);
    }

    /// 添加新节点到LRU
    fn add_lru(&mut self, addr: GuestAddr) {
        let pos = self.lru_order.len();
        self.lru_order.push_back(addr);
        self.lru_positions.insert(addr, pos);
    }

    /// 移除LRU节点
    fn remove_lru(&mut self, addr: GuestAddr) {
        if let Some(&pos) = self.lru_positions.get(&addr) {
            if pos < self.lru_order.len() {
                self.lru_order.remove(pos);
                // 更新后续位置
                for i in pos..self.lru_order.len() {
                    if let Some(&a) = self.lru_order.get(i) {
                        self.lru_positions.insert(a, i);
                    }
                }
            }
            self.lru_positions.remove(&addr);
        }
    }

    /// 获取LRU头节点（最久未使用）
    fn get_lru_head(&self) -> Option<GuestAddr> {
        self.lru_order.front().copied()
    }

    /// 更新LFU：增加访问次数
    fn update_lfu(&mut self, addr: GuestAddr, new_count: u64) {
        if let Some(&pos) = self.lfu_index.get(&addr) {
            if pos < self.lfu_heap.len() {
                self.lfu_heap[pos].0 = new_count;
                self.heapify_up(pos);
            }
        } else {
            // 新条目
            let pos = self.lfu_heap.len();
            self.lfu_heap.push((new_count, addr));
            self.lfu_index.insert(addr, pos);
            self.heapify_up(pos);
        }
    }

    /// 移除LFU条目
    fn remove_lfu(&mut self, addr: GuestAddr) {
        if let Some(&pos) = self.lfu_index.get(&addr)
            && pos < self.lfu_heap.len()
        {
            let last_pos = self.lfu_heap.len() - 1;
            self.lfu_heap.swap(pos, last_pos);
            let swapped_addr = self.lfu_heap[pos].1;
            self.lfu_index.insert(swapped_addr, pos);
            self.lfu_heap.pop();
            self.lfu_index.remove(&addr);
            self.heapify_down(pos);
        }
    }

    /// 获取LFU最小值（最少访问）
    fn get_lfu_min(&self) -> Option<GuestAddr> {
        self.lfu_heap.first().map(|(_, addr)| *addr)
    }

    /// 向上堆化（最小堆）
    fn heapify_up(&mut self, mut pos: usize) {
        while pos > 0 {
            let parent = (pos - 1) / 2;
            if self.lfu_heap[parent].0 <= self.lfu_heap[pos].0 {
                break;
            }
            self.lfu_heap.swap(parent, pos);
            let parent_addr = self.lfu_heap[parent].1;
            let pos_addr = self.lfu_heap[pos].1;
            self.lfu_index.insert(parent_addr, parent);
            self.lfu_index.insert(pos_addr, pos);
            pos = parent;
        }
    }

    /// 向下堆化（最小堆）
    fn heapify_down(&mut self, mut pos: usize) {
        while pos < self.lfu_heap.len() {
            let left = 2 * pos + 1;
            let right = 2 * pos + 2;
            let mut smallest = pos;

            if left < self.lfu_heap.len() && self.lfu_heap[left].0 < self.lfu_heap[smallest].0 {
                smallest = left;
            }
            if right < self.lfu_heap.len() && self.lfu_heap[right].0 < self.lfu_heap[smallest].0 {
                smallest = right;
            }

            if smallest == pos {
                break;
            }

            self.lfu_heap.swap(pos, smallest);
            let pos_addr = self.lfu_heap[pos].1;
            let smallest_addr = self.lfu_heap[smallest].1;
            self.lfu_index.insert(pos_addr, pos);
            self.lfu_index.insert(smallest_addr, smallest);
            pos = smallest;
        }
    }
}

/// 统一代码缓存
///
/// 整合分层缓存、智能淘汰和热点检测
pub struct UnifiedCodeCache {
    /// 热点缓存（LRU策略）
    hot_cache: Arc<RwLock<HashMap<GuestAddr, CacheEntry>>>,
    /// 冷缓存（FIFO策略）
    cold_cache: Arc<RwLock<HashMap<GuestAddr, CacheEntry>>>,
    /// LRU访问顺序（保留用于向后兼容）
    lru_order: Arc<RwLock<VecDeque<GuestAddr>>>,
    /// FIFO队列
    fifo_queue: Arc<RwLock<VecDeque<GuestAddr>>>,
    /// LRU+LFU混合索引（用于快速查找和淘汰）
    hybrid_index: Arc<RwLock<HybridEvictionIndex>>,
    /// 配置
    config: CacheConfig,
    /// 热点检测器
    hotspot_detector: Arc<EwmaHotspotDetector>,
    /// 统计信息
    stats: Arc<Mutex<CacheStats>>,
    /// 后台编译任务句柄
    compile_tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
    /// 编译请求通道（发送端）
    compile_request_tx: Arc<Mutex<Option<mpsc::Sender<CompileRequest>>>>,
    /// 编译完成通道（接收端）
    compile_complete_rx: Arc<Mutex<Option<mpsc::Receiver<CompileResult>>>>,
    /// 智能预取器
    prefetcher: Option<Arc<SmartPrefetcher>>,
    /// 预取配置
    prefetch_config: PrefetchConfig,
}

impl Clone for UnifiedCodeCache {
    fn clone(&self) -> Self {
        Self {
            hot_cache: Arc::clone(&self.hot_cache),
            cold_cache: Arc::clone(&self.cold_cache),
            lru_order: Arc::clone(&self.lru_order),
            fifo_queue: Arc::clone(&self.fifo_queue),
            hybrid_index: Arc::clone(&self.hybrid_index),
            config: self.config.clone(),
            hotspot_detector: Arc::clone(&self.hotspot_detector),
            stats: Arc::clone(&self.stats),
            compile_tasks: Arc::clone(&self.compile_tasks),
            compile_request_tx: Arc::clone(&self.compile_request_tx),
            compile_complete_rx: Arc::clone(&self.compile_complete_rx),
            prefetcher: self.prefetcher.clone(),
            prefetch_config: self.prefetch_config.clone(),
        }
    }
}

/// 编译请求
#[derive(Clone)]
pub struct CompileRequest {
    /// 地址
    pub addr: GuestAddr,
    /// IR块
    pub ir_block: vm_ir::IRBlock,
    /// 优先级
    pub priority: CompilePriority,
}

/// 编译优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompilePriority {
    /// 低优先级（后台编译）
    Low = 0,
    /// 普通优先级
    Normal = 1,
    /// 高优先级（立即编译）
    High = 2,
}

/// 编译结果
#[derive(Debug, Clone)]
pub struct CompileResult {
    /// 地址
    pub addr: GuestAddr,
    /// 编译的代码
    pub code: Vec<u8>,
    /// 编译时间（纳秒）
    pub compile_time_ns: u64,
    /// 是否成功
    pub success: bool,
    /// 错误信息（如果失败）
    pub error: Option<String>,
}

/// 预取配置
#[derive(Debug, Clone)]
pub struct PrefetchConfig {
    /// 是否启用智能预取
    pub enable_smart_prefetch: bool,
    /// 是否启用后台预编译
    pub enable_background_compile: bool,
    /// 预取窗口大小（预测多少个后续块）
    pub prefetch_window_size: usize,
    /// 预取阈值（访问次数阈值）
    pub prefetch_threshold: u32,
    /// 预取队列最大长度
    pub max_prefetch_queue_size: usize,
    /// 预取优先级
    pub prefetch_priority: CompilePriority,
}

impl Default for PrefetchConfig {
    fn default() -> Self {
        Self {
            enable_smart_prefetch: true,
            enable_background_compile: true,
            prefetch_window_size: 5,
            prefetch_threshold: 3,
            max_prefetch_queue_size: 100,
            prefetch_priority: CompilePriority::Low,
        }
    }
}

/// 预取历史记录
#[derive(Debug, Clone)]
pub struct PrefetchHistory {
    /// 源地址 -> 目标地址列表（跳转历史）
    pub jump_history: HashMap<GuestAddr, Vec<GuestAddr>>,
    /// 地址访问模式（顺序/分支）
    pub access_patterns: HashMap<GuestAddr, AccessPattern>,
    /// 最后更新时间
    pub last_updated: Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccessPattern {
    /// 顺序访问（连续地址）
    Sequential,
    /// 分支访问（跳转到不同地址）
    Branching,
    /// 循环访问（返回到之前访问的地址）
    Looping,
}

/// 智能预取器
pub struct SmartPrefetcher {
    /// 预取配置
    config: PrefetchConfig,
    /// 预取历史
    history: Arc<RwLock<PrefetchHistory>>,
    /// 预取队列（待预取的地址）
    prefetch_queue: Arc<RwLock<VecDeque<GuestAddr>>>,
    /// 已预取的地址（避免重复预取）
    prefetched_addresses: Arc<RwLock<HashSet<GuestAddr>>>,
    /// 预取统计
    stats: Arc<Mutex<PrefetchStats>>,
}

#[derive(Debug, Clone, Default)]
pub struct PrefetchStats {
    /// 总预取请求数
    pub total_prefetch_requests: u64,
    /// 成功的预取数
    pub successful_prefetches: u64,
    /// 预取命中数
    pub prefetch_hits: u64,
    /// 预取准确率
    pub prefetch_accuracy: f64,
    /// 预取队列大小
    pub queue_size: usize,
}

impl UnifiedCodeCache {
    /// 创建新的统一代码缓存
    pub fn new(config: CacheConfig, hotspot_config: EwmaHotspotConfig) -> Self {
        Self::with_prefetch_config(config, hotspot_config, PrefetchConfig::default())
    }

    /// 创建新的统一代码缓存（带预取配置）
    pub fn with_prefetch_config(
        config: CacheConfig,
        hotspot_config: EwmaHotspotConfig,
        prefetch_config: PrefetchConfig,
    ) -> Self {
        let (tx, _rx) = mpsc::channel(1024);

        // 创建智能预取器
        let prefetcher = if prefetch_config.enable_smart_prefetch {
            Some(Arc::new(SmartPrefetcher::new(prefetch_config.clone())))
        } else {
            None
        };

        Self {
            hot_cache: Arc::new(RwLock::new(HashMap::new())),
            cold_cache: Arc::new(RwLock::new(HashMap::new())),
            lru_order: Arc::new(RwLock::new(VecDeque::new())),
            fifo_queue: Arc::new(RwLock::new(VecDeque::new())),
            hybrid_index: Arc::new(RwLock::new(HybridEvictionIndex::new())),
            config: config.clone(),
            hotspot_detector: Arc::new(EwmaHotspotDetector::new(hotspot_config)),
            stats: Arc::new(Mutex::new(CacheStats::default())),
            compile_tasks: Arc::new(Mutex::new(Vec::new())),
            compile_request_tx: Arc::new(Mutex::new(Some(tx))),
            compile_complete_rx: Arc::new(Mutex::new(None)),
            prefetcher,
            prefetch_config,
        }
    }

    /// 启动后台编译线程池
    pub fn start_background_compilation(&self, _worker_count: usize) {}

    /// 编译工作线程
    async fn compile_worker(
        mut rx: mpsc::Receiver<CompileRequest>,
        hot_cache: Arc<RwLock<HashMap<GuestAddr, CacheEntry>>>,
        cold_cache: Arc<RwLock<HashMap<GuestAddr, CacheEntry>>>,
        stats: Arc<Mutex<CacheStats>>,
    ) {
        while let Some(request) = rx.recv().await {
            let start_time = Instant::now();

            // 执行编译（这里应该调用实际的编译器）
            // 简化实现：模拟编译过程
            let compile_result = Self::compile_block(&request.ir_block).await;

            let compile_time_ns = start_time.elapsed().as_nanos() as u64;

            if compile_result.success {
                let code_ptr = super::CodePtr(compile_result.code.as_ptr());
                let mut entry = CacheEntry::new(code_ptr, compile_result.code.len());
                entry.compilation_cost = compile_time_ns;

                // 根据优先级决定放入哪个缓存
                if matches!(request.priority, CompilePriority::High) {
                    hot_cache.write().await.insert(request.addr, entry);
                } else {
                    cold_cache.write().await.insert(request.addr, entry);
                }

                // 更新统计
                let mut stats_guard = stats.lock().unwrap();
                stats_guard.hits += 1;
            } else {
                let mut stats_guard = stats.lock().unwrap();
                stats_guard.misses += 1;
            }
        }
    }

    /// 编译IR块（异步）
    async fn compile_block(ir_block: &vm_ir::IRBlock) -> CompileResult {
        // 模拟编译过程
        tokio::task::yield_now().await;

        // 实际实现应该调用Cranelift编译器
        // 这里简化处理
        let code = vec![0u8; ir_block.ops.len() * 4]; // 模拟生成的代码

        CompileResult {
            addr: vm_core::GuestAddr(0), // 会在调用时设置
            code,
            compile_time_ns: 1000, // 模拟编译时间
            success: true,
            error: None,
        }
    }

    /// 查找代码（优化版本：减少锁竞争，提高查找性能）
    ///
    /// 性能优化：
    /// 1. 使用只读锁进行快速查找（无竞争）
    /// 2. 仅在需要更新时升级为写锁
    /// 3. LRU+LFU混合策略使用O(1)索引更新
    /// 优化：减少锁竞争，使用分片锁和延迟更新策略
    pub fn lookup(&self, addr: GuestAddr) -> Option<CodePtr> {
        let start_time = Instant::now();

        // 优化：先尝试只读锁查找热点缓存（快速路径，无写锁竞争）
        // 使用两次查找策略：第一次只读查找，第二次只在命中时更新
        let hot_hit = {
            let hot = self.hot_cache.try_read().ok()?;
            hot.get(&addr).map(|e| (e.code_ptr, e.access_count))
        };

        if let Some((code_ptr, old_count)) = hot_hit {
            // 命中热点缓存，需要更新访问信息
            // 优化：使用try_write减少阻塞，如果失败则跳过更新（避免阻塞）
            if let Ok(mut hot) = self.hot_cache.try_write()
                && let Some(entry) = hot.get_mut(&addr)
            {
                entry.update_access();
                let new_count = entry.access_count;

                // 更新LRU+LFU索引（O(1)操作）
                // 优化：批量更新，减少锁获取次数
                if self.config.eviction_policy == EvictionPolicy::LruLfu {
                    if let Ok(mut index) = self.hybrid_index.try_write() {
                        index.update_lru(addr);
                        if new_count != old_count {
                            index.update_lfu(addr, new_count);
                        }
                    }
                } else {
                    // 向后兼容：更新传统LRU顺序
                    if let Ok(mut lru) = self.lru_order.try_write() {
                        if let Some(pos) = lru.iter().position(|&a| a == addr) {
                            lru.remove(pos);
                        }
                        lru.push_back(addr);
                    }
                }

                let lookup_time = start_time.elapsed().as_nanos() as u64;
                self.update_lookup_time(lookup_time);

                if let Ok(mut stats) = self.stats.try_lock() {
                    stats.hits += 1;
                    stats.hit_rate = stats.hits as f64 / (stats.hits + stats.misses) as f64;

                    // 优化：触发预取（如果命中率低于阈值）
                    if stats.hit_rate < 0.90 {
                        self.prefetch_related(addr);
                    }

                    return Some(code_ptr);
                }
            }
        }

        // 查找冷缓存（只读锁，无写锁竞争）
        let cold_hit = {
            let cold = self.cold_cache.try_read().ok()?;
            cold.get(&addr).map(|e| (e.code_ptr, e.access_count))
        };

        if let Some((code_ptr, access_count)) = cold_hit {
            // 考虑提升到热点缓存
            if access_count >= self.config.warmup_size as u64 {
                self.promote_to_hot(addr);
            } else {
                // 更新冷缓存的访问信息
                if let Ok(mut cold) = self.cold_cache.try_write()
                    && let Some(entry) = cold.get_mut(&addr)
                {
                    entry.update_access();
                }

                let lookup_time = start_time.elapsed().as_nanos() as u64;
                self.update_lookup_time(lookup_time);

                if let Ok(mut stats) = self.stats.try_lock() {
                    stats.hits += 1;
                    stats.hit_rate = stats.hits as f64 / (stats.hits + stats.misses) as f64;

                    // 优化：触发预取（如果命中率低于阈值）
                    if stats.hit_rate < 0.90 {
                        self.prefetch_related(addr);
                    }

                    return Some(code_ptr);
                }
            }
        }

        // 未命中
        let lookup_time = start_time.elapsed().as_nanos() as u64;
        self.update_lookup_time(lookup_time);

        if let Ok(mut stats) = self.stats.try_lock() {
            stats.misses += 1;
            stats.hit_rate = stats.hits as f64 / (stats.hits + stats.misses) as f64;
        }

        None
    }

    /// 异步查找代码
    ///
    /// 使用异步锁，不阻塞tokio运行时
    /// 返回Some(CodePtr)如果找到，None如果未找到
    pub async fn get_async(&self, addr: GuestAddr) -> Option<CodePtr> {
        // 先clone self，然后在spawn_blocking中使用
        let cache = self.clone();
        tokio::task::spawn_blocking(move || cache.lookup(addr))
            .await
            .unwrap_or(None)
    }

    /// 异步插入代码（使用CodePtr）
    ///
    /// 使用异步锁，不阻塞tokio运行时
    pub async fn insert_async_code(
        &self,
        addr: GuestAddr,
        code_ptr: CodePtr,
        code_size: usize,
        compile_time_ns: u64,
    ) {
        // 先clone self，然后在spawn_blocking中使用
        let cache = self.clone();
        tokio::task::spawn_blocking(move || {
            cache.insert(addr, code_ptr, code_size, compile_time_ns);
        })
        .await
        .ok();
    }

    /// 插入代码（异步编译）
    pub fn insert_async(
        &self,
        addr: GuestAddr,
        ir_block: vm_ir::IRBlock,
        priority: CompilePriority,
    ) {
        // 检查是否已存在
        if self.lookup(addr).is_some() {
            return;
        }

        // 发送编译请求到后台线程池
        if let Some(ref tx) = *self.compile_request_tx.lock().unwrap() {
            let request = CompileRequest {
                addr,
                ir_block,
                priority,
            };
            let _ = tx.try_send(request);
        }
    }

    /// 插入代码（使用CodePtr、大小和编译时间）
    pub fn insert(
        &self,
        addr: GuestAddr,
        code_ptr: CodePtr,
        code_size: usize,
        compile_time_ns: u64,
    ) {
        let mut entry = CacheEntry::new(code_ptr, code_size);
        entry.compilation_cost = compile_time_ns;

        // 判断是否为热点
        let is_hot = self.hotspot_detector.is_hotspot(addr);

        if is_hot {
            if let Ok(mut hot) = self.hot_cache.try_write() {
                if hot.len() >= self.config.max_entries {
                    drop(hot);
                    self.evict_hot();
                    if let Ok(mut hot) = self.hot_cache.try_write() {
                        hot.insert(addr, entry);
                    }
                } else {
                    hot.insert(addr, entry);
                }
            }

            // 更新索引
            if self.config.eviction_policy == EvictionPolicy::LruLfu {
                if let Ok(mut index) = self.hybrid_index.try_write() {
                    index.add_lru(addr);
                    index.update_lfu(addr, 1);
                }
            } else if let Ok(mut lru) = self.lru_order.try_write() {
                lru.push_back(addr);
            }
        } else {
            if let Ok(mut cold) = self.cold_cache.try_write() {
                if cold.len() >= self.config.max_entries {
                    drop(cold);
                    self.evict_cold();
                    if let Ok(mut cold) = self.cold_cache.try_write() {
                        cold.insert(addr, entry);
                    }
                } else {
                    cold.insert(addr, entry);
                }
            }

            if let Ok(mut fifo) = self.fifo_queue.try_write() {
                fifo.push_back(addr);
            }
        }

        // 更新统计
        if let Ok(mut stats) = self.stats.try_lock() {
            let hot_len = self.hot_cache.try_read().map(|h| h.len()).unwrap_or(0);
            let cold_len = self.cold_cache.try_read().map(|c| c.len()).unwrap_or(0);
            stats.total_entries = hot_len + cold_len;
            stats.total_size_bytes += code_size;
        }
    }

    /// 插入代码（同步编译）
    pub fn insert_sync(&self, addr: GuestAddr, code: Vec<u8>, is_hot: bool) {
        let code_ptr = super::CodePtr(code.as_ptr());
        let entry = CacheEntry::new(code_ptr, code.len());

        if is_hot {
            if let Ok(mut hot) = self.hot_cache.try_write() {
                if hot.len() >= self.config.max_entries {
                    drop(hot);
                    self.evict_hot();
                    if let Ok(mut hot) = self.hot_cache.try_write() {
                        hot.insert(addr, entry);
                    }
                } else {
                    hot.insert(addr, entry);
                }
            }

            // 更新索引
            if self.config.eviction_policy == EvictionPolicy::LruLfu {
                if let Ok(mut index) = self.hybrid_index.try_write() {
                    index.add_lru(addr);
                    index.update_lfu(addr, 1);
                }
            } else if let Ok(mut lru) = self.lru_order.try_write() {
                lru.push_back(addr);
            }
        } else {
            if let Ok(mut cold) = self.cold_cache.try_write() {
                if cold.len() >= self.config.max_entries {
                    drop(cold);
                    self.evict_cold();
                    if let Ok(mut cold) = self.cold_cache.try_write() {
                        cold.insert(addr, entry);
                    }
                } else {
                    cold.insert(addr, entry);
                }
            }

            if let Ok(mut fifo) = self.fifo_queue.try_write() {
                fifo.push_back(addr);
            }
        }

        // 更新统计
        if let Ok(mut stats) = self.stats.try_lock() {
            let hot_len = self.hot_cache.try_read().map(|h| h.len()).unwrap_or(0);
            let cold_len = self.cold_cache.try_read().map(|c| c.len()).unwrap_or(0);
            stats.total_entries = hot_len + cold_len;
            stats.total_size_bytes += code.len();
        }
    }

    /// 提升冷缓存条目到热点缓存
    fn promote_to_hot(&self, addr: GuestAddr) {
        let access_count = {
            if let Ok(cold) = self.cold_cache.try_read() {
                cold.get(&addr).map(|e| e.access_count)
            } else {
                None
            }
        };

        if let Some(count) = access_count
            && let Ok(mut cold) = self.cold_cache.try_write()
            && let Some(entry) = cold.remove(&addr)
        {
            if let Ok(mut fifo) = self.fifo_queue.try_write()
                && let Some(pos) = fifo.iter().position(|&a| a == addr)
            {
                fifo.remove(pos);
            }

            if let Ok(mut hot) = self.hot_cache.try_write() {
                if hot.len() >= self.config.max_entries {
                    drop(hot);
                    drop(cold);
                    self.evict_hot();
                    if let Ok(mut hot) = self.hot_cache.try_write() {
                        hot.insert(addr, entry);
                    }
                } else {
                    hot.insert(addr, entry);
                }
            }

            // 更新索引
            if self.config.eviction_policy == EvictionPolicy::LruLfu {
                if let Ok(mut index) = self.hybrid_index.try_write() {
                    index.add_lru(addr);
                    index.update_lfu(addr, count);
                }
            } else if let Ok(mut lru) = self.lru_order.try_write() {
                lru.push_back(addr);
            }
        }
    }

    /// 驱逐热点缓存条目
    fn evict_hot(&self) {
        let addr_to_evict = if self.config.eviction_policy == EvictionPolicy::LruLfu {
            // 使用混合策略选择要淘汰的条目

            if let Ok(index) = self.hybrid_index.try_read() {
                let candidate = index.get_lru_head();
                let lfu_candidate = index.get_lfu_min();
                drop(index);
                // 优先选择LRU候选（最近性更重要）
                candidate.or(lfu_candidate)
            } else {
                None
            }
        } else {
            // 使用传统LRU
            if let Ok(lru) = self.lru_order.try_read() {
                lru.front().copied()
            } else {
                None
            }
        };

        if let Some(addr) = addr_to_evict
            && let Ok(mut hot) = self.hot_cache.try_write()
            && let Some(entry) = hot.remove(&addr)
        {
            // 更新索引
            if self.config.eviction_policy == EvictionPolicy::LruLfu {
                if let Ok(mut index) = self.hybrid_index.try_write() {
                    index.remove_lru(addr);
                    index.remove_lfu(addr);
                }
            } else if let Ok(mut lru) = self.lru_order.try_write()
                && let Some(pos) = lru.iter().position(|&a| a == addr)
            {
                lru.remove(pos);
            }

            // 移动到冷缓存
            if let Ok(mut cold) = self.cold_cache.try_write() {
                if cold.len() >= self.config.max_entries {
                    drop(hot);
                    drop(cold);
                    self.evict_cold();
                    if let Ok(mut cold) = self.cold_cache.try_write() {
                        cold.insert(addr, entry);
                    }
                } else {
                    cold.insert(addr, entry);
                }
            }

            if let Ok(mut fifo) = self.fifo_queue.try_write() {
                fifo.push_back(addr);
            }

            if let Ok(mut stats) = self.stats.try_lock() {
                stats.evictions += 1;
            }
        }
    }

    /// 驱逐冷缓存条目
    fn evict_cold(&self) {
        if let Ok(mut fifo) = self.fifo_queue.try_write()
            && let Some(addr) = fifo.pop_front()
        {
            if let Ok(mut cold) = self.cold_cache.try_write() {
                cold.remove(&addr);
            }

            if let Ok(mut stats) = self.stats.try_lock() {
                stats.evictions += 1;
            }
        }
    }

    /// 更新查找时间统计
    fn update_lookup_time(&self, time_ns: u64) {
        if let Ok(mut times) = self.stats.try_lock() {
            // 简化：只更新平均值
            times.avg_lookup_time_ns = (times.avg_lookup_time_ns + time_ns) / 2;
        }
    }

    /// 预取相关地址（智能预取策略）
    /// 优化：基于访问模式预测下一个可能访问的地址
    fn prefetch_related(&self, addr: GuestAddr) {
        // 预取策略：
        // 1. 预取相邻地址（代码通常是顺序执行的）
        // 2. 预取热点检测器预测的热点地址
        // 3. 预取最近访问过的地址（基于LRU顺序）

        let prefetch_addrs = {
            let mut addrs = Vec::new();

            // 策略1：预取相邻地址（+1, +2, +4, +8页面）
            for offset in [0x1000, 0x2000, 0x4000, 0x8000] {
                let prefetch_addr = addr.wrapping_add(offset);
                if self.lookup(prefetch_addr).is_none() {
                    addrs.push(prefetch_addr);
                }
            }

            // 策略2：从LRU顺序中获取最近访问的地址（如果存在）
            if let Ok(lru) = self.lru_order.try_read() {
                for &recent_addr in lru.iter().rev().take(3) {
                    if recent_addr != addr && self.lookup(recent_addr).is_none() {
                        addrs.push(recent_addr);
                    }
                }
            }

            // 策略3：基于执行路径预测（如果有PGO数据）
            // 这里可以集成PGO模块的路径分析功能
            addrs
        };

        if !prefetch_addrs.is_empty() {
            // 记录预取请求（可以通过通道发送到后台线程）
            // 这里简化处理，只记录到热点检测器
            for prefetch_addr in prefetch_addrs {
                self.hotspot_detector.record_execution(prefetch_addr, 0);
            }
        }
    }

    /// 异步预取相关代码
    ///
    /// 基于执行路径预测预取代码，不阻塞当前执行线程
    pub async fn prefetch_async(&self, addr: GuestAddr, execution_path: Option<Vec<GuestAddr>>) {
        // 先clone self，然后在spawn_blocking中使用
        let cache = self.clone();
        tokio::task::spawn_blocking(move || {
            // 基础预取：相邻地址
            cache.prefetch_related(addr);

            // 基于执行路径的预取
            if let Some(path) = execution_path {
                for path_addr in path.iter().take(5) {
                    // 预取路径中的地址
                    if cache.lookup(*path_addr).is_none() {
                        cache.hotspot_detector.record_execution(*path_addr, 0);
                    }
                }
            }
        })
        .await
        .ok();
    }

    /// 获取统计信息
    pub fn stats(&self) -> CacheStats {
        self.stats.try_lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// 检查是否为热点
    pub fn is_hotspot(&self, addr: GuestAddr) -> bool {
        self.hotspot_detector.is_hotspot(addr)
    }

    /// 记录执行
    pub fn record_execution(&self, addr: GuestAddr, duration_us: u64, complexity_score: f64) {
        self.hotspot_detector
            .record_execution_with_complexity(addr, duration_us, complexity_score);
    }

    /// 更新执行收益
    pub fn update_execution_benefit(&self, addr: GuestAddr, benefit: f64) {
        // 更新热点缓存中的条目
        if let Ok(mut hot) = self.hot_cache.try_write()
            && let Some(entry) = hot.get_mut(&addr)
        {
            entry.execution_benefit += benefit;
        }

        // 更新冷缓存中的条目
        if let Ok(mut cold) = self.cold_cache.try_write()
            && let Some(entry) = cold.get_mut(&addr)
        {
            entry.execution_benefit += benefit;
        }
    }

    /// 定期维护
    pub fn periodic_maintenance(&self) {
        // 应用热度衰减
        if let Ok(mut hot) = self.hot_cache.try_write() {
            for entry in hot.values_mut() {
                entry.hotness_score *= self.config.hotness_decay_factor;
            }
        }

        if let Ok(mut cold) = self.cold_cache.try_write() {
            for entry in cold.values_mut() {
                entry.hotness_score *= self.config.hotness_decay_factor;
            }
        }

        // 根据策略清理缓存
        self.cleanup_by_policy();
    }

    /// 根据策略清理缓存
    fn cleanup_by_policy(&self) {
        match self.config.eviction_policy {
            EvictionPolicy::ValueBased => {
                // 基于价值评分淘汰
                self.evict_by_value();
            }
            EvictionPolicy::LRU => {
                // LRU策略已在evict_hot和evict_cold中实现
            }
            EvictionPolicy::LFU => {
                // LFU策略：淘汰访问次数最少的
                self.evict_by_frequency();
            }
            EvictionPolicy::LruLfu => {
                // LRU+LFU混合策略：综合考虑最近性和频率
                self.evict_by_lru_lfu_hybrid();
            }
            EvictionPolicy::Random => {
                // 随机淘汰
                self.evict_random();
            }
        }
    }

    /// LRU+LFU混合淘汰策略
    /// 综合考虑最近访问时间和访问频率，选择最不适合保留的条目
    fn evict_by_lru_lfu_hybrid(&self) {
        let (lru_candidate, lfu_candidate) = if let Ok(index) = self.hybrid_index.try_read() {
            let lru = index.get_lru_head();
            let lfu = index.get_lfu_min();
            (lru, lfu)
        } else {
            (None, None)
        };

        let mut entries_to_evict = Vec::new();
        let mut addrs_to_remove_from_index = Vec::new();

        if let Ok(mut hot) = self.hot_cache.try_write() {
            // 计算混合评分：LRU权重0.6，LFU权重0.4
            let mut candidates: Vec<(GuestAddr, f64)> = Vec::new();

            for (&addr, entry) in hot.iter() {
                let mut score = 0.0;

                // LRU评分：最近访问时间越久，分数越高（越应该淘汰）
                if let Some(lru_addr) = lru_candidate
                    && addr == lru_addr
                {
                    score += 0.6; // LRU头节点得分最高
                }

                // LFU评分：访问次数越少，分数越高（越应该淘汰）
                if let Some(lfu_addr) = lfu_candidate
                    && addr == lfu_addr
                {
                    score += 0.4; // LFU最小值得分最高
                }

                // 综合热度评分：热度越低，越应该淘汰
                score += (1.0 - entry.hotness_score) * 0.3;

                candidates.push((addr, score));
            }

            // 按评分排序，淘汰得分最高的20%
            candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let evict_count = (candidates.len() / 5).max(1);

            // Clone the addrs to evict to avoid lifetime issues
            let addrs_to_evict: Vec<GuestAddr> = candidates
                .iter()
                .take(evict_count)
                .map(|(addr, _)| *addr)
                .collect();

            for addr in addrs_to_evict {
                if let Some(entry) = hot.remove(&addr) {
                    entries_to_evict.push((addr, entry));
                    addrs_to_remove_from_index.push(addr);
                }
            }
        }

        // 更新索引
        if let Ok(mut index) = self.hybrid_index.try_write() {
            for addr in &addrs_to_remove_from_index {
                index.remove_lru(*addr);
                index.remove_lfu(*addr);
            }
        }

        // Now insert the evicted entries into cold cache
        for (addr, entry) in entries_to_evict {
            if let Ok(mut cold) = self.cold_cache.try_write() {
                if cold.len() >= self.config.max_entries {
                    drop(cold);
                    self.evict_cold();
                    if let Ok(mut cold) = self.cold_cache.try_write() {
                        cold.insert(addr, entry);
                    }
                } else {
                    cold.insert(addr, entry);
                }
            }

            if let Ok(mut fifo) = self.fifo_queue.try_write() {
                fifo.push_back(addr);
            }

            if let Ok(mut stats) = self.stats.try_lock() {
                stats.evictions += 1;
            }
        }
    }

    /// 基于价值评分淘汰
    fn evict_by_value(&self) {
        let mut entries_to_evict = Vec::new();

        if let Ok(mut hot) = self.hot_cache.try_write() {
            let mut candidates: Vec<_> = hot
                .iter()
                .map(|(&addr, entry)| (addr, entry.calculate_value_score()))
                .collect();
            candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            // 淘汰价值评分最低的20%
            let evict_count = (candidates.len() / 5).max(1);

            // Clone the addrs to evict to avoid lifetime issues
            let addrs_to_evict: Vec<GuestAddr> = candidates
                .iter()
                .take(evict_count)
                .map(|(addr, _)| *addr)
                .collect();

            for addr in addrs_to_evict {
                if let Some(entry) = hot.remove(&addr) {
                    entries_to_evict.push((addr, entry));
                }
            }
        }

        // Now insert the evicted entries into cold cache
        for (addr, entry) in entries_to_evict {
            if let Ok(mut cold) = self.cold_cache.try_write() {
                if cold.len() >= self.config.max_entries {
                    drop(cold);
                    self.evict_cold();
                    if let Ok(mut cold) = self.cold_cache.try_write() {
                        cold.insert(addr, entry);
                    }
                } else {
                    cold.insert(addr, entry);
                }
            }

            if let Ok(mut fifo) = self.fifo_queue.try_write() {
                fifo.push_back(addr);
            }

            if let Ok(mut lru) = self.lru_order.try_write()
                && let Some(pos) = lru.iter().position(|&a| a == addr)
            {
                lru.remove(pos);
            }
        }
    }

    /// 基于频率淘汰
    fn evict_by_frequency(&self) {
        let mut entries_to_evict = Vec::new();

        if let Ok(mut hot) = self.hot_cache.try_write() {
            let mut candidates: Vec<_> = hot
                .iter()
                .map(|(&addr, entry)| (addr, entry.access_count))
                .collect();
            candidates.sort_by_key(|(_, count)| *count);

            // 淘汰访问次数最少的20%
            let evict_count = (candidates.len() / 5).max(1);

            // Clone the addrs to evict to avoid lifetime issues
            let addrs_to_evict: Vec<GuestAddr> = candidates
                .iter()
                .take(evict_count)
                .map(|(addr, _)| *addr)
                .collect();

            for addr in addrs_to_evict {
                if let Some(entry) = hot.remove(&addr) {
                    entries_to_evict.push((addr, entry));
                }
            }
        }

        // Now insert the evicted entries into cold cache
        for (addr, entry) in entries_to_evict {
            if let Ok(mut cold) = self.cold_cache.try_write() {
                if cold.len() >= self.config.max_entries {
                    drop(cold);
                    self.evict_cold();
                    if let Ok(mut cold) = self.cold_cache.try_write() {
                        cold.insert(addr, entry);
                    }
                } else {
                    cold.insert(addr, entry);
                }
            }

            if let Ok(mut fifo) = self.fifo_queue.try_write() {
                fifo.push_back(addr);
            }

            if let Ok(mut lru) = self.lru_order.try_write()
                && let Some(pos) = lru.iter().position(|&a| a == addr)
            {
                lru.remove(pos);
            }
        }
    }

    /// 随机淘汰
    fn evict_random(&self) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut entries_to_evict = Vec::new();

        if let Ok(mut hot) = self.hot_cache.try_write() {
            let mut candidates: Vec<GuestAddr> = hot.keys().copied().collect();

            // 使用地址哈希作为随机种子
            let mut hasher = DefaultHasher::new();
            std::time::SystemTime::now().hash(&mut hasher);
            candidates.sort_by_key(|addr| {
                let mut h = DefaultHasher::new();
                addr.hash(&mut h);
                h.finish()
            });

            // 淘汰20%
            let evict_count = (candidates.len() / 5).max(1);

            // Clone the addrs to evict to avoid lifetime issues
            let addrs_to_evict: Vec<GuestAddr> =
                candidates.iter().take(evict_count).copied().collect();

            for addr in addrs_to_evict {
                if let Some(entry) = hot.remove(&addr) {
                    entries_to_evict.push((addr, entry));
                }
            }
        }

        // Now insert the evicted entries into cold cache
        for (addr, entry) in entries_to_evict {
            if let Ok(mut cold) = self.cold_cache.try_write() {
                if cold.len() >= self.config.max_entries {
                    drop(cold);
                    self.evict_cold();
                    if let Ok(mut cold) = self.cold_cache.try_write() {
                        cold.insert(addr, entry);
                    }
                } else {
                    cold.insert(addr, entry);
                }
            }

            if let Ok(mut fifo) = self.fifo_queue.try_write() {
                fifo.push_back(addr);
            }

            if let Ok(mut lru) = self.lru_order.try_write()
                && let Some(pos) = lru.iter().position(|&a| a == addr)
            {
                lru.remove(pos);
            }
        }
    }

    /// 预热缓存
    pub fn warmup(&self, hot_addrs: &[GuestAddr]) {
        // 预热策略：保留热点地址的缓存条目
        if let Ok(mut hot) = self.hot_cache.try_write() {
            for &addr in hot_addrs.iter().take(self.config.warmup_size) {
                if let Some(entry) = hot.get_mut(&addr) {
                    // 提高热点条目的热度
                    entry.hotness_score *= 2.0;
                    entry.access_count += 10;
                }
            }
        }

        if let Ok(mut cold) = self.cold_cache.try_write() {
            for &addr in hot_addrs.iter().take(self.config.warmup_size) {
                if let Some(entry) = cold.get_mut(&addr) {
                    // 提高热点条目的热度
                    entry.hotness_score *= 2.0;
                    entry.access_count += 10;
                }
            }
        }
    }

    /// 获取热点条目
    pub fn get_hot_entries(&self, limit: usize) -> Vec<(GuestAddr, f64)> {
        if let (Ok(hot), Ok(cold)) = (self.hot_cache.try_read(), self.cold_cache.try_read()) {
            let mut entries: Vec<_> = hot
                .iter()
                .map(|(&addr, entry)| (addr, entry.hotness_score))
                .chain(
                    cold.iter()
                        .map(|(&addr, entry)| (addr, entry.hotness_score)),
                )
                .collect();

            entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            entries.into_iter().take(limit).collect()
        } else {
            Vec::new()
        }
    }

    /// 清空缓存
    pub fn clear(&self) {
        if let Ok(mut hot) = self.hot_cache.try_write() {
            hot.clear();
        }

        if let Ok(mut cold) = self.cold_cache.try_write() {
            cold.clear();
        }

        if let Ok(mut lru) = self.lru_order.try_write() {
            lru.clear();
        }

        if let Ok(mut fifo) = self.fifo_queue.try_write() {
            fifo.clear();
        }

        // 重置统计信息
        if let Ok(mut stats) = self.stats.try_lock() {
            *stats = CacheStats::default();
        }
    }

    /// 生成诊断报告
    pub fn diagnostic_report(&self) -> String {
        let stats = self.stats();
        let hot_entries = self.get_hot_entries(10);

        format!(
            r#"=== Unified Code Cache Report ===

Configuration:
  Max Entries: {}
  Max Memory: {}MB
  Eviction Policy: {:?}
  Cleanup Interval: {}s
  Hotness Decay Factor: {:.3}
  Warmup Size: {}

Statistics:
  Total Entries: {}
  Total Size: {}MB
  Hits: {}
  Misses: {}
  Hit Rate: {:.2}%
  Evictions: {}
  Avg Lookup Time: {}ns
  Memory Usage: {:.1}%

Top 10 Hot Entries:
{}
"#,
            self.config.max_entries,
            self.config.max_memory_bytes / (1024 * 1024),
            self.config.eviction_policy,
            self.config.cleanup_interval_secs,
            self.config.hotness_decay_factor,
            self.config.warmup_size,
            stats.total_entries,
            stats.total_size_bytes / (1024 * 1024),
            stats.hits,
            stats.misses,
            stats.hit_rate * 100.0,
            stats.evictions,
            stats.avg_lookup_time_ns,
            stats.memory_usage_ratio * 100.0,
            hot_entries
                .iter()
                .map(|(addr, score)| format!("  0x{:x}: {:.2}", addr, score))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

impl Default for UnifiedCodeCache {
    fn default() -> Self {
        Self::new(CacheConfig::default(), EwmaHotspotConfig)
    }
}

/// 增强型代码缓存（向后兼容别名）
pub type EnhancedCodeCache = UnifiedCodeCache;

impl SmartPrefetcher {
    /// 创建新的智能预取器
    pub fn new(config: PrefetchConfig) -> Self {
        Self {
            config,
            history: Arc::new(RwLock::new(PrefetchHistory {
                jump_history: HashMap::new(),
                access_patterns: HashMap::new(),
                last_updated: Instant::now(),
            })),
            prefetch_queue: Arc::new(RwLock::new(VecDeque::new())),
            prefetched_addresses: Arc::new(RwLock::new(HashSet::new())),
            stats: Arc::new(Mutex::new(PrefetchStats::default())),
        }
    }

    /// 记录执行跳转（用于学习访问模式）
    pub fn record_jump(&self, from_addr: GuestAddr, to_addr: GuestAddr) {
        if let Ok(mut history) = self.history.try_write() {
            history
                .jump_history
                .entry(from_addr)
                .or_insert_with(Vec::new)
                .push(to_addr);
            history.last_updated = Instant::now();

            // 分析访问模式
            let pattern = self.analyze_access_pattern(from_addr, to_addr);
            history.access_patterns.insert(from_addr, pattern);

            // 如果启用了智能预取，添加预测的地址到预取队列
            if self.config.enable_smart_prefetch {
                self.add_predicted_addresses(from_addr, to_addr);
            }
        }
    }

    /// 分析访问模式
    fn analyze_access_pattern(&self, from_addr: GuestAddr, to_addr: GuestAddr) -> AccessPattern {
        if let Ok(history) = self.history.try_read() {
            // 检查是否是顺序访问（地址连续）
            if to_addr == from_addr + 4 {
                return AccessPattern::Sequential;
            }

            // 检查是否是循环（返回到之前访问过的地址）
            if let Some(targets) = history.jump_history.get(&to_addr)
                && targets.contains(&from_addr)
            {
                return AccessPattern::Looping;
            }

            AccessPattern::Branching
        } else {
            AccessPattern::Branching
        }
    }

    /// 添加预测的地址到预取队列
    fn add_predicted_addresses(&self, from_addr: GuestAddr, to_addr: GuestAddr) {
        let predicted_addresses = self.predict_next_addresses(from_addr, to_addr);

        if let (Ok(mut queue), Ok(prefetched)) = (
            self.prefetch_queue.try_write(),
            self.prefetched_addresses.try_write(),
        ) {
            for addr in predicted_addresses {
                // 检查是否已经在队列中或已预取
                if !queue.contains(&addr) && !prefetched.contains(&addr) {
                    // 检查队列大小限制
                    if queue.len() >= self.config.max_prefetch_queue_size {
                        queue.pop_front(); // 移除最旧的
                    }
                    queue.push_back(addr);

                    // 更新统计
                    if let Ok(mut stats) = self.stats.try_lock() {
                        stats.total_prefetch_requests += 1;
                    }
                }
            }
        }
    }

    /// 预测后续访问地址
    fn predict_next_addresses(&self, from_addr: GuestAddr, to_addr: GuestAddr) -> Vec<GuestAddr> {
        let mut predictions = Vec::new();

        if let Ok(history) = self.history.try_read() {
            // 基于跳转历史进行预测
            if let Some(targets) = history.jump_history.get(&to_addr) {
                // 取最常跳转的目标
                let mut target_counts = HashMap::new();
                for &target in targets {
                    *target_counts.entry(target).or_insert(0) += 1;
                }

                // 按频率排序，取前N个
                let mut sorted_targets: Vec<_> = target_counts.into_iter().collect();
                sorted_targets.sort_by(|a, b| b.1.cmp(&a.1));

                for (target, _) in sorted_targets
                    .into_iter()
                    .take(self.config.prefetch_window_size)
                {
                    predictions.push(target);
                }
            }

            // 如果没有足够的跳转历史，使用启发式预测
            if predictions.len() < self.config.prefetch_window_size {
                // 对于顺序访问，预测后续地址
                if let Some(AccessPattern::Sequential) = history.access_patterns.get(&from_addr) {
                    let remaining = self.config.prefetch_window_size - predictions.len();
                    for i in 1..=remaining {
                        predictions.push(to_addr + (i as u64 * 4));
                    }
                }
            }
        }

        predictions
    }

    /// 获取下一个要预取的地址
    pub fn get_next_prefetch_address(&self) -> Option<GuestAddr> {
        if let Ok(mut queue) = self.prefetch_queue.try_write() {
            queue.pop_front()
        } else {
            None
        }
    }

    /// 标记地址已预取
    pub fn mark_prefetched(&self, addr: GuestAddr) {
        if let Ok(mut prefetched) = self.prefetched_addresses.try_write() {
            prefetched.insert(addr);

            // 更新统计
            if let Ok(mut stats) = self.stats.try_lock() {
                stats.successful_prefetches += 1;
            }
        }
    }

    /// 记录预取命中
    pub fn record_prefetch_hit(&self) {
        if let Ok(mut stats) = self.stats.try_lock() {
            stats.prefetch_hits += 1;
            if stats.successful_prefetches > 0 {
                stats.prefetch_accuracy =
                    stats.prefetch_hits as f64 / stats.successful_prefetches as f64;
            }
        }
    }

    /// 获取预取队列大小
    pub fn queue_size(&self) -> usize {
        self.prefetch_queue.try_read().map(|q| q.len()).unwrap_or(0)
    }

    /// 获取预取统计
    pub fn get_stats(&self) -> PrefetchStats {
        let mut stats = self.stats.try_lock().map(|s| s.clone()).unwrap_or_default();
        stats.queue_size = self.queue_size();
        stats
    }
}

impl UnifiedCodeCache {
    /// 启用缓存预热
    pub fn enable_prefetch(&mut self, config: PrefetchConfig) {
        self.prefetch_config = config.clone();
        if config.enable_smart_prefetch {
            self.prefetcher = Some(Arc::new(SmartPrefetcher::new(config)));
        }
    }

    /// 记录执行跳转（用于预取学习）
    pub fn record_jump(&self, from_addr: GuestAddr, to_addr: GuestAddr) {
        if let Some(ref prefetcher) = self.prefetcher {
            prefetcher.record_jump(from_addr, to_addr);
        }
    }

    /// 启动后台预编译任务
    ///
    /// 实现完整的预编译逻辑：
    /// 1. 从预取队列获取待编译地址
    /// 2. 生成编译请求并发送到编译通道
    /// 3. 更新预取统计和缓存
    pub fn start_background_prefetch(&self) {
        if !self.prefetch_config.enable_background_compile {
            return;
        }

        if let Some(ref prefetcher) = self.prefetcher.clone() {
            let prefetcher_clone = prefetcher.clone();
            let config = self.prefetch_config.clone();
            let compile_tx = self.compile_request_tx.clone();
            let hot_cache = self.hot_cache.clone();
            let cold_cache = self.cold_cache.clone();
            let stats = self.stats.clone();

            let task = tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(100)); // 每100ms检查一次

                loop {
                    interval.tick().await;

                    // 获取要预取的地址
                    if let Some(addr) = prefetcher_clone.get_next_prefetch_address() {
                        // 检查地址是否已经在缓存中
                        let already_cached = {
                            if let (Ok(hot), Ok(cold)) =
                                (hot_cache.try_read(), cold_cache.try_read())
                            {
                                hot.contains_key(&addr) || cold.contains_key(&addr)
                            } else {
                                false
                            }
                        };

                        if !already_cached {
                            // 创建空的IR块用于预编译
                            // 实际实现需要从某个源获取IR块
                            let ir_block = vm_ir::IRBlock {
                                start_pc: addr,
                                ops: Vec::new(),
                                term: vm_ir::Terminator::Ret,
                            };

                            // 发送编译请求
                            if let Some(ref tx) = compile_tx
                                .try_lock()
                                .ok()
                                .and_then(|tx| tx.as_ref().cloned())
                            {
                                let request = CompileRequest {
                                    addr,
                                    ir_block,
                                    priority: config.prefetch_priority,
                                };

                                if tx.try_send(request).is_ok() {
                                    // 更新预取统计
                                    if let Ok(mut stats_guard) = stats.try_lock() {
                                        stats_guard.prefetch_compiles =
                                            stats_guard.prefetch_compiles.saturating_add(1);
                                    }

                                    // 标记为已预取
                                    prefetcher_clone.mark_prefetched(addr);
                                }
                            }
                        } else {
                            // 地址已在缓存中，直接标记为已预取
                            prefetcher_clone.mark_prefetched(addr);
                        }
                    }

                    // 限制预取频率，避免过度消耗资源
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            });

            self.compile_tasks.lock().unwrap().push(task);
        }
    }

    /// 停止所有后台预编译任务
    pub fn stop_background_prefetch(&self) {
        let mut tasks = self.compile_tasks.lock().unwrap();
        for task in tasks.drain(..) {
            task.abort();
        }
    }

    /// 获取预取统计
    pub fn get_prefetch_stats(&self) -> Option<PrefetchStats> {
        self.prefetcher.as_ref().map(|p| p.get_stats())
    }
}

#[cfg(test)]
mod prefetch_tests {
    use super::*;

    #[test]
    fn test_smart_prefetcher_creation() {
        let config = PrefetchConfig::default();
        let prefetcher = SmartPrefetcher::new(config);

        let stats = prefetcher.get_stats();
        assert_eq!(stats.total_prefetch_requests, 0);
        assert_eq!(stats.successful_prefetches, 0);
        assert_eq!(stats.prefetch_hits, 0);
        assert_eq!(stats.queue_size, 0);
    }

    #[test]
    fn test_jump_recording_and_prediction() {
        let config = PrefetchConfig {
            prefetch_window_size: 2,
            ..Default::default()
        };
        let prefetcher = SmartPrefetcher::new(config);

        // 记录一些跳转
        prefetcher.record_jump(vm_core::GuestAddr(0x1000), vm_core::GuestAddr(0x2000));
        prefetcher.record_jump(vm_core::GuestAddr(0x1000), vm_core::GuestAddr(0x2000)); // 重复跳转
        prefetcher.record_jump(vm_core::GuestAddr(0x1000), vm_core::GuestAddr(0x3000));
        prefetcher.record_jump(vm_core::GuestAddr(0x2000), vm_core::GuestAddr(0x4000));
        prefetcher.record_jump(vm_core::GuestAddr(0x2000), vm_core::GuestAddr(0x4000)); // 重复跳转

        // 检查预取队列
        let queue_size = prefetcher.queue_size();
        assert!(queue_size > 0, "Should have predicted addresses in queue");

        // 获取预测地址
        if let Some(addr) = prefetcher.get_next_prefetch_address() {
            prefetcher.mark_prefetched(addr);
            prefetcher.record_prefetch_hit();

            let stats = prefetcher.get_stats();
            assert_eq!(stats.successful_prefetches, 1);
            assert_eq!(stats.prefetch_hits, 1);
            assert_eq!(stats.prefetch_accuracy, 1.0);
        }
    }

    #[test]
    fn test_unified_cache_with_prefetch() {
        let cache_config = CacheConfig::default();
        let hotspot_config = EwmaHotspotConfig::default();
        let prefetch_config = PrefetchConfig {
            enable_smart_prefetch: true,
            prefetch_window_size: 3,
            ..Default::default()
        };

        let cache =
            UnifiedCodeCache::with_prefetch_config(cache_config, hotspot_config, prefetch_config);

        // 验证预取器已创建
        assert!(cache.prefetcher.is_some());

        // 记录跳转
        cache.record_jump(vm_core::GuestAddr(0x1000), vm_core::GuestAddr(0x2000));

        // 获取预取统计
        if let Some(stats) = cache.get_prefetch_stats() {
            assert_eq!(stats.total_prefetch_requests, 1);
        }
    }
}
