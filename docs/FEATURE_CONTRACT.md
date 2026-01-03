# Feature Contract 文档

本文档定义了 VM 项目中所有 feature 标志的契约、用途、依赖关系和最佳实践。

**最后更新**: 2024年（基于现代化升级计划）

## 目录

1. [Feature 分类](#feature-分类)
2. [核心 Feature](#核心-feature)
3. [架构 Feature](#架构-feature)
4. [性能 Feature](#性能-feature)
5. [设备 Feature](#设备-feature)
6. [硬件加速 Feature](#硬件加速-feature)
7. [Feature 依赖关系](#feature-依赖关系)
8. [最佳实践](#最佳实践)
9. [迁移指南](#迁移指南)

---

## Feature 分类

### 分类原则

- **核心 Feature**: 基础功能，通常包含在 `default` 中
- **架构 Feature**: 架构特定支持（x86_64, ARM64, RISC-V64）
- **性能 Feature**: 性能优化相关（JIT, async, 优化器）
- **设备 Feature**: 设备虚拟化支持
- **硬件加速 Feature**: 硬件虚拟化加速（KVM, HVF, WHPX, SMMU）
- **实验性 Feature**: 实验性功能（GPU/NPU 直通）

---

## 核心 Feature

### `std` (所有核心 crate)

**定义位置**: `vm-core`, `vm-mem`, `vm-device`, `vm-service`

**用途**: 启用标准库支持，提供基础功能

**依赖**: 无

**包含内容**:
- 标准库类型和 trait
- 基础序列化支持（serde_json）
- 日志支持

**使用示例**:
```toml
[dependencies]
vm-core = { path = "../vm-core", features = ["std"] }
```

**状态**: ✅ 稳定，推荐使用

---

### `default` (各 crate)

**定义位置**: 所有 crate

**用途**: 默认启用的 feature 集合

**各 crate 的 default**:
- `vm-core`: `["std"]`
- `vm-mem`: `["std", "optimizations"]`
- `vm-device`: `["std"]`
- `vm-service`: `["std", "devices", "performance"]`
- `vm-accel`: `["acceleration"]`
- `vm-smmu`: `["mmu", "atsu", "tlb", "interrupt"]`

**状态**: ✅ 稳定

---

## 架构 Feature

### `x86_64`, `arm64`, `riscv64` (vm-core)

**定义位置**: `vm-core/Cargo.toml`

**用途**: 架构特定的宏和类型支持

**依赖**: 无

**使用场景**: 主要用于编译时宏，运行时架构选择通过 `GuestArch` 枚举

**状态**: ✅ 稳定，主要用于宏系统

**注意**: 这些 feature 主要用于宏，实际架构选择在运行时通过 `VmConfig.guest_arch` 完成。

---

### `all` (vm-frontend)

**定义位置**: `vm-frontend/Cargo.toml`

**用途**: 启用所有架构的解码器支持

**依赖**: `["vm-mem", "vm-accel"]`

**包含内容**:
- x86_64 解码器
- ARM64 解码器
- RISC-V64 解码器
- 跨架构翻译支持

**使用示例**:
```toml
[dependencies]
vm-frontend = { path = "../vm-frontend", features = ["all"] }
```

**状态**: ✅ 稳定，推荐用于跨架构执行

---

## 性能 Feature

### `async` (vm-core, vm-mem, vm-engine)

**定义位置**: `vm-core/Cargo.toml`, `vm-mem/Cargo.toml`, `vm-engine/Cargo.toml`

**用途**: 启用异步执行支持

**依赖**:
- `vm-core/async`: `["tokio", "futures", "async-trait"]`
- `vm-mem/async`: `["tokio", "async-trait"]` (已合并到 `optimizations`)
- `vm-engine/async`: `["futures", "async-trait", "vm-core/async"]`

**包含内容**:
- 异步 MMU 操作
- 异步设备 I/O
- 异步执行引擎
- 异步事件总线

**使用示例**:
```toml
[dependencies]
vm-core = { path = "../vm-core", features = ["async"] }
vm-mem = { path = "../vm-mem", features = ["optimizations"] }  # async 已包含
```

**状态**: ✅ 稳定

**注意**: 在 `vm-mem` 中，`async` 已合并到 `optimizations` feature 中。

---

### `jit` (vm-engine, vm-service)

**定义位置**: `vm-engine/Cargo.toml`, `vm-service/Cargo.toml`

**用途**: 启用 JIT 编译支持

**依赖**:
- `vm-engine/jit`: 无直接依赖（启用 JIT 模块）
- `vm-service/jit`: `["performance"]` (已弃用，使用 `performance`)

**包含内容**:
- Cranelift JIT 后端
- JIT 编译器
- 热点检测
- 代码缓存

**使用示例**:
```toml
[dependencies]
vm-engine = { path = "../vm-engine", features = ["jit"] }
```

**状态**: ✅ 稳定

**注意**: 在 `vm-service` 中，`jit` 是 `performance` 的别名（已弃用）。

---

### `interpreter` (vm-engine)

**定义位置**: `vm-engine/Cargo.toml`

**用途**: 启用解释器执行引擎

**依赖**: 无

**包含内容**:
- IR 解释器
- 指令融合优化
- 块缓存

**状态**: ✅ 稳定

---

### `performance` (vm-service)

**定义位置**: `vm-service/Cargo.toml`

**用途**: 启用所有性能优化功能（推荐）

**依赖**: `["std", "vm-core/async", "vm-mem/async", "vm-engine/jit", "vm-frontend/all"]`

**包含内容**:
- JIT 编译
- 异步执行
- 前端解码器（所有架构）
- 性能优化

**使用示例**:
```toml
[dependencies]
vm-service = { path = "../vm-service", features = ["performance"] }
```

**状态**: ✅ 稳定，推荐使用

**注意**: 这是 `jit` 和 `async` 的统一入口，推荐使用此 feature 而不是单独启用。

---

### `optimizations` (vm-mem)

**定义位置**: `vm-mem/Cargo.toml`

**用途**: 启用内存管理优化

**依赖**: `["tokio", "async-trait"]`

**包含内容**:
- 异步 MMU
- TLB 优化
- NUMA 感知分配

**状态**: ✅ 稳定

**注意**: 已合并 `async` 和 `tlb` feature。

---

## 设备 Feature

### `devices` (vm-service)

**定义位置**: `vm-service/Cargo.toml`

**用途**: 启用设备虚拟化支持

**依赖**: `["vm-device"]`

**包含内容**:
- VirtIO 设备
- CLINT/PLIC 中断控制器
- 设备服务

**状态**: ✅ 稳定

---

### `smmu` (vm-device, vm-service, vm-accel)

**定义位置**: `vm-device/Cargo.toml`, `vm-service/Cargo.toml`, `vm-accel/Cargo.toml`

**用途**: 启用 SMMUv3 (IOMMU) 支持

**依赖**:
- `vm-device/smmu`: `["dep:vm-smmu", "vm-accel/smmu"]`
- `vm-service/smmu`: `["vm-accel/smmu", "devices", "vm-device/smmu", "vm-smmu"]`
- `vm-accel/smmu`: `["acceleration"]` (已弃用，使用 `acceleration`)

**包含内容**:
- SMMUv3 设备模拟
- DMA 虚拟化
- 地址翻译单元 (ATSU)
- TLB 管理

**状态**: ✅ 稳定

**注意**: 在 `vm-accel` 中，`smmu` 已合并到 `acceleration` feature 中。

---

### `smoltcp` (vm-device)

**定义位置**: `vm-device/Cargo.toml`

**用途**: 启用网络栈支持（使用 smoltcp）

**依赖**: `["dep:smoltcp"]`

**状态**: ✅ 稳定

---

## 硬件加速 Feature

### `accel` (vm-service)

**定义位置**: `vm-service/Cargo.toml`

**用途**: 启用硬件虚拟化加速

**依赖**: `["vm-accel"]`

**包含内容**:
- KVM (Linux)
- HVF (macOS)
- WHPX (Windows)
- VZ (macOS)

**状态**: ✅ 稳定

---

### `acceleration` (vm-accel)

**定义位置**: `vm-accel/Cargo.toml`

**用途**: 启用硬件加速功能（推荐）

**依赖**: `["raw-cpuid", "dep:kvm-ioctls", "dep:kvm-bindings", "dep:vm-smmu"]`

**包含内容**:
- CPU 特性检测
- KVM 支持
- SMMU 支持

**状态**: ✅ 稳定

**注意**: 已合并 `hardware` 和 `smmu` feature。

---

### `mmu`, `atsu`, `tlb`, `interrupt` (vm-smmu)

**定义位置**: `vm-smmu/Cargo.toml`

**用途**: SMMUv3 组件控制

**默认**: 全部启用

**状态**: ✅ 稳定

---

## 实验性 Feature

### `cuda` (vm-passthrough)

**定义位置**: `vm-passthrough/Cargo.toml`

**用途**: 启用 NVIDIA CUDA GPU 直通支持

**依赖**: `["cudarc"]`

**状态**: ⚠️ 实验性，需要 CUDA SDK

**注意**: 当前编译可能失败（需要 CUDA SDK 环境）。

---

### `rocm` (vm-passthrough)

**定义位置**: `vm-passthrough/Cargo.toml`

**用途**: 启用 AMD ROCm GPU 直通支持

**依赖**: 无

**状态**: ⚠️ 实验性，需要 ROCm SDK

---

## Feature 依赖关系

### 依赖图

```
vm-service (default)
├── std
├── devices
│   └── vm-device
└── performance
    ├── std
    ├── vm-core/async
    │   ├── tokio
    │   ├── futures
    │   └── async-trait
    ├── vm-mem/async (via optimizations)
    │   ├── tokio
    │   └── async-trait
    ├── vm-engine/jit
    └── vm-frontend/all
        ├── vm-mem
        └── vm-accel
            └── acceleration
                ├── raw-cpuid
                ├── kvm-ioctls
                ├── kvm-bindings
                └── vm-smmu
```

### 推荐组合

#### 最小配置（仅解释器）
```toml
vm-service = { path = "../vm-service", features = ["std", "devices"] }
vm-engine = { path = "../vm-engine", features = ["interpreter"] }
```

#### 标准配置（推荐）
```toml
vm-service = { path = "../vm-service", features = ["default"] }
# 等同于: ["std", "devices", "performance"]
```

#### 完整配置（所有功能）
```toml
vm-service = { path = "../vm-service", features = ["default", "accel", "smmu", "all-arch"] }
```

#### 跨架构执行
```toml
vm-service = { path = "../vm-service", features = ["performance", "all-arch"] }
vm-frontend = { path = "../vm-frontend", features = ["all"] }
```

---

## 最佳实践

### 1. Feature 命名规范

- **使用描述性名称**: `performance` 而不是 `perf`
- **避免缩写**: `acceleration` 而不是 `accel`（虽然 `accel` 作为别名存在）
- **使用动词形式**: `optimizations` 而不是 `optimized`

### 2. Feature 合并策略

**已合并的 feature**:
- `vm-mem`: `async` + `tlb` → `optimizations`
- `vm-accel`: `hardware` + `smmu` → `acceleration`
- `vm-service`: `jit` + `async` + `frontend` → `performance`

**原则**: 将相关功能合并到统一的 feature 中，减少配置复杂度。

### 3. 向后兼容

**已弃用的 feature**（仍可用，但推荐使用新名称）:
- `vm-service/jit` → 使用 `performance`
- `vm-service/async` → 使用 `performance`
- `vm-mem/async` → 使用 `optimizations`
- `vm-accel/hardware` → 使用 `acceleration`
- `vm-accel/smmu` → 使用 `acceleration`

### 4. 条件编译

**在代码中使用**:
```rust
#[cfg(feature = "async")]
pub async fn async_function() { }

#[cfg(not(feature = "async"))]
pub fn sync_function() { }
```

**原则**:
- 核心功能尽量不使用 `#[cfg]`，通过 trait 抽象
- 仅在集成层使用 `#[cfg]` 控制可选功能

### 5. 测试覆盖

**要求**: 所有 feature 组合必须在 CI 中测试

**测试矩阵**:
- `default`
- `--all-features`
- `--no-default-features`
- 关键组合（如 `performance`, `accel`）

---

## 迁移指南

### 从旧 feature 迁移

#### vm-service

**旧代码**:
```toml
vm-service = { path = "../vm-service", features = ["jit", "async"] }
```

**新代码**:
```toml
vm-service = { path = "../vm-service", features = ["performance"] }
```

#### vm-mem

**旧代码**:
```toml
vm-mem = { path = "../vm-mem", features = ["async", "tlb"] }
```

**新代码**:
```toml
vm-mem = { path = "../vm-mem", features = ["optimizations"] }
```

#### vm-accel

**旧代码**:
```toml
vm-accel = { path = "../vm-accel", features = ["hardware", "smmu"] }
```

**新代码**:
```toml
vm-accel = { path = "../vm-accel", features = ["acceleration"] }
```

---

## Feature 状态总结

| Feature | Crate | 状态 | 推荐 |
|---------|-------|------|------|
| `std` | 所有 | ✅ 稳定 | ✅ 推荐 |
| `default` | 所有 | ✅ 稳定 | ✅ 推荐 |
| `async` | vm-core, vm-mem, vm-engine | ✅ 稳定 | ✅ 推荐 |
| `jit` | vm-engine | ✅ 稳定 | ✅ 推荐 |
| `interpreter` | vm-engine | ✅ 稳定 | ✅ 推荐 |
| `performance` | vm-service | ✅ 稳定 | ✅ 推荐 |
| `optimizations` | vm-mem | ✅ 稳定 | ✅ 推荐 |
| `devices` | vm-service | ✅ 稳定 | ✅ 推荐 |
| `accel` | vm-service | ✅ 稳定 | ✅ 推荐 |
| `acceleration` | vm-accel | ✅ 稳定 | ✅ 推荐 |
| `smmu` | vm-device, vm-service | ✅ 稳定 | ✅ 推荐 |
| `all` | vm-frontend | ✅ 稳定 | ✅ 推荐 |
| `all-arch` | vm-service | ✅ 稳定 | ✅ 推荐 |
| `cuda` | vm-passthrough | ⚠️ 实验性 | ⚠️ 需要 CUDA SDK |
| `rocm` | vm-passthrough | ⚠️ 实验性 | ⚠️ 需要 ROCm SDK |
| `npu` | vm-passthrough | ⚠️ 实验性 | ⚠️ 需要厂商 SDK |
| `gpu` | vm-passthrough | ⚠️ 实验性 | ⚠️ 需要 GPU SDK |
| `all-accelerators` | vm-passthrough | ⚠️ 实验性 | ⚠️ 需要相应 SDK |

---

## 维护指南

### 添加新 Feature

1. **在 Cargo.toml 中定义**:
```toml
[features]
new-feature = ["dependency1", "dependency2"]
```

2. **更新本文档**: 添加 feature 描述、依赖、状态

3. **添加测试**: 确保 feature 组合可编译和测试

4. **更新 CI**: 在 CI 矩阵中添加新 feature 组合

### 废弃 Feature

1. **标记为已弃用**: 在文档中标记，保留代码支持
2. **提供迁移路径**: 在本文档的"迁移指南"中添加说明
3. **保留至少一个版本周期**: 给用户时间迁移

---

## 参考

- [Cargo Features 文档](https://doc.rust-lang.org/cargo/reference/features.html)
- [项目架构文档](./architecture.md)
- [现代化升级报告](./MODERNIZATION_SUMMARY.md)

---

**文档维护者**: VM 项目团队
**最后审查**: 2024年现代化升级计划
