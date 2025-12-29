# 贡献指南

感谢您对 VM 项目的关注！我们欢迎各种形式的贡献。

## 开发环境设置

### 前置要求

- Rust >= 1.85.0 (Edition 2024)
- Cargo (随 Rust 安装)
- Git

### 安装 Rust

```bash
# 使用 rustup 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装稳定版
rustup install stable

# 验证版本
rustc --version  # 应该 >= 1.85.0
cargo --version
```

### 克隆仓库

```bash
git clone https://github.com/your-org/vm.git
cd vm
```

### 构建项目

```bash
# 构建所有组件
cargo build --workspace --all-features

# 运行测试
cargo test --workspace

# 运行 Clippy
cargo clippy --workspace --all-features -- -D warnings

# 格式化代码
cargo fmt
```

## 代码规范

### Rust 代码风格

我们遵循 [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)：

#### 命名规范

- **类型/Trait**: `PascalCase`
- **函数/变量**: `snake_case`
- **常量**: `SCREAMING_SNAKE_CASE`
- **宏**: `snake_case!` (后缀 `!`)

#### 示例

```rust
// ✅ 好的命名
pub struct VirtualMachine { }  // PascalCase
pub fn create_vm() -> VmResult<VirtualMachine> { }  // snake_case
const MAX_MEMORY_SIZE: usize = 1024;  // SCREAMING_SNAKE_CASE
macro_rules! vm_debug { }  // snake_case!

// ❌ 不好的命名
pub struct virtualMachine { }  // 应该是 PascalCase
pub fn CreateVM() { }  // 应该是 snake_case
```

### 注释规范

#### 公共 API 必须有文档注释

```rust
/// 虚拟机内存管理单元
///
/// MMU 负责将客户机虚拟地址（VA）翻译为物理地址（PA）。
///
/// # 示例
///
/// ```
/// use vm_mem::{SoftMMU, PagingMode};
///
/// let mut mmu = SoftMMU::new(1024 * 1024, true);
/// mmu.map_page(0x1000, 0x2000, PageFlags::RW)?;
/// ```
///
/// # 错误处理
///
/// 如果地址映射失败，返回 `MemoryError::PageFault`。
pub struct SoftMMU {
    // ...
}
```

#### 复杂逻辑需要解释注释

```rust
// 为什么使用三层哈希表：
// 1. L1: 快速查找（最近访问的页）
// 2. L2: 中速查找（所有映射的页）
// 3. L3: 慢速查找（页表遍历）
// 这种设计在 TLB 命中率 >95% 时性能最优
```

### 错误处理

#### 使用 `?` 运算符简化错误传播

```rust
// ✅ 好的做法
pub fn load_kernel(&mut self, path: &str) -> VmResult<()> {
    let data = std::fs::read(path)?;
    self.load_program(&data)?;
    Ok(())
}

// ❌ 不好的做法
pub fn load_kernel(&mut self, path: &str) -> VmResult<()> {
    let data = std::fs::read(path).map_err(|e| VmError::Io(e.to_string()))?;
    self.load_program(&data).map_err(|e| VmError::from(e))?;
    Ok(())
}
```

#### 提供有意义的错误信息

```rust
// ✅ 好的做法
Err(VmError::Memory(MemoryError::PageFault {
    addr: vm_core::GuestAddr(0x1000),
    access_type: vm_core::AccessType::Read,
    message: "Access to unmapped page while executing instruction".to_string(),
}))

// ❌ 不好的做法
Err(VmError::Memory(MemoryError::PageFault {
    addr: vm_core::GuestAddr(0x1000),
    access_type: vm_core::AccessType::Read,
    message: "error".to_string(),
}))
```

## 测试规范

### 单元测试

每个模块都应该有单元测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mmu_map_page() {
        let mut mmu = SoftMMU::new(1024, true);
        assert!(mmu.map_page(0x1000, 0x2000, PageFlags::RW).is_ok());
    }

    #[test]
    fn test_mmu_read_write() {
        let mut mmu = SoftMMU::new(1024, true);
        mmu.map_page(0x1000, 0x2000, PageFlags::RW).unwrap();

        mmu.write(0x1000, 0x42).unwrap();
        assert_eq!(mmu.read(0x1000).unwrap(), 0x42);
    }
}
```

### 集成测试

在 `tests/` 目录创建集成测试：

```rust
// tests/integration_test.rs
use vm_core::{VmConfig, GuestArch};
use vm_service::VirtualMachineService;

#[test]
fn test_full_vm_lifecycle() {
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        ..Default::default()
    };

    let vm = VirtualMachineService::new(config).unwrap();
    assert!(vm.start().is_ok());
    assert!(vm.pause().is_ok());
    assert!(vm.stop().is_ok());
}
```

### 基准测试

在 `benches/` 目录创建性能测试：

```rust
// benches/mmu_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_mmu_read(c: &mut Criterion) {
    let mut mmu = SoftMMU::new(1024 * 1024, true);

    c.bench_function("read", |b| {
        b.iter(|| {
            black_box(mmu.read(black_box(0x1000)))
        })
    });
}

criterion_group!(benches, bench_mmu_read);
criterion_main!(benches);
```

## Pull Request 流程

### 1. Fork 并创建分支

```bash
# Fork 项目到你的 GitHub 账号

# 克隆你的 fork
git clone https://github.com/your-username/vm.git
cd vm

# 添加上游仓库
git remote add upstream https://github.com/original-org/vm.git

# 创建功能分支
git checkout -b feature/my-feature
```

### 2. 进行更改

- 编写代码
- 添加测试
- 更新文档
- 运行 `cargo fmt`
- 运行 `cargo clippy`
- 运行 `cargo test`

### 3. 提交更改

```bash
git add .
git commit -m "feat: Add XYZ feature

- Implement XYZ functionality
- Add unit tests
- Update documentation

Closes #123"
```

### 4. 推送到你的 fork

```bash
git push origin feature/my-feature
```

### 5. 创建 Pull Request

- 访问 GitHub 上的原始仓库
- 点击 "New Pull Request"
- 填写 PR 模板（见下文）

## Pull Request 模板

```markdown
## 描述
简要描述此 PR 的目的和内容。

## 类型
- [ ] Bug 修复
- [ ] 新功能
- [ ] 代码重构
- [ ] 文档更新
- [ ] 性能优化
- [ ] 其他（请说明）

## 更改内容
<!-- 列出此 PR 修改的文件和主要内容 -->

## 测试
描述您如何测试这些更改：
- [ ] 单元测试通过
- [ ] 集成测试通过
- [ ] 手动测试步骤

## 检查清单
- [ ] 我的代码遵循此项目的代码规范
- [ ] 我已执行 `cargo fmt`
- [ ] 我已执行 `cargo clippy` 并修复所有警告
- [ ] 我已添加测试以覆盖新功能
- [ ] 新旧测试都通过
- [ ] 我已更新相关文档

## 相关 Issue
Closes #123
Related to #456
```

## 代码审查标准

### 必须满足的条件

1. **零编译错误**: `cargo check` 必须通过
2. **零 Clippy 警告**: `cargo clippy -- -D warnings` 必须通过
3. **测试通过**: `cargo test` 必须通过
4. **文档完整**: 公共 API 必须有文档注释

### 代码质量标准

1. **可读性**: 代码应该易于理解
2. **模块化**: 功能应该合理分解
3. **性能**: 不能有明显的性能退化
4. **安全性**: 不能引入安全漏洞

### 我们特别关注

- **类型安全**: 充分利用 Rust 类型系统
- **内存安全**: 避免 unsafe 代码（除非必要）
- **错误处理**: 正确处理所有错误情况
- **并发安全**: 正确使用 Arc/Mutex/通道

## Issue 报告

### Bug 报告模板

```markdown
## Bug 描述
清晰简洁地描述 bug

## 复现步骤
1. 步骤 1
2. 步骤 2
3. ...

## 期望行为
描述您期望发生什么

## 实际行为
描述实际发生了什么

## 环境
- OS: [e.g. Ubuntu 22.04]
- Rust 版本: [e.g. 1.85.0]
- VM 版本: [e.g. v0.1.0]

## 日志输出
```
粘贴相关的日志输出
```
```

### 功能请求模板

```markdown
## 功能描述
描述您希望添加的功能

## 使用场景
描述这个功能的使用场景

## 替代方案
描述您考虑过的替代方案

## 附加信息
任何其他有助于实现此功能的信息
```

## 社区准则

### 尊重与包容

- 尊重所有贡献者
- 欢迎不同背景和经验水平的人
- 建设性反馈
- 避免人身攻击

### 沟通方式

- 使用英文进行 Issue 和 PR 讨论
- 保持礼貌和专业
- 接受反馈并持续改进

## 获取帮助

### 文档

- [架构文档](./architecture.md)
- [API 指南](./api_guide.md)
- [Rust 官方文档](https://doc.rust-lang.org/)

### 联系方式

- GitHub Issues: [提交问题](https://github.com/your-org/vm/issues)
- Email: dev@example.com
- Discord: [加入我们的 Discord](https://discord.gg/xxx)

## 许可证

通过贡献代码，您同意您的贡献将在与项目相同的许可证下发布。

## 致谢

感谢所有贡献者的努力！
