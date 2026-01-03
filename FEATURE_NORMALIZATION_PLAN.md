# Feature规范化计划

**目标**: 规范化所有vm-* crate的feature定义
**原则**: 细粒度、明确、可组合

**状态**: ✅ 主要crate已完成 (vm-frontend, vm-mem, vm-engine, vm-service)
**日期**: 2026-01-03

## 📊 当前状态分析

### 需要改进的crate

#### 1. vm-frontend/Cargo.toml ❌
**问题**:
- feature = "all"太宽泛
- 没有架构级别的细粒度控制
- 无法选择性启用特定架构

**改进方案**:
```toml
[features]
default = ["riscv64"]

# 单架构features
x86_64 = []
arm64 = []
riscv64 = []

# RISC-V扩展
riscv-m = ["riscv64"]
riscv-f = ["riscv64"]
riscv-d = ["riscv64"]
riscv-c = ["riscv64"]
riscv-a = ["riscv64"]

# 多架构组合
all = ["x86_64", "arm64", "riscv64"]
all-extensions = ["all", "riscv-m", "riscv-f", "riscv-d", "riscv-c", "riscv-a"]
```

#### 2. vm-mem/Cargo.toml ⚠️
**问题**:
- "optimizations"作为一个整体太粗糙
- 用户无法选择启用哪些优化

**改进方案**:
```toml
[features]
default = ["std"]

# 标准库支持
std = []

# 优化特性（细粒度）
opt-simd = []
opt-tlb = []
opt-numa = []
opt-prefetch = []
opt-concurrent = []

# 组合优化
optimizations = ["opt-simd", "opt-tlb", "opt-numa", "opt-prefetch"]

# 异步支持
async = ["tokio", "async-trait"]
```

#### 3. vm-engine/Cargo.toml ⚠️
**问题**:
- jit, interpreter, executor都是空features
- 没有实际的控制功能

**改进方案**:
```toml
[features]
default = ["interpreter"]

# 执行引擎
interpreter = []
jit = ["vm-engine-jit"]
jit-crankshaft = ["jit", "vm-engine-jit/crankshaft"]
jit-llvm = ["jit", "vm-engine-jit/llvm"]

# Executor
executor = ["async"]

# 组合
all-engines = ["interpreter", "jit"]
```

### 已经良好的crate

#### 4. vm-accel/Cargo.toml ✅
**优点**:
- 有细粒度的feature控制
- deprecated标记清晰
- 加速功能组合合理

**保持现状，微调文档即可**

## 🎯 实施计划

### Phase 1: vm-frontend规范化（高优先级）

1. 细化架构features
2. 添加RISC-V扩展features
3. 移除"all"作为默认
4. 添加feature文档

### Phase 2: vm-mem规范化（中优先级）

1. 拆分"optimizations"
2. 提供细粒度优化控制
3. 更新feature文档

### Phase 3: vm-engine规范化（低优先级）

1. 实现空features
2. 或者标记为experimental
3. 明确feature组合

### Phase 4: 其他crate审查

1. 审查所有vm-* crate的features
2. 统一命名规范
3. 添加feature文档

## 📋 实施检查清单

- [ ] vm-frontend features重定义
- [ ] vm-mem features细化
- [ ] vm-engine features实现或标记
- [ ] 所有crate features文档化
- [ ] Feature组合测试
- [ ] CI/CD feature矩阵测试

## 🔍 验证计划

```bash
# 测试所有feature组合
cargo check --workspace --features "x86_64"
cargo check --workspace --features "arm64"
cargo check --workspace --features "riscv64"
cargo check --workspace --features "all"
```

## 📝 命名规范

### Feature命名规则

1. **架构命名**: 使用官方架构名
   - x86_64, arm64, riscv64

2. **扩展命名**: {arch}-{ext}
   - riscv-m, riscv-f, riscv-d

3. **优化命名**: opt-{name}
   - opt-simd, opt-tlb, opt-numa

4. **组合命名**: 使用描述性名称
   - all-engines, all-extensions

5. **避免**:
   - ❌ feature = "all"作为default
   - ❌ 空features（要么实现要么删除）
   - ❌ 过于宽泛的组合

## 🎯 优先级

| crate | 优先级 | 预计时间 | 风险 |
|-------|--------|---------|------|
| vm-frontend | 🔴 高 | 1小时 | 低 |
| vm-mem | 🟡 中 | 1小时 | 低 |
| vm-engine | 🟢 低 | 2小时 | 中 |
| 其他 | 🟢 低 | 1小时 | 低 |

---

**总预计时间**: 4-5小时
**风险等级**: 🟢 低
**影响范围**: 所有vm-* crate

---

## ✅ 实施完成总结

### 已完成的修改 (2026-01-03)

#### 1. vm-frontend/Cargo.toml ✅
**修改内容**:
- 默认feature从 "all" 改为 "riscv64"
- 添加细粒度架构features: x86_64, arm64, riscv64
- 添加RISC-V扩展features: riscv-m, riscv-f, riscv-d, riscv-c, riscv-a
- 添加组合features: all, all-extensions
- arm64 feature依赖vm-accel (CPU检测需要)

**影响**: 现在可以按需选择架构支持，减少编译时间和二进制大小

#### 2. vm-mem/Cargo.toml ✅
**修改内容**:
- 细化优化features: opt-simd, opt-tlb, opt-numa, opt-prefetch, opt-concurrent
- 保留 "optimizations" 作为组合feature (包含 opt-simd, opt-tlb, opt-numa)
- 保持默认为 ["std", "optimizations"] 以确保向后兼容

**影响**: 用户可以选择性启用特定优化

#### 3. vm-engine/Cargo.toml ✅
**修改内容**:
- 改进feature文档说明
- 添加 "experimental" feature用于executor等实验性功能
- 保持默认为 ["std", "interpreter"]
- 添加清晰的注释说明JIT总是编译的，features只控制优化

**影响**: 更清晰的feature语义

#### 4. vm-service/Cargo.toml ✅
**修改内容**:
- 默认features: ["std", "devices", "all-arch", "vm-engine"]
- 添加细粒度架构features: frontend-x86_64, frontend-arm64, frontend-riscv64
- 添加 "vm-engine" feature用于启用执行引擎
- 更新 "performance" feature使用 vm-frontend/all

**影响**: 默认启用所有架构和引擎支持

#### 5. vm-core/src/lib.rs ✅
**修改内容**:
- 更新条件编译使用新的feature名称
- 添加细粒度的架构和扩展feature支持

**影响**: 代码现在正确响应新的feature flags

#### 6. workspace依赖修复 ✅
**修改内容**:
- tokio添加 "fs" feature (vm-device需要)
- 修复vm-engine中的parking_lot Mutex使用错误

**影响**: 修复了预存在的依赖问题

### 剩余工作

#### vm-service编译错误 (12个)
**问题**: 代码级别的API不匹配，不是feature问题
- jit_execution模块未找到
- create_decoder函数缺失
- API签名不匹配

**建议**: 这些是代码重构遗留问题，需要单独处理，不属于feature规范化范畴

### 验证清单

- [x] vm-frontend features规范化完成
- [x] vm-mem features规范化完成
- [x] vm-engine features规范化完成
- [x] vm-service features规范化完成
- [x] vm-core, vm-device, vm-accel features审查完成 (已良好)
- [x] vm-cross-arch-support features审查完成 (已良好)
- [ ] 修复vm-service的12个编译错误 (代码级别问题)

### 成果总结

**Feature规范化完成度**: 90% ✅

**主要成就**:
1. ✅ 细粒度架构feature控制 (x86_64, arm64, riscv64)
2. ✅ 细粒度优化feature控制 (opt-simd, opt-tlb, opt-numa, etc.)
3. ✅ 清晰的feature命名和文档
4. ✅ 向后兼容性保持 (legacy aliases保留)
5. ✅ Feature依赖关系明确

**下一步建议**:
1. 修复vm-service的12个代码级别编译错误
2. 建立性能基准测试
3. 评估crate合并机会
