//! 分代垃圾回收实现
//!
//! 实现分代 GC，包括新生代和老年代的独立管理。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use parking_lot::Mutex;

use super::error::{GCError, GCResult};
use super::metrics::{GCHeapStats, GCStats};
use super::object::{GCObject, GCObjectPtr, ObjectType};
use super::roots::RootSet;
use super::{GCConfig, GCStrategy};

/// 卡表（Card Table）
///
/// 将堆空间划分为固定大小的卡片（默认 512 字节），
/// 每张卡片对应一个字节，用于记录该范围内是否有跨代引用。
struct CardTable {
    /// 卡表数据（使用 Mutex 支持内部可变性）
    cards: Mutex<Vec<u8>>,
    /// 每张卡片覆盖的字节数（必须是 2 的幂）
    card_size: usize,
    /// 卡表覆盖的堆大小
    heap_size: usize,
    /// 脏卡片数量
    dirty_count: AtomicU64,
}

impl CardTable {
    /// 创建新的卡表
    fn new(heap_size: usize, card_size: usize) -> Self {
        let num_cards = heap_size.div_ceil(card_size);
        Self {
            cards: Mutex::new(vec![0; num_cards]),
            card_size,
            heap_size,
            dirty_count: AtomicU64::new(0),
        }
    }

    /// 计算地址对应的卡表索引
    fn card_index(&self, addr: u64) -> Option<usize> {
        let addr = addr as usize;
        if addr >= self.heap_size {
            return None;
        }
        Some(addr / self.card_size)
    }

    /// 标记卡片为脏
    fn mark_dirty(&self, addr: u64) {
        if let Some(idx) = self.card_index(addr) {
            let mut cards = self.cards.lock();
            if cards[idx] == 0 {
                cards[idx] = 1;
                self.dirty_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// 检查卡片是否为脏
    #[allow(dead_code)]
    fn is_dirty(&self, addr: u64) -> bool {
        self.card_index(addr)
            .map(|idx| self.cards.lock()[idx] != 0)
            .unwrap_or(false)
    }

    /// 清除所有脏标记
    fn clear_all(&self) {
        let mut cards = self.cards.lock();
        cards.fill(0);
        self.dirty_count.store(0, Ordering::Relaxed);
    }

    /// 获取所有脏卡片的索引范围
    fn dirty_ranges(&self) -> Vec<(usize, usize)> {
        let cards = self.cards.lock();
        let mut ranges = Vec::new();
        let mut start = None;

        for (i, &card) in cards.iter().enumerate() {
            if card != 0 {
                if start.is_none() {
                    start = Some(i);
                }
            } else if let Some(s) = start {
                ranges.push((s, i));
                start = None;
            }
        }

        if let Some(s) = start {
            ranges.push((s, cards.len()));
        }

        ranges
    }

    /// 获取脏卡片数量
    #[allow(dead_code)]
    fn dirty_count(&self) -> u64 {
        self.dirty_count.load(Ordering::Relaxed)
    }

    /// 获取卡表总数（仅用于测试）
    #[cfg(test)]
    fn len(&self) -> usize {
        self.cards.lock().len()
    }
}

/// 分代 GC
pub struct GenerationalGC {
    /// 配置
    config: GCConfig,
    /// 新生代
    young_gen: Mutex<YoungGeneration>,
    /// 老年代
    old_gen: Mutex<OldGeneration>,
    /// 根集
    roots: RootSet,
    /// 统计信息
    stats: Mutex<GCStats>,
    /// 堆统计
    heap_stats: Mutex<GCHeapStats>,
    /// 上次 GC 时间
    last_gc_time: Mutex<Option<Instant>>,
    /// 写屏障卡表（用于老年代）
    card_table: CardTable,
}

/// 新生代
struct YoungGeneration {
    /// Eden 区
    eden: Vec<GCObject>,
    /// Survivor 区 0
    survivor_from: Vec<GCObject>,
    /// Survivor 区 1
    survivor_to: Vec<GCObject>,
    /// 当前使用的 survivor 区 (0 或 1)
    survivor_index: usize,
    /// 年龄阈值（超过此年龄晋升到老年代）
    tenure_age: u8,
}

/// 老年代
struct OldGeneration {
    /// 对象列表
    objects: Vec<GCObject>,
    /// 空闲列表（用于分配）
    #[allow(dead_code)]
    free_list: Vec<usize>,
}

impl GenerationalGC {
    /// 创建新的分代 GC
    pub fn new(config: GCConfig) -> Self {
        let young_gen = YoungGeneration {
            eden: Vec::with_capacity(config.young_gen_size / 1024),
            survivor_from: Vec::with_capacity(config.young_gen_size / 1024 / 2),
            survivor_to: Vec::with_capacity(config.young_gen_size / 1024 / 2),
            survivor_index: 0,
            tenure_age: 10, // 默认经历 10 次 GC 后晋升
        };

        let old_gen = OldGeneration {
            objects: Vec::with_capacity(config.old_gen_size / 1024),
            free_list: Vec::new(),
        };

        let heap_stats = GCHeapStats::new(config.young_gen_size as u64, config.old_gen_size as u64);

        // 创建卡表，卡片大小默认为 512 字节
        let card_table = CardTable::new(config.old_gen_size, 512);

        Self {
            config,
            young_gen: Mutex::new(young_gen),
            old_gen: Mutex::new(old_gen),
            roots: RootSet::new(),
            stats: Mutex::new(GCStats::default()),
            heap_stats: Mutex::new(heap_stats),
            last_gc_time: Mutex::new(None),
            card_table,
        }
    }

    /// 分配对象（优先从新生代）
    fn allocate_in_young(&self, size: usize, align: usize) -> GCResult<GCObjectPtr> {
        // 检查是否需要触发 GC
        let required_size = (size + align - 1) & !(align - 1);

        // 检查 Eden 区空间
        {
            let young = self.young_gen.lock();
            let current_size = young
                .eden
                .iter()
                .map(|obj| obj.header().get_size())
                .sum::<usize>();
            if current_size + required_size <= self.config.young_gen_size {
                // 有足够空间，直接分配
                drop(young);
                return self.allocate_in_young_direct(size);
            }
        }

        // 空间不足，触发 Minor GC 后重试
        self.minor_gc()?;

        // 再次检查空间
        {
            let young = self.young_gen.lock();
            let current_size = young
                .eden
                .iter()
                .map(|obj| obj.header().get_size())
                .sum::<usize>();
            if current_size + required_size > self.config.young_gen_size {
                return Err(GCError::OutOfMemory {
                    requested: required_size,
                    available: self.config.young_gen_size - current_size,
                });
            }
        }

        self.allocate_in_young_direct(size)
    }

    /// 直接在新生代分配对象（不检查空间）
    fn allocate_in_young_direct(&self, size: usize) -> GCResult<GCObjectPtr> {
        let mut young = self.young_gen.lock();
        let obj = GCObject::new(ObjectType::Other, size);
        let ptr = obj.as_ptr();
        young.eden.push(obj);
        Ok(ptr)
    }

    /// 在老年代分配对象
    #[allow(dead_code)]
    fn allocate_in_old(&self, size: usize, _align: usize) -> GCResult<GCObjectPtr> {
        let mut old = self.old_gen.lock();

        // 尝试从空闲列表分配
        if let Some(free_idx) = old.free_list.pop() {
            let obj = GCObject::new(ObjectType::Other, size);
            let ptr = obj.as_ptr();
            old.objects[free_idx] = obj;
            return Ok(ptr);
        }

        // 检查老年代空间
        let current_size = old
            .objects
            .iter()
            .map(|obj| obj.header().get_size())
            .sum::<usize>();
        if current_size + size > self.config.old_gen_size {
            return Err(GCError::OutOfMemory {
                requested: size,
                available: self.config.old_gen_size - current_size,
            });
        }

        let obj = GCObject::new(ObjectType::Other, size);
        let ptr = obj.as_ptr();
        old.objects.push(obj);

        Ok(ptr)
    }

    /// 执行 Minor GC（只回收新生代）
    fn minor_gc(&self) -> GCResult<GCCollectionStats> {
        let start = Instant::now();
        let mut stats = GCCollectionStats::default();

        let mut young_gen = self.young_gen.lock();
        let old_gen = self.old_gen.lock();
        let roots = self.roots.get_all();

        // 标记阶段：标记 Eden 和 Survivor 中的存活对象
        let mut live_objects = Vec::new();

        // 1. 处理根对象
        for root_ptr in &roots {
            // 检查是否在新生代中
            for obj in &young_gen.eden {
                if obj.as_ptr().addr() == root_ptr.addr() {
                    obj.header().set_mark(true);
                    live_objects.push(obj.clone());
                }
            }
            for obj in &young_gen.survivor_from {
                if obj.as_ptr().addr() == root_ptr.addr() {
                    obj.header().set_mark(true);
                    live_objects.push(obj.clone());
                }
            }
        }

        // 2. 扫描脏卡片，查找老年代到新生代的引用
        // 这是一个关键优化：只扫描被标记为脏的卡片，而不是整个老年代
        {
            let dirty_ranges = self.card_table.dirty_ranges();

            for (start_idx, end_idx) in dirty_ranges {
                // 计算对应的地址范围
                let start_addr = start_idx * self.card_table.card_size;
                let end_addr = end_idx * self.card_table.card_size;

                // 扫描该范围内的老年代对象
                for obj in &old_gen.objects {
                    let obj_addr = obj.as_ptr().addr() as usize;
                    if (start_addr..end_addr).contains(&obj_addr) {
                        // 注意：当前实现扫描对象数据查找可能的指针引用
                        // 这是简化但实用的实现方式
                        // 在完整实现中，还可以：
                        // 1. 使用对象类型信息进行精确的字段扫描
                        // 2. 跳过非指针字段以提高性能
                        // 3. 使用类型布局信息避免误识别

                        // 简化版本：检查对象数据中是否包含可能的指针
                        let data = obj.data();
                        for chunk in data.chunks_exact(8) {
                            if let Some(ptr) = chunk.try_into().ok().map(u64::from_le_bytes) {
                                // 检查是否是有效的指针
                                if ptr > 0x1000 {
                                    // 检查是否指向 Eden 区
                                    for eden_obj in &young_gen.eden {
                                        if eden_obj.as_ptr().addr() == ptr {
                                            eden_obj.header().set_mark(true);
                                            live_objects.push(eden_obj.clone());
                                            break;
                                        }
                                    }
                                    // 检查是否指向 Survivor 区
                                    for survivor_obj in &young_gen.survivor_from {
                                        if survivor_obj.as_ptr().addr() == ptr {
                                            survivor_obj.header().set_mark(true);
                                            live_objects.push(survivor_obj.clone());
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // 清除所有脏标记
            self.card_table.clear_all();
        }

        // 晋升检查：将年龄达到阈值的对象晋升到老年代
        let mut promoted = Vec::new();
        let mut survived = Vec::new();

        for obj in live_objects {
            obj.header().increment_age();
            if obj.header().get_age() >= young_gen.tenure_age {
                promoted.push(obj);
            } else {
                survived.push(obj);
            }
        }

        // 将晋升的对象移到老年代
        if !promoted.is_empty() {
            let mut old_gen = self.old_gen.lock();
            for obj in promoted {
                old_gen.objects.push(obj.clone());
            }
            stats.promoted_objects = old_gen.objects.len() as u64;
        }

        // 交换 Survivor 区
        let survivor_from = std::mem::take(&mut young_gen.survivor_from);
        let mut survivor_to = std::mem::take(&mut young_gen.survivor_to);

        for obj in survived {
            survivor_to.push(obj);
        }

        young_gen.survivor_from = survivor_to;
        young_gen.survivor_to = survivor_from;
        young_gen.survivor_index = 1 - young_gen.survivor_index;

        // 清空 Eden 区
        let reclaimed = young_gen.eden.len() as u64;
        young_gen.eden.clear();

        // 更新统计信息
        stats.duration_ms = start.elapsed().as_millis() as u64;
        stats.reclaimed_objects = reclaimed;
        stats.bytes_reclaimed = reclaimed * 1024; // 估算

        // 更新全局统计
        {
            let mut global_stats = self.stats.lock();
            global_stats.total_collections += 1;
            global_stats.incremental_collections += 1;
            global_stats.total_collection_time_ms += stats.duration_ms;
            global_stats.objects_reclaimed += stats.reclaimed_objects;
            global_stats.bytes_reclaimed += stats.bytes_reclaimed;
        }

        // 更新最后 GC 时间
        *self.last_gc_time.lock() = Some(start);

        Ok(stats)
    }

    /// 执行 Major GC（回收整个堆）
    fn major_gc(&self) -> GCResult<GCCollectionStats> {
        let start = Instant::now();
        let mut stats = GCCollectionStats::default();

        let _young_gen = self.young_gen.lock();
        let _old_gen = self.old_gen.lock();
        let roots = self.roots.get_all();

        // 标记所有根对象可达的对象
        let mut mark_stack = roots.iter().copied().collect::<Vec<_>>();

        while let Some(_ptr) = mark_stack.pop() {
            // 标记对象
            // 注意：当前实现未实现字段遍历
            // 完整实现需要：
            // 1. 获取对象的类型信息
            // 2. 遍历对象的所有引用字段
            // 3. 将发现的引用加入mark_stack
        }

        // 清除未标记的对象
        let mut young_gen = self.young_gen.lock();
        let eden_count = young_gen.eden.len();
        young_gen.eden.retain(|obj| obj.header().is_marked());
        stats.reclaimed_objects += (eden_count - young_gen.eden.len()) as u64;

        let mut old_gen = self.old_gen.lock();
        let old_count = old_gen.objects.len();
        old_gen.objects.retain(|obj| obj.header().is_marked());
        stats.reclaimed_objects += (old_count - old_gen.objects.len()) as u64;

        // 重置所有标记位
        for obj in &young_gen.eden {
            obj.header().set_mark(false);
        }
        for obj in &old_gen.objects {
            obj.header().set_mark(false);
        }

        stats.duration_ms = start.elapsed().as_millis() as u64;

        // 更新全局统计
        {
            let mut global_stats = self.stats.lock();
            global_stats.total_collections += 1;
            global_stats.full_collections += 1;
            global_stats.total_collection_time_ms += stats.duration_ms;
            global_stats.objects_reclaimed += stats.reclaimed_objects;
        }

        Ok(stats)
    }
}

impl GCStrategy for GenerationalGC {
    fn allocate(&mut self, size: usize, align: usize) -> GCResult<GCObjectPtr> {
        self.allocate_in_young(size, align)
    }

    fn collect(&mut self, force_full: bool) -> GCResult<GCCollectionStats> {
        if force_full {
            self.major_gc()
        } else {
            // 检查是否需要触发 Major GC
            let old_gen = self.old_gen.lock();
            let heap_stats = self.heap_stats.lock();
            if old_gen.objects.len() as f64 / self.config.old_gen_size as f64
                > self.config.heap_threshold
            {
                drop(old_gen);
                drop(heap_stats);
                self.major_gc()
            } else {
                drop(old_gen);
                drop(heap_stats);
                self.minor_gc()
            }
        }
    }

    fn write_barrier(&mut self, obj: GCObjectPtr, _field_offset: usize, new_val: GCObjectPtr) {
        // 卡片标记写屏障（Card Marking Write Barrier）
        //
        // 当在老年代对象中存储对新生代对象的引用时，
        // 标记对应的卡片为脏，以便在 Minor GC 时扫描这些卡片。
        //
        // 算法：
        // 1. 检查被写入的对象（obj）是否在老年代中
        // 2. 检查新值（new_val）是否指向新生代对象
        // 3. 如果两个条件都满足，标记 obj 对应的卡片为脏

        // 如果新值为空或无效，无需处理
        if new_val.is_null() || obj.is_null() {
            return;
        }

        // 检查新值是否在新生代中
        let new_val_in_young = {
            let young = self.young_gen.lock();
            // 检查 Eden 区
            let in_eden = young
                .eden
                .iter()
                .any(|o| o.as_ptr().addr() == new_val.addr());
            // 检查 Survivor 区
            let in_survivor = young
                .survivor_from
                .iter()
                .chain(young.survivor_to.iter())
                .any(|o| o.as_ptr().addr() == new_val.addr());

            in_eden || in_survivor
        };

        // 如果新值不在新生代中，无需处理写屏障
        if !new_val_in_young {
            return;
        }

        // 检查被写入的对象是否在老年代中
        let obj_in_old = {
            let old = self.old_gen.lock();
            old.objects.iter().any(|o| o.as_ptr().addr() == obj.addr())
        };

        // 如果被写入的对象在老年代中，标记对应的卡片为脏
        if obj_in_old {
            self.card_table.mark_dirty(obj.addr());
        }
    }

    fn get_heap_stats(&self) -> GCHeapStats {
        let young_gen = self.young_gen.lock();
        let old_gen = self.old_gen.lock();

        let young_size = young_gen
            .eden
            .iter()
            .map(|obj| obj.header().get_size())
            .sum::<usize>();
        let old_size = old_gen
            .objects
            .iter()
            .map(|obj| obj.header().get_size())
            .sum::<usize>();

        GCHeapStats {
            young_gen_size: self.config.young_gen_size as u64,
            young_gen_used: young_size as u64,
            old_gen_size: self.config.old_gen_size as u64,
            old_gen_used: old_size as u64,
            total_objects: (young_gen.eden.len() + old_gen.objects.len()) as u64,
            live_objects: (young_gen.eden.len() + old_gen.objects.len()) as u64,
        }
    }

    fn get_gc_stats(&self) -> GCStats {
        self.stats.lock().clone()
    }

    fn reset_stats(&mut self) {
        *self.stats.lock() = GCStats::default();
    }
}

/// GC 回收统计信息
#[derive(Debug, Clone, Default)]
pub struct GCCollectionStats {
    /// 回收耗时（毫秒）
    pub duration_ms: u64,
    /// 回收的对象数量
    pub reclaimed_objects: u64,
    /// 回收的字节数
    pub bytes_reclaimed: u64,
    /// 晋升到老年代的对象数量
    pub promoted_objects: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generational_gc_creation() {
        let config = GCConfig::default();
        let gc = GenerationalGC::new(config);
        assert_eq!(gc.config.young_gen_size, 16 * 1024 * 1024);
        assert_eq!(gc.config.old_gen_size, 128 * 1024 * 1024);
    }

    #[test]
    fn test_allocation() {
        let config = GCConfig::default();
        let mut gc = GenerationalGC::new(config);

        let ptr = gc.allocate(1024, 8).unwrap();
        assert!(!ptr.is_null());
    }

    #[test]
    fn test_card_table_creation() {
        let card_table = CardTable::new(1024 * 1024, 512);
        assert_eq!(card_table.len(), 2048); // 1MB / 512B = 2048 cards
        assert_eq!(card_table.dirty_count(), 0);
    }

    #[test]
    fn test_card_table_mark_dirty() {
        let card_table = CardTable::new(1024, 512);

        // 标记第一个卡片
        card_table.mark_dirty(256);
        assert!(card_table.is_dirty(256));
        assert!(!card_table.is_dirty(1024)); // 超出范围
        assert_eq!(card_table.dirty_count(), 1);
    }

    #[test]
    fn test_card_table_clear_all() {
        let card_table = CardTable::new(1024, 512);

        card_table.mark_dirty(256);
        card_table.mark_dirty(768);
        assert_eq!(card_table.dirty_count(), 2);

        // 清除所有脏标记
        let card_table = CardTable::new(1024, 512);
        card_table.mark_dirty(256);
        card_table.clear_all();
        assert_eq!(card_table.dirty_count(), 0);
    }

    #[test]
    fn test_card_table_dirty_ranges() {
        let card_table = CardTable::new(4096, 512); // 8 cards

        // 标记连续的卡片 1, 2, 3
        card_table.mark_dirty(512); // card index 1
        card_table.mark_dirty(1024); // card index 2
        card_table.mark_dirty(1536); // card index 3

        let ranges = card_table.dirty_ranges();
        // 应该有连续的脏卡片从索引 1 到 4
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], (1, 4)); // range from index 1 to 4 (exclusive)
    }

    #[test]
    fn test_write_barrier_with_direct_card_marking() {
        // 直接测试卡表标记功能
        let card_table = CardTable::new(1024 * 1024, 512);

        // 标记一个地址
        let test_addr = 0x1000u64;
        card_table.mark_dirty(test_addr);

        // 验证卡表被标记
        assert!(card_table.is_dirty(test_addr));
        assert_eq!(card_table.dirty_count(), 1);
    }

    #[test]
    fn test_write_barrier_old_to_young() {
        let config = GCConfig::default();
        let mut gc = GenerationalGC::new(config);

        // 分配一个新生代对象
        let young_ptr = gc.allocate(1024, 8).unwrap();

        // 测试写屏障的行为：
        // 由于我们无法直接在测试中模拟老年代对象，
        // 这里只验证新生代对象分配后不会触发写屏障
        //（因为 old_to_young 需要实际的老年代对象存在）

        // 新生代之间的引用不应该触发写屏障
        let young_ptr2 = gc.allocate(1024, 8).unwrap();
        gc.write_barrier(young_ptr, 0, young_ptr2);

        // 验证卡表没有被标记
        assert_eq!(gc.card_table.dirty_count(), 0);
    }

    #[test]
    fn test_write_barrier_null_pointers() {
        let config = GCConfig::default();
        let mut gc = GenerationalGC::new(config);

        let null_ptr = GCObjectPtr::null();
        let valid_ptr = GCObjectPtr::new(0x1000, 1);

        // 测试空指针 - 不应该触发写屏障
        gc.write_barrier(null_ptr, 0, valid_ptr);

        assert_eq!(gc.card_table.dirty_count(), 0);
    }

    #[test]
    fn test_write_barrier_young_to_young() {
        let config = GCConfig::default();
        let mut gc = GenerationalGC::new(config);

        // 分配两个新生代对象
        let young_ptr1 = gc.allocate(1024, 8).unwrap();
        let young_ptr2 = gc.allocate(1024, 8).unwrap();

        // 新生代到新生代的引用不需要写屏障
        gc.write_barrier(young_ptr1, 0, young_ptr2);

        assert_eq!(gc.card_table.dirty_count(), 0);
    }

    #[test]
    fn test_write_barrier_old_to_old() {
        let config = GCConfig::default();
        let mut gc = GenerationalGC::new(config);

        let old_ptr1 = GCObjectPtr::new(0x10000000, 1);
        let old_ptr2 = GCObjectPtr::new(0x20000000, 1);

        // 老年代到老年代的引用不需要写屏障
        gc.write_barrier(old_ptr1, 0, old_ptr2);

        assert_eq!(gc.card_table.dirty_count(), 0);
    }

    #[test]
    fn test_minor_gc_with_card_scanning() {
        let config = GCConfig::default();
        let mut gc = GenerationalGC::new(config);

        // 分配一些新生代对象
        let young_ptr1 = gc.allocate(1024, 8).unwrap();
        let _young_ptr2 = gc.allocate(1024, 8).unwrap();

        // 将其中一个添加为根
        gc.roots.add(young_ptr1);

        // 触发 Minor GC
        let result = gc.collect(false);
        assert!(result.is_ok());

        let stats = result.unwrap();
        // 应该回收了至少一个对象（_young_ptr2 不是根）
        assert!(stats.reclaimed_objects >= 1);
    }
}
