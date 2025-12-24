# API 使用示例

本文档提供了虚拟机系统的 API 使用示例。

## 目录

- [创建虚拟机](#创建虚拟机)
- [配置虚拟机](#配置虚拟机)
- [启动和停止虚拟机](#启动和停止虚拟机)
- [内存操作](#内存操作)
- [JIT 编译](#jit-编译)
- [地址转换](#地址转换)
- [TLB 操作](#tlb-操作)
- [垃圾回收](#垃圾回收)
- [协程调度](#协程调度)
- [错误处理](#错误处理)
- [领域事件](#领域事件)

---

## 创建虚拟机

### 基本示例

```rust
use vm_core::{VmConfig, VmId, ExecMode, GuestArch};
use vm_runtime::create_vm;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建虚拟机 ID
    let vm_id = VmId::new("my-vm".to_string())?;

    // 创建虚拟机配置
    let config = VmConfig {
        guest_arch: GuestArch::Riscv64,
        memory_size: 512 * 1024 * 1024, // 512 MB
        vcpu_count: 2,
        exec_mode: ExecMode::JIT,
        kernel_path: Some("/path/to/kernel".to_string()),
        initrd_path: Some("/path/to/initrd".to_string()),
    };

    // 创建虚拟机
    let vm = create_vm(&vm_id, &config).await?;

    println!("Virtual machine created: {}", vm_id.as_str());

    Ok(())
}
```

### 使用 Builder 模式

```rust
use vm_engine_jit::{BaseConfig, JITCompilationConfig, TieredCompilerConfig};

let config = BaseConfig::default()
    .with_debug(true)
    .with_log_level(vm_engine_jit::LogLevel::Debug)
    .with_worker_threads(4)
    .with_max_memory(1024 * 1024 * 1024); // 1 GB
```

---

## 配置虚拟机

### JIT 引擎配置

```rust
use vm_engine_jit::{BaseConfig, JITCompilationConfig, TieredCompilerConfig};

let jit_config = JITCompilationConfig {
    base: BaseConfig::default()
        .with_debug(true)
        .with_worker_threads(4),
    tiered: TieredCompilerConfig::default(),
};
```

### 缓存配置

```rust
use vm_engine_jit::CacheConfig;

let cache_config = CacheConfig::default()
    .with_size_limit(64 * 1024 * 1024) // 64 MB
    .with_entry_limit(10000)
    .with_eviction_policy(vm_engine_jit::EvictionPolicy::LRU)
    .with_ttl(std::time::Duration::from_secs(300));
```

### GC 配置

```rust
use vm_runtime::{GcConfig, GcTriggerPolicy};

let gc_config = GcConfig {
    trigger_policy: GcTriggerPolicy::Adaptive {
        target_heap_growth: 1.5,
        max_pause_time_ms: 50,
    },
    max_pause_time_ms: 100,
    parallel_marking: true,
    incremental: true,
};
```

---

## 启动和停止虚拟机

### 启动虚拟机

```rust
use vm_runtime::VirtualMachine;

let mut vm = create_vm(&vm_id, &config).await?;

// 启动虚拟机
vm.start().await?;

println!("Virtual machine started");
```

### 暂停和恢复

```rust
// 暂停虚拟机
vm.pause().await?;
println!("Virtual machine paused");

// 恢复虚拟机
vm.resume().await?;
println!("Virtual machine resumed");
```

### 停止虚拟机

```rust
// 停止虚拟机
vm.stop().await?;
println!("Virtual machine stopped");
```

### 完整生命周期

```rust
async fn run_vm() -> Result<(), vm_core::VmError> {
    let vm = create_vm(&vm_id, &config).await?;

    // 启动
    vm.start().await?;

    // 执行一段时间
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    // 暂停
    vm.pause().await?;

    // 恢复
    vm.resume().await?;

    // 停止
    vm.stop().await?;

    Ok(())
}
```

---

## 内存操作

### 读取内存

```rust
use vm_core::GuestAddr;
use vm_mem::SoftwareMmu;

let mut mmu = SoftwareMmu::new(
    vm_mem::mmu::MmuArch::RiscVSv39,
    |addr: GuestAddr, size: usize| -> Result<Vec<u8>, vm_core::VmError> {
        // 读取内存的实现
        Ok(vec![0u8; size])
    },
);

// 读取 4 字节
let value = mmu.read(GuestAddr(0x1000), 4)?;
println!("Read value: {:#x}", value);
```

### 写入内存

```rust
// 写入 4 字节
mmu.write(GuestAddr(0x1000), 0xDEADBEEF, 4)?;
println!("Written value: {:#x}", 0xDEADBEEF);
```

### 批量读写

```rust
// 批量读取
let mut buffer = vec![0u8; 1024];
mmu.read_bulk(GuestAddr(0x1000), &mut buffer)?;

// 批量写入
mmu.write_bulk(GuestAddr(0x2000), &buffer)?;
```

### 原子操作

```rust
// Load-Linked (LL)
let ll_value = mmu.load_reserved(GuestAddr(0x3000), 8)?;

// Store-Conditional (SC)
let sc_success = mmu.store_conditional(GuestAddr(0x3000), ll_value + 1, 8)?;

if sc_success {
    println!("Atomic operation succeeded");
} else {
    println!("Atomic operation failed, retry...");
}
```

---

## JIT 编译

### 基本编译

```rust
use vm_engine_jit::{JITCompiler, JITEngine, JITCompilationConfig};

let config = JITCompilationConfig::default();
let mut engine = JITEngine::new(config);

// 创建 IR 块
let ir_block = vm_engine_jit::core::IRBlock {
    instructions: vec![
        vm_engine_jit::core::IRInstruction::Const {
            dest: 0,
            value: 42,
        },
        vm_engine_jit::core::IRInstruction::Return { value: 0 },
    ],
};

// 编译 IR 块
let compiled = engine.compile_block(&ir_block)?;

// 执行编译后的代码
let result = engine.execute_compiled(&compiled)?;
```

### 分层编译

```rust
use vm_engine_jit::{TieredCompilerConfig, BaselineJITConfig, OptimizedJITConfig};

let tiered_config = TieredCompilerConfig {
    interpreter_config: vm_engine_jit::InterpreterConfig::default(),
    baseline_config: BaselineJITConfig::default(),
    optimized_config: OptimizedJITConfig::default(),
    hotspot_config: vm_engine_jit::HotspotConfig::default(),
    baseline_threshold: 100,
    optimized_threshold: 1000,
};

let jit_config = JITCompilationConfig {
    base: BaseConfig::default(),
    tiered: tiered_config,
};

let mut engine = JITEngine::new(jit_config);
```

### 获取编译统计

```rust
let stats = engine.get_stats();

println!("Compiled blocks: {}", stats.compiled_blocks);
println!("Total instructions: {}", stats.total_instructions);
println!("Average compile time: {:?}", stats.average_compile_time);
```

---

## 地址转换

### 使用 SoftwareMmu

```rust
use vm_mem::{SoftwareMmu, GuestAddr};

let mut mmu = SoftwareMmu::new(
    vm_mem::mmu::MmuArch::X86_64,
    |addr: GuestAddr, size: usize| -> Result<Vec<u8>, vm_core::VmError> {
        Ok(vec![0u8; size])
    },
);

// GVA -> GPA 转换
let gva = GuestAddr(0x8000_0000);
let cr3 = GuestAddr(0);

match mmu.translate(gva, cr3) {
    Ok(result) => {
        println!("Translated GPA: {:#x}", result.gpa.0);
        println!("Page size: {}", result.page_size);
    }
    Err(e) => {
        println!("Translation failed: {}", e);
    }
}
```

### 使用 AddressTranslationDomainService

```rust
use vm_mem::domain_services::AddressTranslationDomainService;

let memory = |addr: GuestAddr, size: usize| -> Result<Vec<u8>, vm_core::VmError> {
    Ok(vec![0u8; size])
};

let service = AddressTranslationDomainService::new(
    vm_mem::mmu::MmuArch::X86_64,
    memory,
);

let result = service.translate(GuestAddr(0x8000_0000), GuestAddr(0))?;
```

---

## TLB 操作

### 使用 MultiLevelTlb

```rust
use vm_mem::MultiLevelTlb;

let mut tlb = MultiLevelTlb::new(vm_mem::tlb::TlbConfig::default());

// 添加条目
let entry = vm_mem::tlb::TlbEntry {
    guest_addr: GuestAddr(0x1000),
    phys_addr: vm_mem::GuestPhysAddr(0x2000),
    asid: 0,
    flags: vm_mem::mmu::PageTableFlags::default(),
};

tlb.update(entry);

// 查找条目
if let Some(found) = tlb.lookup(GuestAddr(0x1000), 0, vm_core::AccessType::Read) {
    println!("TLB hit: {:#x}", found.phys_addr.0);
} else {
    println!("TLB miss");
}

// 清空 TLB
tlb.flush();

// 获取统计
if let Some(stats) = tlb.get_stats() {
    println!("Hit rate: {:.2}%", stats.hit_rate() * 100.0);
}
```

### TLB ASID 支持

```rust
// 清空特定 ASID 的条目
tlb.flush_asid(1);

// 清空特定页面的条目
tlb.flush_page(GuestAddr(0x1000));
```

---

## 垃圾回收

### 使用 GcRuntime

```rust
use vm_runtime::{GcRuntime, GcConfig};

let config = GcConfig {
    trigger_policy: vm_runtime::GcTriggerPolicy::Adaptive {
        target_heap_growth: 1.5,
        max_pause_time_ms: 50,
    },
    max_pause_time_ms: 100,
    parallel_marking: true,
    incremental: true,
};

let mut gc = GcRuntime::new(config);

// 标记对象
let mut heap = vec![0u8; 1024 * 1024];
gc.mark(&mut heap, &[0x1000, 0x2000, 0x3000]);

// 清理对象
gc.sweep(&mut heap);

// 获取统计
let stats = gc.get_stats();
println!("Total collected: {}", stats.total_collected);
println!("Live objects: {}", stats.live_objects);
```

### GC 触发策略

```rust
use vm_runtime::GcTriggerPolicy;

// 固定阈值
let fixed = GcTriggerPolicy::FixedThreshold { threshold: 0.7 };

// 自适应
let adaptive = GcTriggerPolicy::Adaptive {
    target_heap_growth: 1.5,
    max_pause_time_ms: 50,
};

// 内存压力
let pressure = GcTriggerPolicy::MemoryPressure {
    min_free_bytes: 10 * 1024 * 1024,
};

// 时间触发
let timed = GcTriggerPolicy::TimeBased {
    interval_ms: 5000,
};
```

---

## 协程调度

### 使用 CoroutineScheduler

```rust
use vm_runtime::{CoroutineScheduler, SchedulerConfig};

let config = SchedulerConfig::default();
let mut scheduler = CoroutineScheduler::new(config);

// 创建协程
let task1 = async {
    println!("Task 1 executing");
    "result1".to_string()
};

let task2 = async {
    println!("Task 2 executing");
    "result2".to_string()
};

// 生成协程
let mut task1 = Box::pin(task1);
let mut task2 = Box::pin(task2);

let handle1 = scheduler.spawn_coroutine(&mut task1);
let handle2 = scheduler.spawn_coroutine(&mut task2);

// 等待协程完成
let result1 = scheduler.wait_for_coroutine(handle1).await;
let result2 = scheduler.wait_for_coroutine(handle2).await;

println!("Task 1 result: {:?}", result1);
println!("Task 2 result: {:?}", result2);
```

### 使用 CoroutinePool

```rust
use vm_runtime::CoroutinePool;

let pool = CoroutinePool::new(10);

// 提交任务
let future = pool.submit(async {
    println!("Task executing in pool");
    42
});

let result = future.await?;
println!("Result: {}", result);
```

---

## 错误处理

### 基本错误处理

```rust
use vm_core::VmError;

fn some_operation() -> Result<(), VmError> {
    // 执行操作
    Ok(())
}

match some_operation() {
    Ok(()) => println!("Operation succeeded"),
    Err(e) => println!("Operation failed: {}", e),
}
```

### 使用 ErrorContext

```rust
use vm_core::error::ErrorContext;

fn read_instruction(mmu: &mut dyn vm_core::MMU, pc: GuestAddr) -> Result<u64, VmError> {
    mmu.read(pc, 8)
        .context("Failed to read instruction at PC")
}

fn execute_program() -> Result<(), VmError> {
    read_instruction(&mut mmu, pc)
        .with_context(|| format!("Failed to execute instruction at PC {:#x}", pc))
}
```

### 错误恢复

```rust
use vm_core::error::{ErrorRecovery, retry_with_strategy};

let result = retry_with_strategy(
    || {
        device.write(offset, value, 4)
    },
    vm_core::error::ErrorRecoveryStrategy::Fixed {
        max_attempts: 3,
        delay_ms: 100,
    },
)?;

match result {
    Ok(()) => println!("Operation succeeded"),
    Err(e) => println!("Operation failed after retries: {}", e),
}
```

---

## 领域事件

### 订阅事件

```rust
use vm_core::{domain_event_bus::EventBus, domain_events::DomainEventEnum};

let mut event_bus = EventBus::new();

// 订阅虚拟机启动事件
let subscription = event_bus.subscribe(|event| {
    if let DomainEventEnum::VmStartedEvent(event) = event {
        println!("VM started: {}", event.vm_id);
    }
});

// 发布事件
event_bus.publish(DomainEventEnum::VmStartedEvent(vm_core::domain_events::VmStartedEvent {
    vm_id: "my-vm".to_string(),
    timestamp: std::time::SystemTime::now(),
}));

// 取消订阅
event_bus.unsubscribe(subscription);
```

### 处理聚合事件

```rust
use vm_core::aggregate_root::{AggregateRoot, VirtualMachineAggregate};

let mut aggregate = VirtualMachineAggregate::new(vm_id, config);

// 触发状态变化
aggregate.start();

// 获取未提交的事件
let events = aggregate.uncommitted_events();
for event in events {
    println!("Event: {:?}", event);
}

// 标记事件为已提交
aggregate.mark_events_as_committed();
```

---

## 总结

本文档提供了虚拟机系统的各种 API 使用示例，包括：

1. 创建和配置虚拟机
2. 启动、暂停、恢复和停止虚拟机
3. 内存读写和原子操作
4. JIT 编译和执行
5. 地址转换和 TLB 操作
6. 垃圾回收
7. 协程调度
8. 错误处理和恢复
9. 领域事件订阅和发布

更多详细信息，请参考各个模块的架构文档：
- [vm-core/ARCHITECTURE.md](vm-core/ARCHITECTURE.md)
- [vm-engine-jit/ARCHITECTURE.md](vm-engine-jit/ARCHITECTURE.md)
- [vm-runtime/ARCHITECTURE.md](vm-runtime/ARCHITECTURE.md)
- [ERROR_HANDLING.md](ERROR_HANDLING.md)
