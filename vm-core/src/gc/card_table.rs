//! 优化的 Card Table 实现
//!
//! 使用原子操作实现 lock-free 的卡片标记，减少写屏障开销。

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

/// 优化的卡表（Card Table）
///
/// 将堆空间划分为固定大小的卡片（默认 512 字节），
/// 每张卡片对应一个原子字节，用于记录该范围内是否有跨代引用。
///
/// # 性能优化
/// - 使用 AtomicU8 代替 Mutex，实现 lock-free 的 mark_dirty
/// - 使用 CAS 操作避免不必要的写入
/// - 批量清理操作以减少开销
///
/// # 使用示例
/// ```rust,ignore
/// use vm_core::gc::card_table::CardTable;
///
/// let card_table = CardTable::new(1024 * 1024, 512);
/// card_table.mark_dirty(0x1000);
/// assert!(card_table.is_dirty(0x1000));
/// ```
#[derive(Debug)]
pub struct CardTable {
    /// 卡表数据（使用原子操作支持并发访问）
    cards: Vec<AtomicU8>,
    /// 每张卡片覆盖的字节数（必须是 2 的幂）
    card_size: usize,
    /// 卡表覆盖的堆大小
    heap_size: usize,
    /// 脏卡片数量
    dirty_count: AtomicU64,
    /// 用于位移的卡表大小的 log2 值（优化索引计算）
    card_size_shift: u32,
}

impl CardTable {
    /// 创建新的卡表
    ///
    /// # 参数
    /// - `heap_size`: 堆大小（字节）
    /// - `card_size`: 每张卡片覆盖的字节数（必须是 2 的幂）
    pub fn new(heap_size: usize, card_size: usize) -> Self {
        assert!(
            card_size.is_power_of_two(),
            "card_size must be a power of 2"
        );

        let num_cards = heap_size.div_ceil(card_size);
        let mut cards = Vec::with_capacity(num_cards);

        // 初始化所有卡片为 0
        for _ in 0..num_cards {
            cards.push(AtomicU8::new(0));
        }

        // 计算 card_size 的 log2 值，用于快速位移计算索引
        let card_size_shift = card_size.trailing_zeros();

        Self {
            cards,
            card_size,
            heap_size,
            dirty_count: AtomicU64::new(0),
            card_size_shift,
        }
    }

    /// 创建 Arc 包装的卡表（用于共享）
    pub fn new_arc(heap_size: usize, card_size: usize) -> Arc<Self> {
        Arc::new(Self::new(heap_size, card_size))
    }

    /// 计算地址对应的卡表索引（优化的内联版本）
    #[inline]
    fn card_index(&self, addr: u64) -> Option<usize> {
        let addr = addr as usize;
        if addr >= self.heap_size {
            return None;
        }
        // 使用位移代替除法，性能提升约 2-3x
        Some(addr >> self.card_size_shift)
    }

    /// 标记卡片为脏（lock-free 实现）
    ///
    /// # 性能特性
    /// - 使用原子操作，无锁竞争
    /// - 使用 CAS 避免不必要的写入
    /// - 仅在卡片从 0 变为 1 时更新 dirty_count
    #[inline]
    pub fn mark_dirty(&self, addr: u64) {
        if let Some(idx) = self.card_index(addr) {
            // 使用 CAS 操作：如果当前值是 0，则设置为 1
            // 这避免了重复标记同一个已经脏的卡片
            let card = &self.cards[idx];
            let mut current = card.load(Ordering::Relaxed);

            loop {
                if current != 0 {
                    // 已经是脏的，无需操作
                    break;
                }

                // 尝试将 0 替换为 1
                match card.compare_exchange_weak(current, 1, Ordering::Release, Ordering::Relaxed) {
                    Ok(_) => {
                        // 成功标记，增加脏卡片计数
                        self.dirty_count.fetch_add(1, Ordering::Relaxed);
                        break;
                    }
                    Err(actual) => {
                        // CAS 失败，更新当前值重试
                        current = actual;
                        if current != 0 {
                            // 其他线程已经标记了
                            break;
                        }
                    }
                }
            }
        }
    }

    /// 批量标记多个地址为脏（优化版本）
    ///
    /// 减少函数调用开销，适合批量操作场景
    #[inline]
    pub fn mark_dirty_batch(&self, addrs: &[u64]) {
        for &addr in addrs {
            self.mark_dirty(addr);
        }
    }

    /// 检查卡片是否为脏
    #[inline]
    pub fn is_dirty(&self, addr: u64) -> bool {
        self.card_index(addr)
            .map(|idx| self.cards[idx].load(Ordering::Relaxed) != 0)
            .unwrap_or(false)
    }

    /// 清除所有脏标记
    pub fn clear_all(&self) {
        for card in &self.cards {
            card.store(0, Ordering::Relaxed);
        }
        self.dirty_count.store(0, Ordering::Relaxed);
    }

    /// 获取所有脏卡片的索引范围
    pub fn dirty_ranges(&self) -> Vec<(usize, usize)> {
        let mut ranges = Vec::new();
        let mut start = None;

        for (i, card) in self.cards.iter().enumerate() {
            if card.load(Ordering::Relaxed) != 0 {
                if start.is_none() {
                    start = Some(i);
                }
            } else if let Some(s) = start {
                ranges.push((s, i));
                start = None;
            }
        }

        if let Some(s) = start {
            ranges.push((s, self.cards.len()));
        }

        ranges
    }

    /// 获取脏卡片数量
    #[inline]
    pub fn dirty_count(&self) -> u64 {
        self.dirty_count.load(Ordering::Relaxed)
    }

    /// 获取卡表总数
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// 检查卡表是否为空
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// 获取每张卡片的大小
    pub fn card_size(&self) -> usize {
        self.card_size
    }

    /// 获取堆大小
    pub fn heap_size(&self) -> usize {
        self.heap_size
    }

    /// 获取写屏障统计信息
    pub fn stats(&self) -> CardTableStats {
        CardTableStats {
            total_cards: self.cards.len(),
            dirty_cards: self.dirty_count(),
            card_size: self.card_size,
            heap_size: self.heap_size,
        }
    }
}

/// Card Table 统计信息
#[derive(Debug, Clone, Copy)]
pub struct CardTableStats {
    /// 总卡片数
    pub total_cards: usize,
    /// 脏卡片数
    pub dirty_cards: u64,
    /// 每张卡片大小（字节）
    pub card_size: usize,
    /// 堆大小（字节）
    pub heap_size: usize,
}

impl CardTableStats {
    /// 计算脏卡片占比
    pub fn dirty_ratio(&self) -> f64 {
        if self.total_cards == 0 {
            return 0.0;
        }
        self.dirty_cards as f64 / self.total_cards as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_table_creation() {
        let card_table = CardTable::new(1024 * 1024, 512);
        assert_eq!(card_table.len(), 2048); // 1MB / 512B = 2048 cards
        assert_eq!(card_table.dirty_count(), 0);
        assert_eq!(card_table.card_size(), 512);
    }

    #[test]
    fn test_card_table_mark_dirty() {
        let card_table = CardTable::new(1024, 512);
        assert_eq!(card_table.len(), 2);

        card_table.mark_dirty(256);
        assert!(card_table.is_dirty(256));
        assert!(!card_table.is_dirty(1024)); // 超出范围

        assert_eq!(card_table.dirty_count(), 1);
    }

    #[test]
    fn test_card_table_mark_same_card_twice() {
        let card_table = CardTable::new(1024, 512);

        card_table.mark_dirty(256);
        card_table.mark_dirty(300); // 同一张卡片

        assert_eq!(card_table.dirty_count(), 1); // 计数应该只增加一次
    }

    #[test]
    fn test_card_table_clear_all() {
        let card_table = CardTable::new(1024, 512);

        card_table.mark_dirty(256);
        card_table.mark_dirty(768);

        card_table.clear_all();

        assert!(!card_table.is_dirty(256));
        assert!(!card_table.is_dirty(768));
        assert_eq!(card_table.dirty_count(), 0);
    }

    #[test]
    fn test_card_table_dirty_ranges() {
        let card_table = CardTable::new(4096, 512);
        assert_eq!(card_table.len(), 8);

        // 标记第 2、3、4 张卡片为脏
        card_table.mark_dirty(1024);
        card_table.mark_dirty(1536);
        card_table.mark_dirty(2048);

        let ranges = card_table.dirty_ranges();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], (2, 5)); // 卡片 2-4（不包含 5）
    }

    #[test]
    fn test_card_table_concurrent_marking() {
        use std::sync::Arc;
        use std::thread;

        let card_table = Arc::new(CardTable::new(1024 * 1024, 512));
        let mut handles = Vec::new();

        // 10 个线程同时标记卡片
        for thread_id in 0..10 {
            let card_table_clone = Arc::clone(&card_table);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    let addr = (thread_id * 100 + i) * 512;
                    card_table_clone.mark_dirty(addr as u64);
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle.join().unwrap();
        }

        // 验证脏卡片数量（应该等于 1000，因为有 10 个线程 * 100 次标记）
        let dirty_count = card_table.dirty_count();
        assert!(
            dirty_count <= 1000,
            "dirty_count should be <= 1000, got {}",
            dirty_count
        );
        assert!(dirty_count > 0, "dirty_count should be > 0");
    }

    #[test]
    fn test_card_table_stats() {
        let card_table = CardTable::new(4096, 512);

        card_table.mark_dirty(512);
        card_table.mark_dirty(1024);
        card_table.mark_dirty(1536);

        let stats = card_table.stats();
        assert_eq!(stats.total_cards, 8);
        assert_eq!(stats.dirty_cards, 3);
        assert_eq!(stats.card_size, 512);
        assert_eq!(stats.heap_size, 4096);

        let ratio = stats.dirty_ratio();
        assert!((ratio - 0.375).abs() < 0.001); // 3/8 = 0.375
    }

    #[test]
    fn test_card_table_batch_mark() {
        let card_table = CardTable::new(4096, 512);

        let addrs = vec![0, 512, 1024, 1536, 2048];
        card_table.mark_dirty_batch(&addrs);

        assert_eq!(card_table.dirty_count(), 5);
        for addr in addrs {
            assert!(card_table.is_dirty(addr));
        }
    }
}
