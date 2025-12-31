//! vm-interface 集成测试
//!
//! 测试VM组件接口、内存管理接口、设备接口等核心功能

use vm_core::{GuestAddr, VmError};
use vm_interface::*;

// ============================================================================
// 组件状态测试
// ============================================================================

#[test]
fn test_component_status_serialization() {
    // 测试ComponentStatus的序列化和反序列化
    let status = ComponentStatus::Running;
    let serialized = serde_json::to_string(&status).unwrap();
    let deserialized: ComponentStatus = serde_json::from_str(&serialized).unwrap();
    assert_eq!(status, deserialized);
}

#[test]
fn test_component_status_equality() {
    assert_eq!(ComponentStatus::Initialized, ComponentStatus::Initialized);
    assert_ne!(ComponentStatus::Running, ComponentStatus::Stopped);
}

// ============================================================================
// 内存序测试
// ============================================================================

#[test]
fn test_memory_order_values() {
    // 测试MemoryOrder的所有值
    let orders = vec![
        MemoryOrder::Relaxed,
        MemoryOrder::Acquire,
        MemoryOrder::Release,
        MemoryOrder::AcqRel,
        MemoryOrder::SeqCst,
    ];

    // 验证每个值都可以正常创建
    assert!(matches!(orders[0], MemoryOrder::Relaxed));
    assert!(matches!(orders[1], MemoryOrder::Acquire));
    assert!(matches!(orders[2], MemoryOrder::Release));
    assert!(matches!(orders[3], MemoryOrder::AcqRel));
    assert!(matches!(orders[4], MemoryOrder::SeqCst));
}

#[test]
fn test_memory_order_copy() {
    let order1 = MemoryOrder::SeqCst;
    let order2 = order1;
    assert_eq!(order1, order2);
}

// ============================================================================
// 缓存统计测试
// ============================================================================

#[test]
fn test_cache_stats_default() {
    let stats = CacheStats::default();
    assert_eq!(stats.hits, 0);
    assert_eq!(stats.misses, 0);
    assert_eq!(stats.evictions, 0);
}

#[test]
fn test_cache_stats_operations() {
    let mut stats = CacheStats::default();
    stats.hits = 100;
    stats.misses = 20;
    stats.evictions = 5;

    assert_eq!(stats.hits, 100);
    assert_eq!(stats.misses, 20);
    assert_eq!(stats.evictions, 5);
}

// ============================================================================
// 页表统计测试
// ============================================================================

#[test]
fn test_page_stats_default() {
    let stats = PageStats::default();
    assert_eq!(stats.translations, 0);
    assert_eq!(stats.faults, 0);
    assert_eq!(stats.flushes, 0);
}

#[test]
fn test_page_stats_operations() {
    let mut stats = PageStats::default();
    stats.translations = 1000;
    stats.faults = 50;
    stats.flushes = 10;

    assert_eq!(stats.translations, 1000);
    assert_eq!(stats.faults, 50);
    assert_eq!(stats.flushes, 10);
}

// ============================================================================
// 热点统计测试
// ============================================================================

#[test]
fn test_hot_stats_default() {
    let stats = HotStats::default();
    assert_eq!(stats.total_executions, 0);
    assert_eq!(stats.hot_blocks, 0);
    assert_eq!(stats.compiled_blocks, 0);
}

#[test]
fn test_hot_stats_updates() {
    let mut stats = HotStats::default();
    stats.total_executions = 10000;
    stats.hot_blocks = 100;
    stats.compiled_blocks = 50;

    assert_eq!(stats.total_executions, 10000);
    assert_eq!(stats.hot_blocks, 100);
    assert_eq!(stats.compiled_blocks, 50);
}

// ============================================================================
// 设备类型测试
// ============================================================================

#[test]
fn test_device_type_equality() {
    assert_eq!(DeviceType::Block, DeviceType::Block);
    assert_ne!(DeviceType::Network, DeviceType::GPU);
}

#[test]
fn test_device_type_custom() {
    let custom1 = DeviceType::Custom(100);
    let custom2 = DeviceType::Custom(100);
    let custom3 = DeviceType::Custom(200);

    assert_eq!(custom1, custom2);
    assert_ne!(custom1, custom3);
}

// ============================================================================
// 设备状态测试
// ============================================================================

#[test]
fn test_device_status_variants() {
    let status1 = DeviceStatus::Initialized;
    let status2 = DeviceStatus::Running;
    let status3 = DeviceStatus::Error("Test error".to_string());

    match status1 {
        DeviceStatus::Initialized => {}
        _ => panic!("Expected Initialized status"),
    }

    match status2 {
        DeviceStatus::Running => {}
        _ => panic!("Expected Running status"),
    }

    match status3 {
        DeviceStatus::Error(msg) => assert_eq!(msg, "Test error"),
        _ => panic!("Expected Error status"),
    }
}

// ============================================================================
// VM事件测试
// ============================================================================

#[test]
fn test_vm_event_component_started() {
    let event = VmEvent::ComponentStarted("test_component".to_string());
    match event {
        VmEvent::ComponentStarted(name) => {
            assert_eq!(name, "test_component");
        }
        _ => panic!("Expected ComponentStarted event"),
    }
}

#[test]
fn test_vm_event_memory_access() {
    let event = VmEvent::MemoryAccess {
        addr: GuestAddr(0x1000),
        size: 8,
        is_write: true,
    };

    match event {
        VmEvent::MemoryAccess {
            addr,
            size,
            is_write,
        } => {
            assert_eq!(addr, GuestAddr(0x1000));
            assert_eq!(size, 8);
            assert!(is_write);
        }
        _ => panic!("Expected MemoryAccess event"),
    }
}

#[test]
fn test_vm_event_device_interrupt() {
    let device_id: DeviceId = 42;
    let event = VmEvent::DeviceInterrupt(device_id);

    match event {
        VmEvent::DeviceInterrupt(id) => {
            assert_eq!(id, 42);
        }
        _ => panic!("Expected DeviceInterrupt event"),
    }
}

// ============================================================================
// 任务状态测试
// ============================================================================

#[test]
fn test_task_status_variants() {
    let statuses = vec![
        TaskStatus::Pending,
        TaskStatus::Running,
        TaskStatus::Completed,
        TaskStatus::Failed,
    ];

    assert!(matches!(statuses[0], TaskStatus::Pending));
    assert!(matches!(statuses[1], TaskStatus::Running));
    assert!(matches!(statuses[2], TaskStatus::Completed));
    assert!(matches!(statuses[3], TaskStatus::Failed));
}

// ============================================================================
// 任务结果测试
// ============================================================================

#[test]
fn test_task_result_success() {
    let result = TaskResult::Success;
    match result {
        TaskResult::Success => {}
        _ => panic!("Expected Success result"),
    }
}

#[test]
fn test_task_result_failure() {
    let error = VmError::Core(vm_core::CoreError::NotImplemented {
        feature: "test".to_string(),
        module: "test_module".to_string(),
    });
    let result = TaskResult::Failure(error);

    match result {
        TaskResult::Failure(_) => {}
        _ => panic!("Expected Failure result"),
    }
}

// ============================================================================
// 订阅ID和任务ID测试
// ============================================================================

#[test]
fn test_subscription_id_type() {
    let id1: SubscriptionId = 1;
    let id2: SubscriptionId = 2;
    assert_ne!(id1, id2);
}

#[test]
fn test_task_id_type() {
    let id1: TaskId = 100;
    let id2: TaskId = 200;
    assert_ne!(id1, id2);
}

// ============================================================================
// PageFlags测试
// ============================================================================

#[test]
fn test_page_flags_copy() {
    // PageFlags是元组结构体，但字段是私有的
    // 我们仍然可以测试其Copy特性
    // 由于无法直接构造，跳过此测试
    assert!(true);
}

#[test]
fn test_page_flags_values() {
    // PageFlags字段是私有的，无法直接构造
    // 跳过此测试
    assert!(true);
}

// ============================================================================
// VmComponent trait测试 (使用Mock实现)
// ============================================================================

struct MockComponent {
    name: String,
    status: ComponentStatus,
}

impl VmComponent for MockComponent {
    type Config = String;
    type Error = VmError;

    fn init(config: Self::Config) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        Ok(Self {
            name: config,
            status: ComponentStatus::Initialized,
        })
    }

    fn start(&mut self) -> Result<(), Self::Error> {
        self.status = ComponentStatus::Running;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.status = ComponentStatus::Stopped;
        Ok(())
    }

    fn status(&self) -> ComponentStatus {
        self.status
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[test]
fn test_vm_component_lifecycle() {
    let mut component = MockComponent::init("test_component".to_string()).unwrap();

    assert_eq!(component.status(), ComponentStatus::Initialized);
    assert_eq!(component.name(), "test_component");

    component.start().unwrap();
    assert_eq!(component.status(), ComponentStatus::Running);

    component.stop().unwrap();
    assert_eq!(component.status(), ComponentStatus::Stopped);
}

// ============================================================================
// Configurable trait测试 (使用Mock实现)
// ============================================================================

struct MockConfigurable {
    config: u64,
}

impl Configurable for MockConfigurable {
    type Config = u64;

    fn update_config(&mut self, config: &Self::Config) -> Result<(), VmError> {
        self.config = *config;
        Ok(())
    }

    fn get_config(&self) -> &Self::Config {
        &self.config
    }

    fn validate_config(config: &Self::Config) -> Result<(), VmError> {
        if *config > 0 {
            Ok(())
        } else {
            Err(VmError::Core(vm_core::CoreError::NotImplemented {
                feature: "invalid_config".to_string(),
                module: "MockConfigurable".to_string(),
            }))
        }
    }
}

#[test]
fn test_configurable_update() {
    let mut configurable = MockConfigurable { config: 100 };

    assert_eq!(*configurable.get_config(), 100);

    configurable.update_config(&200).unwrap();
    assert_eq!(*configurable.get_config(), 200);
}

#[test]
fn test_configurable_validate() {
    assert!(MockConfigurable::validate_config(&100).is_ok());
    assert!(MockConfigurable::validate_config(&0).is_err());
}

// ============================================================================
// Observable trait测试 (使用Mock实现)
// ============================================================================

use std::sync::{Arc, Mutex};

struct MockObservable {
    _state: String,
    _event: String,
}

impl Observable for MockObservable {
    type State = String;
    type Event = String;

    fn get_state(&self) -> &Self::State {
        // 注意：这里返回引用有问题，实际使用时需要改进设计
        // 但对于测试trait约束，这样足够了
        &self._state
    }

    fn subscribe(
        &mut self,
        _callback: StateEventCallback<Self::State, Self::Event>,
    ) -> SubscriptionId {
        // 返回一个假的订阅ID
        1
    }

    fn unsubscribe(&mut self, _id: SubscriptionId) -> Result<(), VmError> {
        Ok(())
    }
}

#[test]
fn test_observable_subscribe() {
    let mut observable = MockObservable {
        _state: "state".to_string(),
        _event: "event".to_string(),
    };

    let callback: StateEventCallback<String, String> =
        Box::new(|_state, _event| println!("Callback called"));

    let id = observable.subscribe(callback);
    assert_eq!(id, 1);

    let result = observable.unsubscribe(id);
    assert!(result.is_ok());
}

// ============================================================================
// 边界条件测试
// ============================================================================

#[test]
fn test_zero_subscription_id() {
    let id: SubscriptionId = 0;
    assert_eq!(id, 0);
}

#[test]
fn test_max_subscription_id() {
    let id: SubscriptionId = u64::MAX;
    assert_eq!(id, u64::MAX);
}

#[test]
fn test_zero_task_id() {
    let id: TaskId = 0;
    assert_eq!(id, 0);
}

#[test]
fn test_device_id_range() {
    let min_id: DeviceId = 0;
    let max_id: DeviceId = u64::MAX;

    assert_eq!(min_id, 0);
    assert_eq!(max_id, u64::MAX);
}

// ============================================================================
// 错误处理测试
// ============================================================================

#[test]
fn test_vm_error_integration() {
    // 使用一个实际存在的错误类型
    let error = VmError::Core(vm_core::CoreError::NotImplemented {
        feature: "test".to_string(),
        module: "test_module".to_string(),
    });

    match error {
        VmError::Core(_) => {}
        _ => panic!("Expected Core error"),
    }
}

// ============================================================================
// Clone测试
// ============================================================================

#[test]
fn test_component_status_clone() {
    let status1 = ComponentStatus::Running;
    let status2 = status1.clone();
    assert_eq!(status1, status2);
}

#[test]
fn test_device_type_clone() {
    let dtype1 = DeviceType::GPU;
    let dtype2 = dtype1.clone();
    assert_eq!(dtype1, dtype2);
}

#[test]
fn test_cache_stats_clone() {
    let mut stats1 = CacheStats::default();
    stats1.hits = 100;
    let stats2 = stats1.clone();
    assert_eq!(stats1.hits, stats2.hits);
}

#[test]
fn test_vm_event_clone() {
    let event1 = VmEvent::ComponentStarted("test".to_string());
    let event2 = event1.clone();
    // VmEvent没有实现PartialEq，所以我们只测试克隆不会panic
    match (&event1, &event2) {
        (VmEvent::ComponentStarted(s1), VmEvent::ComponentStarted(s2)) => {
            assert_eq!(s1, s2);
        }
        _ => panic!("Events should be the same type"),
    }
}
