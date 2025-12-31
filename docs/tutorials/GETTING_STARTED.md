# VM项目快速入门指南

欢迎来到VM项目!这是一个高性能的虚拟机实现,支持多种架构和执行引擎。

## 目录

- [环境准备](#环境准备)
- [安装](#安装)
- [第一个VM程序](#第一个vm程序)
- [执行和调试](#执行和调试)
- [常见问题](#常见问题)
- [下一步](#下一步)

## 环境准备

### 系统要求

- **操作系统**: Linux, macOS, 或 Windows
- **Rust版本**: 1.85或更高
- **内存**: 至少4GB RAM
- **磁盘空间**: 至少2GB可用空间

### 安装Rust

如果还没有安装Rust,请运行:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

这将安装最新稳定版的Rust工具链。

### 验证安装

```bash
rustc --version
cargo --version
```

应该显示类似:
```
rustc 1.85.0 (...)
cargo 1.85.0 (...)
```

## 安装

### 1. 克隆仓库

```bash
git clone https://github.com/example/vm.git
cd vm
```

### 2. 构建项目

```bash
# 构建所有crate
cargo build --release

# 或者构建特定组件
cargo build --release -p vm-engine
cargo build --release -p vm-core
```

### 3. 运行测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test --test integration_tests
```

## 第一个VM程序

让我们创建一个简单的虚拟机程序,计算两个数的和。

### 创建项目

```bash
# 创建新的binary crate
cargo new my_first_vm
cd my_first_vm
```

### 编辑Cargo.toml

```toml
[package]
name = "my_first_vm"
version = "0.1.0"
edition = "2024"

[dependencies]
vm-core = { path = "../vm-core" }
vm-engine = { path = "../vm-engine" }
vm-mem = { path = "../vm-mem" }
vm-frontend = { path = "../vm-frontend" }
anyhow = "1.0"
log = "0.4"
env_logger = "0.11"
```

### 编写代码

编辑`src/main.rs`:

```rust
use anyhow::Result;
use vm_core::{GuestArch, VmConfig};
use vm_engine::{ExecutionEngine, ExecutionResult};
use vm_mem::{MemoryRegion, SoftMmu, MemRegionFlags};

fn main() -> Result<()> {
    // 初始化日志
    env_logger::init();

    println!("Hello VM!");

    // 创建VM配置
    let config = VmConfig {
        arch: GuestArch::Riscv64,
        memory_size: 1024 * 1024,  // 1MB
        ..Default::default()
    };

    // 创建内存
    let mut mmu = SoftMmu::new(config.memory_size);
    mmu.add_region(MemoryRegion {
        base: 0x1000,
        size: 0x1000,
        flags: MemRegionFlags::READ | MemRegionFlags::EXEC,
    })?;

    // 准备RISC-V程序: 10 + 20
    let program: Vec<u8> = vec![
        0x93, 0x00, 0xA0, 0x00,  // li  x1, 10
        0x13, 0x01, 0x40, 0x01,  // li  x2, 20
        0xB3, 0x01, 0x21, 0x00,  // add x3, x1, x2
        0x67, 0x80, 0x00, 0x00,  // ret
    ];

    // 加载程序
    let code_base = 0x1000u64;
    for (i, &byte) in program.iter().enumerate() {
        mmu.write(code_base + i as u64, byte as u64, 1)?;
    }

    // 创建执行引擎
    let mut engine = ExecutionEngine::new(
        config.arch,
        Box::new(mmu),
        Default::default()
    )?;

    engine.set_pc(code_base)?;

    // 执行程序
    println!("执行程序...");
    loop {
        match engine.execute_step()? {
            ExecutionResult::Continue => {}
            ExecutionResult::Halted => break,
            ExecutionResult::Exception(e) => {
                println!("异常: {:?}", e);
                break;
            }
        }
    }

    // 读取结果
    let result = engine.read_register(3)?;
    println!("结果: 10 + 20 = {}", result);

    Ok(())
}
```

### 运行程序

```bash
cargo run
```

预期输出:
```
Hello VM!
执行程序...
结果: 10 + 20 = 30
```

## 执行和调试

### 运行示例

项目包含多个示例程序:

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

### 调试技巧

#### 1. 启用详细日志

```bash
RUST_LOG=debug cargo run
```

#### 2. 使用GDB/LLDB

```bash
# 编译调试版本
cargo build

# 使用gdb(Linux)
gdb target/debug/my_first_vm

# 使用lldb(macOS)
lldb target/debug/my_first_vm
```

常用命令:
```
(b)reak main        # 设置断点
(r)un               # 运行
(s)tep              # 单步执行
(n)ext              # 下一步
(p)rint x1          # 打印变量
```

#### 3. 添加日志

在代码中添加调试信息:

```rust
use log::{debug, info, warn};

debug!("寄存器值: x1={}, x2={}", x1, x2);
info!("程序开始执行");
warn!("检测到异常");
```

#### 4. 使用VM的调试功能

```rust
// 启用调试模式
let config = EngineConfig {
    debug_mode: true,
    trace_execution: true,
    ..Default::default()
};

// 每执行一条指令后检查状态
for _ in 0..100 {
    engine.execute_step()?;

    let pc = engine.pc();
    let x1 = engine.read_register(1)?;
    println!("PC=0x{:x}, x1={}", pc, x1);
}
```

## 常见问题

### 编译错误

#### Q: 编译时出现"error: linker `cc` not found"

A: 需要安装C编译器:
```bash
# Ubuntu/Debian
sudo apt install build-essential

# macOS
xcode-select --install

# Fedora
sudo dnf install gcc
```

#### Q: 依赖解析失败

A: 清理并重新构建:
```bash
cargo clean
cargo update
cargo build
```

### 运行时错误

#### Q: "找不到动态库"

A: 确保链接器配置正确:
```bash
# Linux
export LD_LIBRARY_PATH=/usr/local/lib

# macOS
export DYLD_LIBRARY_PATH=/usr/local/lib
```

#### Q: 程序执行异常

A: 检查:
1. 程序是否正确加载到内存
2. PC是否设置正确
3. 内存区域是否配置正确
4. 程序指令是否合法

```rust
// 添加检查
assert!(code_base >= region.base);
assert!(code_base + program.len() as u64 <= region.base + region.size);
```

### 性能问题

#### Q: 执行速度很慢

A: 考虑:
1. 启用JIT编译
2. 使用release模式
3. 调整JIT阈值

```rust
// 使用优化的配置
let config = EngineConfig {
    enable_jit: true,
    jit_threshold: 100,
    optimization_level: 2,
    ..Default::default()
};
```

#### Q: 内存占用高

A:
1. 减少VM内存大小
2. 限制JIT缓存大小
3. 使用解释器模式

```rust
let config = VmConfig {
    memory_size: 256 * 1024,  // 256KB而不是1MB
    ..Default::default()
};
```

## 下一步

恭喜!你已经成功运行了第一个VM程序。接下来可以:

### 学习更多示例

1. [Fibonacci示例](../../examples/fibonacci/) - 更复杂的程序
2. [自定义设备示例](../../examples/custom_device/) - 添加I/O设备
3. [JIT执行示例](../../examples/jit_execution/) - 性能优化

### 阅读教程

1. [RISC-V编程指南](./RISCV_PROGRAMMING.md) - 学习RISC-V汇编
2. [高级用法](./ADVANCED_USAGE.md) - 深入了解VM功能

### 探索源代码

- `vm-core` - 核心数据结构和定义
- `vm-engine` - 执行引擎实现
- `vm-frontend` - 指令集前端
- `vm-mem` - 内存管理

### 参与贡献

欢迎贡献代码!请阅读[CONTRIBUTING.md](../../CONTRIBUTING.md)

## 获取帮助

- **文档**: [https://docs.rs/vm](https://docs.rs/vm)
- **GitHub Issues**: [https://github.com/example/vm/issues](https://github.com/example/vm/issues)
- **Discord**: [加入我们的Discord社区](https://discord.gg/vm-project)

---

祝你使用VM项目愉快!
