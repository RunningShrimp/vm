# VM Project v0.1.0 快速入门指南

欢迎来到VM Project！本指南将帮助你在10分钟内快速上手VM Project。

---

## 目录

1. [系统要求](#系统要求)
2. [安装](#安装)
3. [快速示例](#快速示例)
4. [核心概念](#核心概念)
5. [常见使用场景](#常见使用场景)
6. [下一步](#下一步)
7. [获取帮助](#获取帮助)

---

## 系统要求

### 最低要求
- **操作系统**: Linux 5.10+, macOS 11.0+, 或 Windows 10+
- **Rust**: 1.85 或更高版本
- **内存**: 4GB RAM
- **磁盘**: 500MB 可用空间
- **CPU**: 支持64位的处理器

### 推荐配置
- **操作系统**: Linux 5.15+ 或 macOS 12.0+
- **内存**: 8GB+ RAM
- **CPU**: 4核以上
- **GPU**: 支持Vulkan/Metal/DX12 (用于GPU加速)

---

## 安装

### 方式1: 从源码构建 (推荐)

```bash
# 1. 克隆仓库
git clone https://github.com/example/vm.git
cd vm

# 2. 构建项目
cargo build --release

# 3. 验证安装
./target/release/vm-cli --version
```

### 方式2: 使用Cargo添加到项目

```bash
# 创建新项目
cargo new my_vm_app
cd my_vm_app

# 添加依赖
cargo add vm-core
cargo add vm-engine
cargo add vm-frontend

# 或者添加到 Cargo.toml
# [dependencies]
# vm-core = "0.1.0"
# vm-engine = "0.1.0"
# vm-frontend = "0.1.0"
```

### 方式3: 使用预编译二进制文件 (可选)

下载最新发布版本并解压：
```bash
wget https://github.com/example/vm/releases/download/v0.1.0/vm-0.1.0-linux-x86_64.tar.gz
tar xzf vm-0.1.0-linux-x86_64.tar.gz
./vm-cli --version
```

---

## 快速示例

### 示例1: Hello World (RISC-V)

创建一个简单的RISC-V程序：

```rust
// examples/hello_world.rs
use vm_core::{Vm, VmConfig};
use vm_frontend::riscv64;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建VM配置
    let config = VmConfig {
        memory_size: 1024 * 1024, // 1MB
        ..Default::default()
    };

    // 创建VM实例
    let mut vm = Vm::new(config)?;

    // 加载程序
    vm.load_program_bytes(&[0x13, 0x05, 0xa0, 0x00])?; // addi a0, zero, 10

    // 运行
    vm.run().await?;

    println!("VM执行完成!");

    Ok(())
}
```

运行：
```bash
cargo run --example hello_world
```

### 示例2: 使用CLI工具

```bash
# 运行RISC-V程序
vm-cli run --arch riscv64 program.elf

# 启用JIT编译
vm-cli run --jit --arch riscv64 program.elf

# 指定内存大小
vm-cli run --memory 512M --arch riscv64 program.elf

# 启用GPU加速
vm-cli run --gpu --arch riscv64 program.elf

# 调试模式
vm-cli run --debug --arch riscv64 program.elf
```

### 示例3: 编译并运行RISC-V程序

```bash
# 1. 编写RISC-V汇编程序
cat > hello.s << 'EOF'
    .section .text
    .global _start

_start:
    li a7, 64      # syscall: write
    li a0, 1       # fd: stdout
    la a1, msg     # buffer
    li a2, 13      # count
    ecall          # call syscall

    li a7, 93      # syscall: exit
    li a0, 0       # exit code
    ecall

    .section .rodata
msg:
    .string "Hello, RISC-V!\n"
EOF

# 2. 编译为ELF
riscv64-unknown-elf-gcc -nostdlib -o hello.elf hello.s

# 3. 运行
vm-cli run hello.elf
```

---

## 核心概念

### 架构组件

```
┌─────────────────────────────────────┐
│         vm-desktop / vm-cli          │  用户界面
└─────────────────────────────────────┘
                ↓
┌─────────────────────────────────────┐
│            vm-core                   │  VM核心
│  - Vm生命周期管理                    │
│  - 事件系统                          │
│  - 插件系统                          │
└─────────────────────────────────────┘
                ↓
┌──────────────┬──────────────┬─────────┐
│ vm-frontend  │  vm-engine   │ vm-mem  │
│ (指令集)     │  (执行引擎)  │(内存)   │
│              │              │         │
│ - RISC-V     │ - JIT编译器  │ - MMU   │
│ - ARM64      │ - 解释器     │ - TLB   │
└──────────────┴──────────────┴─────────┘
                ↓
┌──────────────┬──────────────┬─────────┐
│ vm-device    │  vm-gpu      │vm-accel │
│ (设备)       │  (GPU)       │(加速)   │
│              │              │         │
│ - VirtIO     │ - wgpu       │ - NUMA  │
│ - PCI        │ - 渲染       │ - 亲和性│
└──────────────┴──────────────┴─────────┘
```

### 执行模式

#### 1. 解释执行
```rust
let config = ExecutorConfig {
    mode: ExecutionMode::Interpreted,
    ..Default::default()
};
```
- 适合调试
- 内存占用小
- 执行速度较慢

#### 2. JIT编译
```rust
let config = ExecutorConfig {
    mode: ExecutionMode::Jit,
    optimization_level: OptimizationLevel::High,
    ..Default::default()
};
```
- 性能最优
- 冷启动有开销
- 适合生产环境

### 内存管理

#### NUMA优化
```rust
let config = MemoryConfig {
    numa_policy: NumaPolicy::Bind,
    numa_nodes: vec![0, 1],
    ..Default::default()
};
```

#### TLB优化
- 自动TLB刷新
- 多级TLB缓存
- Lock-free实现

---

## 常见使用场景

### 场景1: 操作系统开发

测试你的操作系统：

```rust
use vm_core::{Vm, VmConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = VmConfig {
        memory_size: 16 * 1024 * 1024, // 16MB
        enable_devices: true,
        ..Default::default()
    };

    let mut vm = Vm::new(config)?;
    vm.load_elf("my_os.elf")?;
    vm.run().await?;

    Ok(())
}
```

### 场景2: 嵌入式测试

测试嵌入式程序：

```bash
# 编译嵌入式程序
riscv64-unknown-elf-gcc -march=rv64gc -o firmware.elf firmware.c

# 在VM中测试
vm-cli run --arch riscv64 --memory 2M firmware.elf
```

### 场景3: 性能基准测试

```bash
# 运行内置基准测试
cargo bench --workspace

# 运行特定基准测试
cargo bench -p vm-engine -- jit_compilation
cargo bench -p vm-mem -- memory_access

# 生成基准报告
cargo bench --workspace -- --save-baseline main
```

### 场景4: 调试

使用GDB调试：

```bash
# 启动VM并监听GDB
vm-cli debug --arch riscv64 --gdb-listen 1234 program.elf

# 在另一个终端连接GDB
riscv64-unknown-elf-gdb program.elf
(gdb) target remote :1234
(gdb) break main
(gdb) continue
```

### 场景5: 批量测试

```bash
# 运行所有测试
cargo test --workspace

# 运行集成测试
cargo test --test integration_tests

# 运行属性测试
cargo test --test instruction_property_tests

# 带覆盖率报告
cargo test --workspace -- --nocapture
```

---

## 配置选项

### CLI配置文件

创建 `vm-config.toml`:

```toml
[vm]
memory_size = "512M"
cpu_count = 1
enable_jit = true
enable_gpu = false

[memory]
numa_policy = "interleave"
tlb_size = 1024
enable_huge_pages = false

[jit]
optimization_level = "high"
cache_size = "64M"
enable_branch_prediction = true

[devices]
enable_virtio_block = true
enable_virtio_net = false
enable_virtio_console = true

[debug]
log_level = "info"
enable_tracing = false
```

使用配置文件：
```bash
vm-cli run --config vm-config.toml program.elf
```

---

## 性能优化技巧

### 1. 启用JIT
```bash
vm-cli run --jit --opt-level=3 program.elf
```

### 2. NUMA绑定
```rust
let config = VmConfig {
    numa_policy: NumaPolicy::Bind,
    preferred_numa_node: 0,
    ..Default::default()
};
```

### 3. 大页内存
```bash
# 启用大页 (需要root权限)
sudo sysctl vm.nr_hugepages=128
vm-cli run --huge-pages program.elf
```

### 4. GPU加速
```bash
vm-cli run --gpu --arch riscv64 program.elf
```

### 5. 并行编译
```bash
vm-cli run --jit --parallel-compiler program.elf
```

---

## 故障排除

### 问题1: 编译失败

```bash
# 清理构建缓存
cargo clean

# 重新构建
cargo build --release
```

### 问题2: 运行时错误

```bash
# 启用详细日志
RUST_LOG=debug vm-cli run program.elf

# 启用回溯
RUST_BACKTRACE=1 vm-cli run program.elf
```

### 问题3: 性能问题

```bash
# 运行性能分析
cargo bench -- workspace

# 检查配置
vm-cli run --debug --profile program.elf
```

### 问题4: 设备问题

```bash
# 检查设备支持
vm-cli info --devices

# 禁用有问题的设备
vm-cli run --no-virtio-net program.elf
```

---

## 下一步

### 学习资源

1. **完整文档**
   - [API文档](https://docs.rs/vm)
   - [架构设计](docs/architecture/)
   - [教程指南](docs/tutorials/)

2. **示例代码**
   - [examples/hello_world/](examples/hello_world/)
   - [examples/fibonacci/](examples/fibonacci/)
   - [examples/jit_execution/](examples/jit_execution/)

3. **测试代码**
   - [vm-core/tests/](vm-core/tests/)
   - [vm-engine/tests/](vm-engine/tests/)

### 进阶主题

- [自定义设备开发](docs/tutorials/device_development.md)
- [JIT编译器优化](docs/tutorials/jit_optimization.md)
- [内存管理深入](docs/tutorials/memory_management.md)
- [性能调优指南](docs/tutorials/performance_tuning.md)

### 贡献

我们欢迎贡献！查看：
- [贡献指南](CONTRIBUTING.md)
- [行为准则](CODE_OF_CONDUCT.md)
- [问题追踪](https://github.com/example/vm/issues)

---

## 获取帮助

### 文档
- **快速参考**: [QUICK_START.md](QUICK_START_v0.1.0.md)
- **完整文档**: https://docs.rs/vm
- **示例代码**: examples/

### 社区
- **GitHub Issues**: https://github.com/example/vm/issues
- **GitHub Discussions**: https://github.com/example/vm/discussions
- **Discord**: https://discord.gg/vm-project

### 报告问题
- Bug报告: [GitHub Issues](https://github.com/example/vm/issues/new?template=bug_report.md)
- 功能请求: [GitHub Issues](https://github.com/example/vm/issues/new?template=feature_request.md)
- 安全问题: security@example.com

---

## 常用命令速查

```bash
# 查看版本
vm-cli --version

# 查看帮助
vm-cli --help
vm-cli run --help

# 运行程序
vm-cli run [--jit] [--gpu] [--debug] [--memory SIZE] [--arch ARCH] <PROGRAM>

# 调试程序
vm-cli debug [--gdb-listen PORT] <PROGRAM>

# 查看信息
vm-cli info [--cpu] [--memory] [--devices]

# 运行测试
cargo test --workspace

# 运行基准测试
cargo bench --workspace

# 生成文档
cargo doc --workspace --open

# 代码检查
cargo clippy --workspace
cargo fmt --check
```

---

## 祝你使用愉快！

VM Project v0.1.0 是我们的首次正式发布。如果你有任何问题或建议，欢迎通过上面的方式联系我们。

**Happy Virtualizing! 🚀**

---

**版本**: v0.1.0
**最后更新**: 2025-12-31
**反馈**: https://github.com/example/vm/issues
