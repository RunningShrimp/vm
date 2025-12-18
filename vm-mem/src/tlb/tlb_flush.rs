//! TLB刷新策略优化
//!
//! 实现智能的TLB刷新策略，减少不必要的刷新操作

use crate::GuestAddr;
use crate::tlb::per_cpu_tlb::PerCpuTlbManager;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use vm_core::VmError;

/// 刷新策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushStrategy {
    /// 立即刷新
    Immediate,
    /// 延迟刷新
    Delayed,
    /// 批量刷新
    Batched,
    /// 智能刷新（基于访问模式）
    Intelligent,
    /// 自适应刷新
    Adaptive,
}

/// 刷新范围
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushScope {
    /// 单个页面
    SinglePage,
    /// 页面范围
    PageRange,
    /// ASID
    Asid,
    /// 全局
    Global,
    /// 智能范围（基于访问模式）
    Intelligent,
}

/// 刷新请求
#[derive(Debug, Clone)]
pub struct FlushRequest {
    /// 请求ID
    pub request_id: u64,
    /// 刷新范围
    pub scope: FlushScope,
    /// Guest虚拟地址（对于页面范围）
    pub gva: GuestAddr,
    /// 结束地址（对于页面范围）
    pub end_gva: GuestAddr,
    /// ASID（对于ASID范围）
    pub asid: u16,
    /// 优先级
    pub priority: u8,
    /// 请求时间
    pub timestamp: Instant,
    /// 源CPU ID
    pub source_cpu: usize,
    /// 是否为强制刷新
    pub force: bool,
}

impl FlushRequest {
    /// 创建新的刷新请求
    pub fn new(
        scope: FlushScope,
        gva: GuestAddr,
        end_gva: GuestAddr,
        asid: u16,
        priority: u8,
        source_cpu: usize,
    ) -> Self {
        static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(0);
        
        Self {
            request_id: REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
            scope,
            gva,
            end_gva,
            asid,
            priority,
            timestamp: Instant::now(),
            source_cpu,
            force: false,
        }
    }

    /// 创建强制刷新请求
    pub fn force(
        scope: FlushScope,
        gva: GuestAddr,
        end_gva: GuestAddr,
        asid: u16,
        source_cpu: usize,
    ) -> Self {
        let mut request = Self::new(scope, gva, end_gva, asid, 255, source_cpu);
        request.force = true;
        request
    }

    /// 检查请求是否过期
    pub fn is_expired(&self, timeout: Duration) -> bool {
        self.timestamp.elapsed() > timeout
    }

    /// 检查请求是否影响特定地址
    pub fn affects_address(&self, gva: GuestAddr, asid: u16) -> bool {
        match self.scope {
            FlushScope::SinglePage => {
                let page_size = 4096;
                let req_page_base = self.gva & !(page_size - 1);
                let addr_page_base = gva & !(page_size - 1);
                req_page_base == addr_page_base && self.asid == asid
            }
            FlushScope::PageRange => {
                gva >= self.gva && gva <= self.end_gva && self.asid == asid
            }
            FlushScope::Asid => {
                self.asid == asid
            }
            FlushScope::Global | FlushScope::Intelligent => {
                true
            }
        }
    }
}

/// 访问模式分析器
#[derive(Debug, Clone)]
pub struct AccessPatternAnalyzer {
    /// 访问历史
    access_history: VecDeque<(GuestAddr, u16, Instant)>,
    /// 最大历史记录数
    max_history: usize,
    /// 热点页面
    hot_pages: HashMap<(GuestAddr, u16), u64>,
    /// 访问频率阈值
    frequency_threshold: u64,
}

impl AccessPatternAnalyzer {
    /// 创建新的访问模式分析器
    pub fn new(max_history: usize, frequency_threshold: u64) -> Self {
        Self {
            access_history: VecDeque::with_capacity(max_history),
            max_history,
            hot_pages: HashMap::new(),
            frequency_threshold,
        }
    }

    /// 记录访问
    pub fn record_access(&mut self, gva: GuestAddr, asid: u16) {
        let now = Instant::now();
        let page_base = GuestAddr(gva.0 & !(4096 - 1));
        let key = (page_base, asid);
        
        // 添加到访问历史
        self.access_history.push_back((page_base, asid, now));
        
        // 保持历史记录大小
        if self.access_history.len() > self.max_history {
            self.access_history.pop_front();
        }
        
        // 更新热点页面计数
        *self.hot_pages.entry(key).or_insert(0) += 1;
        
        // 清理过期的热点页面
        self.cleanup_hot_pages();
    }

    /// 获取热点页面
    pub fn get_hot_pages(&self) -> Vec<(GuestAddr, u16, u64)> {
        let mut hot_pages: Vec<_> = self.hot_pages
            .iter()
            .filter(|&(_, &count)| count >= self.frequency_threshold)
            .map(|(&(gva, asid), &count)| (gva, asid, count))
            .collect();
        
        // 按访问频率排序
        hot_pages.sort_by(|a, b| b.2.cmp(&a.2));
        hot_pages
    }

    /// 分析访问模式
    pub fn analyze_pattern(&self) -> AccessPattern {
        if self.access_history.len() < 3 {
            return AccessPattern::Unknown;
        }
        
        let addresses: Vec<_> = self.access_history
            .iter()
            .map(|(gva, _, _)| *gva)
            .collect();
        
        // 检查是否为顺序访问
        let mut sequential_count = 0;
        for i in 1..addresses.len() {
            if addresses[i] == addresses[i-1] + 4096 {
                sequential_count += 1;
            }
        }
        
        if sequential_count as f64 / (addresses.len() - 1) as f64 > 0.8 {
            return AccessPattern::Sequential;
        }
        
        // 检查是否为步长访问
        if addresses.len() >= 3 {
            let stride = addresses[1] - addresses[0];
            let mut stride_count = 0;
            
            for i in 1..addresses.len() {
                if addresses[i] - addresses[i-1] == stride {
                    stride_count += 1;
                }
            }
            
            if stride_count as f64 / (addresses.len() - 1) as f64 > 0.8 {
                return AccessPattern::Strided { stride };
            }
        }
        
        // 检查是否为局部访问
        let hot_pages = self.get_hot_pages();
        if hot_pages.len() < self.access_history.len() / 4 {
            return AccessPattern::Localized;
        }
        
        AccessPattern::Random
    }

    /// 清理过期的热点页面
    fn cleanup_hot_pages(&mut self) {
        if self.access_history.is_empty() {
            return;
        }
        
        let oldest_time = self.access_history.front().unwrap().2;
        let cutoff_time = oldest_time + Duration::from_secs(60); // 1分钟
        
        // 移除1分钟前的访问历史
        self.access_history.retain(|&(_, _, time)| time >= cutoff_time);
        
        // 如果热点页面过多，清理不活跃的热点页面
        if self.hot_pages.len() > 1000 {
            // 只保留访问次数最多的前800个热点页面
            let mut sorted_pages: Vec<_> = self.hot_pages.iter().collect();
            sorted_pages.sort_by(|a, b| b.1.cmp(a.1));
            
            // 创建新的热点页面映射
            let mut new_hot_pages = HashMap::new();
            for (key, &count) in sorted_pages.into_iter().take(800) {
                new_hot_pages.insert(*key, count);
            }
            
            self.hot_pages = new_hot_pages;
        }
    }
}

/// 访问模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessPattern {
    /// 顺序访问
    Sequential,
    /// 步长访问
    Strided { stride: u64 },
    /// 局部访问
    Localized,
    /// 随机访问
    Random,
    /// 未知模式
    Unknown,
}

/// 刷新配置
#[derive(Debug, Clone)]
pub struct TlbFlushConfig {
    /// 刷新策略
    pub strategy: FlushStrategy,
    /// 默认刷新范围
    pub default_scope: FlushScope,
    /// 批量刷新大小
    pub batch_size: usize,
    /// 批量刷新超时（毫秒）
    pub batch_timeout_ms: u64,
    /// 延迟刷新延迟（毫秒）
    pub delay_ms: u64,
    /// 最大刷新队列大小
    pub max_queue_size: usize,
    /// 是否启用访问模式分析
    pub enable_pattern_analysis: bool,
    /// 访问历史大小
    pub access_history_size: usize,
    /// 热点页面阈值
    pub hot_page_threshold: u64,
    /// 智能刷新阈值
    pub intelligent_threshold: f64,
}

impl Default for TlbFlushConfig {
    fn default() -> Self {
        Self {
            strategy: FlushStrategy::Adaptive,
            default_scope: FlushScope::Intelligent,
            batch_size: 16,
            batch_timeout_ms: 10,
            delay_ms: 5,
            max_queue_size: 1024,
            enable_pattern_analysis: true,
            access_history_size: 256,
            hot_page_threshold: 10,
            intelligent_threshold: 0.8,
        }
    }
}

/// 刷新统计信息
#[derive(Debug, Default)]
pub struct TlbFlushStats {
    /// 总刷新请求数
    pub total_requests: AtomicU64,
    /// 立即刷新次数
    pub immediate_flushes: AtomicU64,
    /// 延迟刷新次数
    pub delayed_flushes: AtomicU64,
    /// 批量刷新次数
    pub batched_flushes: AtomicU64,
    /// 智能刷新次数
    pub intelligent_flushes: AtomicU64,
    /// 自适应刷新次数
    pub adaptive_flushes: AtomicU64,
    /// 跳过的刷新次数（智能优化）
    pub skipped_flushes: AtomicU64,
    /// 合并的刷新次数
    pub merged_flushes: AtomicU64,
    /// 平均刷新时间（纳秒）
    pub avg_flush_time_ns: AtomicU64,
    /// 最大刷新时间（纳秒）
    pub max_flush_time_ns: AtomicU64,
    /// 当前队列大小
    pub current_queue_size: AtomicUsize,
    /// 最大队列大小
    pub max_queue_size: AtomicUsize,
}

impl TlbFlushStats {
    /// 获取统计信息快照
    pub fn snapshot(&self) -> TlbFlushStatsSnapshot {
        let total = self.total_requests.load(Ordering::Relaxed);
        TlbFlushStatsSnapshot {
            total_requests: total,
            immediate_flushes: self.immediate_flushes.load(Ordering::Relaxed),
            delayed_flushes: self.delayed_flushes.load(Ordering::Relaxed),
            batched_flushes: self.batched_flushes.load(Ordering::Relaxed),
            intelligent_flushes: self.intelligent_flushes.load(Ordering::Relaxed),
            adaptive_flushes: self.adaptive_flushes.load(Ordering::Relaxed),
            skipped_flushes: self.skipped_flushes.load(Ordering::Relaxed),
            merged_flushes: self.merged_flushes.load(Ordering::Relaxed),
            avg_flush_time_ns: self.avg_flush_time_ns.load(Ordering::Relaxed),
            max_flush_time_ns: self.max_flush_time_ns.load(Ordering::Relaxed),
            current_queue_size: self.current_queue_size.load(Ordering::Relaxed),
            max_queue_size: self.max_queue_size.load(Ordering::Relaxed),
            optimization_rate: if total > 0 {
                (self.skipped_flushes.load(Ordering::Relaxed) + 
                 self.merged_flushes.load(Ordering::Relaxed)) as f64 / total as f64
            } else {
                0.0
            },
        }
    }

    /// 重置统计信息
    pub fn reset(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.immediate_flushes.store(0, Ordering::Relaxed);
        self.delayed_flushes.store(0, Ordering::Relaxed);
        self.batched_flushes.store(0, Ordering::Relaxed);
        self.intelligent_flushes.store(0, Ordering::Relaxed);
        self.adaptive_flushes.store(0, Ordering::Relaxed);
        self.skipped_flushes.store(0, Ordering::Relaxed);
        self.merged_flushes.store(0, Ordering::Relaxed);
        self.avg_flush_time_ns.store(0, Ordering::Relaxed);
        self.max_flush_time_ns.store(0, Ordering::Relaxed);
        self.max_queue_size.store(self.current_queue_size.load(Ordering::Relaxed), Ordering::Relaxed);
    }
}

/// 刷新统计信息快照
#[derive(Debug, Clone)]
pub struct TlbFlushStatsSnapshot {
    pub total_requests: u64,
    pub immediate_flushes: u64,
    pub delayed_flushes: u64,
    pub batched_flushes: u64,
    pub intelligent_flushes: u64,
    pub adaptive_flushes: u64,
    pub skipped_flushes: u64,
    pub merged_flushes: u64,
    pub avg_flush_time_ns: u64,
    pub max_flush_time_ns: u64,
    pub current_queue_size: usize,
    pub max_queue_size: usize,
    pub optimization_rate: f64,
}

/// TLB刷新管理器
pub struct TlbFlushManager {
    /// 配置
    config: TlbFlushConfig,
    /// Per-CPU TLB管理器
    tlb_manager: Arc<PerCpuTlbManager>,
    /// 刷新请求队列
    flush_queue: Arc<Mutex<VecDeque<FlushRequest>>>,
    /// 访问模式分析器
    pattern_analyzer: Arc<Mutex<AccessPatternAnalyzer>>,
    /// 统计信息
    stats: Arc<TlbFlushStats>,
    /// 最后批量刷新时间
    last_batch_flush: Arc<Mutex<Instant>>,
}

impl TlbFlushManager {
    /// 创建新的TLB刷新管理器
    pub fn new(
        config: TlbFlushConfig,
        tlb_manager: Arc<PerCpuTlbManager>,
    ) -> Self {
        let pattern_analyzer = AccessPatternAnalyzer::new(
            config.access_history_size,
            config.hot_page_threshold,
        );
        
        Self {
            config,
            tlb_manager,
            flush_queue: Arc::new(Mutex::new(VecDeque::new())),
            pattern_analyzer: Arc::new(Mutex::new(pattern_analyzer)),
            stats: Arc::new(TlbFlushStats::default()),
            last_batch_flush: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// 使用默认配置创建TLB刷新管理器
    pub fn with_default_config(tlb_manager: Arc<PerCpuTlbManager>) -> Self {
        Self::new(TlbFlushConfig::default(), tlb_manager)
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> TlbFlushStatsSnapshot {
        self.stats.snapshot()
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        self.stats.reset();
    }

    /// 记录TLB访问
    pub fn record_access(&self, gva: GuestAddr, asid: u16) {
        if self.config.enable_pattern_analysis {
            let mut analyzer = self.pattern_analyzer.lock().unwrap();
            analyzer.record_access(gva, asid);
        }
    }

    /// 请求TLB刷新
    pub fn request_flush(&self, request: FlushRequest) -> Result<(), VmError> {
        // 更新统计信息
        self.stats.total_requests.fetch_add(1, Ordering::Relaxed);

        // 根据刷新策略处理请求
        match self.config.strategy {
            FlushStrategy::Immediate => self.flush_immediate(&request),
            FlushStrategy::Delayed => self.flush_delayed(&request),
            FlushStrategy::Batched => self.flush_batched(&request),
            FlushStrategy::Intelligent => self.flush_intelligent(&request),
            FlushStrategy::Adaptive => self.flush_adaptive(&request),
        }
    }

    /// 立即刷新
    fn flush_immediate(&self, request: &FlushRequest) -> Result<(), VmError> {
        let start_time = Instant::now();
        
        match request.scope {
            FlushScope::SinglePage => {
                self.tlb_manager.flush_page(request.gva, request.asid);
            }
            FlushScope::PageRange => {
                // 逐页刷新
                let page_size = 4096;
                let mut current_gva = GuestAddr(request.gva.0 & !(page_size - 1));
                
                while current_gva <= request.end_gva {
                    self.tlb_manager.flush_page(current_gva, request.asid);
                    current_gva = GuestAddr(current_gva.0 + page_size);
                }
            }
            FlushScope::Asid => {
                self.tlb_manager.flush_asid(request.asid);
            }
            FlushScope::Global | FlushScope::Intelligent => {
                self.tlb_manager.flush_all();
            }
        }
        
        // 更新统计信息
        self.stats.immediate_flushes.fetch_add(1, Ordering::Relaxed);
        self.update_flush_time_stats(start_time.elapsed().as_nanos() as u64);
        
        Ok(())
    }

    /// 延迟刷新
    fn flush_delayed(&self, request: &FlushRequest) -> Result<(), VmError> {
        // 添加到队列
        {
            let mut queue = self.flush_queue.lock().unwrap();
            
            // 检查队列大小
            if queue.len() >= self.config.max_queue_size {
                // 队列已满，强制刷新
                let requests = queue.drain(..).collect::<Vec<_>>();
                self.process_batch_flush(&requests)?;
            }
            
            queue.push_back(request.clone());
            
            // 更新队列大小统计
            let current_size = queue.len();
            self.stats.current_queue_size.store(current_size, Ordering::Relaxed);
            
            let max_size = self.stats.max_queue_size.load(Ordering::Relaxed);
            if current_size > max_size {
                self.stats.max_queue_size.store(current_size, Ordering::Relaxed);
            }
        }
        
        // 延迟处理
        std::thread::spawn({
            let flush_queue = self.flush_queue.clone();
            let stats = self.stats.clone();
            let delay_ms = self.config.delay_ms;
            let tlb_manager = self.tlb_manager.clone();
            
            move || {
                std::thread::sleep(Duration::from_millis(delay_ms));
                
                let requests = {
                    let mut queue = flush_queue.lock().unwrap();
                    let requests = queue.drain(..).collect::<Vec<_>>();
                    stats.current_queue_size.store(0, Ordering::Relaxed);
                    requests
                };
                
                if !requests.is_empty() {
                    Self::process_batch_flush_static(&requests, &tlb_manager, &stats);
                }
            }
        });
        
        self.stats.delayed_flushes.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// 批量刷新
    fn flush_batched(&self, request: &FlushRequest) -> Result<(), VmError> {
        // 添加到队列
        {
            let mut queue = self.flush_queue.lock().unwrap();
            
            // 检查队列大小
            if queue.len() >= self.config.max_queue_size {
                // 队列已满，强制刷新
                let requests = queue.drain(..).collect::<Vec<_>>();
                self.process_batch_flush(&requests)?
            }
            
            queue.push_back(request.clone());
            
            // 更新队列大小统计
            let current_size = queue.len();
            self.stats.current_queue_size.store(current_size, Ordering::Relaxed);
            
            let max_size = self.stats.max_queue_size.load(Ordering::Relaxed);
            if current_size > max_size {
                self.stats.max_queue_size.store(current_size, Ordering::Relaxed);
            }
        }
        
        // 检查是否需要立即处理
        let should_process = {
            let queue = self.flush_queue.lock().unwrap();
            queue.len() >= self.config.batch_size || 
            self.should_process_batch_timeout()
        };
        
        if should_process {
            self.process_batch_queue()?;
        }
        
        Ok(())
    }

    /// 智能刷新
    fn flush_intelligent(&self, request: &FlushRequest) -> Result<(), VmError> {
        if !self.config.enable_pattern_analysis {
            return self.flush_immediate(request);
        }
        
        let analyzer = self.pattern_analyzer.lock().unwrap();
        let pattern = analyzer.analyze_pattern();
        let hot_pages = analyzer.get_hot_pages();
        
        // 检查是否可以跳过刷新
        if !request.force && self.can_skip_flush(request, &pattern, &hot_pages) {
            self.stats.skipped_flushes.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        
        // 智能确定刷新范围
        let optimized_scope = self.optimize_flush_scope(request, &pattern, &hot_pages);
        
        // 执行优化的刷新
        let start_time = Instant::now();
        
        match optimized_scope {
            FlushScope::SinglePage => {
                self.tlb_manager.flush_page(request.gva, request.asid);
            }
            FlushScope::PageRange => {
                // 逐页刷新
                let page_size = 4096;
                let mut current_gva = GuestAddr(request.gva.0 & !(page_size - 1));
                
                while current_gva <= request.end_gva {
                    self.tlb_manager.flush_page(current_gva, request.asid);
                    current_gva = GuestAddr(current_gva.0 + page_size);
                }
            }
            FlushScope::Asid => {
                self.tlb_manager.flush_asid(request.asid);
            }
            FlushScope::Global | FlushScope::Intelligent => {
                self.tlb_manager.flush_all();
            }
        }
        
        // 更新统计信息
        self.stats.intelligent_flushes.fetch_add(1, Ordering::Relaxed);
        self.update_flush_time_stats(start_time.elapsed().as_nanos() as u64);
        
        Ok(())
    }

    /// 自适应刷新
    fn flush_adaptive(&self, request: &FlushRequest) -> Result<(), VmError> {
        // 根据系统负载和请求类型选择策略
        let stats = self.stats.snapshot();
        
        // 如果队列很大或请求是强制的，使用立即刷新
        if stats.current_queue_size > self.config.batch_size || request.force {
            return self.flush_immediate(request);
        }
        
        // 如果刷新失败率高，使用批量刷新
        if stats.total_requests > 0 && 
           (stats.skipped_flushes as f64 / stats.total_requests as f64) < 0.1 {
            return self.flush_batched(request);
        }
        
        // 如果启用了模式分析，使用智能刷新
        if self.config.enable_pattern_analysis {
            return self.flush_intelligent(request);
        }
        
        // 默认使用批量刷新
        self.flush_batched(request)
    }

    /// 检查是否可以跳过刷新
    fn can_skip_flush(
        &self,
        request: &FlushRequest,
        pattern: &AccessPattern,
        hot_pages: &[(GuestAddr, u16, u64)],
    ) -> bool {
        // 强制刷新不能跳过
        if request.force {
            return false;
        }
        
        // 检查是否为热点页面
        let page_base = GuestAddr(request.gva.0 & !(4096 - 1));
        let is_hot_page = hot_pages.iter().any(|(gva, asid, _)| {
            *gva == page_base && *asid == request.asid
        });
        
        // 如果是热点页面且为顺序访问，可能可以跳过
        if is_hot_page && matches!(pattern, AccessPattern::Sequential) {
            return true;
        }
        
        // 如果访问模式为局部且刷新范围很大，可能可以跳过
        if matches!(pattern, AccessPattern::Localized) && 
           matches!(request.scope, FlushScope::Global) {
            return true;
        }
        
        false
    }

    /// 优化刷新范围
    fn optimize_flush_scope(
        &self,
        request: &FlushRequest,
        pattern: &AccessPattern,
        hot_pages: &[(GuestAddr, u16, u64)],
    ) -> FlushScope {
        // 如果请求是强制的，保持原范围
        if request.force {
            return request.scope;
        }
        
        // 检查请求的页面是否为热点页面
        let is_requested_page_hot = hot_pages.iter().any(|(gva, asid, _)| {
            *gva == request.gva && *asid == request.asid
        });
        
        // 根据访问模式优化范围
        match pattern {
            AccessPattern::Sequential => {
                // 顺序访问，可以缩小刷新范围
                if matches!(request.scope, FlushScope::Global) {
                    return FlushScope::PageRange;
                }
            }
            AccessPattern::Strided { stride } => {
                // 步长访问，可以预测下一个访问位置
                // 如果步长较小，可能需要刷新更大的范围
                if matches!(request.scope, FlushScope::SinglePage) && *stride <= 4096 * 8 {
                    return FlushScope::PageRange;
                }
            }
            AccessPattern::Localized => {
                // 局部访问，可以避免全局刷新
                if matches!(request.scope, FlushScope::Global) {
                    return FlushScope::Intelligent;
                }
            }
            _ => {}
        }
        
        // 如果请求的页面是热点页面，使用更精确的刷新范围
        if is_requested_page_hot && matches!(request.scope, FlushScope::PageRange) {
            return FlushScope::SinglePage;
        }
        
        request.scope
    }

    /// 检查是否应该处理批量刷新（基于超时）
    fn should_process_batch_timeout(&self) -> bool {
        let last_flush = self.last_batch_flush.lock().unwrap();
        last_flush.elapsed() >= Duration::from_millis(self.config.batch_timeout_ms)
    }

    /// 处理批量刷新队列
    fn process_batch_queue(&self) -> Result<(), VmError> {
        let requests = {
            let mut queue = self.flush_queue.lock().unwrap();
            let requests = queue.drain(..).collect::<Vec<_>>();
            self.stats.current_queue_size.store(0, Ordering::Relaxed);
            
            // 更新最后批量刷新时间
            let mut last_flush = self.last_batch_flush.lock().unwrap();
            *last_flush = Instant::now();
            
            requests
        };
        
        if !requests.is_empty() {
            self.process_batch_flush(&requests)?;
        }
        
        Ok(())
    }

    /// 处理批量刷新
    fn process_batch_flush(&self, requests: &[FlushRequest]) -> Result<(), VmError> {
        let start_time = Instant::now();
        
        // 合并相似的刷新请求
        let merged_requests = self.merge_flush_requests(requests);
        let merged_count = merged_requests.len();
        
        // 按优先级排序
        let mut sorted_requests = merged_requests;
        sorted_requests.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        // 执行刷新
        for request in sorted_requests {
            match request.scope {
                FlushScope::SinglePage => {
                    self.tlb_manager.flush_page(request.gva, request.asid);
                }
                FlushScope::PageRange => {
                    // 逐页刷新
                let page_size = 4096;
                let mut current_gva = GuestAddr(request.gva.0 & !(page_size - 1));
                
                while current_gva <= request.end_gva {
                    self.tlb_manager.flush_page(current_gva, request.asid);
                    current_gva = GuestAddr(current_gva.0 + page_size);
                }
                }
                FlushScope::Asid => {
                    self.tlb_manager.flush_asid(request.asid);
                }
                FlushScope::Global | FlushScope::Intelligent => {
                    self.tlb_manager.flush_all();
                }
            }
        }
        
        // 更新统计信息
        self.stats.batched_flushes.fetch_add(1, Ordering::Relaxed);
        self.stats.merged_flushes.fetch_add(
            (requests.len() - merged_count) as u64,
            Ordering::Relaxed,
        );
        self.update_flush_time_stats(start_time.elapsed().as_nanos() as u64);
        
        Ok(())
    }

    /// 合并相似的刷新请求
    fn merge_flush_requests(&self, requests: &[FlushRequest]) -> Vec<FlushRequest> {
        let mut merged = Vec::new();
        let mut processed = HashSet::new();
        
        for request in requests {
            if processed.contains(&request.request_id) {
                continue;
            }
            
            let mut merged_request = request.clone();
            processed.insert(request.request_id);
            
            // 查找可以合并的请求
            for other in requests {
                if processed.contains(&other.request_id) {
                    continue;
                }
                
                if self.can_merge_requests(&merged_request, other) {
                    // 合并请求
                    merged_request.gva = merged_request.gva.min(other.gva);
                    merged_request.end_gva = merged_request.end_gva.max(other.end_gva);
                    merged_request.priority = merged_request.priority.max(other.priority);
                    processed.insert(other.request_id);
                }
            }
            
            merged.push(merged_request);
        }
        
        merged
    }

    /// 检查两个请求是否可以合并
    fn can_merge_requests(&self, req1: &FlushRequest, req2: &FlushRequest) -> bool {
        // 只有相同ASID和相同类型的请求才能合并
        if req1.asid != req2.asid || req1.scope != req2.scope {
            return false;
        }
        
        match req1.scope {
            FlushScope::PageRange => {
                // 页面范围请求可以合并
                let req1_end = req1.end_gva.max(req1.gva);
                let req2_start = req2.gva.min(req2.end_gva);
                let req2_end = req2.end_gva.max(req2.gva);
                let req1_start = req1.gva.min(req1.end_gva);
                
                // 如果范围重叠或相邻，可以合并
                req2_start <= req1_end + 4096 || req1_start <= req2_end + 4096
            }
            _ => false,
        }
    }

    /// 更新刷新时间统计信息
    fn update_flush_time_stats(&self, flush_time_ns: u64) {
        let total = self.stats.total_requests.load(Ordering::Relaxed);
        let current_avg = self.stats.avg_flush_time_ns.load(Ordering::Relaxed);
        
        // 计算新的平均值
        let new_avg = if total > 1 {
            (current_avg * (total - 1) + flush_time_ns) / total
        } else {
            flush_time_ns
        };
        
        self.stats.avg_flush_time_ns.store(new_avg, Ordering::Relaxed);
        
        // 更新最大值
        let current_max = self.stats.max_flush_time_ns.load(Ordering::Relaxed);
        if flush_time_ns > current_max {
            self.stats.max_flush_time_ns.store(flush_time_ns, Ordering::Relaxed);
        }
    }

    /// 静态方法：处理批量刷新（用于线程中）
    fn process_batch_flush_static(
        requests: &[FlushRequest],
        tlb_manager: &PerCpuTlbManager,
        stats: &TlbFlushStats,
    ) {
        let start_time = Instant::now();
        
        // 执行刷新
        for request in requests {
            match request.scope {
                FlushScope::SinglePage => {
                    tlb_manager.flush_page(request.gva, request.asid);
                }
                FlushScope::PageRange => {
                    // 逐页刷新
                let page_size = 4096;
                let mut current_gva = GuestAddr(request.gva.0 & !(page_size - 1));
                
                while current_gva <= request.end_gva {
                    tlb_manager.flush_page(current_gva, request.asid);
                    current_gva = GuestAddr(current_gva.0 + page_size);
                }
                }
                FlushScope::Asid => {
                    tlb_manager.flush_asid(request.asid);
                }
                FlushScope::Global | FlushScope::Intelligent => {
                    tlb_manager.flush_all();
                }
            }
        }
        
        // 更新统计信息
        stats.batched_flushes.fetch_add(1, Ordering::Relaxed);
        let flush_time = start_time.elapsed().as_nanos() as u64;
        Self::update_flush_time_stats_static(stats, flush_time);
    }

    /// 静态方法：更新刷新时间统计信息
    fn update_flush_time_stats_static(stats: &TlbFlushStats, flush_time_ns: u64) {
        let total = stats.total_requests.load(Ordering::Relaxed);
        let current_avg = stats.avg_flush_time_ns.load(Ordering::Relaxed);
        
        // 计算新的平均值
        let new_avg = if total > 1 {
            (current_avg * (total - 1) + flush_time_ns) / total
        } else {
            flush_time_ns
        };
        
        stats.avg_flush_time_ns.store(new_avg, Ordering::Relaxed);
        
        // 更新最大值
        let current_max = stats.max_flush_time_ns.load(Ordering::Relaxed);
        if flush_time_ns > current_max {
            stats.max_flush_time_ns.store(flush_time_ns, Ordering::Relaxed);
        }
    }

    /// 强制处理所有待刷新请求
    pub fn flush_queue(&self) -> Result<(), VmError> {
        self.process_batch_queue()
    }

    /// 获取当前刷新队列大小
    pub fn get_queue_size(&self) -> usize {
        self.stats.current_queue_size.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tlb::per_cpu_tlb::PerCpuTlbManager;

    #[test]
    fn test_flush_request_creation() {
        let request = FlushRequest::new(
            FlushScope::SinglePage,
            0x1000,
            0x1000,
            0,
            10,
            0,
        );
        
        assert_eq!(request.scope, FlushScope::SinglePage);
        assert_eq!(request.gva, 0x1000);
        assert_eq!(request.asid, 0);
        assert_eq!(request.priority, 10);
        assert_eq!(request.source_cpu, 0);
        assert!(!request.force);
    }

    #[test]
    fn test_flush_request_affects_address() {
        let request = FlushRequest::new(
            FlushScope::SinglePage,
            0x1000,
            0x1000,
            0,
            10,
            0,
        );
        
        // 同一页面
        assert!(request.affects_address(0x1000, 0));
        assert!(request.affects_address(0x1FFF, 0));
        
        // 不同页面
        assert!(!request.affects_address(0x2000, 0));
        
        // 不同ASID
        assert!(!request.affects_address(0x1000, 1));
    }

    #[test]
    fn test_access_pattern_analyzer() {
        let mut analyzer = AccessPatternAnalyzer::new(10, 5);
        
        // 记录顺序访问
        for i in 0..5 {
            analyzer.record_access(0x1000 + i * 4096, 0);
        }
        
        let pattern = analyzer.analyze_pattern();
        assert_eq!(pattern, AccessPattern::Sequential);
        
        let hot_pages = analyzer.get_hot_pages();
        assert_eq!(hot_pages.len(), 5);
    }

    #[test]
    fn test_tlb_flush_manager_creation() {
        let tlb_manager = Arc::new(PerCpuTlbManager::with_default_config());
        let flush_manager = TlbFlushManager::with_default_config(tlb_manager);
        
        let stats = flush_manager.get_stats();
        assert_eq!(stats.total_requests, 0);
    }

    #[test]
    fn test_immediate_flush() {
        let tlb_manager = Arc::new(PerCpuTlbManager::with_default_config());
        let config = TlbFlushConfig {
            strategy: FlushStrategy::Immediate,
            ..Default::default()
        };
        let flush_manager = TlbFlushManager::new(config, tlb_manager.clone());
        
        let request = FlushRequest::new(
            FlushScope::SinglePage,
            0x1000,
            0x1000,
            0,
            10,
            0,
        );
        
        let result = flush_manager.request_flush(request);
        assert!(result.is_ok());
        
        let stats = flush_manager.get_stats();
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.immediate_flushes, 1);
    }
}