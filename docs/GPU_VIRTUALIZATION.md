# GPU虚拟化支持

## 概述

GPU虚拟化模块（`vm-gpu`）提供了完整的GPU设备抽象、命令队列管理和内存管理功能，支持GPU直通、MDEV和WGPU虚拟化。

## 功能特性

### 1. GPU设备抽象

`GpuDevice` trait定义了GPU设备的统一接口：

- **设备管理**：初始化、启动、停止
- **命令提交**：提交GPU命令到命令队列
- **命令处理**：处理命令队列中的命令
- **内存管理**：GPU内存映射和取消映射
- **寄存器访问**：读写GPU寄存器
- **中断处理**：获取和清除中断状态

### 2. GPU命令队列

`GpuCommandQueue`提供了高效的命令队列管理：

- **命令提交**：支持单个和批量提交
- **命令出队**：支持阻塞和非阻塞模式
- **队列状态管理**：空闲、运行、暂停、错误
- **统计信息**：提交数、完成数、平均等待时间
- **超时控制**：支持超时等待

### 3. GPU内存管理

`GpuMemoryAllocator`提供GPU内存分配和管理：

- **内存分配**：分配GPU内存块
- **CPU映射**：支持GPU内存到CPU地址空间的映射
- **内存释放**：释放已分配的内存
- **内存统计**：跟踪内存使用情况
- **碎片整理**：内存碎片整理（计划中）

## 使用方法

### 创建GPU设备

```rust
use vm_gpu::{GpuDeviceSimulator, GpuDeviceInfo, GpuDeviceType, GpuDevice};

// 创建设备信息
let device_info = GpuDeviceInfo {
    device_id: "gpu0".to_string(),
    name: "Test GPU".to_string(),
    device_type: GpuDeviceType::Nvidia,
    total_memory: 4 * 1024 * 1024 * 1024, // 4GB
    available_memory: 3 * 1024 * 1024 * 1024, // 3GB
    compute_capability: "8.0".to_string(),
    supported_apis: vec!["CUDA".to_string(), "OpenCL".to_string()],
    power_limit_watts: Some(250),
    temperature_celsius: Some(45.0),
};

// 创建设备
let mut device = GpuDeviceSimulator::new(device_info);

// 初始化设备
device.initialize()?;

// 启动设备
device.start()?;
```

### 提交GPU命令

```rust
use vm_gpu::{GpuCommand, GpuCommandType};

// 创建命令
let command = GpuCommand {
    command_type: GpuCommandType::KernelLaunch,
    parameters: vec![kernel_addr, grid_x, grid_y, grid_z],
    submit_time: Instant::now(),
};

// 提交命令
device.submit_command(command)?;

// 处理命令队列
let processed = device.process_command_queue();
println!("Processed {} commands", processed);
```

### 使用命令队列

```rust
use vm_gpu::{GpuCommandQueue, GpuCommand, GpuCommandType};

// 创建命令队列
let queue = GpuCommandQueue::new(1000); // 最大1000个命令

// 启动队列
queue.start();

// 提交命令
let command = GpuCommand {
    command_type: GpuCommandType::MemoryTransferToGpu,
    parameters: vec![src_addr, dst_addr, size],
    submit_time: Instant::now(),
};
queue.submit(command)?;

// 批量提交
let commands: Vec<GpuCommand> = vec![/* ... */];
queue.submit_batch(commands)?;

// 处理命令队列
let processed = queue.process_command_queue(|cmd| {
    // 执行命令
    execute_gpu_command(cmd)?;
    Ok(())
});
```

### GPU内存管理

```rust
use vm_gpu::{GpuMemoryAllocator, GpuMemoryManager};

// 创建内存分配器
let mut allocator = GpuMemoryAllocator::new();

// 初始化（需要设备信息）
let mut devices = HashMap::new();
devices.insert("gpu0".to_string(), device_info);
allocator.initialize(&devices)?;

// 分配GPU内存
let block = allocator.allocate("gpu0", 64 * 1024 * 1024)?; // 64MB
println!("Allocated GPU memory at: 0x{:x}", block.gpu_address);

// 分配带CPU映射的内存
let block_with_cpu = allocator.allocate_with_cpu_map(
    "gpu0",
    64 * 1024 * 1024,
    Some(0x1000000), // CPU地址
)?;

// 释放内存
allocator.free_memory(&block.block_id)?;

// 获取内存统计
let stats = allocator.get_memory_stats();
println!("Total memory: {} MB", stats.total_memory / 1024 / 1024);
println!("Allocated: {} MB", stats.allocated_memory / 1024 / 1024);
```

### 集成到设备管理

```rust
use vm_device::gpu_manager::{UnifiedGpuManager, GpuMode};

// 创建GPU管理器
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

## 命令类型

支持以下GPU命令类型：

- `MemoryTransferToGpu`: CPU到GPU的内存传输
- `MemoryTransferFromGpu`: GPU到CPU的内存传输
- `KernelLaunch`: 启动计算内核
- `Synchronize`: 同步点（等待所有之前的命令完成）
- `MemoryCopy`: GPU内部内存复制
- `MemoryClear`: 清除GPU内存

## 命令队列状态

- `Idle`: 空闲状态
- `Running`: 运行中，可以接受和处理命令
- `Paused`: 暂停状态，不接受新命令但可以处理已有命令
- `Error`: 错误状态，需要重置

## 性能优化

### 批量提交

使用`submit_batch`批量提交命令可以减少锁竞争：

```rust
let commands: Vec<GpuCommand> = (0..100)
    .map(|i| GpuCommand {
        command_type: GpuCommandType::KernelLaunch,
        parameters: vec![i],
        submit_time: Instant::now(),
    })
    .collect();

let submitted = queue.submit_batch(commands)?;
```

### 异步处理

命令队列支持异步处理，可以在后台线程中处理命令：

```rust
use std::thread;

let queue_clone = Arc::clone(&queue);
thread::spawn(move || {
    loop {
        if let Some(command) = queue_clone.dequeue(Some(Duration::from_millis(100))) {
            execute_command(&command);
            let wait_time = command.submit_time.elapsed().as_micros() as u64;
            queue_clone.mark_completed(wait_time);
        }
    }
});
```

## 错误处理

命令队列可能返回以下错误：

- `CommandQueueError::QueueFull`: 队列已满
- `CommandQueueError::QueueError`: 队列处于错误状态
- `CommandQueueError::InvalidCommand`: 无效的命令

```rust
match queue.submit(command) {
    Ok(_) => println!("Command submitted"),
    Err(CommandQueueError::QueueFull) => {
        println!("Queue is full, waiting...");
        // 等待队列有空间
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## 统计和监控

### 命令队列统计

```rust
let stats = queue.get_stats();
println!("Total submitted: {}", stats.total_submitted);
println!("Total completed: {}", stats.total_completed);
println!("Average wait time: {} us", stats.avg_wait_time_us);
println!("Max queue depth: {}", stats.max_queue_depth);
println!("Overflow count: {}", stats.overflow_count);
```

### 内存统计

```rust
let stats = allocator.get_memory_stats();
println!("Total memory: {} bytes", stats.total_memory);
println!("Allocated: {} bytes", stats.allocated_memory);
println!("Available: {} bytes", stats.available_memory);
println!("Allocation count: {}", stats.allocation_count);
```

## 最佳实践

1. **合理设置队列大小**：根据应用需求设置队列大小，避免过大导致内存浪费
2. **批量提交命令**：尽可能批量提交命令以减少锁竞争
3. **异步处理**：在后台线程中处理命令队列，避免阻塞主线程
4. **监控统计信息**：定期检查统计信息，及时发现性能问题
5. **错误处理**：正确处理队列满和错误状态
6. **内存管理**：及时释放不再使用的GPU内存

## 示例

完整示例请参考`vm-gpu/examples/`目录。

## 未来改进

1. **优先级队列**：支持命令优先级调度
2. **命令依赖**：支持命令之间的依赖关系
3. **内存碎片整理**：实现完整的内存碎片整理算法
4. **多GPU支持**：支持多GPU设备的负载均衡
5. **性能分析**：添加详细的性能分析工具


