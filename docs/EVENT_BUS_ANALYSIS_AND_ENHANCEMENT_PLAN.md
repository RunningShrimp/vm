# 领域事件总线分析与增强计划

**创建日期**: 2026-01-06
**任务**: P1-9 - 完善领域事件总线功能
**状态**: 📋 分析完成

---

## 📊 当前状态评估

### ✅ 已实现功能

#### 1. 核心事件系统

**位置**: `vm-core/src/domain_services/events.rs`

**事件类型**:
- ✅ **TranslationEvent** (6种)
  - StrategySelected
  - CompatibilityValidated
  - TranslationPlanned
  - InstructionEncodingValidated
  - RegisterMappingCompleted
  - PipelineOrchestrationCompleted

- ✅ **OptimizationEvent** (20种)
  - PipelineConfigCreated
  - StageCompleted
  - PipelineCompleted
  - HotspotsDetected
  - StrategySelected
  - ResourceConstraintViolation
  - ResourceAllocated
  - ResourceReleased
  - PerformanceThresholdUpdated
  - CacheHit/Miss/Put/Eviction/Promotion
  - CacheResized
  - CachePrefetch
  - TargetOptimizationCompleted
  - OptimizationEffectivenessMonitored
  - PerformanceBottleneckAnalysisCompleted
  - OptimizationRecommendationsGenerated
  - OptimizationPlanCreated
  - OptimizationExecutionCompleted
  - RegisterAllocationCompleted

**总计**: **26种事件类型**

#### 2. 事件总线实现

**InMemoryDomainEventBus** (vm-core/src/domain_services/events.rs:696)

```rust
pub struct InMemoryDomainEventBus {
    handlers: Arc<Mutex<Vec<Arc<dyn DomainEventHandler>>>>,
    events: Arc<Mutex<VecDeque<DomainEventEnum>>>,
    max_events: usize, // 默认1000
}
```

**功能**:
- ✅ 发布/订阅模式
- ✅ 事件存储 (VecDeque，最多1000条)
- ✅ 事件处理器注册
- ✅ 自动清理旧事件
- ✅ 线程安全 (Arc<Mutex<>>)

**API**:
```rust
// 发布事件
fn publish(&self, event: DomainEventEnum)

// 订阅事件
fn subscribe(&self, handler: Arc<dyn DomainEventHandler>)

// 获取所有事件
pub fn get_events(&self) -> Vec<DomainEventEnum>

// 清除事件
pub fn clear_events(&self)

// 处理器数量
pub fn handler_count(&self) -> usize
```

#### 3. 使用情况

**引用**: 18个文件使用DomainEventBus

主要服务:
- vm-engine (Jit结构体)
- vm-service (VmService)
- 13个domain services

**集成度**: 🟢 良好

---

### ❌ 缺失功能

#### 1. 事件持久化 🔴 高优先级

**当前**: 内存存储，最多1000条，重启丢失

**需求**:
- 持久化到数据库
- 重启后恢复
- 历史事件查询
- 事件溯源 (Event Sourcing)

#### 2. 异步事件处理 🟡 中优先级

**当前**: 同步处理，可能阻塞发布者

**需求**:
- 异步事件分发
- 事件队列
- 后台处理线程
- 背压控制

#### 3. 事件过滤和路由 🟡 中优先级

**当前**: 所有处理器接收所有事件

**需求**:
- 基于事件类型过滤
- 通配符订阅
- 事件路由规则
- 优先级支持

#### 4. 事件版本化和迁移 🟢 低优先级

**当前**: 无版本控制

**需求**:
- 事件schema版本
- 向后兼容性
- 事件迁移工具

#### 5. 监控和指标 🟡 中优先级

**当前**: 基础计数

**需求**:
- 发布/处理速率
- 处理延迟
- 错误率
- 死信队列

---

## 🎯 增强计划

### Phase 1: 事件持久化 (1周)

**目标**: 实现事件持久化，支持重启恢复和查询

#### 1.1 设计持久化存储

**选项A: SQLite** ⭐ 推荐
- ✅ 轻量级，无需额外服务
- ✅ 事务支持
- ✅ 易于集成
- ❌ 性能中等

**选项B: PostgreSQL**
- ✅ 高性能
- ✅ 企业级特性
- ❌ 需要额外服务
- ❌ 复杂度高

**选择**: SQLite (适合当前规模)

#### 1.2 实现EventStore trait

```rust
pub trait EventStore: Send + Sync {
    /// 追加事件到存储
    fn append(&self, event: DomainEventEnum) -> Result<(), EventStoreError>;

    /// 重放事件从指定位置
    fn replay(&self, from: SequenceNumber) -> Result<Vec<DomainEventEnum>, EventStoreError>;

    /// 查询事件
    fn query(&self, filter: EventFilter) -> Result<Vec<DomainEventEnum>, EventStoreError>;

    /// 获取最新序列号
    fn latest_sequence(&self) -> Result<SequenceNumber, EventStoreError>;
}
```

#### 1.3 实现SQLiteEventStore

**表结构**:
```sql
CREATE TABLE domain_events (
    sequence_number INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    event_data TEXT NOT NULL, -- JSON序列化
    occurred_at TEXT NOT NULL, -- ISO 8601
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_event_type ON domain_events(event_type);
CREATE INDEX idx_occurred_at ON domain_events(occurred_at);
```

#### 1.4 增强DomainEventBus

```rust
pub struct PersistentDomainEventBus {
    store: Arc<dyn EventStore>,
    in_memory: InMemoryDomainEventBus,
}

impl PersistentDomainEventBus {
    /// 从事件存储重放事件
    pub fn replay(&self) -> Result<(), EventStoreError> {
        // 从store重放到in_memory
    }

    /// 持久化当前内存事件
    pub fn persist(&self) -> Result<(), EventStoreError> {
        // 将in_memory事件写入store
    }
}
```

**预计用时**: 5-7天

---

### Phase 2: 异步事件处理 (1周)

**目标**: 异步分发事件，避免阻塞发布者

#### 2.1 实现AsyncEventBus

```rust
pub struct AsyncDomainEventBus {
    sender: mpsc::UnboundedSender<DomainEventEnum>,
    handlers: Arc<RwLock<Vec<Arc<dyn AsyncDomainEventHandler>>>>,
}

impl AsyncDomainEventBus {
    /// 异步发布事件
    pub async fn publish_async(&self, event: DomainEventEnum) -> Result<(), EventBusError> {
        self.sender.send(event)?;
        Ok(())
    }

    /// 启动事件处理循环
    async fn run(&self) {
        let mut receiver = self.receiver;
        while let Some(event) = receiver.recv().await {
            self.dispatch_to_handlers(event).await;
        }
    }
}
```

#### 2.2 实现背压控制

```rust
pub struct BoundedAsyncEventBus {
    sender: mpsc::Sender<DomainEventEnum>,
    capacity: usize,
}

impl BoundedAsyncEventBus {
    pub async fn publish_async(&self, event: DomainEventEnum) -> Result<(), EventBusError> {
        match self.sender.try_send(event) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                // 背压策略: 丢弃最旧事件
                self.drop_oldest().await;
                self.sender.try_send(event).map_err(Into::into)
            }
            Err(e) => Err(e.into()),
        }
    }
}
```

**预计用时**: 5-7天

---

### Phase 3: 事件过滤和路由 (3-5天)

**目标**: 支持事件过滤和路由

#### 3.1 实现事件过滤器

```rust
pub trait EventFilter: Send + Sync {
    fn matches(&self, event: &DomainEventEnum) -> bool;
}

// 基于类型过滤
pub struct TypeFilter {
    event_types: Vec<String>,
}

// 基于时间范围过滤
pub struct TimeRangeFilter {
    start: SystemTime,
    end: SystemTime,
}

// 组合过滤器
pub struct CompositeFilter {
    filters: Vec<Box<dyn EventFilter>>,
}
```

#### 3.2 实现路由订阅

```rust
impl DomainEventBus {
    /// 订阅特定类型的事件
    pub fn subscribe_by_type<F>(
        &self,
        event_type: &'static str,
        handler: F,
    ) -> SubscriptionHandle
    where
        F: Fn(&DomainEventEnum) + Send + Sync + 'static,
    {
        // 包装处理器，只处理匹配类型的事件
    }

    /// 通配符订阅
    pub fn subscribe_wildcard<F>(
        &self,
        pattern: &str, // "optimization.*"
        handler: F,
    ) -> SubscriptionHandle
    {
        // 使用模式匹配
    }
}
```

**预计用时**: 3-5天

---

### Phase 4: 监控和指标 (3-4天)

**目标**: 添加事件系统监控

#### 4.1 实现EventMetrics

```rust
pub struct EventMetrics {
    publish_count: AtomicU64,
    handle_count: AtomicU64,
    error_count: AtomicU64,
    avg_latency: AtomicU64,
}

impl EventMetrics {
    /// 记录发布
    pub fn record_publish(&self) {
        self.publish_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录处理延迟
    pub fn record_latency(&self, latency: Duration) {
        // EMA计算
    }

    /// 获取指标
    pub fn snapshot(&self) -> EventMetricsSnapshot {
        EventMetricsSnapshot {
            publish_count: self.publish_count.load(Ordering::Relaxed),
            handle_count: self.handle_count.load(Ordering::Relaxed),
            error_count: self.error_count.load(Ordering::Relaxed),
            avg_latency_ns: self.avg_latency.load(Ordering::Relaxed),
        }
    }
}
```

#### 4.2 集成到现有服务

```rust
impl InMemoryDomainEventBus {
    pub fn with_metrics(mut self, metrics: Arc<EventMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }
}
```

**预计用时**: 3-4天

---

## 📊 实施优先级

### 高优先级 (立即执行)

1. **事件持久化** (Phase 1)
   - 价值: ⭐⭐⭐⭐⭐
   - 用时: 5-7天
   - 原因: 重启丢失数据是严重问题

### 中优先级 (第二阶段)

2. **异步事件处理** (Phase 2)
   - 价值: ⭐⭐⭐⭐
   - 用时: 5-7天
   - 原因: 性能和响应性提升

3. **监控和指标** (Phase 4)
   - 价值: ⭐⭐⭐
   - 用时: 3-4天
   - 原因: 可观测性重要

### 低优先级 (第三阶段)

4. **事件过滤和路由** (Phase 3)
   - 价值: ⭐⭐
   - 用时: 3-5天
   - 原因: 优化，非必需

---

## 🚀 快速启动方案

### 方案A: 最小化持久化 (3-5天) ⭐ 推荐

**只实现核心持久化**:
- ✅ SQLite存储
- ✅ 启动时重放
- ✅ 基础查询
- ❌ 跳过异步处理
- ❌ 跳过复杂过滤

**价值**: 快速解决重启丢失问题

---

### 方案B: 完整实现 (3-4周)

**实现所有4个Phase**:
- ✅ 持久化
- ✅ 异步处理
- ✅ 过滤路由
- ✅ 监控指标

**价值**: 完整的事件驱动架构

---

## 📝 实施检查清单

### Phase 1: 事件持久化

- [ ] 创建EventStore trait
- [ ] 实现SQLiteEventStore
- [ ] 创建domain_events表
- [ ] 实现事件序列化/反序列化
- [ ] 实现重放逻辑
- [ ] 实现查询API
- [ ] 集成到DomainEventBus
- [ ] 编写单元测试
- [ ] 编写集成测试
- [ ] 更新文档

### Phase 2: 异步处理

- [ ] 创建AsyncDomainEventBus
- [ ] 实现事件队列
- [ ] 实现后台处理循环
- [ ] 实现背压控制
- [ ] 集成tokio运行时
- [ ] 测试并发场景
- [ ] 性能测试
- [ ] 更新文档

### Phase 3: 过滤路由

- [ ] 实现EventFilter trait
- [ ] 实现TypeFilter
- [ ] 实现TimeRangeFilter
- [ ] 实现通配符订阅
- [ ] 测试过滤逻辑
- [ ] 更新文档

### Phase 4: 监控指标

- [ ] 实现EventMetrics
- [ ] 集成到event bus
- [ ] 暴露metrics API
- [ ] 创建metrics endpoint
- [ ] 集成日志
- [ ] 更新文档

---

## 🎓 最佳实践

### 事件设计

1. **不可变性**: 事件应该是不可变的
2. **幂等性**: 处理同一事件多次应产生相同结果
3. **时间戳**: 所有事件应包含时间戳
4. **序列化**: 事件应支持序列化/反序列化
5. **版本控制**: 考虑事件schema演变

### 处理器设计

1. **快速处理**: 避免阻塞
2. **错误处理**: 容错机制
3. **幂等处理**: 支持重复处理
4. **事务性**: 相关事件的处理

### 性能考虑

1. **批量处理**: 减少I/O
2. **异步处理**: 避免阻塞
3. **背压控制**: 防止溢出
4. **缓存策略**: 热数据缓存

---

## 📊 预期成果

### 定量改进

| 指标 | 当前 | 目标 |
|------|------|------|
| 事件持久化 | ❌ | ✅ SQLite |
| 异步处理 | ❌ | ✅ 支持 |
| 事件过滤 | ❌ | ✅ 类型过滤 |
| 监控指标 | 基础 | 完整 |
| 查询能力 | 内存 | SQL |
| 重启恢复 | ❌ | ✅ |

### 定性改进

1. **可靠性**: ⬆️ 显著提升
   - 事件持久化
   - 重启恢复

2. **性能**: ⬆️ 显著提升
   - 异步处理
   - 背压控制

3. **可观测性**: ⬆️ 显著提升
   - 监控指标
   - 性能分析

4. **可维护性**: ⬆️ 提升
   - 查询工具
   - 事件溯源

---

## ⚠️ 风险和缓解

### 主要风险

1. **时间超期**
   - 缓解: 分阶段实施
   - 备选: 只做Phase 1

2. **性能影响**
   - 缓解: 异步处理
   - 缓解: 批量写入

3. **存储增长**
   - 缓解: 定期归档
   - 缓解: 事件TTL

4. **复杂度增加**
   - 缓解: 清晰的API
   - 缓解: 完整文档

---

## 📞 相关资源

### 领域驱动设计

- Domain Events模式
- Event Sourcing模式
- CQRS模式

### Rust工具

- tokio: 异步运行时
- sqlx: SQLite工具
- serde: 序列化
- tracing: 日志和监控

### 项目文档

- DDD架构: vm-core/ARCHITECTURE.md
- 事件系统: vm-core/src/domain_services/events.rs
- 审查报告: docs/VM_COMPREHENSIVE_REVIEW_REPORT.md

---

## 🎯 总结

**当前状态**: 🟢 基础功能完整，26种事件，18处使用

**主要缺失**: 持久化、异步处理、过滤路由、监控指标

**推荐方案**: 先实施Phase 1 (持久化)，快速见效

**预计用时**:
- 最小化: 3-5天 (仅持久化)
- 完整: 3-4周 (所有phases)

**价值**: 提升可靠性、性能和可观测性

---

**创建者**: VM优化团队
**状态**: 📋 分析完成
**下一步**: 等待决策执行哪个方案
**优先级**: P1-9 中等优先级

🚀 **事件总线分析和增强计划已完成！准备执行！**
