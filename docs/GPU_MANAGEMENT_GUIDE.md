# 统一 GPU 管理系统使用指南

## 概述

本虚拟机监视器提供了统一的 GPU 管理系统，支持三种 GPU 模式，并能够根据系统环境自动选择最佳模式或按用户偏好降级：

1. **GPU Passthrough**（最高优先级）：直接将物理 GPU 分配给虚拟机
2. **Mediated Device Passthrough**（中等优先级）：使用 Intel GVT-g、NVIDIA vGPU 等技术
3. **WGPU 虚拟化**（最低优先级）：基于 WGPU 的软件 GPU 虚拟化

## GPU 模式对比

| 特性 | GPU Passthrough | Mediated Device | WGPU 虚拟化 |
|------|----------------|-----------------|------------|
| 性能 | 接近原生 | 良好 | 中等 |
| 隔离性 | 独占设备 | 共享设备 | 完全隔离 |
| 多虚拟机 | 不支持 | 支持 | 支持 |
| 硬件要求 | IOMMU + 独立 GPU | 支持 mdev 的 GPU | 无特殊要求 |
| 适用场景 | 高性能计算、游戏 | 桌面虚拟化、开发 | 测试、轻量级应用 |

## 前置条件

### GPU Passthrough

**硬件要求**：
- 支持 IOMMU 的 CPU（Intel VT-d 或 AMD-Vi）
- 独立的 GPU（不能是主显卡）
- 主板支持 IOMMU

**软件配置**：

1. 在 BIOS/UEFI 中启用 IOMMU：
   - Intel: 启用 VT-d
   - AMD: 启用 AMD-Vi

2. 添加内核启动参数：

   ```bash
   # Intel
   sudo nano /etc/default/grub
   # 添加: GRUB_CMDLINE_LINUX_DEFAULT="... intel_iommu=on iommu=pt"
   
   # AMD
   # 添加: GRUB_CMDLINE_LINUX_DEFAULT="... amd_iommu=on iommu=pt"
   
   sudo update-grub
   sudo reboot
   ```

3. 加载 VFIO 模块：

   ```bash
   sudo modprobe vfio-pci
   ```

4. 验证 IOMMU 是否启用：

   ```bash
   dmesg | grep -i iommu
   ls /sys/kernel/iommu_groups/
   ```

### Mediated Device Passthrough

**支持的 GPU**：
- Intel: 支持 GVT-g 的集成显卡（第 5 代 Core 及以上）
- NVIDIA: 支持 vGPU 的专业级 GPU（需要 vGPU 许可证）
- AMD: 支持 MxGPU 的专业级 GPU

**软件配置**：

1. 加载相应的内核模块：

   ```bash
   # Intel GVT-g
   sudo modprobe kvmgt
   
   # NVIDIA vGPU
   sudo modprobe nvidia-vgpu-vfio
   ```

2. 验证 mdev 是否可用：

   ```bash
   ls /sys/bus/pci/devices/*/mdev_supported_types/
   ```

### WGPU 虚拟化

无特殊要求，任何支持 Vulkan、Metal、DirectX 12 或 OpenGL 的系统都可以使用。

## 使用方法

### 1. 自动模式（推荐）

自动模式会按优先级扫描可用的 GPU 后端，并选择最佳的一个：

```rust
use vm_device::gpu_manager::UnifiedGpuManager;

// 创建 GPU 管理器
let mut gpu_manager = UnifiedGpuManager::new();

// 扫描可用的后端
gpu_manager.scan_backends().expect("Failed to scan GPU backends");

// 自动选择最佳后端
gpu_manager.auto_select().expect("Failed to select GPU backend");

// 初始化选中的后端
gpu_manager.initialize_selected().expect("Failed to initialize GPU");

// 打印选中的后端信息
if let Some(stats) = gpu_manager.get_stats() {
    println!("{}", stats);
}
```

### 2. 手动选择模式

用户可以手动指定要使用的 GPU 模式：

```rust
use vm_device::gpu_manager::{UnifiedGpuManager, GpuMode};

let mut gpu_manager = UnifiedGpuManager::new();
gpu_manager.scan_backends().expect("Failed to scan GPU backends");

// 打印可用的后端
gpu_manager.print_available_backends();

// 手动选择 GPU Passthrough
match gpu_manager.select_by_mode(GpuMode::Passthrough) {
    Ok(_) => println!("Selected GPU Passthrough"),
    Err(e) => println!("GPU Passthrough not available: {}", e),
}

// 或者通过索引选择
gpu_manager.select_by_index(0).expect("Failed to select backend");

// 初始化
gpu_manager.initialize_selected().expect("Failed to initialize GPU");
```

### 3. 偏好模式 + 自动降级

设置偏好模式，如果不可用则自动降级到下一个优先级：

```rust
use vm_device::gpu_manager::{UnifiedGpuManager, GpuMode};

let mut gpu_manager = UnifiedGpuManager::new();

// 设置偏好为 GPU Passthrough
gpu_manager.set_preferred_mode(GpuMode::Passthrough);

// 启用自动降级（默认已启用）
gpu_manager.set_auto_fallback(true);

// 扫描并自动选择
gpu_manager.scan_backends().expect("Failed to scan GPU backends");
gpu_manager.auto_select().expect("Failed to select GPU backend");

// 如果 Passthrough 不可用，会自动降级到 Mdev 或 WGPU
println!("Selected: {:?}", gpu_manager.get_selected_backend().unwrap().mode());
```

### 4. 命令行使用

通过命令行参数指定 GPU 模式：

```bash
# 自动选择（默认）
vm-cli --kernel kernel.bin

# 强制使用 GPU Passthrough
vm-cli --kernel kernel.bin --gpu-mode passthrough

# 强制使用 Mediated Device
vm-cli --kernel kernel.bin --gpu-mode mdev

# 强制使用 WGPU
vm-cli --kernel kernel.bin --gpu-mode wgpu

# 指定 GPU 设备（用于 Passthrough）
vm-cli --kernel kernel.bin --gpu-device 0000:01:00.0

# 指定 mdev 类型（用于 Mediated Device）
vm-cli --kernel kernel.bin --gpu-mode mdev --mdev-type i915-GVTg_V5_4
```

## 高级配置

### GPU Passthrough 配置

```rust
use vm_device::gpu_passthrough::{GpuPassthrough, scan_available_gpus};
use vm_passthrough::PciAddress;

// 扫描可用的 GPU
let gpus = scan_available_gpus();
for gpu in &gpus {
    println!("Found GPU: {:?} {} at {}", 
        gpu.vendor, gpu.model, gpu.pci_address.to_string());
    println!("  VRAM: {} MB", gpu.vram_size / 1024 / 1024);
    println!("  Driver: {}", gpu.driver);
}

// 选择特定的 GPU
if let Some(gpu_info) = gpus.first() {
    let address = gpu_info.pci_address;
    // 创建 PciDeviceInfo（需要从实际设备读取）
    // let info = ...;
    // let mut gpu_pt = GpuPassthrough::new(address, info).expect("Failed to create GPU passthrough");
    // gpu_pt.prepare().expect("Failed to prepare GPU passthrough");
}
```

### Mediated Device 配置

```rust
use vm_device::gpu_mdev::{GpuMdev, MdevType, scan_mdev_capable_gpus};

// 扫描支持 mdev 的 GPU
let mdev_gpus = scan_mdev_capable_gpus();
for (address, configs) in &mdev_gpus {
    println!("GPU at {}:", address.to_string());
    for config in configs {
        println!("  - {} ({})", config.name, config.type_id);
        println!("    Available instances: {}", config.available_instances);
    }
}

// 创建 mdev 设备
if let Some((address, configs)) = mdev_gpus.first() {
    if let Some(config) = configs.first() {
        let mut mdev = GpuMdev::new(*address, config.mdev_type, config.type_id.clone());
        mdev.create().expect("Failed to create mdev device");
        println!("Created mdev device: {}", mdev.get_uuid().unwrap());
    }
}
```

### WGPU 配置

```rust
use vm_device::gpu_virt::{WgpuBackend, GpuBackend};

let mut wgpu = WgpuBackend::new();

// 初始化
wgpu.init().expect("Failed to initialize WGPU");

// 获取适配器信息
if let Some(info) = wgpu.adapter_info() {
    println!("WGPU Adapter: {} ({:?})", info.name, info.backend);
}

// 获取统计信息
let stats = wgpu.get_stats();
println!("GPU Stats: {:?}", stats);

// 重置统计信息
wgpu.reset_stats();
```

## 故障排查

### GPU Passthrough 问题

**问题：IOMMU 未启用**

```bash
# 检查 IOMMU 是否启用
dmesg | grep -i iommu

# 如果没有输出，检查 BIOS 设置和内核参数
```

**问题：设备已被其他驱动占用**

```bash
# 查看设备当前驱动
lspci -k -s 01:00.0

# 解绑驱动
echo "0000:01:00.0" | sudo tee /sys/bus/pci/drivers/nvidia/unbind

# 绑定到 vfio-pci
echo "10de 1b80" | sudo tee /sys/bus/pci/drivers/vfio-pci/new_id
echo "0000:01:00.0" | sudo tee /sys/bus/pci/drivers/vfio-pci/bind
```

**问题：权限不足**

```bash
# 添加用户到 vfio 组
sudo usermod -a -G vfio $USER

# 或者使用 sudo 运行虚拟机
sudo vm-cli --kernel kernel.bin --gpu-mode passthrough
```

### Mediated Device 问题

**问题：找不到 mdev_supported_types**

```bash
# 检查内核模块是否加载
lsmod | grep kvmgt  # Intel
lsmod | grep nvidia_vgpu  # NVIDIA

# 加载模块
sudo modprobe kvmgt
```

**问题：可用实例数为 0**

某些 mdev 类型限制了可创建的实例数量。尝试使用其他类型或删除现有的 mdev 设备。

### WGPU 问题

**问题：找不到适配器**

WGPU 会尝试使用 Vulkan、Metal、DirectX 12 或 OpenGL。确保系统至少支持其中一种。

```bash
# 检查 Vulkan 支持
vulkaninfo

# 安装 Vulkan 驱动
sudo apt install mesa-vulkan-drivers  # AMD/Intel
sudo apt install nvidia-vulkan-driver  # NVIDIA
```

## 性能优化

### GPU Passthrough 优化

1. **启用 IOMMU 直通模式**：在内核参数中添加 `iommu=pt`，可以提高非直通设备的性能。

2. **使用 hugepages**：为虚拟机分配大页内存，减少 TLB miss。

3. **CPU 亲和性**：将虚拟机的 vCPU 绑定到特定的物理 CPU 核心。

### Mediated Device 优化

1. **选择合适的 mdev 类型**：不同的 mdev 类型有不同的性能特征，选择适合工作负载的类型。

2. **限制实例数量**：减少同时运行的 mdev 实例数量，可以提高单个实例的性能。

### WGPU 优化

1. **选择高性能后端**：WGPU 会自动选择最佳后端，但可以通过环境变量强制使用特定后端：

   ```bash
   export WGPU_BACKEND=vulkan  # 或 dx12, metal, gl
   ```

2. **启用性能模式**：在创建适配器时选择高性能模式（已在代码中默认启用）。

## 示例代码

完整的示例代码请参见：
- `examples/gpu_passthrough_example.rs`
- `examples/gpu_mdev_example.rs`
- `examples/gpu_unified_example.rs`

## 参考资料

- [VFIO - "Virtual Function I/O"](https://www.kernel.org/doc/Documentation/vfio.txt)
- [Intel GVT-g](https://github.com/intel/gvt-linux/wiki)
- [NVIDIA vGPU](https://docs.nvidia.com/grid/latest/grid-vgpu-user-guide/)
- [WGPU Documentation](https://wgpu.rs/)
