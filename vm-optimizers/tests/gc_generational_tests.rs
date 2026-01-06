//! 分代GC测试套件
//!
//! 测试分代垃圾回收的功能和性能

use std::sync::atomic::Ordering;

use vm_gc::common::ObjectPtr;
use vm_gc::generational::enhanced::{Card, CardTable, GenerationalGCConfig, GenerationalGCStats};

// ============================================================================
// CardTable测试
// ============================================================================

#[test]
fn test_card_table_creation() {
    let mut card_table = CardTable::new(1024 * 1024, 512);

    // 验证卡片表创建成功（无法直接访问私有字段验证）
    // 只验证操作不会panic
    card_table.mark_dirty(0);
    assert_eq!(card_table.dirty_cards().count(), 1);
}

#[test]
fn test_card_table_mark_dirty() {
    let mut card_table = CardTable::new(1024, 512);

    // 标记地址0为脏
    card_table.mark_dirty(0);

    // 通过dirty_cards()迭代器验证脏卡片
    let dirty_cards: Vec<_> = card_table.dirty_cards().collect();
    assert_eq!(dirty_cards.len(), 1);
    assert_eq!(dirty_cards[0].0, 0); // 第一个卡片（索引0）
    assert!(dirty_cards[0].1.dirty);
}

#[test]
fn test_card_table_mark_dirty_multiple() {
    let mut card_table = CardTable::new(2048, 512);

    // 标记多个地址为脏
    card_table.mark_dirty(0); // 卡片0
    card_table.mark_dirty(512); // 卡片1
    card_table.mark_dirty(1024); // 卡片2

    // 通过dirty_cards()迭代器验证脏卡片
    let dirty_cards: Vec<_> = card_table.dirty_cards().collect();
    assert_eq!(dirty_cards.len(), 3);
    assert!(dirty_cards.iter().all(|(_, card)| card.dirty));
}

#[test]
fn test_card_table_dirty_cards_iterator() {
    let mut card_table = CardTable::new(2048, 512);

    // 标记一些卡片为脏
    card_table.mark_dirty(0);
    card_table.mark_dirty(512);
    card_table.mark_dirty(1024);

    // 迭代脏卡片
    let dirty_count = card_table.dirty_cards().count();
    assert_eq!(dirty_count, 3);
}

#[test]
fn test_card_table_clear_dirty() {
    let mut card_table = CardTable::new(1024, 512);

    // 标记为脏
    card_table.mark_dirty(0);
    card_table.mark_dirty(512);
    card_table.mark_dirty(1024);

    // 清空脏标记
    card_table.clear_dirty();

    // 所有卡片应该不再为脏（通过dirty_cards()验证）
    let dirty_count = card_table.dirty_cards().count();
    assert_eq!(dirty_count, 0);
}

#[test]
fn test_card_table_add_object_to_card() {
    let mut card_table = CardTable::new(1024, 512);

    // 添加对象到卡片0
    card_table.add_object_to_card(0, ObjectPtr(1));
    card_table.add_object_to_card(0, ObjectPtr(2));
    card_table.add_object_to_card(0, ObjectPtr(3));

    // 标记为脏以便通过公共API访问
    card_table.mark_dirty(0);

    // 通过dirty_cards()迭代器验证对象
    let dirty_cards: Vec<_> = card_table.dirty_cards().collect();
    assert_eq!(dirty_cards.len(), 1);
    assert_eq!(dirty_cards[0].0, 0); // 卡片索引
    assert_eq!(dirty_cards[0].1.objects.len(), 3);
    assert_eq!(dirty_cards[0].1.objects[0], ObjectPtr(1));
    assert_eq!(dirty_cards[0].1.objects[1], ObjectPtr(2));
    assert_eq!(dirty_cards[0].1.objects[2], ObjectPtr(3));
}

#[test]
fn test_card_table_different_cards() {
    let mut card_table = CardTable::new(2048, 512);

    // 添加对象到不同的卡片
    card_table.add_object_to_card(0, ObjectPtr(1));
    card_table.add_object_to_card(512, ObjectPtr(2));
    card_table.add_object_to_card(1024, ObjectPtr(3));

    // 标记为脏以便通过公共API访问
    card_table.mark_dirty(0);
    card_table.mark_dirty(512);
    card_table.mark_dirty(1024);

    // 通过dirty_cards()迭代器验证
    let dirty_cards: Vec<_> = card_table.dirty_cards().collect();
    assert_eq!(dirty_cards.len(), 3);

    // 验证每个卡片都有1个对象
    for (_idx, card) in dirty_cards.iter() {
        assert_eq!(card.objects.len(), 1);
    }
}

// ============================================================================
// Card测试
// ============================================================================

#[test]
fn test_card_creation() {
    let card = Card {
        dirty: false,
        objects: Vec::new(),
    };

    assert!(!card.dirty);
    assert!(card.objects.is_empty());
}

#[test]
fn test_card_with_objects() {
    let mut card = Card {
        dirty: false,
        objects: Vec::new(),
    };

    card.objects.push(ObjectPtr(1));
    card.objects.push(ObjectPtr(2));

    assert_eq!(card.objects.len(), 2);
    assert!(!card.dirty);
}

// ============================================================================
// GenerationalGCConfig测试
// ============================================================================

#[test]
fn test_generational_gc_config_default() {
    let config = GenerationalGCConfig::default();

    assert_eq!(config.nursery_size, 16 * 1024 * 1024); // 16MB
    assert_eq!(config.promotion_age, 3);
    assert_eq!(config.promotion_ratio, 0.8);
    assert!(config.use_card_table);
    assert_eq!(config.card_size, 512);
}

#[test]
fn test_generational_gc_config_custom() {
    let config = GenerationalGCConfig {
        nursery_size: 32 * 1024 * 1024, // 32MB
        promotion_age: 5,
        promotion_ratio: 0.7,
        use_card_table: false,
        card_size: 1024,
    };

    assert_eq!(config.nursery_size, 32 * 1024 * 1024);
    assert_eq!(config.promotion_age, 5);
    assert_eq!(config.promotion_ratio, 0.7);
    assert!(!config.use_card_table);
    assert_eq!(config.card_size, 1024);
}

// ============================================================================
// GenerationalGCStats测试
// ============================================================================

#[test]
fn test_generational_gc_stats_default() {
    let stats = GenerationalGCStats::default();

    assert_eq!(stats.minor_gc_count.load(Ordering::Relaxed), 0);
    assert_eq!(stats.minor_gc_time_ns.load(Ordering::Relaxed), 0);
    assert_eq!(stats.major_gc_count.load(Ordering::Relaxed), 0);
    assert_eq!(stats.major_gc_time_ns.load(Ordering::Relaxed), 0);
    assert_eq!(stats.promoted_objects.load(Ordering::Relaxed), 0);
    assert_eq!(stats.collected_objects.load(Ordering::Relaxed), 0);
}

#[test]
fn test_generational_gc_stats_minor_gc() {
    let stats = GenerationalGCStats::default();

    // 执行3次Minor GC，每次2ms
    stats.minor_gc_count.fetch_add(3, Ordering::Relaxed);
    stats
        .minor_gc_time_ns
        .fetch_add(6_000_000, Ordering::Relaxed); // 6ms total

    // 平均Minor GC时间 = 6ms / 3 = 2ms
    assert_eq!(stats.avg_minor_gc_time_ms(), 2.0);
}

#[test]
fn test_generational_gc_stats_major_gc() {
    let stats = GenerationalGCStats::default();

    // 执行2次Major GC，每次10ms
    stats.major_gc_count.fetch_add(2, Ordering::Relaxed);
    stats
        .major_gc_time_ns
        .fetch_add(20_000_000, Ordering::Relaxed); // 20ms total

    // 平均Major GC时间 = 20ms / 2 = 10ms
    assert_eq!(stats.avg_major_gc_time_ms(), 10.0);
}

#[test]
fn test_generational_gc_stats_promotion() {
    let stats = GenerationalGCStats::default();

    // 晋升100个对象
    stats.promoted_objects.fetch_add(100, Ordering::Relaxed);

    // 回收200个对象
    stats.collected_objects.fetch_add(200, Ordering::Relaxed);

    assert_eq!(stats.promoted_objects.load(Ordering::Relaxed), 100);
    assert_eq!(stats.collected_objects.load(Ordering::Relaxed), 200);
}

#[test]
fn test_generational_gc_stats_avg_time_zero_count() {
    let stats = GenerationalGCStats::default();

    // 当GC次数为0时，平均时间应该为0
    assert_eq!(stats.avg_minor_gc_time_ms(), 0.0);
    assert_eq!(stats.avg_major_gc_time_ms(), 0.0);
}

#[test]
fn test_generational_gc_stats_promotion_rate() {
    let stats = GenerationalGCStats::default();

    // 晋升100个，回收400个
    stats.promoted_objects.fetch_add(100, Ordering::Relaxed);
    stats.collected_objects.fetch_add(400, Ordering::Relaxed);

    let promoted = stats.promoted_objects.load(Ordering::Relaxed);
    let collected = stats.collected_objects.load(Ordering::Relaxed);

    // 晋升率 = 晋升 / (晋升 + 回收) = 100 / 500 = 0.2
    let total = (promoted + collected) as f64;
    let rate = promoted as f64 / total;

    assert_eq!(rate, 0.2);
}

#[test]
fn test_generational_gc_stats_gc_ratio() {
    let stats = GenerationalGCStats::default();

    // 10次Minor GC，2次Major GC
    stats.minor_gc_count.fetch_add(10, Ordering::Relaxed);
    stats.major_gc_count.fetch_add(2, Ordering::Relaxed);

    let minor_count = stats.minor_gc_count.load(Ordering::Relaxed);
    let major_count = stats.major_gc_count.load(Ordering::Relaxed);

    // Minor/Major比例 = 10 / 2 = 5
    let ratio = minor_count as f64 / major_count as f64;

    assert_eq!(ratio, 5.0);
}

// ============================================================================
// CardTable性能测试
// ============================================================================

#[test]
fn test_card_table_mark_dirty_performance() {
    let mut card_table = CardTable::new(1024 * 1024, 512);

    let start = std::time::Instant::now();

    // 标记1000次
    for i in 0..1000 {
        card_table.mark_dirty(i * 512);
    }

    let duration = start.elapsed();

    // 应该很快 (< 10ms)
    assert!(duration.as_millis() < 10);
}

#[test]
fn test_card_table_dirty_iteration_performance() {
    let mut card_table = CardTable::new(1024 * 1024, 512);

    // 标记100个卡片为脏
    for i in 0..100 {
        card_table.mark_dirty(i * 512);
    }

    let start = std::time::Instant::now();

    // 迭代所有脏卡片
    let count = card_table.dirty_cards().count();

    let duration = start.elapsed();

    assert_eq!(count, 100);
    assert!(duration.as_millis() < 10);
}

#[test]
fn test_card_table_add_object_performance() {
    let mut card_table = CardTable::new(1024 * 1024, 512);

    let start = std::time::Instant::now();

    // 添加1000个对象
    for i in 0..1000 {
        card_table.add_object_to_card(0, ObjectPtr(i));
    }

    let duration = start.elapsed();

    // 应该很快 (< 10ms)
    assert!(duration.as_millis() < 10);

    // 标记为脏以便通过公共API验证
    card_table.mark_dirty(0);
    let dirty_cards: Vec<_> = card_table.dirty_cards().collect();
    assert_eq!(dirty_cards[0].1.objects.len(), 1000);
}

// ============================================================================
// 并发测试
// ============================================================================

#[test]
fn test_card_table_concurrent_mark_dirty() {
    use std::sync::Arc;
    use std::thread;

    let card_table = Arc::new(std::sync::Mutex::new(CardTable::new(1024 * 1024, 512)));
    let mut handles = vec![];

    // 多线程并发标记脏
    for i in 0..10 {
        let ct_clone = Arc::clone(&card_table);
        let handle = thread::spawn(move || {
            let mut ct = ct_clone.lock().unwrap();
            for j in 0..100 {
                ct.mark_dirty((i * 100 + j) * 512);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let ct = card_table.lock().unwrap();
    // 应该有1000个脏卡片
    let dirty_count = ct.dirty_cards().count();
    assert_eq!(dirty_count, 1000);
}

#[test]
fn test_gc_stats_concurrent_updates() {
    use std::sync::Arc;
    use std::thread;

    let stats = Arc::new(GenerationalGCStats::default());
    let mut handles = vec![];

    // 多线程并发更新统计
    for _ in 0..10 {
        let stats_clone = Arc::clone(&stats);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                stats_clone.minor_gc_count.fetch_add(1, Ordering::Relaxed);
                stats_clone
                    .minor_gc_time_ns
                    .fetch_add(1_000_000, Ordering::Relaxed); // 1ms
                stats_clone.promoted_objects.fetch_add(i, Ordering::Relaxed);
                stats_clone
                    .collected_objects
                    .fetch_add(i * 2, Ordering::Relaxed);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // 验证统计
    let minor_count = stats.minor_gc_count.load(Ordering::Relaxed);
    assert_eq!(minor_count, 1000); // 10 threads * 100

    let total_time = stats.minor_gc_time_ns.load(Ordering::Relaxed);
    assert_eq!(total_time, 1_000_000_000); // 1000ms

    // 晋升和回收数
    let promoted = stats.promoted_objects.load(Ordering::Relaxed);
    let collected = stats.collected_objects.load(Ordering::Relaxed);

    // 0+1+...+99 = 4950, 乘以10个线程 = 49500
    assert_eq!(promoted, 49500);
    assert_eq!(collected, 99000); // 49500 * 2
}

// ============================================================================
// 边界条件测试
// ============================================================================

#[test]
fn test_card_table_zero_heap_size() {
    let mut card_table = CardTable::new(0, 512);

    // 即使堆大小为0，也应该能创建
    // 无法直接访问cards长度，但验证操作不会panic
    card_table.mark_dirty(0);
    assert_eq!(card_table.dirty_cards().count(), 0);
}

#[test]
fn test_card_table_large_card_size() {
    let mut card_table = CardTable::new(1024, 1024);

    // 卡片大小为1024，应该只有1个卡片
    // 标记有效地址，验证可以正常工作
    card_table.mark_dirty(0);
    assert_eq!(card_table.dirty_cards().count(), 1);
}

#[test]
fn test_card_table_small_card_size() {
    let mut card_table = CardTable::new(1024, 64);

    // 卡片大小为64，应该有 1024/64 = 16 个卡片
    // 标记多个地址，验证都可以正常工作
    for i in 0..16 {
        card_table.mark_dirty(i * 64);
    }
    assert_eq!(card_table.dirty_cards().count(), 16);
}

#[test]
fn test_card_table_mark_out_of_bounds() {
    let mut card_table = CardTable::new(1024, 512);

    // 标记超出范围的地址不应该panic
    card_table.mark_dirty(10 * 1024); // 超出堆大小

    // 所有卡片应该保持不变（没有脏卡片）
    assert_eq!(card_table.dirty_cards().count(), 0);
}

#[test]
fn test_card_table_add_object_out_of_bounds() {
    let mut card_table = CardTable::new(1024, 512);

    // 添加对象到超出范围的地址
    card_table.add_object_to_card(10 * 1024, ObjectPtr(1));

    // 应该安全处理，不添加到任何卡片（没有脏卡片）
    assert_eq!(card_table.dirty_cards().count(), 0);
}

#[test]
fn test_generational_gc_stats_large_values() {
    let stats = GenerationalGCStats::default();

    // 添加非常大的值
    stats
        .minor_gc_count
        .fetch_add(u64::MAX / 2, Ordering::Relaxed);
    stats
        .minor_gc_time_ns
        .fetch_add(u64::MAX / 2, Ordering::Relaxed);

    // 应该不会溢出
    let count = stats.minor_gc_count.load(Ordering::Relaxed);
    assert!(count > 0);
}

#[test]
fn test_card_table_clear_multiple_times() {
    let mut card_table = CardTable::new(1024, 512);

    // 标记为脏
    card_table.mark_dirty(0);
    card_table.mark_dirty(512);

    // 清空多次
    card_table.clear_dirty();
    card_table.clear_dirty();
    card_table.clear_dirty();

    // 所有卡片应该保持不脏
    assert_eq!(card_table.dirty_cards().count(), 0);
}

#[test]
fn test_card_table_dirty_after_adding_object() {
    let mut card_table = CardTable::new(1024, 512);

    // 添加对象不会自动标记为脏
    card_table.add_object_to_card(0, ObjectPtr(1));

    // 验证没有脏卡片
    assert_eq!(card_table.dirty_cards().count(), 0);
}
