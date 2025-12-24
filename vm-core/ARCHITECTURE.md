# vm-core 模块架构说明

## 概述

vm-core 是虚拟机系统的核心模块，实现了领域驱动设计（DDD）模式和事件溯源架构。该模块负责虚拟机的生命周期管理、领域事件发布、以及核心领域逻辑的协调。

## 架构层次

```
vm-core
├── aggregate_root.rs          # 聚合根 - VirtualMachineAggregate
├── domain.rs                 # 领域接口定义
├── domain_events.rs          # 领域事件定义
├── event_sourcing.rs         # 事件溯源核心
├── domain_services/          # 领域服务（贫血模型：业务逻辑所在）
│   ├── adaptive_optimization_service.rs
│   ├── architecture_compatibility_service.rs
│   ├── cache_management_service.rs
│   ├── cross_architecture_translation_service.rs
│   ├── execution_manager_service.rs
│   ├── optimization_pipeline_service.rs
│   ├── page_table_walker_service.rs
│   ├── performance_optimization_service.rs
│   ├── register_allocation_service.rs
│   ├── resource_management_service.rs
│   ├── target_optimization_service.rs
│   ├── tlb_management_service.rs
│   ├── translation_strategy_service.rs
│   └── vm_lifecycle_service.rs
├── debugger/                # 调试器相关
│   ├── enhanced_gdb_server.rs
│   ├── multi_thread_debug.rs
│   └── unified_debugger.rs
├── di/                      # 依赖注入
├── event_store/             # 事件存储
└── vm_state.rs             # VM 状态值对象
```

## DDD 贫血模型

本项目采用 DDD 贫血模型模式：
- **聚合根**（`VirtualMachineAggregate`）只包含状态数据，不包含复杂业务逻辑
- **领域服务**（`domain_services/`）包含所有业务逻辑
- **值对象**（`value_objects.rs`）只包含数据和简单的数据验证

## 核心组件

### 1. VirtualMachineAggregate

聚合根，负责：
- 虚拟机状态管理（`VmLifecycleState`）
- 状态变更事件发布
- 事件版本控制
- 状态一致性保证

```rust
pub struct VirtualMachineAggregate {
    vm_id: String,
    config: VmConfig,
    state: VmLifecycleState,
    event_bus: Option<Arc<DomainEventBus>>,
    uncommitted_events: Vec<DomainEventEnum>,
    version: u64,
}
```

### 2. Domain Services

领域服务是贫血模型中业务逻辑的载体：

| 服务 | 职责 |
|------|--------|
| `CrossArchitectureTranslationDomainService` | 跨架构翻译业务规则 |
| `ExecutionManagerDomainService` | 执行管理 |
| `OptimizationPipelineDomainService` | 优化管道 |
| `PerformanceOptimizationDomainService` | 性能优化 |
| `TlbManagementDomainService` | TLB 管理 |
| `VmLifecycleDomainService` | VM 生命周期管理 |

### 3. Event Sourcing

事件溯源实现：
- `FileEventStore`: 基于文件的事件存储
- `PostgresEventStore`: 基于数据库的事件存储
- `AsyncEventBus`: 异步事件总线
- `UnifiedEventBus`: 统一事件总线

### 4. 依赖注入

DI 容器实现：
- `DiContainer`: 核心容器
- `DiBuilder`: 构建器模式
- `DiInjector`: 依赖注入器

## 领域接口

### TlbManager

TLB 管理接口：
```rust
pub trait TlbManager {
    fn lookup(&mut self, addr: u64) -> Option<TlbEntry>;
    fn insert(&mut self, addr: u64, entry: TlbEntry);
    fn invalidate(&mut self, addr: u64);
    fn flush(&mut self);
}
```

### PageTableWalker

页表遍历接口：
```rust
pub trait PageTableWalker {
    fn walk(&mut self, addr: u64) -> Result<PhysicalAddress, WalkError>;
    fn walk_batch(&mut self, addrs: &[u64]) -> Result<Vec<PhysicalAddress>, WalkError>;
}
```

### ExecutionManager

执行管理接口：
```rust
pub trait ExecutionManager {
    fn execute(&mut self, block: &IRBlock) -> ExecResult;
    fn get_execution_stats(&self) -> ExecutionStats;
}
```

## 领域事件

事件类型：
- `VmCreatedEvent`: 虚拟机创建事件
- `VmStartedEvent`: 虚拟机启动事件
- `VmStoppedEvent`: 虚拟机停止事件
- `VmPausedEvent`: 虚拟机暂停事件
- `VmResumedEvent`: 虚拟机恢复事件
- `VmSnapshotCreatedEvent`: 快照创建事件
- `VmConfigurationUpdatedEvent`: 配置更新事件

## 调试器支持

### GDB 服务器

- `EnhancedGdbServer`: 增强 GDB 服务器
- 支持 RSP 协议
- 多线程调试支持
- 断点管理

### 调试功能

- 断点管理
- 单步执行
- 寄存器查看
- 内存查看
- 调用栈跟踪

## 异步支持

### 异步执行引擎

- `AsyncExecutionEngine`: 异步执行引擎
- `AsyncHybridExecutor`: 异步混合执行器
- 支持协程和 async/await

### 异步 MMU

- `AsyncMmu`: 异步内存管理单元
- 支持异步内存访问
- 与协程调度器集成

## 配置管理

### VmConfig

虚拟机配置：
```rust
pub struct VmConfig {
    pub cpu_count: u32,
    pub memory_size: u64,
    pub cpu_type: CpuType,
    pub enable_jit: bool,
    pub enable_tiered_compilation: bool,
    pub enable_simd: bool,
}
```

## 使用示例

### 创建虚拟机

```rust
use vm_core::{VirtualMachineAggregate, VmConfig, VmLifecycleState};

let config = VmConfig::default();
let mut vm_aggregate = VirtualMachineAggregate::new("vm-001".to_string(), config)?;

vm_aggregate.start()?;
```

### 发布事件

```rust
let event = VmStartedEvent {
    vm_id: "vm-001".to_string(),
    timestamp: SystemTime::now(),
};
vm_aggregate.publish_event(event)?;
```

### 使用领域服务

```rust
use vm_core::domain_services::CrossArchitectureTranslationDomainService;

let translation_service = CrossArchitectureTranslationDomainService::new()?;
let result = translation_service.translate_block(src_block, src_arch, target_arch)?;
```

## 性能优化

### 1. 事件优化

- 异步事件发布
- 事件批量处理
- 事件去重

### 2. 状态管理

- 不可变状态模式
- 快照优化
- 增量快照

### 3. 调试器优化

- 非侵入式断点
- 高效内存访问
- 并发调试支持

## 测试策略

- 单元测试：测试聚合根和值对象
- 集成测试：测试领域服务和事件流
- 领域测试：测试业务规则和不变量

## 扩展点

### 添加新的领域服务

1. 在 `domain_services/` 创建新的服务文件
2. 实现 `DomainService` trait
3. 在聚合根中注册服务

### 添加新的领域事件

1. 在 `domain_events.rs` 定义事件类型
2. 添加到 `DomainEventEnum`
3. 实现事件的序列化和反序列化

### 添加新的领域接口

1. 在 `domain.rs` 定义 trait
2. 为不同实现提供适配器
3. 更新 DI 容器配置

## 与其他模块的交互

| 模块 | 交互方式 |
|------|----------|
| `vm-engine-jit` | 通过 `ExecutionManager` 接口 |
| `vm-runtime` | 通过 `CoroutineScheduler` 集成 |
| `vm-cross-arch` | 通过 `CrossArchitectureTranslationDomainService` |
| `vm-mem` | 通过 `PageTableWalker` 和 `TlbManager` 接口 |
| `vm-accel` | 通过硬件加速接口 |

## 最佳实践

1. **保持聚合根简单**：聚合根只管理状态，不包含业务逻辑
2. **使用领域服务**：所有业务逻辑应在领域服务中实现
3. **事件溯源**：使用事件记录状态变更历史
4. **依赖注入**：使用 DI 容器管理依赖关系
5. **接口隔离**：使用 trait 定义清晰的接口边界

## 未来改进方向

1. 增强领域服务功能
2. 完善事件存储性能
3. 增加更多调试功能
4. 改进异步执行引擎
5. 优化快照和恢复性能
