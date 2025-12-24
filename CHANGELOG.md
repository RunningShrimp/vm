# 变更日志

本项目遵循 [语义化版本](https://semver.org/) 规范（`MAJOR.MINOR.PATCH`）。

## [未发布]

### 计划的功能
- ...

### 已知问题
- ...

---

## 版本模板

使用以下模板添加新版本的变更：

```markdown
## [版本号] - 发布日期

### 新增
- 简短描述新功能
- ...

### 改进
- 简短描述改进
- ...

### 修复
- 简短描述 Bug 修复
- ...

### 破坏性变更
- 描述任何不兼容的 API 变更
- 迁移指南链接（如有）

### 移除
- 简短描述移除的功能
- ...

### 安全修复
- 简短描述安全修复
- ...

### 性能
- 简短描述性能改进
- ...

### 文档
- 简短描述文档更新
- ...

### 其他
- 其他值得注意的变更
- ...
```

---

## 变更类别说明

### 新增（Added）
新功能、新接口、新 API

### 改进（Changed）
现有功能的改进、优化

### 修复（Fixed）
Bug 修复

### 破坏性变更（Breaking）
不兼容的 API 变更、移除的功能

### 移除（Removed）
移除的功能、废弃的 API

### 安全修复（Security）
安全漏洞修复

### 性能（Performance）
性能改进、优化

### 文档（Docs）
文档更新、改进

### 其他（Other）
其他值得注意的变更

---

## 发布流程

1. 更新 `CHANGELOG.md`
2. 更新 `Cargo.toml` 中的版本号
3. 创建 Git 标签
   ```bash
   git tag -a vX.Y.Z -m "Release version X.Y.Z"
   git push origin vX.Y.Z
   ```
4. 发布到 crates.io
   ```bash
   cargo publish
   ```
5. 创建 GitHub Release
   - 复制 `CHANGELOG.md` 中的变更内容
   - 附上下载链接和签名（如有）
