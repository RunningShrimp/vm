# vm-smmu

**VM项目SMMU/IOMMU支持**

[![Rust](https://img.shields.io/badge/rust-2024%20Edition-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

---

## 概述

`vm-smmu` 是VM项目的系统内存管理单元（SMMU）/ IOMMU实现，提供设备DMA地址重映射和内存隔离功能。它实现了ARM SMMUv3架构，支持设备虚拟化和直接设备访问。

## 🎯 核心功能

- **地址空间隔离**: 设备DMA地址重映射和隔离
- **IOMMU支持**: ARM SMMUv3和Intel VT-d架构
- **设备分配**: 安全的设备直通和分配
- **中断重映射**: MSI/MSI-X中断重映射
- **页表管理**: 多级页表和TLB管理

## 📦 主要组件

### 1. SMMU设备

```rust
use vm_smmu::{SmmuDevice, SmmuConfig};

let config = SmmuConfig {
    base_addr: 0x2b400000,
    num_context_banks: 1,
    num_streams: 32,
};

let smmu = SmmuDevice::new(config)?;

// 初始化SMMU
smmu.initialize()?;

// 配置设备流
smmu.configure_stream(device_id, stream_id)?;
```

### 2. 地址映射

```rust
// 映射设备DMA地址
smmu.map_dma(
    device_id,
    guest_addr,    // 客户机物理地址
    host_addr,     // 主机物理地址
    size,          // 映射大小
)?;

// 解除映射
smmu.unmap_dma(device_id, guest_addr, size)?;
```

### 3. 中断重映射

```rust
// 配置MSI中断重映射
smmu.map_msi(
    device_id,
    msi_data,
    msi_address,
    vector_id,
)?;
```

## 🔧 依赖关系

```toml
[dependencies]
vm-core = { path = "../vm-core" }
vm-mem = { path = "../vm-mem" }
```

## 🚀 使用场景

### 场景1: 设备直通

```rust
use vm_smmu::SmmuDevice;

let smmu = SmmuDevice::new(config)?;

// 为直通设备配置地址空间
smmu.map_dma(device_id, 0x1000, host_addr, 0x1000)?;
```

## 📝 API概览

```rust
pub struct SmmuDevice {
    // SMMU设备实现
}

impl SmmuDevice {
    pub fn new(config: SmmuConfig) -> Result<Self, Error>;
    pub fn initialize(&mut self) -> Result<(), Error>;
    pub fn map_dma(&mut self, device_id: u32, guest_addr: u64, host_addr: u64, size: u64) -> Result<(), Error>;
    pub fn unmap_dma(&mut self, device_id: u32, guest_addr: u64, size: u64) -> Result<(), Error>;
}
```

## 📚 相关文档

- [vm-core](../vm-core/README.md) - 核心VM功能
- [vm-passthrough](../vm-passthrough/README.md) - 设备直通
- [MASTER_DOCUMENTATION_INDEX](../MASTER_DOCUMENTATION_INDEX.md)

## 📝 许可证

MIT License - 详见 [LICENSE](../LICENSE)

---

**包版本**: workspace v0.1.0
**最后更新**: 2026-01-07
