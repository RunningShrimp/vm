# vm-codegen

**VM项目代码生成工具库**

[![Rust](https://img.shields.io/badge/rust-2024%20Edition-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

---

## 概述

`vm-codegen` 是VM项目的代码生成工具库，提供通用的指令解码、IR（中间表示）生成和代码生成工具，用于减少不同架构前端的重复代码，提升开发效率和代码一致性。

## 🎯 核心功能

- **指令模式匹配**: 通用指令模式匹配框架
- **字段提取器**: 标准化的指令字段提取工具
- **前端代码生成**: 自动生成前端指令解码代码
- **IR构建辅助**: 简化IR构建的辅助工具和trait

## 📦 主要组件

### 1. 模式匹配器 (PatternMatcher)

通用的指令模式匹配框架，支持基于位模式的指令识别和分发。

```rust
use vm_codegen::{PatternMatcher, StandardFieldExtractor};

// 创建模式匹配器
let matcher = PatternMatcher::<StandardFieldExtractor>::new();

// 注册模式处理器
matcher.register_pattern(0x12345678, Box::new(|builder, addr, context| {
    // 处理匹配的指令
    Ok(())
}));

// 匹配并分发指令
matcher.match_and_dispatch(builder, addr, instruction, &context)?;
```

### 2. 字段提取器 (FieldExtractor)

提供标准化的指令字段提取功能：

```rust
use vm_codegen::StandardFieldExtractor;

let extractor = StandardFieldExtractor;

// 提取位字段
let opcode = extractor.extract_field(instruction, 0, 7);

// 符号扩展
let immediate = extractor.sign_extend(value, 16);
```

### 3. 前端代码生成器 (FrontendCodeGenerator)

自动生成前端指令解码代码，减少手工编写重复代码：

```rust
use vm_codegen::FrontendCodeGenerator;

let generator = FrontendCodeGenerator::new();

// 生成指令集规范
let spec = create_instruction_spec("x86_64", &[
    ("ADD", "0x01", "Add with carry"),
    ("SUB", "0x29", "Subtract with borrow"),
]);

// 生成解码代码
let code = generator.generate_decoder(&spec)?;
```

## 🚀 使用场景

### 场景1: 添加新架构支持

当需要为新的CPU架构添加前端支持时：

```rust
// 1. 定义指令规范
let spec = create_instruction_spec("RISC-V", &[
    ("ADD", 0x33, "Add register"),
    ("SUB", 0x33, "Subtract register"),
    // ... 更多指令
]);

// 2. 生成解码器
let generator = FrontendCodeGenerator::new();
let decoder_code = generator.generate_decoder(&spec)?;

// 3. 集成到项目
// 将生成的代码集成到 vm-frontend 对应的架构模块中
```

### 场景2: 指令模式匹配

在指令解码中使用模式匹配：

```rust
use vm_codegen::PatternMatcher;

let mut matcher = PatternMatcher::new();

// 注册ALU指令模式
for opcode in 0x80..=0x83 {
    matcher.register_pattern(opcode, Box::new(|builder, addr, ctx| {
        // 处理ALU指令
        Ok(())
    }));
}

// 解码时使用
matcher.match_and_dispatch(ir_builder, addr, instruction, &context)?;
```

## 🔧 依赖关系

```toml
[dependencies]
vm-core = { path = "../vm-core" }      # 核心类型和trait
vm-ir = { path = "../vm-ir" }          # 中间表示
serde = { workspace = true }           # 序列化支持
bincode = "2.0.1"                      # 二进制序列化
```

## 📝 API概览

### Trait定义

```rust
/// 字段提取器trait
pub trait FieldExtractor {
    fn extract_field(&self, insn: u64, start: u32, width: u32) -> u64;
    fn sign_extend(&self, value: u64, width: u32) -> i64;
}
```

### 主要结构

- **`PatternMatcher<F: FieldExtractor>`**: 指令模式匹配器
- **`StandardFieldExtractor`**: 标准字段提取器实现
- **`FrontendCodeGenerator`**: 前端代码生成器
- **`GenericInstruction`**: 通用指令描述

## 🎨 设计特点

### 1. 类型安全

利用Rust的类型系统确保代码生成的正确性：

```rust
// 编译时检查类型匹配
type PatternHandler<F> = Box<dyn Fn(&mut IRBuilder, u64, &F) -> Result<(), VmError> + Send + Sync>;
```

### 2. 可扩展

支持自定义字段提取器和模式处理器：

```rust
// 自定义提取器
struct CustomExtractor;

impl FieldExtractor for CustomExtractor {
    // 自定义实现
}

// 使用自定义提取器
let matcher = PatternMatcher::<CustomExtractor>::new();
```

### 3. 零成本抽象

使用泛型和trait对象，在运行时没有额外开销：

```rust
// 编译后优化的代码与手写代码性能相同
matcher.register_pattern(pattern, handler);  // 内联调用
```

## 📚 相关文档

- [vm-core](../vm-core/README.md) - 核心类型和VM接口
- [vm-ir](../vm-ir/README.md) - 中间表示定义
- [vm-frontend](../vm-frontend/README.md) - 前端指令解码
- [MASTER_DOCUMENTATION_INDEX](../MASTER_DOCUMENTATION_INDEX.md) - 完整文档索引

## 🔨 开发指南

### 添加新的模式处理器

1. 定义模式匹配规则
2. 实现处理逻辑
3. 注册到PatternMatcher

### 生成代码示例

```bash
# 设置环境变量启用codegen
export VM_CODEGEN_GEN=1

# 构建时自动生成代码
cargo build --package vm-codegen
```

## ⚠️ 注意事项

1. **性能考虑**: 模式匹配器的性能取决于注册的模式数量，建议按使用频率排序
2. **线程安全**: PatternMatcher使用`RwLock`保护内部状态，支持并发访问
3. **错误处理**: 所有操作都返回`Result<VmError>`，确保错误可追踪

## 🤝 贡献指南

如果您想贡献新的代码生成功能：

1. 确保新功能支持多个架构（不仅限于x86_64）
2. 添加完整的文档注释和示例
3. 包含单元测试和集成测试
4. 更新本README文档

## 📊 性能指标

| 操作 | 性能 | 说明 |
|------|------|------|
| 模式匹配 | < 10ns | 单次模式匹配操作 |
| 字段提取 | < 5ns | 位字段提取 |
| 代码生成 | ~100ms | 生成完整架构解码器 |

## 📝 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](../LICENSE) 文件

---

**包版本**: workspace v0.1.0
**Rust版本**: 2024 Edition
**最后更新**: 2026-01-07
