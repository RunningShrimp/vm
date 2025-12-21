//! # vm-core - 虚拟机核心库
//!
//! 提供虚拟机的核心类型定义、Trait抽象和基础设施。
//!
//! ## 主要组件
//!
//! - **类型定义**: [`GuestAddr`], [`GuestPhysAddr`], [`HostAddr`] 等地址类型
//! - **架构支持**: [`GuestArch`] 枚举支持 RISC-V64, ARM64, x86_64
//! - **执行抽象**: [`ExecutionEngine`] trait 定义执行引擎接口
//! - **内存管理**: [`MMU`] trait 定义内存管理单元接口
//! - **解码器**: [`Decoder`] trait 定义指令解码器接口
//! - **调试支持**: [`gdb`] 模块提供 GDB 远程调试协议实现
//! - **代码生成**: [`TargetArch`] 枚举定义目标架构
//!
//! ## 特性标志
//!
//! - `no_std`: 启用 no_std 支持，用于嵌入式或受限环境
//!
//! ## 示例
//!
//! ```rust,ignore
//! use vm_core::{GuestArch, VmConfig, ExecMode};
//!
//! let config = VmConfig {
//!     guest_arch: GuestArch::Riscv64,
//!     memory_size: 128 * 1024 * 1024, // 128MB
//!     vcpu_count: 1,
//!     exec_mode: ExecMode::Interpreter,
//!     ..Default::default()
//! };
//! ```

#![cfg_attr(feature = "no_std", no_std)]

#[cfg(feature = "no_std")]
extern crate alloc;

#[cfg(feature = "no_std")]
use alloc::{string::String, vec::Vec};

use serde::{Deserialize, Serialize};

// 模块定义
pub mod domain;
#[cfg(not(feature = "no_std"))]
pub mod domain_event_bus;
#[cfg(not(feature = "no_std"))]
pub mod domain_events;
pub mod encoding;
pub mod error;
#[cfg(not(feature = "no_std"))]
pub mod gdb;
pub mod memory_access;
pub mod migration;
pub mod mmu_traits;
pub mod parallel;
pub mod register;
pub use parallel::*;
#[cfg(not(feature = "no_std"))]
pub mod snapshot;
#[cfg(not(feature = "no_std"))]
mod snapshot_legacy;
#[cfg(not(feature = "no_std"))]
pub mod syscall;
#[cfg(not(feature = "no_std"))]
pub mod template;
#[cfg(not(feature = "no_std"))]
pub mod vm_state;

// 重新导出系统调用相关类型
#[cfg(not(feature = "no_std"))]
pub use syscall::SyscallResult;

// Re-export the new MMU trait and its sub-traits from mmu_traits
pub use mmu_traits::{AddressTranslator, MMU, MemoryAccess, MmioManager, MmuAsAny};
mod regs;

// Re-export ExecutionError, VmError and CoreError
pub use error::{
    CoreError, DeviceError, ExecutionError, MemoryError, PlatformError, VmError as CoreVmError,
    VmError,
};

// Re-export domain types
pub use domain::{ExecutionManager, PageTableWalker, TlbEntry, TlbManager, TlbStats};

// Re-export VirtualMachine type alias
pub use vm_state::VirtualMachine;

// ============================================================================
// 基础类型定义
// ============================================================================

/// 客户机虚拟地址
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    bincode::Encode,
    bincode::Decode,
)]
pub struct GuestAddr(pub u64);

impl GuestAddr {
    /// Wrapping addition
    pub fn wrapping_add(self, rhs: u64) -> Self {
        GuestAddr(self.0.wrapping_add(rhs))
    }

    /// Wrapping subtraction
    pub fn wrapping_sub(self, rhs: GuestAddr) -> u64 {
        self.0.wrapping_sub(rhs.0)
    }
}

/// 访问类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    /// 读取
    Read,
    /// 写入
    Write,
    /// 执行
    Execute,
    /// 原子操作
    Atomic,
}

/// 目标架构枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetArch {
    /// x86-64架构
    X86_64,
    /// AArch64 (ARM64)架构
    AArch64,
    /// RISC-V 64位架构
    RiscV64,
}

#[allow(clippy::derivable_impls)]
impl Default for TargetArch {
    fn default() -> Self {
        // 根据主机架构返回对应的目标架构
        #[cfg(target_arch = "x86_64")]
        {
            TargetArch::X86_64
        }

        #[cfg(target_arch = "aarch64")]
        {
            TargetArch::AArch64
        }

        #[cfg(target_arch = "riscv64")]
        {
            TargetArch::RiscV64
        }

        #[cfg(not(any(
            target_arch = "x86_64",
            target_arch = "aarch64",
            target_arch = "riscv64"
        )))]
        {
            TargetArch::X86_64
        }
    }
}

impl core::fmt::Display for TargetArch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TargetArch::X86_64 => write!(f, "x86_64"),
            TargetArch::AArch64 => write!(f, "aarch64"),
            TargetArch::RiscV64 => write!(f, "riscv64"),
        }
    }
}

/// 故障/异常类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// 页面故障
    PageFault {
        /// 访问地址
        addr: GuestAddr,
        /// 访问类型
        access_type: AccessType,
        /// 是否是写操作
        is_write: bool,
        /// 是否是用户模式
        is_user: bool,
    },
    /// 一般保护故障
    GeneralProtection,
    /// 段故障
    SegmentFault,
    /// 对齐故障
    AlignmentFault,
    /// 总线错误
    BusError,
    /// 无效操作码
    InvalidOpcode {
        /// 指令地址
        pc: GuestAddr,
        /// 操作码
        opcode: u32,
    },
}

/// BitAnd implementation for GuestAddr
impl core::ops::BitAnd<u64> for GuestAddr {
    type Output = u64;

    fn bitand(self, rhs: u64) -> Self::Output {
        self.0 & rhs
    }
}

/// BitAnd implementation for &GuestAddr
impl core::ops::BitAnd<u64> for &GuestAddr {
    type Output = u64;

    fn bitand(self, rhs: u64) -> Self::Output {
        self.0 & rhs
    }
}

impl core::ops::Rem<u64> for GuestAddr {
    type Output = u64;

    fn rem(self, rhs: u64) -> Self::Output {
        self.0 % rhs
    }
}

impl core::ops::Sub for GuestAddr {
    type Output = u64;

    fn sub(self, rhs: GuestAddr) -> Self::Output {
        self.0 - rhs.0
    }
}

impl GuestPhysAddr {
    /// 转换为GuestAddr
    pub fn to_guest_addr(self) -> GuestAddr {
        GuestAddr(self.0)
    }
}

impl From<GuestPhysAddr> for GuestAddr {
    fn from(addr: GuestPhysAddr) -> Self {
        GuestAddr(addr.0)
    }
}

impl From<GuestAddr> for GuestPhysAddr {
    fn from(addr: GuestAddr) -> Self {
        GuestPhysAddr(addr.0)
    }
}

impl core::fmt::LowerHex for GuestAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl core::ops::Add<u64> for GuestAddr {
    type Output = GuestAddr;

    fn add(self, rhs: u64) -> Self::Output {
        GuestAddr(self.0 + rhs)
    }
}

impl core::ops::AddAssign<u64> for GuestAddr {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl core::ops::Shr<u32> for GuestAddr {
    type Output = u64;

    fn shr(self, rhs: u32) -> Self::Output {
        self.0 >> rhs
    }
}

/// 客户机物理地址
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuestPhysAddr(pub u64);

/// 主机地址
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostAddr(pub u64);

impl core::ops::Add<u64> for GuestPhysAddr {
    type Output = GuestPhysAddr;

    fn add(self, rhs: u64) -> Self::Output {
        GuestPhysAddr(self.0 + rhs)
    }
}

impl core::ops::AddAssign<u64> for GuestPhysAddr {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl core::ops::Shr<u64> for GuestPhysAddr {
    type Output = u64;

    fn shr(self, rhs: u64) -> Self::Output {
        self.0 >> rhs
    }
}

/// 客户机架构枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GuestArch {
    /// RISC-V 64位架构
    Riscv64,
    /// ARM 64位架构
    Arm64,
    /// x86-64架构
    X86_64,
}

impl GuestArch {
    /// 返回架构的名称字符串
    pub fn name(&self) -> &'static str {
        match self {
            GuestArch::Riscv64 => "riscv64",
            GuestArch::Arm64 => "arm64",
            GuestArch::X86_64 => "x86_64",
        }
    }
}

/// 虚拟机配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    /// 客户机架构
    pub guest_arch: GuestArch,
    /// 内存大小（字节）
    pub memory_size: usize,
    /// 虚拟CPU数量
    pub vcpu_count: usize,
    /// 执行模式
    pub exec_mode: ExecMode,
    /// 内核文件路径
    pub kernel_path: Option<String>,
    /// 初始化RAM磁盘路径
    pub initrd_path: Option<String>,
    /// AOT配置
    pub aot: AotConfig,
}

/// AOT 配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AotConfig {
    /// 是否启用 AOT
    pub enable_aot: bool,
    /// AOT 镜像路径
    pub aot_image_path: Option<String>,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            guest_arch: GuestArch::Riscv64,
            memory_size: 128 * 1024 * 1024, // 128MB
            vcpu_count: 1,
            exec_mode: ExecMode::Interpreter,
            kernel_path: None,
            initrd_path: None,
            aot: AotConfig::default(),
        }
    }
}

/// 执行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecMode {
    /// 解释器模式
    Interpreter,
    /// JIT编译模式
    JIT,
    /// 硬件辅助虚拟化模式
    HardwareAssisted,
}

/// 虚拟机状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmState {
    /// 寄存器状态
    pub regs: GuestRegs,
    /// 内存状态
    pub memory: Vec<u8>,
    /// 程序计数器
    pub pc: GuestAddr,
}

impl Default for VmState {
    fn default() -> Self {
        Self {
            regs: GuestRegs::default(),
            memory: Vec::new(),
            pc: GuestAddr(0),
        }
    }
}

/// 虚拟机结果类型
pub type VmResult<T> = Result<T, VmError>;

/// 指令解码器trait
pub trait Decoder {
    type Instruction;
    type Block;

    /// 解码单条指令
    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> VmResult<Self::Instruction>;

    /// 解码指令块
    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> VmResult<Self::Block>;
}

/// 指令结构
#[derive(Debug, Clone)]
pub struct Instruction {
    /// 操作码
    pub opcode: u8,
    /// 操作数
    pub operands: Vec<u64>,
    /// 指令长度（字节）
    pub length: usize,
}

/// 执行引擎trait
pub trait ExecutionEngine<BlockType>: Send + Sync {
    /// 执行单条指令
    fn execute_instruction(&mut self, instruction: &Instruction) -> VmResult<()>;

    /// 运行虚拟机
    fn run(&mut self, mmu: &mut dyn MMU, block: &BlockType) -> ExecResult;

    /// 获取指定编号的寄存器值
    fn get_reg(&self, idx: usize) -> u64;

    /// 设置指定编号的寄存器值
    fn set_reg(&mut self, idx: usize, val: u64);

    /// 获取程序计数器（PC）
    fn get_pc(&self) -> GuestAddr;

    /// 设置程序计数器（PC）
    fn set_pc(&mut self, pc: GuestAddr);

    /// 获取VCPU状态
    fn get_vcpu_state(&self) -> VcpuStateContainer;

    /// 设置VCPU状态
    fn set_vcpu_state(&mut self, state: &VcpuStateContainer);
}

/// MMIO设备trait
pub trait MmioDevice: Send + Sync {
    /// 读取MMIO寄存器
    fn read(&self, offset: u64, size: u8) -> VmResult<u64>;

    /// 写入MMIO寄存器
    fn write(&mut self, offset: u64, value: u64, size: u8) -> VmResult<()>;
}

/// 系统调用上下文
#[derive(Debug, Clone)]
pub struct SyscallContext {
    /// 系统调用号
    pub syscall_no: u64,
    /// 参数
    pub args: [u64; 6],
    /// 返回值
    pub ret: i64,
    /// 错误码
    pub errno: i64,
    /// brk地址
    pub brk_addr: GuestAddr,
}

impl Default for SyscallContext {
    fn default() -> Self {
        Self {
            syscall_no: 0,
            args: [0; 6],
            ret: 0,
            errno: 0,
            brk_addr: GuestAddr(0),
        }
    }
}

/// VCPU状态容器
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VcpuStateContainer {
    /// VCPU ID
    pub vcpu_id: usize,
    /// VCPU状态
    pub state: VmState,
    /// 是否运行中
    pub running: bool,
}

pub use regs::GuestRegs;

/// 虚拟机生命周期状态枚举
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VmLifecycleState {
    /// 创建完成
    Created,
    /// 运行中
    Running,
    /// 暂停
    Paused,
    /// 停止
    Stopped,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExecStatus {
    /// 继续执行
    Continue,
    /// 执行完成
    Ok,
    /// 执行故障
    Fault(ExecutionError),
    /// IO请求
    IoRequest,
    /// 中断待处理
    InterruptPending,
}

/// 执行统计信息
#[derive(Debug, Clone, Default)]
pub struct ExecStats {
    /// 已执行的指令数
    pub executed_ops: u64,
    /// 已执行的指令数（用于兼容）
    pub executed_insns: u64,
    /// 内存访问次数
    pub mem_accesses: u64,
    /// 执行时间（纳秒）
    pub exec_time_ns: u64,
    /// TLB命中次数
    pub tlb_hits: u64,
    /// TLB未命中次数
    pub tlb_misses: u64,
    /// JIT编译次数
    pub jit_compiles: u64,
    /// JIT编译时间（纳秒）
    pub jit_compile_time_ns: u64,
}

/// 执行结果结构
#[derive(Debug, Clone)]
pub struct ExecResult {
    /// 执行状态
    pub status: ExecStatus,
    /// 执行统计信息
    pub stats: ExecStats,
    /// 下一条指令的程序计数器
    pub next_pc: GuestAddr,
}
