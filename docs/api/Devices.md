# Devices API 参考

## 概述

`vm-device` 提供虚拟机设备的完整实现，包括 VirtIO 设备、中断控制器、GPU虚拟化和硬件检测。该模块是VM设备子系统的核心。

## 主要功能

- **VirtIO设备**: 块设备、网络设备、控制台等
- **中断控制器**: CLINT、PLIC
- **GPU虚拟化**: VirGL、GPU直通、介质设备
- **零拷贝I/O**: 高性能DMA和缓冲池
- **异步I/O**: 异步块设备支持

## 模块组织

### VirtIO设备

#### VirtioBlock (块设备)

虚拟块设备，实现VirtIO块设备规范。

**主要功能**:
- 读写块设备
- 支持多种后端（文件、RAM disk等）
- 异步I/O支持

**类型**: `vm_device::block::VirtioBlock`

**示例**:
```rust
use vm_device::block::VirtioBlock;

// 创建基于文件的块设备
let block_device = VirtioBlock::new(
    "/path/to/disk.img",
    512,  // 扇区大小
    true, // 只读
)?;

// 创建RAM disk
let ramdisk = VirtioBlock::new_ramdisk(1024 * 1024 * 1024); // 1GB
```

#### VirtioNet (网络设备)

虚拟网络设备，实现VirtIO网络设备规范。

**主要功能**:
- 数据包收发
- MAC地址管理
- 多队列支持

**类型**: `vm_device::net::VirtioNet`

**示例**:
```rust
use vm_device::net::VirtioNet;

let net_device = VirtioNet::new(
    "52:54:00:12:34:56", // MAC地址
    2,                    // 队列数量
    1500,                 // MTU
)?;
```

#### VirtioConsole (控制台设备)

虚拟控制台，用于字符I/O。

**类型**: `vm_device::virtio_console::VirtioConsole`

**示例**:
```rust
use vm_device::virtio_console::VirtioConsole;

let console = VirtioConsole::new();
```

#### 其他VirtIO设备

- `VirtioRng` - 随机数生成器
- `VirtioBalloon` - 内存气球
- `VirtioScsi` - SCSI设备
- `Virtio9p` - 9P文件系统
- `VirtioCrypto` - 加密设备
- `VirtioInput` - 输入设备
- `VirtioSound` - 音频设备
- `VirtioWatchdog` - 看门狗
- `VirtioAI` - AI加速器

### 中断控制器

#### CLINT (Core Local Interruptor)

RISC-V核心本地中断控制器。

**主要功能**:
- 软件中断
- 定时器中断
- 每核心独立

**类型**: `vm_device::clint::CLINT`

**示例**:
```rust
use vm_device::clint::CLINT;

let clint = CLINT::new(
    num_cpus,       // CPU数量
    timebase_freq,  // 时间基准频率
)?;
```

#### PLIC (Platform Level Interrupt Controller)

RISC-V平台级中断控制器。

**主要功能**:
- 外部中断路由
- 优先级管理
- 中断掩码

**类型**: `vm_device::plic::PLIC`

**示例**:
```rust
use vm_device::plic::PLIC;

let plic = PLIC::new(
    num_sources,  // 中断源数量
    num_contexts, // 上下文数量
)?;
```

### GPU虚拟化

#### VirtioGPU

VirtIO GPU设备，用于2D/3D图形加速。

**类型**: `vm_device::gpu_virt::VirtioGpu`

#### VirGL

VirGL 3D渲染后端。

**类型**: `vm_device::virgl::VirGLRenderer`

#### GPU Passthrough

GPU设备直通支持。

**类型**: `vm_device::gpu_passthrough::GpuPassthrough`

#### GPU Mediated Device

GPU介质设备，共享物理GPU。

**类型**: `vm_device::gpu_mdev::GpuMdev`

### 零拷贝I/O

#### DirectMemoryAccess

DMA引擎，支持零拷贝I/O。

**类型**: `vm_device::dma::DirectMemoryAccess`

**示例**:
```rust
use vm_device::dma::DirectMemoryAccess;

let dma = DirectMemoryAccess::new(
    guest_mem_base,
    guest_mem_size,
)?;

// 执行DMA传输
dma.copy_to_guest(
    host_addr,
    guest_addr,
    size,
)?;
```

#### BufferPool

缓冲池，管理I/O缓冲区。

**类型**: `vm_device::virtio_zerocopy::BufferPool`

#### ZeroCopyIoManager

零拷贝I/O管理器。

**类型**: `vm_device::zerocopy::ZeroCopyIoManager`

### 异步I/O

#### AsyncBlockDevice

异步块设备实现。

**类型**: `vm_device::block_async::AsyncBlockDevice`

**示例**:
```rust
use vm_device::block_async::AsyncBlockDevice;

let device = AsyncBlockDevice::new("/path/to/disk.img").await?;

// 异步读取
let mut buffer = vec![0u8; 4096];
device.read(0, &mut buffer).await?;

// 异步写入
device.write(0, &buffer).await?;
```

## 使用示例

### 创建块设备

```rust
use vm_device::block::VirtioBlock;
use vm_interface::{Device, DeviceType, DeviceId};

// 创建基于文件的块设备
let mut block = VirtioBlock::new(
    "/path/to/disk.img",
    512,  // 扇区大小
    false, // 读写
)?;

// 设备操作
let device_id = block.device_id();
let device_type = block.device_type();

// 读取扇区
let mut buffer = vec![0u8; 512];
block.handle_read(0, &mut buffer)?;

// 写入扇区
block.handle_write(0, &buffer)?;
```

### 创建网络设备

```rust
use vm_device::net::VirtioNet;

let mut net = VirtioNet::new(
    "52:54:00:12:34:56",
    2,  // 队列数量
    1500,
)?;

// 发送数据包
let packet = vec![0u8; 64];
net.send_packet(&packet)?;

// 接收数据包
let received = net.receive_packet()?;
```

### 设置中断控制器

```rust
use vm_device::{clint::CLINT, plic::PLIC};

// 创建CLINT
let mut clint = CLINT::new(4, 10_000_000)?;

// 设置定时器
clint.set_timer(0, 1000000)?;

// 创建PLIC
let mut plic = PLIC::new(32, 4)?;

// 配置中断优先级
plic.set_priority(1, 7)?;

// 使能中断
plic.enable_interrupt(0, 1)?;
```

### 零拷贝I/O

```rust
use vm_device::{zerocopy::ZeroCopyIoManager, dma::DirectMemoryAccess};

let dma = DirectMemoryAccess::new(0x8000_0000, 1024 * 1024)?;
let mut zc_manager = ZeroCopyIoManager::new(dma)?;

// 零拷贝传输
zc_manager.transfer_to_guest(
    host_buffer,
    guest_addr,
    size,
)?;
```

### 异步块设备

```rust
use vm_device::block_async::AsyncBlockDevice;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let device = AsyncBlockDevice::new("/path/to/disk.img").await?;

    // 异步读取
    let mut buffer = vec![0u8; 4096];
    device.read(0, &mut buffer).await?;

    // 异步写入
    device.write(4096, &buffer).await?;

    Ok(())
}
```

## 设备特性

### VirtIO特性

每个VirtIO设备支持不同的特性位：

**VirtioBlock**:
- `VIRTIO_BLK_F_RO` - 只读
- `VIRTIO_BLK_F_BLK_SIZE` - 块大小协商
- `VIRTIO_BLK_F_FLUSH` - FLUSH命令
- `VIRTIO_BLK_F_DISCARD` - 丢弃命令
- `VIRTIO_BLK_F_WRITE_ZEROES` - 写零命令

**VirtioNet**:
- `VIRTIO_NET_F_CSUM` - 校验和卸载
- `VIRTIO_NET_F_GSO` - 分段卸载
- `VIRTIO_NET_F_MRG_RXBUF` - 合并接收缓冲区
- `VIRTIO_NET_F_MTU` - MTU协商

### 设备状态

所有设备都实现了统一的设备状态：

```rust
pub enum DeviceStatus {
    Uninitialized,
    Initialized,
    Running,
    Stopped,
    Error(String),
}
```

## 设备管理

### 设备注册

```rust
use vm_interface::{DeviceManager, Device};
use vm_device::block::VirtioBlock;

struct MyDeviceManager {
    devices: Vec<Box<dyn Device>>,
}

impl DeviceManager for MyDeviceManager {
    type Device = dyn Device;

    fn register_device(&mut self, device: Box<Self::Device>) -> Result<DeviceId, VmError> {
        let id = self.devices.len() as DeviceId;
        self.devices.push(device);
        Ok(id)
    }

    // ... 其他方法
}

// 使用
let mut manager = MyDeviceManager { devices: Vec::new() };
let block = Box::new(VirtioBlock::new("disk.img", 512, false)?);
let id = manager.register_device(block)?;
```

### 设备路由

```rust
// 路由I/O到设备
let data = manager.route_io_read(id, offset, size)?;
manager.route_io_write(id, offset, value, size)?;

// 处理中断
manager.find_device_mut(id)?.handle_interrupt(vector)?;
```

## 性能优化

### 零拷贝I/O

使用零拷贝技术减少内存复制：

```rust
use vm_device::zerocopy::ScatterGatherList;

let mut sg_list = ScatterGatherList::new();

// 添加散列/聚集条目
sg_list.add(addr1, size1)?;
sg_list.add(addr2, size2)?;

// 执行零拷贝传输
zc_manager.transfer_sglist(&sg_list)?;
```

### 多队列

使用多队列提高并行性能：

```rust
use vm_device::net::VirtioNet;

let net = VirtioNet::new_with_queues(
    "52:54:00:12:34:56",
    4,  // 4个队列
    1500,
)?;
```

### 异步I/O

使用异步I/O提高吞吐量：

```rust
// 并发多个I/O操作
let (read1, read2) = tokio::join!(
    device.read(0, &mut buf1),
    device.read(4096, &mut buf2)
);
```

## 错误处理

所有设备操作返回`Result<T, VmError>`：

```rust
use vm_core::VmError;

match block.handle_read(0, size) {
    Ok(data) => println!("Read: {:x}", data),
    Err(VmError::Device(vm_core::DeviceError::IoError(e))) => {
        eprintln!("I/O error: {}", e);
    }
    Err(e) => eprintln!("Error: {:?}", e),
}
```

常见错误：
- `DeviceError::IoError` - I/O操作失败
- `DeviceError::InvalidParameter` - 无效参数
- `DeviceError::NotSupported` - 不支持的操作

## 注意事项

### 线程安全

- 大多数设备使用内部锁保证线程安全
- 多线程访问时需要注意顺序
- 某些设备可能需要外部同步

### 资源管理

- 设备持有的资源（文件句柄、内存等）会被自动清理
- 使用异步I/O时确保运行时正确关闭

### 性能考虑

- 零拷贝I/O可以显著提高性能
- 多队列可以提高并行度
- 异步I/O适合高吞吐场景

### VirtIO协议

确保VirtIO驱动和设备实现遵循规范：
- 版本协商
- 特性协商
- 队列配置
- 通知机制

## 相关API

- [VmCore API](./VmCore.md) - MmioDevice trait
- [VmInterface API](./VmInterface.md) - Device接口规范
- [VmMemory API](./VmMemory.md) - MMIO支持
