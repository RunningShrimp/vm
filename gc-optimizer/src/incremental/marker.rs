use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use parking_lot::RwLock;

use super::config::IncrementalGcConfig;

/// 增量式标记器
/// 
/// 使用三色标记算法和写屏障实现增量标记：
/// - 白色：未标记的对象
/// - 灰色：已标记但子对象未扫描
/// - 黑色：已标记且子对象已扫描
pub struct IncrementalMarker {
    /// 标记位图
    mark_bitmap: Vec<AtomicU64>,
    /// 灰色队列（待扫描对象）
    gray_queue: Arc<RwLock<VecDeque<u64>>>,
    /// 已标记对象数
    marked_count: AtomicU64,
    /// 标记位图大小（字）
    bitmap_words: AtomicUsize,
    /// 写缓冲区（记录修改的对象）
    write_buffer: Arc<RwLock<Vec<u64>>>,
    /// 当前扫描位置
    scan_position: AtomicU64,
}

impl IncrementalMarker {
    /// 创建新的增量标记器
    pub fn new(config: &IncrementalGcConfig) -> Self {
        let bitmap_words = config.mark_bitmap_size / 8;
        let mut mark_bitmap = Vec::with_capacity(bitmap_words);
        for _ in 0..bitmap_words {
            mark_bitmap.push(AtomicU64::new(0));
        }
        
        Self {
            mark_bitmap,
            gray_queue: Arc::new(RwLock::new(VecDeque::new())),
            marked_count: AtomicU64::new(0),
            bitmap_words: AtomicUsize::new(bitmap_words),
            write_buffer: Arc::new(RwLock::new(Vec::new())),
            scan_position: AtomicU64::new(0),
        }
    }

    /// 执行增量标记步骤
    /// 
    /// 根据配置的 max_mark_per_step 处理最多 N 个对象。
    /// 返回本次步骤标记的对象数。
    pub fn step(&self, config: &IncrementalGcConfig) -> Result<u64, super::super::GcError> {
        let mut marked_this_step = 0u64;
        let max_marks = config.max_mark_per_step as u64;

        // 首先处理写缓冲区中的对象
        {
            let mut write_buffer = self.write_buffer.write();
            if let Some(&obj_addr) = write_buffer.first() {
                drop(write_buffer);
                self.mark_object(obj_addr);
                self.write_buffer.write().remove(0);
                marked_this_step += 1;
            }
        }

        // 然后从灰色队列中处理对象
        while marked_this_step < max_marks {
            let obj_addr = {
                let mut queue = self.gray_queue.write();
                queue.pop_front()
            };

            if let Some(obj_addr) = obj_addr {
                self.mark_and_scan_children(obj_addr);
                marked_this_step += 1;
            } else {
                break;
            }
        }

        Ok(marked_this_step)
    }

    /// 标记对象
    pub fn mark_object(&self, obj_addr: u64) {
        let word_index = (obj_addr / 64) as usize;
        let bit_index = (obj_addr % 64) as u64;

        let bitmap_words = self.bitmap_words.load(Ordering::Relaxed);
        if word_index >= bitmap_words {
            return;
        }

        let old_value = self.mark_bitmap[word_index].fetch_or(1u64 << bit_index, Ordering::Release);
        let was_marked = (old_value & (1u64 << bit_index)) != 0;

        if !was_marked {
            self.marked_count.fetch_add(1, Ordering::Relaxed);
            self.gray_queue.write().push_back(obj_addr);
        }
    }

    /// 标记对象并扫描其子对象
    fn mark_and_scan_children(&self, _obj_addr: u64) {
        // 简化实现：实际需要访问对象图
        // 这里只是示例，实际实现需要遍历对象的引用字段
    }

    /// 记录写操作（写屏障）
    pub fn record_write(&self, obj_addr: u64) {
        if self.is_marked(obj_addr) {
            // 对象已标记（黑色），需要重新标记为灰色
            self.write_buffer.write().push(obj_addr);
        }
    }

    /// 检查对象是否已标记
    pub fn is_marked(&self, obj_addr: u64) -> bool {
        let word_index = (obj_addr / 64) as usize;
        let bit_index = (obj_addr % 64) as u64;

        let bitmap_words = self.bitmap_words.load(Ordering::Relaxed);
        if word_index >= bitmap_words {
            return false;
        }

        let bitmap_word = self.mark_bitmap[word_index].load(Ordering::Acquire);
        (bitmap_word & (1u64 << bit_index)) != 0
    }

    /// 检查标记是否完成
    pub fn is_complete(&self) -> bool {
        let gray_empty = self.gray_queue.read().is_empty();
        let write_empty = self.write_buffer.read().is_empty();
        gray_empty && write_empty
    }

    /// 获取已标记的对象列表
    pub fn get_marked_objects(&self) -> Vec<u64> {
        let mut marked = Vec::new();
        let count = self.marked_count.load(Ordering::Relaxed);
        
        let bitmap_words = self.bitmap_words.load(Ordering::Relaxed);
        for word_idx in 0..bitmap_words {
            let word = self.mark_bitmap[word_idx].load(Ordering::Acquire);
            for bit_idx in 0..64 {
                if (word & (1u64 << bit_idx)) != 0 {
                    marked.push((word_idx * 64 + bit_idx) as u64);
                }
                if marked.len() as u64 >= count {
                    return marked;
                }
            }
        }
        
        marked
    }

    /// 获取标记进度（0.0 - 1.0）
    pub fn progress(&self) -> f64 {
        if self.gray_queue.read().is_empty() {
            1.0
        } else {
            // 估算进度：基于扫描位置和标记的对象数
            let scanned = self.scan_position.load(Ordering::Relaxed);
            let marked = self.marked_count.load(Ordering::Relaxed);
            if marked == 0 {
                0.0
            } else {
                (scanned as f64) / (marked as f64)
            }
        }
    }

    /// 重置标记器
    pub fn reset(&self) {
        let bitmap_words = self.bitmap_words.load(Ordering::Relaxed);
        for word_idx in 0..bitmap_words {
            self.mark_bitmap[word_idx].store(0, Ordering::Release);
        }
        self.marked_count.store(0, Ordering::Release);
        self.gray_queue.write().clear();
        self.write_buffer.write().clear();
        self.scan_position.store(0, Ordering::Release);
    }

    /// 获取已标记对象数量
    pub fn marked_count(&self) -> u64 {
        self.marked_count.load(Ordering::Relaxed)
    }

    /// 获取灰色队列大小
    pub fn gray_queue_size(&self) -> usize {
        self.gray_queue.read().len()
    }
}
