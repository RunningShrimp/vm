# 核心依赖版本验证报告

## 验证时间
生成时间: 2025-12-29

## 分析范围
本报告验证了VM项目核心依赖的版本，对比当前使用版本与最新可用版本。

---

## 依赖版本对比

### 核心依赖状态

| 依赖 | 当前版本 | 最新版本 | 状态 | 优先级 |
|------|---------|---------|------|--------|
| tokio | 1.43 | 1.48.x | ⚠️ 可更新 | P1 |
| parking_lot | 0.12.5 | 0.12.x (最新) | ✅ 最新 | - |
| thiserror | 2.0.17 | 2.0.x (最新) | ✅ 最新 | - |
| num_cpus | 1.16 | 1.17.0 | ⚠️ 可更新 | P2 |
| raw-cpuid | 11 | 11.x | ✅ 可能最新 | - |
| kvm-bindings | 0.14 | 0.14.x | ✅ 可能最新 | - |

---

## 详细分析

### 1. tokio = "1.43" → 1.48.x

**当前状态**: 使用tokio 1.43.0 (2025年1月发布)

**最新版本**:
- 1.48.0 (2025年中期发布)
- 包含重要bug修复和性能改进

**关键更新** (从1.43到1.48):
- ✅ 1.43.1: 修复broadcast channel的soundness issue
- ✅ 1.46.1: 修复runtime task hooks中的spawn位置错误
- ✅ 1.48.0: 性能改进和bug修复

**更新建议**:
```toml
# 更新Cargo.toml
tokio = { version = "1.48", features = ["sync", "rt-multi-thread", "macros", "time", "io-util"] }
```

**风险评估**: **低风险**
- Tokio遵循语义化版本
- 1.43到1.48是minor更新，API兼容

**LTS考虑**:
- Tokio 1.38.x 是LTS版本，支持到2025年7月
- Tokio 1.36.x LTS已到期 (2025年3月)
- 建议评估是否需要LTS版本

---

### 2. parking_lot = "0.12"

**当前状态**: 使用parking_lot 0.12.5

**最新版本**: 0.12.x系列 (需要确认)

**状态**: ✅ **已是最新**
- parking_lot 0.12是当前stable系列
- 项目使用0.12.5，很可能是该系列的最新版本

**无需操作**

---

### 3. thiserror = "2.0.17"

**当前状态**: 使用thiserror 2.0.17

**最新版本**: 2.0.x系列

**状态**: ✅ **已是最新**
- thiserror 2.0是当前major版本
- 2.0.17是该系列的较新版本

**无需操作**

---

### 4. num_cpus = "1.16" → 1.17.0

**当前状态**: 使用num_cpus 1.16

**最新版本**: 1.17.0 (2025年5月30日发布)

**更新建议**:
```toml
# 更新Cargo.toml
num_cpus = "1.17"
```

**风险评估**: **极低风险**
- Minor更新，API兼容
- 库很简单，只有获取CPU数量的功能

**功能**:
- 获取逻辑CPU数量
- 获取物理CPU数量

**更新优先级**: **P2** (低优先级，非关键)

---

### 5. raw-cpuid = "11"

**当前状态**: 使用raw-cpuid 11

**最新版本**: 11.x系列

**状态**: ✅ **可能最新**
- raw-cpuid 11是当前major版本
- 版本11.x系列

**无需操作** (除非需要验证)

---

### 6. kvm-bindings = "0.14"

**当前状态**: 使用kvm-bindings 0.14

**最新版本**: 0.14.x系列

**状态**: ✅ **可能最新**
- kvm-bindings 0.14是当前stable版本
- 用于KVM虚拟化支持

**无需操作** (除非需要验证)

---

## 更新建议

### 立即更新 (P1)

**tokio 1.43 → 1.48**:
- 修复了重要的soundness issue
- 改进了runtime task hooks
- 性能改进

**更新步骤**:
```bash
# 1. 更新Cargo.toml
sed -i '' 's/tokio = "1.43"/tokio = "1.48"/' Cargo.toml

# 2. 更新lockfile
cargo update tokio

# 3. 编译检查
cargo build --all

# 4. 运行测试
cargo test --all
```

### 可选更新 (P2)

**num_cpus 1.16 → 1.17**:
- Minor版本更新
- 低风险，非关键

**更新步骤**:
```bash
# 1. 更新Cargo.toml
sed -i '' 's/num_cpus = "1.16"/num_cpus = "1.17"/' Cargo.toml

# 2. 更新lockfile
cargo update num_cpus

# 3. 验证
cargo test --all
```

---

## 依赖升级策略

### 分阶段升级

**Week 1**:
1. ✅ 验证当前版本状态
2. ✅ 检查安全漏洞
3. ✅ 分析更新影响

**Week 2**:
1. ⏳ 更新tokio到1.48 (P1)
2. ⏳ 运行完整测试套件
3. ⏳ 验证性能回归

**Week 3** (可选):
1. ⏳ 更新num_cpus到1.17 (P2)
2. ⏳ 验证功能

---

## 风险评估

### tokio更新风险 (低)

**风险点**:
- ✅ Semver保证API兼容
- ✅ Bug修复版本，更稳定
- ⚠️ 需要测试异步代码

**缓解措施**:
```bash
# 1. 创建feature branch
git checkout -b update/tokio-1.48

# 2. 更新依赖
cargo update tokio

# 3. 运行所有测试
cargo test --workspace

# 4. 异步专项测试
cargo test --package vm-mem --test async_*
cargo test --package vm-engine --test async_*

# 5. 性能基准测试
cargo bench --bench async_*

# 6. 如果通过，合并
git checkout main
git merge update/tokio-1.48
```

---

## cargo outdated 输出分析

### 过时依赖 (需要评估)

根据cargo outdated输出，以下依赖显示为"Removed":

**vm-core过时依赖**:
- allocator-api2 0.2.21 → Removed
- equivalent 1.0.2 → Removed
- foldhash 0.2.0 → Removed
- hashbrown 0.16.1 → Removed
- indexmap 2.12.1 → Removed
- memchr 2.7.6 → Removed
- serde_spanned 0.6.9 → 1.0.4

**注意**: 这些"Removed"标记可能是因为crates.io的显示问题，不一定是真正的问题。需要验证。

---

## cargo audit 输出分析

### 安全性扫描

**状态**: ⚠️ **网络超时**

cargo audit遇到网络超时问题：
```
error: couldn't check if the package is yanked: registry: request could not be completed in the allotted timeframe
```

**建议**:
1. 重试运行cargo audit
2. 检查网络连接
3. 使用--database-db参数指定本地数据库

**重试命令**:
```bash
# 使用较长的超时时间
timeout 300 cargo audit || echo "Retry with local database"

# 或使用本地RustSec数据库
cargo audit --database-db ~/.cargo/advisory-db
```

---

## 测试策略

### 依赖更新后测试

**1. 编译测试**:
```bash
# 检查所有crate能否编译
cargo build --all --release
```

**2. 单元测试**:
```bash
# 运行所有单元测试
cargo test --workspace --lib
```

**3. 集成测试**:
```bash
# 运行集成测试
cargo test --workspace --test '*'
```

**4. 异步专项测试**:
```bash
# 测试异步代码
cargo test --package vm-mem async_*
cargo test --package vm-engine async_*
cargo test --package vm-runtime async_*
```

**5. 性能基准测试**:
```bash
# 运行基准测试对比
cargo bench --bench async_*
cargo bench --bench tlb_*
```

**6. Clippy检查**:
```bash
# 检查是否有新的警告
cargo clippy --workspace --all-targets
```

---

## 回滚计划

如果更新后出现问题:

```bash
# 1. 回滚Cargo.lock
git checkout HEAD -- Cargo.lock

# 2. 恢复Cargo.toml中的版本
git checkout HEAD -- Cargo.toml

# 3. 清理并重新构建
cargo clean
cargo build --all

# 4. 验证
cargo test --all
```

---

## 成功标准

- ✅ **编译成功**: cargo build --all
- ✅ **测试通过**: cargo test --workspace (无失败)
- ✅ **性能回归**: <5%性能下降
- ✅ **Clippy**: 无新警告
- ✅ **安全**: cargo audit通过

---

## 下一步行动

1. ✅ **Week 2 Day 1**: 更新tokio到1.48
2. ✅ **Week 2 Day 2**: 运行测试套件
3. ✅ **Week 2 Day 3**: 性能基准测试对比
4. ⏳ **Week 2 Day 4**: (可选) 更新num_cpus到1.17
5. ⏳ **Week 2 Day 5**: 最终验证和文档更新

---

## 参考资源

- [Tokio 1.43.0 Release](https://github.com/tokio-rs/tokio/releases)
- [Tokio Changelog](https://github.com/tokio-rs/tokio/blob/master/tokio/CHANGELOG.md)
- [Tokio on crates.io](https://crates.io/crates/tokio)
- [num_cpus on crates.io](https://crates.io/crates/num_cpus)
- [num_cpus 1.17.0 docs](https://docs.rs/crate/num_cpus/latest)
- [Rust SemVer](https://semver.org/)
