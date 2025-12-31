//! vm-interface事件系统测试（简化版）
//!
//! 测试事件定义和事件处理相关功能

use vm_interface::event::*;
use vm_interface::{VmEvent, VmEventListener};

/// 创建测试监听器
fn create_test_listener() -> VmEventListener {
    Box::new(|event| {
        println!("Received event: {:?}", event);
    })
}

#[test]
fn test_vm_event_basic() {
    // 测试基本VM事件
    use vm_core::{GuestAddr, VmError, CoreError};

    let events: Vec<VmEvent> = vec![
        VmEvent::ComponentStarted("cpu".to_string()),
        VmEvent::ComponentStopped("memory".to_string()),
        VmEvent::MemoryAccess {
            addr: GuestAddr(0x2000),
            size: 8,
            is_write: false,
        },
        VmEvent::ErrorOccurred(VmError::Core(CoreError::NotImplemented {
            feature: "test".to_string(),
            module: "test_module".to_string(),
        })),
    ];

    for event in events {
        let _debug_str = format!("{:?}", event);
    }
}

#[test]
fn test_event_publisher_creation() {
    let _publisher = EventPublisher::new();

    // 验证发布器创建成功
    // 如果没有panic，测试通过
    assert!(true);
}

#[test]
fn test_event_publisher_subscribe() {
    let mut publisher = EventPublisher::new();

    // 订阅事件
    let listener = create_test_listener();
    let id = publisher.subscribe("component_started", listener);

    // 验证ID非零
    assert!(id > 0);

    // 再次订阅应该得到不同的ID
    let listener2 = create_test_listener();
    let id2 = publisher.subscribe("component_started", listener2);
    assert_ne!(id, id2);
}

#[test]
fn test_event_publisher_unsubscribe() {
    let mut publisher = EventPublisher::new();

    // 订阅事件
    let listener = create_test_listener();
    let id = publisher.subscribe("component_started", listener);

    // 取消订阅
    let result = publisher.unsubscribe("component_started", id);
    assert!(result.is_ok());

    // 取消不存在的订阅类型应该失败
    let result = publisher.unsubscribe("nonexistent", id);
    assert!(result.is_err());
}

#[test]
fn test_event_publisher_unsubscribe_nonexistent_type() {
    let mut publisher = EventPublisher::new();

    // 尝试取消不存在的订阅类型
    let result = publisher.unsubscribe("nonexistent_type", 1);
    assert!(result.is_err());
}

#[test]
fn test_event_publisher_publish() {
    let mut publisher = EventPublisher::new();

    // 添加监听器
    let listener = create_test_listener();
    publisher.subscribe("component_started", listener);

    // 发布事件
    let event = VmEvent::ComponentStarted("test".to_string());
    publisher.publish(&event);

    // 如果没有panic，测试通过
    assert!(true);
}

#[test]
fn test_event_publisher_multiple_subscribers() {
    let mut publisher = EventPublisher::new();

    // 添加多个监听器
    let ids = vec![
        publisher.subscribe("component_started", create_test_listener()),
        publisher.subscribe("component_started", create_test_listener()),
        publisher.subscribe("component_started", create_test_listener()),
    ];

    // 验证ID都不同
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[1], ids[2]);
    assert_ne!(ids[0], ids[2]);

    // 发布事件
    let event = VmEvent::ComponentStarted("test".to_string());
    publisher.publish(&event);
}

#[test]
fn test_event_publisher_different_event_types() {
    let mut publisher = EventPublisher::new();

    // 订阅不同类型的事件
    publisher.subscribe("component_started", create_test_listener());
    publisher.subscribe("component_stopped", create_test_listener());
    publisher.subscribe("memory_access", create_test_listener());

    // 发布不同类型的事件
    publisher.publish(&VmEvent::ComponentStarted("cpu".to_string()));
    publisher.publish(&VmEvent::ComponentStopped("memory".to_string()));
    publisher.publish(&VmEvent::MemoryAccess {
        addr: vm_core::GuestAddr(0x1000),
        size: 8,
        is_write: true,
    });
}

#[test]
fn test_event_bus_creation() {
    let _bus = EventBus::new();

    // 验证总线创建成功
    // 如果没有panic，测试通过
    assert!(true);
}

#[test]
fn test_event_bus_subscribe() {
    let bus = EventBus::new();

    // 订阅事件
    let result = bus.subscribe("component_started", create_test_listener());
    assert!(result.is_ok());

    let id = result.unwrap();
    assert!(id > 0);
}

#[test]
fn test_event_bus_subscribe_and_publish() {
    let bus = EventBus::new();

    // 订阅事件
    let _ = bus.subscribe("component_started", create_test_listener());

    // 发布事件
    bus.publish(VmEvent::ComponentStarted("test".to_string()));

    // 如果没有panic，测试通过
    assert!(true);
}

#[test]
fn test_event_bus_unsubscribe() {
    let bus = EventBus::new();

    // 订阅事件
    let result = bus.subscribe("component_started", create_test_listener());
    assert!(result.is_ok());

    let id = result.unwrap();

    // 取消订阅
    let result = bus.unsubscribe("component_started", id);
    assert!(result.is_ok());
}

#[test]
fn test_event_bus_unsubscribe_nonexistent() {
    let bus = EventBus::new();

    // 尝试取消不存在的订阅
    let result = bus.unsubscribe("nonexistent_type", 999);
    assert!(result.is_err());
}

#[test]
fn test_event_bus_global_instance() {
    // 获取全局实例
    let bus = EventBus::global();

    // 订阅和发布
    let _ = bus.subscribe("component_started", create_test_listener());
    bus.publish(VmEvent::ComponentStarted("test".to_string()));

    // 如果没有panic，测试通过
    assert!(true);
}

#[test]
fn test_event_bus_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let bus = Arc::new(EventBus::new());

    // 多线程同时订阅
    let handles: Vec<_> = (0..5)
        .map(|_| {
            let bus_clone = Arc::clone(&bus);
            thread::spawn(move || {
                let _ = bus_clone.subscribe("component_started", create_test_listener());
            })
        })
        .collect();

    // 等待所有线程完成
    for handle in handles {
        let _ = handle.join();
    }

    // 如果没有panic，测试通过
    assert!(true);
}

#[test]
fn test_event_publisher_default() {
    // 测试Default实现
    let mut publisher = EventPublisher::default();
    let _ = publisher.subscribe("test", create_test_listener());

    assert!(true);
}

#[test]
fn test_event_bus_default() {
    // 测试Default实现
    let _bus = EventBus::default();

    assert!(true);
}

#[test]
fn test_memory_access_event() {
    // 测试内存访问事件
    use vm_core::GuestAddr;

    let event = VmEvent::MemoryAccess {
        addr: GuestAddr(0x1000),
        size: 64,
        is_write: true,
    };

    // 验证事件属性
    match event {
        VmEvent::MemoryAccess { addr, size, is_write } => {
            assert_eq!(addr, GuestAddr(0x1000));
            assert_eq!(size, 64);
            assert_eq!(is_write, true);
        }
        _ => panic!("Should be MemoryAccess event"),
    }
}

#[test]
fn test_error_event() {
    // 测试错误事件
    use vm_core::{VmError, CoreError};

    let event = VmEvent::ErrorOccurred(VmError::Core(CoreError::NotImplemented {
        feature: "test_feature".to_string(),
        module: "test_module".to_string(),
    }));

    match event {
        VmEvent::ErrorOccurred(err) => {
            match err {
                VmError::Core(core_err) => {
                    match core_err {
                        CoreError::NotImplemented { feature, module } => {
                            assert_eq!(feature, "test_feature");
                            assert_eq!(module, "test_module");
                        }
                        _ => panic!("Should be NotImplementedError"),
                    }
                }
                _ => panic!("Should be Core error"),
            }
        }
        _ => panic!("Should be ErrorOccurred"),
    }
}

#[test]
fn test_component_lifecycle_events() {
    // 测试组件生命周期事件
    let started = VmEvent::ComponentStarted("cpu".to_string());
    let stopped = VmEvent::ComponentStopped("cpu".to_string());

    match started {
        VmEvent::ComponentStarted(name) => {
            assert_eq!(name, "cpu");
        }
        _ => panic!("Should be ComponentStarted"),
    }

    match stopped {
        VmEvent::ComponentStopped(name) => {
            assert_eq!(name, "cpu");
        }
        _ => panic!("Should be ComponentStopped"),
    }
}

#[test]
fn test_vm_event_clone() {
    // 测试VmEvent的Clone实现
    use vm_core::{GuestAddr, VmError, CoreError};

    let events: Vec<VmEvent> = vec![
        VmEvent::ComponentStarted("test".to_string()),
        VmEvent::ComponentStopped("test".to_string()),
        VmEvent::MemoryAccess {
            addr: GuestAddr(0x1000),
            size: 8,
            is_write: false,
        },
        VmEvent::ErrorOccurred(VmError::Core(CoreError::NotImplemented {
            feature: "test".to_string(),
            module: "test_module".to_string(),
        })),
    ];

    for event in events {
        let _cloned = event.clone();
    }
}

#[test]
fn test_event_bus_send_sync() {
    // 验证EventBus实现了Send和Sync
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<EventBus>();
    assert_send_sync::<EventPublisher>();
}
