//! vm-core: 核心类型、错误、配置与抽象接口
//! 
//! 支持 no_std 以便在受限环境中使用（通过 feature flag）

#![cfg_attr(feature = "no_std", no_std)]

#[cfg(feature = "no_std")]
extern crate alloc;

#[cfg(feature = "no_std")]
use alloc::{boxed::Box, vec::Vec, string::String, sync::Arc};
#[cfg(not(feature = "no_std"))]
use std::sync::{Arc, Mutex};

// ============================================================================
// 基础类型定义
// ============================================================================

/// Guest 虚拟地址类型
pub type GuestAddr = u64;
/// Guest 物理地址类型  
pub type GuestPhysAddr = u64;
/// Host 虚拟地址类型
pub type HostAddr = usize;

// ============================================================================
// Guest 架构枚举
// ============================================================================

/// 支持的 Guest 架构
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuestArch {
    Riscv64,
    Arm64,
    X86_64,
}

impl GuestArch {
    pub fn name(&self) -> &'static str {
        match self {
            GuestArch::Riscv64 => "riscv64",
            GuestArch::Arm64 => "arm64",
            GuestArch::X86_64 => "x86_64",
        }
    }
}

// ============================================================================
// 执行模式枚举
// ============================================================================

/// 执行引擎模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecMode {
    /// 纯解释执行
    Interpreter,
    /// JIT 编译执行
    Jit,
    /// 硬件虚拟化加速（KVM/HVF/WHPX）
    Accelerated,
    /// 混合模式：解释器 + JIT 热点编译
    Hybrid,
}

// ============================================================================
// 访问类型与错误
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessType {
    Read,
    Write,
    Exec,
}

/// VM 故障类型
#[derive(Debug, Clone)]
pub enum Fault {
    PageFault { addr: GuestAddr, access: AccessType },
    AccessViolation { addr: GuestAddr, access: AccessType },
    InvalidOpcode { pc: GuestAddr, opcode: u32 },
    AlignmentFault { addr: GuestAddr, size: u8 },
    DeviceError { msg: &'static str },
    Halt,
    Shutdown,
}

/// VM 错误类型
#[derive(Debug)]
pub enum VmError {
    /// 配置错误
    Config(String),
    /// 内存映射错误
    Memory(String),
    /// 设备初始化错误
    Device(String),
    /// 执行错误
    Execution(Fault),
    /// 加速器不可用
    AcceleratorUnavailable,
    /// IO 错误
    Io(String),
}

#[cfg(not(feature = "no_std"))]
impl std::fmt::Display for VmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmError::Config(s) => write!(f, "Configuration error: {}", s),
            VmError::Memory(s) => write!(f, "Memory error: {}", s),
            VmError::Device(s) => write!(f, "Device error: {}", s),
            VmError::Execution(fault) => write!(f, "Execution fault: {:?}", fault),
            VmError::AcceleratorUnavailable => write!(f, "Hardware accelerator unavailable"),
            VmError::Io(s) => write!(f, "IO error: {}", s),
        }
    }
}

#[cfg(not(feature = "no_std"))]
impl std::error::Error for VmError {}

pub type VmResult<T> = Result<T, VmError>;

// ============================================================================
// VM 配置
// ============================================================================

/// VirtIO 设备配置
#[derive(Debug, Clone)]
pub struct VirtioConfig {
    /// 块设备镜像路径
    pub block_image: Option<String>,
    /// 网络后端类型
    pub net_mode: Option<NetMode>,
    /// 是否启用控制台
    pub console: bool,
    /// 是否启用 GPU
    pub gpu: bool,
}

impl Default for VirtioConfig {
    fn default() -> Self {
        Self {
            block_image: None,
            net_mode: None,
            console: true,
            gpu: false,
        }
    }
}

/// 网络模式
#[derive(Debug, Clone)]
pub enum NetMode {
    /// 用户态 NAT（smoltcp）
    User,
    /// TAP 桥接
    Tap(String),
}

/// VM 配置结构
#[derive(Debug, Clone)]
pub struct VmConfig {
    /// Guest 架构
    pub guest_arch: GuestArch,
    /// 内存大小（字节）
    pub memory_size: usize,
    /// vCPU 数量
    pub vcpu_count: u32,
    /// 执行模式
    pub exec_mode: ExecMode,
    /// 是否启用硬件加速
    pub enable_accel: bool,
    /// 内核/固件路径
    pub kernel_path: Option<String>,
    /// initrd 路径
    pub initrd_path: Option<String>,
    /// 内核命令行
    pub cmdline: Option<String>,
    /// VirtIO 设备配置
    pub virtio: VirtioConfig,
    /// 是否启用调试跟踪
    pub debug_trace: bool,
    /// JIT 热点阈值
    pub jit_threshold: u32,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            guest_arch: GuestArch::Riscv64,
            memory_size: 128 * 1024 * 1024, // 128MB
            vcpu_count: 1,
            exec_mode: ExecMode::Hybrid,
            enable_accel: true,
            kernel_path: None,
            initrd_path: None,
            cmdline: None,
            virtio: VirtioConfig::default(),
            debug_trace: false,
            jit_threshold: 100,
        }
    }
}

// ============================================================================
// MMU Trait
// ============================================================================

/// MMIO 设备接口
pub trait MmioDevice: Send {
    fn read(&self, offset: u64, size: u8) -> u64;
    fn write(&mut self, offset: u64, val: u64, size: u8);
    fn reset(&mut self) {}
}

/// 内存管理单元 Trait
pub trait MMU: Send {
    /// 地址翻译：GVA -> GPA
    fn translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestPhysAddr, Fault>;
    
    /// 取指令
    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, Fault>;
    
    /// 读内存
    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, Fault>;
    
    /// 写内存
    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), Fault>;
    
    /// 映射 MMIO 设备
    fn map_mmio(&mut self, base: GuestAddr, size: u64, device: Box<dyn MmioDevice>);
    
    /// TLB 刷新
    fn flush_tlb(&mut self);
    
    /// 获取物理内存大小
    fn memory_size(&self) -> usize;
}

// ============================================================================
// 执行状态与结果
// ============================================================================

/// 执行统计
#[derive(Debug, Clone, Default)]
pub struct ExecStats {
    /// 已执行指令数
    pub executed_insns: u64,
    /// 已执行 IR 操作数
    pub executed_ops: u64,
    /// TLB 命中数
    pub tlb_hits: u64,
    /// TLB 缺失数
    pub tlb_misses: u64,
    /// JIT 编译次数
    pub jit_compiles: u64,
    /// JIT 编译耗时（纳秒）
    pub jit_compile_time_ns: u64,
}

/// 执行状态
#[derive(Debug)]
pub enum ExecStatus {
    /// 正常继续
    Continue,
    /// 执行完成
    Ok,
    /// 发生故障
    Fault(Fault),
    /// 需要 I/O
    IoRequest,
    /// 中断待处理
    InterruptPending,
}

/// 执行结果
pub struct ExecResult {
    pub status: ExecStatus,
    pub stats: ExecStats,
    /// 下一条指令 PC
    pub next_pc: GuestAddr,
}

// ============================================================================
// 核心 Trait 定义
// ============================================================================

/// 执行引擎 Trait
pub trait ExecutionEngine<B>: Send {
    /// 执行单个基本块
    fn run(&mut self, mmu: &mut dyn MMU, block: &B) -> ExecResult;
    
    /// 获取寄存器值
    fn get_reg(&self, idx: usize) -> u64;
    
    /// 设置寄存器值
    fn set_reg(&mut self, idx: usize, val: u64);
    
    /// 获取 PC
    fn get_pc(&self) -> GuestAddr;
    
    /// 设置 PC
    fn set_pc(&mut self, pc: GuestAddr);
}

/// 解码器 Trait
pub trait Decoder: Send {
    type Block;
    
    /// 解码指令到 IR 基本块
    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, Fault>;
}

// ============================================================================
// vCPU 状态
// ============================================================================

/// vCPU 运行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcpuState {
    /// 已创建未运行
    Created,
    /// 正在运行
    Running,
    /// 已暂停
    Paused,
    /// 等待 I/O
    WaitingIo,
    /// 已停止
    Stopped,
}

/// vCPU 退出原因
#[derive(Debug)]
pub enum VcpuExit {
    /// Halt 指令
    Halt,
    /// MMIO 访问
    Mmio { addr: GuestAddr, is_write: bool, size: u8, data: u64 },
    /// I/O 端口访问（x86）
    Io { port: u16, is_write: bool, size: u8, data: u32 },
    /// 中断窗口打开
    IrqWindowOpen,
    /// 关机请求
    Shutdown,
    /// 未知退出
    Unknown(i32),
}

// ============================================================================
// VirtualMachine 核心结构
// ============================================================================

/// 虚拟机状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmState {
    Created,
    Running,
    Paused,
    Stopped,
}

/// 虚拟机核心结构
#[cfg(not(feature = "no_std"))]
pub struct VirtualMachine {
    /// 配置
    config: VmConfig,
    /// 状态
    state: VmState,
    /// MMU（共享访问）
    mmu: Arc<Mutex<Box<dyn MMU>>>,
    /// 执行统计
    stats: ExecStats,
}

#[cfg(not(feature = "no_std"))]
impl VirtualMachine {
    /// 使用提供的 MMU 创建 VM
    pub fn with_mmu(config: VmConfig, mmu: Box<dyn MMU>) -> Self {
        Self {
            config,
            state: VmState::Created,
            mmu: Arc::new(Mutex::new(mmu)),
            stats: ExecStats::default(),
        }
    }
    
    /// 获取 MMU 引用
    pub fn mmu(&self) -> Arc<Mutex<Box<dyn MMU>>> {
        Arc::clone(&self.mmu)
    }
    
    /// 获取配置
    pub fn config(&self) -> &VmConfig {
        &self.config
    }
    
    /// 获取 VM 状态
    pub fn state(&self) -> VmState {
        self.state
    }
    
    /// 获取执行统计
    pub fn stats(&self) -> &ExecStats {
        &self.stats
    }
    
    /// 加载内核镜像到内存
    pub fn load_kernel(&mut self, data: &[u8], load_addr: GuestAddr) -> VmResult<()> {
        let mut mmu = self.mmu.lock().map_err(|_| VmError::Memory("MMU lock poisoned".into()))?;
        
        for (i, &byte) in data.iter().enumerate() {
            mmu.write(load_addr + i as u64, byte as u64, 1)
                .map_err(|f| VmError::Execution(f))?;
        }
        
        Ok(())
    }
    
    /// 从文件加载内核
    #[cfg(not(feature = "no_std"))]
    pub fn load_kernel_file(&mut self, path: &str, load_addr: GuestAddr) -> VmResult<()> {
        use std::fs;
        let data = fs::read(path).map_err(|e| VmError::Io(e.to_string()))?;
        self.load_kernel(&data, load_addr)
    }
    
    /// 启动 VM
    pub fn start(&mut self) -> VmResult<()> {
        if self.state != VmState::Created && self.state != VmState::Paused {
            return Err(VmError::Config("VM not in startable state".into()));
        }
        self.state = VmState::Running;
        Ok(())
    }
    
    /// 暂停 VM
    pub fn pause(&mut self) -> VmResult<()> {
        if self.state != VmState::Running {
            return Err(VmError::Config("VM not running".into()));
        }
        self.state = VmState::Paused;
        Ok(())
    }
    
    /// 停止 VM
    pub fn stop(&mut self) -> VmResult<()> {
        self.state = VmState::Stopped;
        Ok(())
    }
    
    /// 重置 VM
    pub fn reset(&mut self) -> VmResult<()> {
        self.state = VmState::Created;
        self.stats = ExecStats::default();
        let mut mmu = self.mmu.lock().map_err(|_| VmError::Memory("MMU lock poisoned".into()))?;
        mmu.flush_tlb();
        Ok(())
    }
}