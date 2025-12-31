# vm-mem API 参考

## 概述

`vm-mem` 提供虚拟机的内存管理实现，包括物理内存后端、软件MMU、TLB缓存、页表遍历等功能。该模块是VM内存子系统的核心实现。

## 主要功能

- **物理内存管理**: PhysicalMemory 提供分片RwLock实现
- **软件MMU**: SoftMmu 支持多种分页模式（Bare/Sv39/Sv48）
- **TLB优化**: ITLB和DTLB分离，提高翻译性能
- **MMIO支持**: 内存映射I/O设备管理
- **页表遍历**: RISC-V SV39/SV48页表遍历器
- **原子操作**: LR/SC指令对支持
- **大页支持**: 可选2MB huge pages

## 主要类型

### PhysicalMemory

物理内存后端，实现Guest物理内存的存储。使用分片RwLock提高并发性能。

#### 字段

##### `shards: Vec<RwLock<Vec<u8>>>`

物理内存分片，将内存分成16个分片，每个分片有独立的RwLock。

##### `shard_size: usize`

分片大小。

##### `total_size: usize`

总内存大小。

##### `mmio_regions: RwLock<Vec<MmioRegion>>`

MMIO设备区域。

##### `reservations: RwLock<Vec<(GuestPhysAddr, u64, u8)>>`

全局保留地址集合，用于LR/SC指令。

#### 方法

##### `new(size: usize, use_hugepages: bool) -> Self`

创建新的物理内存。

**参数**:
- `size`: 内存大小（字节）
- `use_hugepages`: 是否使用2MB大页

**返回**:
- 物理内存实例

**示例**:
```rust
use vm_mem::PhysicalMemory;

// 创建128MB物理内存
let mem = PhysicalMemory::new(128 * 1024 * 1024, false);

// 创建1GB物理内存，使用大页
let mem_huge = PhysicalMemory::new(1024 * 1024 * 1024, true);
```

##### `read_u8(&self, addr: usize) -> Result<u8, VmError>`

读取8位值。

**参数**:
- `addr`: 物理地址

**返回**:
- 读取的值

##### `read_u16(&self, addr: usize) -> Result<u16, VmError>`

读取16位值（小端序）。

**参数**:
- `addr`: 物理地址

**返回**:
- 读取的值

##### `read_u32(&self, addr: usize) -> Result<u32, VmError>`

读取32位值（小端序）。

**参数**:
- `addr`: 物理地址

**返回**:
- 读取的值

##### `read_u64(&self, addr: usize) -> Result<u64, VmError>`

读取64位值（小端序）。

**参数**:
- `addr`: 物理地址

**返回**:
- 读取的值

##### `write_u8(&self, addr: usize, val: u8) -> Result<(), VmError>`

写入8位值。

**参数**:
- `addr`: 物理地址
- `val`: 要写入的值

##### `write_u16(&self, addr: usize, val: u16) -> Result<(), VmError>`

写入16位值（小端序）。

**参数**:
- `addr`: 物理地址
- `val`: 要写入的值

##### `write_u32(&self, addr: usize, val: u32) -> Result<(), VmError>`

写入32位值（小端序）。

**参数**:
- `addr`: 物理地址
- `val`: 要写入的值

##### `write_u64(&self, addr: usize, val: u64) -> Result<(), VmError>`

写入64位值（小端序）。

**参数**:
- `addr`: 物理地址
- `val`: 要写入的值

##### `read_buf(&self, addr: usize, buf: &mut [u8]) -> Result<(), VmError>`

批量读取字节。

**参数**:
- `addr`: 起始物理地址
- `buf`: 目标缓冲区

**返回**:
- 成功返回Ok(())，失败返回错误

**性能**:
- 自动处理跨分片读取
- 适合大量数据传输

##### `write_buf(&self, addr: usize, buf: &[u8]) -> Result<(), VmError>`

批量写入字节。

**参数**:
- `addr`: 起始物理地址
- `buf`: 源数据

**返回**:
- 成功返回Ok(())，失败返回错误

##### `size(&self) -> usize`

获取物理内存总大小。

**返回**:
- 内存大小（字节）

##### `dump(&self) -> Vec<u8>`

导出内存数据。

**返回**:
- 包含所有内存内容的向量

##### `restore(&self, data: &[u8]) -> Result<(), VmError>`

恢复内存数据。

**参数**:
- `data`: 要恢复的数据

**返回**:
- 成功返回Ok(())，失败返回错误

##### `reserve(&self, pa: GuestPhysAddr, owner: u64, size: u8)`

保留地址（用于LR指令）。

**参数**:
- `pa`: 物理地址
- `owner`: 所有者ID
- `size`: 保留大小

##### `invalidate(&self, pa: GuestPhysAddr, size: u8)`

使保留失效。

**参数**:
- `pa`: 物理地址
- `size`: 大小

##### `store_conditional_ram(&self, pa: GuestPhysAddr, val: u64, size: u8, owner: u64) -> Result<bool, VmError>`

条件存储（SC指令）。

**参数**:
- `pa`: 物理地址
- `val`: 要写入的值
- `size`: 写入大小
- `owner`: 所有者ID

**返回**:
- 成功写入返回true，失败返回false

### SoftMmu

软件MMU实现，支持虚拟地址到物理地址的翻译。

#### 字段

##### `phys_mem: Arc<PhysicalMemory>`

物理内存后端。

##### `itlb: Tlb`

指令TLB（64条目）。

##### `dtlb: Tlb`

数据TLB（128条目）。

##### `paging_mode: PagingMode`

当前分页模式。

##### `page_table_base: GuestPhysAddr`

页表基址。

##### `asid: u16`

地址空间ID。

##### `tlb_hits: u64`

TLB命中次数。

##### `tlb_misses: u64`

TLB未命中次数。

#### 方法

##### `new(size: usize, use_hugepages: bool) -> Self`

创建新的MMU实例。

**参数**:
- `size`: 物理内存大小（字节）
- `use_hugepages`: 是否使用大页

**返回**:
- MMU实例

**示例**:
```rust
use vm_mem::SoftMmu;

// 创建128MB内存的MMU
let mut mmu = SoftMmu::new(128 * 1024 * 1024, false);

// Bare模式：恒等映射
use vm_core::{GuestAddr, AccessType};
let phys = mmu.translate(GuestAddr(0x1000), AccessType::Read).unwrap();
```

##### `new_default() -> Self`

创建默认MMU（64KB内存）。

**返回**:
- MMU实例

##### `set_paging_mode(&mut self, mode: PagingMode)`

设置分页模式。

**参数**:
- `mode`: 新的分页模式

**注意**:
- 切换模式会清空所有TLB条目
- Bare模式：虚拟地址直接映射到物理地址
- Sv39/Sv48模式：需要页表翻译

##### `set_satp(&mut self, satp: u64)`

设置RISC-V SATP寄存器。

**参数**:
- `satp`: SATP寄存器值

**SATP格式**（RISC-V Sv39）:
```
| 63..60 | 59..44 | 43..0  |
| MODE   | ASID   | PPN    |
```
- MODE: 8=Sv39, 9=Sv48
- ASID: 地址空间ID
- PPN: 页表基址的物理页号

**示例**:
```rust
// 设置Sv39模式，ASID=0，页表在0x8000
mmu.set_satp((8u64 << 60) | (0u64 << 44) | 0x8);
```

##### `set_strict_align(&mut self, enable: bool)`

启用或禁用严格对齐检查。

**参数**:
- `enable`: true启用严格对齐，false禁用

**对齐要求**:
- 1字节：任意地址
- 2字节：2字节对齐
- 4字节：4字节对齐
- 8字节：8字节对齐

##### `translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestPhysAddr, VmError>`

虚拟地址到物理地址的翻译。

**参数**:
- `va`: 虚拟地址
- `access`: 访问类型

**返回**:
- 翻译后的物理地址

**性能**:
- TLB命中: ~5-10ns
- TLB未命中: ~50-100ns（需要页表遍历）

**错误**:
- 页面缺失
- 权限违规
- 对齐错误

##### `flush_tlb(&mut self)`

刷新所有TLB条目。

##### `flush_tlb_asid(&mut self, asid: u16)`

刷新特定ASID的TLB条目。

**参数**:
- `asid`: 地址空间ID

##### `flush_tlb_page(&mut self, va: GuestAddr)`

刷新特定页的TLB条目。

**参数**:
- `va`: 虚拟地址

##### `tlb_stats(&self) -> (u64, u64)`

获取TLB统计信息。

**返回**:
- (命中次数, 未命中次数)

**示例**:
```rust
let (hits, misses) = mmu.tlb_stats();
let hit_rate = hits as f64 / (hits + misses) as f64;
println!("TLB hit rate: {:.2}%", hit_rate * 100.0);
```

##### `memory_size(&self) -> usize`

获取物理内存大小。

**返回**:
- 内存大小（字节）

##### `guest_slice(&self, pa: u64, len: usize) -> Option<Vec<u8>>`

从Guest物理地址读取字节切片。

**参数**:
- `pa`: Guest物理地址
- `len`: 要读取的字节数

**返回**:
- 成功返回字节向量，失败返回None

##### `resize_tlbs(&mut self, itlb_size: usize, dtlb_size: usize)`

调整TLB大小。

**参数**:
- `itlb_size`: 指令TLB大小
- `dtlb_size`: 数据TLB大小

**注意**:
- 会清除所有TLB条目和统计

##### `tlb_capacity(&self) -> (usize, usize)`

获取TLB容量。

**返回**:
- (ITLB大小, DTLB大小)

### PagingMode

分页模式枚举。

#### 变体

##### `Bare`

无分页（恒等映射）。虚拟地址直接映射到物理地址。

##### `Sv39`

RISC-V SV39（3级页表，39位虚拟地址）。支持512TB虚拟地址空间。

##### `Sv48`

RISC-V SV48（4级页表，48位虚拟地址）。支持256TB虚拟地址空间。

##### `Arm64`

ARM64四级页表。

##### `X86_64`

x86_64四级页表。

### PageTableBuilder

页表构建器，用于测试和初始化。

#### 方法

##### `new(start_addr: GuestPhysAddr) -> Self`

创建新的页表构建器。

**参数**:
- `start_addr`: 起始地址

##### `alloc_page(&mut self) -> GuestPhysAddr`

分配一个页表页。

**返回**:
- 分配的页地址

##### `map_page_sv39(&mut self, mmu: &mut SoftMmu, va: GuestAddr, pa: GuestPhysAddr, flags: u64, root: GuestPhysAddr) -> Result<(), VmError>`

创建SV39页表映射（4KB页）。

**参数**:
- `mmu`: MMU引用
- `va`: 虚拟地址
- `pa`: 物理地址
- `flags`: 页标志（R/W/X/U/G/A/D）
- `root`: 根页表地址

**返回**:
- 成功返回Ok(())，失败返回错误

**示例**:
```rust
use vm_mem::{SoftMmu, PageTableBuilder};
use vm_core::{GuestAddr, GuestPhysAddr};

let mut mmu = SoftMmu::new(16 * 1024 * 1024, false);
let mut builder = PageTableBuilder::new(GuestPhysAddr(0x10000));

// 映射虚拟地址 0x1000 -> 物理地址 0x2000
let va = GuestAddr(0x1000);
let pa = GuestPhysAddr(0x2000);
let flags = 0xF; // R|W|X|U
let root = GuestPhysAddr(0x8000);

builder.map_page_sv39(&mut mmu, va, pa, flags, root)?;
```

## Trait实现

### MemoryAccess for PhysicalMemory

`PhysicalMemory`实现了`vm_core::MemoryAccess` trait。

#### 方法

##### `read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError>`

读取内存。

##### `write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError>`

写入内存。

##### `fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError>`

获取指令（4字节）。

##### `read_bulk(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError>`

批量读取。

##### `write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError>`

批量写入。

### MemoryAccess for SoftMmu

`SoftMmu`实现了`vm_core::MemoryAccess` trait，提供虚拟地址访问。

#### 额外方法

##### `load_reserved(&mut self, pa: GuestAddr, size: u8) -> Result<u64, VmError>`

加载保留（LR指令）。

##### `store_conditional(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<bool, VmError>`

条件存储（SC指令）。

**返回**:
- 成功写入返回true，失败返回false

##### `invalidate_reservation(&mut self, pa: GuestAddr, size: u8)`

使保留失效。

### AddressTranslator for SoftMmu

`SoftMmu`实现了`vm_core::AddressTranslator` trait。

#### 方法

##### `translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestPhysAddr, VmError>`

地址翻译。

##### `flush_tlb(&mut self)`

刷新TLB。

### MmioManager for PhysicalMemory and SoftMmu

#### 方法

##### `map_mmio(&self, base: GuestAddr, size: u64, device: Box<dyn MmioDevice>)`

映射MMIO设备。

**参数**:
- `base`: 基地址
- `size`: 区域大小
- `device`: MMIO设备

##### `poll_devices(&self)`

轮询设备。

## 页表标志

### pte_flags 模块常量

```rust
pub const V: u64 = 1 << 0;  // Valid
pub const R: u64 = 1 << 1;  // Read
pub const W: u64 = 1 << 2;  // Write
pub const X: u64 = 1 << 3;  // Execute
pub const U: u64 = 1 << 4;  // User
pub const G: u64 = 1 << 5;  // Global
pub const A: u64 = 1 << 6;  // Accessed
pub const D: u64 = 1 << 7;  // Dirty
```

## 使用示例

### 基本用法：Bare模式

```rust
use vm_mem::SoftMmu;
use vm_core::{GuestAddr, AccessType};

let mut mmu = SoftMmu::new(1024 * 1024, false);

// Bare模式：恒等映射
let phys = mmu.translate(GuestAddr(0x1000), AccessType::Read)?;
assert_eq!(phys, vm_core::GuestPhysAddr(0x1000));

// 写入和读取
mmu.write(GuestAddr(0x100), 0xDEADBEEF, 4)?;
assert_eq!(mmu.read(GuestAddr(0x100), 4)?, 0xDEADBEEF);
```

### SV39分页模式

```rust
use vm_mem::{SoftMmu, PageTableBuilder, PagingMode};
use vm_core::{GuestAddr, GuestPhysAddr};

let mut mmu = SoftMmu::new(16 * 1024 * 1024, false);
let mut builder = PageTableBuilder::new(GuestPhysAddr(0x10000));

// 初始化根页表
let root_table = 0x100000;
for i in 0..512 {
    mmu.write_phys(GuestPhysAddr(root_table + i * 8), 0, 8)?;
}

// 映射虚拟地址 0x1000 -> 物理地址 0x200000
let va = GuestAddr(0x1000);
let pa = GuestPhysAddr(0x200000);
let flags = vm_mem::pte_flags::R | vm_mem::pte_flags::W | vm_mem::pte_flags::X;

builder.map_page_sv39(&mut mmu, va, pa, flags, GuestPhysAddr(root_table))?;

// 设置分页模式和SATP
mmu.set_paging_mode(PagingMode::Sv39);
let satp = (8u64 << 60) | (root_table >> 12); // MODE=Sv39, PPN
mmu.set_satp(satp);

// 测试地址翻译
let translated = mmu.translate(GuestAddr(0x1000), vm_core::AccessType::Read)?;
assert_eq!(translated, pa);
```

### MMIO设备映射

```rust
use vm_mem::SoftMmu;
use vm_core::{MmioDevice, VmError, GuestAddr};

struct UartDevice {
    tx_data: u8,
    rx_data: u8,
}

impl MmioDevice for UartDevice {
    fn read(&self, offset: u64, size: u8) -> VmResult<u64> {
        match offset {
            0 => Ok(self.rx_data as u64),
            _ => Ok(0),
        }
    }

    fn write(&mut self, offset: u64, value: u64, size: u8) -> VmResult<()> {
        match offset {
            0 => { self.tx_data = value as u8; Ok(()) }
            _ => Ok(()),
        }
    }
}

let mmu = SoftMmu::new(1024 * 1024, false);

// 映射UART设备到 0x1000_0000
mmu.map_mmio(GuestAddr(0x1000_0000), 0x1000, Box::new(UartDevice { tx_data: 0, rx_data: 0 }));
```

### LR/SC原子操作

```rust
use vm_mem::SoftMmu;
use vm_core::GuestAddr;

let mut mmu = SoftMmu::new(1024 * 1024, false);
let addr = GuestAddr(0x2000);

// 初始化内存
mmu.write(addr, 0x11, 1)?;

// Load-reserved
let v = mmu.load_reserved(addr, 1)?;
assert_eq!(v, 0x11);

// Store-conditional should succeed
let ok = mmu.store_conditional(addr, 0x22, 1)?;
assert!(ok);
assert_eq!(mmu.read(addr, 1)?, 0x22);
```

### 批量内存操作

```rust
use vm_mem::SoftMmu;
use vm_core::GuestAddr;

let mmu = SoftMmu::new(1024 * 1024, false);
let addr = GuestAddr(0x1000);

// 批量写入
let data = vec![0xAA; 4096];
mmu.write_bulk(addr, &data)?;

// 批量读取
let mut buf = vec![0u8; 4096];
mmu.read_bulk(addr, &mut buf)?;
assert_eq!(buf, data);
```

### TLB统计

```rust
use vm_mem::SoftMmu;
use vm_core::{GuestAddr, AccessType};

let mut mmu = SoftMmu::new(1024 * 1024, false);

// 设置分页模式
mmu.set_paging_mode(vm_mem::PagingMode::Sv39);

// 多次访问同一地址
for _ in 0..1000 {
    let _ = mmu.translate(GuestAddr(0x1000), AccessType::Read);
}

// 获取TLB统计
let (hits, misses) = mmu.tlb_stats();
let hit_rate = hits as f64 / (hits + misses) as f64;
println!("TLB hit rate: {:.2}%", hit_rate * 100.0);
```

## 性能优化

### 分片设计

`PhysicalMemory`使用16个分片，每个分片有独立的RwLock，显著提高并发性能：
- 减少锁竞争
- 支持多vCPU并行访问
- 自动处理跨分片访问

### TLB优化

`SoftMmu`使用两级TLB：
- **ITLB**: 64条目，用于指令取指
- **DTLB**: 128条目，用于数据访问

优化技巧：
- 使用ASID隔离地址空间
- 支持全局页条目（G标志）
- LRU替换策略

### 大页支持

使用2MB huge pages可以：
- 减少TLB压力
- 提高页表遍历效率
- 减少页表内存占用

启用方式：
```rust
let mmu = SoftMmu::new(1024 * 1024 * 1024, true); // use_hugepages=true
```

### 批量操作

对于大量数据传输，使用批量操作：
```rust
// 高效
mmu.write_bulk(addr, large_buffer)?;

// 低效（逐字节写入）
for (i, &byte) in large_buffer.iter().enumerate() {
    mmu.write(addr + i as u64, byte as u64, 1)?;
}
```

## 错误处理

所有可能失败的函数都返回`Result<T, VmError>`：

```rust
use vm_core::VmError;

match mmu.translate(GuestAddr(0x1000), AccessType::Read) {
    Ok(phys) => println!("Translated to {:x}", phys),
    Err(VmError::Execution(vm_core::ExecutionError::Fault(fault))) => {
        eprintln!("Page fault: {:?}", fault);
    }
    Err(e) => eprintln!("Error: {:?}", e),
}
```

常见错误：
- `MemoryError::InvalidAddress` - 地址超出范围
- `ExecutionError::Fault(Fault::PageFault)` - 页面缺失
- `ExecutionError::Fault(Fault::AlignmentFault)` - 对齐错误

## 注意事项

### 线程安全

- `PhysicalMemory`使用`Arc<PhysicalMemory>`在线程间共享
- 分片RwLock提供并发访问
- TLB不是线程安全的，每个vCPU应有独立的MMU实例

### 内存对齐

默认情况下，内存访问不需要严格对齐。可以通过环境变量启用：

```bash
export VM_STRICT_ALIGN=1
```

或在代码中设置：
```rust
mmu.set_strict_align(true);
```

### MMIO处理

MMIO区域会自动检测并路由到对应的设备处理函数。MMIO访问不会通过物理内存后端。

### 页表一致性

修改页表后需要手动刷新TLB：
```rust
mmu.flush_tlb();  // 刷新所有
mmu.flush_tlb_page(va);  // 刷新特定页
mmu.flush_tlb_asid(asid);  // 刷新特定ASID
```

## 相关API

- [VmCore API](./VmCore.md) - 核心类型和Trait定义
- [VmInterface API](./VmInterface.md) - 内存管理接口规范
- [InstructionSet API](./InstructionSet.md) - 指令集支持
