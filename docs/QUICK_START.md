# VM 快速开始指南

欢迎来到VM项目！本指南将帮助您在5分钟内启动并运行。

---

## 📋 前置要求

### 必需项

- **Rust**: 1.75 或更高版本
  ```bash
  rustc --version  # 应该显示 1.75+
  ```

- **Cargo**: 包含在Rust工具链中
  ```bash
  cargo --version
  ```

### 可选项

- **Git**: 用于克隆仓库
  ```bash
  git --version
  ```

- **构建工具** (根据平台):

  **Linux**:
  ```bash
  sudo apt-get install build-essential libssl-dev pkg-config
  ```

  **macOS**:
  ```bash
  xcode-select --install
  ```

  **Windows**:
  - 安装 [MSVC Build Tools](https://visualstudio.microsoft.com/downloads/)
  - 安装 [CMake](https://cmake.org/download/)

---

## 🚀 5分钟快速开始

### Step 1: 获取代码

```bash
# 克隆仓库
git clone https://github.com/your-org/vm.git
cd vm
```

### Step 2: 构建项目

```bash
# 开发构建 (快速)
cargo build

# 或者发布构建 (优化)
cargo build --release
```

**预期输出**:
```
   Compiling vm-core v0.1.0
   Compiling vm-mem v0.1.0
   Compiling vm-engine v0.1.0
   Compiling vm-engine-jit v0.1.0
   ...
   Finished dev [unoptimized + debuginfo] target(s) in 2m 30s
```

### Step 3: 运行测试

```bash
# 运行所有测试
cargo test --workspace

# 预期: 466个测试全部通过
# test result: ok. 466 passed; 0 failed
```

### Step 4: 运行示例

```bash
# 运行简单VM示例
cargo run --example simple_vm

# 运行JIT执行示例
cargo run --example jit_execution
```

**预期输出**:
```
VM initialized successfully
Loading program...
Program loaded at 0x1000
Starting execution...
Execution result: Ok(0)
VM execution completed
```

🎉 **恭喜！您已经成功运行了VM！**

---

## 📖 下一步

### 学习资源

1. **阅读架构文档**: [docs/ARCHITECTURE.md](ARCHITECTURE.md)
   - 了解DDD分层架构
   - 学习模块职责
   - 理解设计模式

2. **查看示例代码**: [`examples/`](../examples/)
   - `simple_vm.rs` - 简单VM示例
   - `jit_execution.rs` - JIT执行
   - `memory_management.rs` - 内存管理

3. **探索核心模块**:
   - `vm-core` - 核心领域模型
   - `vm-engine-jit` - JIT编译引擎
   - `vm-mem` - 内存管理

---

## 💻 常见用例

### 创建虚拟机

```rust
use vm_core::{VirtualMachine, VmConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建默认配置的VM
    let vm = VirtualMachine::new()?;

    // 或使用自定义配置
    let config = VmConfig::builder()
        .memory_size(1024 * 1024)  // 1MB
        .vcpu_count(2)              // 2个vCPU
        .enable_jit(true)           // 启用JIT
        .build()?;

    let vm = VirtualMachine::new_with_config(config)?;

    Ok(())
}
```

### 加载并执行程序

```rust
use vm_core::VirtualMachine;
use vm_engine_jit::Jit;
use vm_core::ExecutionEngine;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vm = VirtualMachine::new()?;
    let jit = Jit::new();

    // 加载程序
    vm.load_program("path/to/binary")?;

    // 执行
    let result = jit.run(&mut vm)?;
    println!("Result: {:?}", result);

    Ok(())
}
```

### 内存操作

```rust
use vm_mem::MMU;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut mmu = MMU::new();

    // 写入内存
    mmu.write_u32(0x1000, 0x12345678)?;

    // 读取内存
    let value = mmu.read_u32(0x1000)?;
    println!("Read value: 0x{:08x}", value);

    Ok(())
}
```

---

## 🔧 构建选项

### 标准构建

```bash
# 开发构建 (快速编译，未优化)
cargo build

# 发布构建 (优化性能)
cargo build --release

# 特定crate
cargo build -p vm-core
cargo build -p vm-engine-jit --release
```

### 特性构建

```bash
# 启用所有特性
cargo build --all-features

# 启用JIT优化
cargo build --features "jit-optimizations"

# 启用硬件加速 (Linux)
cargo build --features "kvm"

# 启用硬件加速 (macOS)
cargo build --features "hvf"

# 启用硬件加速 (Windows)
cargo build --features "whpx"
```

### 自定义优化

```bash
# 使用LTO (链接时间优化)
cargo build --release --features lto

# 并行编译 (利用所有CPU核心)
cargo build --release -j $(nproc)

# 指定目标架构
cargo build --target x86_64-unknown-linux-gnu
cargo build --target aarch64-unknown-linux-gnu
cargo build --target riscv64gc-unknown-linux-gnu
```

---

## 🧪 测试

### 运行测试

```bash
# 所有测试
cargo test --workspace

# 特定crate
cargo test -p vm-core
cargo test -p vm-engine-jit

# 显示输出
cargo test -- --nocapture

# 运行被忽略的测试
cargo test -- --ignored
```

### 测试覆盖率

```bash
# 安装llvm-cov
cargo install cargo-llvm-cov

# 生成覆盖率报告
cargo llvm-cov --workspace

# HTML报告 (在浏览器中查看)
cargo llvm-cov --workspace --html --output-dir coverage

# 在浏览器中打开
open coverage/index.html  # macOS
xdg-open coverage/index.html  # Linux
```

### 基准测试

```bash
# 运行所有基准
cargo bench --workspace

# 特定基准
cargo bench -p vm-engine-jit --bench simd

# 比较基准
cargo bench -- --save-baseline main
# ... 做一些改动 ...
cargo bench -- --baseline main
```

---

## 📚 文档

### 生成文档

```bash
# 生成并打开文档
cargo doc --open --workspace

# 包含私有项
cargo doc --open --workspace --document-private-items

# 所有特性的文档
cargo doc --open --workspace --all-features
```

### 在线文档

生成的文档将在 `target/doc/` 目录中，并在浏览器中自动打开。

---

## 🐛 故障排除

### 常见问题

#### 1. 编译错误: `error: linker 'link.exe' not found`

**Windows**: 安装 [MSVC Build Tools](https://visualstudio.microsoft.com/downloads/)

#### 2. 权限错误: `/dev/kvm` permission denied

**Linux**:
```bash
# 将用户添加到kvm组
sudo usermod -aG kvm $USER

# 重新登录或运行
newgrp kvm
```

#### 3. 内存不足

```bash
# 减少并行任务
cargo build -j 2

# 或增加交换空间
# Linux
sudo fallocate -l 4G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
```

#### 4. SSL错误

```bash
# Ubuntu/Debian
sudo apt-get install libssl-dev pkg-config

# Fedora
sudo dnf install openssl-devel

# macOS (使用Homebrew)
brew install openssl
```

### 获取帮助

如果问题仍然存在：

1. 查看 [GitHub Issues](https://github.com/your-org/vm/issues)
2. 搜索或提问在 [GitHub Discussions](https://github.com/your-org/vm/discussions)
3. 发送邮件到 your-email@example.com

---

## 🎓 学习路径

### 初学者 (第1-2周)

1. ✅ 完成本快速开始指南
2. 📖 阅读 [docs/ARCHITECTURE.md](ARCHITECTURE.md)
3. 💻 运行所有示例代码
4. 🧪 运行并理解测试
5. 📝 阅读 `vm-core` 源代码

### 中级开发者 (第3-4周)

1. 🔧 修改示例代码，实验
2. 🚀 深入学习 JIT 编译 (`vm-engine-jit`)
3. 💾 理解内存管理 (`vm-mem`)
4. 🎮 探索设备仿真 (`vm-device`)
5. 📊 查看基准测试和性能

### 高级开发者 (第5-8周)

1. 🏗️ 理解DDD架构和设计模式
2. ⚡ 优化性能 (SIMD, 缓存, TLB)
3. 🔌 开发插件或扩展
4. 🌐 贡献跨架构支持
5. 🤝 参与开源贡献

---

## 🔗 有用链接

- **主仓库**: [https://github.com/your-org/vm](https://github.com/your-org/vm)
- **文档**: [https://docs.your-org.com/vm](https://docs.your-org.com/vm)
- **API文档**: [https://docs.rs/vm-core](https://docs.rs/vm-core)
- **示例**: [examples/](../examples/)
- **博客**: [https://blog.your-org.com](https://blog.your-org.com)

---

## 💡 提示和技巧

### 1. 使用Cargo别名

在 `.cargo/config.toml` 中添加:

```toml
[alias]
b = "build --release"
t = "test --workspace"
d = "doc --open --workspace"
br = "build --release && bench --workspace"
```

然后可以快速运行:
```bash
cargo b  # 构建 (release)
cargo t  # 测试
cargo d  # 文档
cargo br # 基准
```

### 2. 加速编译

```bash
# 使用 Rust nightly (更快编译)
rustup default nightly

# 或使用 sccache (缓存编译)
cargo install sccache
export RUSTC_WRAPPER=sccache
```

### 3. 监控构建

```bash
# 使用 cargo-watch (自动重编译)
cargo install cargo-watch
cargo watch -x build

# 使用 cargo-make (任务自动化)
cargo install cargo-make
```

---

## 🎯 下一步

现在您已经熟悉了基础：

1. 📖 阅读 [docs/CONTRIBUTING.md](CONTRIBUTING.md) 了解如何贡献
2. 🐛 查看 [GitHub Issues](https://github.com/your-org/vm/issues) 寻找要解决的问题
3. 💬 加入 [GitHub Discussions](https://github.com/your-org/vm/discussions) 社区讨论
4. 🚀 开始构建您的第一个VM应用！

---

**祝您使用VM愉快！** 🎉

如有问题，请随时联系我们。记住，唯一愚蠢的问题是您不问的问题！

---

**文档维护**: VM团队
**最后更新**: 2026-01-06
**版本**: 1.0
