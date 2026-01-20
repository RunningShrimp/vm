# Devices

设备模拟框架和虚拟设备实现。

## 包含模块

- **vm-device** - 设备模拟框架
  - Virtio 设备框架
  - 设备热插拔
  - 设备总线管理
  - 中断处理

- **vm-graphics** - 图形设备
  - Virtio GPU
  - 显示管理
  - 图形渲染

- **vm-smmu** - IOMMU/SMMU 支持
  - DMA 地址转换
  - 设备隔离
  - 中断重映射

- **vm-soc** - 片上系统设备
  - UART/串口
  - RTC (实时时钟)
  - GPIO
  - 定时器

## Virtio 设备支持

### 完整支持
- **网络** (virtio-net) - 多队列、TSO、offload
- **块设备** (virtio-blk) - 多队列、快照
- **GPU** (virtio-gpu) - 2D/3D 渲染
- **控制台** (virtio-console)
- **气球** (virtio-balloon)

### 平台设备
- **串口** - UART 16550
- **RTC** - CMOS 实时时钟
- **中断控制器** - APIC, GIC
- **定时器** - PIT, HPET

## 快速开始

查看各模块的 README 了解详细文档：
- [vm-device/README.md](./vm-device/README.md)
- [vm-graphics/README.md](./vm-graphics/README.md)
- [vm-smmu/README.md](./vm-smmu/README.md)
- [vm-soc/README.md](./vm-soc/README.md)
