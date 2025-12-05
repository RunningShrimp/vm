# 贡献指南

感谢您对Rust虚拟机项目的关注！本文档将指导您如何为项目做出贡献。

## 目录

1. [开发环境设置](#开发环境设置)
2. [代码规范](#代码规范)
3. [提交流程](#提交流程)
4. [测试要求](#测试要求)
5. [文档要求](#文档要求)
6. [代码审查流程](#代码审查流程)

## 开发环境设置

### 前置要求

- Rust工具链：1.70.0或更高版本
- Cargo：随Rust工具链一起安装
- Git：用于版本控制
- LLVM：用于AOT编译（可选）

### 安装步骤

1. **克隆仓库**
   ```bash
   git clone https://github.com/example/vm.git
   cd vm
   ```

2. **安装Rust工具链**
   ```bash
   rustup toolchain install stable
   rustup default stable
   ```

3. **安装开发依赖**
   ```bash
   cargo build
   ```

4. **运行测试**
   ```bash
   cargo test
   ```

### 开发工具推荐

- **rustfmt**: 代码格式化工具
  ```bash
  rustup component add rustfmt
  ```

- **clippy**: 代码检查工具
  ```bash
  rustup component add clippy
  ```

- **rust-analyzer**: IDE支持（推荐使用VS Code或IntelliJ IDEA）

## 代码规范

### 代码风格

1. **格式化**: 使用 `rustfmt` 格式化代码
   ```bash
   cargo fmt
   ```

2. **代码检查**: 使用 `clippy` 检查代码
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```

3. **命名规范**:
   - 模块名：`snake_case`
   - 类型名：`PascalCase`
   - 函数名：`snake_case`
   - 常量名：`SCREAMING_SNAKE_CASE`

### 代码组织

1. **模块结构**: 每个模块应该有清晰的职责
2. **文档注释**: 所有公共API必须有文档注释
3. **错误处理**: 使用 `Result<T, E>` 进行错误处理
4. **测试**: 每个模块都应该有对应的测试

### 示例代码风格

```rust
//! 模块级文档注释

use std::collections::HashMap;

/// 结构体文档注释
///
/// # 示例
///
/// ```rust
/// let example = Example::new();
/// ```
pub struct Example {
    /// 字段文档注释
    pub field: u32,
}

impl Example {
    /// 函数文档注释
    ///
    /// # 参数
    /// - `value`: 输入值
    ///
    /// # 返回
    /// 返回处理后的值
    pub fn process(&self, value: u32) -> Result<u32, String> {
        // 实现
        Ok(value)
    }
}
```

## 提交流程

### 1. 创建分支

```bash
git checkout -b feature/your-feature-name
# 或
git checkout -b fix/your-bug-fix-name
```

### 2. 进行更改

- 编写代码
- 添加测试
- 更新文档
- 运行测试确保通过

### 3. 提交更改

提交信息应该清晰描述更改内容：

```bash
git add .
git commit -m "feat: 添加新功能描述"
```

**提交信息格式**:
- `feat:` 新功能
- `fix:` 错误修复
- `docs:` 文档更改
- `refactor:` 代码重构
- `test:` 测试相关
- `chore:` 构建/工具相关

### 4. 推送更改

```bash
git push origin feature/your-feature-name
```

### 5. 创建Pull Request

1. 在GitHub上创建Pull Request
2. 填写PR描述，包括：
   - 更改的目的
   - 实现方式
   - 测试情况
   - 相关issue（如有）

## 测试要求

### 单元测试

每个模块都应该有单元测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example() {
        // 测试代码
    }
}
```

### 集成测试

集成测试位于 `tests/` 目录：

```rust
// tests/integration_test.rs
#[test]
fn test_integration() {
    // 集成测试代码
}
```

### 运行测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test test_example

# 运行测试并显示输出
cargo test -- --nocapture
```

### 测试覆盖率

使用覆盖率工具检查测试覆盖率：

```bash
# 生成覆盖率报告
python scripts/generate-coverage-report.py
```

**覆盖率要求**: 新代码的测试覆盖率应该 >= 80%

## 文档要求

### API文档

所有公共API必须有文档注释：

```rust
/// 函数简短描述
///
/// 详细描述（可选）
///
/// # 参数
/// - `param`: 参数描述
///
/// # 返回
/// 返回值描述
///
/// # 示例
/// ```rust
/// example_function(42);
/// ```
pub fn example_function(param: u32) -> u32 {
    param
}
```

### 模块文档

每个模块应该有模块级文档：

```rust
//! 模块描述
//!
//! 详细说明模块的功能和使用方法
```

### 生成文档

```bash
# 生成API文档
cargo doc --no-deps --open

# 检查文档警告
cargo doc --no-deps 2>&1 | grep warning
```

## 代码审查流程

### 审查检查清单

提交PR前，请确保：

- [ ] 代码通过 `cargo fmt` 格式化
- [ ] 代码通过 `cargo clippy` 检查（无警告）
- [ ] 所有测试通过
- [ ] 添加了新功能的测试
- [ ] 更新了相关文档
- [ ] 提交信息清晰明确

### 审查反馈

- 审查者会在PR中提供反馈
- 请及时回复和修改
- 如果对反馈有疑问，欢迎讨论

### 合并要求

- 至少需要一个审查者批准
- 所有CI检查通过
- 无冲突

## 项目结构

```
vm/
├── vm-core/          # 核心抽象
├── vm-ir/            # 中间表示
├── vm-mem/           # 内存管理
├── vm-device/        # 设备模拟
├── vm-frontend-*/    # 架构前端
├── vm-engine-*/      # 执行引擎
├── vm-cross-arch/    # 跨架构执行
├── vm-service/       # 服务层
├── tests/            # 集成测试
├── benches/          # 性能基准测试
└── docs/             # 文档
```

## 常见问题

### Q: 如何添加新架构支持？

A: 参考 `vm-frontend-riscv64` 的实现，创建新的前端模块。

### Q: 如何添加新的执行引擎？

A: 实现 `ExecutionEngine<IRBlock>` trait，参考 `vm-engine-interpreter` 的实现。

### Q: 如何添加新设备？

A: 实现 `MmioDevice` trait，参考 `vm-device` 中的设备实现。

### Q: 性能基准测试如何运行？

A: 
```bash
cargo bench
```

### Q: 如何调试问题？

A: 
- 使用 `cargo test -- --nocapture` 查看测试输出
- 使用 `RUST_BACKTRACE=1 cargo test` 查看堆栈跟踪
- 使用 `cargo test --features debug` 启用调试功能

## 获取帮助

- **Issue**: 在GitHub上创建issue
- **讨论**: 在GitHub Discussions中讨论
- **邮件**: 联系维护者

## 行为准则

请遵守以下行为准则：

- 尊重他人
- 建设性的反馈
- 包容和友好
- 专业和礼貌

感谢您的贡献！

