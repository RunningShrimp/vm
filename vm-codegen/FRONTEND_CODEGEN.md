# 前端代码生成器使用指南

## 概述

前端代码生成器是vm-codegen模块的一部分，旨在减少不同架构前端之间的重复代码。它提供了一个通用的框架，可以生成各种架构的前端解码器代码。

## 主要组件

### 1. FrontendCodeGenerator

这是主要的代码生成器结构，负责生成前端代码。

```rust
use vm_codegen::{CodegenConfig, FrontendCodeGenerator};

let config = CodegenConfig {
    target_arch: "aarch64".to_string(),
    isa_version: "8.0".to_string(),
    optimization_level: 2,
    enable_debug: true,
};

let generator = FrontendCodeGenerator::new(config, "ARM64", 4, false);
```

### 2. 指令规范

使用`create_instruction_spec`函数创建指令规范：

```rust
use vm_codegen::create_instruction_spec;

let add_spec = create_instruction_spec(
    "ADD_IMM",
    "Add immediate",
    0x1F000000,
    0x11000000,
    r#"let sf = (insn >> 31) & 1;
       let imm12 = (insn >> 10) & 0xFFF;
       let rn = (insn >> 5) & 0x1F;
       let rd = insn & 0x1F;
       b.push(IROp::AddImm { dst: rd, src: rn, imm: imm12 as i64 });"#
);
```

### 3. 指令集

使用`create_instruction_set`函数创建指令集：

```rust
use vm_codegen::create_instruction_set;

let instruction_set = create_instruction_set("ARM64", vec![add_spec, sub_spec, mov_spec]);
```

## 使用示例

### 生成ARM64前端代码

```rust
use vm_codegen::{
    CodegenConfig, FrontendCodeGenerator, 
    create_instruction_spec, create_instruction_set
};

fn main() {
    // 创建ARM64指令集
    let arm64_instructions = vec![
        // ADD/SUB (immediate)
        create_instruction_spec(
            "ADD_IMM",
            "Add immediate",
            0x1F000000,
            0x11000000,
            r#"let sf = (insn >> 31) & 1;
               let imm12 = (insn >> 10) & 0xFFF;
               let rn = (insn >> 5) & 0x1F;
               let rd = insn & 0x1F;
               b.push(IROp::AddImm { dst: rd, src: rn, imm: imm12 as i64 });"#
        ),
        // 更多指令...
    ];

    let instruction_set = create_instruction_set("ARM64", arm64_instructions);

    // 创建代码生成器配置
    let config = CodegenConfig {
        target_arch: "aarch64".to_string(),
        isa_version: "8.0".to_string(),
        optimization_level: 2,
        enable_debug: true,
    };

    // 创建前端代码生成器
    let generator = FrontendCodeGenerator::new(config, "ARM64", 4, false);

    // 生成前端代码
    let generated_code = generator.generate_frontend_code(&instruction_set, true);

    // 保存到文件
    std::fs::write("arm64_frontend_generated.rs", generated_code)
        .expect("Failed to write generated ARM64 frontend code");
}
```

### 生成RISC-V前端代码

```rust
use vm_codegen::{
    CodegenConfig, FrontendCodeGenerator, 
    create_instruction_spec, create_instruction_set
};

fn main() {
    // 创建RISC-V指令集
    let riscv_instructions = vec![
        // LUI
        create_instruction_spec(
            "LUI",
            "Load upper immediate",
            0x7F,
            0x37,
            r#"let imm = ((insn & 0xfffff000) as i32) as i64;
               let rd = ((insn >> 7) & 0x1f) as u32;
               b.push(IROp::AddImm { dst: rd, src: 0, imm });"#
        ),
        // 更多指令...
    ];

    let instruction_set = create_instruction_set("RISC-V", riscv_instructions);

    // 创建代码生成器配置
    let config = CodegenConfig {
        target_arch: "riscv64".to_string(),
        isa_version: "2.1".to_string(),
        optimization_level: 2,
        enable_debug: true,
    };

    // 创建前端代码生成器
    let generator = FrontendCodeGenerator::new(config, "RISC-V", 4, true);

    // 生成前端代码
    let generated_code = generator.generate_frontend_code(&instruction_set, false);

    // 保存到文件
    std::fs::write("riscv_frontend_generated.rs", generated_code)
        .expect("Failed to write generated RISC-V frontend code");
}
```

## 自动构建

可以通过设置环境变量`VM_CODEGEN_GEN=1`来启用自动代码生成：

```bash
VM_CODEGEN_GEN=1 cargo build
```

这将自动运行代码生成示例，并生成相应的前端代码。

## 生成的代码结构

生成的代码包含以下主要部分：

1. **指令结构体**：表示特定架构的指令
2. **解码器结构体**：包含解码缓存和扩展解码器支持
3. **解码实现**：实现`Decoder` trait，提供指令解码功能

### 指令结构体示例

```rust
/// ARM64 指令表示
#[derive(Debug, Clone)]
pub struct ARM64Instruction {
    pub mnemonic: &'static str,
    pub next_pc: GuestAddr,
    pub has_memory_op: bool,
    pub is_branch: bool,
}

impl Instruction for ARM64Instruction {
    fn next_pc(&self) -> GuestAddr {
        self.next_pc
    }

    fn size(&self) -> u8 {
        4 // ARM64 指令固定 4 字节
    }

    fn operand_count(&self) -> usize {
        1 // 简化实现
    }

    fn mnemonic(&self) -> &str {
        self.mnemonic
    }

    fn is_control_flow(&self) -> bool {
        self.is_branch
    }

    fn is_memory_access(&self) -> bool {
        self.has_memory_op
    }
}
```

### 解码器结构体示例

```rust
/// ARM64 解码器，支持解码缓存优化
pub struct ARM64Decoder {
    /// 解码缓存
    decode_cache: Option<std::collections::HashMap<GuestAddr, IRBlock>>,
    /// 缓存大小限制
    cache_size_limit: usize,
    /// 扩展指令解码器
    pub extension_decoders: HashMap<String, Box<dyn ExtensionDecoder>>,
}
```

## 扩展支持

前端代码生成器支持扩展指令解码器，可以通过实现`ExtensionDecoder` trait来添加自定义指令解码逻辑：

```rust
/// 扩展解码器trait
pub trait ExtensionDecoder: Send + Sync {
    fn decode(&self, insn: u32, builder: &mut IRBuilder) -> Result<bool, VmError>;
    fn name(&self) -> &str;
}
```

## 优势

使用前端代码生成器的主要优势包括：

1. **减少重复代码**：不同架构的前端共享相同的代码结构和模式
2. **一致性**：确保所有前端遵循相同的接口和实现模式
3. **易于维护**：修改代码生成器可以同时影响所有架构的前端
4. **可扩展性**：轻松添加新架构支持或扩展现有架构
5. **自动化**：通过构建脚本自动生成代码，减少手动编写的工作量

## 最佳实践

1. **指令规范**：确保指令规范准确描述指令的编码和行为
2. **处理器代码**：保持处理器代码简洁明了，避免复杂逻辑
3. **测试**：为生成的代码编写全面的测试
4. **文档**：为自定义指令和扩展提供清晰的文档
5. **版本控制**：将生成的代码纳入版本控制，以便跟踪更改

## 未来扩展

前端代码生成器计划在未来添加以下功能：

1. **更多架构支持**：添加对x86-64等更多架构的支持
2. **性能优化**：生成更高效的解码代码
3. **调试支持**：增强调试信息的生成
4. **验证工具**：添加指令规范验证工具
5. **可视化工具**：提供指令解码过程的可视化