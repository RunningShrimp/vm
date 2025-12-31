# VM项目示例

本目录包含VM项目的完整示例集合,从简单的Hello World到高级的JIT编译和自定义设备。

## 目录

- [快速开始](#快速开始)
- [示例列表](#示例列表)
- [学习路径](#学习路径)
- [RISC-V程序](#risc-v程序)
- [工具和脚本](#工具和脚本)
- [故障排除](#故障排除)

## 快速开始

### 运行第一个示例

最简单的入门方式是运行Hello World示例:

```bash
cd /path/to/vm
cargo run --example hello_world
```

预期输出:
```
=== VM Hello World 示例 ===
步骤 1: 创建VM配置
✅ 配置创建成功
...
结果验证成功! 10 + 20 = 30
=== 示例完成 ===
```

### 所有示例

运行所有示例:

```bash
# Hello World
cargo run --example hello_world

# Fibonacci计算
cargo run --example fibonacci

# 自定义设备
cargo run --example custom_device

# JIT执行
cargo run --example jit_execution
```

## 示例列表

### 1. Hello World

**位置**: [`hello_world/`](./hello_world/)

**难度**: ⭐ 初学者

**功能**:
- 创建基本的虚拟机
- 加载和执行简单程序
- 读取寄存器和内存
- 验证执行结果

**学习要点**:
- VM配置和初始化
- 内存管理单元(MMU)
- RISC-V指令执行
- 结果验证

**运行**:
```bash
cargo run --example hello_world
```

**文档**: [README](./hello_world/README.md)

---

### 2. Fibonacci计算

**位置**: [`fibonacci/`](./fibonacci/)

**难度**: ⭐⭐ 进阶

**功能**:
- 执行复杂的RISC-V程序
- 使用内存数组存储数据
- 实现循环和控制流
- 验证计算结果

**学习要点**:
- RISC-V循环和条件跳转
- 数组访问和索引计算
- 内存读写操作
- 程序设计和调试

**运行**:
```bash
cargo run --example fibonacci
```

**文档**: [README](./fibonacci/README.md)

---

### 3. 自定义设备

**位置**: [`custom_device/`](./custom_device/)

**难度**: ⭐⭐⭐ 高级

**功能**:
- 创建自定义设备
- 实现MMIO接口
- 处理设备中断
- 模拟设备行为

**学习要点**:
- 设备驱动开发
- 内存映射I/O
- 中断处理
- 设备模拟

**运行**:
```bash
cargo run --example custom_device
```

**文档**: [README](./custom_device/README.md)

---

### 4. JIT编译执行

**位置**: [`jit_execution/`](./jit_execution/)

**难度**: ⭐⭐⭐ 高级

**功能**:
- 使用JIT编译器
- 配置优化选项
- 性能基准测试
- JIT vs 解释器对比

**学习要点**:
- JIT编译原理
- 性能优化技巧
- 基准测试方法
- 配置调优

**运行**:
```bash
cargo run --example jit_execution
```

**文档**: [README](./jit_execution/README.md)

---

## 学习路径

我们推荐按以下顺序学习:

### 第一步: 基础 (1-2天)

1. **Hello World** - 了解基本概念
   - 创建VM
   - 加载程序
   - 执行和验证

2. **阅读文档**
   - [快速入门指南](../docs/tutorials/GETTING_STARTED.md)
   - [RISC-V编程基础](../docs/tutorials/RISCV_PROGRAMMING.md)

### 第二步: 进阶 (3-5天)

3. **Fibonacci** - 学习复杂程序
   - 循环和条件
   - 内存访问
   - 数组操作

4. **RISC-V汇编**
   - 编写自己的程序
   - 使用工具链编译
   - 调试技巧

### 第三步: 高级 (1-2周)

5. **自定义设备** - 设备开发
   - MMIO接口
   - 中断处理
   - 设备模拟

6. **JIT编译** - 性能优化
   - JIT原理
   - 性能分析
   - 优化技巧

7. **高级特性**
   - 跨架构支持
   - 多线程执行
   - 安全特性

### 推荐资源

- [RISC-V规范](https://riscv.org/technical/specifications/)
- [编译器原理](https://www.compilerbook.org/)
- [JIT编译技术](https://en.wikipedia.org/wiki/Just-in-time_compilation)

## RISC-V程序

### 汇编示例

**位置**: [`programs/riscv/`](./programs/riscv/)

包含完整的RISC-V汇编程序示例:

| 程序 | 描述 | 难度 |
|------|------|------|
| [`hello.asm`](./programs/riscv/hello.asm) | Hello World | ⭐ |
| [`fibonacci.asm`](./programs/riscv/fibonacci.asm) | 斐波那契数列 | ⭐⭐ |
| [`prime_numbers.asm`](./programs/riscv/prime_numbers.asm) | 质数筛选 | ⭐⭐⭐ |
| [`matrix_mul.asm`](./programs/riscv/matrix_mul.asm) | 矩阵乘法 | ⭐⭐⭐ |

### 编译RISC-V程序

使用提供的编译脚本:

```bash
cd programs

# 编译单个文件
./assemble.sh hello.asm

# 编译所有文件
./assemble.sh -a

# 生成hexdump和Rust数组
./assemble.sh -a -h -r
```

**要求**:
- RISC-V工具链
- macOS: `brew install riscv-tools`
- Ubuntu: `sudo apt install gcc-riscv64-unknown-elf`

详见: [编译脚本说明](./programs/assemble.sh)

### 在VM中使用编译后的程序

```rust
// 读取编译后的二进制文件
let program = std::fs::read("programs/riscv/hello.bin")?;

// 加载到VM
let code_base = 0x1000u64;
for (i, &byte) in program.iter().enumerate() {
    mmu.write(code_base + i as u64, byte as u64, 1)?;
}

// 执行
engine.set_pc(code_base)?;
engine.execute()?;
```

## 工具和脚本

### 编译脚本

**assemble.sh** - RISC-V汇编编译器

功能:
- 编译.asm文件为.bin
- 生成hexdump
- 生成Rust数组
- 批量编译

使用:
```bash
./assemble.sh --help
```

### 调试工具

#### 反汇编

```bash
riscv64-unknown-elf-objdump -d program.elf
```

#### 符号查看

```bash
riscv64-unknown-elf-objdump -t program.elf
```

#### 十六进制查看

```bash
xxd program.bin
```

## 故障排除

### 编译错误

#### 错误: 找不到vm-core

**解决方案**:
```bash
# 确保在项目根目录
cd /path/to/vm

# 构建所有crate
cargo build --release
```

#### 错误: 示例未找到

**解决方案**:
```bash
# 列出所有示例
cargo example --list

# 使用完整路径
cargo run --example hello_world
```

### 运行时错误

#### 错误: 内存访问违规

**可能原因**:
1. 内存区域未配置
2. 访问权限不正确
3. 地址计算错误

**调试**:
```rust
// 启用详细日志
RUST_LOG=debug cargo run --example hello_world

// 检查内存区域
println!("内存区域: {:?}", mmu.regions());
```

#### 错误: 非法指令

**可能原因**:
1. 程序加载不正确
2. 字节序错误
3. 代码损坏

**调试**:
```bash
# 检查二进制文件
xxd program.bin | head

# 反汇编检查
riscv64-unknown-elf-objdump -D binary -m riscv64 -b binary program.bin
```

### 性能问题

#### JIT未生效

**检查**:
```rust
let config = EngineConfig {
    enable_jit: true,
    jit_threshold: 10,
    ..Default::default()
};

// 查看日志
RUST_LOG=vm_engine::jit=debug cargo run --example jit_execution
```

#### 执行速度慢

**优化**:
1. 使用release模式
2. 启用JIT编译
3. 增加编译阈值
4. 使用批量执行

```bash
# Release模式
cargo run --release --example fibonacci
```

## 贡献示例

欢迎贡献新的示例!

### 示例模板

```rust
//! 示例标题
//!
//! 这个示例展示了如何:
//! - 功能1
//! - 功能2

use anyhow::Result;

fn main() -> Result<()> {
    println!("=== 示例标题 ===\n");

    // 示例代码

    println!("\n=== 示例完成 ===");
    Ok(())
}
```

### 提交示例

1. Fork仓库
2. 创建新分支: `git checkout -b example/my-example`
3. 添加示例代码和文档
4. 提交: `git commit -am "添加新示例"`
5. 推送: `git push origin example/my-example`
6. 创建Pull Request

## 相关文档

### 教程

- [快速入门](../docs/tutorials/GETTING_STARTED.md)
- [RISC-V编程](../docs/tutorials/RISCV_PROGRAMMING.md)
- [高级用法](../docs/tutorials/ADVANCED_USAGE.md)

### API文档

- [vm-core](https://docs.rs/vm-core)
- [vm-engine](https://docs.rs/vm-engine)
- [vm-mem](https://docs.rs/vm-mem)
- [vm-frontend](https://docs.rs/vm-frontend)

### 外部资源

- [RISC-V官方网站](https://riscv.org/)
- [RISC-V GitHub](https://github.com/riscv)
- [Cranelift JIT](https://cranelift.dev/)

## 获取帮助

- **Issues**: [GitHub Issues](https://github.com/example/vm/issues)
- **Discussions**: [GitHub Discussions](https://github.com/example/vm/discussions)
- **Discord**: [加入我们的社区](https://discord.gg/vm-project)

---

开始你的VM之旅吧!
