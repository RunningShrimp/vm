# 统一错误处理机制 - 实施计划

**日期**: 2026-01-06
**任务**: P1 #5 - 统一错误处理机制
**优先级**: 高
**预计时间**: 2-3天
**复杂度**: 中等

---

## 当前状态分析

### 现有统一错误基础 ✅

**vm-core/src/error.rs** 已经提供:
- ✅ VmError 统一错误类型
- ✅ 6个分类错误 (Core, Memory, Execution, Device, Platform, Io)
- ✅ WithContext 错误包装器
- ✅ 多错误聚合支持
- ✅ 完整的错误转换trait

### 发现的问题

**vm-accel模块**使用多个自定义错误类型:
```rust
// vm-accel/src/lib.rs
pub enum AccelLegacyError { ... }

// vm-accel/src/accel_fallback.rs
pub enum FallbackError { ... }

// vm-accel/src/hvf_impl.rs
pub enum HvfError { ... }
```

**问题**:
1. ❌ 错误类型分散,难以统一处理
2. ❌ 缺少错误上下文追踪
3. ❌ 转换层增加复杂性
4. ❌ 错误信息不一致

---

## 改进目标

### 主要目标

1. **统一所有vm-accel错误到VmError**
   - 移除AccelLegacyError
   - 移除FallbackError
   - 移除HvfError
   - 所有平台使用VmError::Platform

2. **添加错误上下文**
   - 使用VmError::WithContext包装
   - 记录错误发生的模块和操作
   - 保留关键调试信息

3. **简化错误转换**
   - 实现From<VmError> for所有平台错误
   - 统一错误传播模式
   - 减少样板代码

4. **改进错误消息**
   - 一致的错误格式
   - 包含关键上下文信息
   - 便于调试和日志

---

## 实施计划

### Phase 1: 评估与设计 (2小时)

**任务**:
1. 审计所有自定义错误类型
2. 设计统一的错误映射
3. 创建迁移指南
4. 更新文档

**输出**:
- 错误类型清单
- 迁移策略文档
- 新的错误使用模式

### Phase 2: vm-accel统一 (1天)

**任务**:
1. 移除AccelLegacyError
2. 移除FallbackError
3. 移除HvfError
4. 更新所有使用VmError::Platform

**影响**:
- lib.rs
- accel_fallback.rs
- hvf_impl.rs
- 所有平台实现文件

**预期**: 减少~150行错误定义代码

### Phase 3: 添加错误上下文 (半天)

**任务**:
1. 创建辅助宏添加上下文
2. 更新关键错误路径
3. 验证错误信息质量

**示例**:
```rust
// Before
return Err(AccelError::VcpuError("Failed to create vCPU".to_string()));

// After
return Err(VmError::Platform(PlatformError::ResourceAllocationFailed(
    format!("Failed to create vCPU {}: {}", id, e)
)).with_context("vm-accel", "vcpu_create"));
```

### Phase 4: 测试与验证 (半天)

**任务**:
1. 编译测试 (cargo check --workspace)
2. 运行测试套件
3. 验证错误消息质量
4. 性能验证 (零成本抽象)

---

## 实施细节

### 辅助宏设计

```rust
/// 为错误添加上下文的宏
#[macro_export]
macro_rules! error_context {
    ($error:expr, $module:expr, $operation:expr) => {
        $error.with_context($module, $operation)
    };
}

/// 简化平台错误的宏
#[macro_export]
macro_rules! platform_error {
    ($message:expr) => {
        VmError::Platform(PlatformError::AccessDenied($message.to_string()))
    };

    ($variant:ident, $message:expr) => {
        VmError::Platform(PlatformError::$variant($message.to_string()))
    };
}
```

### 错误转换映射

```rust
// vm-accel/src/lib.rs

// 移除 AccelLegacyError
// pub enum AccelLegacyError { ... }

// 添加到VmError的转换
impl From<HvfError> for VmError {
    fn from(err: HvfError) -> Self {
        VmError::Platform(PlatformError::AccessDenied(format!(
            "HVF error: {}", err
        )))
    }
}

// 或者直接移除中间类型,使用VmError
impl Accel for HvfAccel {
    fn run(&mut self) -> Result<VcpuExit, VmError> {
        // 直接返回VmError::Platform
        self.0.run().map_err(|e| {
            VmError::Platform(PlatformError::ExecutionFailed(
                format!("HVF vCPU run failed: {}", e)
            ))
        })
    }
}
```

---

## 成功标准

### 量化指标

| 指标 | 当前 | 目标 |
|------|------|------|
| 自定义错误类型 | 3个 | 0个 |
| 错误转换层 | 2-3层 | 0-1层 |
| 错误定义代码 | ~200行 | ~50行 |
| 重复错误消息 | 高 | 低 |

### 质量指标

| 方面 | 目标 |
|------|------|
| 错误消息一致性 | ✅ 统一格式 |
| 上下文信息完整 | ✅ 包含模块/操作 |
| 调试信息充足 | ✅ 关键参数可见 |
| 转换开销 | ✅ 零成本 |

---

## 风险评估

### 低风险 ✅

- **编译风险**: 低 (类型系统保证)
- **功能风险**: 无 (仅内部重构)
- **性能风险**: 无 (优化器内联)

### 缓解措施

1. **增量迁移**: 一个模块一个模块
2. **测试覆盖**: 每个阶段都验证
3. **保持兼容**: 公开API保持不变
4. **回退准备**: Git commit每个阶段

---

## 预期收益

### 可维护性提升

- ✅ 单一错误类型,易于处理
- ✅ 统一的错误消息格式
- ✅ 更好的错误追踪
- ✅ 简化的测试代码

### 代码质量提升

- ✅ 减少~150行错误定义
- ✅ 减少~100行错误转换
- ✅ 消除重复的错误模式
- ✅ 提高代码一致性

### 开发体验改进

- ✅ 更清晰的错误信息
- ✅ 更容易调试问题
- ✅ 更少的模板代码
- ✅ 更好的IDE支持

---

## 时间表

| 阶段 | 任务 | 时间 | 产出 |
|------|------|------|------|
| Phase 1 | 评估与设计 | 2h | 迁移计划 |
| Phase 2 | vm-accel统一 | 1d | 移除自定义类型 |
| Phase 3 | 添加错误上下文 | 0.5d | 改进错误消息 |
| Phase 4 | 测试与验证 | 0.5d | 质量保证 |
| **总计** | **全流程** | **2-3d** | **完成统一** |

---

## 后续改进

### 短期 (1-2周)

1. **错误分析工具**
   - 错误统计dashboard
   - 错误趋势分析
   - 自动化错误报告

2. **错误文档**
   - 常见错误处理指南
   - 错误恢复策略
   - 故障排查手册

### 中期 (1-2月)

1. **结构化日志**
   - 集成error!宏
   - 结构化错误字段
   - 日志聚合

2. **监控集成**
   - 错误率监控
   - 告警规则
   - 性能关联

---

## 实施准备

### 前置条件

✅ 已完成:
- VmError统一类型存在
- thiserror依赖可用
- 错误转换trait已定义

### 需要准备

- [ ] 审计所有自定义错误类型
- [ ] 创建错误映射表
- [ ] 准备测试用例
- [ ] 更新文档

---

**状态**: 📋 计划完成,等待实施
**优先级**: 高
**下一步**: 开始Phase 1评估

---

## 附录: 错误类型清单

### 需要迁移的错误类型

1. **AccelLegacyError** (lib.rs)
   - NotSupported
   - IoError
   - VmError(Arc<VmError>) - 已包装
   - → 映射到VmError对应类型

2. **FallbackError** (accel_fallback.rs)
   - UnsupportedOperation
   - → 映射到VmError::Core(CoreError::NotSupported)

3. **HvfError** (hvf_impl.rs)
   - InvalidVcpu
   - VcpuError
   - ExitReadError
   - → 映射到VmError::Platform或VmError::Execution

### 迁移优先级

**高优先级** (立即迁移):
- HvfError (最常用)
- AccelLegacyError (影响最广)

**中优先级** (Phase 3):
- FallbackError (使用较少)

---

**文档创建**: 2026-01-06
**任务**: P1 #5 统一错误处理
**状态**: 准备实施
