# DDD 架构清晰化文档

本文档描述了如何将技术子系统从领域层移至基础设施层，以符合领域驱动设计（DDD）原则。

**最后更新**: 2024年（基于现代化升级计划）

## 目录

1. [问题分析](#问题分析)
2. [DDD 分层原则](#ddd-分层原则)
3. [当前架构问题](#当前架构问题)
4. [重构方案](#重构方案)
5. [迁移计划](#迁移计划)
6. [接口设计](#接口设计)
7. [实施指南](#实施指南)

---

## 问题分析

### 当前状态

在 `vm-core/src/domain_services/` 中存在大量技术实现细节，这些应该属于基础设施层：

**技术子系统（应移至基础设施层）**:
- `tlb_management_service.rs` - TLB 管理实现
- `cache_management_service.rs` - 缓存管理实现
- `optimization_pipeline_service.rs` - 优化管道实现
- `register_allocation_service.rs` - 寄存器分配实现
- `page_table_walker_service.rs` - 页表遍历实现
- `performance_optimization_service.rs` - 性能优化实现

**领域服务（应保留在领域层）**:
- `vm_lifecycle_service.rs` - VM 生命周期管理（业务逻辑）
- `translation_strategy_service.rs` - 翻译策略选择（业务规则）
- `cross_architecture_translation_service.rs` - 跨架构翻译协调（业务逻辑）
- `architecture_compatibility_service.rs` - 架构兼容性检查（业务规则）

### 问题影响

1. **领域层污染**: 技术实现细节混入领域层，违反 DDD 原则
2. **依赖混乱**: 领域层依赖具体技术实现，难以测试和替换
3. **职责不清**: 领域服务和基础设施服务职责重叠
4. **可维护性差**: 技术变更影响领域层，增加维护成本

---

## DDD 分层原则

### 领域层 (Domain Layer)

**职责**:
- 定义业务概念和规则
- 封装业务逻辑
- 定义领域事件
- 提供领域服务接口（trait）

**不应包含**:
- 具体技术实现
- 基础设施细节
- 平台特定代码
- 性能优化实现

### 基础设施层 (Infrastructure Layer)

**职责**:
- 实现领域层定义的接口
- 提供技术实现（TLB、缓存、优化器等）
- 处理平台差异
- 性能优化实现

**可以包含**:
- TLB 管理实现
- 缓存管理实现
- 优化器实现
- 寄存器分配实现

---

## 当前架构问题

### 问题 1: TLB 管理服务

**当前位置**: `vm-core/src/domain_services/tlb_management_service.rs`

**问题**:
- 包含具体的 TLB 实现细节（LRU、LFU 等替换策略）
- 包含 TLB 条目管理逻辑
- 这是基础设施实现，不是领域逻辑

**应该**:
- 领域层定义 `TlbManager` trait（已存在）
- 基础设施层实现具体的 TLB 管理（在 `vm-mem` 中）

### 问题 2: 缓存管理服务

**当前位置**: `vm-core/src/domain_services/cache_management_service.rs`

**问题**:
- 包含具体的缓存实现（LRU、LFU 等）
- 包含缓存层级管理
- 这是基础设施实现

**应该**:
- 领域层定义缓存策略接口
- 基础设施层实现具体缓存（在 `vm-engine` 或 `vm-mem` 中）

### 问题 3: 优化管道服务

**当前位置**: `vm-core/src/domain_services/optimization_pipeline_service.rs`

**问题**:
- 包含具体的优化阶段实现
- 包含优化器协调逻辑
- 这是基础设施实现

**应该**:
- 领域层定义优化策略接口
- 基础设施层实现具体优化器（在 `vm-engine` 或 `vm-optimizers` 中）

### 问题 4: 寄存器分配服务

**当前位置**: `vm-core/src/domain_services/register_allocation_service.rs`

**问题**:
- 包含具体的寄存器分配算法
- 包含图着色、线性扫描等实现
- 这是基础设施实现

**应该**:
- 领域层定义寄存器分配策略接口
- 基础设施层实现具体算法（在 `vm-engine` 中）

### 问题 5: 页表遍历服务

**当前位置**: `vm-core/src/domain_services/page_table_walker_service.rs`

**问题**:
- 包含具体的页表遍历实现
- 包含页表条目解析逻辑
- 这是基础设施实现

**应该**:
- 领域层定义 `PageTableWalker` trait（已存在）
- 基础设施层实现具体遍历（在 `vm-mem` 中）

---

## 重构方案

### 方案概述

1. **保留领域接口**: 在 `vm-core/src/domain/` 中定义 trait
2. **移动实现**: 将具体实现移至相应的基础设施 crate
3. **更新依赖**: 更新 crate 间的依赖关系
4. **保持兼容**: 通过 re-export 保持向后兼容

### 详细方案

#### 1. TLB 管理

**领域层** (`vm-core/src/domain.rs`):
```rust
// 已存在
pub trait TlbManager: Send + Sync {
    fn lookup(&self, addr: GuestAddr, asid: u16, access: AccessType) -> Option<TlbEntry>;
    fn update(&mut self, entry: TlbEntry);
    fn invalidate(&mut self, addr: GuestAddr, asid: u16);
    // ...
}
```

**基础设施层** (`vm-mem/src/tlb/`):
```rust
// 具体实现
pub struct MultiLevelTlb { ... }
impl TlbManager for MultiLevelTlb { ... }
```

**迁移**:
- 将 `TlbManagementDomainService` 的实现移至 `vm-mem`
- 保留领域层的 trait 定义
- 通过 DI 容器注入具体实现

#### 2. 缓存管理

**领域层** (`vm-core/src/domain.rs`):
```rust
pub trait CacheManager<K, V>: Send + Sync {
    fn get(&self, key: &K) -> Option<V>;
    fn put(&mut self, key: K, value: V);
    fn evict(&mut self, key: &K);
    fn clear(&mut self);
}
```

**基础设施层** (`vm-engine/src/jit/cache/`):
```rust
// 具体实现
pub struct LruCache<K, V> { ... }
impl CacheManager<K, V> for LruCache<K, V> { ... }
```

**迁移**:
- 将 `CacheManagementDomainService` 的实现移至 `vm-engine`
- 定义领域层的 trait
- 通过工厂模式创建具体实现

#### 3. 优化管道

**领域层** (`vm-core/src/domain.rs`):
```rust
pub trait OptimizationStrategy: Send + Sync {
    fn optimize(&self, ir: &IRBlock) -> VmResult<IRBlock>;
    fn get_optimization_level(&self) -> u32;
}
```

**基础设施层** (`vm-engine/src/jit/optimizer/`):
```rust
// 具体实现
pub struct OptimizationPipeline { ... }
impl OptimizationStrategy for OptimizationPipeline { ... }
```

**迁移**:
- 将 `OptimizationPipelineDomainService` 的实现移至 `vm-engine`
- 定义领域层的策略接口
- 通过策略模式选择优化器

#### 4. 寄存器分配

**领域层** (`vm-core/src/domain.rs`):
```rust
pub trait RegisterAllocator: Send + Sync {
    fn allocate(&mut self, ir: &IRBlock) -> VmResult<RegisterMapping>;
}
```

**基础设施层** (`vm-engine/src/jit/register_allocator/`):
```rust
// 具体实现
pub struct GraphColoringAllocator { ... }
pub struct LinearScanAllocator { ... }
impl RegisterAllocator for GraphColoringAllocator { ... }
```

**迁移**:
- 将 `RegisterAllocationDomainService` 的实现移至 `vm-engine`
- 定义领域层的 trait
- 通过工厂模式创建分配器

#### 5. 页表遍历

**领域层** (`vm-core/src/domain.rs`):
```rust
// 已存在
pub trait PageTableWalker: Send + Sync {
    fn walk(&self, addr: GuestAddr, mmu: &dyn MMU) -> VmResult<GuestPhysAddr>;
}
```

**基础设施层** (`vm-mem/src/mmu/`):
```rust
// 具体实现
pub struct SoftPageTableWalker { ... }
impl PageTableWalker for SoftPageTableWalker { ... }
```

**迁移**:
- 将 `PageTableWalkerDomainService` 的实现移至 `vm-mem`
- 保留领域层的 trait 定义
- 通过 DI 容器注入具体实现

---

## 迁移计划

### 阶段 1: 接口定义 ✅ (已完成)

**目标**: 在领域层定义清晰的 trait 接口

**状态**:
- ✅ `TlbManager` trait 已定义
- ✅ `PageTableWalker` trait 已定义
- ⚠️ `CacheManager` trait 需要定义
- ⚠️ `OptimizationStrategy` trait 需要定义
- ⚠️ `RegisterAllocator` trait 需要定义

### 阶段 2: 实现迁移 ⚠️ (进行中)

**目标**: 将具体实现移至基础设施层

**待迁移**:
- [ ] TLB 管理实现 → `vm-mem/src/tlb/`
- [ ] 缓存管理实现 → `vm-engine/src/jit/cache/`
- [ ] 优化管道实现 → `vm-engine/src/jit/optimizer/`
- [ ] 寄存器分配实现 → `vm-engine/src/jit/register_allocator/`
- [ ] 页表遍历实现 → `vm-mem/src/mmu/`

### 阶段 3: 依赖更新 ⚠️ (待开始)

**目标**: 更新 crate 依赖关系

**待更新**:
- [ ] `vm-service` 依赖基础设施实现而非领域服务
- [ ] `vm-engine` 实现领域层定义的 trait
- [ ] `vm-mem` 实现领域层定义的 trait
- [ ] 移除领域层对基础设施的依赖

### 阶段 4: 清理和文档 ⚠️ (待开始)

**目标**: 清理遗留代码，更新文档

**待完成**:
- [ ] 移除 `domain_services/` 中的技术实现
- [ ] 更新架构文档
- [ ] 更新 API 文档
- [ ] 添加迁移指南

---

## 接口设计

### 领域层接口（Trait）

#### CacheManager

```rust
// vm-core/src/domain.rs

/// 缓存管理接口
pub trait CacheManager<K, V>: Send + Sync {
    /// 获取缓存值
    fn get(&self, key: &K) -> Option<V>;

    /// 插入缓存值
    fn put(&mut self, key: K, value: V);

    /// 移除缓存值
    fn evict(&mut self, key: &K);

    /// 清空缓存
    fn clear(&mut self);

    /// 获取缓存统计
    fn stats(&self) -> CacheStats;
}

/// 缓存统计
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub size: usize,
    pub capacity: usize,
}
```

#### OptimizationStrategy

```rust
// vm-core/src/domain.rs

/// 优化策略接口
pub trait OptimizationStrategy: Send + Sync {
    /// 优化 IR 块
    fn optimize(&self, ir: &IRBlock) -> VmResult<IRBlock>;

    /// 获取优化级别
    fn optimization_level(&self) -> u32;

    /// 是否支持特定优化
    fn supports_optimization(&self, opt: OptimizationType) -> bool;
}

/// 优化类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationType {
    ConstantFolding,
    DeadCodeElimination,
    InstructionCombining,
    LoopOptimization,
}
```

#### RegisterAllocator

```rust
// vm-core/src/domain.rs

/// 寄存器分配器接口
pub trait RegisterAllocator: Send + Sync {
    /// 分配寄存器
    fn allocate(&mut self, ir: &IRBlock) -> VmResult<RegisterMapping>;

    /// 获取分配统计
    fn stats(&self) -> RegisterAllocationStats;
}

/// 寄存器映射
#[derive(Debug, Clone)]
pub struct RegisterMapping {
    pub virtual_to_physical: HashMap<RegId, PhysicalReg>,
    pub spills: Vec<SpillSlot>,
}

/// 分配统计
#[derive(Debug, Clone)]
pub struct RegisterAllocationStats {
    pub total_allocations: usize,
    pub spills: usize,
    pub physical_regs_used: usize,
}
```

---

## 实施指南

### 步骤 1: 定义领域接口

在 `vm-core/src/domain.rs` 中添加新的 trait 定义：

```rust
// 添加到 vm-core/src/domain.rs

pub trait CacheManager<K, V>: Send + Sync { ... }
pub trait OptimizationStrategy: Send + Sync { ... }
pub trait RegisterAllocator: Send + Sync { ... }
```

### 步骤 2: 创建基础设施实现

在相应的基础设施 crate 中创建实现：

```rust
// vm-engine/src/jit/cache/mod.rs
pub struct LruCache<K, V> { ... }
impl CacheManager<K, V> for LruCache<K, V> { ... }

// vm-engine/src/jit/optimizer/mod.rs
pub struct OptimizationPipeline { ... }
impl OptimizationStrategy for OptimizationPipeline { ... }

// vm-engine/src/jit/register_allocator/mod.rs
pub struct GraphColoringAllocator { ... }
impl RegisterAllocator for GraphColoringAllocator { ... }
```

### 步骤 3: 更新领域服务

将领域服务改为使用 trait 而非具体实现：

```rust
// vm-core/src/domain_services/optimization_pipeline_service.rs

// 之前：直接使用具体实现
pub struct OptimizationPipelineDomainService {
    pipeline: OptimizationPipeline,  // ❌ 具体实现
}

// 之后：使用 trait
pub struct OptimizationPipelineDomainService {
    strategy: Arc<dyn OptimizationStrategy>,  // ✅ trait
}
```

### 步骤 4: 通过 DI 注入

使用依赖注入容器注入具体实现：

```rust
// vm-service/src/lib.rs

let di_container = DIContainer::new();
di_container.register::<dyn OptimizationStrategy, OptimizationPipeline>();
di_container.register::<dyn RegisterAllocator, GraphColoringAllocator>();
di_container.register::<dyn CacheManager<usize, IRBlock>, LruCache<usize, IRBlock>>();
```

### 步骤 5: 迁移实现代码

将 `domain_services/` 中的实现代码移至基础设施层：

```bash
# 示例：迁移 TLB 管理
mv vm-core/src/domain_services/tlb_management_service.rs \
   vm-mem/src/tlb/management.rs

# 更新导入
# vm-mem/src/tlb/mod.rs
pub mod management;
pub use management::TlbManagementService;
```

### 步骤 6: 更新文档

更新相关文档：
- 架构文档
- API 文档
- 迁移指南

---

## 领域服务 vs 基础设施服务

### 领域服务（保留在领域层）

**特征**:
- 封装业务规则
- 协调多个聚合
- 无状态操作
- 使用领域语言

**示例**:
- `VmLifecycleDomainService` - VM 生命周期管理
- `TranslationStrategyDomainService` - 翻译策略选择
- `CrossArchitectureTranslationDomainService` - 跨架构翻译协调
- `ArchitectureCompatibilityDomainService` - 架构兼容性检查

### 基础设施服务（移至基础设施层）

**特征**:
- 技术实现细节
- 性能优化
- 平台特定代码
- 可替换实现

**示例**:
- `TlbManagementService` → `vm-mem/src/tlb/`
- `CacheManagementService` → `vm-engine/src/jit/cache/`
- `OptimizationPipelineService` → `vm-engine/src/jit/optimizer/`
- `RegisterAllocationService` → `vm-engine/src/jit/register_allocator/`
- `PageTableWalkerService` → `vm-mem/src/mmu/`

---

## 重构检查清单

### 领域层检查

- [ ] 只包含业务逻辑和规则
- [ ] 定义清晰的 trait 接口
- [ ] 不依赖具体技术实现
- [ ] 使用领域语言命名
- [ ] 无状态服务

### 基础设施层检查

- [ ] 实现领域层定义的 trait
- [ ] 包含具体技术实现
- [ ] 可替换实现
- [ ] 性能优化代码
- [ ] 平台特定代码

### 依赖关系检查

- [ ] 领域层不依赖基础设施层
- [ ] 基础设施层实现领域层接口
- [ ] 应用层通过 DI 注入实现
- [ ] 依赖方向正确（上层 → 下层）

---

## 迁移示例

### 示例 1: TLB 管理服务迁移

**迁移前** (`vm-core/src/domain_services/tlb_management_service.rs`):
```rust
pub struct TlbManagementDomainService {
    tlb: HashMap<u64, TlbManagedEntry>,
    policy: TlbReplacementPolicy,
    // 具体实现细节
}
```

**迁移后**:

**领域层** (`vm-core/src/domain.rs`):
```rust
pub trait TlbManager: Send + Sync {
    fn lookup(&self, addr: GuestAddr, asid: u16, access: AccessType) -> Option<TlbEntry>;
    fn update(&mut self, entry: TlbEntry);
    // ...
}
```

**基础设施层** (`vm-mem/src/tlb/management.rs`):
```rust
pub struct TlbManagementService {
    tlb: HashMap<u64, TlbManagedEntry>,
    policy: TlbReplacementPolicy,
    // 具体实现
}

impl TlbManager for TlbManagementService {
    // 实现 trait
}
```

**领域服务** (`vm-core/src/domain_services/tlb_management_service.rs`):
```rust
pub struct TlbManagementDomainService {
    tlb_manager: Arc<dyn TlbManager>,  // 使用 trait
    event_bus: Arc<dyn DomainEventBus>,
}

impl TlbManagementDomainService {
    pub fn new(tlb_manager: Arc<dyn TlbManager>) -> Self {
        // 协调业务逻辑，不包含具体实现
    }
}
```

---

## 向后兼容

### Re-export 策略

为了保持向后兼容，可以在 `vm-core/src/lib.rs` 中 re-export：

```rust
// vm-core/src/lib.rs

// Re-export 领域接口
pub use domain::{TlbManager, PageTableWalker, CacheManager, OptimizationStrategy, RegisterAllocator};

// Re-export 领域服务（现在使用 trait）
pub use domain_services::{
    TlbManagementDomainService,
    CacheManagementDomainService,
    OptimizationPipelineDomainService,
    // ...
};
```

### 迁移路径

1. **阶段 1**: 添加 trait 定义，保持现有服务
2. **阶段 2**: 实现基础设施服务，通过 DI 注入
3. **阶段 3**: 更新领域服务使用 trait
4. **阶段 4**: 移除领域服务中的具体实现

---

## 参考

- [DDD 分层架构](./architecture/SYSTEM_OVERVIEW.md)
- [充血模型 ADR](./architecture/adr/ADR-004-rich-domain-model.md)
- [Feature Contract 文档](./FEATURE_CONTRACT.md)
- [现代化升级报告](./MODERNIZATION_SUMMARY.md)

---

**文档维护者**: VM 项目团队
**最后审查**: 2024年现代化升级计划
