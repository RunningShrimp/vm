# 事件驱动架构迁移指南

## 概述

本文档说明如何将核心模块迁移到事件驱动架构，使JIT编译、内存操作和设备操作发布领域事件。

## 已完成的工作

### 1. JIT编译器事件发布 ✓

**位置**: `vm-engine-jit/src/lib.rs`

**已实现**:
- 在 `Jit` 结构体中添加了可选的事件总线字段 (`event_bus`) 和 VM ID (`vm_id`)
- 添加了 `set_event_bus()` 和 `set_vm_id()` 方法用于配置事件发布
- 在 `compile()` 方法成功编译后发布 `CodeBlockCompiled` 事件
- 在 `record_execution()` 方法检测到热点时发布 `HotspotDetected` 事件

**使用示例**:
```rust
use vm_core::domain_event_bus::DomainEventBus;
use vm_engine_jit::Jit;

let mut jit = Jit::new();
let event_bus = Arc::new(DomainEventBus::new());
jit.set_event_bus(event_bus.clone());
jit.set_vm_id("vm-001".to_string());

// 当代码块被编译时，会自动发布 CodeBlockCompiled 事件
// 当检测到热点时，会自动发布 HotspotDetected 事件
```

**发布的事件**:
- `ExecutionEvent::CodeBlockCompiled` - 代码块编译完成
- `ExecutionEvent::HotspotDetected` - 检测到热点代码

## 待完成的工作

### 2. MMU内存操作事件发布

**位置**: `vm-mem/src/lib.rs` 和 `vm-mem/src/unified_mmu.rs`

**需要添加**:
1. 在 `SoftMmu` 和 `UnifiedMmu` 结构体中添加可选的事件总线字段
2. 添加设置事件总线和VM ID的方法
3. 在以下位置发布事件：
   - 内存分配时：`MemoryEvent::MemoryAllocated`
   - 内存释放时：`MemoryEvent::MemoryFreed`
   - 页错误时：`MemoryEvent::PageFault`
   - 内存映射时：`MemoryEvent::MemoryMapped`
   - 内存取消映射时：`MemoryEvent::MemoryUnmapped`

**实现步骤**:
```rust
// 1. 在 SoftMmu 结构体中添加字段
pub struct SoftMmu {
    // ... 现有字段 ...
    event_bus: Option<Arc<vm_core::domain_event_bus::DomainEventBus>>,
    vm_id: Option<String>,
}

// 2. 添加设置方法
impl SoftMmu {
    pub fn set_event_bus(&mut self, event_bus: Arc<vm_core::domain_event_bus::DomainEventBus>) {
        self.event_bus = Some(event_bus);
    }

    pub fn set_vm_id(&mut self, vm_id: String) {
        self.vm_id = Some(vm_id);
    }

    // 3. 添加事件发布辅助方法
    fn publish_memory_allocated(&self, addr: GuestAddr, size: u64) {
        if let (Some(ref bus), Some(ref vm_id)) = (&self.event_bus, &self.vm_id) {
            let event = vm_core::domain_events::MemoryEvent::MemoryAllocated {
                vm_id: vm_id.clone(),
                addr,
                size,
                occurred_at: std::time::SystemTime::now(),
            };
            let _ = bus.publish(event);
        }
    }

    // 类似地添加其他事件发布方法...
}

// 4. 在适当的位置调用事件发布
impl MMU for SoftMmu {
    fn translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestPhysAddr, VmError> {
        // ... 现有代码 ...
        
        // 如果发生页错误，发布事件
        if page_fault_occurred {
            self.publish_page_fault(va, access == AccessType::Write);
        }
        
        // ... 现有代码 ...
    }
}
```

### 3. 设备操作事件发布

**位置**: `vm-device/src/` 中的各个设备模块

**需要添加**:
1. 在设备结构体中添加可选的事件总线字段
2. 添加设置事件总线和VM ID的方法
3. 在以下位置发布事件：
   - 设备中断时：`DeviceEvent::DeviceInterrupt`
   - I/O完成时：`DeviceEvent::DeviceIoCompleted`
   - 设备添加时：`DeviceEvent::DeviceAdded`
   - 设备移除时：`DeviceEvent::DeviceRemoved`

**实现步骤**:
```rust
// 1. 在设备结构体中添加字段（或使用设备管理器统一管理）
pub struct DeviceManager {
    // ... 现有字段 ...
    event_bus: Option<Arc<vm_core::domain_event_bus::DomainEventBus>>,
    vm_id: Option<String>,
}

// 2. 在设备I/O完成时发布事件
impl IoScheduler {
    pub fn poll(&mut self) -> Vec<IoCompletion> {
        let completions = self.backend.poll_completions();
        
        // 发布I/O完成事件
        for completion in &completions {
            if let Some(ref bus) = self.event_bus {
                let event = vm_core::domain_events::DeviceEvent::DeviceIoCompleted {
                    vm_id: self.vm_id.clone().unwrap_or_default(),
                    device_id: completion.device_id.to_string(),
                    bytes_transferred: completion.result,
                    occurred_at: std::time::SystemTime::now(),
                };
                let _ = bus.publish(event);
            }
        }
        
        completions
    }
}

// 3. 在设备中断时发布事件
impl MmioDevice for SomeDevice {
    fn notify(&mut self, mmu: &mut dyn MMU, offset: u64) {
        // ... 现有代码 ...
        
        // 发布设备中断事件
        if let Some(ref bus) = self.event_bus {
            let event = vm_core::domain_events::DeviceEvent::DeviceInterrupt {
                vm_id: self.vm_id.clone().unwrap_or_default(),
                device_id: self.device_id.clone(),
                irq: self.irq_number,
                occurred_at: std::time::SystemTime::now(),
            };
            let _ = bus.publish(event);
        }
    }
}
```

## 事件订阅示例

**订阅JIT编译事件**:
```rust
use vm_core::domain_event_bus::{DomainEventBus, EventHandler};
use vm_core::domain_events::{DomainEvent, ExecutionEvent};

struct CompilationEventHandler;

impl EventHandler for CompilationEventHandler {
    fn handle(&self, event: &dyn DomainEvent) -> Result<(), VmError> {
        if let Some(exec_event) = event.as_any().downcast_ref::<ExecutionEvent>() {
            match exec_event {
                ExecutionEvent::CodeBlockCompiled { pc, block_size, .. } => {
                    println!("Code block compiled at PC: 0x{:x}, size: {}", pc, block_size);
                }
                ExecutionEvent::HotspotDetected { pc, execution_count, .. } => {
                    println!("Hotspot detected at PC: 0x{:x}, count: {}", pc, execution_count);
                }
                _ => {}
            }
        }
        Ok(())
    }
}

let event_bus = Arc::new(DomainEventBus::new());
event_bus.subscribe(
    "execution.code_block_compiled",
    Box::new(CompilationEventHandler),
    None,
)?;
```

## 性能考虑

1. **可选事件发布**: 所有事件发布都是可选的，如果未设置事件总线，不会影响性能
2. **异步事件总线**: 对于高频事件，可以使用 `AsyncEventBus` 进行异步处理
3. **事件过滤**: 使用事件过滤器可以只处理感兴趣的事件，减少开销
4. **批量处理**: 对于大量事件，可以使用批处理机制

## 测试

为事件发布添加测试：

```rust
#[test]
fn test_jit_compilation_event() {
    let event_bus = Arc::new(DomainEventBus::new());
    let mut jit = Jit::new();
    jit.set_event_bus(event_bus.clone());
    jit.set_vm_id("test-vm".to_string());
    
    // 订阅事件
    let event_received = Arc::new(AtomicBool::new(false));
    let event_received_clone = event_received.clone();
    
    struct TestHandler {
        flag: Arc<AtomicBool>,
    }
    
    impl EventHandler for TestHandler {
        fn handle(&self, event: &dyn DomainEvent) -> Result<(), VmError> {
            if event.event_type() == "execution.code_block_compiled" {
                self.flag.store(true, Ordering::Relaxed);
            }
            Ok(())
        }
    }
    
    event_bus.subscribe(
        "execution.code_block_compiled",
        Box::new(TestHandler { flag: event_received_clone }),
        None,
    ).unwrap();
    
    // 触发编译
    let block = create_test_block();
    jit.compile(&block);
    
    // 验证事件已发布
    assert!(event_received.load(Ordering::Relaxed));
}
```

## 迁移检查清单

- [x] JIT编译器事件发布
  - [x] 添加事件总线字段
  - [x] 添加设置方法
  - [x] 发布 CodeBlockCompiled 事件
  - [x] 发布 HotspotDetected 事件
- [ ] MMU内存操作事件发布
  - [ ] 添加事件总线字段
  - [ ] 发布 MemoryAllocated 事件
  - [ ] 发布 MemoryFreed 事件
  - [ ] 发布 PageFault 事件
  - [ ] 发布 MemoryMapped 事件
  - [ ] 发布 MemoryUnmapped 事件
- [ ] 设备操作事件发布
  - [ ] 添加事件总线字段
  - [ ] 发布 DeviceInterrupt 事件
  - [ ] 发布 DeviceIoCompleted 事件
  - [ ] 发布 DeviceAdded 事件
  - [ ] 发布 DeviceRemoved 事件

## 注意事项

1. **向后兼容**: 所有事件发布都是可选的，不会破坏现有API
2. **性能影响**: 事件发布是同步的，如果性能敏感，应该使用异步事件总线
3. **错误处理**: 事件发布失败不应该影响主要功能，使用 `let _ = bus.publish(event);` 忽略错误
4. **VM ID**: 需要确保每个VM实例都有唯一的ID，用于事件追踪


