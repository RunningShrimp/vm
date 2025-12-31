# InstructionSet API 参考

## 概述

`vm-frontend` 提供多架构的指令前端解码器，支持 RISC-V64、ARM64、x86_64 等多种CPU架构。该模块负责将二进制机器码解码为可执行的指令表示。

## 主要功能

- **多架构支持**: RISC-V64、ARM64、x86_64
- **指令解码**: 将机器码解码为指令对象
- **基本块解码**: 解码指令序列为基本块
- **反汇编支持**: 用于调试和分析

## 支持的架构

### RISC-V 64位

完整的RISC-V 64位指令集支持。

**支持的扩展**:
- **RV64I**: 整数基础指令集
- **RV64M**: 乘除法扩展
- **RV64A**: 原子指令扩展
- **RV64F**: 单精度浮点
- **RV64D**: 双精度浮点
- **RV64C**: 压缩指令

**解码器类型**: `RiscvDecoder` / `Riscv64Decoder`

### ARM64 (AArch64)

完整的ARM64指令集支持。

**支持的指令类别**:
- 数据处理指令
- 分支指令
- 加载/存储指令
- 系统指令
- 浮点和SIMD指令

**解码器类型**: `Arm64Decoder`

### x86_64

完整的x86-64指令集支持。

**支持的指令类别**:
- 通用指令
- SSE/AVX指令
- 系统指令
- 特权指令

**解码器类型**: `X86Decoder`

## 主要类型

### RiscvDecoder

RISC-V指令解码器。

#### 方法

##### `new() -> Self`

创建新的RISC-V解码器。

**返回**:
- 解码器实例

**示例**:
```rust
use vm_frontend::riscv64::RiscvDecoder;

let decoder = RiscvDecoder::new();
```

##### `decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> VmResult<RiscvInstruction>`

解码单条RISC-V指令。

**参数**:
- `mmu`: MMU引用，用于读取指令
- `pc`: 程序计数器

**返回**:
- 解码后的指令

##### `decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> VmResult<RiscvBasicBlock>`

解码RISC-V基本块。

**参数**:
- `mmu`: MMU引用
- `pc`: 起始地址

**返回**:
- 解码后的基本块

### RiscvInstruction

RISC-V指令表示。

#### 字段

##### `opcode: u8`

操作码。

##### `rd: u8`

目标寄存器。

##### `rs1: u8`

源寄存器1。

##### `rs2: u8`

源寄存器2。

##### `imm: i64`

立即数。

##### `mnemonic: RiscvMnemonic`

指令助记符。

### RiscvMnemonic

RISC-V指令助记符枚举。

#### 常见指令

##### 整数运算

- `ADD` - 加法
- `SUB` - 减法
- `MUL` - 乘法
- `DIV` - 除法
- `REM` - 取余

##### 逻辑运算

- `AND` - 与
- `OR` - 或
- `XOR` - 异或
- `SLL` - 左移
- `SRL` - 逻辑右移
- `SRA` - 算术右移

##### 访存指令

- `LB` - 加载字节
- `LH` - 加载半字
- `LW` - 加载字
- `LD` - 加载双字
- `SB` - 存储字节
- `SH` - 存储半字
- `SW` - 存储字
- `SD` - 存储双字

##### 分支指令

- `BEQ` - 相等跳转
- `BNE` - 不等跳转
- `BLT` - 小于跳转
- `BGE` - 大于等于跳转
- `JAL` - 跳转并链接
- `JALR` - 寄存器跳转并链接

### Arm64Decoder

ARM64指令解码器。

#### 方法

##### `new() -> Self`

创建新的ARM64解码器。

**返回**:
- 解码器实例

**示例**:
```rust
use vm_frontend::arm64::Arm64Decoder;

let decoder = Arm64Decoder::new();
```

##### `decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> VmResult<Arm64Instruction>`

解码单条ARM64指令。

##### `decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> VmResult<Arm64BasicBlock>`

解码ARM64基本块。

### X86Decoder

x86-64指令解码器。

#### 方法

##### `new() -> Self`

创建新的x86解码器。

**返回**:
- 解码器实例

**示例**:
```rust
use vm_frontend::x86_64::X86Decoder;

let decoder = X86Decoder::new();
```

##### `decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> VmResult<X86Instruction>`

解码单条x86指令。

##### `decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> VmResult<X86BasicBlock>`

解码x86基本块。

### X86Instruction

x86指令表示。

#### 字段

##### `mnemonic: X86Mnemonic`

指令助记符。

##### `operands: Vec<X86Operand>`

操作数列表。

##### `length: usize`

指令长度。

### X86Mnemonic

x86指令助记符枚举。

包含所有x86-64指令，如：
- `MOV` - 数据传送
- `ADD` - 加法
- `SUB` - 减法
- `JMP` - 跳转
- `CALL` - 调用
- `RET` - 返回
等等

### X86Operand

x86操作数类型。

#### 变体

##### `Register(u8)`

寄存器操作数。

##### `Immediate(i64)`

立即数操作数。

##### `Memory(u64)`

内存操作数。

##### `Relative(i32)`

相对地址。

## 使用示例

### RISC-V指令解码

```rust
use vm_frontend::riscv64::RiscvDecoder;
use vm_core::{MMU, GuestAddr};

let mut decoder = RiscvDecoder::new();
let mmu = setup_mmu();

// 解码单条指令
let pc = GuestAddr(0x1000);
let insn = decoder.decode_insn(&mmu, pc)?;

println!("Instruction: {:?}", insn.mnemonic);
println!("RD: x{}", insn.rd);
println!("RS1: x{}", insn.rs1);
println!("RS2: x{}", insn.rs2);
println!("Immediate: {}", insn.imm);
```

### RISC-V基本块解码

```rust
use vm_frontend::riscv64::RiscvDecoder;

let mut decoder = RiscvDecoder::new();
let mmu = setup_mmu();

// 解码基本块
let pc = GuestAddr(0x1000);
let block = decoder.decode(&mmu, pc)?;

println!("Block has {} instructions", block.instructions.len());

for insn in &block.instructions {
    println!("  {:?}", insn.mnemonic);
}
```

### ARM64指令解码

```rust
use vm_frontend::arm64::Arm64Decoder;

let mut decoder = Arm64Decoder::new();
let mmu = setup_mmu();

let pc = vm_core::GuestAddr(0x1000);
let insn = decoder.decode_insn(&mmu, pc)?;

println!("ARM64 Instruction: {:?}", insn);
```

### x86指令解码

```rust
use vm_frontend::x86_64::{X86Decoder, X86Mnemonic};

let mut decoder = X86Decoder::new();
let mmu = setup_mmu();

let pc = vm_core::GuestAddr(0x1000);
let insn = decoder.decode_insn(&mmu, pc)?;

println!("x86 Instruction: {:?}", insn.mnemonic);

match insn.mnemonic {
    X86Mnemonic::MOV => println!("Move instruction"),
    X86Mnemonic::JMP => println!("Jump instruction"),
    _ => println!("Other instruction"),
}

for operand in &insn.operands {
    println!("  Operand: {:?}", operand);
}
```

### 多架构统一接口

```rust
use vm_frontend::architectures::*;

fn decode_and_execute(arch: vm_core::GuestArch, mmu: &dyn MMU, pc: vm_core::GuestAddr) -> vm_core::VmResult<()> {
    match arch {
        vm_core::GuestArch::Riscv64 => {
            let mut decoder = Riscv64Decoder::new();
            let insn = decoder.decode_insn(mmu, pc)?;
            // 执行RISC-V指令
        }
        vm_core::GuestArch::Arm64 => {
            let mut decoder = Arm64Decoder::new();
            let insn = decoder.decode_insn(mmu, pc)?;
            // 执行ARM64指令
        }
        vm_core::GuestArch::X86_64 => {
            let mut decoder = X86Decoder::new();
            let insn = decoder.decode_insn(mmu, pc)?;
            // 执行x86指令
        }
        _ => unimplemented!(),
    }
    Ok(())
}
```

### 反汇编工具

```rust
use vm_frontend::riscv64::RiscvDecoder;

fn disassemble(mmu: &dyn MMU, start: vm_core::GuestAddr, count: usize) {
    let mut decoder = RiscvDecoder::new();
    let mut pc = start;

    for _ in 0..count {
        match decoder.decode_insn(mmu, pc) {
            Ok(insn) => {
                println!("{:08x}: {:?}", pc.0, insn.mnemonic);
                pc += insn.length as u64;
            }
            Err(e) => {
                println!("{:08x}: Error: {:?}", pc.0, e);
                break;
            }
        }
    }
}
```

## RISC-V M扩展详解

vm-frontend 现在包含完整的RISC-V M扩展（乘除法）支持。

### 乘法指令

- `MUL` - 乘法（低32位）
- `MULH` - 有符号乘法（高32位）
- `MULHSU` - 混合符号乘法（高32位）
- `MULHU` - 无符号乘法（高32位）

示例：
```rust
// mul x1, x2, x3
// x1 = (x2 * x3)[31:0]
```

### 除法指令

- `DIV` - 有符号除法
- `DIVU` - 无符号除法
- `REM` - 有符号取余
- `REMU` - 无符号取余

示例：
```rust
// div x1, x2, x3
// x1 = x2 / x3 (有符号)

// rem x4, x5, x6
// x4 = x5 % x6 (有符号)
```

### 文件位置

- `/vm-frontend/src/riscv64/mul.rs` - 乘法指令实现
- `/vm-frontend/src/riscv64/div.rs` - 除法指令实现

## 架构特性对比

| 特性 | RISC-V | ARM64 | x86_64 |
|------|--------|-------|--------|
| 指令长度 | 固定4字节（2字节压缩） | 固定4字节 | 可变（1-15字节） |
| 寄存器数量 | 32个x寄存器 | 31个x寄存器 | 16个通用寄存器 |
| 地址模式 | 简单 | 复杂 | 非常复杂 |
| 编码难度 | 简单 | 中等 | 困难 |
| 解码速度 | 快 | 快 | 较慢 |

## 注意事项

### 特性标志

vm-frontend默认编译所有架构支持（通过"all" feature）：

```toml
[dependencies]
vm-frontend = { version = "0.1", features = ["all"] }
```

如果只需要特定架构：

```toml
vm-frontend = { version = "0.1", features = ["riscv64"] }
```

### 指令格式

不同架构的指令格式差异很大：

**RISC-V**:
- 固定4字节对齐
- 规整的字段划分
- 易于解码

**ARM64**:
- 固定4字节对齐
- 灵活的编码
- 中等复杂度

**x86_64**:
- 可变长度
- 复杂的前缀系统
- 解码器复杂

### 错误处理

解码错误会返回`VmError`：

```rust
match decoder.decode_insn(&mmu, pc) {
    Ok(insn) => { /* 处理指令 */ }
    Err(vm_core::VmError::Execution(
        vm_core::ExecutionError::Fault(
            vm_core::Fault::InvalidOpcode { .. }
        )
    )) => {
        eprintln!("Invalid opcode at {:x}", pc);
    }
    Err(e) => {
        eprintln!("Decode error: {:?}", e);
    }
}
```

### 性能考虑

- 解码开销相对较小
- 建议缓存解码结果
- 基本块解码比单条解码更高效
- JIT编译器会重用解码结果

## 相关API

- [VmCore API](./VmCore.md) - Decoder trait定义
- [VmEngine API](./VmEngine.md) - 执行引擎
- [VmMemory API](./VmMemory.md) - 内存管理
