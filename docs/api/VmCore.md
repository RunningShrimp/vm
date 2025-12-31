# vm-core API 参考

## 概述

`vm-core` 是虚拟机项目的核心库，提供了虚拟机的基础类型定义、Trait抽象和基础设施。它是所有其他VM组件的依赖基础。

## 主要功能

- **类型定义**: 地址类型（GuestAddr、GuestPhysAddr、HostAddr）
- **架构支持**: GuestArch 枚举支持 RISC-V64、ARM64、x86_64、PowerPC64
- **执行抽象**: ExecutionEngine、Decoder trait
- **内存管理**: MMU、MemoryAccess trait
- **错误处理**: 统一的错误类型系统
- **事件系统**: EventStore trait 用于事件持久化

## 主要类型

### GuestAddr

客户机虚拟地址，表示Guest操作系统的虚拟地址。

#### 方法

##### `wrapping_add(self, rhs: u64) -> GuestAddr`

环绕加法运算，模拟CPU的溢出行为。

**参数**:
- `rhs`: 要加上的值

**返回**:
- 加法结果（如果溢出则从0重新开始）

**示例**:
```rust
use vm_core::GuestAddr;

let addr = GuestAddr(0xFFFF_FFFF_FFFF_FFFF);
let result = addr.wrapping_add(1);
assert_eq!(result, GuestAddr(0));
```

##### `wrapping_sub(self, rhs: u64) -> GuestAddr`

环绕减法运算。

**参数**:
- `rhs`: 要减去的值

**返回**:
- 减法结果（如果溢出则从最大值重新开始）

##### `value(self) -> u64`

获取地址的原始u64值。

**返回**:
- 地址的原始u64表示

##### `as_i64(self) -> i64`

转换为i64，用于偏移量计算。

**返回**:
- i64表示的地址值

### GuestPhysAddr

客户机物理地址，表示Guest视角的物理地址。

#### 方法

##### `to_guest_addr(self) -> GuestAddr`

转换为GuestAddr。

**返回**:
- 对应的GuestAddr

### GuestArch

客户机架构枚举，定义支持的Guest CPU架构类型。

#### 变体

##### `Riscv64`

RISC-V 64位架构，支持RV64I基础指令集和M/A/F/D/C扩展。

##### `Arm64`

ARM 64位架构，支持AArch64指令集。

##### `X86_64`

x86-64架构，支持AMD64指令集。

##### `PowerPC64`

PowerPC 64位架构。

#### 方法

##### `name(&self) -> &'static str`

获取架构名称。

**返回**:
- 架构的小写字符串名称，如 "riscv64"、"arm64" 等

**示例**:
```rust
use vm_core::GuestArch;

let arch = GuestArch::Riscv64;
println!("Architecture: {}", arch.name()); // "riscv64"
```

### VmConfig

虚拟机配置结构，定义虚拟机的核心配置参数。

#### 字段

##### `guest_arch: GuestArch`

客户机架构类型。

##### `memory_size: usize`

内存大小（字节）。

##### `vcpu_count: usize`

虚拟CPU数量。

##### `exec_mode: ExecMode`

执行模式（解释器/JIT/硬件辅助）。

##### `kernel_path: Option<String>`

内核文件路径（可选）。

##### `initrd_path: Option<String>`

初始化RAM磁盘路径（可选）。

#### 实现

##### `Default`

默认配置：RISC-V64架构、128MB内存、1个vCPU、解释器模式。

**示例**:
```rust
use vm_core::{VmConfig, GuestArch, ExecMode};

let config = VmConfig {
    guest_arch: GuestArch::Riscv64,
    memory_size: 128 * 1024 * 1024, // 128MB
    vcpu_count: 1,
    exec_mode: ExecMode::Interpreter,
    kernel_path: Some("/path/to/kernel".to_string()),
    initrd_path: None,
};

// 或使用默认配置
let default_config = VmConfig::default();
```

### ExecMode

执行模式枚举，定义虚拟机的执行引擎类型。

#### 变体

##### `Interpreter`

解释器模式，逐条或逐块解释执行Guest指令。

**特点**:
- 实现简单，便于调试
- 启动快速，无编译开销
- 性能较低，通常为原生的1-5%

##### `JIT`

JIT编译模式，即时编译Guest指令为Host机器码。

**特点**:
- 高性能，可达原生的50-80%
- 需要编译时间
- 内存占用较大（代码缓存）

##### `HardwareAssisted`

硬件辅助虚拟化模式，利用Intel VT-x、AMD-V等硬件虚拟化技术。

**特点**:
- 最高性能，接近原生
- 依赖硬件支持
- 主要用于系统虚拟化

### AccessType

访问类型枚举，定义内存访问的类型。

#### 变体

##### `Read`

读取访问，检查页表R(Read)位。

##### `Write`

写入访问，检查页表W(Write)位和D(Dirty)位。

##### `Execute`

执行访问，用于指令获取，检查页表X(Execute)位。

##### `Atomic`

原子操作，用于LR/SC等原子指令。

**示例**:
```rust
use vm_core::AccessType;

let read = AccessType::Read;
let write = AccessType::Write;
let execute = AccessType::Execute;
```

### Fault

故障/异常类型枚举，表示虚拟机执行过程中可能发生的各种故障和异常。

#### 变体

##### `PageFault`

页面故障，虚拟地址无法翻译到物理地址，或者权限不足。

**字段**:
- `addr: GuestAddr` - 触发故障的虚拟地址
- `access_type: AccessType` - 访问类型（读/写/执行）
- `is_write: bool` - 是否是写操作
- `is_user: bool` - 是否是用户模式访问

##### `GeneralProtection`

一般保护故障，通用的保护违规，如权限不足、特权指令等。

##### `SegmentFault`

段故障，段描述符相关错误（主要用于x86架构）。

##### `AlignmentFault`

对齐故障，访问未对齐的内存地址。

##### `BusError`

总线错误，物理内存访问失败，如访问无效的物理地址。

##### `InvalidOpcode`

无效操作码，解码器无法识别的指令。

**字段**:
- `pc: GuestAddr` - 指令地址
- `opcode: u32` - 原始操作码

**示例**:
```rust
use vm_core::{Fault, GuestAddr, AccessType};

let fault = Fault::PageFault {
    addr: GuestAddr(0x1000),
    access_type: AccessType::Read,
    is_write: false,
    is_user: true,
};
```

## Trait

### Decoder

指令解码器trait，负责将二进制机器码解码为可执行的指令表示。

#### 关联类型

##### `type Instruction`

指令类型。

##### `type Block`

基本块类型。

#### 方法

##### `decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> VmResult<Self::Instruction>`

解码单条指令。

**参数**:
- `mmu`: MMU引用，用于读取虚拟内存中的指令
- `pc`: 程序计数器，指向要解码的指令地址

**返回**:
- 解码后的指令对象

##### `decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> VmResult<Self::Block>`

解码指令块（基本块）。

**参数**:
- `mmu`: MMU引用，用于读取虚拟内存中的指令
- `pc`: 程序计数器，指向基本块起始地址

**返回**:
- 解码后的基本块对象

### ExecutionEngine

执行引擎trait，负责执行解码后的指令或基本块，管理vCPU状态。

#### 方法

##### `execute_instruction(&mut self, instruction: &Instruction) -> VmResult<()>`

执行单条指令。

**参数**:
- `instruction`: 要执行的指令

##### `run(&mut self, mmu: &mut dyn MMU, block: &BlockType) -> ExecResult`

运行虚拟机，执行一个基本块或执行上下文。

**参数**:
- `mmu`: 可变MMU引用，用于内存访问
- `block`: 要执行的基本块

**返回**:
- 执行结果，包含终止原因和可能的错误

##### `get_reg(&self, idx: usize) -> u64`

获取指定编号的寄存器值。

**参数**:
- `idx`: 寄存器编号

**返回**:
- 寄存器的当前值

##### `set_reg(&mut self, idx: usize, val: u64)`

设置指定编号的寄存器值。

**参数**:
- `idx`: 寄存器编号
- `val`: 要设置的值

##### `get_pc(&self) -> GuestAddr`

获取程序计数器（PC）。

**返回**:
- 当前程序计数器值

##### `set_pc(&mut self, pc: GuestAddr)`

设置程序计数器（PC）。

**参数**:
- `pc`: 新的程序计数器值

##### `get_vcpu_state(&self) -> VcpuStateContainer`

获取VCPU状态。

**返回**:
- vCPU的完整状态容器，包含所有寄存器和控制寄存器

##### `set_vcpu_state(&mut self, state: &VcpuStateContainer)`

设置VCPU状态。

**参数**:
- `state`: 要设置的vCPU状态

### MMU

内存管理单元trait（从mmu_traits模块重新导出）。

#### 子Trait

##### `AddressTranslator`

地址翻译器。

**方法**:
- `translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestPhysAddr, VmError>`
- `flush_tlb(&mut self)`
- `flush_tlb_asid(&mut self, asid: u16)`
- `flush_tlb_page(&mut self, va: GuestAddr)`

##### `MemoryAccess`

内存访问接口。

**方法**:
- `read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError>`
- `write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError>`
- `fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError>`
- `read_bulk(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError>`
- `write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError>`

##### `MmioManager`

MMIO设备管理器。

**方法**:
- `map_mmio(&self, base: GuestAddr, size: u64, device: Box<dyn MmioDevice>)`
- `poll_devices(&self)`

##### `MmuAsAny`

类型转换支持。

**方法**:
- `as_any(&self) -> &dyn Any`
- `as_any_mut(&mut self) -> &mut dyn Any`

### MmioDevice

MMIO设备trait，定义内存映射I/O设备的接口。

#### 方法

##### `read(&self, offset: u64, size: u8) -> VmResult<u64>`

读取MMIO寄存器。

**参数**:
- `offset`: 设备内的偏移地址（字节）
- `size`: 读取大小（1/2/4/8 字节）

**返回**:
- 读取的数据值

##### `write(&mut self, offset: u64, value: u64, size: u8) -> VmResult<()>`

写入MMIO寄存器。

**参数**:
- `offset`: 设备内的偏移地址（字节）
- `value`: 要写入的值
- `size`: 写入大小（1/2/4/8 字节）

**返回**:
- 写入成功返回Ok(())，失败返回错误

**示例**:
```rust
struct UartDevice {
    tx_data: u8,
    rx_data: u8,
}

impl vm_core::MmioDevice for UartDevice {
    fn read(&self, offset: u64, size: u8) -> vm_core::VmResult<u64> {
        match offset {
            0 => Ok(self.rx_data as u64),
            _ => Ok(0),
        }
    }

    fn write(&mut self, offset: u64, value: u64, size: u8) -> vm_core::VmResult<()> {
        match offset {
            0 => { self.tx_data = value as u8; Ok(()) }
            _ => Ok(()),
        }
    }
}
```

## 错误类型

### VmError

虚拟机错误类型，统一错误处理。

#### 变体

##### `Memory(MemoryError)`

内存相关错误。

##### `Execution(ExecutionError)`

执行相关错误。

##### `Core(CoreError)`

核心错误。

##### `Device(DeviceError)`

设备错误。

##### `Platform(PlatformError)`

平台错误。

### MemoryError

内存错误类型。

#### 变体

##### `InvalidAddress(GuestAddr)`

无效地址。

##### `AccessViolation(GuestAddr)`

访问违规。

##### `AlignmentError`

对齐错误。

### ExecutionError

执行错误类型。

#### 变体

##### `Fault(Fault)`

执行故障。

##### `InvalidInstruction`

无效指令。

## 辅助类型

### VmState

虚拟机状态结构。

#### 字段

##### `regs: GuestRegs`

寄存器状态。

##### `memory: Vec<u8>`

内存状态。

##### `pc: GuestAddr`

程序计数器。

### VcpuStateContainer

VCPU状态容器。

#### 字段

##### `vcpu_id: usize`

VCPU ID。

##### `state: VmState`

VCPU状态。

##### `running: bool`

是否运行中。

### ExecStatus

执行状态枚举。

#### 变体

##### `Continue`

继续执行。

##### `Ok`

执行完成。

##### `Fault(ExecutionError)`

执行故障。

##### `IoRequest`

IO请求。

##### `InterruptPending`

中断待处理。

### ExecStats

执行统计信息结构。

#### 字段

##### `executed_insns: u64`

已执行的指令数。

##### `mem_accesses: u64`

内存访问次数。

##### `exec_time_ns: u64`

执行时间（纳秒）。

##### `tlb_hits: u64`

TLB命中次数。

##### `tlb_misses: u64`

TLB未命中次数。

##### `jit_compiles: u64`

JIT编译次数。

##### `jit_compile_time_ns: u64`

JIT编译时间（纳秒）。

### ExecResult

执行结果结构。

#### 字段

##### `status: ExecStatus`

执行状态。

##### `stats: ExecStats`

执行统计信息。

##### `next_pc: GuestAddr`

下一条指令的程序计数器。

## 使用示例

### 基本用法：创建虚拟机配置

```rust
use vm_core::{VmConfig, GuestArch, ExecMode};

let config = VmConfig {
    guest_arch: GuestArch::Riscv64,
    memory_size: 128 * 1024 * 1024, // 128MB
    vcpu_count: 1,
    exec_mode: ExecMode::Interpreter,
    kernel_path: Some("/path/to/kernel".to_string()),
    initrd_path: None,
};
```

### 地址操作

```rust
use vm_core::GuestAddr;

let addr1 = GuestAddr(0x1000);
let addr2 = addr1 + 0x100;  // 0x1100
let diff = addr2 - addr1;   // 0x100

// 环绕运算
let max_addr = GuestAddr(0xFFFF_FFFF_FFFF_FFFF);
let wrapped = max_addr.wrapping_add(1); // 0
```

### 故障处理

```rust
use vm_core::{Fault, GuestAddr, AccessType};

// 创建页面故障
let fault = Fault::PageFault {
    addr: GuestAddr(0x1000),
    access_type: AccessType::Read,
    is_write: false,
    is_user: true,
};

// 匹配处理
match fault {
    Fault::PageFault { addr, access_type, .. } => {
        println!("Page fault at {:x} during {:?}", addr, access_type);
    }
    _ => {}
}
```

## 注意事项

### 地址类型

- `GuestAddr` 是虚拟地址，在分页模式下需要通过MMU翻译
- `GuestPhysAddr` 是Guest物理地址，在虚拟化环境中可能不是真正的Host物理地址
- `HostAddr` 是Host视角的地址，通常是Host虚拟地址

### 执行模式选择

- **Interpreter**: 适合调试、快速原型开发
- **JIT**: 适合需要高性能的场景
- **HardwareAssisted**: 适合系统虚拟化，需要硬件支持

### 错误处理

所有可能失败的函数都返回 `VmResult<T>`，即 `Result<T, VmError>`：

```rust
use vm_core::{VmResult, VmError};

fn example() -> VmResult<()> {
    // 可能失败的操作
    Ok(())
}
```

### 线程安全

- `GuestAddr`、`GuestPhysAddr` 等类型是 `Send + Sync` 的
- `ExecutionEngine` trait 要求 `Send + Sync`
- MMU实现通常使用内部锁保证线程安全

## 性能考虑

### 地址运算

使用 `wrapping_add` 和 `wrapping_sub` 进行地址运算，避免检查开销。

### TLB优化

MMU实现通常包含TLB缓存，合理使用TLB刷新：
- `flush_tlb()` - 刷新所有TLB
- `flush_tlb_asid(asid)` - 刷新特定地址空间
- `flush_tlb_page(va)` - 刷新特定页

### 批量内存操作

使用 `read_bulk` 和 `write_bulk` 进行大量数据传输，提高效率。

## 相关API

- [VmMemory API](./VmMemory.md) - 内存管理详细文档
- [VmInterface API](./VmInterface.md) - VM接口规范
- [InstructionSet API](./InstructionSet.md) - 指令集支持
