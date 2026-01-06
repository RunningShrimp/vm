# 快速开始测试实施 - 2026-01-06

**目标**: 从 62.39% 提升到 80%+ 覆盖率
**预计时间**: 60-81小时 (2-3周)
**当前状态**: ✅ 分析完成，准备开始

---

## 🚀 立即开始 - Top 5 高ROI测试

预计 **8-12小时**，提升 **~5-6%** 覆盖率

### 1. error.rs - 错误处理测试 (2-3小时)

**文件**: `vm-core/src/error.rs`
**当前覆盖率**: 0% (413行未覆盖)
**目标覆盖率**: 80%

```bash
# 1. 打开文件
vim vm-core/src/error.rs

# 2. 添加测试模块
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_error_variants() {
        // 测试每个错误变体能正确创建
        let error = VMError::Memory(MemoryError::OutOfMemory {
            requested: 1024,
            available: 512,
        });
        assert!(matches!(error, VMError::Memory(_)));
    }

    #[test]
    fn test_error_severity_levels() {
        // 测试不同严重级别
        let fatal = VMError::Fatal("System failure".to_string());
        assert_eq!(fatal.severity(), Severity::Fatal);
    }

    #[test]
    fn test_recoverable_vs_unrecoverable() {
        // 测试可恢复vs不可恢复
    }

    #[test]
    fn test_error_display_and_format() {
        // 测试错误显示格式
    }
}

# 3. 运行测试
cargo test --package vm-core --lib error

# 4. 验证通过
```

### 2. domain.rs - 领域模式测试 (1-2小时)

**文件**: `vm-core/src/domain.rs`
**当前覆盖率**: 0% (6行未覆盖)
**目标覆盖率**: 90%

```bash
# 1. 打开文件
vim vm-core/src/domain.rs

# 2. 添加测试
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_id_generation() {
        let id = DomainId::new();
        assert!(id.value() > 0);
    }

    #[test]
    fn test_domain_event_sequence() {
        // 测试事件序列号
    }

    #[test]
    fn test_domain_timestamp() {
        // 测试时间戳
    }
}

# 3. 运行测试
cargo test --package vm-core --lib domain
```

### 3. vm_state.rs - VM状态测试 (2-3小时)

**文件**: `vm-core/src/vm_state.rs`
**当前覆盖率**: 0% (43行未覆盖)
**目标覆盖率**: 75%

```bash
# 1. 打开文件
vim vm-core/src/vm_state.rs

# 2. 添加测试
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_state_creation() {
        let state = VMState::new();
        assert_eq!(state.status(), VMStatus::Stopped);
    }

    #[test]
    fn test_valid_state_transitions() {
        let mut state = VMState::new();

        // Stopped → Running
        assert!(state.transition_to(VMStatus::Running).is_ok());

        // Running → Paused
        assert!(state.transition_to(VMStatus::Paused).is_ok());

        // Paused → Running
        assert!(state.transition_to(VMStatus::Running).is_ok());

        // Running → Stopped
        assert!(state.transition_to(VMStatus::Stopped).is_ok());
    }

    #[test]
    fn test_invalid_state_transitions() {
        // 测试无效转换被拒绝
    }

    #[test]
    fn test_state_serialization() {
        // 测试序列化/反序列化
    }
}

# 3. 运行测试
cargo test --package vm-core --lib vm_state
```

### 4. runtime/resources.rs - 资源管理测试 (2-3小时)

**文件**: `vm-core/src/runtime/resources.rs`
**当前覆盖率**: 0% (111行未覆盖)
**目标覆盖率**: 70%

```bash
# 1. 打开文件
vim vm-core/src/runtime/resources.rs

# 2. 添加测试
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_pool_creation() {
        let pool = ResourcePool::new(100);
        assert_eq!(pool.capacity(), 100);
    }

    #[test]
    fn test_resource_allocation() {
        let mut pool = ResourcePool::new(100);

        let resource = pool.allocate(10).unwrap();
        assert_eq!(pool.available(), 90);
    }

    #[test]
    fn test_resource_release() {
        // 测试资源释放
    }

    #[test]
    fn test_resource_limits() {
        // 测试资源限制
    }

    #[test]
    fn test_resource_monitoring() {
        // 测试资源监控
    }
}

# 3. 运行测试
cargo test --package vm-core --lib resources
```

### 5. mmu_traits.rs - MMU trait测试 (2-3小时)

**文件**: `vm-core/src/mmu_traits.rs`
**当前覆盖率**: 0% (30行未覆盖)
**目标覆盖率**: 70%

```bash
# 1. 打开文件
vim vm-core/src/mmu_traits.rs

# 2. 添加测试
#[cfg(test)]
mod tests {
    use super::*;

    struct MockMMU;

    impl MMU for MockMMU {
        fn translate(&self, addr: u64) -> Result<u64, MMUError> {
            Ok(addr)
        }

        fn check_permissions(&self, addr: u64, perm: Permissions) -> Result<bool, MMUError> {
            Ok(true)
        }
    }

    #[test]
    fn test_mmu_trait_implementation() {
        let mmu = MockMMU;

        let translated = mmu.translate(0x1000).unwrap();
        assert_eq!(translated, 0x1000);
    }

    #[test]
    fn test_address_translation() {
        // 测试地址转换
    }

    #[test]
    fn test_permission_checking() {
        // 测试权限检查
    }

    #[test]
    fn test_tlb_operations() {
        // 测试TLB操作
    }
}

# 3. 运行测试
cargo test --package vm-core --lib mmu
```

---

## 📊 验证进度

### 运行所有新测试

```bash
# 运行vm-core所有测试
cargo test --package vm-core --lib

# 应该看到:
# running 364 tests (359现有 + 5新增)
# test result: ok. 364 passed; 0 failed
```

### 生成新覆盖率报告

```bash
# 生成覆盖率报告
cargo llvm-cov --package vm-core --html --output-dir target/llvm-cov/vm-core-after-phase1

# 查看报告
open target/llvm-cov/vm-core-after-phase1/html/index.html
```

### 检查覆盖率提升

```bash
# 查看覆盖率统计
cargo llvm-cov report --package vm-core

# 预期看到:
# TOTAL: 65.39% (从62.39%提升3%)
```

---

## 📋 每日检查清单

### Day 1-2: error.rs + domain.rs

- [ ] error.rs测试完成 (413行)
  - [ ] 测试所有错误变体
  - [ ] 测试错误严重级别
  - [ ] 测试错误上下文
  - [ ] 测试错误显示
- [ ] domain.rs测试完成 (6行)
  - [ ] 测试领域ID生成
  - [ ] 测试事件序列
  - [ ] 测试时间戳
- [ ] 测试全部通过
- [ ] 生成覆盖率报告
- [ ] 验证覆盖率提升

**预期**: 覆盖率 62.39% → ~64%

### Day 3-4: vm_state.rs + runtime/resources.rs

- [ ] vm_state.rs测试完成 (43行)
  - [ ] 测试状态创建
  - [ ] 测试有效转换
  - [ ] 测试无效转换
  - [ ] 测试序列化
- [ ] runtime/resources.rs测试完成 (111行)
  - [ ] 测试资源池
  - [ ] 测试资源分配
  - [ ] 测试资源释放
  - [ ] 测试资源限制
  - [ ] 测试资源监控
- [ ] 测试全部通过
- [ ] 生成覆盖率报告
- [ ] 验证覆盖率提升

**预期**: 覆盖率 ~64% → ~65.5%

### Day 5: mmu_traits.rs + 收尾

- [ ] mmu_traits.rs测试完成 (30行)
  - [ ] 测试trait实现
  - [ ] 测试地址转换
  - [ ] 测试权限检查
  - [ ] 测试TLB操作
- [ ] 所有测试通过
- [ ] Phase 1完成报告
- [ ] 生成最终覆盖率报告
- [ ] 验证达到65%+

**预期**: 覆盖率 ~65.5% → **65.39%+** (Phase 1目标达成!)

---

## 🎯 Phase 1 完成标准

### 定量指标

- ✅ 覆盖率从 62.39% → 65.39%+ (+3%)
- ✅ 新增测试 ~30-50个
- ✅ 5个核心文件从0% → 70%+

### 定性指标

- ✅ 所有P0核心文件有基础测试
- ✅ 所有错误变体有测试
- ✅ VM状态转换完整测试
- ✅ 资源管理功能测试
- ✅ MMU trait实现测试

### 交付物

- ✅ 5个文件的新测试代码
- ✅ 新的覆盖率报告 (65.39%+)
- ✅ Phase 1完成总结文档

---

## 📞 需要帮助?

### 查看覆盖率报告

```bash
# macOS
open target/llvm-cov/vm-core/html/index.html

# 查看特定文件
open target/llvm-cov/vm-core/html/src/文件路径.html
```

### 查看详细分析

```bash
# 查看覆盖率缺口分析
cat docs/COVERAGE_GAP_ANALYSIS_2026_01_06.md

# 查看测试计划
cat docs/COVERAGE_ANALYSIS_SESSION_SUMMARY_2026_01_06.md
```

### 运行特定测试

```bash
# 运行单个测试
cargo test --package vm-core --lib test_error_variants

# 运行带输出
cargo test --package vm-core --lib -- --nocapture

# 运行并显示详细输出
cargo test --package vm-core --lib -- --show-output
```

---

## 🎓 成功提示

1. **一次只做一个文件**: 专注完成一个文件再开始下一个
2. **先运行测试**: 确保现有测试都通过再添加新测试
3. **小步提交**: 每完成一个文件的测试就提交一次
4. **查看覆盖率**: 每天生成覆盖率报告查看进展
5. **保持节奏**: 每天2-3小时，5天完成Phase 1

---

## 📊 进度跟踪

### 里程碑

- [ ] **Milestone 1.1**: error.rs完成 - 覆盖率 ~63.5% (Day 1-2)
- [ ] **Milestone 1.2**: domain.rs完成 - 覆盖率 ~64% (Day 2)
- [ ] **Milestone 1.3**: vm_state.rs完成 - 覆盖率 ~64.5% (Day 3)
- [ ] **Milestone 1.4**: runtime/resources.rs完成 - 覆盖率 ~65.5% (Day 4)
- [ ] **Milestone 1.5**: mmu_traits.rs完成 - 覆盖率 65.39%+ (Day 5)
- [ ] **Phase 1 完成**: 所有P0核心文件测试完成 ✨

---

**开始时间**: 2026-01-06
**预计完成**: 2026-01-10或之前 (5天)
**工作量**: 8-12小时
**覆盖率提升**: +3% (62.39% → 65.39%+)

🚀 **准备开始！Good luck with testing!** 🎯
