# Version {{VERSION}} Release Notes

**Release Date**: {{YYYY-MM-DD}}
**Download**: [GitHub Releases](https://github.com/example/vm/releases/tag/v{{VERSION}})
**Documentation**: [API Docs](https://docs.rs/vm/{{VERSION}}/vm)

---

## 🎉 Highlights

本版本的主要亮点：

- **亮点1**: 简短描述最重要的新功能或改进
- **亮点2**: 另一个重要的功能
- **亮点3**: 性能提升或重要修复

{{EXAMPLE_HIGHLIGHTS}}

---

## ✨ New Features

### 功能类别1

- **功能名称1**: 简短描述新功能 ([#123](https://github.com/example/vm/issues/123))
  - 详细说明
  - 使用示例（如适用）
  - 相关文档链接

- **功能名称2**: 简短描述 ([#456](https://github.com/example/vm/pull/456))

### 功能类别2

- **功能名称3**: 简短描述

{{EXAMPLE_NEW_FEATURES}}

---

## 🚀 Improvements

### 性能优化

- **优化1**: JIT编译性能提升30% ([#789](https://github.com/example/vm/pull/789))
  - 改进前：____ ops/s
  - 改进后：____ ops/s
  - 提升幅度：____%

- **优化2**: TLB缓存命中率提升15%

### 代码质量

- **改进1**: 重构XXX模块，提高可维护性
- **改进2**: 改进错误处理和错误消息
- **改进3**: 优化内存使用，减少20%内存占用

### 用户体验

- **改进1**: 改进CLI界面和交互
- **改进2**: 添加更多示例代码
- **改进3**: 改进错误提示信息

{{EXAMPLE_IMPROVEMENTS}}

---

## 🐛 Bug Fixes

### 严重Bug

- **修复1**: 修复内存泄漏问题 ([#101](https://github.com/example/vm/issues/101))
  - 影响：长时间运行后内存持续增长
  - 修复：正确释放资源

- **修复2**: 修复JIT编译导致的崩溃 ([#202](https://github.com/example/vm/issues/202))
  - 影响：特定指令组合触发崩溃
  - 修复：修正寄存器分配逻辑

### 一般Bug

- **修复3**: 修复VirtIO块设备I/O错误 ([#303](https://github.com/example/vm/pull/303))
- **修复4**: 修复ARM64特定指令的解码问题
- **修复5**: 修复文档中的错误示例

{{EXAMPLE_BUG_FIXES}}

---

## ⚠️ Breaking Changes

### 变更1: API重命名

**影响范围**: 用户使用XXX API的代码

**变更前**:
```rust
fn old_api_name(&self) -> Result<Type>;
```

**变更后**:
```rust
fn new_api_name(&self) -> Result<Type>;
```

**迁移指南**:
1. 搜索所有 `old_api_name` 使用
2. 替换为 `new_api_name`
3. 运行测试验证

**详细迁移文档**: [链接到迁移指南](MIGRATION_GUIDE.md)

### 变更2: 行为变更

**影响范围**: XXX功能的默认行为

**变更前**: 行为描述
**变更后**: 新行为描述

**影响**: 如果您的代码依赖旧行为，需要调整

**迁移指南**:
```rust
// 旧代码
let result = vm.foo();

// 新代码
let result = vm.foo().with_option(NewOption);
```

{{EXAMPLE_BREAKING_CHANGES}}

---

## 🔒 Security Fixes

- **安全修复1**: 修复XXX安全漏洞 (CVE-2025-XXXXX)
  - 严重性: 高/中/低
  - 影响: 描述影响
  - 修复: 描述修复方案
  - 建议: 升级到此版本

{{EXAMPLE_SECURITY_FIXES}}

---

## 📚 Documentation

- 新增 [XXX指南](docs/XXX.md)
- 更新 [API文档](https://docs.rs/vm/{{VERSION}}/vm)
- 新增 [教程](docs/tutorials/XXX.md)
- 改进 [示例代码](examples/XXX.rs)
- 新增 [性能调优指南](docs/PERFORMANCE.md)

---

## 🔄 Deprecations

以下功能在本版本中标记为废弃，将在未来版本中移除：

- **功能1**: 将在0.2.0版本中移除
  - 替代方案: 使用新功能YYY
  - 时间线: 0.1.x版本支持，0.2.0移除

- **API2**: 将在1.0.0版本中移除
  - 替代方案: 使用新API
  - 迁移指南: [链接]

{{EXAMPLE_DEPRECATIONS}}

---

## 🧪 Testing

### 测试覆盖率

- 整体覆盖率: XX% (提升X%)
- 核心模块覆盖率: XX%
- 新增测试用例: XX个

### 测试矩阵

| 平台 | Rust版本 | 状态 |
|------|----------|------|
| Linux x86_64 | 1.85, Stable | ✅ |
| macOS x86_64 | 1.85, Stable | ✅ |
| macOS ARM64 | 1.85, Stable | ✅ |
| Windows x86_64 | 1.85, Stable | ✅ |

---

## 📦 Installation

### From crates.io

```bash
cargo install vm --version {{VERSION}}
```

### From Source

```bash
git clone https://github.com/example/vm.git
cd vm
git checkout v{{VERSION}}
cargo build --release
cargo install --path .
```

### From Binaries

下载预编译二进制文件：

- [Linux x86_64](https://github.com/example/vm/releases/download/v{{VERSION}}/vm-{{VERSION}}-linux-x86_64.tar.gz)
- [macOS x86_64](https://github.com/example/vm/releases/download/v{{VERSION}}/vm-{{VERSION}}-macos-x86_64.tar.gz)
- [macOS ARM64](https://github.com/example/vm/releases/download/v{{VERSION}}/vm-{{VERSION}}-macos-aarch64.tar.gz)
- [Windows x86_64](https://github.com/example/vm/releases/download/v{{VERSION}}/vm-{{VERSION}}-windows-x86_64.zip)

### Docker

```bash
docker pull example/vm:{{VERSION}}
```

---

## 🚀 Quick Start

```rust
use vm::{VirtualMachine, Config};

fn main() -> vm::Result<()> {
    let config = Config::default();
    let mut vm = VirtualMachine::new(config)?;

    // 加载程序
    vm.load_program("path/to/program")?;

    // 运行
    vm.run()?;

    Ok(())
}
```

更多示例: [examples/](https://github.com/example/vm/tree/v{{VERSION}}/examples)

---

## 🔄 Upgrade Guide

### From 0.X.X to {{VERSION}}

#### 步骤1: 更新依赖

```toml
# Cargo.toml
[dependencies]
vm = "{{VERSION}}"
```

#### 步骤2: 运行更新

```bash
cargo update
```

#### 步骤3: 处理破坏性变更

如果有破坏性变更，请参考 [Breaking Changes](#breaking-changes) 部分

#### 步骤4: 运行测试

```bash
cargo test
```

#### 步骤5: 构建和验证

```bash
cargo build --release
```

详细迁移指南: [MIGRATION.md](https://github.com/example/vm/blob/v{{VERSION}}/MIGRATION.md)

---

## ⚠️ Known Issues

- **已知问题1**: 描述问题 ([#404](https://github.com/example/vm/issues/404))
  - 影响: 受影响的场景
  - 临时方案: 临时解决方案
  - 修复计划: 预计在X.X.X版本修复

- **已知问题2**: 描述问题
  - 影响: 受影响的场景
  - 临时方案: 临时解决方案

{{EXAMPLE_KNOWN_ISSUES}}

---

## 🙏 Contributors

感谢以下贡献者对本版本的贡献：

- [@contributor1](https://github.com/contributor1) - 主要功能1
- [@contributor2](https://github.com/contributor2) - Bug修复
- [@contributor3](https://github.com/contributor3) - 文档改进
- [@yourname](https://github.com/yourname) - 你的贡献

**统计数据**:
- 参与人数: XX
- 提交数: XXX
- PRs合并: XX
- Issues关闭: XX

---

## 📊 What's Next

### 下一版本计划 (0.X.0)

计划中的功能：

- [ ] RISC-V C扩展实现
- [ ] ARM SVE支持
- [ ] 更多设备模拟
- [ ] 性能优化

路线图: [ROADMAP.md](https://github.com/example/vm/blob/master/ROADMAP.md)

---

## 💬 Feedback

### 问题报告

遇到问题？请在 [GitHub Issues](https://github.com/example/vm/issues) 报告

### 功能请求

有好想法？请在 [GitHub Issues](https://github.com/example/vm/issues/new?template=feature_request.md) 提出

### 讨论

加入讨论: [GitHub Discussions](https://github.com/example/vm/discussions)

---

## 📖 Full Changelog

查看完整的变更列表: [CHANGELOG.md](https://github.com/example/vm/blob/v{{VERSION}}/CHANGELOG.md)

主要变更类别:
- ✨ New Features: XX
- 🚀 Improvements: XX
- 🐛 Bug Fixes: XX
- ⚠️ Breaking Changes: XX
- 🔒 Security Fixes: XX
- 📚 Documentation: XX

---

## 🔗 Links

- [Website](https://example.com)
- [Documentation](https://docs.rs/vm/{{VERSION}}/vm)
- [GitHub Repository](https://github.com/example/vm)
- [crates.io](https://crates.io/crates/vm)
- [Examples](https://github.com/example/vm/tree/v{{VERSION}}/examples)
- [Contributing](https://github.com/example/vm/blob/master/CONTRIBUTING.md)

---

**Previous Release**: [vX.Y.Z](https://github.com/example/vm/releases/tag/vX.Y.Z)
**Next Release**: 计划于 YYYY-MM-DD

---

**Release Date**: {{YYYY-MM-DD}}
**Git Tag**: [v{{VERSION}}](https://github.com/example/vm/tree/v{{VERSION}})
**Commit**: [SHA](https://github.com/example/vm/commit/SHA)

---

## 📝 License

此版本继续使用 [MIT OR Apache-2.0](https://github.com/example/vm/blob/master/LICENSE) 许可证。

---

**VM Project** - 高性能虚拟机模拟器
*Fast, Flexible, and Extensible Virtual Machine for RISC-V and ARM64*
