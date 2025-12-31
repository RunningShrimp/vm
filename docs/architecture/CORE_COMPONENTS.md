# 核心组件架构

## 目录

- [组件概述](#组件概述)
- [vm-core: 核心库](#vm-core-核心库)
- [vm-interface: 统一接口](#vm-interface-统一接口)
- [vm-engine: 执行引擎](#vm-engine-执行引擎)
- [vm-mem: 内存子系统](#vm-mem-内存子系统)
- [vm-frontend: 指令前端](#vm-frontend-指令前端)
- [vm-ir: 中间表示](#vm-ir-中间表示)
- [vm-device: 设备仿真](#vm-device-设备仿真)
- [vm-runtime: 运行时](#vm-runtime-运行时)
- [组件交互图](#组件交互图)
- [职责分离原则](#职责分离原则)

---

## 组件概述

VM项目的核心组件采用分层架构设计，每一层都有明确的职责和边界：

```
┌─────────────────────────────────────────────────────────┐
│                     应用层                              │
│  vm-service, vm-runtime, vm-boot, vm-cli, vm-desktop   │
└─────────────────────────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────┐
│                    领域层                               │
│              vm-core (核心类型和trait)                  │
└─────────────────────────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────┐
│                   基础设施层                             │
│  vm-engine, vm-mem, vm-device, vm-frontend, vm-ir      │
└─────────────────────────────────────────────────────────┘
```

---

## vm-core: 核心库

### 职责

`vm-core`是整个VM项目的基础，定义了核心类型、trait抽象和领域模型。

### 主要组件

#### 1. 核心类型 (Core Types)

```rust
// 地址类型
pub struct GuestAddr(pub u64);           // Guest虚拟地址
pub struct GuestPhysAddr(pub u64);       // Guest物理地址
pub struct HostAddr(pub u64);            // Host地址

// 架构枚举
pub enum GuestArch {
    Riscv64,
    Arm64,
    X86_64,
    PowerPC64,
}

// 执行模式
pub enum ExecMode {
    Interpreter,       // 解释器
    JIT,              // JIT编译
    HardwareAssisted, // 硬件辅助
}
```

**设计要点**:
- **强类型**: 使用newtype模式避免混淆不同类型的地址
- **类型安全**: 编译时保证地址类型的正确使用
- **零成本抽象**: 地址操作编译为高效的内联代码

#### 2. 核心Trait (Core Traits)

```rust
/// MMU trait - 定义内存管理单元接口
pub trait MMU: AddressTranslator + MemoryAccess {
    fn translate(&mut self, va: GuestAddr, access: AccessType)
        -> Result<GuestPhysAddr, VmError>;
    fn flush_tlb(&mut self);
    // ...
}

/// Decoder trait - 定义指令解码器接口
pub trait Decoder {
    type Instruction;
    type Block;

    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Instruction>;
    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Block>;
}

/// ExecutionEngine trait - 定义执行引擎接口
pub trait ExecutionEngine<BlockType>: Send + Sync {
    fn execute_instruction(&mut self, instruction: &Instruction)
        -> VmResult<()>;
    fn run(&mut self, mmu: &mut dyn MMU, block: &BlockType)
        -> ExecResult;
    // ...
}

/// MmioDevice trait - 定义MMIO设备接口
pub trait MmioDevice: Send + Sync {
    fn read(&self, offset: u64, size: u8) -> VmResult<u64>;
    fn write(&mut self, offset: u64, value: u64, size: u8)
        -> VmResult<()>;
}
```

#### 3. 领域模型 (Domain Model)

```rust
/// VM配置 - 值对象
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    pub guest_arch: GuestArch,
    pub memory_size: usize,
    pub vcpu_count: usize,
    pub exec_mode: ExecMode,
    pub kernel_path: Option<String>,
    pub initrd_path: Option<String>,
}

/// VM状态 - 聚合根
pub struct VmState {
    pub regs: GuestRegs,
    pub memory: Vec<u8>,
    pub pc: GuestAddr,
}

/// 执行统计
pub struct ExecStats {
    pub executed_insns: u64,
    pub mem_accesses: u64,
    pub exec_time_ns: u64,
    pub tlb_hits: u64,
    pub tlb_misses: u64,
    pub jit_compiles: u64,
    pub jit_compile_time_ns: u64,
}
```

#### 4. 错误类型 (Error Types)

```rust
/// VM错误类型
pub enum VmError {
    Execution(ExecutionError),
    Memory(MemoryError),
    Core(CoreError),
    Device(DeviceError),
    Platform(PlatformError),
}

/// 执行错误
pub enum ExecutionError {
    Fault(Fault),
    InvalidOpcode { pc: GuestAddr, opcode: u32 },
    DivisionByZero,
    Overflow,
}

/// 内存错误
pub enum MemoryError {
    InvalidAddress(GuestAddr),
    AccessViolation(GuestAddr, AccessType),
    MisalignedAccess(GuestAddr, usize),
}

/// 故障类型
pub enum Fault {
    PageFault {
        addr: GuestAddr,
        access_type: AccessType,
        is_write: bool,
        is_user: bool,
    },
    GeneralProtection,
    AlignmentFault,
    BusError,
    InvalidOpcode { pc: GuestAddr, opcode: u32 },
}
```

### 模块组织

```
vm-core/
├── src/
│   ├── lib.rs                 # 核心类型和trait
│   ├── config.rs              # 配置管理
│   ├── domain.rs              # 领域模型
│   ├── error.rs               # 错误类型
│   ├── mmu_traits.rs          # MMU接口
│   ├── value_objects.rs       # 值对象
│   ├── vm_state.rs            # VM状态
│   ├── event_store/           # 事件存储
│   │   ├── mod.rs
│   │   ├── postgres_event_store.rs
│   │   └── in_memory_event_store.rs
│   ├── domain_services/       # 领域服务
│   │   ├── translation_strategy_service.rs
│   │   ├── optimization_pipeline_service.rs
│   │   └── performance_optimization_service.rs
│   └── snapshot/              # 快照功能
│       ├── mod.rs
│       └── base.rs
```

### 关键设计模式

#### 1. Trait对象多态

```rust
/// 使用trait对象实现运行时多态
pub struct VirtualMachine {
    engine: Box<dyn ExecutionEngine<IRBlock>>,
    mmu: Box<dyn MMU>,
    decoder: Box<dyn Decoder>,
}

impl VirtualMachine {
    pub fn new(config: &VmConfig) -> Self {
        let engine: Box<dyn ExecutionEngine<IRBlock>> = match config.exec_mode {
            ExecMode::Interpreter => Box::new(InterpreterEngine::new()),
            ExecMode::JIT => Box::new(JITEngine::new()),
            ExecMode::HardwareAssisted => Box::new(HardwareEngine::new()),
        };
        // ...
    }
}
```

#### 2. 建造者模式

```rust
/// 配置建造者
pub struct VmConfigBuilder {
    arch: Option<GuestArch>,
    memory_size: Option<usize>,
    vcpu_count: Option<usize>,
    exec_mode: Option<ExecMode>,
}

impl VmConfigBuilder {
    pub fn new() -> Self {
        Self {
            arch: None,
            memory_size: None,
            vcpu_count: None,
            exec_mode: None,
        }
    }

    pub fn arch(mut self, arch: GuestArch) -> Self {
        self.arch = Some(arch);
        self
    }

    pub fn memory_size(mut self, size: usize) -> Self {
        self.memory_size = Some(size);
        self
    }

    pub fn build(self) -> Result<VmConfig, ConfigError> {
        Ok(VmConfig {
            guest_arch: self.arch.unwrap_or(GuestArch::Riscv64),
            memory_size: self.memory_size.unwrap_or(128 * 1024 * 1024),
            vcpu_count: self.vcpu_count.unwrap_or(1),
            exec_mode: self.exec_mode.unwrap_or(ExecMode::Interpreter),
            kernel_path: None,
            initrd_path: None,
        })
    }
}
```

---

## vm-interface: 统一接口

### 职责

`vm-interface`定义了VM各组件的统一接口规范，遵循SOLID原则，提供标准化的组件交互方式。

### 核心接口

#### 1. 组件生命周期 (VmComponent)

```rust
pub trait VmComponent: Send + Sync {
    type Config;
    type Error;

    /// 初始化组件
    fn init(config: Self::Config) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// 启动组件
    fn start(&mut self) -> Result<(), Self::Error>;

    /// 停止组件
    fn stop(&mut self) -> Result<(), Self::Error>;

    /// 获取组件状态
    fn status(&self) -> ComponentStatus;

    /// 获取组件名称
    fn name(&self) -> &str;
}
```

#### 2. 可配置性 (Configurable)

```rust
pub trait Configurable {
    type Config;

    /// 更新配置
    fn update_config(&mut self, config: &Self::Config)
        -> Result<(), VmError>;

    /// 获取当前配置
    fn get_config(&self) -> &Self::Config;

    /// 验证配置
    fn validate_config(config: &Self::Config)
        -> Result<(), VmError>;
}
```

#### 3. 可观察性 (Observable)

```rust
pub trait Observable {
    type State;
    type Event;

    /// 获取当前状态
    fn get_state(&self) -> &Self::State;

    /// 订阅状态变化
    fn subscribe(
        &mut self,
        callback: StateEventCallback<Self::State, Self::Event>,
    ) -> SubscriptionId;

    /// 取消订阅
    fn unsubscribe(&mut self, id: SubscriptionId)
        -> Result<(), VmError>;
}
```

#### 4. 扩展的执行引擎 (ExecutionEngine)

```rust
pub trait ExecutionEngine<I>:
    VmComponent + Configurable + Observable
{
    type State;
    type Stats;

    /// 执行IR块
    fn execute<M: MemoryManager>(
        &mut self,
        mmu: &mut M,
        block: &I
    ) -> ExecResult;

    /// 获取/设置寄存器
    fn get_register(&self, index: usize) -> u64;
    fn set_register(&mut self, index: usize, value: u64);

    /// 获取/设置PC
    fn get_pc(&self) -> GuestAddr;
    fn set_pc(&mut self, pc: GuestAddr);

    /// 获取执行状态和统计
    fn get_execution_state(&self) -> &Self::State;
    fn get_execution_stats(&self) -> &Self::Stats;

    /// 重置状态
    fn reset(&mut self);
}
```

#### 5. 内存管理器 (MemoryManager)

```rust
pub trait MemoryManager: VmComponent + Configurable {
    /// 读取内存
    fn read_memory(&self, addr: GuestAddr, size: usize)
        -> Result<Vec<u8>, VmError>;

    /// 写入内存
    fn write_memory(&mut self, addr: GuestAddr, data: &[u8])
        -> Result<(), VmError>;

    /// 原子操作
    fn read_atomic(&self, addr: GuestAddr, size: usize,
        order: MemoryOrder) -> Result<u64, VmError>;

    fn write_atomic(&mut self, addr: GuestAddr, value: u64,
        size: usize, order: MemoryOrder) -> Result<(), VmError>;

    fn compare_exchange(&mut self, addr: GuestAddr,
        expected: u64, desired: u64, size: usize,
        success: MemoryOrder, failure: MemoryOrder)
        -> Result<u64, VmError>;
}
```

#### 6. 设备接口 (Device)

```rust
pub trait Device:
    VmComponent + Configurable + Observable
{
    type IoRegion;

    /// 获取设备ID和类型
    fn device_id(&self) -> DeviceId;
    fn device_type(&self) -> DeviceType;

    /// 获取I/O区域
    fn io_regions(&self) -> &[Self::IoRegion];

    /// 处理I/O操作
    fn handle_read(&mut self, offset: u64, size: usize)
        -> Result<u64, VmError>;
    fn handle_write(&mut self, offset: u64, value: u64,
        size: usize) -> Result<(), VmError>;

    /// 处理中断
    fn handle_interrupt(&mut self, vector: u32)
        -> Result<(), VmError>;
}
```

### 设计优势

1. **SOLID原则**:
   - 单一职责：每个trait只负责一个方面
   - 开闭原则：通过trait扩展，无需修改现有代码
   - 里氏替换：所有实现可以透明替换
   - 接口隔离：客户端只依赖需要的接口
   - 依赖倒置：依赖抽象而非具体实现

2. **组合优于继承**:
   ```rust
   // 多个trait组合
   struct MyEngine {
       // VmComponent + Configurable + Observable
   }

   impl VmComponent for MyEngine { /* ... */ }
   impl Configurable for MyEngine { /* ... */ }
   impl Observable for MyEngine { /* ... */ }
   ```

3. **类型安全**:
   ```rust
   // 编译时保证接口实现
   fn run_engine<E: ExecutionEngine<IRBlock>>(engine: &mut E) {
       // E必须实现ExecutionEngine的所有要求
   }
   ```

---

## vm-engine: 执行引擎

### 职责

`vm-engine`提供多种执行引擎实现，支持解释执行、JIT编译和混合模式。

### 架构

```
vm-engine/
├── src/
│   ├── lib.rs
│   ├── interpreter.rs          # 解释器实现
│   ├── jit.rs                  # JIT编译器
│   ├── hybrid.rs               # 混合引擎
│   └── executor.rs             # 执行器
```

### 执行模式

#### 1. 解释器 (Interpreter)

**特点**:
- 简单易实现
- 启动快速
- 便于调试
- 性能较低（1-5%原生）

**架构**:
```
┌─────────────────────────────────────┐
│        InterpreterEngine            │
│  ┌───────────────────────────────┐  │
│  │   Fetch-Decode-Execute循环   │  │
│  │  1. Fetch: 从内存读取指令     │  │
│  │  2. Decode: 解码为IR指令      │  │
│  │  3. Execute: 解释执行IR       │  │
│  │  4. Update: 更新PC和状态      │  │
│  └───────────────────────────────┘  │
└─────────────────────────────────────┘
```

**代码示例**:
```rust
pub struct InterpreterEngine {
    regs: [u64; 32],
    pc: GuestAddr,
    stats: ExecStats,
}

impl ExecutionEngine<IRBlock> for InterpreterEngine {
    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock)
        -> ExecResult
    {
        for insn in &block.instructions {
            self.execute_instruction(insn)?;
        }
        Ok(ExecResult {
            status: ExecStatus::Continue,
            next_pc: self.pc,
            stats: self.stats.clone(),
        })
    }
}
```

#### 2. JIT编译器 (JITCompiler)

**特点**:
- 高性能（50-80%原生）
- 需要编译时间
- 内存占用较大
- 代码缓存管理

**架构**:
```
┌─────────────────────────────────────────────┐
│             JITEngine                       │
│  ┌───────────────────────────────────────┐  │
│  │         JIT编译流程                   │  │
│  │  1. IR基本块分析                     │  │
│  │  2. 优化Passes                       │  │
│  │     - 常量折叠                       │  │
│  │     - 死代码消除                     │  │
│  │     - 寄存器分配                     │  │
│  │  3. 代码生成 (Cranelift/Dynasm)      │  │
│  │  4. 机器码缓存                       │  │
│  │  5. 执行编译后的代码                 │  │
│  └───────────────────────────────────────┘  │
│                                             │
│  ┌───────────────────────────────────────┐  │
│  │         代码缓存管理                 │  │
│  │  - LRU淘汰策略                       │  │
│  │  - 大小区分                          │  │
│  │  - 热点统计                          │  │
│  └───────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

**代码示例**:
```rust
pub struct JITEngine {
    code_cache: HashMap<GuestAddr, CompiledCode>,
    compiler: JITCompiler,
    stats: ExecStats,
}

impl ExecutionEngine<IRBlock> for JITEngine {
    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock)
        -> ExecResult
    {
        let entry_addr = block.address;

        // 检查缓存
        if !self.code_cache.contains_key(&entry_addr) {
            // 编译新代码
            let compiled = self.compiler.compile(block)?;
            self.code_cache.insert(entry_addr, compiled);
            self.stats.jit_compiles += 1;
        }

        // 执行编译后的代码
        let code = &self.code_cache[&entry_addr];
        let result = code.execute(mmu, self)?;

        Ok(result)
    }
}
```

#### 3. 混合引擎 (HybridEngine)

**特点**:
- 自适应优化
- 快速启动
- 渐进式优化

**架构**:
```
┌──────────────────────────────────────────┐
│         HybridEngine                     │
│  ┌────────────────────────────────────┐  │
│  │      热点检测机制                 │  │
│  │  - 基本块执行计数                 │  │
│  │  - 边界检测 (热度阈值)            │  │
│  │  - 降级策略                       │  │
│  └────────────────────────────────────┘  │
│                                           │
│  ┌────────────────────────────────────┐  │
│  │      执行模式切换                 │  │
│  │  冷代码 → 解释器执行              │  │
│  │  温代码 → 低优化JIT               │  │
│  │  热代码 → 高优化JIT               │  │
│  └────────────────────────────────────┘  │
└──────────────────────────────────────────┘
```

**代码示例**:
```rust
pub struct HybridEngine {
    interpreter: InterpreterEngine,
    jit: JITEngine,
    hot_threshold: u64,    // 热点阈值
    execution_counts: HashMap<GuestAddr, u64>,
}

impl ExecutionEngine<IRBlock> for HybridEngine {
    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock)
        -> ExecResult
    {
        let addr = block.address;
        let count = self.execution_counts.entry(addr).or_insert(0);
        *count += 1;

        if *count < self.hot_threshold {
            // 解释执行
            self.interpreter.run(mmu, block)
        } else {
            // JIT执行
            self.jit.run(mmu, block)
        }
    }
}
```

---

## vm-mem: 内存子系统

### 职责

`vm-mem`实现完整的内存管理子系统，包括物理内存、MMU、TLB和页表遍历。

### 架构

```
vm-mem/
├── src/
│   ├── lib.rs                 # 主入口
│   ├── mmu.rs                 # MMU实现
│   ├── tlb.rs                 # TLB优化
│   ├── memory.rs              # 物理内存
│   ├── async_mmu.rs           # 异步MMU
│   ├── domain_services/       # 领域服务
│   └── optimization/          # 优化模块
```

### 核心组件

#### 1. 物理内存 (PhysicalMemory)

```rust
pub struct PhysicalMemory {
    /// 分片内存 (16个分片，减少锁竞争)
    shards: Vec<RwLock<Vec<u8>>>,
    shard_size: usize,
    total_size: usize,
    /// MMIO设备区域
    mmio_regions: RwLock<Vec<MmioRegion>>,
    /// LR/SC保留
    reservations: RwLock<Vec<(GuestPhysAddr, u64, u8)>>,
    huge_page_allocator: HugePageAllocator,
}

impl PhysicalMemory {
    /// 创建物理内存
    pub fn new(size: usize, use_hugepages: bool) -> Self;

    /// 读写操作
    pub fn read_u8(&self, addr: usize) -> Result<u8, VmError>;
    pub fn read_u16(&self, addr: usize) -> Result<u16, VmError>;
    pub fn read_u32(&self, addr: usize) -> Result<u32, VmError>;
    pub fn read_u64(&self, addr: usize) -> Result<u64, VmError>;
    pub fn write_u8(&self, addr: usize, val: u8) -> Result<(), VmError>;
    // ...
}
```

**设计要点**:
- **分片锁**: 16个分片减少锁竞争
- **大页支持**: 可选2MB大页减少TLB压力
- **MMIO映射**: 支持内存映射I/O
- **原子操作**: 支持LR/SC指令对

#### 2. 软件MMU (SoftMmu)

```rust
pub struct SoftMmu {
    id: u64,
    phys_mem: Arc<PhysicalMemory>,
    itlb: Tlb,              // 指令TLB
    dtlb: Tlb,              // 数据TLB
    paging_mode: PagingMode,
    page_table_base: GuestPhysAddr,
    asid: u16,              // 地址空间ID
    page_table_walker: Option<Box<dyn PageTableWalker>>,
    tlb_hits: u64,
    tlb_misses: u64,
    strict_align: bool,
}

impl SoftMmu {
    /// 创建MMU
    pub fn new(size: usize, use_hugepages: bool) -> Self;

    /// 地址翻译
    pub fn translate(&mut self, va: GuestAddr, access: AccessType)
        -> Result<GuestPhysAddr, VmError>;

    /// 设置分页模式
    pub fn set_paging_mode(&mut self, mode: PagingMode);

    /// 设置RISC-V SATP寄存器
    pub fn set_satp(&mut self, satp: u64);
}
```

**地址翻译流程**:
```
虚拟地址
    ↓
TLB查找 (ITLB/DTLB)
    ↓ 命中
返回物理地址
    ↓ 未命中
页表遍历
    ↓
更新TLB
    ↓
返回物理地址
```

#### 3. TLB优化 (Tlb)

```rust
struct Tlb {
    entries: HashMap<u64, TlbEntry>,
    lru: LruCache<u64, ()>,
    global_entries: HashMap<u64, TlbEntry>,
    max_size: usize,
}

impl Tlb {
    /// 创建TLB
    fn new(size: usize) -> Self;

    /// 查找
    fn lookup(&mut self, vpn: u64, asid: u16)
        -> Option<(u64, u64)>;

    /// 插入
    fn insert(&mut self, vpn: u64, ppn: u64, flags: u64,
        asid: u16);

    /// 刷新
    fn flush(&mut self);
    fn flush_asid(&mut self, target_asid: u16);
    fn flush_page(&mut self, vpn: u64);
}
```

**TLB特性**:
- **分离TLB**: ITLB(64条目)和DTLB(128条目)
- **ASID支持**: 地址空间ID隔离
- **全局页**: G标志的全局页条目
- **LRU淘汰**: 最近最少使用策略

#### 4. 页表遍历器 (PageTableWalker)

```rust
pub trait PageTableWalker {
    /// 页表遍历
    fn walk(
        &mut self,
        va: GuestAddr,
        access: AccessType,
        asid: u16,
        mmu: &dyn MMU,
    ) -> Result<(GuestPhysAddr, u64), VmError>;
}

/// RISC-V SV39页表遍历器
pub struct Sv39PageTableWalker {
    root_table: GuestPhysAddr,
    asid: u16,
}

impl PageTableWalker for Sv39PageTableWalker {
    fn walk(&mut self, va: GuestAddr, access: AccessType,
        asid: u16, mmu: &dyn MMU)
        -> Result<(GuestPhysAddr, u64), VmError>
    {
        // 3级页表遍历
        // ...
    }
}
```

---

## vm-frontend: 指令前端

### 职责

`vm-frontend`提供多架构的指令解码器，将二进制机器码转换为IR指令。

### 架构

```
vm-frontend/
├── src/
│   ├── lib.rs
│   ├── riscv64/              # RISC-V解码器
│   │   ├── mod.rs
│   │   ├── decode.rs
│   │   ├── formats.rs
│   │   └── instructions.rs
│   ├── arm64/                # ARM64解码器
│   │   ├── mod.rs
│   │   └── decode.rs
│   └── x86_64/               # x86解码器
│       ├── mod.rs
│       └── decode.rs
```

### 解码器接口

```rust
pub trait Decoder {
    type Instruction;
    type Block;

    /// 解码单条指令
    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Instruction>;

    /// 解码基本块
    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Block>;
}
```

### RISC-V解码器

```rust
pub struct RiscvDecoder {
    insn_cache: LruCache<GuestAddr, RiscvInstruction>,
}

impl Decoder for RiscvDecoder {
    type Instruction = RiscvInstruction;
    type Block = IRBlock;

    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr)
        -> VmResult<Self::Instruction>
    {
        // 读取指令字
        let insn_word = mmu.fetch_insn(pc)? as u32;

        // 解码指令
        let opcode = (insn_word & 0x7F) as u8;
        let rd = ((insn_word >> 7) & 0x1F) as usize;
        let funct3 = ((insn_word >> 12) & 0x7) as u8;

        // 根据opcode解码
        match opcode {
            0x33 => Ok(RiscvInstruction::RType { /* ... */ }),
            0x13 => Ok(RiscvInstruction::IType { /* ... */ }),
            0x63 => Ok(RiscvInstruction::BType { /* ... */ }),
            // ...
            _ => Err(VmError::Execution(
                ExecutionError::Fault(Fault::InvalidOpcode {
                    pc, opcode: insn_word
                })
            )),
        }
    }
}
```

---

## vm-ir: 中间表示

### 职责

`vm-ir`定义与架构无关的中间表示，作为解码器和执行引擎之间的桥梁。

### IR指令

```rust
pub enum IRInstruction {
    /// 算术运算
    Add { dst: Reg, src1: Reg, src2: Operand },
    Sub { dst: Reg, src1: Reg, src2: Operand },
    Mul { dst: Reg, src1: Reg, src2: Operand },
    Div { dst: Reg, src1: Reg, src2: Operand },

    /// 逻辑运算
    And { dst: Reg, src1: Reg, src2: Operand },
    Or  { dst: Reg, src1: Reg, src2: Operand },
    Xor { dst: Reg, src1: Reg, src2: Operand },

    /// 内存访问
    Load  { dst: Reg, addr: MemOperand, size: u8 },
    Store { src: Reg, addr: MemOperand, size: u8 },

    /// 分支跳转
    Branch { cond: CondCode, target: GuestAddr },
    Jump   { target: GuestAddr },
    Call   { target: GuestAddr },
    Ret,

    /// 系统指令
    Syscall,
    Mret,
    Sret,
}

pub enum Operand {
    Reg(Reg),
    Imm(i64),
}

pub enum MemOperand {
    BaseDisp { base: Reg, disp: i64 },
    IndexScale { base: Reg, index: Reg, scale: u8 },
}
```

### IR基本块

```rust
pub struct IRBlock {
    pub address: GuestAddr,
    pub instructions: Vec<IRInstruction>,
    pub successors: Vec<GuestAddr>,
    pub is_exit: bool,
}

impl IRBlock {
    pub fn new(address: GuestAddr) -> Self {
        Self {
            address,
            instructions: Vec::new(),
            successors: Vec::new(),
            is_exit: false,
        }
    }

    pub fn push(&mut self, insn: IRInstruction) {
        self.instructions.push(insn);
    }
}
```

---

## vm-device: 设备仿真

### 职责

`vm-device`提供设备仿真框架，支持VirtIO设备、MMIO设备和PCI设备。

### 架构

```
vm-device/
├── src/
│   ├── lib.rs
│   ├── bus.rs                 # 设备总线
│   ├── mmio.rs                # MMIO框架
│   ├── interrupt.rs           # 中断控制器
│   └── virtio/                # VirtIO设备
│       ├── mod.rs
│       ├── block.rs           # 块设备
│       ├── net.rs             # 网络设备
│       ├── console.rs         # 控制台
│       └── queue.rs           # VirtQueue
```

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
```

### VirtIO块设备

```rust
pub struct VirtioBlockDevice {
    config: VirtioBlockConfig,
    queue: VirtQueue,
    backend: Box<dyn BlockBackend>,
    status: DeviceStatus,
}

impl MmioDevice for VirtioBlockDevice {
    fn read(&self, offset: u64, size: u8) -> VmResult<u64> {
        match offset {
            0x0 => Ok(self.status.bits()),
            0x4 => Ok(self.queue.get_driver_select()),
            // ...
            _ => Ok(0),
        }
    }

    fn write(&mut self, offset: u64, value: u64, size: u8)
        -> VmResult<()>
    {
        match offset {
            0x0 => self.status = DeviceStatus::from_bits_truncate(value as u32),
            0x4 => self.queue.set_driver_select(value as u32),
            // ...
            _ => Ok(()),
        }
    }
}
```

---

## vm-runtime: 运行时

### 职责

`vm-runtime`管理VM的生命周期、vCPU调度和资源管理。

### VM实例

```rust
pub struct VirtualMachine {
    id: VmId,
    config: VmConfig,
    vcpus: Vec<Vcpu>,
    memory: Arc<PhysicalMemory>,
    devices: DeviceManager,
    state: VmLifecycleState,
}

impl VirtualMachine {
    /// 创建VM
    pub fn new(config: VmConfig) -> Result<Self, VmError>;

    /// 启动VM
    pub fn start(&mut self) -> Result<(), VmError>;

    /// 停止VM
    pub fn stop(&mut self) -> Result<(), VmError>;

    /// 暂停/恢复
    pub fn pause(&mut self) -> Result<(), VmError>;
    pub fn resume(&mut self) -> Result<(), VmError>;
}
```

### VCPU

```rust
pub struct Vcpu {
    id: usize,
    engine: Box<dyn ExecutionEngine<IRBlock>>,
    mmu: SoftMmu,
    state: VcpuState,
    running: Arc<AtomicBool>,
}

impl Vcpu {
    /// 运行vCPU
    pub fn run(&mut self) -> ExecResult {
        while self.running.load(Ordering::Relaxed) {
            // 获取下一个基本块
            let block = self.fetch_block()?;

            // 执行
            let result = self.engine.run(&mut self.mmu, &block)?;

            // 处理结果
            match result.status {
                ExecStatus::Continue => continue,
                ExecStatus::Ok => break,
                ExecStatus::Fault(e) => return Err(e.into()),
                ExecStatus::IoRequest => self.handle_io()?,
                ExecStatus::InterruptPending => self.handle_interrupt()?,
            }
        }

        Ok(result)
    }
}
```

---

## 组件交互图

### 完整执行流程

```
┌─────────────────────────────────────────────────────────┐
│                    vm-runtime                           │
│                 (VirtualMachine)                        │
└────────────────┬────────────────────────────────────────┘
                 │ 创建
                 ↓
┌─────────────────────────────────────────────────────────┐
│                      Vcpu                               │
│  ┌─────────────────────────────────────────────────┐    │
│  │  run() loop:                                    │    │
│  │    1. fetch_block()                             │    │
│  │    2. engine.run()                              │    │
│  │    3. handle events                             │    │
│  └─────────────────────────────────────────────────┘    │
└───┬───────────┬───────────┬──────────────┬──────────────┘
    │           │           │              │
    ↓           ↓           ↓              ↓
┌─────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
│frontend │ │  engine  │ │   mmu    │ │ devices  │
│(解码器)  │ │(执行引擎) │ │ (MMU)    │ │ (设备)    │
└────┬────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘
     │           │            │             │
     ↓           ↓            ↓             ↓
┌──────────────────────────────────────────────────────┐
│                    vm-ir                             │
│              (IR指令和基本块)                          │
└──────────────────────────────────────────────────────┘
```

### 具体交互序列

```
Guest二进制
    │
    ↓
┌─────────────────────┐
│ vm-frontend         │  解码指令
│ (RiscvDecoder)      │
└─────────┬───────────┘
          │ IRBlock
          ↓
┌─────────────────────┐
│ vm-engine           │  执行IR
│ (JITEngine)         │
└─────────┬───────────┘
          │ 内存访问
          ↓
┌─────────────────────┐
│ vm-mem              │  地址翻译
│ (SoftMmu)           │  TLB查找/页表遍历
└─────────┬───────────┘
          │ 物理地址
          ↓
┌─────────────────────┐
│ PhysicalMemory      │  物理内存访问
└─────────┬───────────┘
          │ MMIO访问
          ↓
┌─────────────────────┐
│ vm-device           │  设备处理
│ (VirtioBlock)       │
└─────────────────────┘
```

---

## 职责分离原则

### 1. 单一职责

每个组件只负责一个明确的职责：

| 组件 | 职责 | 不负责 |
|------|------|--------|
| vm-core | 定义类型和trait | 具体实现 |
| vm-interface | 定义接口 | 实现逻辑 |
| vm-engine | 执行指令 | 内存管理 |
| vm-mem | 管理内存 | 指令解码 |
| vm-frontend | 解码指令 | 执行指令 |
| vm-device | 仿真设备 | CPU执行 |

### 2. 依赖方向

```
┌──────────────┐
│  vm-service  │  应用层（依赖领域层）
└──────┬───────┘
       ↓
┌──────────────┐
│   vm-core    │  领域层（不依赖任何人）
└──────┬───────┘
       ↓
┌──────────────┐
│ vm-engine    │  基础设施层（实现领域接口）
└──────────────┘
```

**规则**:
- 上层可以依赖下层
- 下层不能依赖上层
- 同层之间通过接口交互

### 3. 接口隔离

```rust
// 好的设计：精简的接口
trait MMU {
    fn translate(&mut self, va: GuestAddr, access: AccessType)
        -> Result<GuestPhysAddr, VmError>;
    fn flush_tlb(&mut self);
}

// 避免这样的设计：臃肿的接口
trait MMU_Bad {
    fn translate(&mut self, ...) -> Result<...>;
    fn flush_tlb(&mut self);
    fn get_stats(&self) -> TlbStats;      // 不应该在这里
    fn set_config(&mut self, ...);        // 不应该在这里
    fn dump_memory(&self) -> Vec<u8>;     // 不应该在这里
}
```

---

## 扩展性设计

### 添加新架构

```rust
// 1. 在vm-frontend添加新解码器
#[cfg(feature = "all")]
pub mod powerpc64 {
    pub struct PowerPC64Decoder { /* ... */ }

    impl Decoder for PowerPC64Decoder {
        type Instruction = PowerPCInstruction;
        type Block = IRBlock;
        // ...
    }
}

// 2. 在vm-core注册新架构
pub enum GuestArch {
    Riscv64,
    Arm64,
    X86_64,
    PowerPC64,  // 新增
}
```

### 添加新执行引擎

```rust
// 1. 实现ExecutionEngine trait
struct MyCustomEngine {
    // ...
}

impl ExecutionEngine<IRBlock> for MyCustomEngine {
    fn execute_instruction(&mut self, insn: &Instruction)
        -> VmResult<()> { /* ... */ }
    fn run(&mut self, mmu: &mut dyn MMU, block: &IRBlock)
        -> ExecResult { /* ... */ }
    // ...
}

// 2. 在VmConfig中注册
pub enum ExecMode {
    Interpreter,
    JIT,
    HardwareAssisted,
    Custom,  // 新增
}
```

---

**文档版本**: 1.0
**最后更新**: 2025-12-31
**作者**: VM开发团队
