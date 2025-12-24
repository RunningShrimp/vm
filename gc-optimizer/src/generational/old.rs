use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashSet;
use parking_lot::Mutex;

use crate::GcError;
use super::config::OldGenConfig;

/// 老生代
/// 
/// 使用标记-清除算法回收长寿命对象
pub struct OldGeneration {
    /// 堆空间
    heap: Mutex<Vec<u8>>,
    /// 标记位图
    mark_bitmap: Vec<AtomicU64>,
    /// 活跃对象集合
    live_objects: Mutex<HashSet<u64>>,
    /// 已回收对象数
    collected_count: AtomicU64,
    /// 回收次数
    collection_count: AtomicU64,
    /// 配置
    config: OldGenConfig,
}

impl OldGeneration {
    /// 创建新的老生代
    pub fn new(config: &OldGenConfig) -> Self {
        let bitmap_words = config.old_gen_size / 64;
        let mut mark_bitmap = Vec::with_capacity(bitmap_words);

        for _ in 0..bitmap_words {
            mark_bitmap.push(AtomicU64::new(0));
        }

        Self {
            heap: Mutex::new(vec![0; config.old_gen_size]),
            mark_bitmap,
            live_objects: Mutex::new(HashSet::new()),
            collected_count: AtomicU64::new(0),
            collection_count: AtomicU64::new(0),
            config: config.clone(),
        }
    }

    /// 添加对象（从新生代晋升）
    pub fn add_object(&self, obj_addr: u64, size: usize) -> Result<(), GcError> {
        let mut heap = self.heap.lock();
        let current_size = heap.len();
        let required = obj_addr as usize + size;

        if required > current_size {
            if required > self.config.max_old_gen_size {
                return Err(GcError::OutOfMemory {
                    required,
                    available: current_size,
                });
            }

            // 扩展堆
            let new_size = (current_size * 2).min(self.config.max_old_gen_size);
            heap.resize(new_size, 0);
        }

        Ok(())
    }

    /// 执行 Major GC（标记-清除）
    pub fn collect(&self, young_roots: &[u64], old_roots: &[u64]) -> Result<u64, GcError> {
        let start_time = std::time::Instant::now();

        // 1. 标记阶段
        let mut marked_count = 0u64;

        for root in young_roots.iter().chain(old_roots.iter()) {
            if self.contains(*root) {
                self.mark_object(*root);
                marked_count += 1;
            }
        }

        // 2. 清除阶段
        self.sweep_unmarked();

        // 3. 检查停顿时间
        let duration_us = start_time.elapsed().as_micros() as u64;
        if duration_us > self.config.max_pause_time_us {
            eprintln!("Warning: Major GC pause time exceeded: {}us", duration_us);
        }

        self.collection_count.fetch_add(1, Ordering::Relaxed);

        Ok(marked_count)
    }

    /// 标记对象
    fn mark_object(&self, obj_addr: u64) {
        let word_index = (obj_addr / 64) as usize;
        let bit_index = (obj_addr % 64) as u64;

        if word_index >= self.mark_bitmap.len() {
            return;
        }

        self.mark_bitmap[word_index]
            .fetch_or(1u64 << bit_index, Ordering::Release);
        self.live_objects.lock().insert(obj_addr);
    }

    /// 清除未标记的对象
    fn sweep_unmarked(&self) {
        let mut freed = 0u64;

        for (_addr, is_live) in self.scan_heap() {
            if !is_live {
                freed += 1;
            }
        }

        self.collected_count.fetch_add(freed, Ordering::Relaxed);
    }

    /// 扫描堆检查标记状态
    fn scan_heap(&self) -> Vec<(u64, bool)> {
        let mut result = Vec::new();
        let heap = self.heap.lock();
        let heap_size = heap.len();

        for i in (0..heap_size).step_by(64) {
            let addr = i as u64;
            let is_marked = self.is_marked(addr);
            result.push((addr, is_marked));
        }

        result
    }

    /// 检查对象是否已标记
    fn is_marked(&self, obj_addr: u64) -> bool {
        let word_index = (obj_addr / 64) as usize;
        let bit_index = (obj_addr % 64) as u64;

        if word_index >= self.mark_bitmap.len() {
            return false;
        }

        let bitmap_word = self.mark_bitmap[word_index].load(Ordering::Acquire);
        (bitmap_word & (1u64 << bit_index)) != 0
    }

    /// 检查地址是否在老生代
    pub fn contains(&self, addr: u64) -> bool {
        addr < self.heap.lock().len() as u64
    }

    /// 获取已用字节数
    pub fn used_bytes(&self) -> usize {
        self.live_objects.lock().len() * 64
    }

    /// 获取容量
    pub fn capacity(&self) -> usize {
        self.heap.lock().len()
    }

    /// 获取回收的对象数
    pub fn collected_count(&self) -> u64 {
        self.collected_count.load(Ordering::Relaxed)
    }

    /// 获取回收次数
    pub fn collection_count(&self) -> u64 {
        self.collection_count.load(Ordering::Relaxed)
    }
}
