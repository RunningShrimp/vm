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
use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};

use serde::{Deserialize, Serialize};

// 模块定义
pub mod gdb;
pub mod error;
pub mod mmu_traits;
pub mod domain;
pub mod domain_type_safety;
pub mod value_objects;
pub mod syscall;
pub mod domain_event_bus;
pub mod migration;
pub mod snapshot;
pub mod template;
pub mod vm_state;
pub mod domain_events;
pub mod device_emulation;

// 重新导出系统调用相关类型
pub use syscall::SyscallResult;

// Re-export the new MMU trait and its sub-traits from mmu_traits
pub use mmu_traits::{MMU, AddressTranslator, MemoryAccess, MmioManager, MmuAsAny};
mod regs;

// Re-export ExecutionError, VmError, CoreError and MemoryError
pub use error::{ExecutionError, VmError as CoreVmError, VmError, CoreError, MemoryError, DeviceError, PlatformError};

// Re-export domain types
pub use domain::{TlbManager, TlbEntry, TlbStats, PageTableWalker, ExecutionManager};
pub use domain_type_safety::{GuestAddrExt, GuestPhysAddrExt, PageSize};
pub use value_objects::{MemorySize, VmId, VcpuCount, PortNumber, DeviceId};

// ============================================================================
// 基础类型定义
// ============================================================================

/// 客户机虚拟地址
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
impl std::ops::BitAnd<u64> for GuestAddr {
    type Output = u64;
    
    fn bitand(self, rhs: u64) -> Self::Output {
        self.0 & rhs
    }
}

/// BitAnd implementation for &GuestAddr
impl std::ops::BitAnd<u64> for &GuestAddr {
    type Output = u64;
    
    fn bitand(self, rhs: u64) -> Self::Output {
        self.0 & rhs
    }
}

impl std::ops::Rem<u64> for GuestAddr {
    type Output = u64;
    
    fn rem(self, rhs: u64) -> Self::Output {
        self.0 % rhs
    }
}

impl std::ops::Sub for GuestAddr {
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

impl std::fmt::LowerHex for GuestAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl std::ops::Add<u64> for GuestAddr {
    type Output = GuestAddr;
    
    fn add(self, rhs: u64) -> Self::Output {
        GuestAddr(self.0 + rhs)
    }
}

impl std::ops::AddAssign<u64> for GuestAddr {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl std::ops::Shr<u32> for GuestAddr {
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

impl std::ops::Add<u64> for GuestPhysAddr {
    type Output = GuestPhysAddr;
    
    fn add(self, rhs: u64) -> Self::Output {
        GuestPhysAddr(self.0 + rhs)
    }
}

impl std::ops::AddAssign<u64> for GuestPhysAddr {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl std::ops::Shr<u64> for GuestPhysAddr {
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
    /// PowerPC 64位架构
    PowerPC64,
}

impl GuestArch {
    pub fn name(&self) -> &'static str {
        match self {
            GuestArch::Riscv64 => "riscv64",
            GuestArch::Arm64 => "arm64",
            GuestArch::X86_64 => "x86_64",
            GuestArch::PowerPC64 => "powerpc64",
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
///
/// 负责将二进制机器码解码为可执行的指令表示。这是虚拟机执行流程的第一步，
/// 将目标架构的原始字节码转换为虚拟机内部可处理的指令对象。
///
/// # 使用场景
/// - 解释执行模式：每次执行前解码一条指令
/// - JIT编译模式：解码基本块进行编译优化
/// - 调试器：反汇编和单步调试时使用
/// - 静态分析：二进制代码分析工具
///
/// # 示例
/// ```ignore
/// let mut decoder = X86Decoder::new();
/// let insn = decoder.decode_insn(&mmu, GuestAddr(0x1000))?;
/// ```
pub trait Decoder {
    /// 指令类型关联类型
    type Instruction;
    
    /// 基本块类型关联类型
    type Block;
    
    /// 解码单条指令
    ///
    /// 从指定地址解码一条指令，返回对应的指令表示。
    ///
    /// # 参数
    /// - `mmu`: MMU引用，用于读取虚拟内存中的指令
    /// - `pc`: 程序计数器，指向要解码的指令地址
    ///
    /// # 返回
    /// 解码后的指令对象
    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> VmResult<Self::Instruction>;
    
    /// 解码指令块
    ///
    /// 从指定地址解码一个基本块，直到遇到跳转指令或其他终止指令。
    /// 基本块是指只有一个入口和一个出口的指令序列。
    ///
    /// # 参数
    /// - `mmu`: MMU引用，用于读取虚拟内存中的指令
    /// - `pc`: 程序计数器，指向基本块起始地址
    ///
    /// # 返回
    /// 解码后的基本块对象
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
///
/// 负责执行解码后的指令或基本块，管理vCPU状态（寄存器、PC等）。
/// 这是虚拟机的核心执行组件，支持解释执行和JIT执行两种模式。
///
/// # 使用场景
/// - 解释执行模式：逐条或逐块解释执行指令
/// - JIT执行模式：执行JIT编译生成的机器码
/// - vCPU状态管理：保存和恢复vCPU上下文
/// - 调试器：单步执行、断点、状态检查
///
/// # 示例
/// ```ignore
/// let mut engine = InterpreterEngine::new();
/// engine.run(&mut mmu, &block)?;
/// ```
pub trait ExecutionEngine<BlockType>: Send + Sync {
    /// 执行单条指令
    ///
    /// 解释执行单条指令，更新vCPU状态。
    /// 适用于调试和单步执行场景。
    ///
    /// # 参数
    /// - `instruction`: 要执行的指令
    fn execute_instruction(&mut self, instruction: &Instruction) -> VmResult<()>;
    
    /// 运行虚拟机
    ///
    /// 执行一个基本块或执行上下文，直到遇到终止条件（如系统调用、中断等）。
    ///
    /// # 参数
    /// - `mmu`: 可变MMU引用，用于内存访问
    /// - `block`: 要执行的基本块
    ///
    /// # 返回
    /// 执行结果，包含终止原因和可能的错误
    fn run(&mut self, mmu: &mut dyn MMU, block: &BlockType) -> ExecResult;
    
    /// 获取指定编号的寄存器值
    ///
    /// # 参数
    /// - `idx`: 寄存器编号
    ///
    /// # 返回
    /// 寄存器的当前值
    fn get_reg(&self, idx: usize) -> u64;
    
    /// 设置指定编号的寄存器值
    ///
    /// # 参数
    /// - `idx`: 寄存器编号
    /// - `val`: 要设置的值
    fn set_reg(&mut self, idx: usize, val: u64);
    
    /// 获取程序计数器（PC）
    ///
    /// # 返回
    /// 当前程序计数器值
    fn get_pc(&self) -> GuestAddr;
    
    /// 设置程序计数器（PC）
    ///
    /// # 参数
    /// - `pc`: 新的程序计数器值
    fn set_pc(&mut self, pc: GuestAddr);
    
    /// 获取VCPU状态
    ///
    /// # 返回
    /// vCPU的完整状态容器，包含所有寄存器和控制寄存器
    fn get_vcpu_state(&self) -> VcpuStateContainer;
    
    /// 设置VCPU状态
    ///
    /// # 参数
    /// - `state`: 要设置的vCPU状态
    fn set_vcpu_state(&mut self, state: &VcpuStateContainer);
}



/// MMIO设备trait
///
/// 定义内存映射I/O设备的接口。MMIO设备是通过内存地址映射进行访问的外设，
/// 如UART、PCI设备、VGA显卡等。
///
/// # 使用场景
/// - 外设模拟：UART、PCI设备、网络控制器等
/// - 设备驱动开发：Guest OS驱动程序与设备交互
/// - 调试和测试：模拟特定硬件环境
/// - 虚拟化：半虚拟化设备的实现
///
/// # 示例
/// ```ignore
/// struct UartDevice {
///     tx_data: u8,
///     rx_data: u8,
/// }
///
/// impl MmioDevice for UartDevice {
///     fn read(&self, offset: u64, size: u8) -> VmResult<u64> {
///         match offset {
///             0 => Ok(self.rx_data as u64),
///             _ => Ok(0),
///         }
///     }
///
///     fn write(&mut self, offset: u64, value: u64, size: u8) -> VmResult<()> {
///         match offset {
///             0 => { self.tx_data = value as u8; Ok(()) }
///             _ => Ok(()),
///         }
///     }
/// }
/// ```
pub trait MmioDevice: Send + Sync {
    /// 读取MMIO寄存器
    ///
    /// 从设备指定偏移地址读取数据。
    ///
    /// # 参数
    /// - `offset`: 设备内的偏移地址（字节）
    /// - `size`: 读取大小（1/2/4/8 字节）
    ///
    /// # 返回
    /// 读取的数据值
    fn read(&self, offset: u64, size: u8) -> VmResult<u64>;
    
    /// 写入MMIO寄存器
    ///
    /// 向设备指定偏移地址写入数据。
    ///
    /// # 参数
    /// - `offset`: 设备内的偏移地址（字节）
    /// - `value`: 要写入的值
    /// - `size`: 写入大小（1/2/4/8 字节）
    ///
    /// # 返回
    /// 写入成功返回Ok(())，失败返回错误
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guest_addr_wrapping_add() {
        let addr1 = GuestAddr(0xFFFF_FFFF_FFFF_FFFF);
        let addr2 = addr1.wrapping_add(1);
        assert_eq!(addr2, GuestAddr(0x0000_0000_0000_0000));
    }

    #[test]
    fn test_guest_addr_wrapping_sub() {
        let addr1 = GuestAddr(0x0000_0000_0000_0000);
        let result = addr1.wrapping_sub(GuestAddr(1));
        assert_eq!(result, 0xFFFF_FFFF_FFFF_FFFF);
    }

    #[test]
    fn test_guest_addr_equality() {
        let addr1 = GuestAddr(0x1000);
        let addr2 = GuestAddr(0x1000);
        let addr3 = GuestAddr(0x2000);

        assert_eq!(addr1, addr2);
        assert_ne!(addr1, addr3);
    }

    #[test]
    fn test_guest_addr_ord() {
        let addr1 = GuestAddr(0x1000);
        let addr2 = GuestAddr(0x2000);
        let addr3 = GuestAddr(0x1000);

        assert!(addr1 < addr2);
        assert!(addr2 > addr1);
        assert!(addr1 <= addr3);
    }

    #[test]
    fn test_access_type_variants() {
        let read = AccessType::Read;
        let write = AccessType::Write;
        let execute = AccessType::Execute;
        let atomic = AccessType::Atomic;

        assert_eq!(read, AccessType::Read);
        assert_eq!(write, AccessType::Write);
        assert_eq!(execute, AccessType::Execute);
        assert_eq!(atomic, AccessType::Atomic);
    }
}
