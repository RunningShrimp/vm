# VM 虚拟机 - API 文档

## 1. vm-core API

### 1.1 基础类型

#### `GuestAddr` 和 `GuestPhysAddr`
```rust
pub type GuestAddr = u64;          // 虚拟地址
pub type GuestPhysAddr = u64;      // 物理地址
pub type HostAddr = u64;           // 主机地址
```

#### `GuestArch` 枚举
```rust
pub enum GuestArch {
    Riscv64,
    Arm64,
    X86_64,
}

impl GuestArch {
    pub fn bits(&self) -> u8;  // 返回架构位数
    pub fn register_count(&self) -> usize;  // 通用寄存器数
}
```

#### `ExecMode` 枚举
```rust
pub enum ExecMode {
    Interpreter,    // 解释执行
    JIT,            // JIT 编译执行
    Accelerated,    // 硬件加速
    Hybrid,         // 混合模式
}
```

### 1.2 错误类型

#### `VmError`
```rust
pub enum VmError {
    InvalidAddress(GuestAddr),
    MemoryAccess { addr: GuestAddr, reason: String },
    ExecutionError(String),
    InvalidInstruction(u64),
    DeviceError(String),
    // ...
}

impl Display for VmError { ... }
impl Error for VmError { ... }
```

#### `Result<T>`
```rust
pub type Result<T> = std::result::Result<T, VmError>;
```

### 1.3 配置结构体

#### `VmConfig`
```rust
pub struct VmConfig {
    pub arch: GuestArch,
    pub exec_mode: ExecMode,
    pub memory_size: u64,           // 虚拟机内存大小 (字节)
    pub num_vcpu: u32,              // vCPU 数量
    pub enable_jit: bool,
    pub jit_threshold: u64,         // 编译热度阈值
    pub tlb_size: usize,            // TLB 条目数
    // ...
}

impl VmConfig {
    pub fn new(arch: GuestArch) -> Self;
    pub fn with_exec_mode(mut self, mode: ExecMode) -> Self;
    pub fn with_memory_size(mut self, size: u64) -> Self;
}
```

### 1.4 核心 Trait

#### `MMU` Trait
```rust
pub trait MMU {
    /// 读内存
    fn read(&self, addr: GuestAddr, size: u8) -> Result<u64>;
    
    /// 写内存
    fn write(&mut self, addr: GuestAddr, val: u64, size: u8) -> Result<()>;
    
    /// 批量读 (优化版本)
    fn read_bulk(&self, addr: GuestAddr, buf: &mut [u8]) -> Result<()>;
    
    /// 批量写 (优化版本)
    fn write_bulk(&mut self, addr: GuestAddr, buf: &[u8]) -> Result<()>;
    
    /// TLB 刷新
    fn flush_tlb(&mut self, addr: Option<GuestAddr>);
    
    /// 设置页表基地址
    fn set_page_table_base(&mut self, base: GuestAddr);
}
```

#### `ExecutionEngine<B>` Trait
```rust
pub trait ExecutionEngine<B: Backend> {
    /// 执行一个基本块
    fn execute_block(&mut self, block: &IRBlock) -> Result<()>;
    
    /// 运行直到退出
    fn run(&mut self, exit_reason: Box<dyn ExitCondition>) -> Result<ExitReason>;
    
    /// 读通用寄存器
    fn read_reg(&self, idx: u8) -> u64;
    
    /// 写通用寄存器
    fn write_reg(&mut self, idx: u8, val: u64);
    
    /// 读浮点寄存器
    fn read_freg(&self, idx: u8) -> f64;
    
    /// 写浮点寄存器
    fn write_freg(&mut self, idx: u8, val: f64);
}
```

#### `Decoder` Trait
```rust
pub trait Decoder {
    type Instruction: Instruction;
    
    /// 解码一条指令
    fn decode_insn(
        &mut self,
        mmu: &dyn MMU,
        pc: GuestAddr,
    ) -> Result<Option<Self::Instruction>>;
}
```

#### `Instruction` Trait
```rust
pub trait Instruction {
    /// 下一条指令 PC
    fn next_pc(&self) -> u64;
    
    /// 指令大小 (字节)
    fn size(&self) -> u8;
    
    /// 是否是分支指令
    fn is_branch(&self) -> bool;
    
    /// 是否是跳转指令
    fn is_jump(&self) -> bool;
    
    /// 是否是陷入指令
    fn is_trap(&self) -> bool;
}
```

#### `MmioDevice` Trait
```rust
pub trait MmioDevice: Send + Sync {
    /// 读操作
    fn read(&self, offset: u64, size: u8) -> u64;
    
    /// 写操作
    fn write(&mut self, offset: u64, val: u64, size: u8);
    
    /// 处理中断
    fn interrupt(&self) -> Option<u32>;
}
```

### 1.5 主虚拟机类

#### `VirtualMachine<B>`
```rust
pub struct VirtualMachine<B> {
    config: VmConfig,
    // ...
}

impl<B> VirtualMachine<B> {
    /// 创建新虚拟机
    pub fn new(config: VmConfig) -> Result<Self>;
    
    /// 启动虚拟机
    pub fn boot(&mut self) -> Result<()>;
    
    /// 停止虚拟机
    pub fn stop(&mut self);
    
    /// 虚拟机是否运行中
    pub fn is_running(&self) -> bool;
}
```

---

## 2. vm-mem API

### 2.1 SoftMmu 结构

#### 创建和配置
```rust
impl SoftMmu {
    /// 创建新 MMU
    pub fn new(
        phys_mem_size: u64,
        paging_mode: PagingMode,
    ) -> Result<Self>;
    
    /// 启用 JIT 编译
    pub fn enable_jit_mode(&mut self);
    
    /// 禁用 JIT 编译
    pub fn disable_jit_mode(&mut self);
}
```

#### 内存访问
```rust
impl MMU for SoftMmu {
    fn read(&self, addr: GuestAddr, size: u8) -> Result<u64>;
    fn write(&mut self, addr: GuestAddr, val: u64, size: u8) -> Result<()>;
    
    /// 原子操作 - 比较交换
    pub fn atomic_cas(
        &mut self,
        addr: GuestAddr,
        expected: u64,
        new: u64,
    ) -> Result<u64>;
    
    /// 原子操作 - 加载预留
    pub fn atomic_lr(&mut self, addr: GuestAddr) -> Result<u64>;
    
    /// 原子操作 - 条件存储
    pub fn atomic_sc(
        &mut self,
        addr: GuestAddr,
        val: u64,
    ) -> Result<bool>;
}
```

### 2.2 TLB 管理

#### TlbEntry 结构
```rust
pub struct TlbEntry {
    pub vpn: u64,           // 虚拟页号
    pub ppn: u64,           // 物理页号
    pub asid: u16,          // 地址空间 ID
    pub valid: bool,
    pub writable: bool,
    pub executable: bool,
    pub user: bool,
    pub cacheable: bool,
}
```

#### TLB 查询
```rust
impl SoftMmu {
    /// 查询 TLB
    pub fn tlb_lookup(
        &self,
        vpn: u64,
        asid: u16,
    ) -> Option<&TlbEntry>;
    
    /// 插入 TLB 条目
    pub fn tlb_insert(&mut self, entry: TlbEntry);
    
    /// 刷新整个 TLB
    pub fn flush_tlb_all(&mut self);
    
    /// 刷新指定地址的 TLB 条目
    pub fn flush_tlb_addr(&mut self, addr: GuestAddr);
    
    /// 获取 TLB 统计信息
    pub fn tlb_stats(&self) -> TlbStats;
}

pub struct TlbStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub hit_rate: f64,
}
```

### 2.3 页表管理

#### 分页模式
```rust
pub enum PagingMode {
    None,                   // 无分页
    Sv39,                   // RISC-V Sv39
    Sv48,                   // RISC-V Sv48
    AArch64Upa,            // ARM64 统一页面地址转换
    LongMode64,            // x86-64 长模式
}
```

#### 页表遍历
```rust
impl SoftMmu {
    /// 遍历页表
    pub fn walk_page_table(
        &self,
        vaddr: GuestAddr,
        write: bool,
    ) -> Result<TlbEntry>;
    
    /// 获取物理地址
    pub fn virt_to_phys(
        &self,
        vaddr: GuestAddr,
    ) -> Result<GuestPhysAddr>;
}
```

---

## 3. vm-engine-interpreter API

### 3.1 Interpreter 结构

#### 创建和运行
```rust
impl Interpreter {
    /// 创建新解释器
    pub fn new(config: &VmConfig) -> Result<Self>;
    
    /// 启用块缓存
    pub fn enable_block_cache(&mut self, max_size: usize);
    
    /// 禁用块缓存
    pub fn disable_block_cache(&mut self);
}
```

#### 执行控制
```rust
impl ExecutionEngine for Interpreter {
    fn execute_block(&mut self, block: &IRBlock) -> Result<()>;
    
    fn run(&mut self, cond: Box<dyn ExitCondition>) -> Result<ExitReason>;
}
```

### 3.2 块缓存 API

#### BlockCache 结构
```rust
pub struct BlockCache {
    cache: HashMap<GuestAddr, CachedBlock>,
    max_size: usize,
    hits: u64,
    misses: u64,
}

pub struct CachedBlock {
    pub block: IRBlock,
    pub exec_count: u64,
    pub last_exec: Instant,
}
```

#### 缓存操作
```rust
impl BlockCache {
    /// 查询块缓存
    pub fn get(&self, addr: GuestAddr) -> Option<&IRBlock>;
    
    /// 插入块缓存
    pub fn insert(&mut self, addr: GuestAddr, block: IRBlock);
    
    /// 获取缓存统计
    pub fn stats(&self) -> CacheStats;
    
    /// 清空缓存
    pub fn clear(&mut self);
}

pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub size: usize,
}
```

### 3.3 指令融合 API

#### InstructionFuser 结构
```rust
pub struct InstructionFuser {
    fused_count: u64,
    patterns: Vec<FusedPattern>,
}

pub enum FusedOp {
    LoadAdd { reg: u8, offset: i32 },
    AddStore { reg: u8, offset: i32 },
    CmpBranch { reg: u8, target: u64 },
    // ...
}
```

#### 融合操作
```rust
impl InstructionFuser {
    /// 尝试融合指令
    pub fn try_fuse(
        &self,
        prev: &Instruction,
        curr: &Instruction,
    ) -> Option<FusedOp>;
    
    /// 获取融合统计
    pub fn stats(&self) -> FuserStats;
}

pub struct FuserStats {
    pub fused_count: u64,
    pub total_checked: u64,
    pub fusion_rate: f64,
}
```

---

## 4. vm-engine-jit API

### 4.1 JIT 结构

#### 创建和配置
```rust
impl Jit {
    /// 创建新 JIT 编译器
    pub fn new(config: &VmConfig) -> Result<Self>;
    
    /// 设置编译阈值
    pub fn set_compile_threshold(&mut self, threshold: u64);
    
    /// 启用自适应阈值调整
    pub fn enable_adaptive_threshold(&mut self);
}
```

#### 编译和缓存
```rust
impl ExecutionEngine for Jit {
    fn execute_block(&mut self, block: &IRBlock) -> Result<()>;
    
    fn run(&mut self, cond: Box<dyn ExitCondition>) -> Result<ExitReason>;
}

impl Jit {
    /// 编译 IR 块到本机代码
    pub fn compile(&mut self, block: &IRBlock) -> Result<*const u8>;
    
    /// 获取已编译代码
    pub fn get_compiled(&self, pc: GuestAddr) -> Option<*const u8>;
    
    /// 清空编译缓存
    pub fn flush_code_cache(&mut self);
    
    /// 获取编译统计
    pub fn compile_stats(&self) -> JitStats;
}

pub struct JitStats {
    pub blocks_compiled: u64,
    pub total_compile_time_ms: f64,
    pub code_cache_size: usize,
    pub avg_compile_time_ms: f64,
}
```

### 4.2 热点检测 API

#### AdaptiveThreshold 结构
```rust
pub struct AdaptiveThreshold {
    current: u64,
    min: u64,
    max: u64,
    history: VecDeque<u64>,
}

impl AdaptiveThreshold {
    /// 更新热度计数
    pub fn update(&mut self, count: u64);
    
    /// 检查是否应编译
    pub fn should_compile(&self) -> bool;
    
    /// 调整阈值
    pub fn adjust(&mut self);
}
```

### 4.3 浮点操作 API

#### 浮点寄存器访问
```rust
impl Jit {
    /// 读浮点寄存器
    pub fn read_freg(&self, idx: u8) -> f64;
    
    /// 写浮点寄存器
    pub fn write_freg(&mut self, idx: u8, val: f64);
    
    /// 读单精度浮点
    pub fn read_freg_f32(&self, idx: u8) -> f32;
    
    /// 写单精度浮点
    pub fn write_freg_f32(&mut self, idx: u8, val: f32);
}
```

#### 支持的浮点操作
```rust
// 双精度 (F64)
Fadd, Fsub, Fmul, Fdiv, Fsqrt, Fmin, Fmax

// 单精度 (F32)
FaddS, FsubS, FmulS, FdivS, FsqrtS, FminS, FmaxS

// 转换
F64ToF32, F32ToF64, F64ToI64, I64ToF64
```

---

## 5. vm-device API

### 5.1 VirtIO Block

#### VirtioBlock 结构
```rust
pub struct VirtioBlock {
    file: Arc<Mutex<DiskFile>>,
    capacity: u64,
    sector_size: u32,
}

impl VirtioBlock {
    /// 创建块设备
    pub fn new(file_path: &str) -> Result<Self>;
    
    /// 获取容量
    pub fn capacity(&self) -> u64;
    
    /// 设置为只读
    pub fn set_readonly(&mut self, readonly: bool);
}
```

#### MMIO 接口
```rust
impl MmioDevice for VirtioBlockMmio {
    fn read(&self, offset: u64, size: u8) -> u64;
    fn write(&mut self, offset: u64, val: u64, size: u8);
}
```

### 5.2 中断控制器

#### CLINT (RISC-V)
```rust
pub struct Clint {
    mtimecmp: [u64; 32],
    mtime: u64,
}

impl MmioDevice for Clint {
    fn read(&self, offset: u64, size: u8) -> u64;
    fn write(&mut self, offset: u64, val: u64, size: u8);
}
```

#### PLIC (RISC-V)
```rust
pub struct Plic {
    priorities: Vec<u32>,
    enabled: Vec<u32>,
    claimed: u32,
}

impl MmioDevice for Plic {
    fn read(&self, offset: u64, size: u8) -> u64;
    fn write(&mut self, offset: u64, val: u64, size: u8);
}
```

### 5.3 通用 MMIO 设备接口

#### 设备注册
```rust
pub trait MmioDevice: Send + Sync {
    fn read(&self, offset: u64, size: u8) -> u64;
    fn write(&mut self, offset: u64, val: u64, size: u8);
    fn interrupt(&self) -> Option<u32>;
}

pub struct MmioMap {
    devices: BTreeMap<GuestAddr, Box<dyn MmioDevice>>,
}

impl MmioMap {
    pub fn register(
        &mut self,
        base: GuestAddr,
        size: u64,
        device: Box<dyn MmioDevice>,
    );
    
    pub fn unregister(&mut self, base: GuestAddr);
    
    pub fn read(&self, addr: GuestAddr, size: u8) -> Option<u64>;
    
    pub fn write(&mut self, addr: GuestAddr, val: u64, size: u8) -> bool;
}
```

---

## 6. vm-service API

### 6.1 VmService 主接口

#### 创建虚拟机
```rust
#[async_trait]
impl VmService {
    /// 创建新虚拟机服务
    pub async fn new(
        config: VmConfig,
        kernel: Vec<u8>,
        initrd: Option<Vec<u8>>,
    ) -> Result<Self>;
    
    /// 启动虚拟机
    pub async fn start(&mut self) -> Result<()>;
    
    /// 停止虚拟机
    pub async fn stop(&mut self) -> Result<()>;
    
    /// 暂停虚拟机
    pub async fn pause(&mut self) -> Result<()>;
    
    /// 恢复虚拟机
    pub async fn resume(&mut self) -> Result<()>;
}
```

#### 查询状态
```rust
impl VmService {
    /// 获取虚拟机状态
    pub fn state(&self) -> VmState;
    
    /// 是否运行中
    pub fn is_running(&self) -> bool;
    
    /// 获取性能统计
    pub fn stats(&self) -> PerformanceStats;
}

pub struct PerformanceStats {
    pub instructions_executed: u64,
    pub jit_blocks_compiled: u64,
    pub tlb_hit_rate: f64,
    pub cache_hit_rate: f64,
    pub uptime_ms: u64,
}
```

### 6.2 设备操作

#### 添加设备
```rust
impl VmService {
    /// 添加块设备
    pub fn add_block_device(
        &mut self,
        name: &str,
        file_path: &str,
    ) -> Result<()>;
    
    /// 添加网络设备
    pub fn add_network_device(
        &mut self,
        name: &str,
        mac: [u8; 6],
    ) -> Result<()>;
    
    /// 移除设备
    pub fn remove_device(&mut self, name: &str) -> Result<()>;
}
```

### 6.3 调试接口

#### GDB 支持
```rust
impl VmService {
    /// 启用 GDB 调试 (TCP 127.0.0.1:port)
    pub fn enable_gdb_debugging(&mut self, port: u16) -> Result<()>;
    
    /// 禁用 GDB 调试
    pub fn disable_gdb_debugging(&mut self);
}
```

---

## 7. 错误处理指南

### 常见错误

#### `InvalidAddress`
```rust
// 原因: 尝试访问无效的虚拟地址
// 解决: 检查页表和 TLB 配置

if let Err(VmError::InvalidAddress(addr)) = mmu.read(addr, 8) {
    eprintln!("Invalid address: 0x{:x}", addr);
}
```

#### `MemoryAccess`
```rust
// 原因: 内存访问权限问题或 MMIO 超时
// 解决: 检查页面权限或设备状态

if let Err(VmError::MemoryAccess { addr, reason }) = mmu.write(addr, val, 8) {
    eprintln!("Memory access failed: {} at 0x{:x}", reason, addr);
}
```

#### `ExecutionError`
```rust
// 原因: 执行引擎错误（非法指令等）
// 解决: 检查指令流和执行状态

if let Err(VmError::ExecutionError(msg)) = engine.execute_block(block) {
    eprintln!("Execution error: {}", msg);
}
```

---

## 8. 使用示例

### 完整虚拟机启动

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // 1. 创建配置
    let config = VmConfig::new(GuestArch::Riscv64)
        .with_exec_mode(ExecMode::JIT)
        .with_memory_size(1024 * 1024 * 1024)  // 1 GB
        .with_vcpu_count(4);
    
    // 2. 加载内核
    let kernel = std::fs::read("vmlinux")?;
    
    // 3. 创建虚拟机
    let mut vm = VmService::new(config, kernel, None).await?;
    
    // 4. 添加设备
    vm.add_block_device("root", "disk.img")?;
    
    // 5. 启动虚拟机
    vm.start().await?;
    
    // 6. 监控执行
    loop {
        std::thread::sleep(Duration::from_secs(1));
        
        let stats = vm.stats();
        println!("Executed {} instructions", stats.instructions_executed);
        
        if !vm.is_running() {
            break;
        }
    }
    
    // 7. 清理
    vm.stop().await?;
    Ok(())
}
```

---

**最后更新**: 2025年11月29日
**版本**: 1.0
