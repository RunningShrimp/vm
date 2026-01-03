# CI/CD 优化方案

## 当前问题分析

### 1. 版本问题
- `ci.yml` 仍使用 Rust 1.85，应更新到 1.92
- MSRV (Minimum Supported Rust Version) 需要明确

### 2. 重复检查
- `ci.yml` 和 `code-quality.yml` 都有 formatting 和 clippy 检查
- 可以合并以节省 CI 时间

### 3. 缺失的工具
- 未使用 `cargo-hakari` 减少编译时间
- 未检查拼写错误 (`typos`)
- 未检查未使用的依赖 (`cargo-machete`)
- 缺少依赖审查 (`Dependency Review Action`)

### 4. 缓存策略
- 缓存键可以更精细
- 可以添加增量编译缓存

### 5. 性能监控
- 缺少构建时间趋势追踪
- 缺少测试时间监控

---

## 优化方案

### Phase 1: 紧急修复 (立即)

#### 1.1 更新 Rust 版本

**文件**: `.github/workflows/ci.yml`

**修改**:
```yaml
matrix:
  rust: [stable, '1.92']  # 更新到最新稳定版
```

**理由**: 与 rust-toolchain.toml 保持一致

---

#### 1.2 添加 Dependabot

**新建文件**: `.github/dependabot.yml`

```yaml
version: 2
updates:
  # Cargo dependencies
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
    open-pull-requests-limit: 10
    labels:
      - "dependencies"
      - "rust"
    reviewers:
      - "maintainer-team"
    commit-message:
      prefix: "deps"
      include: "scope"

  # GitHub Actions
  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
```

**收益**:
- 自动更新依赖
- 减少安全漏洞
- 节省维护时间

---

### Phase 2: 性能优化 (1-2周)

#### 2.1 启用 cargo-hakari

**添加到**:
- `Cargo.toml` (dev-dependencies)
- `.github/workflows/ci.yml`

```toml
[dev-dependencies]
cargo-hakari = "0.9"
```

```yaml
- name: Optimize dependency graph
  run: |
    cargo install cargo-hakari
    cargo hakari generate
    cargo build --workspace
```

**收益**: 减少 10-30% 编译时间

---

#### 2.2 优化缓存策略

**修改**: `.github/workflows/ci.yml`

```yaml
- name: Cache dependencies
  uses: actions/cache@v4
  with:
    path: |
      ~/.cargo/registry
      ~/.cargo/git
      target
    key: ${{ runner.os }}-cargo-${{ matrix.rust }}-${{ hashFiles('**/Cargo.lock') }}-${{ hashFiles('.cargo/config.toml', '**/Cargo.toml') }}
    restore-keys: |
      ${{ runner.os }}-cargo-${{ matrix.rust }}-
      ${{ runner.os }}-cargo-
```

**收益**: 更精确的缓存命中

---

#### 2.3 添加并行构建

```yaml
env:
  CARGO_BUILD_JOBS: 4  # 使用更多并行任务
  CARGO_PROFILE_DEV_SPLIT_DEBUGINFO: "packed"
```

**收益**: 更快的编译

---

### Phase 3: 质量增强 (2-3周)

#### 3.1 添加拼写检查

**新建文件**: `.github/workflows/typos.yml`

```yaml
name: Typos Check

on:
  push:
    branches: [master, main]
  pull_request:

jobs:
  typos:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Check typos
        uses: crate-ci/typos@master
```

**收益**: 自动检测拼写错误

---

#### 3.2 添加未使用依赖检查

```yaml
- name: Check for unused dependencies
  run: |
    cargo install cargo-machete
    cargo machete
```

**收益**: 保持依赖最小化

---

#### 3.3 添加依赖审查

**添加到** `.github/workflows/ci.yml`:

```yaml
- name: Dependency Review
  uses: actions/dependency-review-action@v4
  with:
    fail-on-severity: moderate
    allow-licenses: MIT, Apache-2.0, BSD-3-Clause
```

**收益**: 自动检测有问题的依赖

---

### Phase 4: 监控和报告 (3-4周)

#### 4.1 构建时间追踪

**新建文件**: `.github/workflows/build-metrics.yml`

```yaml
name: Build Metrics

on:
  push:
    branches: [master, main]
  pull_request:

jobs:
  build-time:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable

      - name: Measure build time
        run: |
          start=$(date +%s)
          cargo build --workspace --release
          end=$(date +%s)
          echo "Build time: $((end-start)) seconds"

          # 记录到 GitHub Metrics
          echo "Build time: $((end-start))s" >> $GITHUB_STEP_SUMMARY
```

**收益**: 追踪构建时间趋势

---

#### 4.2 测试时间监控

```yaml
- name: Run tests with timing
  run: |
    cargo test --workspace --all-features -- --test-threads=1 \
      --nocapture --test-threads=1 -Z unstable-options --format json \
      > test-results.json

    # 解析并报告慢测试
    python3 scripts/analyze_test_times.py test-results.json
```

---

#### 4.3 代码覆盖率趋势

**增强**: `.github/workflows/coverage.yml`

```yaml
- name: Compare coverage
  run: |
    # 与基线对比
    cargo llvm-cov --workspace --all-features --lcov --output-path lcov.info

    # 检查覆盖率是否下降
    python3 scripts/check_coverage_drop.py lcov.info baseline_coverage.json

    # 更新基线
    cp lcov.info baseline/
```

---

## 实施计划

### Week 1
- [ ] 更新 Rust 版本到 1.92
- [ ] 添加 Dependabot 配置
- [ ] 更新 MSRV 声明

### Week 2
- [ ] 集成 cargo-hakari
- [ ] 优化缓存策略
- [ ] 添加并行构建配置

### Week 3
- [ ] 添加拼写检查
- [ ] 添加未使用依赖检查
- [ ] 添加依赖审查

### Week 4
- [ ] 实现构建时间追踪
- [ ] 实现测试时间监控
- [ ] 增强覆盖率报告

---

## 成功指标

### 性能指标
- CI 总运行时间减少 20%
- 缓存命中率提升至 80%+
- 编译时间减少 15%

### 质量指标
- 自动检测 100% 的依赖更新
- 拼写错误检测覆盖率 100%
- 依赖审查覆盖率 100%

### 维护指标
- 手动依赖更新减少 90%
- CI 维护时间减少 30%
- 问题响应时间缩短 50%

---

## 风险缓解

### 风险1: 新工具引入问题
- **缓解**: 在 feature 分支测试
- **回滚**: 保持旧 workflow 可用

### 风险2: CI 时间增加
- **缓解**: 并行化任务
- **优化**: 缓存策略优化

### 风险3: 误报增加
- **缓解**: 配置合理的例外
- **调整**: 根据实际情况调整阈值

---

## 持续改进

### 监控
- 每月审查 CI 性能
- 收集开发者反馈
- 追踪失败率

### 优化
- 定期更新工具版本
- 优化缓存键
- 调整并行度

### 文档
- 保持本文档更新
- 记录所有变更
- 分享最佳实践
