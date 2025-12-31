//! vm-interface类型和基本功能测试
//!
//! 测试vm-interface中定义的各种类型和基本功能

use vm_interface::*;

#[test]
fn test_component_status() {
    // 测试ComponentStatus枚举的所有状态
    let statuses = vec![
        ComponentStatus::Uninitialized,
        ComponentStatus::Initialized,
        ComponentStatus::Starting,
        ComponentStatus::Running,
        ComponentStatus::Stopping,
        ComponentStatus::Stopped,
        ComponentStatus::Error,
    ];

    for status in statuses {
        // 验证状态可以被克隆和比较
        let cloned = status;
        assert_eq!(status, cloned);

        // 验证状态可以被Debug打印
        let _debug_str = format!("{:?}", status);

        // 验证状态可以用于模式匹配
        match status {
            ComponentStatus::Running => {}
            ComponentStatus::Stopped => {}
            _ => {}
        }
    }
}

#[test]
fn test_task_status() {
    // 测试TaskStatus枚举
    let statuses = vec![
        TaskStatus::Pending,
        TaskStatus::Running,
        TaskStatus::Completed,
        TaskStatus::Failed,
    ];

    for status in statuses {
        let _cloned = status.clone();
        let _debug_str = format!("{:?}", status);
    }
}

#[test]
fn test_task_result() {
    use vm_core::{VmError, CoreError};

    // 测试TaskResult枚举
    let success = TaskResult::Success;
    let failure = TaskResult::Failure(VmError::Core(CoreError::NotImplemented {
        feature: "test".to_string(),
        module: "test_module".to_string(),
    }));

    match success {
        TaskResult::Success => {}
        TaskResult::Failure(_) => panic!("Should be Success"),
    }

    match failure {
        TaskResult::Success => panic!("Should be Failure"),
        TaskResult::Failure(e) => {
            let _error_msg = format!("{:?}", e);
        }
    }
}

#[test]
fn test_memory_order() {
    // 测试MemoryOrder枚举
    let orders = vec![
        MemoryOrder::Relaxed,
        MemoryOrder::Acquire,
        MemoryOrder::Release,
        MemoryOrder::AcqRel,
        MemoryOrder::SeqCst,
    ];

    for order in orders {
        let cloned = order;
        assert_eq!(order, cloned);

        let _debug_str = format!("{:?}", order);
    }
}

#[test]
fn test_page_flags() {
    // PageFlags是一个不透明的包装类型
    // 测试它的类型属性而不是具体值

    // 验证PageFlags的大小
    assert_eq!(std::mem::size_of::<PageFlags>(), std::mem::size_of::<u64>());

    // PageFlags实现了Copy和PartialEq
    fn assert_page_flags_properties<T: Copy + PartialEq>() {}
    assert_page_flags_properties::<PageFlags>();

    // PageFlags可以被Debug打印（但无法构造实例进行测试）
    let _type_name = std::any::type_name::<PageFlags>();
    assert_eq!(_type_name, "vm_interface::PageFlags");
}

#[test]
fn test_cache_stats() {
    // 测试CacheStats结构
    let stats = CacheStats {
        hits: 100,
        misses: 20,
        evictions: 5,
    };

    assert_eq!(stats.hits, 100);
    assert_eq!(stats.misses, 20);
    assert_eq!(stats.evictions, 5);

    // 测试Default实现
    let default_stats = CacheStats::default();
    assert_eq!(default_stats.hits, 0);
    assert_eq!(default_stats.misses, 0);
    assert_eq!(default_stats.evictions, 0);

    // 测试Clone
    let cloned = stats.clone();
    assert_eq!(stats.hits, cloned.hits);

    // 测试Debug
    let _debug_str = format!("{:?}", stats);
}

#[test]
fn test_page_stats() {
    // 测试PageStats结构
    let stats = PageStats {
        translations: 1000,
        faults: 50,
        flushes: 10,
    };

    assert_eq!(stats.translations, 1000);
    assert_eq!(stats.faults, 50);
    assert_eq!(stats.flushes, 10);

    // 测试Default实现
    let default_stats = PageStats::default();
    assert_eq!(default_stats.translations, 0);
    assert_eq!(default_stats.faults, 0);
    assert_eq!(default_stats.flushes, 0);

    // 测试Clone
    let cloned = stats.clone();
    assert_eq!(stats.translations, cloned.translations);

    // 测试Debug
    let _debug_str = format!("{:?}", stats);
}

#[test]
fn test_hot_stats() {
    // 测试HotStats结构
    let stats = HotStats {
        total_executions: 10000,
        hot_blocks: 100,
        compiled_blocks: 50,
    };

    assert_eq!(stats.total_executions, 10000);
    assert_eq!(stats.hot_blocks, 100);
    assert_eq!(stats.compiled_blocks, 50);

    // 测试Default实现
    let default_stats = HotStats::default();
    assert_eq!(default_stats.total_executions, 0);
    assert_eq!(default_stats.hot_blocks, 0);
    assert_eq!(default_stats.compiled_blocks, 0);

    // 测试Clone
    let cloned = stats.clone();
    assert_eq!(stats.total_executions, cloned.total_executions);

    // 测试Debug
    let _debug_str = format!("{:?}", stats);
}

#[test]
fn test_type_aliases() {
    // 测试类型别名是否正确
    let _id: SubscriptionId = 12345;
    let _task_id: TaskId = 67890;

    // 类型别名应该是u64
    assert_eq!(std::mem::size_of::<SubscriptionId>(), std::mem::size_of::<u64>());
    assert_eq!(std::mem::size_of::<TaskId>(), std::mem::size_of::<u64>());
}

#[test]
fn test_component_status_serialization() {
    // 测试ComponentStatus的序列化
    use serde_json;

    let status = ComponentStatus::Running;

    // 序列化
    let serialized = serde_json::to_string(&status);
    assert!(serialized.is_ok());

    // 反序列化
    let serialized_str = serialized.unwrap();
    let deserialized: Result<ComponentStatus, _> = serde_json::from_str(&serialized_str);
    assert!(deserialized.is_ok());

    assert_eq!(status, deserialized.unwrap());
}

#[test]
fn test_all_component_status_serialize() {
    use serde_json;

    let statuses = vec![
        ComponentStatus::Uninitialized,
        ComponentStatus::Initialized,
        ComponentStatus::Starting,
        ComponentStatus::Running,
        ComponentStatus::Stopping,
        ComponentStatus::Stopped,
        ComponentStatus::Error,
    ];

    for status in statuses {
        let serialized = serde_json::to_string(&status);
        assert!(serialized.is_ok(), "Failed to serialize {:?}", status);

        let serialized_str = serialized.unwrap();
        let deserialized: Result<ComponentStatus, _> = serde_json::from_str(&serialized_str);
        assert!(deserialized.is_ok(), "Failed to deserialize {}", serialized_str);

        assert_eq!(status, deserialized.unwrap());
    }
}

#[test]
fn test_cache_stats_update() {
    let mut stats = CacheStats::default();

    // 模拟更新统计
    stats.hits += 10;
    stats.misses += 5;
    stats.evictions += 2;

    assert_eq!(stats.hits, 10);
    assert_eq!(stats.misses, 5);
    assert_eq!(stats.evictions, 2);

    // 计算命中率
    let total = stats.hits + stats.misses;
    if total > 0 {
        let hit_rate = stats.hits as f64 / total as f64;
        assert!((hit_rate - 0.666).abs() < 0.01); // 10/15 ≈ 0.666
    }
}

#[test]
fn test_page_stats_update() {
    let mut stats = PageStats::default();

    // 模拟页表操作
    stats.translations += 100;
    stats.faults += 5;
    stats.flushes += 1;

    assert_eq!(stats.translations, 100);
    assert_eq!(stats.faults, 5);
    assert_eq!(stats.flushes, 1);

    // 计算故障率
    if stats.translations > 0 {
        let fault_rate = stats.faults as f64 / stats.translations as f64;
        assert!((fault_rate - 0.05).abs() < 0.01); // 5/100 = 0.05
    }
}

#[test]
fn test_hot_metrics() {
    let mut stats = HotStats::default();

    // 模拟执行和编译
    stats.total_executions = 1000;
    stats.hot_blocks = 50;
    stats.compiled_blocks = 25;

    // 计算热点块比例
    if stats.total_executions > 0 {
        let hot_ratio = stats.hot_blocks as f64 / stats.total_executions as f64;
        assert!((hot_ratio - 0.05).abs() < 0.01); // 50/1000 = 0.05
    }

    // 计算编译率
    if stats.hot_blocks > 0 {
        let compile_ratio = stats.compiled_blocks as f64 / stats.hot_blocks as f64;
        assert!((compile_ratio - 0.5).abs() < 0.01); // 25/50 = 0.5
    }
}

#[test]
fn test_stats_send_sync() {
    // 验证统计类型实现了Send和Sync
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<CacheStats>();
    assert_send_sync::<PageStats>();
    assert_send_sync::<HotStats>();
    assert_send_sync::<ComponentStatus>();
    assert_send_sync::<MemoryOrder>();
    assert_send_sync::<PageFlags>();
}

#[test]
fn test_copy_types() {
    // 验证哪些类型实现了Copy
    fn assert_copy<T: Copy>() {}

    assert_copy::<ComponentStatus>();
    assert_copy::<MemoryOrder>();
    assert_copy::<PageFlags>();

    // 这些类型不应该实现Copy（包含堆数据或复杂逻辑）
    // CacheStats, PageStats, HotStats通过Clone实现
}
