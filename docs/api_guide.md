# VM API 使用指南

## 快速开始

### 1. 创建虚拟机

```rust
use vm_core::{VmConfig, GuestArch, ExecutionEngine};
use vm_service::VirtualMachineService;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建配置
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 1024 * 1024 * 1024, // 1GB
        ..Default::default()
    };

    // 创建 VM 服务
    let vm = VirtualMachineService::new(config)?;

    // 加载内核
    vm.load_kernel_file("kernel.bin", vm_core::GuestAddr(0x80000000))?;

    // 启动 VM
    vm.start()?;

    Ok(())
}
```

### 2. 内存管理

#### 2.1 创建 MMU

```rust
use vm_mem::{SoftMMU, PagingMode, PageFlags};

// 创建软件 MMU
let mut mmu = SoftMMU::new(
    1024 * 1024 * 1024, // 1GB 内存
    true,               // 启用 TLB
);

// 映射页面
mmu.map_page(
    vm_core::GuestAddr(0x1000),
    vm_core::GuestAddr(0x2000),
    PageFlags::RW | PageFlags::U,
)?;

// 读写内存
let value: u64 = mmu.read(vm_core::GuestAddr(0x1000))?;
mmu.write(vm_core::GuestAddr(0x1008), 0x42)?;
```

#### 2.2 NUMA 感知分配

```rust
use vm_mem::{NumaAllocator, NumaAllocPolicy};

// 创建 NUMA 分配器
let allocator = NumaAllocator::new(NumaAllocPolicy::Local);

// 分配内存页
let page = allocator.allocate_page(4096)?;
```

### 3. 执行引擎

#### 3.1 使用解释器

```rust
use vm_engine_interpreter::Interpreter;
use vm_frontend::riscv64::RiscvDecoder;

let mut interp = Interpreter::new();
let decoder = RiscvDecoder;

// 执行指令
let result = interp.execute(&mut mmu, &mut decoder, vm_core::GuestAddr(0x8000))?;
println!("Execution status: {:?}", result.status);
```

#### 3.2 使用 JIT 编译

```rust
use vm_engine_jit::{Jit, AdaptiveThresholdConfig};

// 创建 JIT 编译器
let mut jit = Jit::new();

// 配置自适应阈值
let config = AdaptiveThresholdConfig {
    cold_threshold: 100,
    hot_threshold: 1000,
    enable_adaptive: true,
    ..Default::default()
};
jit.set_config(config);

// 编译并执行
let code = &instructions[..];
jit.compile(code)?;
let result = jit.run(&mut mmu, code, vm_core::GuestAddr(0x8000))?;
```

### 4. 跨架构翻译

```rust
use vm_cross_arch::{ArchTranslator, SourceArch, TargetArch};
use vm_ir::IRBlock;

// 创建翻译器（x86_64 -> ARM64）
let translator = ArchTranslator::new(
    SourceArch::X86_64,
    TargetArch::ARM64,
);

// 翻译 IR 块
let source_block: IRBlock = /* ... */;
let translated = translator.translate_block(&source_block)?;

// 使用翻译后的代码
vm.load_code(&translated, vm_core::GuestAddr(0x8000))?;
```

### 5. 设备管理

#### 5.1 注册设备

```rust
use vm_device::{BusDevice, DeviceType};

// 创建 UART 设备
let uart = vm_device::uart::Uart::new(0x10000000);

// 注册到总线
let mut bus = vm_device::Bus::new();
bus.register_device(0x10000000, 0x1000, Arc::new(uart))?;
```

#### 5.2 处理中断

```rust
use vm_core::InterruptController;

// 发送中断
let controller = vm_device::clint::Clint::new(1); // 1 hart
controller.set_msip(0, true)?;  // 发送机器软件中断

// 在执行循环中检查中断
if let Some(interrupt) = controller.pending_interrupt() {
    vm.handle_interrupt(interrupt)?;
}
```

### 6. 快照管理

```rust
// 创建快照
let snapshot_id = vm.create_snapshot(
    "snapshot1".to_string(),
    "Initial state".to_string(),
)?;

// 恢复快照
vm.restore_snapshot(&snapshot_id)?;

// 异步快照（需要 performance feature）
#[cfg(feature = "performance")]
vm.restore_snapshot_async(&snapshot_id).await?;
```

### 7. 性能监控

```rust
use vm_service::PerformanceStats;

// 获取性能统计
#[cfg(feature = "performance")]
let stats = vm.get_performance_stats()?;
if let Some(stats) = stats {
    println!("Execution count: {}", stats.execution_count);
    println!("Cold threshold: {}", stats.cold_threshold);
    println!("Hot threshold: {}", stats.hot_threshold);
    println!("Code pool size: {}", stats.code_pool_size);
}
```

## 高级用法

### 自定义 JIT 配置

```rust
use vm_engine_jit::{JitConfig, OptimizationLevel};

let config = JitConfig {
    enable_jit: true,
    optimization_level: OptimizationLevel::Aggressive,
    enable_inline: true,
    enable_vectorization: true,
    ..Default::default()
};

let jit = Jit::with_config(config)?;
```

### 多线程执行

```rust
use vm_service::ExecutionContext;
use std::sync::atomic::{AtomicBool, Ordering};

// 创建执行上下文
let ctx = ExecutionContext::new(vm_core::GuestArch::Riscv64);

let run_flag = Arc::new(AtomicBool::new(true));
let pause_flag = Arc::new(AtomicBool::new(false));

// 多线程执行
vm.run_async(
    ctx,
    vm_core::GuestAddr(0x80000000),
    run_flag,
    pause_flag,
    4,  // 4 个 VCPU
    usize::MAX,
).await?;
```

### 自定义内存后端

```rust
use vm_mem::MappedMemory;

// 使用映射内存（直接访问 host 内存）
let mem = unsafe { MappedMemory::from_ptr(ptr, size) };
mmu.add_memory_region(vm_core::GuestAddr(0), 4096, mem)?;
```

## 错误处理

### 常见错误类型

```rust
use vm_core::{VmError, VmResult};

fn vm_operation() -> VmResult<()> {
    // 页错误
    Err(VmError::Memory(MemoryError::PageFault {
        addr: vm_core::GuestAddr(0x1000),
        access_type: vm_core::AccessType::Read,
    }))?;

    // 不支持的操作
    Err(VmError::Core(CoreError::NotSupported {
        feature: "SVE".to_string(),
        module: "vm_service".to_string(),
    }))?;

    Ok(())
}
```

### 错误恢复

```rust
match vm_operation() {
    Ok(_) => println!("Success"),
    Err(VmError::Memory(e)) => {
        eprintln!("Memory error: {:?}", e);
        // 尝试恢复
        vm.reset()?;
    }
    Err(e) => {
        eprintln!("Fatal error: {:?}", e);
        std::process::exit(1);
    }
}
```

## 配置示例

### RISC-V Linux 配置

```rust
let config = VmConfig {
    guest_arch: GuestArch::Riscv64,
    memory_size: 512 * 1024 * 1024, // 512MB
    boot_args: "console=ttyS0 root=/dev/vda ro".to_string(),
    initrd: Some("initrd.img".to_string()),
    cpu_count: 1,
    ..Default::default()
};
```

### ARM64 配置

```rust
let config = VmConfig {
    guest_arch: GuestArch::Arm64,
    memory_size: 1024 * 1024 * 1024, // 1GB
    boot_args: "console=ttyAMA0 root=/dev/vda rw".to_string(),
    cpu_count: 4,
    enable_el1: true,  // 启用 EL1（虚拟化支持）
    ..Default::default()
};
```

## 调试技巧

### 启用日志

```bash
RUST_LOG=vm_core=debug,vm_engine_jit=trace cargo run
```

### 性能分析

```rust
use std::time::Instant;

let start = Instant::now();
vm.run_sync(ctx, pc, mmu.clone(), false)?;
let duration = start.elapsed();
println!("Execution took: {:?}", duration);
```

### JIT 代码检查

```rust
// 启用 JIT 调试输出
use vm_engine_jit::JitDebug;

let jit = Jit::new_with_debug(JitDebug {
    print_ir: true,
    print_asm: true,
    print_stats: true,
});
```

## 最佳实践

### 1. 使用 Feature Flags

```toml
# 开发配置
vm-service = { version = "0.1.0", features = ["performance", "devices"] }

# 生产配置
vm-service = { version = "0.1.0", features = ["performance", "accel", "smmu"] }
```

### 2. 错误处理

```rust
// ❌ 不好的做法
let result = vm.start().unwrap();

// ✅ 好的做法
vm.start().map_err(|e| {
    eprintln!("Failed to start VM: {:?}", e);
    std::process::exit(1);
})?;
```

### 3. 资源管理

```rust
// 使用 RAII 确保资源清理
use vm_service::VirtualMachineService;

{
    let vm = VirtualMachineService::new(config)?;
    vm.start()?;
    // VM 在此处自动清理
}
```

### 4. 性能优化

```rust
// ❌ 不好的做法：频繁创建 MMU
for _ in 0..1000 {
    let mmu = SoftMMU::new(1024, true);
    // 使用 mmu...
}

// ✅ 好的做法：重用 MMU
let mmu = SoftMMU::new(1024, true);
for _ in 0..1000 {
    // 使用 mmu...
}
```

## 参考资料

- [架构文档](./architecture.md)
- [贡献指南](./CONTRIBUTING.md)
- [API 文档](https://docs.rs/vm)
