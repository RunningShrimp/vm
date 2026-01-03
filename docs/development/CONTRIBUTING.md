# 贡献指南

感谢您对 Rust 虚拟机项目的关注！我们欢迎各种形式的贡献，包括但不限于：

- 🐛 报告 Bug
- 💡 提出新功能建议
- 📝 改进文档
- 🔧 提交代码修复
- ✨ 实现新功能
- 🧪 添加测试用例

## 目录

- [快速开始](#快速开始)
- [开发环境设置](#开发环境设置)
- [质量标准速览](#质量标准速览)
- [代码提交指南](#代码提交指南)
- [代码风格](#代码风格)
- [测试要求](#测试要求)
- [文档要求](#文档要求)
- [Pull Request 流程](#pull-request-流程)
- [发布流程](#发布流程)
- [附录：质量标准详细说明](#附录质量标准详细说明)

---

## 快速开始

### 1. Fork 仓库

点击 GitHub 仓库右上角的 "Fork" 按钮，创建您自己的分支。

### 2. 克隆仓库

```bash
git clone https://github.com/YOUR_USERNAME/vm.git
cd vm
```

### 3. 创建分支

```bash
git checkout -b feature/your-feature-name
# 或
git checkout -b fix/your-bug-fix
```

---

## 开发环境设置

### 系统要求

- Rust 1.75 或更高版本
- Cargo（随 Rust 一起安装）
- Make（可选，用于运行构建脚本）
- Git 2.0 或更高版本

### 安装 Rust 工具链

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装 nightly 工具链（用于某些高级功能）
rustup install nightly

# 设置默认工具链
rustup default stable
```

### 安装开发依赖

```bash
# 运行项目设置脚本
./scripts/setup-dev.sh

# 手动安装常用工具
cargo install cargo-watch
cargo install cargo-tarpaulin  # 用于测试覆盖率
cargo install cargo-audit   # 用于安全审计
cargo install cargo-outdated # 用于检查过时的依赖
```

### 构建项目

```bash
# Debug 构建
cargo build

# Release 构建
cargo build --release

# 构建特定包
cargo build --package vm-core
```

---

## 质量标准速览

本项目实施严格的质量门控体系。在提交代码前，请确保以下所有检查项都通过：

### 必须通过的质量门（Required）

| 检查项 | 工具 | 命令 | 要求 |
|--------|------|------|------|
| **代码格式化** | rustfmt | `cargo fmt --all -- --check` | 100% 符合格式规范 |
| **Clippy 检查** | clippy | `cargo clippy --workspace --all-features --all-targets -- -D warnings` | 零警告 |
| **编译检查** | cargo | `cargo build --workspace --all-features` | 无错误无警告 |
| **测试套件** | cargo test | `cargo test --workspace --all-features` | 所有测试通过 |
| **代码覆盖率** | llvm-cov | `cargo llvm-cov --workspace --all-features` | 最低 50% |
| **文档构建** | cargo doc | `cargo doc --no-deps --workspace --all-features` | 无错误无断链 |

### 本地快速检查

在提交前，请运行以下命令：

```bash
# 1. 格式化代码
cargo fmt

# 2. 运行 Clippy（修复警告）
cargo clippy --workspace --all-features --all-targets -- -D warnings

# 3. 运行测试
cargo test --workspace --all-features

# 4. 检查覆盖率（需要先安装 cargo-llvm-cov）
cargo llvm-cov --workspace --all-features --summary

# 5. 构建文档
cargo doc --no-deps --workspace --all-features
```

### CI/CD 自动检查

所有代码在合并前必须通过 CI/CD 管道的以下质量门：

1. **quality-gates.yml** - 主要质量强制检查
2. **ci.yml** - 全面 CI 管道
3. **code-quality.yml** - 额外质量检查
4. **coverage.yml** - 覆盖率报告

详细的质量标准请参考：[QUALITY_STANDARDS.md](../QUALITY_STANDARDS.md)

---

## 代码提交指南

### 提交信息格式

我们使用 [Conventional Commits](https://www.conventionalcommits.org/) 规范：

```
<type>(<scope>): <subject>

<body>

<footer>
```

#### 类型（Type）

- `feat`: 新功能
- `fix`: Bug 修复
- `docs`: 文档更新
- `style`: 代码格式调整（不影响功能）
- `refactor`: 代码重构（既不是新功能也不是 Bug 修复）
- `perf`: 性能改进
- `test`: 添加或更新测试
- `chore`: 构建过程或辅助工具的变动
- `ci`: CI/CD 配置更改

#### 示例

```bash
feat(jit): add tiered compilation support

- Implement baseline JIT compiler
- Add optimized JIT compiler with advanced optimizations
- Add interpreter for cold code

Closes #123
```

```bash
fix(mmu): resolve page fault in address translation

The previous implementation did not correctly handle page table
walks when using 4KB pages. This fix ensures
proper alignment and flag checking.

Fixes #456
```

### 提交前检查

在提交代码前，请确保：

1. **代码通过编译**
   ```bash
   cargo check
   ```

2. **所有测试通过**
   ```bash
   cargo test
   ```

3. **代码格式正确**
   ```bash
   cargo fmt -- --check
   ```

4. **代码通过 Clippy 检查**
   ```bash
   cargo clippy -- -D warnings
   ```

5. **测试覆盖率足够**
   ```bash
   cargo tarpaulin --out Html
   ```

---

## 代码风格

### Rust 代码风格

1. **遵循 Rust 官方风格指南**
   - 使用 `cargo fmt` 自动格式化代码
   - 遵循 [Rust API 指南](https://rust-lang.github.io/api-guidelines/)

2. **命名约定**
   - 类型：`PascalCase` (例如：`VirtualMachine`)
   - 函数：`snake_case` (例如：`create_vm`)
   - 常量：`SCREAMING_SNAKE_CASE` (例如：`MAX_VCPUS`)
   - 模块：`snake_case` (例如：`device_emulation`)

3. **文档注释**
   - 公开 API 必须包含文档注释
   - 使用 `///` 表示文档注释
   - 示例：
     ```rust
     /// 创建一个新的虚拟机实例
     ///
     /// # 参数
     ///
     /// * `id` - 虚拟机唯一标识符
     /// * `config` - 虚拟机配置
     ///
     /// # 返回
     ///
     /// 返回创建的虚拟机实例
     ///
     /// # 示例
     ///
     /// ```
     /// let vm = VirtualMachine::new(id, config)?;
     /// ```
     pub fn new(id: VmId, config: VmConfig) -> Result<Self, VmError> {
         // ...
     }
     ```

4. **错误处理**
   - 使用 `Result<T, E>` 表示可能失败的操作
   - 使用 `?` 运算符传播错误
   - 使用 `thiserror` 或 `anyhow` 创建错误类型
   - 示例：
     ```rust
     use thiserror::Error;

     #[derive(Debug, Error)]
     pub enum VmError {
         #[error("内存访问违规")]
         MemoryAccessViolation,
         #[error("指令解码错误: {0}")]
         DecodeError(String),
     }
     ```

5. **并发安全**
   - 使用 `Arc` 共享只读数据
   - 使用 `Mutex` 或 `RwLock` 保护可变状态
   - 避免不必要的锁竞争
   - 考虑使用无锁数据结构（`vm-common` 模块提供了实现）

### 代码组织

遵循项目的模块结构：

```
vm/
├── vm-core/           # 核心领域模型和接口
├── vm-engine-jit/     # JIT 编译引擎
├── vm-runtime/        # 运行时环境
├── vm-mem/           # 内存管理
├── vm-cross-arch/     # 跨架构支持
└── ...
```

---

## 测试要求

### 单元测试

每个模块都应有单元测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operation() {
        // 测试代码
    }

    #[test]
    fn test_error_handling() {
        // 测试错误情况
    }
}
```

### 集成测试

集成测试放在 `tests/` 目录下：

```rust
// tests/integration_tests.rs
use vm_core::{VmConfig, VmId};

#[test]
fn test_vm_lifecycle() {
    // 集成测试代码
}
```

### 测试覆盖率

- 目标覆盖率：> 80%
- 使用 `cargo tarpaulin` 检查覆盖率
- 关键路径必须 100% 覆盖

### 性能测试

性能测试放在 `benches/` 目录下：

```rust
// benches/performance_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_function(c: &mut Criterion) {
    c.bench_function("my_function", |b| {
        b.iter(|| {
            black_box(my_function())
        });
    });
}

criterion_group!(benches, bench_function);
criterion_main!(benches);
```

---

## 文档要求

### API 文档

所有公开 API 必须有文档注释：

```rust
/// JIT 编译器接口
///
/// 提供将中间表示（IR）编译为目标机器码的功能
pub trait JITCompiler {
    /// 编译 IR 块为机器码
    ///
    /// # 参数
    ///
    /// * `ir_block` - 要编译的 IR 块
    ///
    /// # 返回
    ///
    /// 返回编译后的代码和元数据
    fn compile(&mut self, ir_block: &IRBlock) -> Result<CompiledCode, CompileError>;
}
```

### 架构文档

关键模块应有架构文档：

- `vm-core/ARCHITECTURE.md`
- `vm-engine-jit/ARCHITECTURE.md`
- `vm-runtime/ARCHITECTURE.md`
- `vm-cross-arch/ARCHITECTURE.md`

### 更新文档

- 添加新功能时更新 `API_EXAMPLES.md`
- 修改 API 时更新相关文档
- 重大变更时更新 `CHANGELOG.md`

---

## Pull Request 流程

### 创建 Pull Request

1. 推送到您的 Fork
   ```bash
   git push origin feature/your-feature-name
   ```

2. 在 GitHub 上创建 Pull Request
   - 标题遵循提交信息格式
   - 描述清楚变更的目的和影响
   - 关联相关的 Issue

### PR 检查清单

提交 PR 前，请确认：

- [ ] 代码通过 `cargo check`
- [ ] 所有测试通过 `cargo test`
- [ ] 代码通过 `cargo clippy`
- [ ] 代码已格式化 `cargo fmt`
- [ ] 测试覆盖率 > 80%
- [ ] 已添加或更新相关文档
- [ ] PR 标题符合提交信息规范
- [ ] PR 描述清楚说明变更内容

### PR 审查流程

1. **自动检查**
   - CI 运行所有测试
   - 检查代码覆盖率
   - 运行安全审计

2. **人工审查**
   - 至少一位维护者审查代码
   - 可能要求修改
   - 审查通过后合并

---

## 发布流程

### 版本号

遵循 [语义化版本](https://semver.org/)：

- `MAJOR.MINOR.PATCH`
  - `MAJOR`: 不兼容的 API 变更
  - `MINOR`: 向后兼容的功能添加
  - `PATCH`: 向后兼容的 Bug 修复

### 发布步骤

1. 更新 `Cargo.toml` 中的版本号
2. 更新 `CHANGELOG.md`
3. 创建 Git 标签
   ```bash
   git tag -a v1.0.0 -m "Release version 1.0.0"
   git push origin v1.0.0
   ```
4. 发布到 crates.io
   ```bash
   cargo publish
   ```

---

## 问题报告

### Bug 报告模板

```markdown
**描述**
简明扼要地描述问题。

**复现步骤**
1. 执行 '...'
2. 点击 '....'
3. 滚动到 '....'
4. 看到错误

**预期行为**
描述您期望发生的事情。

**实际行为**
描述实际发生的事情。

**环境**
- OS: [例如：macOS 14.0]
- Rust 版本: [例如：1.75.0]
- 项目版本: [例如：0.1.0]

**附加信息**
- 日志文件
- 截图
- 其他相关信息
```

### 功能请求模板

```markdown
**问题描述**
清晰简洁地描述您想要的功能。

**为什么需要这个功能？**
解释这个功能如何帮助您或社区。

**建议的实现**
描述您认为如何实现这个功能（可选）。

**替代方案**
描述您考虑过的其他解决方案（可选）。
```

---

## 社区指南

### 行为准则

- 尊重所有贡献者
- 建设性的反馈
- 接受不同意见
- 关注项目本身，而非个人

### 获取帮助

- GitHub Issues: 报告 Bug 或请求功能
- GitHub Discussions: 一般讨论
- Discord/Slack: 实时聊天（如有）

---

## 附录：质量标准详细说明

本文档提供了质量标准的快速参考。有关完整详细说明，请参阅：

- [QUALITY_STANDARDS.md](../QUALITY_STANDARDS.md) - 完整质量标准文档
- [CI_CD_GUIDE.md](../CI_CD_GUIDE.md) - CI/CD 管道详细指南

### 质量门控工作流

项目使用以下 GitHub Actions 工作流来强制执行质量标准：

#### 1. quality-gates.yml（主要质量门）
- **触发条件**：Push/PR 到 master/main/develop 分支
- **必须通过的检查**：
  - ✅ 代码格式化（rustfmt）
  - ✅ Clippy 检查（严格模式）
  - ✅ 多平台编译（Linux/macOS/Windows）
  - ✅ 完整测试套件
  - ✅ 文档构建
  - ✅ 代码覆盖率（最低 50%）
- **可选检查**：
  - 安全审计（警告）
  - Unsafe 代码审计（信息）
  - 依赖分析（信息）

#### 2. ci.yml（全面 CI）
- 多平台测试矩阵
- Rust 版本测试（stable + MSRV）
- 所有功能组合测试

#### 3. code-quality.yml（代码质量）
- 文档覆盖率检查
- 代码复杂度分析
- 重复依赖检测
- 构建时间测量

#### 4. coverage.yml（覆盖率报告）
- 使用 cargo-llvm-cov 生成详细覆盖率报告
- 上传到 Codecov 进行趋势跟踪
- PR 中自动评论覆盖率变化

### 质量指标趋势

CI 自动跟踪以下指标的趋势：

- 📊 测试覆盖率变化
- 🔍 Clippy 警告数量
- ⏱️ 构建时间
- 📦 依赖数量
- ⚠️ Unsafe 代码行数
- 🐛 测试失败率

### 故障排除

#### 常见问题

**Q: CI 中格式化检查失败，但我本地运行 `cargo fmt` 后仍然失败？**

A: 确保使用 `--all` 标志：
```bash
cargo fmt --all
git add -A
git commit -m "fix: format code"
```

**Q: Clippy 检查失败，如何快速修复？**

A: 使用自动修复功能：
```bash
cargo clippy --workspace --all-features --all-targets -- --fix
```

**Q: 代码覆盖率不足 50%，怎么办？**

A:
1. 查看覆盖率报告识别未覆盖区域
2. 为关键路径添加测试
3. 使用 `#[cfg(test)]` 模块添加单元测试
4. 使用 `proptest` 进行属性测试

**Q: 文档构建失败，提示 broken links？**

A:
1. 检查文档中的链接格式
2. 确保所有公共 API 都正确引用
3. 使用完整路径引用：`crate::module::Item`

### 质量改进建议

如果你想帮助改进项目的质量标准：

1. **提高覆盖率阈值**
   - 在 issue 中讨论当前阈值是否合理
   - 提供数据支持（如关键路径覆盖率已经很高）
   - 创建 PR 更新 `quality-gates.yml` 中的阈值

2. **添加新的 Clippy 规则**
   - 评估新规则的收益
   - 在项目中试验
   - 更新 `.clippy.toml` 配置

3. **改进 CI/CD 效率**
   - 识别瓶颈
   - 提出缓存策略改进
   - 优化并行化

4. **增强测试策略**
   - 添加模糊测试
   - 添加属性测试
   - 改进集成测试

### 相关文档

- [CONTRIBUTOR_CI_CD_HANDBOOK.md](../CONTRIBUTOR_CI_CD_HANDBOOK.md) - 贡献者 CI/CD 手册
- [CODE_REVIEW_GUIDE.md](../CODE_REVIEW_GUIDE.md) - 代码审查指南
- [DEVELOPER_SETUP.md](../DEVELOPER_SETUP.md) - 开发环境设置
- [TESTING_GUIDE.md](../ADVANCED_TESTING_GUIDE.md) - 高级测试指南

---

## 许可证

通过贡献代码，您同意您的贡献将在与项目相同的许可证下发布。

---

再次感谢您的贡献！🙏
