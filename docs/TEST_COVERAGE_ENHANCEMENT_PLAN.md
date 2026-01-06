# 测试覆盖率提升计划

**创建日期**: 2026-01-06
**任务**: P1-10 - 提升测试覆盖率至80%+
**状态**: 📋 规划中

---

## 📊 当前状态评估

### 工具可用性 ✅

```bash
$ cargo llvm-cov --version
cargo-llvm-cov 0.6.22
```

**状态**: ✅ 工具已安装，可以生成覆盖率报告

---

## 🎯 目标设定

### 总体目标

| 指标 | 当前 | 目标 | 提升 |
|------|------|------|------|
| 整体覆盖率 | 待评估 | 80%+ | +?% |
| 行覆盖率 | 待评估 | 85%+ | +?% |
| 区域覆盖率 | 待评估 | 75%+ | +?% |
| 函数覆盖率 | 待评估 | 90%+ | +?% |

### 分crate目标

| Crate | 当前 | 目标 | 优先级 |
|-------|------|------|--------|
| vm-core | 待评估 | 85%+ | P0 |
| vm-engine-jit | 待评估 | 80%+ | P0 |
| vm-mem | 待评估 | 85%+ | P0 |
| vm-device | 待评估 | 75%+ | P1 |
| vm-ir | 待评估 | 80%+ | P1 |
| vm-accel | 待评估 | 70%+ | P2 |

---

## 📋 执行计划

### Phase 1: 评估当前覆盖率 (0.5天)

**任务**:
1. ✅ 检查工具可用性
2. ⏳ 生成workspace级别覆盖率报告
3. ⏳ 识别覆盖率最低的crate
4. ⏳ 识别测试盲点
5. ⏳ 生成详细报告

**输出**:
- 覆盖率HTML报告
- 各crate覆盖率统计
- 测试盲点列表

---

### Phase 2: 优先级分析 (0.5天)

**任务**:
1. 根据覆盖率排序crate
2. 识别关键路径代码
3. 评估测试缺失类型:
   - 单元测试缺失
   - 集成测试缺失
   - 边界条件缺失
   - 错误处理缺失
4. 制定测试策略

**输出**:
- 测试优先级列表
- 测试策略文档
- 测试用例清单

---

### Phase 3: 核心crate测试提升 (1-2周)

#### vm-core (目标: 85%+)

**关键模块**:
- `src/domain_services/` - 领域服务
- `src/di/` - 依赖注入
- `src/error.rs` - 错误处理
- `src/migration.rs` - 迁移管理

**测试需求**:
1. 领域服务单元测试
2. DI容器集成测试
3. 错误处理边界测试
4. 迁移流程测试

**预计用时**: 3-4天

---

#### vm-engine-jit (目标: 80%+)

**关键模块**:
- `src/jit/` - JIT编译核心
- `src/executor/` - 执行器
- `src/jit_advanced/` - 高级JIT
- SIMD集成代码

**测试需求**:
1. JIT编译流程测试
2. 代码生成测试
3. SIMD操作测试
4. 热点检测测试
5. 优化器测试

**预计用时**: 4-5天

---

#### vm-mem (目标: 85%+)

**关键模块**:
- `src/tlb/` - TLB管理
- `src/memory/` - 内存管理
- `src/unified_mmu_v2.rs` - MMU实现
- `src/simd_memcpy.rs` - SIMD内存拷贝

**测试需求**:
1. TLB管理测试
2. 内存分配测试
3. MMU翻译测试
4. NUMA测试
5. SIMD内存操作测试

**预计用时**: 3-4天

---

### Phase 4: 次要crate测试提升 (1周)

#### vm-device, vm-ir, vm-accel

**任务**:
1. 设备模拟测试
2. IR操作测试
3. 加速器集成测试

**预计用时**: 5-7天

---

### Phase 5: 集成测试增强 (3-5天)

**任务**:
1. 跨crate集成测试
2. 端到端测试
3. 性能回归测试
4. 压力测试

**预计用时**: 3-5天

---

## 📊 预期成果

### 定量目标

| 指标 | 当前 | 目标 |
|------|------|------|
| 整体覆盖率 | ?% | 80%+ |
| vm-core | ?% | 85%+ |
| vm-engine-jit | ?% | 80%+ |
| vm-mem | ?% | 85%+ |
| 测试数量 | ? | +500+ |
| 测试行数 | ? | +5000+ |

### 定性目标

1. **代码质量**: ⬆️ 显著提升
   - 减少bug率
   - 提高重构信心
   - 改善API设计

2. **文档价值**: ⬆️ 提升
   - 测试即文档
   - 使用示例清晰
   - 边界条件明确

3. **维护性**: ⬆️ 显著提升
   - 快速定位问题
   - 安全重构
   - 持续集成友好

---

## 🔧 测试策略

### 单元测试原则

1. **测试隔离**: 每个测试独立
2. **快速执行**: 单元测试<1ms
3. **清晰命名**: 测试名描述意图
4. **AAA模式**: Arrange-Act-Assert
5. **边界覆盖**: 正常、边界、异常

### 集成测试原则

1. **真实场景**: 模拟实际使用
2. **跨模块**: 验证模块协作
3. **端到端**: 完整流程验证
4. **性能验证**: 关键路径性能

### 测试组织

```
vm-core/tests/
├── unit/           # 单元测试
│   ├── domain_services/
│   ├── di/
│   └── error/
├── integration/    # 集成测试
│   ├── services/
│   └── migrations/
└── e2e/           # 端到端测试
    └── workflows/
```

---

## 📈 进度跟踪

### 里程碑

| 里程碑 | 目标日期 | 状态 |
|--------|---------|------|
| Phase 1: 评估 | Day 1 | 🔄 进行中 |
| Phase 2: 分析 | Day 2 | ⏳ 待开始 |
| Phase 3: 核心 | Week 2-3 | ⏳ 待开始 |
| Phase 4: 次要 | Week 4 | ⏳ 待开始 |
| Phase 5: 集成 | Week 5 | ⏳ 待开始 |
| 总体验证 | Week 5 | ⏳ 待开始 |

### 每周报告

**内容**:
- 覆盖率提升百分比
- 新增测试数量
- 发现的bug数量
- 阻塞问题

---

## 🚀 快速启动指南

### 生成覆盖率报告

```bash
# 生成HTML报告
cargo llvm-cov --workspace --html

# 在终端查看
cargo llvm-cov --workspace

# 特定crate
cargo llvm-cov --package vm-core --html
```

### 运行特定测试

```bash
# 单元测试
cargo test --lib

# 集成测试
cargo test --test '*'

# 带覆盖率的测试
cargo llvm-cov --package vm-core -- --test-threads=1
```

### 查看覆盖率报告

```bash
# 打开HTML报告
open target/llvm-cov/html/index.html

# 或在浏览器中
firefox target/llvm-cov/html/index.html
```

---

## 🎯 成功标准

### 必须达成

- ✅ 整体覆盖率 ≥ 80%
- ✅ 核心crate (vm-core, vm-engine-jit, vm-mem) ≥ 85%
- ✅ 所有crate ≥ 70%
- ✅ 无关键路径未测试

### 期望达成

- ✅ 覆盖率 ≥ 85%
- ✅ 集成测试完整
- ✅ 性能回归测试建立

### 可选达成

- ✅ 覆盖率 ≥ 90%
- ✅ 模糊测试集成
- ✅ 属性测试支持

---

## 📝 测试模板

### 单元测试模板

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_case() {
        // Arrange
        let input = setup_test_data();

        // Act
        let result = function_under_test(&input);

        // Assert
        assert_eq!(result.expected, result.actual);
    }

    #[test]
    fn test_edge_case_empty() {
        // 边界条件测试
        let input = empty_input();
        let result = function_under_test(&input);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_handling() {
        // 错误处理测试
        let invalid_input = invalid_data();
        let result = function_under_test(&invalid_input);
        assert!(matches!(result, Err(Error::InvalidInput)));
    }
}
```

### 集成测试模板

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_service_integration() {
        // 设置依赖
        let mut container = DIContainer::new();
        container.register_mock_services();

        // 执行集成测试
        let service = container.get_service();
        let result = service.execute_workflow().await;

        // 验证结果
        assert!(result.is_ok());
        verify_side_effects().await;
    }
}
```

---

## 🔍 覆盖率检查清单

### 每个模块检查

- [ ] 正常路径覆盖
- [ ] 边界条件覆盖
- [ ] 错误处理覆盖
- [ ] 并发场景覆盖
- [ ] 资源清理覆盖

### 关键代码路径

- [ ] JIT编译流程
- [ ] 内存管理
- [ ] 领域服务
- [ ] 设备模拟
- [ ] 跨架构翻译

---

## 📊 资源分配

### 时间分配

| 阶段 | 预计用时 | 占比 |
|------|---------|------|
| Phase 1: 评估 | 0.5天 | 2% |
| Phase 2: 分析 | 0.5天 | 2% |
| Phase 3: 核心 | 10-12天 | 48% |
| Phase 4: 次要 | 5-7天 | 29% |
| Phase 5: 集成 | 3-5天 | 19% |
| **总计** | **3-4周** | **100%** |

### 人力分配

建议1人全职执行，或2人协作:
- **开发者A**: vm-core, vm-mem测试
- **开发者B**: vm-engine-jit, 其他crate测试

---

## ⚠️ 风险和缓解

### 主要风险

1. **时间超期**
   - 缓解: 分阶段执行，优先核心crate
   - 备选: 降低次要crate目标

2. **测试编写困难**
   - 缓解: 先易后难，积累经验
   - 备选: 重构代码提高可测试性

3. **测试运行缓慢**
   - 缓解: 单元测试隔离，并行执行
   - 备选: 使用测试选择功能

4. **覆盖率平台期**
   - 缓解: 聚焦关键路径
   - 备选: 接受70%+覆盖率

---

## 🎓 最佳实践

### 测试命名

```rust
// ❌ 差
fn test1() { }
fn test_works() { }

// ✅ 好
fn test_execute_addition_returns_sum() { }
fn test_handle_invalid_input_returns_error() { }
fn test_cache_miss_triggers_compilation() { }
```

### 测试结构

```rust
// ✅ AAA模式
fn test_user_login_success() {
    // Arrange: 准备测试数据
    let user = create_test_user();
    let auth = AuthService::new();

    // Act: 执行被测试函数
    let result = auth.login(&user.username, &user.password);

    // Assert: 验证结果
    assert!(result.is_ok());
    assert_eq!(result.unwrap().id, user.id);
}
```

### Mock使用

```rust
// ✅ 使用mock隔离依赖
struct MockMMU {
    translate_result: Cell<Result<PhysAddr, TranslateError>>,
}

impl MMU for MockMMU {
    fn translate(&self, addr: VirtAddr) -> Result<PhysAddr, TranslateError> {
        self.translate_result.replace(Err(TranslateError::InvalidAddress))
    }
}
```

---

## 📞 相关资源

### 工具文档

- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)
- [Rust测试指南](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Criterion基准测试](https://bheisler.github.io/criterion.rs/book/)

### 项目文档

- DDD架构: vm-core/ARCHITECTURE.md
- JIT引擎: vm-engine-jit/README.md
- 审查报告: docs/VM_COMPREHENSIVE_REVIEW_REPORT.md

---

**创建者**: VM优化团队
**状态**: 📋 规划完成
**下一步**: 等待覆盖率评估结果
**预计完成**: 3-4周

🚀 **测试覆盖率提升计划已制定！准备执行！**
