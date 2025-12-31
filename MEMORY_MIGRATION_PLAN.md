# 内存管理统一迁移计划

**执行日期**: 2025-01-01
**状态**: 进行中
**阶段**: P2-1 (统一内存管理架构)

---

## 已完成工作

### P2-1a: 统一MMU接口设计 ✅

**文件**: `vm-mem/src/unified_mmu_v2.rs` (856行)

**核心特性**:
1. **UnifiedMMU trait**: 整合同步和异步接口
   - 同步方法: `translate`, `read`, `write`, `fetch_insn`, `read_bulk`, `write_bulk`
   - 异步方法: `translate_async`, `read_async`, `write_async` 等 (feature-gated)

2. **HybridMMU实现**: 混合同步MMU和异步MMU
   - 使用SoftMmu作为同步实现
   - 使用tokio::task::spawn_blocking实现异步接口
   - 统一的统计信息和配置

3. **统一配置**: `UnifiedMmuConfigV2`
   - 多级TLB配置
   - 并发优化配置
   - 页表缓存配置
   - 预取配置

**导出类型**:
```rust
pub use unified_mmu_v2::{
    HybridMMU,
    UnifiedMMU as UnifiedMMUV2,
    UnifiedMmuConfigV2,
    UnifiedMmuStats as UnifiedMmuStatsV2
};
```

### P2-1b: 统一TLB层次结构 ✅

**文件**: `vm-mem/src/tlb/unified_hierarchy.rs` (598行)

**核心特性**:
1. **UnifiedTlbHierarchy**: 三层TLB层次结构
   - L1: ITLB + DTLB (分离)
   - L2: 统一TLB
   - L3: 共享TLB

2. **层次化查找**: L1 -> L2 -> L3
   - 自动提升：L2命中提升到L1，L3命中提升到L2和L1

3. **AdaptiveTlbManager**: 自适应TLB管理
   - 运行时访问模式分析
   - 动态容量调整

**导出类型**:
```rust
pub use unified_hierarchy::{
    UnifiedTlbHierarchy,
    AdaptiveTlbManager,
    HierarchyStats,
    ReplacementPolicy
};
```

---

## 待删除文件清单（迁移完成后）

### 1. 冗余MMU实现

**文件**:
- `vm-mem/src/async_mmu.rs` (20,662 bytes)
- `vm-mem/src/async_mmu_optimized.rs` (7,066 bytes)

**删除原因**:
- 功能已合并到 `unified_mmu_v2.rs` 中的 `HybridMMU`
- `AsyncMmuWrapper` 被 `HybridMMU` 的异步方法替代

**影响范围**:
- 测试文件: `vm-mem/tests/async_mmu_extended.rs`
- 基准测试: `vm-mem/benches/async_mmu_performance.rs`
- 需要更新这些文件以使用新的 `HybridMMU` 接口

### 2. 冗余TLB实现

**文件**:
- `vm-mem/src/tlb/core/unified.rs` (43,962 bytes)
- `vm-mem/src/tlb/core/concurrent.rs` (23,567 bytes) - 可能保留
- `vm-mem/src/tlb/core/per_cpu.rs` (25,485 bytes) - 可能保留

**删除原因**:
- `unified.rs` 的功能被 `unified_hierarchy.rs` 替代
- `MultiLevelTlb` 被 `UnifiedTlbHierarchy` 替代
- `concurrent.rs` 和 `per_cpu.rs` 可能仍被其他模块使用，需要检查

**保留文件**:
- `vm-mem/src/tlb/core/basic.rs` - 简单实现，仍有用
- `vm-mem/src/tlb/core/lockfree.rs` - 无锁实现，特殊场景需要

### 3. 旧的页表遍历器（如果有）

**需要检查**:
- `vm-mem/src/page_table/sv39_walker.rs`
- `vm-mem/src/page_table/sv48_walker.rs`

**删除原因**:
- 已有通用实现: `vm-mem/src/memory/page_table_walker.rs`
- 包含 `Sv39PageTableWalker` 和 `Sv48PageTableWalker`

---

## 迁移步骤

### 步骤1: 更新vm-core

**目标文件**: `vm-core/src/lib.rs`, `vm-core/src/vm_state.rs`

**更改**:
```rust
// Old:
use vm_mem::{AsyncMMU, UnifiedMmu};

// New:
use vm_mem::{HybridMMU, UnifiedMMU as UnifiedMMUV2};
```

**影响**:
- `ExecutionEngine` trait 的MMU类型
- `VmState` 中的MMU字段

### 步骤2: 更新vm-engine

**目标文件**: `vm-engine/src/interpreter.rs`

**更改**:
```rust
// Old:
use vm_mem::async_mmu::AsyncMMU;

// New:
use vm_mem::HybridMMU;
```

### 步骤3: 更新vm-engine-jit

**目标文件**: `vm-engine-jit/src/lib.rs`

**更改**:
```rust
// Old:
use vm_mem::async_mmu::AsyncMmuWrapper;

// New:
use vm_mem::HybridMMU;
```

### 步骤4: 更新测试文件

**目标文件**:
- `vm-mem/tests/async_mmu_extended.rs`
- `vm-mem/benches/async_mmu_performance.rs`

**更改**:
```rust
// Old:
use vm_mem::async_mmu::async_impl::AsyncMMU;

// New:
use vm_mem::{HybridMMU, UnifiedMMU as UnifiedMMUV2};
```

### 步骤5: 全面验证

**命令序列**:
```bash
# 1. 重新编译workspace
cargo build --workspace --all-features 2>&1 | tee build.log

# 2. 检查编译错误
grep "error" build.log

# 3. 运行完整测试套件
cargo test --workspace --all-features 2>&1 | tee test.log

# 4. 检查测试失败
grep "FAILED" test.log

# 5. 性能基准测试
cargo bench --workspace -- --save-baseline after_migration

# 6. 对比重构前后
cargo bench --workspace -- --baseline before_migration
```

---

## 风险缓解

### 高风险: 删除async_mmu.rs

**缓解措施**:
1. 保留 `legacy_async_mmu` feature flag作为回退
2. 6个月并行运行期（旧代码仍可编译）
3. 每日性能基准对比

**回滚计划**:
```bash
# 如果出现严重问题
git revert <commit-hash>
# 或使用feature flag
cargo build --workspace --features "legacy_async_mmu"
```

### 中风险: 删除unified.rs (TLB)

**缓解措施**:
1. 保留旧的 `MultiLevelTlb` 类型别名
2. 逐步迁移测试用例

**检查清单**:
- [ ] 确认没有外部crate依赖 `vm-mem::tlb::core::unified`
- [ ] 更新所有内部使用点
- [ ] 运行完整测试套件

---

## 代码行数对比

### 删除前
```bash
cloc vm-mem/src/
# Rust: ~15,000行 (估计)
```

### 删除后
```bash
cloc vm-mem/src/
# Rust: ~10,000行 (估计，减少33%)
```

**预期收益**:
- 代码行数减少: ~5,000行 (33%)
- 编译时间减少: 5-10%
- 维护成本降低: 统一接口，减少重复代码

---

## 时间表

| 任务 | 预计时间 | 状态 |
|------|----------|------|
| P2-1a: 统一MMU接口设计 | 4天 | ✅ 完成 |
| P2-1b: 统一TLB层次结构 | 5天 | ✅ 完成 |
| P2-1c: 删除冗余实现 | 4天 | 🔄 进行中 |
| P2-1d: 迁移和验证 | 4天 | ⏳ 待开始 |

**总计**: 17天

---

## 下一步行动

1. ✅ 创建统一MMU接口 (`unified_mmu_v2.rs`)
2. ✅ 创建统一TLB层次 (`unified_hierarchy.rs`)
3. 🔄 **当前**: 创建迁移计划（本文档）
4. ⏳ 更新vm-core使用新接口
5. ⏳ 更新vm-engine使用新接口
6. ⏳ 更新测试和基准
7. ⏳ 全面验证
8. ⏳ 删除冗余文件（在验证通过后）

---

**创建日期**: 2025-01-01
**最后更新**: 2025-01-01
**负责人**: Claude (AI Assistant)
**审查人**: 待定
