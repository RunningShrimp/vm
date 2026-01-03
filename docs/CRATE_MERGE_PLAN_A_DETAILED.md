# Crate合并方案A - 完全合并详细计划

**评估日期**: 2026-01-03
**方案**: 方案A - 完全合并 vm-engine-jit 到 vm-engine
**当前状态**: 🟡 评估阶段
**推荐指数**: ⭐⭐⭐⭐

---

## 📊 方案A概述

### 目标
将 vm-engine-jit 完全合并到 vm-engine 中，创建统一的JIT执行引擎crate。

### 为什么选择方案A

✅ **长期最佳选择**
- 彻底解决crate拆分问题
- 简化依赖关系
- 统一维护入口

✅ **性能优势**
- 更好的内联优化
- 减少跨crate调用开销
- 统一的编译缓存

✅ **维护简化**
- 单一代码库
- 统一版本管理
- 简化CI/CD流程

⚠️ **挑战**
- 破坏性变更
- 大规模重构
- 需要用户迁移

---

## 🔍 当前状态分析

### vm-engine (合并后)

**代码规模**: ~35,000 + ~43,000 = ~78,000行

**合并后的模块结构**:
```
vm-engine/
├── src/
│   ├── lib.rs                    # 统一的入口点
│   ├── interpreter/               # 解释器 (现有)
│   ├── jit/                       # 基础JIT (现有)
│   ├── jit_advanced/              # 高级JIT (来自vm-engine-jit) ⭐ NEW
│   │   ├── cranelift.rs           # Cranelift后端
│   │   ├── llvm.rs                # LLVM后端
│   │   ├── tiered_compiler.rs     # 分层编译
│   │   ├── compile_cache.rs       # 编译缓存
│   │   ├── aot/                   # AOT编译
│   │   │   ├── cache.rs
│   │   │   ├── format.rs
│   │   │   └── loader.rs
│   │   ├── ml/                    # ML引导优化
│   │   │   ├── model.rs
│   │   │   └── hotspot.rs
│   │   ├── optimization/          # 优化passes
│   │   │   ├── block_chaining.rs
│   │   │   ├── loop_opt.rs
│   │   │   └── inline_cache.rs
│   │   ├── gc/                    # GC集成
│   │   │   └── unified_gc.rs
│   │   ├── adaptive/              # 自适应优化
│   │   │   └── optimizer.rs
│   │   └── vendor/                # 厂商优化
│   │       └── optimizations.rs
│   └── executor/                  # 执行器 (现有)
└── examples/
    ├── jit_basic.rs               # 基础JIT示例
    └── jit_advanced.rs            # 高级JIT示例
```

### 依赖关系变化

**合并前**:
```
vm-engine → vm-engine-jit (可选依赖)
   ↓              ↓
vm-core      vm-core
```

**合并后**:
```
vm-engine
   ↓
vm-core, vm-mem, vm-ir, vm-accel, Cranelift, LLVM(可选)
```

---

## ⚙️ 实施计划

### Phase 1: 准备阶段 (1-2天)

#### 1.1 创建合并分支
```bash
git checkout -b crate-merge-vm-engine-jit
git push -u origin crate-merge-vm-engine-jit
```

#### 1.2 建立基线
- 运行完整测试套件记录基线
- 运行性能基准测试
- 记录当前API列表

```bash
# 测试基线
cargo test --workspace 2>&1 | tee tests_baseline.txt

# 性能基线
cargo bench --workspace 2>&1 | tee perf_baseline.txt

# API清单
cargo doc --no-deps --workspace 2>&1 | tee api_baseline.txt
```

#### 1.3 影响分析
- 查找所有使用vm-engine-jit的代码
- 分析公共API
- 识别破坏性变更

```bash
# 查找依赖
grep -r "vm-engine-jit" --include="*.toml" . > dependents.txt

# 查找import
grep -r "use vm_engine_jit" --include="*.rs" . > imports.txt
```

---

### Phase 2: 合并实施 (3-5天)

#### 2.1 代码迁移 (1天)

##### 2.1.1 创建目录结构
```bash
cd vm-engine/src
mkdir -p jit_advanced/{aot,ml,optimization,gc,adaptive,vendor}
```

##### 2.1.2 移动文件
```bash
# 从vm-engine-jit/src复制到vm-engine/src/jit_advanced/
cp ../../../vm-engine-jit/src/cranelift_backend.rs jit_advanced/cranelift.rs
cp ../../../vm-engine-jit/src/llvm_backend.rs jit_advanced/llvm.rs
cp ../../../vm-engine-jit/src/tiered_compiler.rs jit_advanced/
cp ../../../vm-engine-jit/src/compile_cache.rs jit_advanced/
# ... 继续复制其他文件
```

##### 2.1.3 自动化脚本
```bash
#!/bin/bash
# scripts/merge_vm_engine_jit.sh

VM_ENGINE_JIT="../vm-engine-jit/src"
TARGET="vm-engine/src/jit_advanced"

# 创建映射文件
declare -A FILE_MAP=(
    ["cranelift_backend.rs"]="jit_advanced/cranelift.rs"
    ["llvm_backend.rs"]="jit_advanced/llvm.rs"
    ["tiered_compiler.rs"]="jit_advanced/tiered_compiler.rs"
    ["compile_cache.rs"]="jit_advanced/compile_cache.rs"
    ["aot_cache.rs"]="jit_advanced/aot/cache.rs"
    ["aot_format.rs"]="jit_advanced/aot/format.rs"
    ["aot_loader.rs"]="jit_advanced/aot/loader.rs"
    ["ml_model.rs"]="jit_advanced/ml/model.rs"
    ["ewma_hotspot.rs"]="jit_advanced/ml/hotspot.rs"
    ["block_chaining.rs"]="jit_advanced/optimization/block_chaining.rs"
    ["loop_opt.rs"]="jit_advanced/optimization/loop_opt.rs"
    ["inline_cache.rs"]="jit_advanced/optimization/inline_cache.rs"
    ["unified_gc.rs"]="jit_advanced/gc/unified_gc.rs"
    ["adaptive_optimizer.rs"]="jit_advanced/adaptive/optimizer.rs"
    ["vendor_optimizations.rs"]="jit_advanced/vendor/optimizations.rs"
)

# 复制文件
for file in "${!FILE_MAP[@]}"; do
    target_path="${TARGET}/${FILE_MAP[$file]}"
    mkdir -p "$(dirname "$target_path")"
    cp "$VM_ENGINE_JIT/$file" "$target_path"
    echo "✓ Copied $file → $target_path"
done

echo "✓ Migration complete!"
```

#### 2.2 更新模块引用 (1天)

##### 2.2.1 创建mod.rs
```rust
// vm-engine/src/jit_advanced/mod.rs
//! 高级JIT编译功能
//!
//! 本模块包含来自vm-engine-jit的高级JIT功能：
//! - Cranelift和LLVM后端
//! - 分层编译
//! - AOT编译
//! - ML引导优化
//! - GC集成

pub mod cranelift;
pub mod llvm;
pub mod tiered_compiler;
pub mod compile_cache;

pub mod aot {
    pub mod cache;
    pub mod format;
    pub mod loader;
}

pub mod ml {
    pub mod model;
    pub mod hotspot;
}

pub mod optimization {
    pub mod block_chaining;
    pub mod loop_opt;
    pub mod inline_cache;
}

pub mod gc {
    pub mod unified_gc;
}

pub mod adaptive {
    pub mod optimizer;
}

pub mod vendor {
    pub mod optimizations;
}

// 重新导出常用类型
pub use tiered_compiler::TieredCompiler;
pub use compile_cache::CompileCache;
pub use aot::cache::AotCache;
pub use aot::format::AotFormat;
pub use aot::loader::AotLoader;
pub use ml::model::MLModel;
pub use ml::hotspot::EwmaHotspotDetector;
pub use optimization::block_chaining::{BlockChainer, BlockChain};
pub use optimization::loop_opt::LoopOptimizer;
pub use optimization::inline_cache::InlineCache;
pub use gc::unified_gc::UnifiedGC;
pub use adaptive::optimizer::{AdaptiveOptimizer, AdaptiveParameters};
pub use vendor::optimizations::{CpuVendor, VendorOptimizer, CpuFeature};
```

##### 2.2.2 更新lib.rs
```rust
// vm-engine/src/lib.rs

// ... 现有代码 ...

// 高级JIT功能
#[cfg(feature = "jit-advanced")]
pub mod jit_advanced;

// 当启用jit-advanced时，重新导出类型（保持向后兼容）
#[cfg(feature = "jit-advanced")]
pub use jit_advanced::{
    TieredCompiler, CompileCache, AotCache, AotFormat, AotLoader,
    MLModel, EwmaHotspotDetector,
    BlockChainer, BlockChain, LoopOptimizer, InlineCache,
    UnifiedGC, AdaptiveOptimizer, AdaptiveParameters,
    CpuVendor, VendorOptimizer, CpuFeature,
};
```

#### 2.3 更新Cargo.toml (0.5天)

```toml
[package]
name = "vm-engine"
version = "0.2.0"  # 大版本升级
edition = "2024"

[dependencies]
# ... 现有依赖保持不变 ...

# 从vm-engine-jit迁移来的依赖
cranelift = { version = "0.110", package = "cranelift-codegen", optional = true }
llvm-sys = { version = "180", optional = true }

# Features
[features]
default = ["std", "interpreter", "jit"]

# 基础JIT (现有)
jit = ["cranelift"]

# 高级JIT (从vm-engine-jit迁移)
jit-advanced = [
    "jit",
    "cranelift",
    "llvm-sys",  # 可选LLVM后端
]

# AOT编译
aot = ["jit-advanced"]

# ML优化
ml-optimization = ["jit-advanced"]

# 完整JIT功能
jit-full = ["jit-advanced", "aot", "ml-optimization"]

# 所有引擎
all-engines = ["interpreter", "jit-full"]
```

#### 2.4 更新import语句 (1天)

##### 2.4.1 自动化脚本
```bash
#!/bin/bash
# scripts/update_imports.sh

echo "更新import语句..."

# 在vm-engine内部
find vm-engine/src -name "*.rs" -type f -exec sed -i.bak '
    s/use vm_engine_jit::/use crate::jit_advanced::/g
    s/use super::/use crate::jit_advanced::/g
' {} \;

# 在其他crate中
find . -name "*.rs" -type f -not -path "./vm-engine/*" -not -path "./target/*" -exec sed -i.bak '
    s/use vm_engine_jit::/use vm_engine::jit_advanced::/g
' {} \;

echo "✓ Import语句更新完成"
```

##### 2.4.2 手动检查
```bash
# 查找遗漏的import
grep -r "vm_engine_jit" --include="*.rs" . | grep -v ".bak"
```

#### 2.5 解决命名冲突 (0.5天)

##### 可能的冲突类型

1. **类型名称冲突**
```rust
// vm-engine/src/jit/core.rs
pub struct JITCompiler { }

// vm-engine/src/jit_advanced/tiered_compiler.rs
pub struct TieredCompiler { }  // ✅ 无冲突
```

2. **函数名称冲突**
```rust
// 如果有冲突，使用命名空间
use crate::jit::JITCompiler as BasicJIT;
use crate::jit_advanced::TieredCompiler;
```

3. **Trait冲突**
```rust
// 使用where子句或完全限定语法
fn process<T: jit::JITTrait>(compiler: T) { }
fn process<T: jit_advanced::AdvancedJITTrait>(compiler: T) { }
```

#### 2.6 更新测试 (0.5天)

```bash
# 移动vm-engine-jit的测试到vm-engine
mkdir -p vm-engine/tests/jit_advanced

cp ../vm-engine-jit/tests/*.rs vm-engine/tests/jit_advanced/

# 更新测试中的import
cd vm-engine/tests/jit_advanced
for file in *.rs; do
    sed -i.bak 's/use vm_engine_jit::/use vm_engine::jit_advanced::/g' "$file"
done
```

#### 2.7 更新文档 (0.5天)

##### 2.7.1 更新README
```markdown
## JIT编译

vm-engine提供完整的JIT编译功能：

### 基础JIT
```toml
vm-engine = { path = "../vm-engine", features = ["jit"] }
```

### 高级JIT
```toml
vm-engine = { path = "../vm-engine", features = ["jit-advanced"] }
```

### 完整JIT
```toml
vm-engine = { path = "../vm-engine", features = ["jit-full"] }
```

#### 迁移说明
从v0.1.x迁移到v0.2.0，需要更新import：
```rust
// 旧版本
use vm_engine_jit::TieredCompiler;

// 新版本
use vm_engine::jit_advanced::TieredCompiler;
// 或使用便捷导入 (推荐)
use vm_engine::{TieredCompiler};
```
```

##### 2.7.2 创建迁移指南
```markdown
# v0.2.0 迁移指南

## 破坏性变更
vm-engine-jit已合并到vm-engine。

## 迁移步骤

### 1. 更新Cargo.toml
```toml
# 移除
vm-engine-jit = { path = "../vm-engine-jit" }

# 更新
vm-engine = { path = "../vm-engine", features = ["jit-advanced"] }
```

### 2. 更新import
```rust
// 查找替换
:,%s/use vm_engine_jit::/use vm_engine::jit_advanced::/g

// 或使用便捷导入
use vm_engine::{TieredCompiler, AotCache, MLModel};
```

### 3. 更新feature
```toml
# 移除
features = ["jit", "llvm"]

# 使用
features = ["jit-advanced"]
```
```

---

### Phase 3: 测试验证 (1-2天)

#### 3.1 编译验证
```bash
# 清理构建
cargo clean

# 验证编译
cargo build --workspace 2>&1 | tee compile.log

# 检查错误
grep "error" compile.log | wc -l  # 应该为0
```

#### 3.2 单元测试
```bash
# 运行所有测试
cargo test --workspace 2>&1 | tee test_results.txt

# 检查通过率
grep "test result" test_results.txt

# 应该看到
# test result: ok. X passed in Ys
```

#### 3.3 集成测试
```bash
# 运行集成测试
cargo test --workspace --test '*_integration*' 2>&1

# 运行示例
cargo run --example jit_advanced --features jit-advanced
```

#### 3.4 性能基准测试
```bash
# 运行完整benchmark套件
cargo bench --workspace 2>&1 | tee bench_results.txt

# 对比基线
diff perf_baseline.txt bench_results.txt

# 检查性能回归
# 允许 ±5% 的波动
```

#### 3.5 API验证
```bash
# 生成文档
cargo doc --workspace --no-deps 2>&1 | tee doc.log

# 检查文档错误
grep "warning: unused" doc.log | wc -l
grep "error" doc.log | wc -l  # 应该为0
```

---

### Phase 4: 发布准备 (1天)

#### 4.1 版本管理
```toml
# vm-engine/Cargo.toml
[package]
name = "vm-engine"
version = "0.2.0"  # 大版本升级，允许破坏性变更
```

#### 4.2 CHANGELOG更新
```markdown
# Changelog

## [0.2.0] - 2026-01-XX

### Added
- 合并vm-engine-jit到vm-engine
- 统一的JIT编译接口
- 新增jit-advanced feature
- 新增jit-full feature
- 完整的AOT编译支持
- ML引导的JIT优化

### Changed
- **BREAKING**: vm-engine-jit已合并到vm-engine
- **BREAKING**: API路径变更，见迁移指南
- JIT编译性能提升 10-20%
- 统一的编译缓存管理

### Removed
- vm-engine-jit crate (已合并)

### Migration
见 [MIGRATION_GUIDE.md](./MIGRATION_GUIDE.md)
```

#### 4.3 CI/CD更新
```yaml
# .github/workflows/test.yml
- name: Test vm-engine
  run: |
    cargo test --package vm-engine --features jit
    cargo test --package vm-engine --features jit-advanced
    cargo test --package vm-engine --features jit-full
```

---

## 📊 风险评估与缓解

### 风险矩阵

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| 编译失败 | 🟡 中 | 🔴 高 | 增量迁移，每个阶段验证 |
| 测试失败 | 🟡 中 | 🟡 中 | 完整测试覆盖，并行运行新旧版本 |
| 性能回归 | 🟢 低 | 🟡 中 | 性能baseline，持续监控 |
| 用户迁移困难 | 🟡 中 | 🔴 高 | 详细文档，自动化迁移工具 |
| API破坏性变更 | 🔴 高 | 🟡 中 | 大版本号，长时间deprecation期 |

### 回滚计划

```bash
# 如果合并失败，回滚步骤
git checkout master
git branch -D crate-merge-vm-engine-jit
git push origin --delete crate-merge-vm-engine-jit

# 恢复vm-engine-jit
# 从上一个tag恢复
```

---

## 🎯 成功标准

### 必须达到
- ✅ 所有测试通过 (100% pass rate)
- ✅ 编译无错误 (0 errors)
- ✅ 性能无回归 (< 5% 差异)
- ✅ 文档完整更新
- ✅ CI/CD通过

### 期望达到
- ✅ 代码质量提升 (Clippy警告减少)
- ✅ 编译时间优化 (< 10% 增加)
- ✅ 二进制大小优化 (< 5% 增加)

---

## 📅 时间表

| 阶段 | 任务 | 时间 | 负责人 |
|------|------|------|--------|
| **Phase 1** | 准备 | 1-2天 | - |
| 1.1 | 创建分支 | 0.5天 | - |
| 1.2 | 建立基线 | 0.5天 | - |
| 1.3 | 影响分析 | 0.5天 | - |
| **Phase 2** | 实施 | 3-5天 | - |
| 2.1 | 代码迁移 | 1天 | - |
| 2.2 | 更新模块 | 1天 | - |
| 2.3 | 更新Cargo.toml | 0.5天 | - |
| 2.4 | 更新import | 1天 | - |
| 2.5 | 解决冲突 | 0.5天 | - |
| 2.6 | 更新测试 | 0.5天 | - |
| 2.7 | 更新文档 | 0.5天 | - |
| **Phase 3** | 测试 | 1-2天 | - |
| 3.1 | 编译验证 | 0.5天 | - |
| 3.2 | 单元测试 | 0.5天 | - |
| 3.3 | 集成测试 | 0.5天 | - |
| 3.4 | 性能测试 | 0.5天 | - |
| 3.5 | API验证 | 0.5天 | - |
| **Phase 4** | 发布 | 1天 | - |
| 4.1 | 版本管理 | 0.2天 | - |
| 4.2 | CHANGELOG | 0.3天 | - |
| 4.3 | CI/CD | 0.5天 | - |
| **总计** | | **6-10天** | - |

---

## 🔄 迁移工具

### 自动化迁移脚本

```bash
#!/bin/bash
# scripts/migrate_to_v0.2.sh

echo "=== vm-engine v0.2.0 迁移工具 ==="
echo ""

# 检查当前使用vm-engine-jit的依赖
echo "1. 检查依赖..."
grep -r "vm-engine-jit" --include="Cargo.toml" . > /tmp/vm_engine_jit_deps.txt

if [ -s /tmp/vm_engine_jit_deps.txt ]; then
    echo "发现以下文件使用vm-engine-jit:"
    cat /tmp/vm_engine_jit_deps.txt
    echo ""
    echo "正在自动更新..."

    # 更新Cargo.toml
    find . -name "Cargo.toml" -type f -exec sed -i.bak '
        s/vm-engine-jit = { path = ".*" }/vm-engine = { path = "..\/vm-engine", features = ["jit-advanced"] }/g
    ' {} \;

    echo "✓ Cargo.toml已更新"
else
    echo "✓ 未发现vm-engine-jit依赖"
fi

# 更新import语句
echo ""
echo "2. 更新import语句..."
grep -r "use vm_engine_jit" --include="*.rs" . > /tmp/vm_engine_jit_imports.txt

if [ -s /tmp/vm_engine_jit_imports.txt ]; then
    echo "发现以下import需要更新:"
    cat /tmp/vm_engine_jit_imports.txt
    echo ""
    echo "正在自动更新..."

    # 更新import
    find . -name "*.rs" -type f -exec sed -i.bak '
        s/use vm_engine_jit::/use vm_engine::jit_advanced::/g
    ' {} \;

    echo "✓ Import语句已更新"
else
    echo "✓ 未发现需要更新的import"
fi

echo ""
echo "3. 验证更新..."
cargo check --workspace 2>&1 | grep -E "(error|warning)" | head -20

echo ""
echo "迁移完成！请检查上面的输出，然后运行以下命令验证:"
echo "  cargo test --workspace"
echo "  cargo build --workspace"
```

---

## 📝 后续步骤

### 立即可执行
1. ✅ 评审本计划
2. ⏳ 创建合并分支
3. ⏳ 建立性能baseline
4. ⏳ 通知用户即将进行的破坏性变更

### 短期 (1-2周)
5. ⏳ 执行Phase 1 (准备)
6. ⏳ 执行Phase 2 (实施)
7. ⏳ 执行Phase 3 (测试)

### 中期 (1个月)
8. ⏳ 执行Phase 4 (发布)
9. ⏳ 收集用户反馈
10. ⏳ 修复发现的问题

### 长期 (3-6个月)
11. ⏳ 移除旧版本的vm-engine-jit
12. ⏳ 清理deprecation代码
13. ⏳ 优化合并后的代码

---

## 🎯 关键决策点

### 决策1: 是否执行合并？

**选项A: 立即执行 (推荐)**
- 优点：彻底解决问题，长期收益最大
- 缺点：短期内需要用户迁移
- 建议：✅ 推荐执行

**选项B: 延迟执行**
- 优点：给用户更多准备时间
- 缺点：技术债务持续积累
- 建议：❌ 不推荐

**选项C: 不执行**
- 优点：零风险
- 缺点：继续维护两套代码
- 建议：❌ 不推荐

### 决策2: 发布策略

**选项A: 硬性切换 (推荐)**
- v0.2.0直接发布合并版本
- 优点：清晰的里程碑
- 缺点：强制用户迁移
- 建议：✅ 推荐

**选项B: 渐进式迁移**
- 保留vm-engine-jit，标记为deprecated
- v0.3.0移除
- 优点：给用户缓冲时间
- 缺点：维护双倍代码
- 建议：🟡 可选

### 决策3: API设计

**选项A: 完全重命名 (推荐)**
```rust
// 新API
use vm_engine::jit_advanced::TieredCompiler;
```
- 优点：清晰，避免混淆
- 缺点：需要用户修改代码
- 建议：✅ 推荐

**选项B: 重导出到顶层**
```rust
// 便捷导入
use vm_engine::{TieredCompiler};
```
- 优点：使用简单
- 缺点：可能污染命名空间
- 建议：✅ 推荐（同时提供）

---

## 📚 参考资料

- [Crate合并评估报告](./CRATE_MERGE_EVALUATION.md)
- [方案C实施报告](../crate_merge_plan_c_report.md)
- [性能基准测试](./PERFORMANCE_BASELINE.md)
- [Feature规范化计划](../FEATURE_NORMALIZATION_PLAN.md)

---

*计划版本: 1.0*
*创建日期: 2026-01-03*
*状态: 🟡 评审中*
*下一步: 等待用户确认后开始执行*
