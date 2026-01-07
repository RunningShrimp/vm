# vm-device

Comprehensive device emulation framework providing unified device models, network stack integration, GPU acceleration, and device hotplug support with virtio device implementations.

## Overview

`vm-device` provides the device emulation layer for the Rust VM project, implementing a wide range of virtual devices including network interfaces, storage controllers, display devices, and platform devices with support for both virtio and legacy device models.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                vm-device (Device Emulation)              │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │  Virtio Net  │  │  Virtio Blk  │  │  Virtio GPU  │ │
│  │              │  │              │  │              │ │
│  │ • TX/RX      │  │ • Read/Write │  │ • 2D/3D      │ │
│  │ • offload    │  │ • Queues     │  │ • Render     │ │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘ │
│         │                  │                  │         │
│         └──────────────────┼──────────────────┘         │
│                            │                            │
│                  ┌─────────▼──────────┐                 │
│                  │  Unified Device   │                 │
│                  │     Manager       │                 │
│                  └─────────┬──────────┘                 │
│                            │                            │
│  ┌─────────────────────────┼─────────────────────────┐ │
│  │  ┌──────────────────────▼─────────────────────┐  │ │
│  │  │         Device Bus & Hotplug                │  │ │
│  │  │  • PCIe bus emulation                      │  │ │
│  │  │  • Device discovery                        │  │ │
│  │  │  • Hotplug add/remove                      │  │ │
│  │  │  • Resource allocation                     │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌────────────────────────────────────────────┐  │ │
│  │  │           Network Stack                     │  │ │
│  │  │  • smoltcp integration                     │  │ │
│  │  │  • TCP/UDP/IP                              │  │ │
│  │  │  • Virtual network topologies             │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌────────────────────────────────────────────┐  │ │
│  │  │           GPU & Display                     │  │ │
│  │  │  • wgpu-based rendering                    │  │ │
│  │  │  • Vulkan/DX12/Metal backend               │  │ │
│  │  │  • Display configuration                   │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌────────────────────────────────────────────┐  │ │
│  │  │            Storage                          │  │ │
│  │  │  • Block device emulation                  │  │ │
│  │  │  • File-backed storage                     │  │ │
│  │  │  • Snapshot support                        │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  │                                                   │ │
│  │  ┌────────────────────────────────────────────┐  │ │
│  │  │         Platform Devices                    │  │ │
│  │  │  • Serial/UART                             │  │ │
│  │  │  • RTC (Real-time clock)                   │  │ │
│  │  │  • Interrupt controller                    │  │ │
│  │  │  • Timer devices                           │  │ │
│  │  └────────────────────────────────────────────┘  │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

## Key Components

### 1. Device Manager (`src/manager.rs`)
Central device lifecycle management.

**Features**:
- Device registration and discovery
- Resource allocation (MMIO, IRQ, DMA)
- Device hotplug support
- Bus management

**Usage**:
```rust
use vm_device::manager::DeviceManager;

let mut manager = DeviceManager::new()?;

// Register device
manager.register_device(network_device)?;

// Hotplug add
manager.add_device(virtio_net)?;

// Hotplug remove
manager.remove_device(device_id)?;
```

### 2. Virtio Devices

#### Virtio Network (`src/virtio/net.rs`)
- Feature negotiation
- Multiple queue pairs
- Offload support (TSO, checksum)
- Bridge and tap interfaces

#### Virtio Block (`src/virtio/block.rs`)
- Read/write operations
- Multiple queues
- Feature negotiation (write-back, write-through)
- Snapshot support

#### Virtio GPU (`src/virtio/gpu.rs`)
- 2D rendering (blit, resource creation)
- 3D rendering (Vulkan passthrough)
- Display configuration
- Cursor support

### 3. Network Stack (`src/net.rs`)
Integrated TCP/IP stack using smoltcp.

**Features**:
- TCP/UDP protocols
- IPv4/IPv6 support
- Virtual network topologies
- Bridge and router modes

**Usage**:
```rust
use vm_device::net::{VirtualNetwork, NetworkConfig};

let config = NetworkConfig::bridge("br0");
let network = VirtualNetwork::new(config)?;

// Send packet
network.send_packet(data)?;

// Receive packet
let packet = network.recv_packet()?;
```

### 4. GPU & Display (`src/gpu.rs`)
Hardware-accelerated graphics rendering.

**Features**:
- wgpu-based rendering
- Multiple backends (Vulkan, DX12, Metal)
- Display configuration
- Framebuffer management

**Usage**:
```rust
use vm_device::gpu::{GpuDevice, DisplayConfig};

let config = DisplayConfig {
    width: 1920,
    height: 1080,
    format: PixelFormat::XRGB8888,
};

let gpu = GpuDevice::new(config)?;

// Render frame
gpu.render_frame(buffer)?;
```

### 5. Storage (`src/storage.rs`)
Block device emulation.

**Features**:
- File-backed storage
- Block device emulation
- Multiple formats (raw, qcow2)
- Async I/O

**Usage**:
```rust
use vm_device::storage::{BlockDevice, StorageBackend};

let backend = StorageBackend::file("disk.img")?;
let device = BlockDevice::new(backend)?;

// Read block
let data = device.read_block(block_number)?;

// Write block
device.write_block(block_number, data)?;
```

### 6. Platform Devices (`src/platform/`)

#### Serial/UART
- Character device I/O
- Console support
- Configurable baud rates

#### RTC (Real-Time Clock)
- Timekeeping
- Alarm support
- Timezone handling

#### Interrupt Controller
- IRQ routing
- Priority levels
- Edge/level triggering

#### Timer Devices
- Periodic timers
- One-shot timers
- Watchdog timers

### 7. Device Hotplug (`src/hotplug/`)
Runtime device addition and removal.

**Features**:
- PCIe hotplug
- Virtio device hotplug
- Resource management
- Safe removal

**Usage**:
```rust
use vm_device::hotplug::HotplugController;

let controller = HotplugController::new()?;

// Add device
controller.add_device(virtio_net)?;

// Remove device
controller.remove_device(device_id)?;
```

## Features

### Default Features
- **`std`**: Standard library support

### Optional Features
- **`smoltcp`**: Network stack support (TCP/IP)
- **`smmu`**: SMMU/IOMMU support for device DMA

## Usage

### Creating Network Device

```rust
use vm_device::virtio::net::VirtioNet;
use vm_device::net::NetworkConfig;

let config = NetworkConfig::new();
let virtio_net = VirtioNet::new(config)?;

// Connect to backend
virtio_net.connect()?;

// Start device
virtio_net.start()?;
```

### Creating Block Device

```rust
use vm_device::virtio::block::VirtioBlock;
use vm_device::storage::StorageBackend;

let backend = StorageBackend::file("disk.img")?;
let virtio_blk = VirtioBlock::new(backend)?;

// Start device
virtio_blk.start()?;
```

### Creating GPU Device

```rust
use vm_device::virtio::gpu::VirtioGpu;
use vm_device::gpu::DisplayConfig;

let config = DisplayConfig {
    width: 1920,
    height: 1080,
    format: PixelFormat::XRGB8888,
};

let virtio_gpu = VirtioGpu::new(config)?;

// Start GPU
virtio_gpu.start()?;
```

### Platform Device

```rust
use vm_device::platform::serial::SerialDevice;
use std::fs::File;

let serial = SerialDevice::new()?;

// Connect to file
serial.connect_output(File::create("console.log")?)?;

// Write data
serial.write(b"Hello, VM!\n")?;
```

## Device Models

### Virtio Transport

```
┌─────────────────────────────────────┐
│        Guest Driver                  │
└────────────┬────────────────────────┘
             │ Virtio Queues
┌────────────▼────────────────────────┐
│     Virtio Device Frontend           │
│  • Feature negotiation               │
│  • Queue management                  │
│  • Notification handling             │
└────────────┬────────────────────────┘
             │ Device-specific operations
┌────────────▼────────────────────────┐
│     Device Backend                  │
│  • Network: TX/RX processing        │
│  • Block: Read/write operations     │
│  • GPU: Rendering commands          │
└─────────────────────────────────────┘
```

### Device Hotplug Flow

```
1. Hotplug Request
   ↓
2. Device Manager validates request
   ↓
3. Allocate resources (MMIO, IRQ)
   ↓
4. Initialize device
   ↓
5. Notify guest via interrupt
   ↓
6. Guest enumerates and configures device
   ↓
7. Device operational
```

## Performance Optimization

### Network Performance

**Features**:
- Zero-copy I/O
- Multiple queue pairs
- Interrupt coalescing
- Offload support

**Tuning**:
```rust
let config = NetworkConfig {
    num_queues: 4,              // Multiple queues
    queue_size: 1024,            // Larger queues
    interrupt_coalescing: true,  // Reduce interrupts
    tso: true,                   // TCP segmentation offload
};
```

### Storage Performance

**Features**:
- Async I/O
- Multi-queue support
- Write-back caching
- Direct I/O

**Tuning**:
```rust
let config = BlockConfig {
    num_queues: 2,
    queue_size: 256,
    write_cache: CacheMode::WriteBack,
    use_direct_io: true,
};
```

### GPU Performance

**Features**:
- Hardware acceleration (wgpu)
- Async rendering
- Texture compression
- Buffer caching

**Tuning**:
```rust
let config = GpuConfig {
    backend: GpuBackend::Vulkan,  // Fastest on most systems
    async_mode: true,             // Enable async
    texture_compression: true,    // Reduce memory
};
```

## Configuration

### Device Manager

```rust
use vm_device::manager::DeviceManagerConfig;

let config = DeviceManagerConfig {
    max_devices: 32,
    enable_hotplug: true,
    enable_smmu: true,
};

let manager = DeviceManager::with_config(config)?;
```

### Network

```rust
use vm_device::net::NetworkConfig;

let config = NetworkConfig {
    mode: NetworkMode::Bridge("br0".to_string()),
    num_queues: 2,
    queue_size: 256,
    mtu: 1500,
};
```

### Storage

```rust
use vm_device::storage::BlockConfig;

let config = BlockConfig {
    backend: StorageBackend::file("disk.img"),
    num_queues: 1,
    queue_size: 128,
    readonly: false,
    cache_mode: CacheMode::WriteBack,
};
```

## Architecture Diagram

```
┌──────────────────────────────────────────────────────────┐
│                       vm-device                          │
├──────────────────────────────────────────────────────────┤
│                                                            │
│  ┌──────────────────────────────────────────────────┐   │
│  │                Device Manager                     │   │
│  │  • Device registration                           │   │
│  │  • Resource allocation                           │   │
│  │  • Hotplug management                            │   │
│  └──────────────────────────────────────────────────┘   │
│                                                            │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐           │
│  │  Virtio   │  │  Virtio   │  │  Virtio   │           │
│  │   Net     │  │   Block   │  │   GPU     │           │
│  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘           │
│        │              │              │                   │
│  ┌─────▼──────────────▼──────────────▼────┐            │
│  │       Virtio Transport Layer           │            │
│  │  • Queue management                    │            │
│  │  • Feature negotiation                 │            │
│  │  • Notification handling               │            │
│  └────────────────────┬───────────────────┘            │
│                       │                                 │
│  ┌────────────────────▼───────────────────┐            │
│  │           Device Backends              │            │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐│            │
│  │  │ Network │  │ Storage │  │   GPU   ││            │
│  │  │ Stack   │  │ Backend │  │Renderer ││            │
│  │  └─────────┘  └─────────┘  └─────────┘│            │
│  └────────────────────────────────────────┘            │
│                                                            │
│  ┌──────────────────────────────────────────────────┐   │
│  │            Platform Devices                       │   │
│  │  • Serial, RTC, Interrupt Controller, Timer       │   │
│  └──────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────┘
```

## Best Practices

1. **Use virtio devices**: Better performance and features
2. **Enable multiple queues**: For parallel I/O
3. **Use interrupt coalescing**: Reduce overhead
4. **Enable SMMU**: For secure DMA
5. **Test hotplug**: Ensure safe device removal

## Testing

```bash
# Run all tests
cargo test -p vm-device

# Run virtio tests
cargo test -p vm-device --lib virtio

# Run network tests
cargo test -p vm-device --lib net --features smoltcp

# Run GPU tests
cargo test -p vm-device --lib gpu
```

## Related Crates

- **vm-core**: Domain models and error handling
- **vm-mem**: Memory management (for DMA)
- **vm-accel**: Hardware acceleration (for interrupt delivery)
- **vm-osal**: OS abstraction (for platform-specific code)
- **vm-passthrough**: Device passthrough (for direct device assignment)

## Device Support Matrix

| Device Type | Status | Virtio | Legacy | Features |
|-------------|--------|--------|--------|----------|
| Network | ✅ | ✅ | ❌ | Multi-queue, offload |
| Block | ✅ | ✅ | ❌ | Multi-queue, snapshots |
| GPU | ✅ | ✅ | ❌ | 2D/3D, Vulkan |
| Serial | ✅ | ✅ | ✅ | Console |
| RTC | ✅ | ❌ | ✅ | Alarms |
| Input | ✅ | ✅ | ❌ | Keyboard/mouse |
| Balloon | ✅ | ✅ | ❌ | Memory stats |

## License

[Your License Here]

## Contributing

Contributions welcome! Please:
- Follow virtio specifications
- Test with guest drivers
- Handle hotplug safely
- Document device quirks

## See Also

- [Virtio Specification](https://docs.oasis-open.org/virtio/virtio/v1.2/cs01/virtio-v1.2-cs01.html)
- [wgpu Documentation](https://docs.rs/wgpu/)
- [smoltcp Documentation](https://docs.rs/smoltcp/)
