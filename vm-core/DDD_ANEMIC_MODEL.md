# DDD 贫血模型实现文档

本文档确认虚拟机系统遵循领域驱动设计（DDD）的贫血模型原则。

## 目录

- [贫血模型概述](#贫血模型概述)
- [当前实现](#当前实现)
- [架构设计](#架构设计)
- [设计验证](#设计验证)

---

## 贫血模型概述

贫血模型（Anemic Domain Model）是 DDD 中的一种设计模式，其核心思想是：

1. **领域对象只包含数据**：实体和值对象只存储状态，不包含业务逻辑
2. **业务逻辑在服务层**：所有业务操作通过领域服务（Domain Service）实现
3. **聚合根管理不变式**：聚合根负责维护聚合内部的一致性边界
4. **事件驱动**：通过领域事件发布状态变化

贫血模型的优点：
- 简单直观，易于理解和维护
- 数据和行为分离，职责清晰
- 适合复杂业务逻辑的场景
- 便于测试和扩展

---

## 当前实现

### 1. 值对象 (Value Objects)

值对象是不可变的、只包含数据和简单验证逻辑的对象。

#### VmId (vm-core/src/value_objects.rs:13-52)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VmId(String);

impl VmId {
    /// 验证规则：
    /// - 长度必须在1-64字符之间
    /// - 只能包含字母、数字、连字符和下划线
    pub fn new(id: String) -> Result<Self, VmError> { ... }
    
    /// 获取ID字符串
    pub fn as_str(&self) -> &str { ... }
}
```

**符合贫血模型**：
- 只包含数据 (`String`)
- 验证逻辑简单，无复杂业务逻辑
- 不可变（内部字符串不可变）

#### MemorySize (vm-core/src/value_objects.rs:66-150)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MemorySize {
    bytes: u64,
}

impl MemorySize {
    pub fn from_bytes(bytes: u64) -> Result<Self, VmError> { ... }
    pub fn from_mb(mb: u64) -> Result<Self, VmError> { ... }
    pub fn from_gb(gb: u64) -> Result<Self, VmError> { ... }
    
    pub fn bytes(&self) -> u64 { ... }
    pub fn as_mb(&self) -> u64 { ... }
    pub fn as_gb(&self) -> u64 { ... }
    pub fn is_page_aligned(&self) -> bool { ... }
}
```

**符合贫血模型**：
- 只包含数据 (`u64`)
- 单位转换是纯数学计算，无业务逻辑
- 不可变

#### VcpuCount (vm-core/src/value_objects.rs:152-195)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VcpuCount {
    count: u32,
}

impl VcpuCount {
    pub fn new(count: u32) -> Result<Self, VmError> { ... }
    pub fn count(&self) -> u32 { ... }
}
```

**符合贫血模型**：
- 只包含数据 (`u32`)
- 只有简单的验证逻辑
- 不可变

#### PortNumber (vm-core/src/value_objects.rs:197-226)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PortNumber {
    port: u16,
}

impl PortNumber {
    pub fn new(port: u16) -> Self { ... }
    pub fn port(&self) -> u16 { ... }
    pub fn is_privileged(&self) -> bool { ... }
}
```

**符合贫血模型**：
- 只包含数据 (`u16`)
- `is_privileged()` 是简单的数学比较，无业务逻辑

#### DeviceId (vm-core/src/value_objects.rs:228-261)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new(id: String) -> Result<Self, VmError> { ... }
    pub fn as_str(&self) -> &str { ... }
}
```

**符合贫血模型**：
- 只包含数据 (`String`)
- 只有简单的验证逻辑
- 不可变

---

### 2. 聚合根 (Aggregate Root)

聚合根是聚合的入口点，负责管理聚合内部的一致性。

#### VirtualMachineAggregate (vm-core/src/aggregate_root.rs:1-50)

```rust
#[derive(Debug, Clone)]
pub struct VirtualMachineAggregate {
    vm_id: VmId,
    config: VmConfig,
    state: VmLifecycleState,
    version: u64,
    uncommitted_events: Vec<DomainEventEnum>,
}

impl AggregateRoot for VirtualMachineAggregate {
    fn aggregate_id(&self) -> &str { ... }
    fn uncommitted_events(&self) -> Vec<DomainEventEnum> { ... }
    fn mark_events_as_committed(&mut self) { ... }
}
```

**符合贫血模型**：
- 只包含状态数据 (`vm_id`, `config`, `state`, `version`, `uncommitted_events`)
- 只实现 `AggregateRoot` trait，没有复杂业务逻辑
- 业务操作通过领域服务实现

---

### 3. 领域服务 (Domain Services)

领域服务包含复杂的业务逻辑，操作值对象和聚合根。

#### TlbManager (vm-core/src/domain.rs:1-155)

```rust
pub trait TlbManager: Send + Sync {
    fn lookup(&mut self, addr: GuestAddr, asid: u16, access: AccessType) -> Option<TlbEntry>;
    fn update(&mut self, entry: TlbEntry);
    fn flush(&mut self);
    fn flush_asid(&mut self, asid: u16);
    fn flush_page(&mut self, _va: GuestAddr);
    fn get_stats(&self) -> Option<TlbStats>;
}
```

**符合贫血模型**：
- 服务接口，包含业务逻辑
- 操作值对象 (`TlbEntry`, `GuestAddr`)
- 不在值对象中实现业务逻辑

#### PageTableWalker (vm-core/src/domain.rs:157-182)

```rust
pub trait PageTableWalker: Send + Sync {
    fn walk(
        &mut self,
        addr: GuestAddr,
        access: AccessType,
        asid: u16,
        mmu: &mut dyn MMU,
    ) -> Result<(GuestPhysAddr, u64), VmError>;
}
```

**符合贫血模型**：
- 服务接口，包含页表遍历的业务逻辑
- 操作值对象 (`GuestAddr`, `GuestPhysAddr`)
- 不在值对象中实现业务逻辑

#### LifecycleBusinessRule (vm-core/src/domain_services/rules/lifecycle_rules.rs:1-60)

```rust
pub trait LifecycleBusinessRule: Send + Sync {
    fn can_transition(&self, from: &VmLifecycleState, to: &VmLifecycleState) -> Result<(), VmError>;
    fn valid_transitions(&self) -> Vec<(VmLifecycleState, VmLifecycleState)>;
}
```

**符合贫血模型**：
- 业务规则在服务中实现
- 状态转换逻辑不在状态对象中
- 使用领域事件发布变化

---

### 4. 状态对象 (State Objects)

状态对象是纯数据容器，用于存储状态。

#### VirtualMachineState (vm-core/src/vm_state.rs:11-102)

```rust
pub struct VirtualMachineState<B> {
    pub config: VmConfig,
    pub state: VmLifecycleState,
    pub mmu: Arc<Mutex<Box<dyn MMU>>>,
    pub vcpus: Vec<Arc<Mutex<dyn ExecutionEngine<B>>>>,
    pub stats: ExecStats,
    pub snapshot_manager: Arc<Mutex<snapshot_legacy::SnapshotMetadataManager>>,
    pub template_manager: Arc<Mutex<template::TemplateManager>>,
}
```

**符合贫血模型**：
- 注释明确说明："这是一个纯数据结构，仅包含状态数据，不包含业务逻辑。所有业务操作应通过 VirtualMachineService 进行。"
- 只包含数据字段
- 提供简单的 getter/setter 方法

#### VmConfig (vm-core/src/lib.rs:270-298)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    pub guest_arch: GuestArch,
    pub memory_size: usize,
    pub vcpu_count: usize,
    pub exec_mode: ExecMode,
    pub kernel_path: Option<String>,
    pub initrd_path: Option<String>,
}
```

**符合贫血模型**：
- 只包含数据字段
- 没有 `impl` 块（除了 `Default` trait）
- 没有计算逻辑或业务逻辑

---

### 5. 领域事件 (Domain Events)

领域事件用于发布聚合状态变化。

#### DomainEventEnum (vm-core/src/domain_events.rs:1-200+)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainEventEnum {
    VmCreatedEvent(VmCreatedEvent),
    VmStartedEvent(VmStartedEvent),
    VmStoppedEvent(VmStoppedEvent),
    VmPausedEvent(VmPausedEvent),
    VmResumedEvent(VmResumedEvent),
    VmDestroyedEvent(VmDestroyedEvent),
    ...
}
```

**符合贫血模型**：
- 事件只包含数据（时间戳、事件类型等）
- 不包含业务逻辑
- 通过 `AggregateRoot::uncommitted_events()` 发布

---

## 架构设计

### 层次结构

```
应用层 (Application Layer)
    ↓
领域服务层 (Domain Service Layer)
    ↓
聚合根 (Aggregate Root)
    ↓
实体 (Entities) + 值对象 (Value Objects)
```

### 职责划分

| 层级 | 职责 | 示例 |
|-----|------|------|
| 值对象 | 数据 + 简单验证 | `VmId`, `MemorySize`, `VcpuCount` |
| 聚合根 | 状态管理 + 事件发布 | `VirtualMachineAggregate` |
| 领域服务 | 复杂业务逻辑 | `TlbManager`, `PageTableWalker`, `LifecycleBusinessRule` |
| 仓储 | 持久化 | `AggregateRepository`, `EventRepository` |

---

## 设计验证

### 检查清单

| 检查项 | 状态 | 说明 |
|-------|------|------|
| 值对象只包含数据 | ✅ | 所有值对象只存储数据，无复杂逻辑 |
| 值对象不可变 | ✅ | 所有值对象使用 `Clone`，内部字段不可变 |
| 聚合根只管理状态 | ✅ | `VirtualMachineAggregate` 只管理状态和事件 |
| 业务逻辑在服务层 | ✅ | 所有业务逻辑通过领域服务实现 |
| 使用领域事件 | ✅ | 通过 `DomainEventEnum` 发布状态变化 |
| 仓储模式分离持久化 | ✅ | `AggregateRepository` 和 `EventRepository` |

### 代码审查结果

经过全面代码审查，未发现贫血模型违规的情况：

1. ✅ **值对象**：`VmId`, `MemorySize`, `VcpuCount`, `PortNumber`, `DeviceId` 都只包含数据和简单验证
2. ✅ **聚合根**：`VirtualMachineAggregate` 只管理状态和事件，没有复杂业务逻辑
3. ✅ **领域服务**：所有业务逻辑都在 `TlbManager`, `PageTableWalker`, `LifecycleBusinessRule` 等服务中
4. ✅ **状态对象**：`VirtualMachineState`, `VmConfig` 是纯数据结构
5. ✅ **领域事件**：`DomainEventEnum` 只包含事件数据

---

## 总结

虚拟机系统完全遵循 DDD 贫血模型的设计原则：

1. **数据和行为分离**：领域对象只包含数据，业务逻辑在服务层
2. **不变式管理**：聚合根维护一致性边界
3. **事件驱动**：通过领域事件发布状态变化
4. **仓储模式**：持久化逻辑与领域逻辑分离

这种设计使代码更易理解、测试和维护，符合 DDD 的最佳实践。

---

## 参考

- [Domain-Driven Design: Tackling Complexity in the Heart of Software](https://www.domainlanguage.com/ddd/reference/)
- [Anemic Domain Model](https://www.martinfowler.com/bliki/AnemicDomainModel.html)
- vm-core/ARCHITECTURE.md - 架构说明文档
