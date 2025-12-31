# 发布快速开始指南

本指南提供VM项目发布的快速步骤。

## 📋 发布前准备

### 1. 确保所有测试通过

```bash
# 运行所有检查
./scripts/pre_release_check.sh
```

### 2. 更新CHANGELOG.md

在`CHANGELOG.md`中添加新版本条目：

```markdown
## [0.2.0] - 2025-01-15

### Added
- 新功能1
- 新功能2

### Fixed
- Bug修复1
- Bug修复2
```

### 3. 完成发布检查清单

使用 `.github/RELEASE_CHECKLIST.md` 确保所有项目完成。

---

## 🚀 发布流程

### 方式1: 自动化发布（推荐）

#### 步骤1: 更新版本号

```bash
# 更新版本号并创建Git提交和tag
./scripts/bump_version.sh minor  # major/minor/patch
```

这个脚本会：
- 更新 `Cargo.toml` 中的版本号
- 更新 `CHANGELOG.md`
- 创建Git提交
- 创建Git tag

#### 步骤2: 推送tag

```bash
git push origin master
git push origin v0.2.0
```

#### 步骤3: GitHub Actions自动发布

推送tag后，GitHub Actions会自动：
- ✅ 运行完整测试套件
- ✅ 构建多平台二进制文件
- ✅ 创建GitHub Release
- ✅ 发布到crates.io（可选）

#### 步骤4: 验证发布

访问 [GitHub Releases](https://github.com/example/vm/releases) 验证。

### 方式2: 手动发布

#### 步骤1: 更新版本号

```bash
./scripts/bump_version.sh minor
```

#### 步骤2: 运行发布前检查

```bash
./scripts/pre_release_check.sh
```

#### 步骤3: 推送变更

```bash
git push origin master
git push origin v0.2.0
```

#### 步骤4: 创建GitHub Release

```bash
./scripts/create_github_release.sh 0.2.0
```

#### 步骤5: 发布到crates.io（可选）

```bash
./scripts/publish_to_crates.sh 0.2.0
```

---

## 📝 发布版本类型

### Major版本 (重大更新)

```bash
./scripts/bump_version.sh major
# 0.1.0 -> 1.0.0
```

**适用于**:
- 不兼容的API变更
- 架构重构
- 里程碑式的新功能

### Minor版本 (新功能)

```bash
./scripts/bump_version.sh minor
# 0.1.0 -> 0.2.0
```

**适用于**:
- 向后兼容的新功能
- 大型功能改进
- 性能显著提升

### Patch版本 (Bug修复)

```bash
./scripts/bump_version.sh patch
# 0.1.0 -> 0.1.1
```

**适用于**:
- Bug修复
- 小型改进
- 文档更新

---

## 🔍 发布后验证

### 1. 验证GitHub Release

访问发布页面检查：
- [ ] Release说明正确
- [ ] 附件文件完整
- [ ] 链接有效

### 2. 验证crates.io（如果发布）

```bash
# 检查包是否可用
cargo search vm

# 测试安装
cargo install vm --version 0.2.0
```

### 3. 监控问题

发布后72小时内：
- [ ] 监控GitHub Issues
- [ ] 监控GitHub Discussions
- [ ] 响应用户反馈

---

## 🆘 紧急发布（Hotfix）

如果发现严重问题需要快速修复：

```bash
# 1. 创建hotfix分支
git checkout -b hotfix/v0.1.1

# 2. 修复问题
# ... 进行修复 ...

# 3. 更新版本号
./scripts/bump_version.sh patch

# 4. 快速发布
git push origin master
git push origin v0.1.1

# 5. 创建release
./scripts/create_github_release.sh 0.1.1
```

详细紧急发布流程：[docs/RELEASE_PROCESS.md](RELEASE_PROCESS.md)

---

## 📚 相关文档

- **[完整发布流程](RELEASE_PROCESS.md)** - 详细的发布策略和流程
- **[发布检查清单](../.github/RELEASE_CHECKLIST.md)** - 发布前检查清单
- **[Release Notes模板](../.github/RELEASE_NOTES_TEMPLATE.md)** - 发布说明模板
- **[CHANGELOG.md](../CHANGELOG.md)** - 版本变更日志

---

## 🔧 常用命令

### 查看当前版本

```bash
grep 'version =' Cargo.toml
```

### 查看最近的tag

```bash
git tag -l --sort=-v:refname | head -n 5
```

### 比较两个版本

```bash
git diff v0.1.0 v0.2.0
```

### 查看版本提交历史

```bash
git log v0.1.0..v0.2.0 --oneline
```

### 撤销本地tag

```bash
git tag -d v0.2.0
```

### 删除远程tag

```bash
git push origin :refs/tags/v0.2.0
```

### Yank crates.io版本

```bash
cargo yank vm 0.2.0
```

---

## ⚠️ 常见问题

### Q: 如何回滚已发布的版本？

A: 参考 [docs/RELEASE_PROCESS.md](RELEASE_PROCESS.md#回滚流程)

### Q: 发布后发现Bug怎么办？

A: 根据严重性决定：
- 小Bug：等待下一个patch版本
- 严重Bug：创建hotfix
- 致命Bug：考虑yank版本

### Q: 如何预览发布内容？

A: 使用 `--dry-run` 选项：

```bash
./scripts/bump_version.sh minor --dry-run
./scripts/create_github_release.sh 0.2.0 --draft
```

### Q: 多久发布一次？

A:
- **Patch**: 按需（每周1-4次）
- **Minor**: 每季度
- **Major**: 每年1-2次

---

## 📞 获取帮助

- **Issues**: [GitHub Issues](https://github.com/example/vm/issues)
- **Discussions**: [GitHub Discussions](https://github.com/example/vm/discussions)
- **文档**: [完整文档](../README.md#文档)

---

**快速开始指南版本**: 1.0.0
**最后更新**: 2025-12-31
