# 事件溯源实现文档

## 更新时间
2025-12-04

## 概述

本文档说明事件溯源（Event Sourcing）模式的实现，包括事件存储、事件回放和集成到EventDrivenVmService。

## 架构设计

### 核心组件

1. **EventStore**: 事件存储trait，定义事件持久化接口
2. **InMemoryEventStore**: 内存事件存储实现（用于测试和开发）
3. **VirtualMachineAggregate::from_events**: 从事件回放重建聚合状态
4. **EventDrivenVmService**: 集成事件存储和回放功能

### 事件流

```
用户操作 -> VirtualMachineAggregate -> 生成事件 -> 提交到事件总线 -> 存储到EventStore
                                                                    ↓
回放: EventStore -> 加载事件 -> VirtualMachineAggregate::from_events -> 重建状态
```

## 主要功能

### 1. 事件存储

所有领域事件都会被存储到`EventStore`中，包括：

- VM生命周期事件（创建、启动、暂停、恢复、停止）
- 内存事件（分配、释放、映射、页错误）
- 执行引擎事件（指令执行、代码编译、热点检测）
- 设备事件（添加、移除、中断、I/O完成）
- 快照事件（创建、恢复、删除）

### 2. 事件回放

通过`VirtualMachineAggregate::from_events`方法，可以从事件流重建聚合状态：

```rust
use vm_core::{VirtualMachineAggregate, EventStore, InMemoryEventStore};
use vm_ir::IRBlock;

// 创建事件存储
let store = InMemoryEventStore::new();

// 加载事件
let events = store.load_events("vm-001", None, None)?;

// 从事件重建聚合
let aggregate = VirtualMachineAggregate::from_events(
    "vm-001".to_string(),
    config,
    events,
);
```

### 3. 集成到EventDrivenVmService

`EventDrivenVmService`现在支持：

- **自动事件存储**: 所有通过聚合根产生的事件都会自动存储
- **事件回放**: 可以通过`replay_events()`方法重建状态
- **从事件重建**: 创建服务时，如果有历史事件，会自动回放重建状态

## 使用示例

### 基本使用

```rust
use vm_service::vm_service_event_driven::EventDrivenVmService;
use vm_core::{VmId, VmConfig, EventStore, InMemoryEventStore};
use vm_core::vm_state::VirtualMachineState;
use vm_mem::SoftMmu;

// 创建事件存储
let event_store = Arc::new(InMemoryEventStore::new());

// 创建服务（使用事件存储）
let vm_id = VmId::new("vm-001".to_string())?;
let config = VmConfig::default();
let mmu = Box::new(SoftMmu::new(128 * 1024 * 1024, false));
let state = VirtualMachineState::new(config.clone(), mmu);

let service = EventDrivenVmService::with_event_store(
    vm_id,
    config,
    state,
    event_store.clone(),
)?;

// 执行操作（事件会自动存储）
service.start()?;
service.pause()?;
service.resume()?;

// 回放事件重建状态
service.replay_events()?;

// 查询事件
let events = event_store.load_events("vm-001", None, None)?;
println!("Total events: {}", events.len());
```

### 事件查询

```rust
// 获取所有事件
let all_events = event_store.load_events("vm-001", None, None)?;

// 获取指定范围的事件
let range_events = event_store.load_events("vm-001", Some(1), Some(10))?;

// 获取事件总数
let count = event_store.get_event_count("vm-001")?;

// 获取最后一个事件序号
let last_seq = event_store.get_last_sequence_number("vm-001")?;

// 列出所有VM ID
let vm_ids = event_store.list_vm_ids()?;
```

## 实现细节

### EventStore Trait

```rust
pub trait EventStore: Send + Sync {
    /// 追加事件到事件流
    fn append(
        &self,
        vm_id: &str,
        sequence_number: Option<u64>,
        event: DomainEventEnum,
    ) -> VmResult<u64>;

    /// 加载指定VM的所有事件
    fn load_events(
        &self,
        vm_id: &str,
        from_sequence: Option<u64>,
        to_sequence: Option<u64>,
    ) -> VmResult<Vec<StoredEvent>>;

    /// 获取指定VM的最后一个事件序号
    fn get_last_sequence_number(&self, vm_id: &str) -> VmResult<u64>;

    /// 获取指定VM的事件总数
    fn get_event_count(&self, vm_id: &str) -> VmResult<usize>;

    /// 列出所有有事件的VM ID
    fn list_vm_ids(&self) -> VmResult<Vec<String>>;

    /// 删除指定VM的所有事件
    fn delete_events(&self, vm_id: &str) -> VmResult<()>;
}
```

### 事件回放

`VirtualMachineAggregate::from_events`方法通过应用每个事件来重建状态：

```rust
pub fn from_events(
    vm_id: String,
    config: VmConfig,
    events: Vec<StoredEvent>
) -> Self {
    let mut aggregate = Self {
        vm_id: vm_id.clone(),
        config: config.clone(),
        state: VmState::Created,
        event_bus: None,
        uncommitted_events: Vec::new(),
        version: 0,
    };

    // 回放所有事件
    for stored_event in events {
        aggregate.apply_event(&stored_event.event);
        aggregate.version = stored_event.sequence_number;
    }

    aggregate
}
```

## 未来改进

1. **持久化存储**: 实现基于文件或数据库的事件存储
2. **快照机制**: 定期创建快照，加速事件回放
3. **事件版本化**: 支持事件模式演进和版本迁移
4. **事件压缩**: 压缩旧事件，减少存储空间
5. **事件查询优化**: 支持按事件类型、时间范围等查询

## 相关文档

- `docs/ARCHITECTURE.md`: 架构文档
- `docs/EVENT_DRIVEN_MIGRATION.md`: 事件驱动迁移指南
- `ARCHITECTURE_EVOLUTION.md`: 架构演进报告

