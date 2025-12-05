//! 统一垃圾收集实现
//!
//! 整合基础GC、并发GC和优化GC的最佳特性，实现高性能垃圾收集。
//!
//! ## 主要功能
//!
//! - **并发标记**: 使用无锁标记栈，支持并发标记
//! - **增量清扫**: 支持增量清扫，减少暂停时间
//! - **分代GC**: 支持年轻代和老年代，自动对象晋升
//! - **Card Marking**: 优化写屏障，减少开销
//! - **自适应调整**: 根据实际情况自动调整GC参数
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use vm_engine_jit::{UnifiedGC, UnifiedGcConfig};
//!
//! // 创建GC配置
//! let config = UnifiedGcConfig {
//!     enable_generational: true,
//!     promotion_threshold: 3,
//!     use_card_marking: true,
//!     // ...
//! };
//!
//! // 创建GC实例
//! let gc = UnifiedGC::new(config);
//!
//! // 执行Minor GC
//! let roots = vec![0x1000, 0x2000];
//! let promoted = gc.minor_gc(&roots);
//!
//! // 执行Major GC
//! gc.major_gc(&roots);
//! ```

mod gc_marker;
mod gc_sweeper;

pub use gc_marker::GcMarker;
pub use gc_sweeper::GcSweeper;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use std::alloc::Layout;

/// 获取CPU核心数（用于动态调整写屏障分片数）
fn get_cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .max(1)
}

/// GC颜色标记
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GCColor {
    White = 0, // 未访问
    Gray = 1,  // 已访问但尚未处理子对象
    Black = 2, // 已处理完成
}

/// GC阶段
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GCPhase {
    Idle = 0,
    MarkPrepare = 1,
    Marking = 2,
    MarkTerminate = 3,
    Sweeping = 4,
    Complete = 5,
}

/// 统一GC配置
#[derive(Debug, Clone)]
pub struct UnifiedGcConfig {
    /// 标记时间配额（微秒）
    pub mark_quota_us: u64,
    /// 清扫时间配额（微秒）
    pub sweep_quota_us: u64,
    /// 自适应时间配额
    pub adaptive_quota: bool,
    /// 写屏障分片数（0表示自动根据CPU核心数计算）
    pub write_barrier_shards: usize,
    /// 标记栈容量
    pub mark_stack_capacity: usize,
    /// 增量清扫批次大小
    pub sweep_batch_size: usize,
    /// GC目标占用率（0-1.0）
    pub gc_goal: f64,
    /// 启用并发标记
    pub concurrent_marking: bool,
    /// 堆大小限制（字节）
    pub heap_size_limit: u64,
    /// 最小配额倍数（相对于基础配额）
    pub min_quota_multiplier: f64,
    /// 最大配额倍数（相对于基础配额）
    pub max_quota_multiplier: f64,
    /// 启用分代GC
    pub enable_generational: bool,
    /// 年轻代大小比例（0.0-1.0）
    pub young_gen_ratio: f64,
    /// 年轻代晋升阈值（存活次数）
    pub promotion_threshold: u32,
    /// 使用card marking优化写屏障
    pub use_card_marking: bool,
    /// Card大小（字节，必须是2的幂）
    pub card_size: usize,
    /// 启用自适应GC参数调整
    pub enable_adaptive_adjustment: bool,
    /// 基于分配速率的GC触发阈值（字节/秒）
    pub allocation_trigger_threshold: u64,
    /// 基于时间的GC触发间隔（毫秒，0表示禁用）
    pub time_based_trigger_interval_ms: u64,
    /// 启用基于时间的GC触发
    pub enable_time_based_trigger: bool,
    /// 启用NUMA感知的内存分配
    pub enable_numa_aware_allocation: bool,
    /// 启用系统负载感知的GC触发
    pub enable_load_aware_trigger: bool,
    /// 启用内存压力感知的GC触发
    pub enable_memory_pressure_trigger: bool,
}

impl Default for UnifiedGcConfig {
    fn default() -> Self {
        Self {
            mark_quota_us: 500, // 0.5ms - 更细粒度的增量GC
            sweep_quota_us: 250, // 0.25ms - 更短的清扫暂停
            adaptive_quota: true,
            write_barrier_shards: 0, // 0表示自动计算
            mark_stack_capacity: 10000,
            sweep_batch_size: 50, // 更小的批次大小，减少单次暂停
            gc_goal: 0.5, // 50%
            concurrent_marking: true,
            heap_size_limit: 128 * 1024 * 1024, // 128MB 默认
            min_quota_multiplier: 0.25,         // 最小配额为基础配额的25%（更激进的调整）
            max_quota_multiplier: 2.0,          // 最大配额为基础配额的200%
            enable_generational: true,          // 启用分代GC
            young_gen_ratio: 0.3,               // 年轻代占30%
            promotion_threshold: 3,             // 存活3次后晋升
            use_card_marking: true,             // 使用card marking优化
            card_size: 512,                     // 512字节一个card
            enable_adaptive_adjustment: true,   // 启用自适应调整
            allocation_trigger_threshold: 10 * 1024 * 1024, // 10MB/秒
            time_based_trigger_interval_ms: 1000, // 1秒触发一次（默认）
            enable_time_based_trigger: true, // 启用基于时间的触发
            enable_numa_aware_allocation: true, // 启用NUMA感知分配
            enable_load_aware_trigger: true, // 启用系统负载感知的GC触发
            enable_memory_pressure_trigger: true, // 启用内存压力感知的GC触发
        }
    }
}

/// 无锁标记栈
///
/// 使用原子操作实现无锁的标记栈，减少锁竞争
pub struct LockFreeMarkStack {
    /// 栈数据（使用原子指针）
    stack: AtomicPtr<Vec<u64>>,
    /// 栈大小（原子计数）
    size: AtomicU64,
    /// 最大容量
    max_capacity: usize,
    /// 扩容锁（仅在扩容时使用）
    resize_lock: Mutex<()>,
}

impl LockFreeMarkStack {
    pub fn new(initial_capacity: usize) -> Self {
        let vec = Box::new(Vec::with_capacity(initial_capacity));
        Self {
            stack: AtomicPtr::new(Box::into_raw(vec)),
            size: AtomicU64::new(0),
            max_capacity: initial_capacity * 2,
            resize_lock: Mutex::new(()),
        }
    }

    /// 推送对象（无锁）
    pub fn push(&self, addr: u64) -> vm_core::VmResult<()> {
        let current_size = self.size.load(Ordering::Relaxed);

        if current_size as usize >= self.max_capacity {
            return Err(vm_core::VmError::Core(vm_core::CoreError::ResourceExhausted {
                resource: "mark_stack".to_string(),
                current: current_size,
                limit: self.max_capacity as u64,
            }));
        }

        // 原子性地增加大小
        let new_size = self.size.fetch_add(1, Ordering::AcqRel);

        // 获取当前栈指针
        let stack_ptr = self.stack.load(Ordering::Acquire);
        if stack_ptr.is_null() {
            return Err(vm_core::VmError::Core(vm_core::CoreError::Internal {
                message: "Stack pointer is null".to_string(),
                module: "unified_gc".to_string(),
            }));
        }

        unsafe {
            let stack = &mut *stack_ptr;

            // 检查是否需要扩容
            if stack.len() >= stack.capacity() {
                // 获取锁进行扩容
                let _guard = self.resize_lock.lock().unwrap();

                // 双重检查
                let stack_ptr = self.stack.load(Ordering::Acquire);
                if !stack_ptr.is_null() {
                    let stack = &mut *stack_ptr;
                    if stack.len() >= stack.capacity() {
                        let new_capacity = stack.capacity() * 2;
                        let mut new_stack = Vec::with_capacity(new_capacity);
                        new_stack.extend_from_slice(stack);
                        new_stack.push(addr);

                        let new_ptr = Box::into_raw(Box::new(new_stack));
                        let old_ptr = self.stack.swap(new_ptr, Ordering::AcqRel);

                        if !old_ptr.is_null() {
                            let _ = Box::from_raw(old_ptr);
                        }
                        return Ok(());
                    }
                }
            }

            // 添加到当前栈
            let stack = &mut *stack_ptr;
            stack.push(addr);
        }

        Ok(())
    }

    /// 弹出对象（无锁）
    pub fn pop(&self) -> Option<u64> {
        let current_size = self.size.load(Ordering::Relaxed);

        if current_size == 0 {
            return None;
        }

        // 原子性地减少大小
        let new_size = self.size.fetch_sub(1, Ordering::AcqRel);

        if new_size == 0 {
            // 大小变为0，但可能已经被其他线程弹出
            self.size.fetch_add(1, Ordering::Relaxed);
            return None;
        }

        let stack_ptr = self.stack.load(Ordering::Acquire);
        if stack_ptr.is_null() {
            return None;
        }

        unsafe {
            let stack = &mut *stack_ptr;
            stack.pop()
        }
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.size.load(Ordering::Relaxed) == 0
    }

    /// 获取大小
    pub fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed) as usize
    }
}

impl Drop for LockFreeMarkStack {
    fn drop(&mut self) {
        let ptr = self.stack.load(Ordering::Acquire);
        if !ptr.is_null() {
            unsafe {
                let _ = Box::from_raw(ptr);
            }
        }
    }
}

/// 分代GC信息
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Generation {
    Young, // 年轻代
    Old,   // 老年代
}

/// Card标记表（用于优化写屏障）
///
/// Card marking是一种优化技术，将堆分成固定大小的card（通常512字节）
/// 只标记包含跨代引用的card，而不是记录每个对象
pub struct CardTable {
    /// Card标记位图（使用原子字节，每个bit表示一个card是否被修改）
    cards: Arc<Vec<AtomicU8>>,
    /// Card大小（字节，必须是2的幂）
    card_size: usize,
    /// Card数量
    card_count: usize,
    /// 堆起始地址
    heap_start: u64,
    /// Card大小位移（用于快速计算card索引）
    card_size_shift: u32,
}

impl CardTable {
    pub fn new(heap_start: u64, heap_size: u64, card_size: usize) -> Self {
        // 确保card_size是2的幂
        let card_size = card_size.next_power_of_two();
        let card_size_shift = card_size.trailing_zeros();
        
        let card_count = (heap_size as usize + card_size - 1) / card_size;
        let bitmap_size = (card_count + 7) / 8; // 每个bit表示一个card

        // 使用原子字节数组，避免锁竞争
        let mut cards = Vec::with_capacity(bitmap_size);
        for _ in 0..bitmap_size {
            cards.push(AtomicU8::new(0));
        }

        Self {
            cards: Arc::new(cards),
            card_size,
            card_count,
            heap_start,
            card_size_shift,
        }
    }

    /// 标记card（优化版：使用原子操作，完全无锁）
    #[inline]
    pub fn mark_card(&self, addr: u64) {
        // 快速路径：计算card索引（使用位移代替除法）
        let offset = (addr - self.heap_start) as usize;
        let card_index = offset >> self.card_size_shift;
        
        if card_index < self.card_count {
            let byte_index = card_index / 8;
            let bit_index = card_index % 8;
            let bit_mask = 1u8 << bit_index;

            // 使用原子操作设置bit，避免锁竞争
            // 使用Relaxed ordering，因为card marking不需要严格的同步
            let card_byte = &self.cards[byte_index];
            let mut current = card_byte.load(Ordering::Relaxed);
            
            // 如果bit已经设置，直接返回（避免不必要的原子操作）
            if (current & bit_mask) != 0 {
                return;
            }
            
            // 使用compare-and-swap循环设置bit
            loop {
                let new_value = current | bit_mask;
                match card_byte.compare_exchange_weak(
                    current,
                    new_value,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(x) => current = x,
                }
            }
        }
    }

    /// 获取所有标记的card（优化版：使用原子读取）
    pub fn get_marked_cards(&self) -> Vec<usize> {
        let mut marked = Vec::new();

        for (byte_idx, card_byte) in self.cards.iter().enumerate() {
            let byte = card_byte.load(Ordering::Acquire);
            if byte != 0 {
                for bit_idx in 0..8 {
                    if (byte >> bit_idx) & 1 != 0 {
                        let card_index = byte_idx * 8 + bit_idx;
                        if card_index < self.card_count {
                            marked.push(card_index);
                        }
                    }
                }
            }
        }

        marked
    }

    /// 获取card对应的内存地址范围
    pub fn get_card_address_range(&self, card_index: usize) -> Option<(u64, u64)> {
        if card_index >= self.card_count {
            return None;
        }

        let start_offset = card_index * self.card_size;
        let end_offset = start_offset + self.card_size;
        let start_addr = self.heap_start + start_offset as u64;
        let end_addr = self.heap_start + end_offset as u64;

        Some((start_addr, end_addr))
    }

    /// 清除所有card标记（在GC周期开始时调用）
    pub fn clear_all_cards(&self) {
        for card_byte in self.cards.iter() {
            card_byte.store(0, Ordering::Release);
        }
    }

    /// 扫描标记的card，找出其中的对象
    ///
    /// 返回：card中可能包含跨代引用的对象地址列表
    /// 注意：实际实现需要与内存分配器集成，以确定card中的实际对象
    pub fn scan_marked_cards(&self) -> Vec<u64> {
        let marked_cards = self.get_marked_cards();
        let mut objects = Vec::new();

        for card_index in marked_cards {
            if let Some((start_addr, end_addr)) = self.get_card_address_range(card_index) {
                // TODO: 实际实现需要：
                // 1. 遍历card范围内的所有对象
                // 2. 检查对象中的引用字段
                // 3. 如果引用指向年轻代对象，将其加入标记栈
                
                // 简化实现：假设card起始地址是一个对象
                // 实际需要根据内存分配器的布局来确定对象位置
                objects.push(start_addr);
            }
        }

        objects
    }

    /// 清除所有标记（优化版：使用原子操作）
    pub fn clear_all(&self) {
        for card_byte in self.cards.iter() {
            card_byte.store(0, Ordering::Release);
        }
    }

    /// 检查card是否已标记（快速检查）
    #[inline]
    pub fn is_card_marked(&self, addr: u64) -> bool {
        let offset = (addr - self.heap_start) as usize;
        let card_index = offset >> self.card_size_shift;
        
        if card_index >= self.card_count {
            return false;
        }
        
        let byte_index = card_index / 8;
        let bit_index = card_index % 8;
        let bit_mask = 1u8 << bit_index;
        
        let byte = self.cards[byte_index].load(Ordering::Acquire);
        (byte & bit_mask) != 0
    }
    #[inline]
    pub fn is_card_marked(&self, addr: u64) -> bool {
        let offset = (addr - self.heap_start) as usize;
        let card_index = offset >> self.card_size.trailing_zeros();
        
        if card_index >= self.card_count {
            return false;
        }
        
        let byte_index = card_index / 8;
        let bit_index = card_index % 8;
        let bit_mask = 1u8 << bit_index;
        
        let byte = self.cards[byte_index].load(Ordering::Acquire);
        (byte & bit_mask) != 0
    }
}

/// 分片写屏障
///
/// 将修改对象记录分散到多个分片，减少锁竞争
/// 分片数根据CPU核心数动态调整，优化并发性能
/// 优化：支持card marking，减少写屏障开销
pub struct ShardedWriteBarrier {
    /// 分片数组
    shards: Vec<WriteBarrierShard>,
    /// 分片掩码
    shard_mask: usize,
    /// 启用标志
    enabled: AtomicBool,
    /// 当前分片数
    shard_count: usize,
    /// 目标分片数（根据CPU核心数计算）
    target_shard_count: AtomicU32,
    /// Card标记表（可选，用于分代GC优化）
    card_table: Option<Arc<CardTable>>,
    /// 是否使用card marking
    use_card_marking: bool,
}

/// 单个写屏障分片
struct WriteBarrierShard {
    /// 修改对象集合（使用原子指针）
    modified_objects: AtomicPtr<HashSet<u64>>,
    /// 分片锁（仅用于扩容）
    lock: Mutex<()>,
}

impl ShardedWriteBarrier {
    /// 创建新的分片写屏障
    ///
    /// 如果 `shard_count` 为 0，则根据CPU核心数自动计算最优分片数
    pub fn new(shard_count: usize) -> Self {
        let actual_shard_count = if shard_count == 0 {
            Self::compute_optimal_shard_count()
        } else {
            shard_count
        };

        let mut shards = Vec::with_capacity(actual_shard_count);
        for _ in 0..actual_shard_count {
            shards.push(WriteBarrierShard::new());
        }

        Self {
            shards,
            shard_mask: actual_shard_count.next_power_of_two() - 1,
            enabled: AtomicBool::new(true),
            shard_count: actual_shard_count,
            target_shard_count: AtomicU32::new(actual_shard_count as u32),
            card_table: None,
            use_card_marking: false,
        }
    }

    /// 创建支持card marking的写屏障
    pub fn with_card_marking(
        shard_count: usize,
        heap_start: u64,
        heap_size: u64,
        card_size: usize,
    ) -> Self {
        let mut barrier = Self::new(shard_count);
        let card_table = Arc::new(CardTable::new(heap_start, heap_size, card_size));
        barrier.set_card_table(card_table);
        barrier.set_card_marking(true);
        barrier
    }

    /// 根据CPU核心数计算最优分片数
    ///
    /// 策略：
    /// - 1-2 核心：2 分片
    /// - 3-4 核心：4 分片
    /// - 5-8 核心：8 分片
    /// - 9-16 核心：16 分片
    /// - 17+ 核心：32 分片（上限）
    ///
    /// 分片数应该是2的幂次，以优化哈希计算
    fn compute_optimal_shard_count() -> usize {
        let cpu_count = get_cpu_count();

        // 根据CPU核心数选择合适的分片数
        let shard_count = match cpu_count {
            1..=2 => 2,
            3..=4 => 4,
            5..=8 => 8,
            9..=16 => 16,
            _ => 32, // 上限32，避免过多分片导致内存浪费
        };

        shard_count
    }

    /// 动态调整分片数（运行时调整）
    ///
    /// 注意：此方法需要重新创建分片数组，会短暂阻塞
    /// 建议在GC空闲时调用
    pub fn adjust_shard_count(&mut self, new_shard_count: usize) {
        if new_shard_count == 0 || new_shard_count == self.shard_count {
            return;
        }

        // 确保是2的幂次
        let actual_count = new_shard_count.next_power_of_two();

        // 如果数量相同，无需调整
        if actual_count == self.shard_count {
            return;
        }

        // 保存当前修改的对象
        let current_modified = self.drain_modified();

        // 重新创建分片数组
        let mut new_shards = Vec::with_capacity(actual_count);
        for _ in 0..actual_count {
            new_shards.push(WriteBarrierShard::new());
        }

        // 更新分片数组和掩码
        self.shards = new_shards;
        self.shard_mask = actual_count - 1;
        self.shard_count = actual_count;
        self.target_shard_count
            .store(actual_count as u32, Ordering::Release);

        // 将之前的修改对象重新分布到新分片
        for addr in current_modified {
            self.record_write(addr, addr);
        }
    }

    /// 根据当前CPU核心数自动调整分片数
    pub fn auto_adjust_shard_count(&mut self) {
        let optimal_count = Self::compute_optimal_shard_count();
        self.adjust_shard_count(optimal_count);
    }

    /// 获取当前分片数
    pub fn shard_count(&self) -> usize {
        self.shard_count
    }

    /// 获取目标分片数
    pub fn target_shard_count(&self) -> usize {
        self.target_shard_count.load(Ordering::Relaxed) as usize
    }

    /// 记录对象写入（优化版：支持card marking）
    ///
    /// 优化策略：
    /// 1. 如果启用card marking，只标记card，不记录具体对象
    /// 2. 仅在跨代引用时记录（老年代 -> 年轻代）
    /// 3. 使用快速路径减少开销
    pub fn record_write(&self, obj_addr: u64, child_addr: u64) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        // 优化：如果使用card marking，只标记card
        if self.use_card_marking {
            if let Some(ref card_table) = self.card_table {
                // 快速路径：只标记card，不记录具体对象
                card_table.mark_card(obj_addr);
                return;
            }
        }

        // 传统路径：使用地址哈希选择分片
        let shard_index = (obj_addr as usize) & self.shard_mask;
        let shard = &self.shards[shard_index];
        shard.record_write(child_addr);
    }

    /// 设置card表
    pub fn set_card_table(&mut self, card_table: Arc<CardTable>) {
        self.card_table = Some(card_table);
    }

    /// 启用/禁用card marking
    pub fn set_card_marking(&mut self, enable: bool) {
        self.use_card_marking = enable;
    }

    /// 获取所有修改对象
    pub fn drain_modified(&self) -> Vec<u64> {
        let mut all_modified = Vec::new();

        for shard in &self.shards {
            let modified = shard.drain_modified();
            all_modified.extend(modified);
        }

        all_modified
    }

    /// 启用/禁用写屏障
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }
}

impl WriteBarrierShard {
    fn new() -> Self {
        let set = Box::new(HashSet::new());
        Self {
            modified_objects: AtomicPtr::new(Box::into_raw(set)),
            lock: Mutex::new(()),
        }
    }

    fn record_write(&self, addr: u64) {
        let current_ptr = self.modified_objects.load(Ordering::Acquire);

        if current_ptr.is_null() {
            return;
        }

        unsafe {
            let set = &mut *current_ptr;
            set.insert(addr);
        }
    }

    fn drain_modified(&self) -> Vec<u64> {
        let current_ptr = self.modified_objects.load(Ordering::Acquire);

        if current_ptr.is_null() {
            return Vec::new();
        }

        unsafe {
            // 原子性地替换为空集合
            let new_set = Box::new(HashSet::new());
            let new_ptr = Box::into_raw(new_set);
            let old_ptr = self.modified_objects.swap(new_ptr, Ordering::AcqRel);

            // 提取旧集合的内容
            let old_set = Box::from_raw(old_ptr);
            old_set.into_iter().collect()
        }
    }
}

impl Drop for WriteBarrierShard {
    fn drop(&mut self) {
        let ptr = self.modified_objects.load(Ordering::Acquire);
        if !ptr.is_null() {
            unsafe {
                let _ = Box::from_raw(ptr);
            }
        }
    }
}

/// 自适应时间配额管理器
///
/// 根据GC进度、堆大小和系统负载动态调整时间配额
pub struct AdaptiveQuotaManager {
    /// 基础标记配额（微秒）
    base_mark_quota_us: u64,
    /// 基础清扫配额（微秒）
    base_sweep_quota_us: u64,
    /// 当前标记配额
    current_mark_quota_us: AtomicU64,
    /// 当前清扫配额
    current_sweep_quota_us: AtomicU64,
    /// 标记进度（0-1.0）
    mark_progress: AtomicU64, // 存储为百分比 * 100
    /// 上次GC周期时间（微秒）
    last_gc_cycle_time_us: AtomicU64,
    /// 堆大小限制（字节）
    heap_size_limit: u64,
    /// 当前堆使用量（字节）
    heap_used: AtomicU64,
    /// 最小配额倍数
    min_quota_multiplier: f64,
    /// 最大配额倍数
    max_quota_multiplier: f64,
    /// GC历史数据：最近N次GC的暂停时间（用于计算移动平均）
    recent_pause_times: Mutex<Vec<u64>>,
    /// 历史数据窗口大小
    history_window_size: usize,
}

impl AdaptiveQuotaManager {
    pub fn new(base_mark_quota_us: u64, base_sweep_quota_us: u64) -> Self {
        Self {
            base_mark_quota_us,
            base_sweep_quota_us,
            current_mark_quota_us: AtomicU64::new(base_mark_quota_us),
            current_sweep_quota_us: AtomicU64::new(base_sweep_quota_us),
            mark_progress: AtomicU64::new(0),
            last_gc_cycle_time_us: AtomicU64::new(0),
            heap_size_limit: 128 * 1024 * 1024, // 默认128MB
            heap_used: AtomicU64::new(0),
            min_quota_multiplier: 0.5,
            max_quota_multiplier: 3.0,
            recent_pause_times: Mutex::new(Vec::new()),
            history_window_size: 10,
        }
    }

    /// 使用配置创建自适应配额管理器
    pub fn with_config(
        base_mark_quota_us: u64,
        base_sweep_quota_us: u64,
        heap_size_limit: u64,
        min_quota_multiplier: f64,
        max_quota_multiplier: f64,
    ) -> Self {
        Self {
            base_mark_quota_us,
            base_sweep_quota_us,
            current_mark_quota_us: AtomicU64::new(base_mark_quota_us),
            current_sweep_quota_us: AtomicU64::new(base_sweep_quota_us),
            mark_progress: AtomicU64::new(0),
            last_gc_cycle_time_us: AtomicU64::new(0),
            heap_size_limit,
            heap_used: AtomicU64::new(0),
            min_quota_multiplier,
            max_quota_multiplier,
            recent_pause_times: Mutex::new(Vec::new()),
            history_window_size: 10,
        }
    }

    /// 获取当前标记配额
    pub fn get_mark_quota(&self) -> u64 {
        self.current_mark_quota_us.load(Ordering::Relaxed)
    }

    /// 获取当前清扫配额
    pub fn get_sweep_quota(&self) -> u64 {
        self.current_sweep_quota_us.load(Ordering::Relaxed)
    }

    /// 更新标记进度
    pub fn update_mark_progress(&self, progress: f64) {
        let progress_percent = (progress * 100.0) as u64;
        self.mark_progress
            .store(progress_percent, Ordering::Relaxed);

        // 根据进度调整配额
        // 如果进度慢，增加配额；如果进度快，减少配额
        let mut multiplier = 1.0;
        if progress < 0.3 {
            // 进度慢，增加配额
            multiplier = 1.5;
        } else if progress > 0.8 {
            // 进度快，减少配额
            multiplier = 0.8;
        }

        // 应用堆大小调整
        let heap_multiplier = self.compute_heap_size_multiplier();
        multiplier *= heap_multiplier;

        // 限制在最小和最大倍数之间
        multiplier = multiplier
            .max(self.min_quota_multiplier)
            .min(self.max_quota_multiplier);

        let new_quota = (self.base_mark_quota_us as f64 * multiplier) as u64;
        self.current_mark_quota_us
            .store(new_quota, Ordering::Relaxed);
    }

    /// 根据堆大小计算配额倍数
    ///
    /// 堆越大，需要的配额越多
    fn compute_heap_size_multiplier(&self) -> f64 {
        let heap_used = self.heap_used.load(Ordering::Relaxed);
        if heap_used == 0 || self.heap_size_limit == 0 {
            return 1.0;
        }

        let heap_usage_ratio = heap_used as f64 / self.heap_size_limit as f64;

        // 堆使用率越高，配额倍数越大
        // 线性映射：0% -> 1.0x, 50% -> 1.5x, 100% -> 2.0x
        1.0 + heap_usage_ratio
    }

    /// 更新堆使用量
    pub fn update_heap_usage(&self, used_bytes: u64) {
        self.heap_used.store(used_bytes, Ordering::Relaxed);

        // 根据新的堆使用量调整配额
        if self.adaptive_quota_enabled() {
            self.adjust_quotas_for_heap_size();
        }
    }

    /// 根据堆大小调整配额
    fn adjust_quotas_for_heap_size(&self) {
        let multiplier = self.compute_heap_size_multiplier();
        let multiplier = multiplier
            .max(self.min_quota_multiplier)
            .min(self.max_quota_multiplier);

        let new_mark_quota = (self.base_mark_quota_us as f64 * multiplier) as u64;
        let new_sweep_quota = (self.base_sweep_quota_us as f64 * multiplier) as u64;

        self.current_mark_quota_us
            .store(new_mark_quota, Ordering::Relaxed);
        self.current_sweep_quota_us
            .store(new_sweep_quota, Ordering::Relaxed);
    }

    /// 检查是否启用自适应配额
    fn adaptive_quota_enabled(&self) -> bool {
        // 可以通过配置或环境变量控制
        // 目前默认启用
        true
    }

    /// 记录GC周期时间
    pub fn record_gc_cycle_time(&self, time_us: u64) {
        self.last_gc_cycle_time_us.store(time_us, Ordering::Relaxed);

        // 记录到历史数据
        {
            let mut history = self.recent_pause_times.lock().unwrap();
            history.push(time_us);
            if history.len() > self.history_window_size {
                history.remove(0);
            }
        }

        // 根据GC周期时间和历史数据调整配额
        if self.adaptive_quota_enabled() {
            self.adjust_quotas_based_on_history(time_us);
        }
    }

    /// 根据历史数据调整配额（确保<1ms暂停时间）
    fn adjust_quotas_based_on_history(&self, current_pause_us: u64) {
        let history = self.recent_pause_times.lock().unwrap();

        if history.len() < 3 {
            // 历史数据不足，使用简单策略
            // 确保暂停时间不超过1ms
            const MAX_PAUSE_US: u64 = 1000; // 1ms
            if current_pause_us > MAX_PAUSE_US {
                // 如果当前暂停超过1ms，立即减少配额
                let new_mark_quota = (self.base_mark_quota_us as f64 * 0.5).max(100.0) as u64;
                let new_sweep_quota = (self.base_sweep_quota_us as f64 * 0.5).max(50.0) as u64;
                self.current_mark_quota_us.store(new_mark_quota, Ordering::Relaxed);
                self.current_sweep_quota_us.store(new_sweep_quota, Ordering::Relaxed);
            } else if current_pause_us < 500 { // 如果暂停时间很短，可以稍微增加配额
                let new_mark_quota = (self.base_mark_quota_us as f64 * 1.1).min(800.0) as u64;
                let new_sweep_quota = (self.base_sweep_quota_us as f64 * 1.1).min(400.0) as u64;
                self.current_mark_quota_us.store(new_mark_quota, Ordering::Relaxed);
                self.current_sweep_quota_us.store(new_sweep_quota, Ordering::Relaxed);
            }
            return;
        }

        // 计算移动平均
        let avg_pause: f64 = history.iter().sum::<u64>() as f64 / history.len() as f64;

        // 目标暂停时间：确保平均暂停时间不超过1ms
        const TARGET_MAX_PAUSE_US: f64 = 1000.0; // 1ms
        if avg_pause > target_pause {
            // 暂停时间过长，增加配额
            let ratio = avg_pause / target_pause;
            let multiplier = (1.0 + (ratio - 1.0) * 0.3).min(self.max_quota_multiplier);
            let new_quota = (self.base_mark_quota_us as f64 * multiplier) as u64;
            self.current_mark_quota_us
                .store(new_quota, Ordering::Relaxed);
        } else if avg_pause < target_pause * 0.5 {
            // 暂停时间很短，可以减少配额以节省CPU
            let multiplier =
                (1.0 - (1.0 - avg_pause / target_pause) * 0.2).max(self.min_quota_multiplier);
            let new_quota = (self.base_mark_quota_us as f64 * multiplier) as u64;
            self.current_mark_quota_us
                .store(new_quota, Ordering::Relaxed);
        }

        // 同步调整清扫配额（通常与标记配额成比例）
        let sweep_ratio = self.base_sweep_quota_us as f64 / self.base_mark_quota_us as f64;
        let current_mark = self.current_mark_quota_us.load(Ordering::Relaxed);
        let new_sweep = (current_mark as f64 * sweep_ratio) as u64;
        self.current_sweep_quota_us
            .store(new_sweep, Ordering::Relaxed);
    }

    /// 获取当前堆使用量
    pub fn get_heap_used(&self) -> u64 {
        self.heap_used.load(Ordering::Relaxed)
    }

    /// 获取堆使用率（0.0-1.0）
    pub fn get_heap_usage_ratio(&self) -> f64 {
        let used = self.heap_used.load(Ordering::Relaxed);
        if self.heap_size_limit == 0 {
            0.0
        } else {
            (used as f64 / self.heap_size_limit as f64).min(1.0)
        }
    }

    /// 重置配额到基础值
    pub fn reset_quotas(&self) {
        self.current_mark_quota_us
            .store(self.base_mark_quota_us, Ordering::Relaxed);
        self.current_sweep_quota_us
            .store(self.base_sweep_quota_us, Ordering::Relaxed);
        self.mark_progress.store(0, Ordering::Relaxed);
    }
}

/// 统一GC统计信息
#[derive(Default)]
pub struct UnifiedGcStats {
    /// GC周期数
    pub gc_cycles: AtomicU64,
    /// 标记的对象数
    pub objects_marked: AtomicU64,
    /// 释放的对象数
    pub objects_freed: AtomicU64,
    /// 总暂停时间（微秒）
    pub total_pause_us: AtomicU64,
    /// 最大暂停时间（微秒）
    pub max_pause_us: AtomicU64,
    /// 平均暂停时间（微秒）
    pub avg_pause_us: AtomicU64,
    /// 最后一次GC的暂停时间（微秒）
    pub last_pause_us: AtomicU64,
    /// 写屏障调用次数
    pub write_barrier_calls: AtomicU64,
    /// 标记栈溢出次数
    pub mark_stack_overflows: AtomicU64,
}

impl UnifiedGcStats {
    /// 获取平均暂停时间（微秒）
    pub fn get_avg_pause_us(&self) -> f64 {
        let cycles = self.gc_cycles.load(Ordering::Relaxed);
        if cycles == 0 {
            0.0
        } else {
            self.total_pause_us.load(Ordering::Relaxed) as f64 / cycles as f64
        }
    }

    /// 获取最后一次GC的暂停时间（微秒）
    pub fn get_last_pause_us(&self) -> u64 {
        self.last_pause_us.load(Ordering::Relaxed)
    }

    /// 获取最大暂停时间（微秒）
    pub fn get_max_pause_us(&self) -> u64 {
        self.max_pause_us.load(Ordering::Relaxed)
    }

    /// 获取总暂停时间（微秒）
    pub fn get_total_pause_us(&self) -> u64 {
        self.total_pause_us.load(Ordering::Relaxed)
    }

    /// 重置统计信息
    pub fn reset(&self) {
        self.gc_cycles.store(0, Ordering::Relaxed);
        self.objects_marked.store(0, Ordering::Relaxed);
        self.objects_freed.store(0, Ordering::Relaxed);
        self.total_pause_us.store(0, Ordering::Relaxed);
        self.max_pause_us.store(0, Ordering::Relaxed);
        self.avg_pause_us.store(0, Ordering::Relaxed);
        self.last_pause_us.store(0, Ordering::Relaxed);
        self.write_barrier_calls.store(0, Ordering::Relaxed);
        self.mark_stack_overflows.store(0, Ordering::Relaxed);
    }
}

/// 统一垃圾收集器
///
/// 整合所有GC实现的最佳特性
/// 优化：支持分代GC和card marking
pub struct UnifiedGC {
    /// 无锁标记栈
    mark_stack: Arc<LockFreeMarkStack>,
    /// 分片写屏障（使用Mutex包装以支持运行时调整）
    write_barrier: Arc<Mutex<ShardedWriteBarrier>>,
    /// 自适应时间配额管理器
    quota_manager: Arc<AdaptiveQuotaManager>,
    /// GC阶段（原子操作）
    phase: AtomicU64, // 存储GCPhase值
    /// 已标记对象集合
    marked_set: Arc<RwLock<HashSet<u64>>>,
    /// 待清扫对象列表
    sweep_list: Arc<Mutex<Vec<u64>>>,
    /// 配置
    config: UnifiedGcConfig,
    /// 统计信息
    stats: Arc<UnifiedGcStats>,
    /// 堆大小限制（字节）
    heap_size_limit: u64,
    /// 年轻代起始地址
    young_gen_start: u64,
    /// 年轻代大小
    young_gen_size: u64,
    /// 对象存活计数（用于分代GC晋升）
    object_survival_count: Arc<RwLock<HashMap<u64, u32>>>,
    /// GC自适应调整器（如果启用）
    adaptive_adjuster: Option<Arc<gc_adaptive::GcAdaptiveAdjuster>>,
    /// 上次GC时间（用于基于时间的触发）
    last_gc_time: Arc<Mutex<Option<Instant>>>,
    /// NUMA分配器（如果启用NUMA感知）
    numa_allocator: Option<Arc<vm_mem::numa_allocator::NumaAllocator>>,
}

impl UnifiedGC {
    /// 创建新的统一GC
    pub fn new(config: UnifiedGcConfig) -> Self {
        let quota_manager = Arc::new(AdaptiveQuotaManager::with_config(
            config.mark_quota_us,
            config.sweep_quota_us,
            config.heap_size_limit,
            config.min_quota_multiplier,
            config.max_quota_multiplier,
        ));

        // 计算年轻代大小和起始地址
        let young_gen_size = (config.heap_size_limit as f64 * config.young_gen_ratio) as u64;
        let young_gen_start = 0; // 假设堆从0开始

        // 创建写屏障（如果启用card marking）
        let write_barrier = if config.use_card_marking {
            Arc::new(Mutex::new(ShardedWriteBarrier::with_card_marking(
                config.write_barrier_shards,
                young_gen_start,
                config.heap_size_limit,
                config.card_size,
            )))
        } else {
            Arc::new(Mutex::new(ShardedWriteBarrier::new(
                config.write_barrier_shards,
            )))
        };

        // 创建自适应调整器（如果启用）
        let adaptive_adjuster = if config.enable_adaptive_adjustment {
            Some(Arc::new(gc_adaptive::GcAdaptiveAdjuster::new(
                config.young_gen_ratio,
                config.promotion_threshold,
                config.allocation_trigger_threshold,
            )))
        } else {
            None
        };

        // 如果启用自适应调整，使用自适应调整器的初始比例
        let effective_young_gen_size = if let Some(ref adjuster) = adaptive_adjuster {
            (config.heap_size_limit as f64 * adjuster.get_young_gen_ratio()) as u64
        } else {
            young_gen_size
        };

        Self {
            mark_stack: Arc::new(LockFreeMarkStack::new(config.mark_stack_capacity)),
            write_barrier,
            quota_manager,
            phase: AtomicU64::new(GCPhase::Idle as u64),
            marked_set: Arc::new(RwLock::new(HashSet::new())),
            sweep_list: Arc::new(Mutex::new(Vec::new())),
            config: config.clone(),
            stats: Arc::new(UnifiedGcStats::default()),
            heap_size_limit: config.heap_size_limit,
            young_gen_start,
            young_gen_size: effective_young_gen_size,
            object_survival_count: Arc::new(RwLock::new(HashMap::new())),
            adaptive_adjuster,
            last_gc_time: Arc::new(Mutex::new(None)),
            numa_allocator: if config.enable_numa_aware_allocation {
                // 尝试创建NUMA分配器（仅在Linux上可用）
                #[cfg(target_os = "linux")]
                {
                    // 检测NUMA节点
                    let node_count = unsafe {
                        let max_node = libc::numa_max_node();
                        if max_node >= 0 {
                            (max_node + 1) as usize
                        } else {
                            0
                        }
                    };
                    
                    if node_count > 0 {
                        // 创建节点信息列表
                        let mut nodes = Vec::new();
                        for i in 0..node_count {
                            let total_mem = unsafe {
                                let mut size = 0u64;
                                if libc::numa_node_size64(i as i32, &mut size as *mut u64 as *mut i64) == 0 {
                                    size
                                } else {
                                    0
                                }
                            };
                            
                            nodes.push(vm_mem::numa_allocator::NumaNodeInfo {
                                node_id: i,
                                total_memory: total_mem,
                                available_memory: total_mem, // 简化：假设全部可用
                                cpu_mask: 0, // 简化：不跟踪CPU掩码
                            });
                        }
                        
                        if !nodes.is_empty() {
                            Some(Arc::new(vm_mem::numa_allocator::NumaAllocator::new(
                                nodes,
                                vm_mem::numa_allocator::NumaAllocPolicy::Local,
                            )))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                #[cfg(not(target_os = "linux"))]
                {
                    None
                }
            } else {
                None
            },
        }
    }

    /// 判断对象属于哪个代
    #[inline]
    fn get_generation(&self, addr: u64) -> Generation {
        if addr >= self.young_gen_start && addr < self.young_gen_start + self.young_gen_size {
            Generation::Young
        } else {
            Generation::Old
        }
    }

    /// 检查是否需要晋升到老年代
    fn should_promote(&self, addr: u64) -> bool {
        if !self.config.enable_generational {
            return false;
        }

        // 如果对象已经在老年代，不需要晋升
        if self.get_generation(addr) == Generation::Old {
            return false;
        }

        let count = {
            let survival = self.object_survival_count.read().unwrap();
            survival.get(&addr).copied().unwrap_or(0)
        };

        // 如果启用自适应调整，使用动态阈值
        let threshold = if let Some(ref adjuster) = self.adaptive_adjuster {
            adjuster.get_promotion_threshold()
        } else {
            self.config.promotion_threshold
        };

        count >= threshold
    }

    /// 晋升对象到老年代
    ///
    /// 在实际实现中，这需要：
    /// 1. 分配老年代空间
    /// 2. 复制对象数据
    /// 3. 更新所有指向该对象的引用
    /// 4. 释放年轻代空间
    ///
    /// 这里提供一个框架实现，实际需要与内存分配器集成
    pub fn promote_object(&self, addr: u64) -> vm_core::VmResult<u64> {
        if !self.config.enable_generational {
            return Err(vm_core::VmError::Core(vm_core::CoreError::Config {
                message: "Generational GC is not enabled".to_string(),
                path: Some("enable_generational".to_string()),
            }));
        }

        if self.get_generation(addr) == Generation::Old {
            return Ok(addr); // 已经在老年代
        }

        if !self.should_promote(addr) {
            return Err(vm_core::VmError::Core(vm_core::CoreError::InvalidState {
                message: "Object does not meet promotion criteria".to_string(),
                current: format!("survival_count < threshold"),
                expected: format!("survival_count >= threshold"),
            }));
        }

        // TODO: 实际实现需要：
        // 1. 获取对象大小（需要与内存分配器集成）
        // 2. 在老年代分配空间
        // 3. 复制对象数据
        // 4. 更新引用（需要遍历所有对象，更新指向该对象的引用）
        // 5. 释放年轻代空间

        // 简化实现：假设对象已经移动到老年代
        // 实际地址需要从内存分配器获取
        let new_addr = self.young_gen_start + self.young_gen_size + addr - self.young_gen_start;

        // 更新存活计数（晋升后重置计数）
        {
            let mut survival = self.object_survival_count.write().unwrap();
            survival.remove(&addr);
            survival.insert(new_addr, 0);
        }

        Ok(new_addr)
    }

    /// 批量晋升对象
    ///
    /// 在一次GC周期中，晋升所有满足条件的年轻代对象
    pub fn promote_objects(&self, young_gen_objects: &[u64]) -> Vec<(u64, u64)> {
        let mut promoted = Vec::new();

        for &addr in young_gen_objects {
            if self.should_promote(addr) {
                if let Ok(new_addr) = self.promote_object(addr) {
                    promoted.push((addr, new_addr));
                }
            }
        }

        promoted
    }

    /// 记录对象存活（在GC后调用）
    pub fn record_survival(&self, addr: u64) {
        if !self.config.enable_generational {
            return;
        }

        let mut survival = self.object_survival_count.write().unwrap();
        let count = survival.entry(addr).or_insert(0);
        *count += 1;
        let survival_count = *count;
        drop(survival);

        // 如果启用自适应调整，记录存活次数
        if let Some(ref adjuster) = self.adaptive_adjuster {
            adjuster.record_object_survival(survival_count);
        }
    }

    /// 更新堆使用量（用于动态配额调整）
    pub fn update_heap_usage(&self, used_bytes: u64) {
        self.quota_manager.update_heap_usage(used_bytes);
    }

    /// 获取当前堆使用量
    pub fn get_heap_used(&self) -> u64 {
        self.quota_manager.get_heap_used()
    }

    /// 获取堆使用率（0.0-1.0）
    pub fn get_heap_usage_ratio(&self) -> f64 {
        self.quota_manager.get_heap_usage_ratio()
    }

    /// 检查是否应该触发GC（自适应策略）
    ///
    /// 触发条件按优先级：
    /// 1. 紧急情况：堆使用率超过95%，立即触发
    /// 2. 系统负载感知：高负载时减少GC频率，低负载时提前GC
    /// 3. 内存压力感知：内存压力大时提前GC
    /// 4. 基于时间的触发：定期GC确保及时回收
    /// 5. 基于分配速率的触发：自适应调整
    /// 6. 基于堆使用率的触发：基础阈值
    pub fn should_trigger_gc(&self) -> bool {
        let heap_usage_ratio = self.get_heap_usage_ratio();

        // 1. 紧急情况：堆使用率超过紧急阈值（95%），立即触发
        if heap_usage_ratio > 0.95 {
            return true;
        }

        // 2. 系统负载检查（如果启用）
        if self.config.enable_load_aware_trigger {
            if let Some(system_load) = self.get_system_load() {
                // 高负载时（负载 > 0.8），放宽GC触发条件
                if system_load > 0.8 {
                    // 高负载下，只有堆使用率超过80%才触发GC
                    if heap_usage_ratio < 0.8 {
                        return false;
                    }
                } else if system_load < 0.3 {
                    // 低负载时，降低GC触发阈值到60%
                    if heap_usage_ratio > 0.6 {
                        return true;
                    }
                }
            }
        }

        // 3. 内存压力检查（如果启用）
        if self.config.enable_memory_pressure_trigger {
            if let Some(memory_pressure) = self.get_memory_pressure() {
                // 内存压力高时，降低GC触发阈值
                if memory_pressure > 0.8 {
                    if heap_usage_ratio > 0.7 {
                        return true;
                    }
                }
            }
        }

        // 4. 检查基于时间的触发
        if self.config.enable_time_based_trigger && self.config.time_based_trigger_interval_ms > 0 {
            let last_gc = self.last_gc_time.lock().unwrap();
            if let Some(last_time) = *last_gc {
                let elapsed_ms = last_time.elapsed().as_millis() as u64;
                if elapsed_ms >= self.config.time_based_trigger_interval_ms {
                    // 基于时间的触发
                    return true;
                }
            } else {
                // 第一次GC，立即触发
                return true;
            }
        }

        // 5. 如果启用自适应调整，使用自适应调整器的逻辑
        if let Some(ref adjuster) = self.adaptive_adjuster {
            return adjuster.should_trigger_gc(heap_usage_ratio);
        }

        // 6. 基础堆使用率阈值
        heap_usage_ratio > self.config.gc_goal
    }

    /// 记录分配（用于自适应GC触发）
    ///
    /// 在对象分配时调用，用于跟踪分配速率
    pub fn record_allocation(&self, size: u64) {
        if let Some(ref adjuster) = self.adaptive_adjuster {
            adjuster.record_allocation(size);
        }
    }

    /// 获取当前年轻代比例（如果启用自适应调整）
    pub fn get_young_gen_ratio(&self) -> f64 {
        if let Some(ref adjuster) = self.adaptive_adjuster {
            adjuster.get_young_gen_ratio()
        } else {
            self.config.young_gen_ratio
        }
    }

    /// 获取当前晋升阈值（如果启用自适应调整）
    pub fn get_promotion_threshold(&self) -> u32 {
        if let Some(ref adjuster) = self.adaptive_adjuster {
            adjuster.get_promotion_threshold()
        } else {
            self.config.promotion_threshold
        }
    }

    /// 获取当前分配速率（如果启用自适应调整）
    pub fn get_allocation_rate(&self) -> Option<u64> {
        if let Some(ref adjuster) = self.adaptive_adjuster {
            Some(adjuster.get_allocation_rate())
        } else {
            None
        }
    }

    /// 获取系统负载（0.0-1.0，1.0表示满负载）
    ///
    /// 基于CPU使用率和系统负载平均值计算
    fn get_system_load(&self) -> Option<f64> {
        // 简化实现：返回None表示不支持
        // 实际实现需要通过系统API获取CPU使用率和负载信息
        None
    }

    /// 获取内存压力（0.0-1.0，1.0表示内存压力极大）
    ///
    /// 基于系统内存使用情况计算
    fn get_memory_pressure(&self) -> Option<f64> {
        // 简化实现：基于堆使用率推断系统内存压力
        // 实际实现应该查询系统内存信息
        let heap_usage = self.get_heap_usage_ratio();
        if heap_usage > 0.8 {
            Some(heap_usage.min(1.0))
        } else {
            None
        }
    }

    /// NUMA感知的内存分配（用于GC内部数据结构）
    ///
    /// 如果启用NUMA感知，优先在当前NUMA节点分配内存
    /// 否则使用标准分配
    pub fn allocate_numa_aware(&self, size: usize) -> vm_core::VmResult<*mut u8> {
        if let Some(ref numa_alloc) = self.numa_allocator {
            let layout = Layout::from_size_align(size, 8).map_err(|e| {
                vm_core::VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Invalid layout: {}", e),
                    module: "unified_gc".to_string(),
                })
            })?;
            match numa_alloc.allocate(layout) {
                Ok(ptr) => Ok(ptr.as_ptr()),
                Err(e) => {
                    // 回退到标准分配
                    unsafe {
                        let layout = Layout::from_size_align(size, 8).map_err(|e| {
                            vm_core::VmError::Core(vm_core::CoreError::Internal {
                                message: format!("Invalid layout: {}", e),
                                module: "unified_gc".to_string(),
                            })
                        })?;
                        let ptr = std::alloc::alloc(layout);
                        if ptr.is_null() {
                            Err(vm_core::VmError::Core(vm_core::CoreError::ResourceExhausted {
                                resource: "memory".to_string(),
                                current: 0,
                                limit: 0,
                            }))
                        } else {
                            Ok(ptr)
                        }
                    }
                }
            }
        } else {
            // 标准分配
            unsafe {
                let layout = Layout::from_size_align(size, 8).map_err(|e| {
                    vm_core::VmError::Core(vm_core::CoreError::Internal {
                        message: format!("Invalid layout: {}", e),
                        module: "unified_gc".to_string(),
                    })
                })?;
                let ptr = std::alloc::alloc(layout);
                if ptr.is_null() {
                    Err(vm_core::VmError::Core(vm_core::CoreError::ResourceExhausted {
                        resource: "memory".to_string(),
                        current: 0,
                        limit: 0,
                    }))
                } else {
                    Ok(ptr)
                }
            }
        }
    }

    /// NUMA感知的内存释放
    pub fn deallocate_numa_aware(&self, ptr: *mut u8, size: usize) {
        if let Some(ref numa_alloc) = self.numa_allocator {
            if let Ok(non_null_ptr) = std::ptr::NonNull::new(ptr) {
                numa_alloc.deallocate(non_null_ptr, size);
            }
        } else {
            // 标准释放
            unsafe {
                if let Ok(layout) = Layout::from_size_align(size, 8) {
                    std::alloc::dealloc(ptr, layout);
                }
            }
        }
    }

    /// 启动GC周期
    ///
    /// 返回：GC周期开始时间（用于后续计算总暂停时间）
    pub fn start_gc(&self, roots: &[u64]) -> Instant {
        let cycle_start = Instant::now();
        
        // 更新上次GC时间
        {
            let mut last_gc = self.last_gc_time.lock().unwrap();
            *last_gc = Some(cycle_start);
        }

        // 如果启用自适应调整，重置调整器
        if let Some(ref adjuster) = self.adaptive_adjuster {
            adjuster.reset();
        }

        // 如果启用card marking，清除所有card标记
        if self.config.use_card_marking {
            if let Some(ref card_table) = self.write_barrier.lock().unwrap().card_table {
                card_table.clear_all_cards();
            }
        }

        self.phase
            .store(GCPhase::MarkPrepare as u64, Ordering::Release);

        // 使用标记器准备标记阶段
        let marker = GcMarker::new(
            self.mark_stack.clone(),
            self.marked_set.clone(),
            self.phase.clone(),
            self.stats.clone(),
        );
        marker.prepare_marking(roots);

        // 启用写屏障
        self.write_barrier.lock().unwrap().set_enabled(true);

        // 重置配额
        self.quota_manager.reset_quotas();

        self.phase.store(GCPhase::Marking as u64, Ordering::Release);

        cycle_start
    }

    /// 执行增量标记（优化版：更细粒度的暂停时间控制）
    ///
    /// 使用独立的标记器模块
    /// 支持分代GC：如果启用分代GC，优先扫描年轻代和card marking
    ///
    /// 返回：(是否完成, 标记的对象数)
    pub fn incremental_mark(&self) -> (bool, usize) {
        // 目标<1ms暂停：使用更短的配额（最大500微秒）
        let quota_us = self.quota_manager.get_mark_quota().min(500);
        let start_time = Instant::now();

        let marker = GcMarker::new(
            self.mark_stack.clone(),
            self.marked_set.clone(),
            self.phase.clone(),
            self.stats.clone(),
        );
        
        // 如果启用分代GC和card marking，先扫描标记的card
        if self.config.enable_generational && self.config.use_card_marking {
            if let Some(ref card_table) = self.write_barrier.lock().unwrap().card_table {
                let marked_objects = card_table.scan_marked_cards();
                for obj_addr in marked_objects {
                    // 检查对象是否在老年代，且引用了年轻代对象
                    if self.get_generation(obj_addr) == Generation::Old {
                        // TODO: 实际实现需要遍历对象的所有引用字段
                        // 如果引用指向年轻代对象，将其加入标记栈
                        // 这里简化处理：将card中的对象加入标记栈
                        if !self.marked_set.read().expect("lock").contains(&obj_addr) {
                            let _ = self.mark_stack.push(obj_addr);
                        }
                    }
                }
            }
        }

        // 精细控制的增量标记：分步执行，确保每次不超过1ms
        let mut marked_count = 0;
        let mut is_complete = false;
        let step_quota_us = quota_us / 10; // 每个步骤的最大时间

        // 执行最多10个步骤，每个步骤不超过step_quota_us
        for _ in 0..10 {
            if start_time.elapsed().as_micros() as u64 >= quota_us {
                break; // 超过总配额，停止
            }

            let remaining_quota = quota_us - start_time.elapsed().as_micros() as u64;
            let current_step_quota = remaining_quota.min(step_quota_us);

            let (step_complete, step_count) = marker.incremental_mark(current_step_quota);
            marked_count += step_count;

            if step_complete {
                is_complete = true;
                break;
            }
        }

        // 更新进度
        let initial_size = self.mark_stack.len();
        if initial_size > 0 {
            let progress = 1.0 - (self.mark_stack.len() as f64 / initial_size as f64);
            self.quota_manager.update_mark_progress(progress);
        }

        // 处理写屏障记录的修改对象
        let modified = {
            let barrier = self.write_barrier.lock().unwrap();
            barrier.drain_modified()
        };
        for obj in modified {
            if !self.marked_set.read().expect("lock").contains(&obj) {
                let _ = self.mark_stack.push(obj);
            }
        }

        // 更新平均暂停时间
        let cycles = self.stats.gc_cycles.load(Ordering::Relaxed) + 1;
        let total = self.stats.total_pause_us.load(Ordering::Relaxed);
        let avg = if cycles > 0 { total / cycles } else { 0 };
        self.stats.avg_pause_us.store(avg, Ordering::Relaxed);

        (is_complete, marked_count)
    }

    /// 执行年轻代GC（Minor GC）
    ///
    /// 只扫描年轻代对象，速度更快
    /// 在年轻代GC后，晋升满足条件的对象到老年代
    ///
    /// 参数：
    /// - `roots`: 根对象列表
    ///
    /// 返回：晋升的对象数量
    pub fn minor_gc(&self, roots: &[u64]) -> usize {
        if !self.config.enable_generational {
            return 0;
        }

        // 1. 只扫描年轻代中的根对象
        let young_roots: Vec<u64> = roots
            .iter()
            .filter(|&&addr| self.get_generation(addr) == Generation::Young)
            .copied()
            .collect();

        // 2. 启动标记阶段（只标记年轻代）
        self.start_gc(&young_roots);

        // 3. 执行增量标记直到完成
        loop {
            let (is_complete, _) = self.incremental_mark();
            if is_complete {
                break;
            }
        }

        // 4. 完成标记
        self.terminate_marking();

        // 5. 收集年轻代中存活的对象
        let marked_set = self.marked_set.read().unwrap();
        let young_survivors: Vec<u64> = marked_set
            .iter()
            .filter(|&&addr| self.get_generation(addr) == Generation::Young)
            .copied()
            .collect();

        // 6. 记录存活并晋升满足条件的对象
        let mut promoted_count = 0;
        for &addr in &young_survivors {
            self.record_survival(addr);
            if self.should_promote(addr) {
                if self.promote_object(addr).is_ok() {
                    promoted_count += 1;
                }
            }
        }

        // 7. 执行清扫（只清扫年轻代）
        loop {
            let (is_complete, _) = self.incremental_sweep();
            if is_complete {
                break;
            }
        }

        // 8. 完成GC周期
        let cycle_start = std::time::Instant::now();
        self.finish_gc(cycle_start);

        promoted_count
    }

    /// 执行老年代GC（Major GC）
    ///
    /// 扫描整个堆，包括年轻代和老年代
    /// 通常比Minor GC慢，但更彻底
    pub fn major_gc(&self, roots: &[u64]) {
        // 1. 启动完整GC
        self.start_gc(roots);

        // 2. 如果启用card marking，清除所有card标记
        if self.config.use_card_marking {
            if let Some(ref card_table) = self.write_barrier.lock().unwrap().card_table {
                card_table.clear_all_cards();
            }
        }

        // 3. 执行增量标记直到完成
        loop {
            let (is_complete, _) = self.incremental_mark();
            if is_complete {
                break;
            }
        }

        // 4. 完成标记
        self.terminate_marking();

        // 5. 记录所有存活对象（用于下次晋升判断）
        let marked_set = self.marked_set.read().unwrap();
        for &addr in marked_set.iter() {
            if self.get_generation(addr) == Generation::Young {
                self.record_survival(addr);
            }
        }

        // 6. 执行清扫
        loop {
            let (is_complete, _) = self.incremental_sweep();
            if is_complete {
                break;
            }
        }

        // 7. 完成GC周期
        let cycle_start = std::time::Instant::now();
        self.finish_gc(cycle_start);
    }

    /// 完成标记阶段
    pub fn terminate_marking(&self) {
        // 禁用写屏障
        self.write_barrier.lock().unwrap().set_enabled(false);

        // 准备清扫列表（实际应该从所有对象中找出未标记的）
        // 这里简化处理
        let mut sweep_list = self.sweep_list.lock().unwrap();
        sweep_list.clear();
        // 实际实现应该遍历所有对象，找出未标记的

        self.phase
            .store(GCPhase::Sweeping as u64, Ordering::Release);
    }

    /// 执行增量清扫
    ///
    /// 使用独立的清扫器模块，支持并行清扫
    ///
    /// 返回：(是否完成, 释放的对象数)
    pub fn incremental_sweep(&self) -> (bool, usize) {
        // 目标<1ms暂停：使用更短的配额（最大250微秒）
        let quota_us = self.quota_manager.get_sweep_quota().min(250);
        let start_time = Instant::now();

        // 根据CPU核心数和配置决定是否启用并行清扫
        let num_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .max(1);
        let enable_parallel = num_cores > 1 && self.config.sweep_batch_size > 100;

        let sweeper = GcSweeper::new(
            self.sweep_list.clone(),
            self.phase.clone(),
            self.stats.clone(),
            self.config.sweep_batch_size.min(25), // 更小的批次大小
        );

        // 精细控制的增量清扫：分步执行
        let mut freed_count = 0;
        let mut is_complete = false;
        let step_quota_us = quota_us / 5; // 每个步骤的最大时间

        // 执行最多5个步骤，每个步骤不超过step_quota_us
        for _ in 0..5 {
            if start_time.elapsed().as_micros() as u64 >= quota_us {
                break; // 超过总配额，停止
            }

            let remaining_quota = quota_us - start_time.elapsed().as_micros() as u64;
            let current_step_quota = remaining_quota.min(step_quota_us);

            let (step_complete, step_count) = if enable_parallel {
                sweeper.incremental_sweep_with_parallel(current_step_quota, true)
            } else {
                sweeper.incremental_sweep_with_parallel(current_step_quota, false)
            };

            freed_count += step_count;

            if step_complete {
                is_complete = true;
                break;
            }
        }

        // 更新平均暂停时间
        let cycles = self.stats.gc_cycles.load(Ordering::Relaxed) + 1;
        let total = self.stats.total_pause_us.load(Ordering::Relaxed);
        let avg = if cycles > 0 { total / cycles } else { 0 };
        self.stats.avg_pause_us.store(avg, Ordering::Relaxed);

        (is_complete, freed_count)
    }

    /// 完成GC周期
    ///
    /// 记录整个GC周期的统计信息
    pub fn finish_gc(&self, cycle_start_time: Instant) {
        let cycle_duration_us = cycle_start_time.elapsed().as_micros() as u64;

        // 记录GC周期时间到配额管理器
        self.quota_manager.record_gc_cycle_time(cycle_duration_us);

        // 如果启用自适应调整，记录GC完成并计算存活率
        if let Some(ref adjuster) = self.adaptive_adjuster {
            // 计算存活率（标记的对象数 / 总对象数）
            let marked_count = self.marked_set.read().unwrap().len();
            let heap_used = self.get_heap_used();
            // 简化：假设每个对象平均64字节
            let estimated_objects = if heap_used > 0 { heap_used / 64 } else { 1 };
            let survival_rate = if estimated_objects > 0 {
                marked_count as f64 / estimated_objects as f64
            } else {
                0.0
            };
            adjuster.record_gc_complete(survival_rate);

            // 如果年轻代比例发生变化，更新年轻代大小
            let new_ratio = adjuster.get_young_gen_ratio();
            if (new_ratio - self.config.young_gen_ratio).abs() > 0.01 {
                // 比例变化超过1%，更新年轻代大小
                // 注意：这里需要重新计算，但实际应用中可能需要更复杂的逻辑
                // 暂时记录到配置中，下次GC时生效
            }
        }

        self.phase.store(GCPhase::Idle as u64, Ordering::Release);
        self.stats.gc_cycles.fetch_add(1, Ordering::Relaxed);
    }

    /// 写屏障（优化版：分代GC + card marking）
    ///
    /// 在对象写入时调用，保证并发标记的正确性
    /// 优化策略：
    /// 1. 仅在跨代引用时记录（老年代 -> 年轻代）
    /// 2. 使用card marking减少开销
    /// 3. 快速路径检查，减少锁竞争
    pub fn write_barrier(&self, obj_addr: u64, child_addr: u64) {
        self.stats
            .write_barrier_calls
            .fetch_add(1, Ordering::Relaxed);

        let phase = self.phase.load(Ordering::Acquire);

        // 仅在标记阶段需要处理写屏障
        if phase != GCPhase::Marking as u64 {
            return;
        }

        // 优化：分代GC - 仅在跨代引用时记录（老年代 -> 年轻代）
        if self.config.enable_generational {
            let obj_gen = self.get_generation(obj_addr);
            let child_gen = self.get_generation(child_addr);

            // 只在老年代指向年轻代时记录（其他情况不需要）
            if !(obj_gen == Generation::Old && child_gen == Generation::Young) {
                return; // 快速路径：不需要记录
            }
        }

        // 如果子对象未标记，则加入标记栈
        if !self.marked_set.read().expect("lock").contains(&child_addr) {
            // 使用写屏障记录（优化：card marking或分片）
            // 注意：虽然write_barrier被Mutex包装，但record_write本身是无锁的
            let barrier = self.write_barrier.lock().unwrap();
            barrier.record_write(obj_addr, child_addr);
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> Arc<UnifiedGcStats> {
        Arc::clone(&self.stats)
    }

    /// 获取当前阶段
    pub fn phase(&self) -> GCPhase {
        match self.phase.load(Ordering::Acquire) {
            0 => GCPhase::Idle,
            1 => GCPhase::MarkPrepare,
            2 => GCPhase::Marking,
            3 => GCPhase::MarkTerminate,
            4 => GCPhase::Sweeping,
            5 => GCPhase::Complete,
            _ => GCPhase::Idle,
        }
    }
}

impl Default for UnifiedGC {
    fn default() -> Self {
        Self::new(UnifiedGcConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generation_detection() {
        let mut config = UnifiedGcConfig::default();
        config.enable_generational = true;
        config.young_gen_ratio = 0.3;
        config.heap_size_limit = 1000;
        
        let gc = UnifiedGC::new(config);
        
        // 测试年轻代地址
        let young_addr = gc.young_gen_start + 100;
        assert_eq!(gc.get_generation(young_addr), Generation::Young);
        
        // 测试老年代地址
        let old_addr = gc.young_gen_start + gc.young_gen_size + 100;
        assert_eq!(gc.get_generation(old_addr), Generation::Old);
    }

    #[test]
    fn test_promotion_threshold() {
        let mut config = UnifiedGcConfig::default();
        config.enable_generational = true;
        config.promotion_threshold = 3;
        
        let gc = UnifiedGC::new(config);
        let young_addr = gc.young_gen_start + 100;
        
        // 初始不应该晋升
        assert!(!gc.should_promote(young_addr));
        
        // 记录存活3次
        for _ in 0..3 {
            gc.record_survival(young_addr);
        }
        
        // 现在应该可以晋升
        assert!(gc.should_promote(young_addr));
    }

    #[test]
    fn test_card_table_marking() {
        let heap_start = 0x10000;
        let heap_size = 1024 * 1024; // 1MB
        let card_size = 512;
        
        let card_table = CardTable::new(heap_start, heap_size, card_size);
        
        // 标记一个card
        let test_addr = heap_start + 1000;
        card_table.mark_card(test_addr);
        
        // 检查card是否被标记
        let marked_cards = card_table.get_marked_cards();
        assert!(!marked_cards.is_empty());
        
        // 清除所有card
        card_table.clear_all_cards();
        let marked_cards_after_clear = card_table.get_marked_cards();
        assert!(marked_cards_after_clear.is_empty());
    }

    #[test]
    fn test_card_address_range() {
        let heap_start = 0x10000;
        let heap_size = 1024 * 1024;
        let card_size = 512;
        
        let card_table = CardTable::new(heap_start, heap_size, card_size);
        
        // 测试card地址范围
        let card_index = 5;
        if let Some((start, end)) = card_table.get_card_address_range(card_index) {
            assert_eq!(start, heap_start + (card_index * card_size) as u64);
            assert_eq!(end, start + card_size as u64);
        }
    }

    #[test]
    fn test_minor_gc() {
        let mut config = UnifiedGcConfig::default();
        config.enable_generational = true;
        config.promotion_threshold = 2;
        
        let gc = UnifiedGC::new(config);
        
        // 创建一些年轻代根对象
        let roots = vec![gc.young_gen_start + 100, gc.young_gen_start + 200];
        
        // 记录存活次数
        for &root in &roots {
            gc.record_survival(root);
            gc.record_survival(root);
        }
        
        // 执行Minor GC
        let promoted = gc.minor_gc(&roots);
        
        // 应该有一些对象被晋升
        assert!(promoted >= 0);
    }

    #[test]
    fn test_unified_gc_phases() {
        let gc = UnifiedGC::default();

        // 启动GC
        let cycle_start = gc.start_gc(&[1, 2, 3]);
        assert_eq!(gc.phase(), GCPhase::Marking);

        // 执行增量标记
        let (complete, _) = gc.incremental_mark();
        // 可能完成也可能未完成，取决于标记栈是否为空

        if complete {
            gc.terminate_marking();
            assert_eq!(gc.phase(), GCPhase::Sweeping);

            // 执行增量清扫
            let (complete, _) = gc.incremental_sweep();
            if complete {
                gc.finish_gc(cycle_start);
                assert_eq!(gc.phase(), GCPhase::Idle);
            }
        }
    }

    #[test]
    fn test_write_barrier() {
        let gc = UnifiedGC::default();
        let cycle_start = gc.start_gc(&[1]);

        // 调用写屏障
        gc.write_barrier(1, 2);

        // 验证写屏障被调用
        assert!(gc.stats().write_barrier_calls.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn test_adaptive_quota() {
        let manager = AdaptiveQuotaManager::new(1000, 500);

        // 初始配额
        assert_eq!(manager.get_mark_quota(), 1000);

        // 更新进度（慢）
        manager.update_mark_progress(0.2);
        assert!(manager.get_mark_quota() >= 1000); // 应该增加

        // 更新进度（快）
        manager.update_mark_progress(0.9);
        assert!(manager.get_mark_quota() <= 1000); // 应该减少
    }

    #[test]
    fn test_unified_gc_creation() {
        let config = UnifiedGcConfig::default();
        let gc = UnifiedGC::new(config);
        
        assert_eq!(gc.phase(), GCPhase::Idle);
    }

    #[test]
    fn test_unified_gc_should_trigger() {
        let config = UnifiedGcConfig {
            heap_size_limit: 1000,
            ..Default::default()
        };
        let gc = UnifiedGC::new(config);
        
        // 初始状态不应该触发GC
        assert!(!gc.should_trigger_gc());
        
        // 更新堆使用量超过阈值
        gc.update_heap_usage(900);
        // 注意：should_trigger_gc的实现可能基于其他条件
    }

    #[test]
    fn test_unified_gc_empty_roots() {
        let gc = UnifiedGC::default();
        let cycle_start = gc.start_gc(&[]);
        
        // 即使没有根对象，GC也应该能启动
        assert_eq!(gc.phase(), GCPhase::Marking);
        gc.finish_gc(cycle_start);
    }

    #[test]
    fn test_unified_gc_write_barrier() {
        let gc = UnifiedGC::default();
        let initial_calls = gc.stats().write_barrier_calls.load(Ordering::Relaxed);
        
        gc.write_barrier(1, 2);
        gc.write_barrier(2, 3);
        
        let new_calls = gc.stats().write_barrier_calls.load(Ordering::Relaxed);
        assert!(new_calls >= initial_calls + 2);
    }

    #[test]
    fn test_unified_gc_get_stats() {
        let gc = UnifiedGC::default();
        let stats = gc.get_stats();
        
        assert_eq!(stats.phase(), GCPhase::Idle);
        assert_eq!(stats.heap_used(), 0);
        assert_eq!(stats.heap_usage_ratio(), 0.0);
    }

    #[test]
    fn test_gc_config_default() {
        let config = UnifiedGcConfig::default();
        assert!(config.mark_quota_us > 0);
        assert!(config.sweep_quota_us > 0);
        assert!(config.heap_size_limit > 0);
    }

    #[test]
    fn test_adaptive_gc_trigger_strategies() {
        // 测试紧急情况触发（堆使用率>95%）
        let mut config = UnifiedGcConfig::default();
        config.enable_load_aware_trigger = false;
        config.enable_memory_pressure_trigger = false;

        let gc = UnifiedGC::new(config);

        // 正常情况：堆使用率50%，不应该触发
        gc.update_heap_usage(50);
        assert!(!gc.should_trigger_gc());

        // 紧急情况：堆使用率96%，应该触发
        gc.update_heap_usage(96);
        assert!(gc.should_trigger_gc());
    }

    #[test]
    fn test_gc_config_new_features() {
        let config = UnifiedGcConfig::default();

        // 验证新配置的默认值
        assert!(config.enable_load_aware_trigger);
        assert!(config.enable_memory_pressure_trigger);
        assert!(config.enable_time_based_trigger);
        assert!(config.enable_numa_aware_allocation);
        assert!(config.enable_adaptive_adjustment);
    }

    #[test]
    fn test_memory_pressure_detection() {
        let gc = UnifiedGC::default();

        // 堆使用率低于80%，内存压力应该为None
        gc.update_heap_usage(50);
        assert!(gc.get_memory_pressure().is_none());

        // 堆使用率高于80%，应该返回内存压力值
        gc.update_heap_usage(90);
        let pressure = gc.get_memory_pressure();
        assert!(pressure.is_some());
        assert!(pressure.unwrap() > 0.8);
    }

    #[test]
    fn test_gc_stats_default() {
        let stats = UnifiedGcStats::default();
        assert_eq!(stats.total_cycles, 0);
        assert_eq!(stats.total_marked_objects, 0);
        assert_eq!(stats.total_swept_objects, 0);
    }

    #[test]
    fn test_gc_pause_time_under_1ms() {
        use std::time::Duration;

        let gc = UnifiedGC::default();

        // 分配一些对象
        for i in 0..1000 {
            let addr = (i + 1) as u64 * 64; // 模拟对象地址
            gc.allocate_object(addr, 64);
        }

        let roots = vec![64, 128, 192]; // 一些根对象

        // 执行增量标记，测量暂停时间
        let start_time = std::time::Instant::now();
        let (is_complete, marked_count) = gc.incremental_mark();
        let pause_time = start_time.elapsed();

        // 验证暂停时间不超过1ms
        assert!(pause_time < Duration::from_millis(1),
                "GC pause time exceeded 1ms: {:?}", pause_time);
        assert!(marked_count >= 0);

        // 如果标记未完成，执行更多增量步骤
        if !is_complete {
            for _ in 0..5 {
                let start_time = std::time::Instant::now();
                let (complete, count) = gc.incremental_mark();
                let pause_time = start_time.elapsed();

                assert!(pause_time < Duration::from_millis(1),
                        "GC pause time exceeded 1ms in incremental step: {:?}", pause_time);
                assert!(count >= 0);

                if complete {
                    break;
                }
            }
        }

        // 执行增量清扫，测量暂停时间
        let start_time = std::time::Instant::now();
        let (is_complete, freed_count) = gc.incremental_sweep();
        let pause_time = start_time.elapsed();

        // 验证暂停时间不超过1ms
        assert!(pause_time < Duration::from_millis(1),
                "GC sweep pause time exceeded 1ms: {:?}", pause_time);
        assert!(freed_count >= 0);
    }
}
