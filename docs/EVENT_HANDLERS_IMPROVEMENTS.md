# 事件处理器完善总结

## 概述

本文档总结了事件处理器的完善工作，包括过滤、路由、重试逻辑和统计功能的实现。

## 完成的工作

### 1. 增强的事件处理器模块 (`vm-service/src/event_handlers.rs`)

**新增功能**:
- **重试机制**: `RetryEventHandler` - 支持失败重试，可配置最大重试次数和延迟
- **统计功能**: `StatsEventHandler` - 自动收集处理统计信息（成功/失败/处理时间）
- **事件过滤**: 
  - `VmIdFilter` - 按VM ID过滤事件
  - `AddressRangeFilter` - 按地址范围过滤内存事件
  - `EventTypeFilter` - 按事件类型过滤
  - `AndFilter` / `OrFilter` - 组合过滤器（AND/OR逻辑）
- **事件路由**: `EventRouter` - 支持事件路由和转换

**使用示例**:
```rust
use vm_service::event_handlers::{RetryEventHandler, StatsEventHandler, VmIdFilter};
use vm_core::domain_event_bus::{DomainEventBus, SimpleEventHandler};

// 创建带重试的处理器
let base_handler = SimpleEventHandler::new(|event| {
    // 处理逻辑
    Ok(())
});
let retry_handler = RetryEventHandler::new(
    Box::new(base_handler),
    3,  // 最大重试3次
    100, // 重试延迟100ms
);

// 创建带统计的处理器
let stats_handler = StatsEventHandler::new(Box::new(retry_handler));
let stats = stats_handler.stats(); // 获取统计信息

// 使用过滤器
let filter = Box::new(VmIdFilter::new("vm-001".to_string()));
event_bus.subscribe("memory.allocated", Box::new(stats_handler), Some(filter))?;
```

### 2. 完善的内存事件处理器 (`vm-service/src/memory_event_handler.rs`)

**改进**:
- 添加了 `MemoryEventStats` 统计结构
- 实现了内存分配/释放/映射/页错误的统计跟踪
- 添加了 `unregister_handlers()` 方法用于取消订阅
- 改进了日志记录（使用 `info!` 和 `warn!`）

**统计信息**:
- `total_allocated` - 总分配内存（字节）
- `total_freed` - 总释放内存（字节）
- `page_fault_count` - 页错误次数
- `mapping_count` - 内存映射次数
- `current_allocated` - 当前分配的内存（字节）

### 3. 完善的执行引擎事件处理器 (`vm-service/src/execution_event_handler.rs`)

**改进**:
- 添加了 `ExecutionEventStats` 统计结构
- 实现了指令执行/代码编译/热点检测/vCPU退出的统计跟踪
- 添加了 `unregister_handlers()` 方法
- 改进了日志记录

**统计信息**:
- `total_instructions` - 执行的指令总数
- `total_compiled_blocks` - 编译的代码块数
- `total_hotspots` - 检测到的热点数
- `total_vcpu_exits` - vCPU退出次数
- `hotspot_counts` - 热点PC到执行次数的映射

### 4. 完善的设备事件处理器 (`vm-service/src/device_event_handler.rs`)

**改进**:
- 添加了 `DeviceEventStats` 统计结构
- 实现了设备添加/移除/中断/I/O完成的统计跟踪
- 添加了 `unregister_handlers()` 方法
- 改进了日志记录

**统计信息**:
- `devices_added` - 添加的设备数
- `devices_removed` - 移除的设备数
- `total_interrupts` - 设备中断总数
- `total_io_bytes` - I/O完成的总字节数
- `device_interrupt_counts` - 每个设备的中断次数

### 5. 新增快照事件处理器 (`vm-service/src/snapshot_event_handler.rs`)

**功能**:
- 实现了快照创建/恢复/删除的事件处理
- 添加了 `SnapshotEventStats` 统计结构
- 提供了事件发布辅助函数

**统计信息**:
- `snapshots_created` - 创建的快照数
- `snapshots_restored` - 恢复的快照数
- `snapshots_deleted` - 删除的快照数
- `total_snapshot_size` - 总快照大小（字节）

### 6. 事件总线增强 (`vm-core/src/domain_event_bus.rs`)

**新增功能**:
- `unsubscribe_by_id()` - 通过订阅ID取消订阅（自动查找事件类型）

## 使用示例

### 完整的事件处理器设置

```rust
use vm_core::domain_event_bus::DomainEventBus;
use vm_service::{
    memory_event_handler::MemoryEventHandler,
    execution_event_handler::ExecutionEventHandler,
    device_event_handler::DeviceEventHandler,
    snapshot_event_handler::SnapshotEventHandler,
};

let event_bus = Arc::new(DomainEventBus::new());
let vm_id = "vm-001".to_string();

// 创建并注册所有事件处理器
let memory_handler = MemoryEventHandler::new(vm_id.clone(), event_bus.clone());
memory_handler.register_handlers()?;

let execution_handler = ExecutionEventHandler::new(vm_id.clone(), event_bus.clone());
execution_handler.register_handlers()?;

let device_handler = DeviceEventHandler::new(vm_id.clone(), event_bus.clone());
device_handler.register_handlers()?;

let snapshot_handler = SnapshotEventHandler::new(vm_id.clone(), event_bus.clone());
snapshot_handler.register_handlers()?;

// 获取统计信息
let memory_stats = memory_handler.stats();
let execution_stats = execution_handler.stats();
let device_stats = device_handler.stats();
let snapshot_stats = snapshot_handler.stats();

// 取消订阅（清理）
memory_handler.unregister_handlers()?;
execution_handler.unregister_handlers()?;
device_handler.unregister_handlers()?;
snapshot_handler.unregister_handlers()?;
```

### 使用重试和统计

```rust
use vm_service::event_handlers::{RetryEventHandler, StatsEventHandler};
use vm_core::domain_event_bus::{DomainEventBus, SimpleEventHandler};

let base_handler = SimpleEventHandler::new(|event| {
    // 可能失败的操作
    risky_operation()?;
    Ok(())
});

// 包装重试逻辑
let retry_handler = RetryEventHandler::new(
    Box::new(base_handler),
    3,   // 最大重试3次
    100, // 重试延迟100ms
);

// 包装统计逻辑
let stats_handler = StatsEventHandler::new(Box::new(retry_handler));

event_bus.subscribe("some.event", Box::new(stats_handler), None)?;

// 稍后获取统计
let stats = stats_handler.stats();
println!("Processed: {}, Succeeded: {}, Failed: {}, Retried: {}",
    stats.total_processed,
    stats.total_succeeded,
    stats.total_failed,
    stats.total_retried);
```

### 使用过滤器

```rust
use vm_service::event_handlers::{VmIdFilter, AddressRangeFilter, AndFilter};
use vm_core::domain_event_bus::{DomainEventBus, EventFilter};

// 创建组合过滤器
let vm_filter = Box::new(VmIdFilter::new("vm-001".to_string()));
let addr_filter = Box::new(AddressRangeFilter::new(0x1000, 0x2000));
let combined_filter = Box::new(AndFilter::new(vec![vm_filter, addr_filter]));

event_bus.subscribe("memory.allocated", handler, Some(combined_filter))?;
```

## 注意事项

1. **类型限制**: 由于 `DomainEvent` trait 的限制，无法直接从 trait 对象中提取具体的事件数据。实际使用时应该使用类型匹配或事件枚举。

2. **性能考虑**: 
   - 事件处理是同步的，如果性能敏感，应该使用异步事件总线
   - 过滤器会增加开销，应该谨慎使用
   - 统计信息使用 `Arc<Mutex<>>`，对于高频事件可能有锁竞争

3. **错误处理**: 
   - 事件处理器返回错误不会阻止其他处理器的执行
   - 重试机制应该用于临时性错误，而不是永久性错误

4. **内存管理**: 
   - 记得调用 `unregister_handlers()` 清理订阅
   - 长期运行的系统应该监控订阅数量，避免内存泄漏

## 后续改进建议

1. **异步处理**: 为高频事件实现异步处理器
2. **事件序列化**: 支持事件的序列化和持久化
3. **事件溯源**: 实现完整的事件溯源功能
4. **性能优化**: 优化统计信息的收集，减少锁竞争
5. **类型安全**: 改进类型系统，支持从事件中提取具体数据


