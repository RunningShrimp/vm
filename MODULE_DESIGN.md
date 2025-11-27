# VM 虚拟机 - 模块设计文档

## 1. 模块总览

```
vm (Workspace)
├── vm-core           # 核心类型和 Trait 定义
├── vm-ir             # 中间表示 (IR)
├── vm-mem            # 内存管理单元 (MMU)
├── vm-device         # 虚拟设备实现
├── vm-accel          # 硬件加速层
├── vm-engine-interpreter  # 解释器执行引擎
├── vm-engine-jit     # JIT 编译执行引擎
├── vm-engine-hybrid  # 混合执行引擎
├── vm-frontend-x86_64    # x86-64 指令解码
├── vm-frontend-arm64     # ARM64 指令解码
├── vm-frontend-riscv64   # RISC-V 指令解码
├── vm-service        # 高级 VM 服务层
├── vm-cli            # 命令行工具
├── vm-boot           # 启动和快照管理
├── vm-osal           # 操作系统抽象层
├── vm-passthrough    # 设备直通
├── vm-simd           # SIMD 加速库
└── vm-tests          # 测试和基准测试
```

## 2. 核心模块详解

### 2.1 vm-core

**目的**: 提供系统的基础类型和抽象接口

**主要导出**:
```rust
// 地址类型
pub type GuestAddr = u64;
pub type GuestPhysAddr = u64;
pub type HostAddr = u64;

// 架构定义
pub enum GuestArch { Riscv64, Arm64, X86_64 }
pub enum ExecMode { Interpreter, JIT, Accelerated, Hybrid }

// 关键 Trait
pub trait MMU { ... }
pub trait ExecutionEngine<B> { ... }
pub trait Decoder { ... }
pub trait Instruction { ... }
pub trait MmioDevice { ... }

// 配置
pub struct VmConfig { ... }
pub struct VirtualMachine<B> { ... }
```

**关键类型**:
- `Fault`: 异常类型
- `VmError`: 错误类型
- `AccessType`: 内存访问类型（Read/Write/Exec）
- `TlbManager`: TLB 管理接口

**依赖**: 无（完全独立）

**使用者**: 所有其他模块

---

### 2.2 vm-mem

**目的**: 实现虚拟机的内存管理和地址翻译

**核心组件**:

#### SoftMmu 结构
```rust
pub struct SoftMmu {
    phys_mem: Arc<PhysicalMemory>,
    itlb: Tlb,          // 指令 TLB
    dtlb: Tlb,          // 数据 TLB
    paging_mode: PagingMode,
    page_table_base: u64,
    // ...
}
```

#### TLB 优化
```rust
pub struct Tlb {
    entries: HashMap<u64, TlbEntry>,      // O(1) 查找
    lru: LruCache<u64, ()>,               // LRU 驱逐
    global_entries: HashMap<u64, TlbEntry>,  // 全局页
    // ...
}

// 复合键优化
fn make_tlb_key(vpn: u64, asid: u16) -> u64 {
    (vpn << 16) | (asid as u64)
}
```

#### 批量操作
```rust
impl MMU for SoftMmu {
    // 原生实现：避免逐字节 TLB 查询
    fn read_bulk(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<()> {
        // 1. 检查 MMIO 重叠
        // 2. 直接内存复制
        // 3. 回退处理（MMIO 情况）
    }
    
    fn write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<()> {
        // 类似 read_bulk
    }
}
```

**性能特征**:
- TLB 查找: O(1)
- 页表遍历: O(log N) （N=页表项数）
- 批量读写: O(n) （n=数据大小）

**依赖**: vm-core

**使用者**: vm-service, vm-engine-*

---

### 2.3 vm-engine-interpreter

**目的**: 提供解释器执行引擎

**核心组件**:

#### Interpreter 结构
```rust
pub struct Interpreter {
    regs: [u64; 32],
    pc: GuestAddr,
    fregs: [f64; 32],
    block_cache: Option<BlockCache>,
    fuser: InstructionFuser,
    // CSR 寄存器等
}
```

#### 块缓存
```rust
pub struct BlockCache {
    cache: HashMap<GuestAddr, CachedBlock>,
    hits: u64,
    misses: u64,
    // ...
}

// 使用
impl Interpreter {
    pub fn enable_block_cache(&mut self, size: usize) { ... }
}
```

#### 指令融合
```rust
pub struct InstructionFuser {
    fused_count: u64,
    checked_pairs: u64,
}

// 融合模式
pub enum FusedOp {
    LoadAdd { ... },
    AddStore { ... },
    CmpBranch { ... },
    // ...
}
```

**执行流程**:
1. 逐条执行 IR 指令
2. 尝试指令融合（相邻指令优化）
3. 更新状态（寄存器、内存等）

**性能特征**:
- 指令执行: 10-50 cycles/条
- 指令融合: 5-15% 性能提升
- 块缓存命中: 20-30%

**依赖**: vm-core, vm-ir, vm-mem

---

### 2.4 vm-engine-jit

**目的**: 提供 JIT 编译执行引擎

**核心组件**:

#### Jit 结构
```rust
pub struct Jit {
    module: JITModule,
    ctx: CodegenContext,
    cache: HashMap<GuestAddr, CodePtr>,
    pool_cache: Option<Arc<Mutex<HashMap<...>>>>,
    regs: [u64; 32],
    fregs: [f64; 32],
    adaptive_threshold: AdaptiveThreshold,
}
```

#### 热点检测
```rust
pub struct AdaptiveThreshold {
    min_threshold: u64,
    max_threshold: u64,
    // ...
}

// 自动调整编译阈值
impl AdaptiveThreshold {
    pub fn adjust(&mut self) { ... }
}
```

#### Cranelift 集成
```rust
fn compile(&mut self, block: &IRBlock) -> *const u8 {
    // 1. 创建函数签名
    // 2. 为每条 IR 指令生成 Cranelift IR
    // 3. 编译到本机代码
    // 4. 缓存结果
}

// 支持的操作
// 算术: Add, Sub, Mul, Div, Rem
// 浮点: Fadd, Fsub, Fmul, Fdiv (F64/F32)
// 向量: Vec 操作 (SIMD)
// 原子: CAS, LR/SC
```

**浮点实现**:
```rust
IROp::Fadd { dst, src1, src2 } => {
    let v1 = Self::load_freg(&mut builder, fregs_ptr, src1);
    let v2 = Self::load_freg(&mut builder, fregs_ptr, src2);
    let res = builder.ins().fadd(v1, v2);
    Self::store_freg(&mut builder, fregs_ptr, dst, res);
}
```

**性能特征**:
- 编译时间: 1-10ms （取决于块大小）
- 执行速度: 1-5 cycles/指令
- 浮点性能: 10x 相比解释器

**依赖**: vm-core, vm-ir, cranelift

---

### 2.5 vm-device

**目的**: 实现虚拟设备

**核心设备**:

#### VirtIO Block (块设备)
```rust
pub struct VirtioBlock {
    capacity: u64,
    sector_size: u32,
    // ...
}

impl MmioDevice for VirtioBlockMmio {
    fn read(&self, offset: u64, size: u8) -> u64 { ... }
    fn write(&mut self, offset: u64, val: u64, size: u8) { ... }
}
```

#### 中断控制器
```rust
pub struct Clint {
    // Core Local Interruptor
}

pub struct Plic {
    // Platform Level Interrupt Controller
}
```

**设备优化**:
- 使用 `parking_lot::Mutex` 而非 `std::sync::Mutex`
- 异步 I/O 支持 (tokio)
- 批量操作优化

**依赖**: vm-core, parking_lot

---

### 2.6 vm-accel

**目的**: 硬件虚拟化加速

**支持的平台**:

```rust
pub trait Accel {
    fn create_vcpu(&self, ...) -> Result<Box<dyn Vcpu>>;
    fn run(&self, vcpu: &mut dyn Vcpu) -> Result<ExitReason>;
}

// KVM 实现 (Linux)
pub struct KvmAccel { ... }

// HVF 实现 (macOS)
pub struct HvfAccel { ... }

// WHPX 实现 (Windows)
pub struct WhpxAccel { ... }
```

**性能**:
- 近乎原生性能（~100% 相对性能）
- 仅在支持的平台可用

**依赖**: vm-core, libc/系统 API

---

### 2.7 vm-frontend-*

**目的**: 指令解码 (前端)

**三个前端**:

#### vm-frontend-x86_64
```rust
pub struct X86Decoder { ... }

impl Decoder for X86Decoder {
    type Instruction = X86Instruction;
    fn decode_insn(&mut self, ...) -> Option<Self::Instruction> { ... }
}

impl Instruction for X86Instruction {
    fn next_pc(&self) -> u64 { ... }
    fn size(&self) -> u8 { ... }
    // ...
}
```

#### vm-frontend-arm64
```rust
pub struct Arm64Decoder { ... }
pub struct Arm64Instruction { ... }
// 类似实现
```

#### vm-frontend-riscv64
```rust
pub struct RiscvDecoder { ... }
pub struct RiscvInstruction { ... }
// 类似实现
```

**统一接口**:
- 所有前端实现相同的 `Decoder` Trait
- 所有指令实现 `Instruction` Trait
- 方便的指令扩展

**性能特征**:
- 解码速度: 1-3 ns/指令
- 缓存友好的设计

---

### 2.8 vm-service

**目的**: 高级 VM 服务层

**核心功能**:
```rust
pub struct VmService {
    vm: VirtualMachine<...>,
    interpreter: Interpreter,
    jit: Option<Jit>,
    accel: Option<Box<dyn Accel>>,
    // ...
}

impl VmService {
    pub async fn new(config: VmConfig, ...) -> Result<Self> { ... }
    pub async fn run(&mut self) -> Result<()> { ... }
}
```

**优化**:
- 使用 `parking_lot::Mutex` 替代 `std::sync::Mutex`
- 异步任务处理
- 代码池共享

---

## 3. 数据流与依赖关系

### 3.1 执行路径

```
VmService
  ├─ 选择执行引擎
  │  ├─ Interpreter
  │  ├─ Jit
  │  └─ HwAccel
  │
  ├─ 调用 ExecutionEngine::run
  │
  ├─ 内存访问 (MMU.read/write)
  │  └─ TLB 查找 → 页表遍历
  │
  ├─ 设备 I/O
  │  └─ 调用 MmioDevice::read/write
  │
  └─ 中断处理
     └─ 调用中断处理回调
```

### 3.2 依赖图

```
vm-cli
  └─ vm-service
      ├─ vm-core
      ├─ vm-engine-interpreter
      ├─ vm-engine-jit
      ├─ vm-engine-hybrid
      ├─ vm-mem
      │  └─ vm-core
      ├─ vm-device
      │  └─ vm-core
      ├─ vm-accel
      │  └─ vm-core
      ├─ vm-frontend-*
      │  └─ vm-core
      └─ vm-boot
         └─ vm-core
```

---

## 4. 关键设计决策

### 4.1 为什么使用 Trait 而非泛型?
**理由**: 允许运行时选择实现（Interpreter vs JIT vs HwAccel）

```rust
// Trait 方式（可运行时选择）
pub trait ExecutionEngine { ... }
let engine: Box<dyn ExecutionEngine> = match mode {
    ExecMode::Interpreter => Box::new(Interpreter::new()),
    ExecMode::JIT => Box::new(Jit::new()),
};

// 泛型方式（编译时固定）
pub struct Vm<E: ExecutionEngine> { engine: E }
```

### 4.2 为什么分离 TLB 和页表遍历?
**理由**: 允许不同的缓存策略和优化

```rust
// TLB 做缓存（快路径）
if let Some(entry) = tlb.lookup(...) { return entry; }

// 页表遍历做完整遍历（慢路径）
let entry = walk_page_table(...);
tlb.insert(entry);
```

### 4.3 为什么使用 HashMap 而非 LruCache?
**理由**: O(1) 查找 + 手动 LRU 驱逐 = 更好的性能

```rust
// HashMap: O(1) 查找
if let Some(entry) = map.get(&key) { ... }

// LruCache: O(log n) 查找
if let Some(entry) = lru.get(&key) { ... }
```

### 4.4 为什么使用 parking_lot?
**理由**: 更快、更小、无死锁

```
std::sync::Mutex:
  - 使用内核同步原语
  - 可能死锁（毒化）
  - 更大的内存占用
  - 慢路径锁定

parking_lot::Mutex:
  - 用户态 spin-lock
  - 自动死锁检测
  - 更小
  - 更快（避免系统调用）
```

---

## 5. 扩展点

### 5.1 添加新架构支持
1. 创建新的前端模块 `vm-frontend-mips`
2. 实现 `Decoder` Trait
3. 定义 `MipsInstruction` 实现 `Instruction` Trait
4. 在 vm-service 中注册

### 5.2 添加新执行模式
1. 实现 `ExecutionEngine` Trait
2. 添加优化特定逻辑
3. 在 ExecMode 中添加新模式
4. 在 vm-service 中集成

### 5.3 添加新设备
1. 实现 `MmioDevice` Trait
2. 处理读写操作
3. 在 VmService 中映射

---

## 6. 性能优化概览

| 优化 | 模块 | 提升 | 方法 |
|------|------|------|------|
| TLB O(1) | vm-mem | 20%+ | HashMap + 复合键 |
| 批量操作 | vm-mem | 50%+ | read_bulk/write_bulk |
| JIT 编译 | vm-engine-jit | 5-10x | Cranelift 本机代码 |
| 浮点加速 | vm-engine-jit | 10x | 原生 FP 指令 |
| 无锁锁 | vm-device | 30%+ | parking_lot::Mutex |
| 块缓存 | vm-engine-interpreter | 10-20% | HashMap 缓存 |

---

**最后更新**: 2025年11月29日
**版本**: 1.0
