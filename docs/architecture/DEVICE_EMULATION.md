# 设备仿真架构

## 目录

- [设备仿真概述](#设备仿真概述)
- [设备框架](#设备框架)
- [VirtIO设备](#virtio设备)
- [中断系统](#中断系统)
- [DMA支持](#dma支持)
- [设备热插拔](#设备热插拔)

---

## 设备仿真概述

### 职责

设备仿真子系统负责模拟Guest可访问的各种硬件设备，包括块设备、网络设备、串口、GPU等。

### 架构设计

```
┌─────────────────────────────────────────────────────────┐
│                   vm-device                             │
│                                                          │
│  ┌──────────────────────────────────────────────────┐  │
│  │              DeviceManager                        │  │
│  │  - register_device()                              │  │
│  │  - unregister_device()                            │  │
│  │  - route_io()                                     │  │
│  └──────────────────────────────────────────────────┘  │
│                          ↓                              │
│  ┌──────────────────────────────────────────────────┐  │
│  │              DeviceBus                            │  │
│  │  - map_device()                                  │  │
│  │  - unmap_device()                                │  │
│  │  - translate_address()                           │  │
│  └──────────────────────────────────────────────────┘  │
│                          ↓                              │
│  ┌───────────┬───────────┬───────────┬─────────────┐  │
│  │ VirtioBlock│ VirtioNet │ VirtioGPU│  UART       │  │
│  └───────────┴───────────┴───────────┴─────────────┘  │
└─────────────────────────────────────────────────────────┘
```

---

## 设备框架

### 设备接口

```rust
pub trait Device: MmioDevice {
    /// 设备ID
    fn device_id(&self) -> DeviceId;

    /// 设备类型
    fn device_type(&self) -> DeviceType;

    /// 重置设备
    fn reset(&mut self);

    /// 处理中断
    fn handle_interrupt(&mut self, vector: u32)
        -> Result<(), VmError>;
}

#[derive(Debug, Clone, Copy)]
pub enum DeviceType {
    Block,
    Network,
    GPU,
    Input,
    Audio,
    Serial,
    Custom(u32),
}
```

### 设备管理器

```rust
pub struct DeviceManager {
    devices: HashMap<DeviceId, Arc<RwLock<Box<dyn Device>>>>,
    device_bus: DeviceBus,
    next_device_id: AtomicU64,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            device_bus: DeviceBus::new(),
            next_device_id: AtomicU64::new(1),
        }
    }

    /// 注册设备
    pub fn register_device(&mut self, device: Box<dyn Device>)
        -> Result<DeviceId, VmError>
    {
        let device_id = self.next_device_id.fetch_add(1, Ordering::Relaxed);
        let device_addr = Arc::new(RwLock::new(device));

        self.devices.insert(device_id, device_addr.clone());
        
        Ok(device_id)
    }

    /// 注销设备
    pub fn unregister_device(&mut self, device_id: DeviceId)
        -> Result<Box<dyn Device>, VmError>
    {
        self.devices.remove(&device_id)
            .map(|arc| {
                // 尝试获取唯一所有权（如果没有其他引用）
                Arc::try_unwrap(arc)
                    .ok_or_else(|| VmError::Device("Device still in use".into()))
                    .map(|rw| RwLock::into_inner(rw).unwrap())
            })
            .transpose()
            .flatten()
    }

    /// 查找设备
    pub fn find_device(&self, device_id: DeviceId)
        -> Option<&Arc<RwLock<Box<dyn Device>>>>
    {
        self.devices.get(&device_id)
    }

    /// 路由I/O读
    pub fn route_io_read(&mut self, device_id: DeviceId,
        offset: u64, size: usize) -> Result<u64, VmError>
    {
        let device = self.devices.get(&device_id)
            .ok_or_else(|| VmError::Device("Device not found".into()))?;

        let guard = device.read();
        guard.handle_read(offset, size)
    }

    /// 路由I/O写
    pub fn route_io_write(&mut self, device_id: DeviceId,
        offset: u64, value: u64, size: usize) -> Result<(), VmError>
    {
        let device = self.devices.get(&device_id)
            .ok_or_else(|| VmError::Device("Device not found".into()))?;

        let mut guard = device.write();
        guard.handle_write(offset, value, size)
    }
}
```

### 设备总线

```rust
pub struct DeviceBus {
    mappings: Vec<DeviceMapping>,
}

struct DeviceMapping {
    device_id: DeviceId,
    base_addr: u64,
    size: u64,
}

impl DeviceBus {
    pub fn new() -> Self {
        Self {
            mappings: Vec::new(),
        }
    }

    /// 映射设备到总线地址
    pub fn map_device(&mut self, device_id: DeviceId,
        base_addr: u64, size: u64) -> Result<(), VmError>
    {
        // 检查地址冲突
        if self.mappings.iter().any(|m| {
            !(base_addr + size <= m.base_addr || base_addr >= m.base_addr + m.size)
        }) {
            return Err(VmError::Device("Address range conflict".into()));
        }

        self.mappings.push(DeviceMapping {
            device_id,
            base_addr,
            size,
        });

        Ok(())
    }

    /// 地址到设备翻译
    pub fn translate_address(&self, addr: u64)
        -> Option<(DeviceId, u64)>
    {
        self.mappings.iter()
            .find(|m| addr >= m.base_addr && addr < m.base_addr + m.size)
            .map(|m| (m.device_id, addr - m.base_addr))
    }
}
```

---

## VirtIO设备

### VirtIO规范

VirtIO是虚拟化设备的标准接口，定义了高效的数据传输机制。

#### VirtQueue结构

```
VirtQueue:
┌──────────────────────────────────────────────────────┐
│  Descriptor Table  (512 descriptors)                 │
│  ┌──────────────┬──────────────┬──────────────┐     │
│  │ addr (64-bit)│ len (32-bit) │ flags (16-bit)│     │
│  │ next (16-bit)│              │              │     │
│  └──────────────┴──────────────┴──────────────┘     │
└──────────────────────────────────────────────────────┘
┌──────────────────────────────────────────────────────┐
│  Available Ring (guest → device)                     │
│  ┌──────────────┬──────────────┬──────────────┐     │
│  │ flags (16-bit)│ idx (16-bit) │ ring[]       │     │
│  └──────────────┴──────────────┴──────────────┘     │
└──────────────────────────────────────────────────────┘
┌──────────────────────────────────────────────────────┐
│  Used Ring (device → guest)                          │
│  ┌──────────────┬──────────────┬──────────────┐     │
│  │ flags (16-bit)│ idx (16-bit) │ ring[]       │     │
│  │ elem[]        │              │              │     │
│  └──────────────┴──────────────┴──────────────┘     │
└──────────────────────────────────────────────────────┘
```

### VirtIO块设备

```rust
pub struct VirtioBlockDevice {
    config: VirtioBlockConfig,
    queue: VirtQueue,
    backend: Box<dyn BlockBackend>,
    status: DeviceStatus,
    features: u64,
    driver_features: u64,
}

#[derive(Debug, Clone)]
pub struct VirtioBlockConfig {
    pub capacity: u64,      // 扇区数
    pub size_max: u32,      // 最大段大小
    pub seg_max: u32,       // 最大段数
    pub geometry: Geometry,
    pub blk_size: u32,      // 块大小
}

#[derive(Debug, Clone)]
pub struct Geometry {
    pub cylinders: u16,
    pub heads: u8,
    pub sectors: u8,
}

pub enum BlockRequest {
    Read { sector: u64, data: Vec<u8> },
    Write { sector: u64, data: Vec<u8> },
    Flush,
}

pub enum BlockResult {
    Ok,
    IoError,
    Unsupported,
}

impl MmioDevice for VirtioBlockDevice {
    fn read(&self, offset: u64, size: u8) -> VmResult<u64> {
        match offset {
            0x000 => Ok(self.status.bits() as u64),
            0x004 => Ok(self.queue.get_driver_select()),
            0x00C => Ok(self.queue.get_driver()),
            0x010 => Ok(self.queue.get_device()),
            0x014 => Ok(0),  // TODO: ISR status
            0x020 => {
                // 设备特定配置
                match size {
                    1 => Ok(self.config.blk_size as u64),
                    2 => Ok(self.config.geometry.sectors as u64),
                    4 => Ok(self.config.seg_max as u64),
                    8 => Ok(self.config.capacity),
                    _ => Ok(0),
                }
            }
            0x034 => Ok(self.features),
            _ => Ok(0),
        }
    }

    fn write(&mut self, offset: u64, value: u64, size: u8)
        -> VmResult<()>
    {
        match offset {
            0x000 => {
                self.status = DeviceStatus::from_bits_truncate(value as u32);
                if self.status.contains(DeviceStatus::DRIVER_OK) {
                    self.activate()?;
                }
            }
            0x004 => {
                self.queue.set_driver_select(value as u32);
            }
            0x008 => {
                self.queue.set_queue_size(value as u32);
            }
            0x00C => {
                self.queue.set_driver(value as u16);
            }
            0x010 => {
                self.queue.set_device(value as u16);
                if value != 0 {
                    self.queue.notify();
                }
            }
            0x020 => {
                // 配置空间（只读）
            }
            0x034 => {
                self.driver_features = value;
            }
            _ => {}
        }
        Ok(())
    }
}
```

### VirtQueue实现

```rust
pub struct VirtQueue {
    /// 描述符表
    descriptors: Vec<VirtqDesc>,
    /// 可用环
    avail_ring: VirtqAvail,
    /// 已用环
    used_ring: VirtqUsed,
    /// 队列大小
    queue_size: u16,
    /// 当前驱动索引
    driver_idx: u16,
    /// 当前设备索引
    device_idx: u16,
}

#[repr(C)]
pub struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
pub struct VirtqAvail {
    flags: u16,
    idx: u16,
    ring: [u16; 1],  // Flexible array
    used_event: u16,
}

#[repr(C)]
pub struct VirtqUsed {
    flags: u16,
    idx: u16,
    ring: [VirtqUsedElem; 1],  // Flexible array
    avail_event: u16,
}

#[repr(C)]
pub struct VirtqUsedElem {
    id: u32,
    len: u32,
}

impl VirtQueue {
    /// 处理队列请求
    pub fn process_requests<F>(&mut self, mut handler: F)
        -> Result<(), VmError>
    where
        F: FnMut(&[VirtqDesc]) -> Result<u32, VmError>,
    {
        while self.driver_idx != self.avail_ring.idx as u16 {
            // 获取下一个可用描述符
            let desc_index = self.avail_ring.ring[(self.driver_idx % self.queue_size) as usize] as usize;
            
            // 获取描述符链
            let mut desc_chain = Vec::new();
            let mut current_desc = &self.descriptors[desc_index];
            
            loop {
                desc_chain.push(current_desc.clone());
                
                if current_desc.flags & 1 == 0 {
                    // NEXT flag未设置，链结束
                    break;
                }
                
                let next_index = current_desc.next as usize;
                current_desc = &self.descriptors[next_index];
            }

            // 调用处理器
            let written = handler(&desc_chain)?;

            // 更新已用环
            let used_elem = VirtqUsedElem {
                id: desc_index as u32,
                len: written,
            };

            let used_idx = self.used_ring.idx as u16 % self.queue_size;
            self.used_ring.ring[used_idx as usize] = used_elem;
            
            self.driver_idx = self.driver_idx.wrapping_add(1);
            self.used_ring.idx = self.used_ring.idx.wrapping_add(1);
        }

        Ok(())
    }
}
```

---

## 中断系统

### 中断控制器

```rust
pub struct InterruptController {
    pending: u64,        // 待处理中断
    enabled: u64,        // 已使能中断
    serving: u64,        // 正在服务的中断
}

impl InterruptController {
    pub fn new() -> Self {
        Self {
            pending: 0,
            enabled: 0,
            serving: 0,
        }
    }

    /// 触发中断
    pub fn raise(&mut self, vector: u8) {
        self.pending |= 1 << vector;
    }

    /// 使能中断
    pub fn enable(&mut self, vector: u8) {
        self.enabled |= 1 << vector;
    }

    /// 禁用中断
    pub fn disable(&mut self, vector: u8) {
        self.enabled &= !(1 << vector);
    }

    /// 获取最高优先级待处理中断
    pub fn get_pending(&self) -> Option<u8> {
        let pending_enabled = self.pending & self.enabled & !self.serving;
        if pending_enabled == 0 {
            return None;
        }

        // 返回最低位的待处理中断
        Some(pending_enabled.trailing_zeros() as u8)
    }

    /// 完成中断服务
    pub fn complete(&mut self, vector: u8) {
        self.pending &= !(1 << vector);
        self.serving &= !(1 << vector);
    }
}
```

### PLIC (Platform-Level Interrupt Controller)

```rust
pub struct PLIC {
    /// 优先级寄存器 (最多1024个中断源)
    priorities: [u32; 1024],
    /// 待处理中断位图
    pending: [u32; 32],
    /// 使能寄存器 (每个上下文一份)
    enables: Vec<[u32; 32]>,
    /// 阈值和完成声明 (每个上下文一份)
    contexts: Vec<PLICContext>,
}

struct PLICContext {
    threshold: u32,
    complete: u32,
}

impl PLIC {
    pub fn new(num_contexts: usize) -> Self {
        Self {
            priorities: [0; 1024],
            pending: [0; 32],
            enables: vec![[0; 32]; num_contexts],
            contexts: vec![PLICContext {
                threshold: 0,
                complete: 0,
            }; num_contexts],
        }
    }

    /// 触发中断
    pub fn raise(&mut self, source: u32) {
        let word = source / 32;
        let bit = source % 32;
        self.pending[word as usize] |= 1 << bit;
    }

    /// 获取最高优先级待处理中断
    pub fn get_pending(&self, context: usize) -> Option<u32> {
        let ctx = &self.contexts[context];

        // 找出所有使能且高于阈值的待处理中断
        let mut highest_source = None;
        let mut highest_priority = ctx.threshold;

        for (word_idx, &pending_word) in self.pending.iter().enumerate() {
            let enabled_word = self.enables[context][word_idx];
            let candidates = pending_word & enabled_word;

            if candidates == 0 {
                continue;
            }

            for bit_idx in 0..32 {
                if candidates & (1 << bit_idx) != 0 {
                    let source = (word_idx * 32 + bit_idx) as u32;
                    let priority = self.priorities[source as usize];

                    if priority > highest_priority {
                        highest_priority = priority;
                        highest_source = Some(source);
                    }
                }
            }
        }

        highest_source
    }

    /// 完成中断
    pub fn complete(&mut self, context: usize, source: u32) {
        let word = source / 32;
        let bit = source % 32;
        self.pending[word as usize] &= !(1 << bit);
        self.contexts[context].complete = source;
    }
}
```

---

## DMA支持

### DMA引擎

```rust
pub struct DMAEngine {
    channels: Vec<DMAChannel>,
    address_space: Arc<PhysicalMemory>,
}

pub struct DMAChannel {
    source: u64,
    destination: u64,
    length: u64,
    control: DMAControl,
}

bitflags! {
    pub struct DMAControl: u32 {
        const ENABLE = 1 << 0;
        const DIRECTION = 1 << 1;  // 0: mem→mem, 1: mem→dev
        const INTERRUPT = 1 << 2;
        const AUTO_RELOAD = 1 << 3;
    }
}

impl DMAEngine {
    /// 执行DMA传输
    pub fn execute_transfer(&mut self, channel_id: usize)
        -> Result<(), VmError>
    {
        let channel = &mut self.channels[channel_id];

        if !channel.control.contains(DMAControl::ENABLE) {
            return Ok(());
        }

        let mut offset = 0;
        while offset < channel.length {
            let chunk_size = (channel.length - offset).min(4096) as usize;

            // 读取源数据
            let src_addr = (channel.source + offset) as usize;
            let mut data = vec![0u8; chunk_size];
            self.address_space.read_buf(src_addr, &mut data)?;

            // 写入目标
            let dst_addr = (channel.destination + offset) as usize;
            self.address_space.write_buf(dst_addr, &data)?;

            offset += chunk_size as u64;
        }

        // 如果使能了中断，通知设备
        if channel.control.contains(DMAControl::INTERRUPT) {
            self.notify_complete(channel_id)?;
        }

        // 自动重载
        if channel.control.contains(DMAControl::AUTO_RELOAD) {
            // 保持传输使能
        } else {
            channel.control.remove(DMAControl::ENABLE);
        }

        Ok(())
    }
}
```

---

## 设备热插拔

### 热插拔管理器

```rust
pub struct HotplugManager {
    slots: Vec<HotplugSlot>,
    events: VecDeque<HotplugEvent>,
}

pub struct HotplugSlot {
    slot_id: u32,
    device: Option<Box<dyn Device>>,
    state: SlotState,
}

pub enum SlotState {
    Empty,
    Present,
    PoweringOn,
    PoweredOn,
    PoweringOff,
}

pub enum HotplugEvent {
    DeviceAdded { slot_id: u32 },
    DeviceRemoved { slot_id: u32 },
    DevicePowerOnFailed { slot_id: u32, error: String },
}

impl HotplugManager {
    /// 添加设备
    pub fn add_device(&mut self, slot_id: u32, device: Box<dyn Device>)
        -> Result<(), VmError>
    {
        let slot = &mut self.slots[slot_id as usize];
        
        if slot.device.is_some() {
            return Err(VmError::Device("Slot already occupied".into()));
        }

        slot.device = Some(device);
        slot.state = SlotState::Present;

        self.events.push_back(HotplugEvent::DeviceAdded { slot_id });

        Ok(())
    }

    /// 移除设备
    pub fn remove_device(&mut self, slot_id: u32)
        -> Result<Box<dyn Device>, VmError>
    {
        let slot = &mut self.slots[slot_id as usize];

        slot.device.take()
            .ok_or_else(|| VmError::Device("Slot is empty".into()))
            .map(|device| {
                slot.state = SlotState::Empty;
                self.events.push_back(HotplugEvent::DeviceRemoved { slot_id });
                device
            })
    }

    /// 获取待处理事件
    pub fn poll_events(&mut self) -> Vec<HotplugEvent> {
        std::mem::take(&mut self.events).into()
    }
}
```

---

**文档版本**: 1.0
**最后更新**: 2025-12-31
**作者**: VM开发团队
