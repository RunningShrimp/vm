# vm-cli

**VM项目命令行工具**

[![Rust](https://img.shields.io/badge/rust-2024%20Edition-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

---

## 概述

`vm-cli` 是VM项目的主要命令行接口工具，提供用户友好的命令行界面来配置和运行虚拟机。它集成了所有VM核心功能，包括硬件加速、JIT编译、GPU直通等，是用户与VM系统交互的主要方式。

## 🎯 核心功能

- **虚拟机配置**: 灵活的VM配置选项（内存、CPU、磁盘等）
- **执行模式选择**: 解释器、JIT编译器、混合模式
- **硬件加速**: 自动检测和使用KVM、HVF、WHPX等硬件加速
- **GPU直通**: 支持CUDA、ROCm等GPU加速
- **JIT调优**: 可配置的JIT编译参数
- **硬件检测**: 自动检测主机硬件能力
- **调试支持**: GDB调试接口和详细日志

## 📦 安装

```bash
# 从源码构建
cargo build --release --package vm-cli

# 安装到系统
cargo install --path .

# 或者使用预构建二进制
wget https://github.com/your-org/vm/releases/latest/download/vm-cli
chmod +x vm-cli
sudo mv vm-cli /usr/local/bin/
```

## 🚀 快速开始

### 基础使用

```bash
# 启动简单的虚拟机
vm-cli --kernel vmlinux --disk rootfs.ext4

# 配置内存和CPU
vm-cli --kernel vmlinux --memory 512M --vcpus 2

# 启用硬件加速
vm-cli --kernel vmlinux --enable-accel

# 使用JIT编译器
vm-cli --kernel vmlinux --mode jit

# 调试模式
vm-cli --kernel vmlinux --debug
```

### 高级配置

```bash
# 完整配置示例
vm-cli \
  --kernel vmlinux \
  --disk rootfs.ext4 \
  --memory 1G \
  --vcpus 4 \
  --mode jit \
  --enable-accel \
  --gpu-backend cuda \
  --jit-min-threshold 1000 \
  --jit-max-threshold 10000 \
  --jit-sample-window 1000 \
  --jit-compile-weight 0.7 \
  --jit-benefit-weight 0.3 \
  --debug

# 硬件检测
vm-cli --detect-hw

# 混合执行模式
vm-cli --kernel vmlinux --mode hybrid --jit-share-pool
```

## 📋 命令行选项

### 基本选项

| 选项 | 短选项 | 说明 | 默认值 |
|------|--------|------|--------|
| `--kernel` | `-k` | 内核镜像路径 | None |
| `--disk` | `-d` | 磁盘镜像路径 | None |
| `--memory` | `-m` | 内存大小 (支持K/M/G后缀) | 128M |
| `--vcpus` | `-c` | 虚拟CPU数量 | 1 |
| `--mode` | `-M` | 执行模式 (interpreter/jit/hybrid) | interpreter |

### 加速选项

| 选项 | 说明 | 默认值 |
|------|------|--------|
| `--enable-accel` | 启用硬件加速 (KVM/HVF/WHPX) | false |
| `--gpu-backend` | GPU后端 (cuda/rocm/none) | None |
| `--detect-hw` | 检测主机硬件能力并退出 | false |

### JIT调优选项

| 选项 | 说明 | 默认值 |
|------|------|--------|
| `--jit-min-threshold` | JIT编译最小执行次数阈值 | 100 |
| `--jit-max-threshold` | JIT编译最大执行次数阈值 | 10000 |
| `--jit-sample-window` | JIT采样窗口大小 | 1000 |
| `--jit-compile-weight` | JIT编译时间权重 (0.0-1.0) | 0.5 |
| `--jit-benefit-weight` | JIT性能收益权重 (0.0-1.0) | 0.5 |
| `--jit-share-pool` | JIT共享代码池 | true |

### 调试选项

| 选项 | 说明 | 默认值 |
|------|------|--------|
| `--debug` | 启用调试模式 (GDB服务器) | false |
| `--trace` | 启用详细执行跟踪 | false |
| `--log-level` | 日志级别 (error/warn/info/debug/trace) | info |

## 🔧 配置文件

除了命令行参数，vm-cli也支持配置文件：

**~/.vm/config.toml**:
```toml
[vm]
memory = "512M"
vcpus = 2
mode = "jit"
enable_accel = true

[jit]
min_threshold = 1000
max_threshold = 10000
sample_window = 1000
compile_weight = 0.7
benefit_weight = 0.3
share_pool = true

[gpu]
backend = "cuda"

[debug]
enabled = false
log_level = "info"
```

## 📊 硬件检测

vm-cli可以自动检测主机硬件能力：

```bash
$ vm-cli --detect-hw

=== Hardware Detection Results ===
Host Architecture: x86_64
Host OS: Linux 6.5.0

CPU Features:
  - VMX (Intel VT-x): ✓ Supported
  - RDTSCP: ✓ Supported
  - SSE4.2: ✓ Supported
  - AVX: ✓ Supported
  - AVX2: ✓ Supported

Hardware Acceleration:
  - KVM: ✓ Available
  - HVF: ✗ Not available
  - WHPX: ✗ Not available

GPU Capabilities:
  - NVIDIA CUDA: ✓ Available (Device: NVIDIA GeForce RTX 3090)
  - AMD ROCm: ✗ Not detected

Recommendations:
  - Use KVM for best performance
  - Enable JIT compilation
  - CUDA GPU acceleration available
```

## 🎨 使用场景

### 场景1: 开发测试

```bash
# 快速启动开发VM
vm-cli --kernel vmlinux --disk test.ext4 --memory 256M --debug
```

### 场景2: 高性能生产

```bash
# 生产环境配置
vm-cli \
  --kernel vmlinux \
  --disk rootfs.ext4 \
  --memory 4G \
  --vcpus 8 \
  --mode jit \
  --enable-accel \
  --jit-min-threshold 500 \
  --jit-max-threshold 5000
```

### 场景3: GPU加速计算

```bash
# 使用CUDA GPU加速
vm-cli \
  --kernel vmlinux \
  --memory 2G \
  --vcpus 4 \
  --gpu-backend cuda \
  --mode jit
```

### 场景4: 跨架构测试

```bash
# 在x86_64主机上运行ARM64 VM
vm-cli \
  --kernel vmlinux-arm64 \
  --arch arm64 \
  --mode jit \
  --enable-accel
```

## 📝 环境变量

vm-cli也支持通过环境变量配置：

| 环境变量 | 说明 | 示例 |
|----------|------|------|
| `VM_MEMORY` | 默认内存大小 | `VM_MEMORY=1G` |
| `VM_VCPUS` | 默认CPU数量 | `VM_VCPUS=4` |
| `VM_MODE` | 执行模式 | `VM_MODE=jit` |
| `VM_ENABLE_ACCEL` | 启用硬件加速 | `VM_ENABLE_ACCEL=1` |
| `VM_GPU_BACKEND` | GPU后端 | `VM_GPU_BACKEND=cuda` |
| `VM_LOG_LEVEL` | 日志级别 | `VM_LOG_LEVEL=debug` |

## 🔌 与其他模块集成

vm-cli集成了以下VM项目模块：

- **vm-core**: 核心VM功能
- **vm-engine**: 执行引擎
- **vm-accel**: 硬件加速
- **vm-passthrough**: 设备直通（包括GPU）
- **vm-device**: 设备仿真
- **vm-service**: VM服务层
- **vm-frontend**: 前端指令解码
- **vm-osal**: 操作系统抽象层

## 📚 相关文档

- [vm-core](../vm-core/README.md) - 核心VM功能
- [vm-engine](../vm-engine/README.md) - 执行引擎
- [vm-accel](../vm-accel/README.md) - 硬件加速
- [vm-passthrough](../vm-passthrough/README.md) - 设备直通
- [DEPLOYMENT_GUIDE](../DEPLOYMENT_GUIDE.md) - 部署指南
- [MASTER_DOCUMENTATION_INDEX](../MASTER_DOCUMENTATION_INDEX.md) - 完整文档索引

## ⚠️ 注意事项

1. **权限要求**: 使用硬件加速需要适当的权限（如/dev/kvm访问）
2. **GPU要求**: CUDA需要NVIDIA GPU和驱动，ROCm需要AMD GPU
3. **内存限制**: 确保主机有足够的物理内存
4. **调试性能**: 调试模式会显著降低性能

## 🤝 贡献指南

如果您想改进vm-cli：

1. 确保新功能有命令行选项和文档
2. 添加错误处理和用户友好的错误消息
3. 更新本README和帮助文本
4. 添加使用示例

## 🐛 故障排查

### 常见问题

**Q: 硬件加速无法启用**
```bash
# 检查KVM访问权限
ls -l /dev/kvm

# 如果权限不足，添加用户到kvm组
sudo usermod -a -G kvm $USER
```

**Q: CUDA GPU不可用**
```bash
# 检查NVIDIA驱动
nvidia-smi

# 检查CUDA安装
nvcc --version
```

**Q: JIT编译导致性能下降**
```bash
# 调整JIT阈值
vm-cli --kernel vmlinux --jit-min-threshold 5000 --mode hybrid
```

## 📝 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](../LICENSE) 文件

---

**包版本**: workspace v0.1.0
**Rust版本**: 2024 Edition
**最后更新**: 2026-01-07
