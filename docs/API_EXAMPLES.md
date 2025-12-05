# API使用示例

本文档提供了虚拟机项目主要公共API的使用示例，帮助开发者快速上手。

## 目录

- [虚拟机服务 (vm-service)](#虚拟机服务-vm-service)
- [核心库 (vm-core)](#核心库-vm-core)
- [JIT引擎 (vm-engine-jit)](#jit引擎-vm-engine-jit)
- [AOT构建器 (aot-builder)](#aot构建器-aot-builder)
- [内存管理 (vm-mem)](#内存管理-vm-mem)
- [设备管理 (vm-device)](#设备管理-vm-device)
- [GPU虚拟化 (vm-gpu)](#gpu虚拟化-vm-gpu)
- [事件驱动架构](#事件驱动架构)

## 虚拟机服务 (vm-service)

### 创建和配置虚拟机

```rust
use vm_service::VmService;
use vm_core::{VmConfig, GuestArch, ExecMode};
use vm_mem::SoftMmu;

// 创建VM配置
let config = VmConfig {
    guest_arch: GuestArch::Riscv64,
    memory_size: 128 * 1024 * 1024, // 128MB
    vcpu_count: 1,
    exec_mode: ExecMode::Jit,
    ..Default::default()
};

// 创建VM服务
let mut vm_service = VmService::new(config)?;

// 加载内核
let kernel_data = std::fs::read("kernel.bin")?;
vm_service.load_kernel(&kernel_data, 0x80000000)?;

// 启动VM
vm_service.start()?;
```

### 执行虚拟机

```rust
use vm_service::VmService;
use vm_core::GuestAddr;

// 同步执行
let start_pc: GuestAddr = 0x80000000;
vm_service.run(start_pc, false)?; // false = 非调试模式

// 异步执行（需要async feature）
#[cfg(feature = "async")]
{
    vm_service.run_async(start_pc, false, 1, ExecMode::Jit).await?;
}
```

### 快照管理

```rust
use vm_service::VmService;

// 创建快照
let snapshot_id = vm_service.create_snapshot(
    "snapshot1".to_string(),
    "Initial state".to_string()
)?;

// 列出所有快照
let snapshots = vm_service.list_snapshots()?;
for snapshot in snapshots {
    println!("Snapshot: {} - {}", snapshot.id, snapshot.description);
}

// 恢复快照
vm_service.restore_snapshot(&snapshot_id)?;
```

### JIT配置

```rust
use vm_service::VmService;
use vm_engine_jit::{EwmaHotspotConfig, AdaptiveThresholdConfig};

// 配置热点检测
let hotspot_config = EwmaHotspotConfig {
    alpha: 0.1,
    threshold: 200,
    min_samples: 10,
};

vm_service.set_hot_config(hotspot_config)?;

// 获取热点统计
let stats = vm_service.hot_stats()?;
println!("Hot spots detected: {}", stats.hot_spots);
```

## 核心库 (vm-core)

### 虚拟机配置

```rust
use vm_core::{VmConfig, GuestArch, ExecMode};

let config = VmConfig {
    guest_arch: GuestArch::X86_64,
    memory_size: 256 * 1024 * 1024, // 256MB
    vcpu_count: 2,
    exec_mode: ExecMode::Hybrid, // 混合执行模式
    enable_jit: true,
    enable_aot: false,
    ..Default::default()
};
```

### 执行引擎

```rust
use vm_core::{ExecutionEngine, GuestAddr};
use vm_engine_jit::Jit;
use vm_mem::SoftMmu;
use vm_ir::IRBlock;

let mut jit = Jit::new();
let mut mmu = SoftMmu::new(1024 * 1024 * 1024, false); // 1GB内存

// 创建IR块
let block = IRBlock {
    start_pc: 0x1000,
    ops: vec![/* ... */],
    term: Terminator::Return,
};

// 执行
let result = jit.run(&mut mmu, &block)?;
println!("Execution result: {:?}", result);
```

### 内存管理单元 (MMU)

```rust
use vm_core::{MMU, GuestAddr, AccessType};
use vm_mem::SoftMmu;

let mut mmu = SoftMmu::new(1024 * 1024 * 1024, false); // 1GB内存

// 读取内存
let value = mmu.read(0x1000, 8)?; // 读取8字节

// 写入内存
mmu.write(0x1000, 0x1234567890ABCDEF, 8)?;

// 映射内存
mmu.map(
    0x80000000,  // Guest地址
    0x10000000,  // Host地址
    4096,        // 大小
    0x7,         // 权限（RWX）
)?;

// 翻译地址
let host_addr = mmu.translate(0x80000000, 0, AccessType::Read)?;
```

### 领域事件总线

```rust
use vm_core::domain_event_bus::DomainEventBus;
use vm_core::domain_events::{DomainEventEnum, VmLifecycleEvent};
use std::sync::Arc;

// 创建事件总线
let event_bus = Arc::new(DomainEventBus::new());

// 订阅事件
let subscription_id = event_bus.subscribe(
    "vm.created",
    Box::new(|event: &dyn DomainEvent| {
        println!("VM created: {:?}", event);
        Ok(())
    }),
    None, // 无过滤器
)?;

// 发布事件
let event = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated {
    vm_id: "vm-001".to_string(),
    config: VmConfig::default(),
    occurred_at: std::time::SystemTime::now(),
});
event_bus.publish(event)?;

// 取消订阅
event_bus.unsubscribe_by_id(subscription_id)?;
```

## JIT引擎 (vm-engine-jit)

### 基本使用

```rust
use vm_engine_jit::Jit;
use vm_core::{ExecutionEngine, GuestAddr};
use vm_mem::SoftMmu;
use vm_ir::{IRBlock, IROp, Terminator};

let mut jit = Jit::new();
let mut mmu = SoftMmu::new(1024 * 1024 * 1024, false);

// 创建IR块
let block = IRBlock {
    start_pc: 0x1000,
    ops: vec![
        IROp::MovImm { dst: 1, imm: 42 },
        IROp::Add { dst: 1, src1: 1, src2: 2 },
    ],
    term: Terminator::Return,
};

// 执行
let result = jit.run(&mut mmu, &block)?;
```

### 热点检测配置

```rust
use vm_engine_jit::{Jit, EwmaHotspotDetector, EwmaHotspotConfig};

let mut jit = Jit::new();

// 配置热点检测
let config = EwmaHotspotConfig {
    alpha: 0.1,      // EWMA平滑因子
    threshold: 200, // 热点阈值
    min_samples: 10, // 最小样本数
};

// 设置配置（需要通过内部方法，这里展示概念）
// jit.set_hotspot_config(config);
```

### 事件驱动集成

```rust
use vm_engine_jit::Jit;
use vm_core::domain_event_bus::DomainEventBus;
use std::sync::Arc;

let mut jit = Jit::new();

// 设置事件总线
let event_bus = Arc::new(DomainEventBus::new());
jit.set_event_bus(Arc::clone(&event_bus));
jit.set_vm_id("vm-001".to_string());

// 现在JIT编译会自动发布事件
// - CodeBlockCompiled: 代码块编译完成
// - HotspotDetected: 检测到热点代码
```

### 分层编译

```rust
use vm_engine_jit::{Jit, tiered_compiler::TieredCompiler};

// 分层编译会自动根据执行次数选择编译策略
// - < 200次执行: 快速路径（基础优化）
// - >= 200次执行: 优化路径（完整优化）

let mut jit = Jit::new();
// 分层编译已集成到Jit::compile方法中
```

### GC配置

```rust
use vm_engine_jit::unified_gc::{UnifiedGC, UnifiedGcConfig};

let config = UnifiedGcConfig {
    enable_generational: true,
    young_gen_ratio: 0.3,      // 年轻代占30%
    promotion_threshold: 3,     // 存活3次后晋升
    enable_adaptive_adjustment: true, // 启用自适应调整
    allocation_trigger_threshold: 10 * 1024 * 1024, // 10MB/秒
    ..Default::default()
};

let gc = UnifiedGC::new(config);

// 记录分配
gc.record_allocation(1024)?;

// 检查是否应该触发GC
if gc.should_trigger_gc() {
    let roots = vec![0x1000, 0x2000];
    let cycle_start = gc.start_gc(&roots);
    // ... 执行GC ...
    gc.finish_gc(cycle_start);
}
```

## AOT构建器 (aot-builder)

### 基本使用

```rust
use aot_builder::{AotBuilder, CompilationOptions, CodegenMode};
use vm_ir_lift::ISA;

// 创建构建器
let mut builder = AotBuilder::new();

// 添加预编译的代码块
builder.add_compiled_block(0x1000, vec![0x90, 0xC3], 1)?; // NOP + RET

// 从原始机器码编译
builder.add_raw_code_block(0x2000, &[0x48, 0x89, 0xC3], 1)?; // MOV RBX, RAX

// 从IR块编译
use vm_ir::{IRBlock, IROp, Terminator};
let ir_block = IRBlock {
    start_pc: 0x3000,
    ops: vec![IROp::MovImm { dst: 1, imm: 42 }],
    term: Terminator::Return,
};
builder.add_ir_block(0x3000, &ir_block, 1)?;

// 构建AOT镜像
let image = builder.build()?;

// 保存到文件
use std::fs::File;
let mut file = File::create("aot_image.bin")?;
image.serialize(&mut file)?;
```

### 增量AOT编译

```rust
use aot_builder::incremental::{IncrementalAotBuilder, IncrementalConfig};
use vm_ir::{IRBlock, IROp, Terminator};
use std::collections::HashMap;

let config = IncrementalConfig {
    enable_incremental: true,
    create_new_if_missing: true,
};

let mut incremental_builder = IncrementalAotBuilder::new(
    CompilationOptions::default(),
    config,
);

// 加载现有AOT镜像（如果存在）
if std::path::Path::new("existing_aot.bin").exists() {
    incremental_builder.load_existing_image("existing_aot.bin")?;
}

// 准备当前IR块
let mut current_blocks = HashMap::new();
current_blocks.insert(0x1000, IRBlock {
    start_pc: 0x1000,
    ops: vec![IROp::MovImm { dst: 1, imm: 100 }],
    term: Terminator::Return,
});

// 检测变更
incremental_builder.detect_changes(&current_blocks)?;

// 执行增量编译
incremental_builder.compile_incremental(&current_blocks)?;

// 更新现有镜像
incremental_builder.update_existing_image("updated_aot.bin")?;

// 获取变更统计
let stats = incremental_builder.change_stats();
println!("Added: {}, Modified: {}, Removed: {}, Unchanged: {}",
    stats.added_blocks, stats.modified_blocks,
    stats.removed_blocks, stats.unchanged_blocks);
```

### 编译选项

```rust
use aot_builder::{CompilationOptions, CodegenMode};
use vm_ir_lift::{ISA, OptimizationLevel};

let options = CompilationOptions {
    optimization_level: 2, // O2优化
    target_isa: ISA::X86_64,
    enable_applicability_check: true,
    codegen_mode: CodegenMode::LLVM, // 使用LLVM代码生成
};

let mut builder = AotBuilder::with_options(options);
```

## 内存管理 (vm-mem)

### SoftMmu使用

```rust
use vm_mem::SoftMmu;
use vm_core::{MMU, GuestAddr, AccessType};

// 创建MMU
let mut mmu = SoftMmu::new(1024 * 1024 * 1024, false); // 1GB内存

// 映射内存页
mmu.map(0x1000, 0x2000, 4096, 0x7)?; // 4KB页，RWX权限

// 读取内存
let value = mmu.read(0x1000, 8)?;

// 写入内存
mmu.write(0x1000, 0x1234567890ABCDEF, 8)?;

// 地址翻译
let host_addr = mmu.translate(0x1000, 0, AccessType::Read)?;
```

### 异步MMU

```rust
#[cfg(feature = "async")]
use vm_mem::async_mmu::AsyncSoftMmu;
#[cfg(feature = "async")]
use vm_core::{AsyncMMU, GuestAddr, AccessType};

#[cfg(feature = "async")]
async fn async_mmu_example() -> Result<(), Box<dyn std::error::Error>> {
    let mut mmu = AsyncSoftMmu::new(1024 * 1024 * 1024, false);

    // 异步映射
    mmu.map_async(0x1000, 0x2000, 4096, 0x7).await?;

    // 异步读取
    let value = mmu.read_async(0x1000, 8).await?;

    // 异步写入
    mmu.write_async(0x1000, 0x1234567890ABCDEF, 8).await?;

    Ok(())
}
```

### TLB管理

```rust
use vm_mem::tlb_concurrent::{ConcurrentTlbManager, ConcurrentTlbConfig};
use vm_core::{AccessType, GuestAddr};

let config = ConcurrentTlbConfig {
    sharded_capacity: 1024,
    shard_count: 4,
    fast_path_capacity: 256,
    enable_fast_path: true,
    enable_adaptive: false,
};

let tlb = ConcurrentTlbManager::new(config);

// 插入TLB条目
tlb.insert(0x1000, 0x2000, 0x7, 0)?;

// 查找TLB
if let Some(entry) = tlb.translate(0x1000, 0, AccessType::Read) {
    println!("TLB hit: 0x{:x} -> 0x{:x}", entry.vpn, entry.ppn);
}

// 刷新TLB
tlb.flush_all();
```

## 设备管理 (vm-device)

### GPU管理器

```rust
use vm_device::gpu_manager::{UnifiedGpuManager, GpuMode};

let mut gpu_manager = UnifiedGpuManager::new();

// 扫描可用的GPU后端
gpu_manager.scan_backends()?;

// 设置偏好模式
gpu_manager.set_preferred_mode(GpuMode::Passthrough);

// 自动选择最佳后端
gpu_manager.auto_select()?;

// 初始化选中的后端
gpu_manager.initialize_selected()?;

// 获取统计信息
if let Some(stats) = gpu_manager.get_stats() {
    println!("GPU Stats:\n{}", stats);
}
```

### VirtIO设备

```rust
use vm_device::virtio_devices::block::VirtioBlock;
use std::fs::File;

// 创建块设备
let file = File::open("disk.img")?;
let mut block_device = VirtioBlock::new(file, 512)?; // 512字节扇区

// 读取块
let mut buffer = vec![0u8; 512];
block_device.read_block(0, &mut buffer)?;

// 写入块
block_device.write_block(0, &buffer)?;
```

## GPU虚拟化 (vm-gpu)

### GPU设备

```rust
use vm_gpu::{GpuDeviceSimulator, GpuDeviceInfo, GpuDeviceType, GpuDevice};
use vm_gpu::{GpuCommand, GpuCommandType};

// 创建设备信息
let device_info = GpuDeviceInfo {
    device_id: "gpu0".to_string(),
    name: "Test GPU".to_string(),
    device_type: GpuDeviceType::Nvidia,
    total_memory: 4 * 1024 * 1024 * 1024, // 4GB
    available_memory: 3 * 1024 * 1024 * 1024, // 3GB
    compute_capability: "8.0".to_string(),
    supported_apis: vec!["CUDA".to_string()],
    power_limit_watts: Some(250),
    temperature_celsius: Some(45.0),
};

// 创建设备
let mut device = GpuDeviceSimulator::new(device_info);

// 初始化并启动
device.initialize()?;
device.start()?;

// 提交命令
let command = GpuCommand {
    command_type: GpuCommandType::KernelLaunch,
    parameters: vec![kernel_addr, grid_x, grid_y, grid_z],
    submit_time: std::time::Instant::now(),
};
device.submit_command(command)?;

// 处理命令队列
let processed = device.process_command_queue();
println!("Processed {} commands", processed);
```

### GPU命令队列

```rust
use vm_gpu::{GpuCommandQueue, GpuCommand, GpuCommandType};

let queue = GpuCommandQueue::new(1000); // 最大1000个命令
queue.start();

// 提交命令
let command = GpuCommand {
    command_type: GpuCommandType::MemoryTransferToGpu,
    parameters: vec![src_addr, dst_addr, size],
    submit_time: std::time::Instant::now(),
};
queue.submit(command)?;

// 批量提交
let commands: Vec<GpuCommand> = vec![/* ... */];
queue.submit_batch(commands)?;

// 处理命令队列
let processed = queue.process_command_queue(|cmd| {
    execute_gpu_command(cmd)?;
    Ok(())
});
```

### GPU内存管理

```rust
use vm_gpu::{GpuMemoryAllocator, GpuMemoryManager};
use std::collections::HashMap;

let mut allocator = GpuMemoryAllocator::new();

// 初始化
let mut devices = HashMap::new();
devices.insert("gpu0".to_string(), device_info);
allocator.initialize(&devices)?;

// 分配GPU内存
let block = allocator.allocate("gpu0", 64 * 1024 * 1024)?; // 64MB

// 分配带CPU映射的内存
let block_with_cpu = allocator.allocate_with_cpu_map(
    "gpu0",
    64 * 1024 * 1024,
    Some(0x1000000), // CPU地址
)?;

// 释放内存
allocator.free_memory(&block.block_id)?;

// 获取统计
let stats = allocator.get_memory_stats();
```

## 事件驱动架构

### 事件处理器

```rust
use vm_service::event_handlers::{
    RetryEventHandler, StatsEventHandler, VmIdFilter, EventHandlerStats
};
use vm_core::domain_event_bus::DomainEventBus;
use std::sync::Arc;

let event_bus = Arc::new(DomainEventBus::new());

// 创建带重试和统计的事件处理器
let handler = Box::new(|event: &dyn DomainEvent| {
    println!("Event received: {}", event.event_type());
    Ok(())
});

let retry_handler = RetryEventHandler::new(
    Box::new(StatsEventHandler::new(handler)),
    3,   // 最大重试3次
    100, // 重试延迟100ms
);

// 订阅事件（带过滤器）
let filter = Box::new(VmIdFilter::new("vm-001".to_string()));
let subscription_id = event_bus.subscribe(
    "memory.allocated",
    Box::new(retry_handler),
    Some(filter),
)?;
```

### 事件路由

```rust
use vm_service::event_handlers::EventRouter;
use vm_core::domain_event_bus::DomainEventBus;
use vm_core::domain_events::DomainEventEnum;
use std::sync::Arc;

let event_bus = Arc::new(DomainEventBus::new());
let mut router = EventRouter::new(Arc::clone(&event_bus));

// 添加路由规则
router.add_route(
    "execution.code_block_compiled".to_string(),
    EventRoute {
        target_type: "monitoring.performance".to_string(),
        condition: Some("block_size > 1000".to_string()),
    },
);

// 路由事件
let event = DomainEventEnum::Execution(/* ... */);
router.route(&event)?;
```

## 完整示例：创建并运行虚拟机

```rust
use vm_service::VmService;
use vm_core::{VmConfig, GuestArch, ExecMode};
use vm_mem::SoftMmu;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建VM配置
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 128 * 1024 * 1024, // 128MB
        vcpu_count: 1,
        exec_mode: ExecMode::Jit,
        enable_jit: true,
        ..Default::default()
    };

    // 2. 创建VM服务
    let mut vm_service = VmService::new(config)?;

    // 3. 加载内核
    let kernel_data = std::fs::read("kernel.bin")?;
    vm_service.load_kernel(&kernel_data, 0x80000000)?;

    // 4. 创建快照
    let snapshot_id = vm_service.create_snapshot(
        "initial".to_string(),
        "Initial state before execution".to_string(),
    )?;

    // 5. 启动VM
    vm_service.start()?;

    // 6. 执行VM
    let start_pc = 0x80000000;
    vm_service.run(start_pc, false)?;

    // 7. 停止VM
    vm_service.stop()?;

    Ok(())
}
```

## 异步执行示例

```rust
#[cfg(feature = "async")]
use vm_service::VmService;
#[cfg(feature = "async")]
use vm_core::{VmConfig, GuestArch, ExecMode};

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 128 * 1024 * 1024,
        vcpu_count: 2, // 多vCPU
        exec_mode: ExecMode::Jit,
        ..Default::default()
    };

    let mut vm_service = VmService::new(config)?;
    
    // 加载内核
    let kernel_data = std::fs::read("kernel.bin")?;
    vm_service.load_kernel(&kernel_data, 0x80000000)?;

    // 异步执行
    vm_service.start()?;
    vm_service.run_async(0x80000000, false, 2, ExecMode::Jit).await?;
    vm_service.stop()?;

    Ok(())
}
```

## 更多示例

更多详细示例请参考：
- `examples/` 目录下的示例代码
- 各模块的文档注释（使用`cargo doc --open`查看）
- 测试代码（`tests/`目录）

## 注意事项

1. **错误处理**：所有API都可能返回错误，请务必处理`Result`类型
2. **资源管理**：确保在不再使用时正确释放资源
3. **线程安全**：某些API是线程安全的，某些不是，请查看文档
4. **特性标志**：某些功能需要启用相应的feature标志（如`async`）
5. **平台差异**：某些功能在不同平台上可能有不同的行为

## 获取帮助

- 查看API文档：`cargo doc --open`
- 查看示例代码：`examples/`目录
- 查看测试代码：`tests/`目录
- 提交问题：GitHub Issues


