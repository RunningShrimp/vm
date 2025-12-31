# 版本发布流程

本文档定义了VM项目的版本发布流程和策略。

## 目录

- [版本策略](#版本策略)
- [发布周期](#发布周期)
- [发布准备](#发布准备)
- [发布步骤](#发布步骤)
- [发布后](#发布后)
- [紧急发布流程](#紧急发布流程)
- [回滚流程](#回滚流程)

---

## 版本策略

### 语义化版本（Semver）

项目遵循 [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html) 规范：

```
MAJOR.MINOR.PATCH

示例：0.1.0
- MAJOR: 0 - 不兼容的API变更
- MINOR: 1 - 向后兼容的功能新增
- PATCH: 0 - 向后兼容的Bug修复
```

### 版本号规则

| 变更类型 | 版本影响 | 示例 | 说明 |
|---------|---------|------|------|
| **破坏性变更** | MAJOR+ | 0.1.0 → 1.0.0 | 不兼容的API变更、移除的功能 |
| **新功能** | MINOR+ | 0.1.0 → 0.2.0 | 向后兼容的新功能 |
| **Bug修复** | PATCH+ | 0.1.0 → 0.1.1 | Bug修复、小改进 |
| **预发布版本** | -PRERELEASE | 0.1.0-alpha.1 | Alpha/Beta/RC版本 |
| **构建元数据** | +METADATA | 0.1.0+20210101 | 构建信息 |

### 预发布版本标识

```
alpha: 内部测试版本，功能不稳定
beta: 公开测试版本，功能基本稳定
rc: 候选发布版本，只修复关键Bug

示例：
0.1.0-alpha.1
0.1.0-beta.1
0.1.0-rc.1
```

---

## 发布周期

### 主版本（Major）- 每年1-2次

**时机**：
- 重大架构变更
- 不兼容的API变更
- 里程碑式的新功能

**准备时间**：至少提前2个月规划
**测试周期**：至少4周的全面测试
**公告期**：至少提前1个月通知

### 次版本（Minor）- 每季度

**时机**：
- 重要的新功能
- 大型功能改进
- 性能显著提升

**准备时间**：提前4-6周规划
**测试周期**：至少2周的全面测试
**公告期**：提前2周通知

### 修订版本（Patch）- 按需

**时机**：
- Bug修复
- 安全漏洞修复
- 小型改进
- 文档更新

**准备时间**：1-2周
**测试周期**：至少1周回归测试
**发布频率**：每月1-4次

### 紧急版本（Hotfix）- 随时

**时机**：
- 关键安全漏洞
- 严重Bug导致系统崩溃
- 数据丢失或损坏风险

**准备时间**：立即
**测试周期**：至少24小时
**发布频率**：按需

---

## 发布准备

### 功能冻结

**时间**：发布前2周（Minor/Major）或1周（Patch）

**原则**：
- 停止合并新功能
- 只接受Bug修复
- 更新文档和测试

**例外**：
- 紧急安全修复
- 关键Bug修复（需维护者批准）

### 测试验证

#### 测试清单

```bash
# 1. 单元测试
cargo test --workspace

# 2. 集成测试
cargo test --workspace --test '*'

# 3. 基准测试
cargo bench --workspace

# 4. 覆盖率检查
./scripts/coverage.sh

# 5. Clippy检查
cargo clippy --workspace -- -D warnings

# 6. 格式检查
cargo fmt -- --check

# 7. 文档构建
cargo doc --workspace --no-deps

# 8. 安全审计
cargo audit
```

#### 质量标准

| 指标 | 要求 | 说明 |
|------|------|------|
| 测试通过率 | 100% | 所有测试必须通过 |
| 代码覆盖率 | ≥70% | 核心模块≥85% |
| Clippy警告 | 0 | 无clippy警告 |
| 文档完整性 | 100% | 所有public API有文档 |
| 安全漏洞 | 0 | 无已知安全漏洞 |

### 性能基准

```bash
# 运行完整基准测试套件
cargo bench --workspace

# 生成性能报告
./scripts/generate_benchmark_report.py > benchmarks/report.md

# 性能回归检测
./scripts/detect_regression.sh
```

**要求**：
- 无显著性能退化（>5%）
- 关键路径性能提升或持平
- 内存使用无明显增长

### 文档更新

**必更文档**：
- [ ] CHANGELOG.md
- [ ] README.md（如需要）
- [ ] API文档（cargo doc）
- [ ] 发布说明（Release Notes）
- [ ] 迁移指南（如有破坏性变更）

**可选更新**：
- [ ] 架构文档
- [ ] 使用指南
- [ ] 示例代码
- [ ] 性能文档

---

## 发布步骤

### 1. 创建发布分支

**适用于**: Major和Minor版本

```bash
# 从master创建发布分支
git checkout -b release/vX.Y.Z

# 示例：release/v0.2.0
```

**Patch版本和Hotfix**：直接在master分支进行

### 2. 更新版本号

#### Workspace版本

编辑 `Cargo.toml`:

```toml
[workspace.package]
version = "X.Y.Z"  # 更新此版本号
```

#### 子crate版本

所有子crate继承workspace版本，无需单独更新。

**注意**：
- 首次发布前所有crate应为0.1.0
- 发布后统一版本号
- 避免版本号混乱

### 3. 更新CHANGELOG

```bash
# 使用脚本更新CHANGELOG
./scripts/bump_version.sh minor

# 或手动编辑CHANGELOG.md
```

**CHANGELOG格式**：

```markdown
## [Unreleased]

### Added
- 新功能（待发布）

## [X.Y.Z] - YYYY-MM-DD

### Added
- 新功能1（[#123](https://github.com/example/vm/issues/123))
- 新功能2

### Fixed
- Bug修复1（[#456](https://github.com/example/vm/issues/456)）

### Performance
- 性能提升1（30% faster）
```

### 4. 运行完整测试

```bash
# 清理构建
cargo clean

# 完整测试
./scripts/test_all.sh

# 或手动执行
cargo test --workspace --all-features
cargo clippy --workspace -- -D warnings
cargo fmt -- --check
```

**所有测试必须通过才能继续**。

### 5. 生成发布包

```bash
# 构建release版本
cargo build --workspace --release

# 运行发布前的最终检查
./scripts/pre_release_check.sh

# 生成发布包
./scripts/generate_release_artifacts.sh vX.Y.Z
```

### 6. 提交变更

```bash
git add .
git commit -m "chore: Release version X.Y.Z

- 更新版本号至X.Y.Z
- 更新CHANGELOG.md
- 完成发布前测试验证"
```

### 7. 创建Git Tag

```bash
# 创建带注释的tag
git tag -a vX.Y.Z -m "Release version X.Y.Z

主要变更：
- 新增功能1
- 修复Bug2
- 性能提升3"

# 示例
git tag -a v0.2.0 -m "Release version 0.2.0

主要变更：
- 实现RISC-V M扩展（30个指令）
- JIT编译性能提升30%
- 修复内存泄漏问题"
```

### 8. 推送到GitHub

```bash
# 推送分支（如有）
git push origin release/vX.Y.Z

# 推送master分支
git push origin master

# 推送tag
git push origin vX.Y.Z
```

### 9. 创建GitHub Release

#### 方式1: 使用脚本（推荐）

```bash
./scripts/create_github_release.sh vX.Y.Z
```

#### 方式2: 手动创建

1. 访问 GitHub Releases 页面
2. 点击 "Draft a new release"
3. 填写信息：

```
Tag: vX.Y.Z
Target: master
Title: Version X.Y.Z
Description: （复制CHANGELOG中的相关部分）
```

4. 上传发布包：
   - `vm-x.y.z.tar.gz` - 源码包
   - `vm-x.y.z.zip` - Windows二进制包
   - `vm-x.y-z-linux.tar.gz` - Linux二进制包

5. 点击 "Publish release"

### 10. 发布到crates.io（可选）

```bash
# 发布workspace成员
for crate in vm-core vm-mem vm-engine; do
  cd $crate
  cargo publish
  cd ..
done

# 或使用脚本
./scripts/publish_to_crates.sh vX.Y.Z
```

**注意**：
- 使用 `--no-verify` 跳过重复发布
- 按依赖顺序发布
- 等待crates.io索引更新

---

## 发布后

### 公告发布

#### 发布渠道

1. **GitHub Release** - 已完成
2. **GitHub Discussions** - 讨论帖
3. **项目博客** - 详细文章（如有）
4. **社交媒体** - Twitter/Reddit（如有）
5. **邮件列表** - 订阅者通知（如有）

#### 公告模板

```markdown
# VM Version X.Y.Z 发布

我们很高兴地宣布 VM X.Y.Z 版本正式发布！

## 主要亮点

- 🎉 新功能1
- 🚀 性能提升2
- 🐛 Bug修复3

## 重要变更

### 破坏性变更
- 变更1（迁移指南：[链接]）

### 新功能
- 功能1
- 功能2

### Bug修复
- 修复1（[#123](https://github.com/example/vm/issues/123)）

## 升级指南

```bash
# 更新到最新版本
cargo update vm
```

详细迁移指南请查看：[链接]

## 下载

- GitHub Release: [链接]
- Cargo: `cargo install vm --version X.Y.Z`
- crates.io: [链接]

## 贡献者

感谢以下贡献者：
- @contributor1
- @contributor2

## 完整变更日志

查看 [CHANGELOG.md](https://github.com/example/vm/blob/master/CHANGELOG.md)

## 反馈

如有问题请在 [GitHub Issues](https://github.com/example/vm/issues) 反馈。
```

### 更新文档

**立即更新**：
- [ ] 版本号文档
- [ ] 下载链接
- [ ] 升级指南

**后续更新**：
- [ ] 示例代码
- [ ] 教程文档
- [ ] API参考

### 监控问题

**发布后72小时密切监控**：

1. **GitHub Issues**
   - 新问题报告
   - 发布相关问题
   - 回退请求

2. **GitHub Discussions**
   - 用户讨论
   - 使用问题
   - 功能请求

3. **性能监控**
   - Benchmarks
   - CI/CD通过率
   - 用户反馈

4. **安全报告**
   - 新发现的安全漏洞
   - 依赖项安全问题

**应急响应**：
- 关键Bug：24小时内发布hotfix
- 安全漏洞：立即评估并修复
- 文档问题：48小时内更新

---

## 紧急发布流程

### 触发条件

- 关键安全漏洞（CVE级别）
- 数据丢失或损坏风险
- 系统崩溃影响>50%用户
- 依赖项严重问题

### 流程

**时间要求**：24-48小时内完成

#### 1. 紧急评估（1小时）

- 召集维护者会议
- 确认问题严重性
- 制定修复方案

#### 2. 创建Hotfix分支（立即）

```bash
git checkout -b hotfix/vX.Y.Z+1
```

#### 3. 实施修复（2-4小时）

- 修复问题
- 添加测试用例
- 最小化变更范围

#### 4. 快速测试（1-2小时）

```bash
# 只运行相关测试
cargo test -p vm-core

# 运行CI
# （等待CI通过）
```

#### 5. 快速发布（1小时）

```bash
# 更新版本号
vim Cargo.toml  # X.Y.Z -> X.Y.Z+1

# 更新CHANGELOG
vim CHANGELOG.md

# 提交和发布
git commit -m "hotfix: 修复XXX紧急问题"
git tag -a vX.Y.Z+1 -m "Hotfix: XXX"
git push origin hotfix/vX.Y.Z+1
git push origin vX.Y.Z+1

# 创建GitHub Release
# （手动或脚本）
```

#### 6. 公告（立即）

- GitHub Release
- 安全公告（如适用）
- 邮件列表（如适用）

#### 7. 后续跟进

- 合并hotfix到master
- 计划下一个版本包含完整修复
- 更新文档

---

## 回滚流程

### 触发条件

- 发布后发现严重Bug
- 影响范围>30%用户
- 无法快速修复

### 回滚步骤

#### 1. 评估回滚必要性（30分钟）

- 确认问题严重性
- 评估修复vs回滚
- 获得维护者共识

#### 2. 发布回滚公告（立即）

```markdown
# 回滚公告：版本 X.Y.Z

由于发现严重问题[描述]，我们决定回滚版本 X.Y.Z。

受影响用户请回退到版本 X.Y.Z-1：

```bash
cargo install vm --version X.Y.Z-1
```

我们将在[时间]发布修复版本 X.Y.Z+1。

我们深表歉意。
```

#### 3. 从crates.io yank版本（可选）

```bash
# Yank版本（不会删除已下载的）
cargo yank vm X.Y.Z

# 如需恢复
cargo yank --undo vm X.Y.Z
```

#### 4. 修复并重新发布

- 在hotfix分支修复问题
- 全面测试
- 作为X.Y.Z+1重新发布

#### 5. 事后分析

- 编写事故报告
- 分析根本原因
- 改进发布流程
- 更新检查清单

---

## 工具和脚本

### 可用脚本

```bash
# 版本号更新
./scripts/bump_version.sh [major|minor|patch]

# 发布前检查
./scripts/pre_release_check.sh

# 生成发布包
./scripts/generate_release_artifacts.sh vX.Y.Z

# 创建GitHub Release
./scripts/create_github_release.sh vX.Y.Z

# 发布到crates.io
./scripts/publish_to_crates.sh vX.Y.Z
```

### 自动化

推荐使用GitHub Actions自动化部分流程：

- [ ] 自动运行测试
- [ ] 自动生成发布包
- [ ] 自动创建GitHub Release
- [ ] 自动发布到crates.io（需批准）

---

## 最佳实践

### 发布前

1. **提前规划**：至少提前2周规划Major/Minor版本
2. **功能冻结**：严格遵守功能冻结期
3. **全面测试**：不要跳过任何测试步骤
4. **文档优先**：提前准备好迁移指南
5. **性能基准**：确保无性能退化

### 发布中

1. **小步快跑**：频繁的Patch版本好于大的Minor版本
2. **自动化优先**：使用脚本减少人为错误
3. **双重检查**：版本号、CHANGELOG、tag
4. **保持冷静**：遇到问题不要慌张
5. **记录一切**：详细记录发布过程

### 发布后

1. **快速响应**：72小时内密切监控问题
2. **及时公告**：重大问题立即发布公告
3. **持续改进**：每次发布后改进流程
4. **感谢贡献**：公开感谢所有贡献者
5. **收集反馈**：主动收集用户反馈

---

## 参考资源

- [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html)
- [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)
- [GitHub Releases Guide](https://docs.github.com/en/repositories/releasing-projects-on-github)
- [cargo publish](https://doc.rust-lang.org/cargo/commands/cargo-publish.html)

---

**文档版本**: 1.0.0
**最后更新**: 2025-12-31
**维护者**: VM Development Team
