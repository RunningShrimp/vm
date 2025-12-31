# 发布检查清单

版本发布前必须完成所有检查项。

**版本**: v[填写版本号]
**发布日期**: [填写日期]
**发布负责人**: [填写姓名]

---

## 📋 发布前检查

### 代码质量

- [ ] **所有测试通过**
  ```bash
  cargo test --workspace --all-features
  ```
  - [ ] 单元测试通过
  - [ ] 集成测试通过
  - [ ] 文档测试通过
  - 测试通过率: ____ / 100%

- [ ] **代码覆盖率达标**
  ```bash
  ./scripts/coverage.sh
  ```
  - [ ] 整体覆盖率 ≥ 70%
  - [ ] 核心模块覆盖率 ≥ 85%
  - 实际覆盖率: ____ %

- [ ] **Clippy检查无警告**
  ```bash
  cargo clippy --workspace -- -D warnings
  ```
  - 警告数: 0

- [ ] **代码格式正确**
  ```bash
  cargo fmt -- --check
  ```
  - 格式问题: 0

- [ ] **文档完整**
  ```bash
  cargo doc --workspace --no-deps
  ```
  - [ ] 所有public API有文档注释
  - [ ] 无文档警告
  - [ ] 示例代码可运行

- [ ] **编译无错误和警告**
  ```bash
  cargo build --workspace --all-features
  ```
  - 编译错误: 0
  - 编译警告: 0

### 安全审计

- [ ] **依赖项安全审计**
  ```bash
  cargo audit
  ```
  - [ ] 无已知安全漏洞
  - 漏洞数: ____

- [ ] **许可证检查**
  ```bash
  cargo install cargo-license
  cargo license
  ```
  - [ ] 所有依赖许可证兼容
  - [ ] MIT/Apache-2.0兼容

- [ ] **秘密信息检查**
  ```bash
  git grep -i "password\|secret\|api_key\|token"
  ```
  - [ ] 无硬编码的秘密信息

### 性能测试

- [ ] **基准测试运行**
  ```bash
  cargo bench --workspace
  ```
  - [ ] 关键性能指标无退化
  - [ ] 性能提升文档化

- [ ] **内存泄漏检查**
  ```bash
  valgrind ./target/debug/vm-example
  ```
  - [ ] 无内存泄漏
  - [ ] 无未释放资源

- [ ] **性能对比报告**
  ```bash
  ./scripts/detect_regression.sh
  ```
  - [ ] 性能回归 < 5%
  - [ ] 关键路径性能提升

### 兼容性测试

- [ ] **平台兼容性**
  - [ ] Linux (x86_64)
  - [ ] macOS (x86_64/ARM64)
  - [ ] Windows (x86_64)
  - 失败平台: ____

- [ ] **Rust版本兼容性**
  ```bash
  # 测试最低支持的Rust版本
  rustup install 1.85
  rustup override set 1.85
  cargo build
  ```
  - [ ] 最低Rust版本: 1.85
  - [ ] 在MSRV上编译通过

### 文档更新

- [ ] **CHANGELOG.md更新**
  - [ ] 版本号正确
  - [ ] 变更类型分类正确
  - [ ] 变更描述详细
  - [ ] Issue/PR链接添加
  - [ ] 发布日期填写

- [ ] **README.md更新**
  - [ ] 当前版本信息
  - [ ] 新功能说明
  - [ ] 下载链接
  - [ ] 升级指南

- [ ] **API文档更新**
  ```bash
  cargo doc --workspace --no-deps --open
  ```
  - [ ] 新增API有文档
  - [ ] 变更API有说明
  - [ ] 示例代码更新

- [ ] **迁移指南**（如有破坏性变更）
  - [ ] 破坏性变更说明
  - [ ] 迁移步骤清晰
  - [ ] 代码示例完整
  - [ ] 测试验证通过

---

## 📦 发布包检查

### 构建检查

- [ ] **Release构建成功**
  ```bash
  cargo build --workspace --release
  ```
  - [ ] 优化级别正确（opt-level = 3）
  - [ ] LTO启用
  - [ ] 二进制大小合理

- [ ] **多平台构建**
  - [ ] Linux x86_64
  - [ ] macOS x86_64
  - [ ] macOS ARM64
  - [ ] Windows x86_64

### 发布包

- [ ] **源码包**
  ```bash
  git archive --format=tar.gz --prefix=vm-X.Y.Z/ v{X.Y.Z} > vm-X.Y.Z.tar.gz
  ```
  - [ ] 文件完整性验证
  - [ ] SHA256校验和
  - [ ] GPG签名（可选）

- [ ] **二进制包**
  - [ ] Linux二进制
  - [ ] macOS二进制
  - [ ] Windows二进制
  - [ ] 可执行权限正确

- [ ] **包大小检查**
  - Linux: ____ MB
  - macOS: ____ MB
  - Windows: ____ MB

---

## 🚀 发布流程

### Git操作

- [ ] **版本号更新**
  ```bash
  ./scripts/bump_version.sh [major|minor|patch]
  ```
  - [ ] Cargo.toml版本号
  - [ ] 所有crate版本号一致

- [ ] **Git提交创建**
  ```bash
  git commit -m "chore: Release version X.Y.Z"
  ```
  - [ ] 提交信息规范
  - [ ] 变更完整

- [ ] **Git Tag创建**
  ```bash
  git tag -a vX.Y.Z -m "Release version X.Y.Z"
  ```
  - [ ] Tag名称格式正确（vX.Y.Z）
  - [ ] Tag注释完整

- [ ] **推送到远程**
  ```bash
  git push origin master
  git push origin vX.Y.Z
  ```
  - [ ] 提交推送成功
  - [ ] Tag推送成功

### GitHub Release

- [ ] **Release创建**
  - [ ] Tag选择正确
  - [ ] Release标题格式: "Version X.Y.Z"
  - [ ] Release说明完整
    - 主要亮点
    - 新功能列表
    - Bug修复列表
    - 破坏性变更
    - 升级指南
    - 已知问题

- [ ] **发布包上传**
  - [ ] 源码包（.tar.gz）
  - [ ] Linux二进制
  - [ ] macOS二进制
  - [ ] Windows二进制
  - [ ] SHA256校验和文件

- [ ] **链接添加**
  - [ ] CHANGELOG.md链接
  - [ ] 升级指南链接
  - [ ] 文档链接

### Crates.io发布（可选）

- [ ] **发布到crates.io**
  ```bash
  ./scripts/publish_to_crates.sh X.Y.Z
  ```
  - [ ] 按依赖顺序发布
  - [ ] 所有crate发布成功
  - [ ] 验证发布可用

---

## 📢 发布后检查

### 公告和宣传

- [ ] **GitHub公告**
  - [ ] GitHub Release发布
  - [ ] GitHub Discussions创建
  - [ ] 更新README版本徽章

- [ ] **社区公告**（如适用）
  - [ ] 项目博客文章
  - [ ] 邮件列表通知
  - [ ] 社交媒体发布（Twitter/Reddit）
  - [ ] 中文社区公告

- [ ] **文档更新**
  - [ ] 官网版本号更新
  - [ ] API文档链接更新
  - [ ] 示例代码更新

### 监控和支持

- [ ] **问题监控**（72小时）
  - [ ] GitHub Issues监控
  - [ ] 新问题及时响应
  - [ ] 严重问题快速处理

- [ ] **性能监控**
  - [ ] Benchmarks运行
  - [ ] CI/CD通过率
  - [ ] 用户反馈收集

- [ ] **依赖更新**
  - [ ] 更新依赖分支
  - [ ] 文档依赖版本

---

## ✅ 发布确认

**发布负责人签名**: ________________

**发布日期**: ________________

**发布版本**: v____________

**发布状态**:
- [ ] ✅ 成功
- [ ] ⚠️ 部分成功（附注: ____）
- [ ] ❌ 失败（原因: ____）

**备注**:

---

## 🔙 回滚计划（如需要）

如果发布后出现严重问题，执行回滚：

- [ ] **从crates.io yank版本**
  ```bash
  cargo yank vm X.Y.Z
  ```

- [ ] **发布回滚公告**
  - [ ] GitHub Release公告
  - [ ] 邮件列表通知

- [ ] **创建hotfix分支**
  ```bash
  git checkout -b hotfix/vX.Y.Z+1
  ```

- [ ] **修复并重新发布**

---

## 📊 发布统计

**开发周期**: ____ 天
**提交数**: ____ 个
** contributors**: ____ 人
** issues关闭**: ____ 个
** PRs合并**: ____ 个
** 新功能**: ____ 个
** Bug修复**: ____ 个
** 文档更新**: ____ 个
** 性能提升**: ____ %

---

**检查清单版本**: 1.0.0
**最后更新**: 2025-12-31
**维护者**: VM Development Team
