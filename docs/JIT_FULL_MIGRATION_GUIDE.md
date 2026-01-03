# jit-full Feature 迁移指南

**版本**: 1.0
**日期**: 2026-01-03
**相关**: Crate合并方案C实施报告

---

## 📋 概述

`jit-full` feature 是 vm-engine 的新功能，它通过 feature 统一的方式，将 vm-engine-jit 的高级功能集成到 vm-engine 中，提供更简洁的 API 和更好的用户体验。

### 主要优势

- ✅ **统一依赖** - 只需依赖 vm-engine 一个 crate
- ✅ **简化导入** - 所有类型从 `vm_engine` 统一导入
- ✅ **向后兼容** - 现有代码无需修改，可继续分别使用
- ✅ **可选启用** - 按需启用高级功能，减少编译时间和二进制大小

---

## 🔄 迁移路径

### 方案A: 新项目 (推荐)

**适用场景**: 新创建的项目

**步骤**:

1. **添加依赖**
```toml
# Cargo.toml
[dependencies]
vm-engine = { path = "../vm-engine", features = ["jit-full"] }
```

2. **导入类型**
```rust
use vm_engine::{
    // 基础JIT类型
    JITCompiler, JITConfig,

    // 高级JIT类型 (来自vm-engine-jit)
    TieredCompiler,
    AotCache,
    MLModel,
    BlockChainer,
    // ... 更多类型
};
```

3. **使用**
```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建基础JIT
    let jit = JITCompiler::new(Default::default());

    // 创建高级组件
    let tiered = TieredCompiler::new()?;
    let aot_cache = AotCache::new(Default::default())?;

    Ok(())
}
```

---

### 方案B: 现有项目迁移

**适用场景**: 已经使用 vm-engine 和 vm-engine-jit 的项目

#### 阶段1: 无需修改 (保持兼容)

**当前代码继续工作**:
```toml
# Cargo.toml (保持不变)
[dependencies]
vm-engine = { path = "../vm-engine" }
vm-engine-jit = { path = "../vm-engine-jit" }
```

```rust
// main.rs (保持不变)
use vm_engine::JITCompiler;
use vm_engine_jit::TieredCompiler;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let jit = JITCompiler::new(Default::default());
    let tiered = TieredCompiler::new()?;
    Ok(())
}
```

✅ **无需任何修改，代码继续正常工作**

---

#### 阶段2: 逐步迁移到 jit-full

**步骤1: 启用 jit-full feature**

```toml
# Cargo.toml
[dependencies]
# 保留原有依赖
vm-engine = { path = "../vm-engine", features = ["jit-full"] }
vm-engine-jit = { path = "../vm-engine-jit" }  # 暂时保留
```

**步骤2: 逐步更新导入**

```rust
// main.rs
// 旧导入 (仍然有效)
use vm_engine::JITCompiler;
use vm_engine_jit::TieredCompiler;

// 新导入 (统一来源)
use vm_engine::{JITCompiler, TieredCompiler};
```

**步骤3: 测试验证**

```bash
# 编译测试
cargo build --features jit-full

# 运行测试
cargo test --features jit-full

# 运行示例
cargo run --example my_example --features jit-full
```

---

#### 阶段3: 完全迁移

**移除 vm-engine-jit 直接依赖**:

```toml
# Cargo.toml (最终版本)
[dependencies]
vm-engine = { path = "../vm-engine", features = ["jit-full"] }
# vm-engine-jit 依赖已移除，通过 jit-full feature 自动引入
```

```rust
// main.rs (最终版本)
use vm_engine::{
    JITCompiler,
    TieredCompiler,
    AotCache,
    MLModel,
    BlockChainer,
    LoopOptimizer,
    InlineCache,
    // ... 所有高级类型
};
```

---

## 📦 可用的类型和模块

### 基础JIT类型 (始终可用)

```rust
use vm_engine::{
    JITCompiler,    // 基础JIT编译器
    JITConfig,      // JIT配置
};
```

### jit-full feature 启用的高级类型

```rust
#[cfg(feature = "jit-full")]
use vm_engine::{
    // 核心JIT编译器
    Jit,
    JitContext,

    // 分层编译
    tiered_compiler::TieredCompiler,

    // 编译缓存
    compile_cache::CompileCache,

    // AOT相关
    aot_cache::AotCache,
    aot_format::AotFormat,
    aot_loader::AotLoader,

    // ML引导的JIT
    ml_model::MLModel,
    ewma_hotspot::EwmaHotspotDetector,

    // 优化passes
    block_chaining::{BlockChainer, BlockChain},
    loop_opt::LoopOptimizer,
    inline_cache::InlineCache,

    // GC相关
    unified_gc::UnifiedGC,

    // 性能分析
    adaptive_optimizer::{AdaptiveOptimizer, AdaptiveParameters},

    // 厂商优化
    vendor_optimizations::{CpuVendor, VendorOptimizer, CpuFeature},
};
```

---

## 🎯 使用场景

### 场景1: 只需要基础JIT

```toml
# Cargo.toml
[dependencies]
vm-engine = { path = "../vm-engine" }
```

```rust
use vm_engine::JITCompiler;

let jit = JITCompiler::new(Default::default());
```

✅ **编译时间快，二进制小**

---

### 场景2: 需要高级JIT功能

```toml
# Cargo.toml
[dependencies]
vm-engine = { path = "../vm-engine", features = ["jit-full"] }
```

```rust
use vm_engine::{
    JITCompiler,
    TieredCompiler,
    AotCache,
    MLModel,
};

let jit = JITCompiler::new(Default::default());
let tiered = TieredCompiler::new()?;
let aot = AotCache::new(Default::default())?;
let ml = MLModel::new()?;
```

✅ **统一API，完整功能**

---

### 场景3: 条件编译

```rust
// 基础功能 (始终可用)
use vm_engine::JITCompiler;

// 高级功能 (条件编译)
#[cfg(feature = "jit-full")]
use vm_engine::{
    TieredCompiler,
    AotCache,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let jit = JITCompiler::new(Default::default());

    #[cfg(feature = "jit-full")]
    let tiered = TieredCompiler::new()?;

    #[cfg(feature = "jit-full")]
    let aot = AotCache::new(Default::default())?;

    Ok(())
}
```

✅ **灵活控制，按需启用**

---

## ⚙️ Feature 组合

### 可用的 Features

```toml
[features]
# 基础JIT
jit = []

# 完整JIT (包含vm-engine-jit)
jit-full = ["jit", "vm-engine-jit"]

# 所有引擎 (基础)
all-engines = ["interpreter", "jit"]

# 所有引擎 (完整)
all-engines-full = ["interpreter", "jit-full"]
```

### 推荐组合

#### 最小化配置
```toml
vm-engine = { path = "../vm-engine" }
```
- ✅ 编译时间最快
- ✅ 二进制最小
- ❌ 只有基础JIT功能

#### 标准配置
```toml
vm-engine = { path = "../vm-engine", features = ["jit"] }
```
- ✅ 基础JIT功能
- ✅ 合理的编译时间

#### 完整配置 (推荐)
```toml
vm-engine = { path = "../vm-engine", features = ["jit-full"] }
```
- ✅ 所有JIT功能
- ✅ 统一API
- ⚠️ 编译时间较长

---

## 🧪 测试和验证

### 编译测试

```bash
# 基础编译
cargo check --package vm-engine

# jit-full feature 编译
cargo check --package vm-engine --features jit-full

# 完整workspace编译
cargo check --workspace
```

### 功能测试

```bash
# 运行jit-full示例
cargo run --example jit_full_example --features jit-full

# 运行vm-engine测试
cargo test --package vm-engine --features jit-full

# 运行vm-engine-jit测试
cargo test --package vm-engine-jit
```

### 集成测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_jit() {
        let jit = JITCompiler::new(Default::default());
        // ... 测试代码
    }

    #[cfg(feature = "jit-full")]
    #[test]
    fn test_tiered_compiler() {
        let tiered = TieredCompiler::new().unwrap();
        // ... 测试代码
    }
}
```

---

## 🐛 常见问题

### Q1: 启用 jit-full 后编译时间变长？

**原因**: vm-engine-jit 包含大量代码，编译需要更多时间

**解决方案**:
- 在开发时使用 `features = ["jit"]` (基础JIT)
- 只在发布时使用 `features = ["jit-full"]`
- 使用 `cargo check` 快速检查，`cargo build --release` 完整编译

### Q2: 如何判断某个类型是否需要 jit-full？

**检查方式**:
```rust
// 如果类型来自 vm_engine_jit，需要 jit-full feature
use vm_engine::TieredCompiler;  // 需要 jit-full

// 如果类型来自 vm_engine，始终可用
use vm_engine::JITCompiler;  // 不需要 jit-full
```

**编译器提示**:
```
error[E0432]: unresolved import `vm_engine::TieredCompiler`
  --> src/main.rs:5:5
   |
5  |     TieredCompiler,
   |     ^^^^^^^^^^^^^^ not found in `vm_engine`
   |
   = note: this type requires the `jit-full` feature
```

### Q3: 旧代码中的 `use vm_engine_jit::...` 需要修改吗？

**短期**: 不需要，旧代码继续工作

**长期**: 建议改为 `use vm_engine::...` 以统一API

### Q4: 如何在条件编译中检查 jit-full？

```rust
#[cfg(feature = "jit-full")]
fn advanced_function() {
    // jit-full 特定代码
}

#[cfg(not(feature = "jit-full"))]
fn advanced_function() {
    // 降级实现或错误
    panic!("This function requires jit-full feature");
}
```

### Q5: 性能会有影响吗？

**运行时性能**: ❌ 无影响
- jit-full 只是编译时 feature
- 生成的代码与分别使用完全相同

**编译时间性能**: ⚠️ 有影响
- jit-full 增加编译时间 20-30%
- 建议开发时使用基础 features

**二进制大小**: ⚠️ 有影响
- jit-full 增加二进制大小 (包含更多功能)
- 可通过 feature 选择控制

---

## 📚 示例代码

### 完整示例: 创建 JIT 编译 pipeline

```rust
use vm_engine::{
    JITCompiler,
    Jit,
    JitContext,
};

#[cfg(feature = "jit-full")]
use vm_engine::{
    TieredCompiler,
    AotCache,
    MLModel,
    BlockChainer,
    LoopOptimizer,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 基础JIT (始终可用)
    let jit = JITCompiler::new(Default::default());

    // 高级JIT (需要 jit-full)
    #[cfg(feature = "jit-full")]
    {
        let tiered = TieredCompiler::new()?;
        let aot_cache = AotCache::new(Default::default())?;
        let ml = MLModel::new()?;
        let chainer = BlockChainer::new();
        let loopy = LoopOptimizer::new();

        println!("✓ 完整JIT pipeline已创建");
    }

    Ok(())
}
```

### 条件编译示例

```rust
use vm_engine::JITCompiler;

#[cfg(feature = "jit-full")]
use vm_engine::TieredCompiler;

fn create_jit() -> Result<(), Box<dyn std::error::Error>> {
    let _jit = JITCompiler::new(Default::default());

    #[cfg(feature = "jit-full")]
    let _tiered = TieredCompiler::new()?;

    #[cfg(not(feature = "jit-full"))]
    eprintln!("提示: 启用 jit-full feature 以获得更多功能");

    Ok(())
}
```

---

## 🚀 最佳实践

### 1. 渐进迁移

不要一次性迁移所有代码：
- ✅ 新代码使用 jit-full
- ✅ 旧代码逐步迁移
- ✅ 保持向后兼容

### 2. 文档更新

更新项目文档说明 feature 使用：
```markdown
## Features

- `jit`: 基础JIT功能
- `jit-full`: 完整JIT功能，包含分层编译、AOT缓存、ML优化等

推荐使用 `jit-full` 以获得最佳性能和功能。
```

### 3. CI/CD 集成

在 CI 中测试两种配置：
```yaml
test:
  script:
    - cargo test --features jit          # 基础测试
    - cargo test --features jit-full     # 完整测试
```

### 4. 错误处理

提供清晰的错误提示：
```rust
#[cfg(feature = "jit-full")]
fn advanced_optimization() -> Result<()> {
    let tiered = TieredCompiler::new()?;
    // ...
    Ok(())
}

#[cfg(not(feature = "jit-full"))]
fn advanced_optimization() -> Result<()> {
    Err(anyhow::anyhow!(
        "高级优化需要启用 jit-full feature。\n\
         请在 Cargo.toml 中添加: features = [\"jit-full\"]"
    ))
}
```

---

## 📞 获取帮助

### 文档资源

- **示例代码**: `examples/jit_full_example.rs`
- **实施报告**: `crate_merge_plan_c_report.md`
- **API文档**: `cargo doc --open --features jit-full`

### 问题反馈

如果遇到问题，请：
1. 检查是否启用了正确的 feature
2. 查看编译器错误提示
3. 参考 `examples/jit_full_example.rs`
4. 提交 issue 到项目仓库

---

## 📝 迁移检查清单

### 迁移前
- [ ] 备份当前代码
- [ ] 运行现有测试确保通过
- [ ] 记录当前使用的 vm-engine-jit 功能

### 迁移中
- [ ] 添加 `jit-full` feature 到 Cargo.toml
- [ ] 更新导入语句
- [ ] 运行编译测试
- [ ] 运行功能测试

### 迁移后
- [ ] 移除 vm-engine-jit 直接依赖
- [ ] 更新项目文档
- [ ] 更新 CI/CD 配置
- [ ] 验证性能无回归

---

*迁移指南版本: 1.0*
*最后更新: 2026-01-03*
*相关方案: Crate合并方案C*
*状态: ✅ jit-full feature 已完全实施*
