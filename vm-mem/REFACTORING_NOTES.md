# vm-mem 模块重构说明

## 文件组织

### 核心MMU实现
- `mmu.rs`: 基础MMU实现（SoftMMU）
- `unified_mmu.rs`: 统一MMU实现，整合多级TLB、并发TLB和页表缓存

### TLB实现
- `tlb.rs`: 基础TLB实现
- `tlb_manager.rs`: 标准TLB管理器
- `tlb_concurrent.rs`: 并发TLB实现（分片锁）
- `tlb_optimized.rs`: 优化的多级TLB实现（MultiLevelTlb）
- `tlb_async.rs`: 异步TLB实现（在vm-core中）

### 优化实现
- ~~`mmu_optimized.rs`~~: 已删除，功能已合并到`unified_mmu.rs`

### 其他
- `page_table_walker.rs`: 页表遍历器（SV39/SV48）
- `numa_allocator.rs`: NUMA感知的内存分配器
- `lockless_optimizations.rs`: 无锁优化

## 使用建议

### 新代码推荐
- **统一使用 `unified_mmu.rs` 中的 `UnifiedMmu`**，它整合了所有优化特性
- 支持策略选择：`MultiLevel`、`Concurrent`、`Hybrid`

### TLB选择
- 基础场景：使用 `tlb_manager.rs` 中的 `StandardTlbManager`
- 高并发场景：使用 `tlb_concurrent.rs` 中的 `ConcurrentTlbManager`
- 高性能场景：使用 `tlb_optimized.rs` 中的 `MultiLevelTlb`
- 异步场景：使用 `tlb_async.rs`（在vm-core中）

## 已完成的重构

1. **合并mmu_optimized和unified_mmu** ✓
   - 已将 `OptimizedSoftMmu` 的功能合并到 `UnifiedMmu`
   - 统一了配置接口（`UnifiedMmuConfig`）
   - 删除了 `mmu_optimized.rs` 文件
   - 更新了所有测试和基准测试使用 `UnifiedMmu`

2. **统一TLB接口** ✓
   - 所有TLB实现现在都实现了 `TlbManager` trait（定义在 `vm-core/src/domain.rs`）
   - `StandardTlbManager` - 已实现 `TlbManager` trait
   - `MultiLevelTlb` - 已实现 `TlbManager` trait
   - `ConcurrentTlbManager` - 通过 `ConcurrentTlbManagerAdapter` 实现 `TlbManager` trait
   - 统一了接口，便于在不同TLB实现之间切换

3. **清理重复代码**
   - 识别并提取公共功能
   - 减少代码重复

