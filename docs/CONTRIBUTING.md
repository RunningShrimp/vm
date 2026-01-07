# 贡献指南

感谢您对VM项目的关注！我们欢迎各种形式的贡献。

---

## 📋 目录

- [行为准则](#行为准则)
- [如何贡献](#如何贡献)
- [开发工作流](#开发工作流)
- [代码规范](#代码规范)
- [提交规范](#提交规范)
- [Pull Request流程](#pull-request流程)
- [获得帮助](#获得帮助)

---

## 🤝 行为准则

### 我们的承诺

为了营造开放和友好的环境，我们承诺：

- 🌈 **尊重差异**: 欢迎不同背景和观点
- 💬 **友好沟通**: 使用包容和友好的语言
- 🎯 **专注协作**: 关注什么对社区最好
- 👏 **表示感谢**: 感谢贡献者的努力

### 不可接受的行为

- ❌ 性别歧视、种族歧视等
- ❌ 骚扰、人身攻击
- ❌ 发布他人隐私信息
- ❌ 其他不专业或不恰当的行为

**报告**: 如遇问题，请联系 your-email@example.com

---

## 🚀 如何贡献

### 贡献类型

我们欢迎以下类型的贡献:

- 🐛 **修复Bug**
- ✨ **新功能**
- 📝 **文档改进**
- 🌐 **翻译**
- 🎨 **代码重构**
- ⚡ **性能优化**
- 🧪 **添加测试**
- 🔍 **代码审查**

### 开始之前

1. **检查现有Issue**: [GitHub Issues](https://github.com/your-org/vm/issues)
2. **讨论大改动**: 创建Issue或Discussion讨论
3. **寻找好的第一任务**: 标签为 `good first issue` 的问题

---

## 🛠️ 开发工作流

### 1. Fork和Clone

```bash
# Fork仓库到您的GitHub账号
# 然后克隆您的fork
git clone https://github.com/YOUR_USERNAME/vm.git
cd vm

# 添加上游仓库
git remote add upstream https://github.com/original-org/vm.git
```

### 2. 创建分支

```bash
# 从main创建新分支
git checkout main
git pull upstream main
git checkout -b feature/your-feature-name

# 或修复bug
git checkout -b fix/your-bug-fix

# 或文档
git checkout -b docs/your-doc-update
```

**分支命名规范**:
- `feature/` - 新功能
- `fix/` - Bug修复
- `refactor/` - 重构
- `docs/` - 文档更新
- `test/` - 测试相关
- `perf/` - 性能优化
- `chore/` - 构建/工具相关

### 3. 进行更改

```bash
# 进行您的更改
# ... 编辑代码 ...

# 运行测试
cargo test --workspace

# 运行Clippy
cargo clippy --workspace -- -D warnings

# 格式化代码
cargo fmt --all

# 检查编译
cargo build --workspace
```

### 4. 提交更改

```bash
# 添加更改的文件
git add path/to/changed/files

# 或添加所有更改
git add .

# 提交 (使用语义化提交消息)
git commit -m "feat: add amazing new feature"
```

### 5. 同步和推送

```bash
# 从上游同步
git fetch upstream main
git rebase upstream/main

# 推送到您的fork
git push origin feature/your-feature-name
```

### 6. 创建Pull Request

1. 访问 GitHub: https://github.com/original-org/vm
2. 点击 "Compare & pull request"
3. 填写PR模板
4. 等待审查

---

## 📝 代码规范

### Rust代码风格

我们遵循标准的Rust代码风格:

```bash
# 格式化代码
cargo fmt --all

# 检查格式
cargo fmt --all -- --check
```

### Clippy检查

```bash
# 运行Clippy
cargo clippy --workspace -- -D warnings

# 自动修复简单问题
cargo clippy --workspace --fix
```

**常见的Clippy警告**:
- 未使用的导入
- 未使用的变量
- 可以简化的表达式
- 性能问题

### 代码组织

**文件结构**:
```rust
// 1. License和文档注释
//! 模块文档

// 2. 导入 (按字母序)
use std::collections::HashMap;
use crate::module::Type;

// 3. 类型定义
pub struct MyStruct {
    // ...
}

// 4. Trait实现
impl MyStruct {
    pub fn new() -> Self {
        // ...
    }
}

// 5. 测试模块
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example() {
        // ...
    }
}
```

### 命名规范

遵循Rust命名约定:

```rust
// 结构体/枚举: PascalCase
pub struct VirtualMachine { }
pub enum ExecutionStatus { }

// 函数/变量: snake_case
pub fn create_vm() { }
let vm_count = 42;

// 常量: SCREAMING_SNAKE_CASE
pub const MAX_CPUS: usize = 8;

// Trait: PascalCase
pub trait ExecutionEngine { }

// 模块: snake_case
pub mod vm_core { }
```

### 文档注释

**公开API必须有文档**:

```rust
/// 创建一个新的虚拟机实例
///
/// # 参数
///
/// * `config` - VM配置
///
/// # 返回
///
/// 返回一个`Result`，包含`VirtualMachine`实例或`Error`
///
/// # 错误
///
/// 当配置无效时返回`Error::InvalidConfig`
///
/// # 示例
///
/// ```
/// use vm_core::VirtualMachine;
///
/// let vm = VirtualMachine::new()?;
/// # Ok::<(), vm_core::Error>(())
/// ```
pub fn new(config: VmConfig) -> Result<Self, Error> {
    // ...
}
```

### 测试规范

**测试必须有目的**:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_creation() {
        // Arrange
        let config = VmConfig::default();

        // Act
        let vm = VirtualMachine::new(config).unwrap();

        // Assert
        assert_eq!(vm.vcpu_count(), 1);
    }

    #[test]
    fn test_vm_creation_with_invalid_config() {
        let config = VmConfig::invalid();

        let result = VirtualMachine::new(config);

        assert!(matches!(result, Err(Error::InvalidConfig)));
    }
}
```

---

## ✍️ 提交规范

### 语义化提交

我们使用[Conventional Commits](https://www.conventionalcommits.org/)规范:

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

### 提交类型

- **feat**: 新功能
- **fix**: Bug修复
- **docs**: 仅文档更改
- **style**: 代码格式（不影响功能）
- **refactor**: 重构（既不是新功能也不是修复）
- **perf**: 性能改进
- **test**: 添加或修改测试
- **chore**: 构建/工具相关
- **ci**: CI/CD相关

### 提交示例

**好的提交**:
```
feat(jit): add loop optimization pass

Implement loop unrolling and vectorization for
improved performance in tight loops.

Performance improvement: 15-20% for loop-heavy code

Closes #123
```

```
fix(mmu): correct page table translation for large addresses

Fixes bug where addresses > 4GB were incorrectly translated.
Now uses 64-bit arithmetic throughout.

Fixes #456
```

**不好的提交**:
```
update stuff
fix bug
changes
```

### 提交消息模板

```bash
# 简短说明 (50字符或更少)
# 更详细的解释 (72字符换行)

# Further paragraphs come after blank lines.
# - Bullet points are okay, too
# - Use a hanging indent

# 提供Issue或PR链接
# Fixes #123
# See also #456
```

---

## 🔀 Pull Request流程

### PR模板

创建PR时请填写:

```markdown
## 描述
简要描述此PR的更改

## 类型
- [ ] Bug修复
- [ ] 新功能
- [ ] 重构
- [ ] 文档更新
- [ ] 其他 (请说明)

## 更改内容
- 更改1
- 更改2

## 测试
- [ ] 包含测试
- [ ] 所有测试通过 (`cargo test --workspace`)
- [ ] 添加了测试文档

## 文档
- [ ] 更新了相关文档
- [ ] 添加了示例代码

## 检查清单
- [ ] 遵循代码规范 (`cargo fmt`, `cargo clippy`)
- [ ] 自我审查了代码
- [ ] 注释了复杂代码
- [ ] 更新了文档
- [ ] 无新的警告
- [ ] 添加了测试
- [ ] 通过了所有CI检查

## 相关Issue
Closes #(issue number)
```

### PR审查流程

1. **自动检查**: CI自动运行测试和Clippy
2. **人工审查**: 维护者审查代码
3. **反馈**: 可能要求更改
4. **批准**: 批准后合并

**审查关注点**:
- ✅ 代码质量
- ✅ 测试覆盖
- ✅ 文档完整
- ✅ 性能影响
- ✅ 向后兼容

### 响应反馈

- 🙏 **感谢反馈**: 审查者帮助改进代码
- 🔄 **及时响应**: 尽快处理反馈
- 💬 **讨论问题**: 有疑问请提问
- ✅ **标记完成**: 反馈处理完成后评论

---

## 🎯 贡献想法

### 好的第一任务

搜索标签为 `good first issue` 的Issue:

```bash
# 查找适合新手的Issue
gh issue list --label "good first issue"
```

### 需要帮助的贡献

**文档**:
- 补充API文档
- 添加示例代码
- 翻译文档
- 改进教程

**测试**:
- 提高测试覆盖率
- 添加集成测试
- 添加基准测试
- 改进测试文档

**代码**:
- 修复Bug
- 实现简单功能
- 重构代码
- 性能优化

**工具**:
- 改进构建脚本
- 添加CI/CD检查
- 开发工具
- 文档生成工具

---

## 📊 项目里程碑

我们使用里程碑来跟踪进度:

- [v0.1.0](https://github.com/your-org/vm/milestone/1) - 基础功能
- [v0.2.0](https://github.com/your-org/vm/milestone/2) - JIT优化
- [v0.3.0](https://github.com/your-org/vm/milestone/3) - 跨架构支持

查看[所有里程碑](https://github.com/your-org/vm/milestones)

---

## 🏆 贡献者

感谢所有贡献者！在[CONTRIBUTORS.md](CONTRIBUTORS.md)查看完整列表。

### 成为贡献者

任何被合并的PR都将被添加到贡献者列表！

---

## 💬 获得帮助

### 沟通渠道

- **GitHub Issues**: 报告Bug和功能请求
- **GitHub Discussions**: 一般讨论和问题
- **Gitter/Discord**: 实时聊天
- **邮件**: your-email@example.com

### 资源

- **文档**: [docs/](../docs/)
- **示例**: [examples/](../examples/)
- **API文档**: [https://docs.rs/vm](https://docs.rs/vm)
- **Rust文档**: [https://doc.rust-lang.org/](https://doc.rust-lang.org/)

### 寻求指导

- 创建Issue标记为 `help wanted`
- 在Discussions中提问
- 在Gitter/Discord中实时讨论

---

## ⚖️ 许可证

通过贡献，您同意您的贡献将在与项目相同的许可证下发布:

- MIT License
- Apache License, Version 2.0

您可以选择任一许可证。

---

## 🎓 学习资源

### Rust资源

- [Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [The Rustonomicon](https://doc.rust-lang.org/nomicon/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

### DDD资源

- [Domain-Driven Design](https://www.domainlanguage.com/ddd/)
- [Implementing DDD](https://www.iamtimcorey.com/implementing-ddd/)

### 虚拟化资源

- [Rust VMM](https://github.com/rust-vmm/vm-vmm)
- [Cranelift](https://github.com/bytecodealliance/cranelift)

---

## 🎉 感谢贡献者

再次感谢您的贡献！每一个贡献都让项目变得更好。

**记住**: 即使是最小的贡献也有价值！

---

**维护者**: VM团队
**最后更新**: 2026-01-06
**版本**: 1.0

🚀 **准备贡献? 现在就开始吧！** 🚀
