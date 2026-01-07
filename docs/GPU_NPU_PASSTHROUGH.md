# GPU/NPU 直通功能治理文档

本文档描述了 GPU/NPU 直通功能的分阶段实现计划、当前状态和使用指南。

**最后更新**: 2024年（基于现代化升级计划）

## 目录

1. [功能概述](#功能概述)
2. [当前状态](#当前状态)
3. [分阶段实现计划](#分阶段实现计划)
4. [Feature 标志](#feature-标志)
5. [使用指南](#使用指南)
6. [环境要求](#环境要求)
7. [已知限制](#已知限制)
8. [未来计划](#未来计划)

---

## 功能概述

GPU/NPU 直通功能允许虚拟机直接访问物理 GPU 或 NPU 设备，实现硬件加速计算。支持以下加速器：

- **NVIDIA GPU**: 通过 CUDA API
- **AMD GPU**: 通过 ROCm API
- **ARM NPU**: 通过厂商特定 API（实验性）

### 主要功能

1. **设备枚举**: 扫描系统中的 PCIe GPU/NPU 设备
2. **设备直通**: 将物理设备附加到虚拟机
3. **JIT 编译**: 将 IR 代码编译为 GPU/NPU 可执行代码
4. **内存管理**: 设备内存分配和主机-设备内存传输
5. **计算执行**: 在加速器上执行计算内核

---

## 当前状态

### 实现完成度

| 组件 | CUDA | ROCm | ARM NPU | 状态 |
|------|------|------|---------|------|
| 设备枚举 | ✅ | ✅ | ✅ | 完成 |
| PCIe 直通 | ✅ | ✅ | ✅ | 完成 |
| 设备初始化 | ⚠️ | ⚠️ | ⚠️ | 部分实现 |
| 内存管理 | ⚠️ | ⚠️ | ⚠️ | 部分实现 |
| JIT 编译 | ⚠️ | ⚠️ | ❌ | 部分实现 |
| 内核执行 | ❌ | ❌ | ❌ | 未实现 |

**图例**:
- ✅ 完全实现
- ⚠️ 部分实现（有 TODO 标记）
- ❌ 未实现

### 已知问题

1. **编译依赖**: CUDA 需要 CUDA SDK，ROCm 需要 ROCm SDK
2. **平台支持**: 主要在 Linux 上测试，Windows/macOS 支持有限
3. **功能完整性**: 部分 API 调用使用占位实现（标记为 TODO）

---

## 分阶段实现计划

### 阶段 1: 基础架构 ✅ (已完成)

**目标**: 建立设备枚举和直通基础架构

**完成内容**:
- ✅ PCIe 设备扫描
- ✅ 设备分类和识别
- ✅ 直通管理器框架
- ✅ Feature 标志系统

**状态**: 已完成

---

### 阶段 2: 设备管理 ⚠️ (进行中)

**目标**: 实现设备初始化和生命周期管理

**当前状态**:
- ✅ 设备枚举
- ⚠️ 设备初始化（部分实现，有 TODO）
- ⚠️ 设备清理（部分实现）
- ❌ 设备热插拔

**待完成**:
- [ ] 完成 CUDA 设备初始化
- [ ] 完成 ROCm 设备初始化
- [ ] 实现设备清理逻辑
- [ ] 添加设备状态监控

**预计时间**: 2-3 周

---

### 阶段 3: 内存管理 ⚠️ (进行中)

**目标**: 实现主机-设备内存传输

**当前状态**:
- ⚠️ 内存分配（部分实现，有 TODO）
- ⚠️ 内存释放（部分实现）
- ⚠️ 内存复制（部分实现）
- ❌ 统一内存（Unified Memory）

**待完成**:
- [ ] 完成 CUDA 内存操作
- [ ] 完成 ROCm 内存操作
- [ ] 实现内存池管理
- [ ] 添加内存使用统计

**预计时间**: 2-3 周

---

### 阶段 4: JIT 编译 ⚠️ (进行中)

**目标**: 将 IR 代码编译为 GPU/NPU 可执行代码

**当前状态**:
- ✅ IR 到 PTX/AMDGPU 代码生成框架
- ⚠️ CUDA PTX 生成（部分实现）
- ⚠️ ROCm AMDGPU 代码生成（部分实现）
- ❌ ARM NPU 代码生成
- ❌ 代码优化

**待完成**:
- [ ] 完成 PTX 代码生成
- [ ] 完成 AMDGPU 代码生成
- [ ] 实现代码缓存
- [ ] 添加编译优化选项

**预计时间**: 3-4 周

---

### 阶段 5: 内核执行 ❌ (未开始)

**目标**: 在 GPU/NPU 上执行计算内核

**当前状态**:
- ❌ CUDA 内核启动
- ❌ ROCm 内核启动
- ❌ ARM NPU 推理执行
- ❌ 执行结果收集

**待完成**:
- [ ] 实现 CUDA 内核启动
- [ ] 实现 ROCm 内核启动
- [ ] 实现 ARM NPU 推理
- [ ] 添加执行监控和错误处理

**预计时间**: 4-5 周

---

### 阶段 6: 优化和集成 ❌ (未开始)

**目标**: 性能优化和系统集成

**待完成**:
- [ ] 性能基准测试
- [ ] 内存使用优化
- [ ] 编译缓存优化
- [ ] 与 vm-service 集成
- [ ] 文档和示例

**预计时间**: 3-4 周

---

## Feature 标志

### 基础 Features

```toml
# CUDA GPU 支持
cuda = ["cudarc"]

# ROCm GPU 支持
rocm = []

# ARM NPU 支持（实验性）
npu = []
```

### 组合 Features

```toml
# 所有 GPU 支持
gpu = ["cuda", "rocm"]

# 所有加速器支持
all-accelerators = ["cuda", "rocm", "npu"]
```

### 使用示例

#### 仅启用 CUDA
```toml
[dependencies]
vm-passthrough = { path = "../vm-passthrough", features = ["cuda"] }
```

#### 启用所有 GPU
```toml
[dependencies]
vm-passthrough = { path = "../vm-passthrough", features = ["gpu"] }
```

#### 启用所有加速器
```toml
[dependencies]
vm-passthrough = { path = "../vm-passthrough", features = ["all-accelerators"] }
```

---

## 使用指南

### 基本使用

#### 1. 扫描设备

```rust
use vm_passthrough::PassthroughManager;

let mut manager = PassthroughManager::new();
manager.scan_devices()?;
manager.print_devices();
```

#### 2. 筛选 GPU 设备

```rust
use vm_passthrough::{DeviceType, PassthroughManager};

let manager = PassthroughManager::new();
let gpus = manager.filter_by_type(DeviceType::GpuNvidia);
for gpu in gpus {
    println!("Found GPU: {}", gpu.name);
}
```

#### 3. CUDA JIT 编译（需要 cuda feature）

```rust
#[cfg(feature = "cuda")]
use vm_passthrough::{CudaJITCompiler, CudaAccelerator};
use vm_ir::IRBlock;

#[cfg(feature = "cuda")]
{
    let accelerator = Arc::new(CudaAccelerator::new(0)?);
    let mut compiler = CudaJITCompiler::new(accelerator);

    let block = IRBlock::new(vm_ir::GuestAddr(0x1000));
    let kernel = compiler.compile(&block)?;

    // 执行内核（待实现）
}
```

### 高级使用

#### 设备直通

```rust
use vm_passthrough::{PassthroughManager, PciAddress};

let mut manager = PassthroughManager::new();
let address = PciAddress::from_str("0000:01:00.0")?;

// 创建直通设备（需要实现 PassthroughDevice trait）
// let device = Box::new(MyPassthroughDevice::new(address)?);
// manager.attach_device(address, device)?;
```

---

## 环境要求

### CUDA

**必需**:
- NVIDIA GPU（支持 CUDA）
- CUDA Toolkit 11.0 或更高版本
- NVIDIA 驱动程序

**安装**:
```bash
# Ubuntu/Debian
sudo apt-get install nvidia-cuda-toolkit

# 或从 NVIDIA 官网下载
# https://developer.nvidia.com/cuda-downloads
```

**验证**:
```bash
nvcc --version
nvidia-smi
```

### ROCm

**必需**:
- AMD GPU（支持 ROCm）
- ROCm 5.0 或更高版本

**安装**:
```bash
# Ubuntu
wget -qO - https://repo.radeon.com/rocm/rocm.gpg.key | sudo apt-key add -
echo 'deb [arch=amd64] https://repo.radeon.com/rocm/apt/5.7/ jammy main' | sudo tee /etc/apt/sources.list.d/rocm.list
sudo apt-get update
sudo apt-get install rocm-dev
```

**验证**:
```bash
rocminfo
```

### ARM NPU

**状态**: 实验性，需要厂商特定 SDK

**支持平台**:
- Qualcomm Hexagon DSP: 需要 Hexagon SDK
- HiSilicon Da Vinci NPU: 需要 HiAI SDK
- MediaTek APU: 需要 NeuroPilot SDK
- Apple Neural Engine: 需要 Core ML（仅 macOS）

---

## 已知限制

### 编译限制

1. **CUDA SDK 依赖**:
   - 编译时需要 CUDA SDK
   - 如果未安装，`cargo build --features cuda` 会失败
   - 建议使用 `cargo build --features cuda` 仅在需要时启用

2. **ROCm SDK 依赖**:
   - 编译时需要 ROCm SDK
   - 当前使用占位实现，实际 API 调用待实现

### 功能限制

1. **内核执行**: 当前未实现，仅提供编译框架
2. **内存管理**: 部分功能使用占位实现
3. **错误处理**: 错误处理需要完善
4. **性能**: 未进行性能优化

### 平台限制

1. **Windows**: PCIe 设备扫描未实现
2. **macOS**: 仅支持 Apple Neural Engine（通过 Core ML）
3. **Linux**: 主要支持平台

---

## 未来计划

### 短期（1-2 个月）

1. **完成阶段 2-3**: 设备管理和内存管理
2. **完善 JIT 编译**: 完成 PTX 和 AMDGPU 代码生成
3. **添加测试**: 单元测试和集成测试

### 中期（3-6 个月）

1. **实现内核执行**: 完成阶段 5
2. **性能优化**: 内存池、编译缓存
3. **文档完善**: API 文档、示例代码

### 长期（6-12 个月）

1. **系统集成**: 与 vm-service 深度集成
2. **高级功能**: 统一内存、多 GPU 支持
3. **生产就绪**: 错误处理、监控、日志

---

## 贡献指南

### 报告问题

如果遇到问题，请提供：
1. 使用的 feature 标志
2. 环境信息（OS, GPU, SDK 版本）
3. 错误消息和日志
4. 复现步骤

### 贡献代码

1. **遵循分阶段计划**: 优先完成当前阶段的任务
2. **添加测试**: 新功能必须包含测试
3. **更新文档**: 更新本文档和相关 API 文档
4. **标记 TODO**: 未完成的功能使用 TODO 标记

---

## 参考

- [CUDA 编程指南](https://docs.nvidia.com/cuda/cuda-c-programming-guide/)
- [ROCm 文档](https://rocm.docs.amd.com/)
- [Feature Contract 文档](./FEATURE_CONTRACT.md)
- [现代化升级报告](./MODERNIZATION_SUMMARY.md)

---

**文档维护者**: VM 项目团队
**最后审查**: 2024年现代化升级计划

