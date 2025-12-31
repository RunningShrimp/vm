# vm-interface API 参考

## 概述

`vm-interface` 提供虚拟机各组件的统一接口规范，遵循SOLID原则，提高代码的可维护性和扩展性。该模块定义了VM组件、执行引擎、内存管理、设备管理等核心接口。

## 主要功能

- **组件生命周期**: VmComponent trait 定义初始化、启动、停止
- **配置管理**: Configurable trait 提供动态配置支持
- **状态观察**: Observable trait 实现发布-订阅模式
- **执行引擎接口**: 扩展的ExecutionEngine trait
- **内存管理接口**: 统一的内存管理抽象
- **设备管理接口**: 设备注册、路由、中断处理

## 核心Trait

### VmComponent

VM组件基础trait，定义生命周期管理。

#### 关联类型

##### `type Config`

配置类型。

##### `type Error`

错误类型。

#### 方法

##### `init(config: Self::Config) -> Result<Self, Self::Error> where Self: Sized`

初始化组件。

**参数**:
- `config`: 组件配置

**返回**:
- 成功返回初始化的组件实例，失败返回错误

**示例**:
```rust
use vm_interface::VmComponent;

struct MyComponent {
    config: MyConfig,
}

impl VmComponent for MyComponent {
    type Config = MyConfig;
    type Error = vm_core::VmError;

    fn init(config: Self::Config) -> Result<Self, Self::Error> {
        Ok(Self { config })
    }

    fn start(&mut self) -> Result<(), Self::Error> {
        // 启动逻辑
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        // 停止逻辑
        Ok(())
    }

    fn status(&self) -> ComponentStatus {
        ComponentStatus::Running
    }

    fn name(&self) -> &str {
        "MyComponent"
    }
}
```

##### `start(&mut self) -> Result<(), Self::Error>`

启动组件。

**返回**:
- 成功返回Ok(())，失败返回错误

##### `stop(&mut self) -> Result<(), Self::Error>`

停止组件。

**返回**:
- 成功返回Ok(())，失败返回错误

##### `status(&self) -> ComponentStatus`

获取组件状态。

**返回**:
- 当前组件状态

##### `name(&self) -> &str`

获取组件名称。

**返回**:
- 组件名称字符串

### Configurable

配置管理trait。

#### 关联类型

##### `type Config`

配置类型。

#### 方法

##### `update_config(&mut self, config: &Self::Config) -> Result<(), VmError>`

更新配置。

**参数**:
- `config`: 新配置

**返回**:
- 成功返回Ok(())，失败返回错误

##### `get_config(&self) -> &Self::Config`

获取当前配置。

**返回**:
- 当前配置的引用

##### `validate_config(config: &Self::Config) -> Result<(), VmError>`

验证配置。

**参数**:
- `config`: 要验证的配置

**返回**:
- 配置有效返回Ok(())，无效返回错误

**示例**:
```rust
use vm_interface::Configurable;

struct MyEngine {
    config: EngineConfig,
}

impl Configurable for MyEngine {
    type Config = EngineConfig;

    fn update_config(&mut self, config: &Self::Config) -> Result<(), VmError> {
        Self::validate_config(config)?;
        self.config = config.clone();
        Ok(())
    }

    fn get_config(&self) -> &Self::Config {
        &self.config
    }

    fn validate_config(config: &Self::Config) -> Result<(), VmError> {
        if config.memory_size == 0 {
            return Err(VmError::Core(vm_core::CoreError::InvalidParameter {
                name: "memory_size".to_string(),
                value: "0".to_string(),
                message: "Memory size must be greater than 0".to_string(),
            }));
        }
        Ok(())
    }
}
```

### Observable

状态观察trait，实现发布-订阅模式。

#### 关联类型

##### `type State`

状态类型。

##### `type Event`

事件类型。

#### 方法

##### `get_state(&self) -> &Self::State`

获取当前状态。

**返回**:
- 当前状态的引用

##### `subscribe(&mut self, callback: StateEventCallback<Self::State, Self::Event>) -> SubscriptionId`

订阅状态变化。

**参数**:
- `callback`: 状态变化回调函数

**返回**:
- 订阅ID

##### `unsubscribe(&mut self, id: SubscriptionId) -> Result<(), VmError>`

取消订阅。

**参数**:
- `id`: 订阅ID

**返回**:
- 成功返回Ok(())，失败返回错误

**示例**:
```rust
use vm_interface::{Observable, StateEventCallback, SubscriptionId};
use vm_core::ExecStats;

struct MyObservable {
    state: ExecStats,
    subscribers: Vec<(SubscriptionId, StateEventCallback<ExecStats, VmEvent>)>,
    next_id: SubscriptionId,
}

impl Observable for MyObservable {
    type State = ExecStats;
    type Event = VmEvent;

    fn get_state(&self) -> &Self::State {
        &self.state
    }

    fn subscribe(&mut self, callback: StateEventCallback<Self::State, Self::Event>) -> SubscriptionId {
        let id = self.next_id;
        self.next_id += 1;
        self.subscribers.push((id, callback));
        id
    }

    fn unsubscribe(&mut self, id: SubscriptionId) -> Result<(), VmError> {
        self.subscribers.retain(|(sub_id, _)| *sub_id != id);
        Ok(())
    }
}
```

## 执行引擎接口

### ExecutionEngine

扩展的执行引擎trait，结合了VmComponent、Configurable和Observable。

#### 继承

```rust
pub trait ExecutionEngine<I>:
    VmComponent +
    Configurable +
    Observable<State = Self::State, Event = Self::Event>
{
    // ...
}
```

#### 关联类型

##### `type State`

执行状态类型。

##### `type Stats`

统计信息类型。

#### 方法

##### `execute<M: MemoryManager>(&mut self, mmu: &mut M, block: &I) -> ExecResult`

执行IR块。

**参数**:
- `mmu`: 内存管理器引用
- `block`: 要执行的IR块

**返回**:
- 执行结果

##### `get_register(&self, index: usize) -> u64`

获取寄存器值。

**参数**:
- `index`: 寄存器索引

**返回**:
- 寄存器值

##### `set_register(&mut self, index: usize, value: u64)`

设置寄存器值。

**参数**:
- `index`: 寄存器索引
- `value`: 要设置的值

##### `get_pc(&self) -> GuestAddr`

获取程序计数器。

**返回**:
- 当前PC值

##### `set_pc(&mut self, pc: GuestAddr)`

设置程序计数器。

**参数**:
- `pc`: 新的PC值

##### `get_execution_state(&self) -> &Self::State`

获取执行状态。

**返回**:
- 执行状态的引用

##### `get_execution_stats(&self) -> &Self::Stats`

获取执行统计。

**返回**:
- 统计信息的引用

##### `reset(&mut self)`

重置执行状态。

##### `execute_async<M: MemoryManager>(&mut self, mmu: &mut M, block: &I) -> impl Future<Output = ExecResult> + Send`

异步执行版本。

**参数**:
- `mmu`: 内存管理器引用
- `block`: 要执行的IR块

**返回**:
- 异步执行结果的Future

### HotCompilationManager

热编译管理trait（用于JIT和Hybrid引擎）。

#### 方法

##### `set_hot_threshold(&mut self, min: u64, max: u64)`

设置热点阈值。

**参数**:
- `min`: 最小执行次数
- `max`: 最大执行次数

##### `get_hot_stats(&self) -> &HotStats`

获取热点统计。

**返回**:
- 热点统计信息引用

##### `clear_hot_cache(&mut self)`

清除热点缓存。

##### `precompile_block(&mut self, address: GuestAddr) -> Result<(), VmError>`

预编译块。

**参数**:
- `address`: 块地址

**返回**:
- 成功返回Ok(())，失败返回错误

### StateSynchronizer

状态同步trait（用于Hybrid引擎）。

#### 方法

##### `sync_state_to<E: ExecutionEngine<IRBlock>>(&mut self, target: &mut E) -> Result<(), VmError>`

从源同步状态到目标。

**参数**:
- `target`: 目标执行引擎

**返回**:
- 成功返回Ok(())，失败返回错误

##### `sync_state_from<E: ExecutionEngine<IRBlock>>(&mut self, source: &E) -> Result<(), VmError>`

从目标同步状态到源。

**参数**:
- `source`: 源执行引擎

**返回**:
- 成功返回Ok(())，失败返回错误

##### `needs_sync(&self) -> bool`

检查状态是否需要同步。

**返回**:
- 需要同步返回true，否则返回false

## 内存管理接口

### MemoryManager

统一的内存管理接口，结合了VmComponent和Configurable。

#### 方法

##### `read_memory(&self, addr: GuestAddr, size: usize) -> Result<Vec<u8>, VmError>`

读取内存。

**参数**:
- `addr`: 起始地址
- `size`: 读取字节数

**返回**:
- 读取的数据

##### `write_memory(&mut self, addr: GuestAddr, data: &[u8]) -> Result<(), VmError>`

写入内存。

**参数**:
- `addr`: 起始地址
- `data`: 要写入的数据

**返回**:
- 成功返回Ok(())，失败返回错误

##### `read_atomic(&self, addr: GuestAddr, size: usize, order: MemoryOrder) -> Result<u64, VmError>`

原子读取。

**参数**:
- `addr`: 地址
- `size`: 读取大小
- `order`: 内存序

**返回**:
- 读取的值

##### `write_atomic(&mut self, addr: GuestAddr, value: u64, size: usize, order: MemoryOrder) -> Result<(), VmError>`

原子写入。

**参数**:
- `addr`: 地址
- `value`: 要写入的值
- `size`: 写入大小
- `order`: 内存序

**返回**:
- 成功返回Ok(())，失败返回错误

##### `compare_exchange(&mut self, addr: GuestAddr, expected: u64, desired: u64, size: usize, success: MemoryOrder, failure: MemoryOrder) -> Result<u64, VmError>`

原子比较交换。

**参数**:
- `addr`: 地址
- `expected`: 期望值
- `desired`: 期望写入的值
- `size`: 操作大小
- `success`: 成功时的内存序
- `failure`: 失败时的内存序

**返回**:
- 实际的旧值

##### `read_memory_async(&self, addr: GuestAddr, size: usize) -> impl Future<Output = Result<Vec<u8>, VmError>> + Send`

异步读取内存。

##### `write_memory_async(&mut self, addr: GuestAddr, data: Vec<u8>) -> impl Future<Output = Result<(), VmError>> + Send`

异步写入内存。

### CacheManager

缓存管理接口。

#### 关联类型

##### `type Key`

缓存键类型。

##### `type Value`

缓存值类型。

#### 方法

##### `get(&self, key: &Self::Key) -> Option<&Self::Value>`

获取缓存项。

##### `set(&mut self, key: Self::Key, value: Self::Value)`

设置缓存项。

##### `remove(&mut self, key: &Self::Key) -> Option<Self::Value>`

删除缓存项。

##### `clear(&mut self)`

清空缓存。

##### `get_stats(&self) -> &CacheStats`

获取缓存统计。

### PageTableManager

页表管理接口。

#### 方法

##### `translate(&self, vaddr: GuestAddr, access_type: vm_core::AccessType) -> Result<vm_core::GuestPhysAddr, VmError>`

地址翻译。

##### `update_entry(&mut self, vaddr: GuestAddr, paddr: vm_core::GuestPhysAddr, flags: PageFlags) -> Result<(), VmError>`

更新页表项。

##### `flush_tlb(&mut self, vaddr: Option<GuestAddr>)`

刷新TLB。

##### `get_page_stats(&self) -> &PageStats`

获取页表统计。

## 设备管理接口

### Device

统一的设备接口。

#### 继承

```rust
pub trait Device:
    VmComponent +
    Configurable +
    Observable
{
    // ...
}
```

#### 关联类型

##### `type IoRegion`

I/O区域类型。

#### 方法

##### `device_id(&self) -> DeviceId`

获取设备ID。

##### `device_type(&self) -> DeviceType`

获取设备类型。

##### `io_regions(&self) -> &[Self::IoRegion]`

获取I/O区域列表。

##### `handle_read(&mut self, offset: u64, size: usize) -> Result<u64, VmError>`

处理I/O读操作。

##### `handle_write(&mut self, offset: u64, value: u64, size: usize) -> Result<(), VmError>`

处理I/O写操作。

##### `handle_interrupt(&mut self, vector: u32) -> Result<(), VmError>`

处理中断。

##### `device_status(&self) -> DeviceStatus`

获取设备状态。

### DeviceManager

设备管理器接口。

#### 关联类型

##### `type Device: Device`

设备类型。

#### 方法

##### `register_device(&mut self, device: Box<Self::Device>) -> Result<DeviceId, VmError>`

注册设备。

##### `unregister_device(&mut self, device_id: DeviceId) -> Result<Box<Self::Device>, VmError>`

注销设备。

##### `find_device(&self, device_id: DeviceId) -> Option<&Self::Device>`

查找设备。

##### `find_device_mut(&mut self, device_id: DeviceId) -> Option<&mut Self::Device>`

查找设备（可变）。

##### `list_devices(&self) -> Vec<&Self::Device>`

列出所有设备。

##### `route_io_read(&mut self, device_id: DeviceId, offset: u64, size: usize) -> Result<u64, VmError>`

路由I/O读操作。

##### `route_io_write(&mut self, device_id: DeviceId, offset: u64, value: u64, size: usize) -> Result<(), VmError>`

路由I/O写操作。

### DeviceBus

设备总线接口。

#### 方法

##### `map_device(&mut self, device_id: DeviceId, base_addr: u64, size: u64) -> Result<(), VmError>`

映射设备到总线地址。

##### `unmap_device(&mut self, device_id: DeviceId) -> Result<(), VmError>`

取消设备映射。

##### `translate_address(&self, addr: u64) -> Option<(DeviceId, u64)>`

地址到设备的翻译。

## 异步接口

### AsyncExecutionContext

异步执行上下文。

#### 方法

##### `runtime(&self) -> &tokio::runtime::Runtime`

获取异步运行时。

##### `scheduler(&self) -> &dyn TaskScheduler`

获取任务调度器。

##### `generate_task_id(&self) -> TaskId`

生成任务ID。

### TaskScheduler

任务调度器接口。

#### 方法

##### `submit_task(&self, task: Box<dyn AsyncTask<Result = TaskResult>>) -> impl Future<Output = TaskId>`

提交任务。

##### `cancel_task(&self, task_id: TaskId) -> impl Future<Output = Result<(), VmError>>`

取消任务。

##### `get_task_status(&self, task_id: TaskId) -> impl Future<Output = TaskStatus>`

获取任务状态。

##### `wait_task(&self, task_id: TaskId) -> impl Future<Output = Result<TaskResult, VmError>>`

等待任务完成。

### AsyncTask

异步任务trait。

#### 关联类型

##### `type Result`

任务结果类型。

#### 方法

##### `execute(&mut self) -> impl Future<Output = Result<Self::Result, VmError>>`

执行任务。

##### `description(&self) -> &str`

获取任务描述。

## 类型定义

### ComponentStatus

组件状态枚举。

#### 变体

- `Uninitialized` - 未初始化
- `Initialized` - 已初始化
- `Starting` - 正在启动
- `Running` - 运行中
- `Stopping` - 正在停止
- `Stopped` - 已停止
- `Error` - 错误状态

### DeviceType

设备类型枚举。

#### 变体

- `Block` - 块设备
- `Network` - 网络设备
- `GPU` - GPU设备
- `Input` - 输入设备
- `Audio` - 音频设备
- `Serial` - 串口设备
- `Custom(u32)` - 自定义设备

### DeviceStatus

设备状态枚举。

#### 变体

- `Uninitialized` - 未初始化
- `Initialized` - 已初始化
- `Running` - 运行中
- `Stopped` - 已停止
- `Error(String)` - 错误状态

### MemoryOrder

内存序类型。

#### 变体

- `Relaxed` - 松散序
- `Acquire` - 获取序
- `Release` - 释放序
- `AcqRel` - 获取-释放序
- `SeqCst` - 顺序一致性序

### VmEvent

VM事件枚举。

#### 变体

- `ComponentStarted(String)` - 组件启动
- `ComponentStopped(String)` - 组件停止
- `ExecutionCompleted(ExecStats)` - 执行完成
- `MemoryAccess { addr, size, is_write }` - 内存访问
- `DeviceInterrupt(DeviceId)` - 设备中断
- `ErrorOccurred(VmError)` - 发生错误

### TaskStatus

任务状态枚举。

#### 变体

- `Pending` - 待处理
- `Running` - 运行中
- `Completed` - 已完成
- `Failed` - 失败

### TaskResult

任务结果枚举。

#### 变体

- `Success` - 成功
- `Failure(VmError)` - 失败

### 统计类型

- `CacheStats` - 缓存统计（hits, misses, evictions）
- `PageStats` - 页表统计（translations, faults, flushes）
- `HotStats` - 热点统计（total_executions, hot_blocks, compiled_blocks）

## 使用示例

### 实现自定义组件

```rust
use vm_interface::{VmComponent, ComponentStatus, Configurable, Observable};
use vm_core::{VmError, VmConfig};

struct MyVmComponent {
    name: String,
    status: ComponentStatus,
    config: VmConfig,
}

impl VmComponent for MyVmComponent {
    type Config = VmConfig;
    type Error = VmError;

    fn init(config: Self::Config) -> Result<Self, Self::Error> {
        Ok(Self {
            name: "MyComponent".to_string(),
            status: ComponentStatus::Initialized,
            config,
        })
    }

    fn start(&mut self) -> Result<(), Self::Error> {
        self.status = ComponentStatus::Running;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.status = ComponentStatus::Stopped;
        Ok(())
    }

    fn status(&self) -> ComponentStatus {
        self.status
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl Configurable for MyVmComponent {
    type Config = VmConfig;

    fn update_config(&mut self, config: &Self::Config) -> Result<(), VmError> {
        self.config = config.clone();
        Ok(())
    }

    fn get_config(&self) -> &Self::Config {
        &self.config
    }

    fn validate_config(config: &Self::Config) -> Result<(), VmError> {
        if config.memory_size == 0 {
            return Err(VmError::Core(vm_core::CoreError::InvalidParameter {
                name: "memory_size".to_string(),
                value: "0".to_string(),
                message: "Memory size cannot be zero".to_string(),
            }));
        }
        Ok(())
    }
}
```

### 使用Observable订阅事件

```rust
use vm_interface::{Observable, StateEventCallback, VmEvent};
use vm_core::ExecStats;

let mut component = MyObservable::new();

// 订阅状态变化
let subscription_id = component.subscribe(Box::new(
    |state: &ExecStats, event: &VmEvent| {
        match event {
            VmEvent::ExecutionCompleted(stats) => {
                println!("Execution completed: {} instructions", stats.executed_insns);
            }
            _ => {}
        }
    }
));

// 取消订阅
component.unsubscribe(subscription_id)?;
```

### 使用设备管理器

```rust
use vm_interface::{DeviceManager, Device, DeviceId};
use vm_core::VmError;

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

    fn unregister_device(&mut self, device_id: DeviceId) -> Result<Box<Self::Device>, VmError> {
        if device_id < self.devices.len() as DeviceId {
            Ok(self.devices.remove(device_id as usize))
        } else {
            Err(VmError::Core(vm_core::CoreError::InvalidParameter {
                name: "device_id".to_string(),
                value: device_id.to_string(),
                message: "Device not found".to_string(),
            }))
        }
    }

    fn find_device(&self, device_id: DeviceId) -> Option<&Self::Device> {
        self.devices.get(device_id as usize).map(|d| d.as_ref())
    }

    fn find_device_mut(&mut self, device_id: DeviceId) -> Option<&mut Self::Device> {
        self.devices.get_mut(device_id as usize).map(|d| d.as_mut())
    }

    fn list_devices(&self) -> Vec<&Self::Device> {
        self.devices.iter().map(|d| d.as_ref()).collect()
    }

    fn route_io_read(&mut self, device_id: DeviceId, offset: u64, size: usize) -> Result<u64, VmError> {
        if let Some(device) = self.find_device_mut(device_id) {
            device.handle_read(offset, size)
        } else {
            Err(VmError::Core(vm_core::CoreError::InvalidParameter {
                name: "device_id".to_string(),
                value: device_id.to_string(),
                message: "Device not found".to_string(),
            }))
        }
    }

    fn route_io_write(&mut self, device_id: DeviceId, offset: u64, value: u64, size: usize) -> Result<(), VmError> {
        if let Some(device) = self.find_device_mut(device_id) {
            device.handle_write(offset, value, size)
        } else {
            Err(VmError::Core(vm_core::CoreError::InvalidParameter {
                name: "device_id".to_string(),
                value: device_id.to_string(),
                message: "Device not found".to_string(),
            }))
        }
    }
}
```

## 注意事项

### Trait组合

vm-interface使用trait组合提供灵活的接口设计：
- `ExecutionEngine` = `VmComponent` + `Configurable` + `Observable`
- `Device` = `VmComponent` + `Configurable` + `Observable`
- `MemoryManager` = `VmComponent` + `Configurable`

### 异步支持

异步trait方法返回`impl Future`，需要在async上下文中调用：

```rust
// 异步读取内存
let data = memory_manager.read_memory_async(addr, size).await?;
```

### 错误处理

所有接口统一使用`VmError`作为错误类型，确保错误处理的一致性。

### 生命周期管理

组件的生命周期：
1. `init()` - 初始化
2. `start()` - 启动
3. `stop()` - 停止

每次状态转换都应该更新`status()`返回的状态。

## 相关API

- [VmCore API](./VmCore.md) - 核心类型定义
- [VmMemory API](./VmMemory.md) - 内存管理实现
- [Devices API](./Devices.md) - 设备实现
