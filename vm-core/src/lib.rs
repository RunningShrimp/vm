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
//! - `使用std`: 启用 no_std 支持，用于嵌入式或受限环境
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

#[cfg(feature = "std")]
extern crate alloc;

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

// 模块定义
pub mod aggregate_root;
pub mod config;
pub mod constants;
pub mod device_emulation;
pub mod domain;
pub mod domain_event_bus;
pub mod domain_services;
pub mod domain_type_safety;
pub mod error;
pub mod gc;
pub mod gdb;
pub mod macros;
pub mod migration;
pub mod mmu_traits;
pub mod runtime;
pub mod snapshot;
pub mod syscall;
pub mod template;
pub mod value_objects;
pub mod vm_state;

// Merged modules (from vm-common and vm-foundation)
pub mod common;
pub mod foundation;

// 重新导出系统调用相关类型
pub use syscall::SyscallResult;

// Re-export the new MMU trait and its sub-traits from mmu_traits
pub use mmu_traits::{AddressTranslator, MMU, MemoryAccess, MmioManager, MmuAsAny};
mod regs;

// Re-export ExecutionError, VmError, CoreError and MemoryError
pub use error::IntoVmError;
pub use error::{
    CoreError, DeviceError, ExecutionError, MemoryError, PlatformError, VmError as CoreVmError,
    VmError,
};

// Re-export config types
pub use config::{Config, ConfigBuilder, ConfigDiff, ConfigError, ConfigVecExt};

// Re-export constants
pub use constants::{DEFAULT_MEMORY_SIZE, MAX_GUEST_MEMORY, PAGE_SIZE};

// Re-export domain types
pub use domain::{ExecutionManager, PageTableWalker, TlbEntry, TlbManager, TlbStats};
pub use domain_type_safety::{GuestAddrExt, GuestPhysAddrExt, PageSize};
pub use value_objects::{DeviceId, MemorySize, PortNumber, VcpuCount, VmId};

// ============================================================================
// 基础类型定义
// ============================================================================

/// 客户机虚拟地址
///
/// 表示Guest操作系统的虚拟地址。这是一个强类型包装器，确保虚拟地址的正确使用。
///
/// # 使用场景
/// - 指令指针：程序计数器(PC)使用GuestAddr
/// - 虚拟内存访问：通过MMU翻译的虚拟地址
/// - 数据指针：Guest数据结构的地址
///
/// # 示例
/// ```
/// use vm_core::GuestAddr;
///
/// // 创建虚拟地址
/// let addr = GuestAddr(0x1000);
///
/// // 地址运算
/// let addr2 = addr + 0x100;  // 0x1100
/// let diff = addr2 - addr;   // 0x100
///
/// // 格式化输出
/// println!("Address: {:x}", addr);  // "Address: 1000"
/// ```
///
/// # 注意
/// - GuestAddr在Bare模式下等同于GuestPhysAddr
/// - 在分页模式下需要通过MMU翻译才能访问物理内存
/// - 支持wrap-around运算，模拟溢出行为
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GuestAddr(pub u64);

impl GuestAddr {
    /// 环绕加法
    ///
    /// 执行带溢出环绕的加法运算，模拟CPU的溢出行为。
    ///
    /// # 参数
    /// - `rhs`: 要加上的值
    ///
    /// # 返回
    /// 加法结果（如果溢出则从0重新开始）
    ///
    /// # 示例
    /// ```
    /// use vm_core::GuestAddr;
    ///
    /// let addr = GuestAddr(0xFFFF_FFFF_FFFF_FFFF);
    /// let result = addr.wrapping_add(1);
    /// assert_eq!(result, GuestAddr(0));
    /// ```
    pub fn wrapping_add(self, rhs: u64) -> Self {
        GuestAddr(self.0.wrapping_add(rhs))
    }

    /// 环绕减法
    ///
    /// 执行带溢出环绕的减法运算。
    ///
    /// # 参数
    /// - `rhs`: 要减去的值
    ///
    /// # 返回
    /// 减法结果（如果溢出则从最大值重新开始）
    pub fn wrapping_sub(self, rhs: u64) -> Self {
        GuestAddr(self.0.wrapping_sub(rhs))
    }

    /// 与GuestAddr的环绕加法
    ///
    /// # 参数
    /// - `rhs`: 要加上的GuestAddr
    pub fn wrapping_add_addr(self, rhs: GuestAddr) -> Self {
        GuestAddr(self.0.wrapping_add(rhs.0))
    }

    /// 与GuestAddr的环绕减法
    ///
    /// # 参数
    /// - `rhs`: 要减去的GuestAddr
    pub fn wrapping_sub_addr(self, rhs: GuestAddr) -> Self {
        GuestAddr(self.0.wrapping_sub(rhs.0))
    }

    /// 转换为i64（用于偏移量计算）
    ///
    /// # 返回
    /// i64表示的地址值，可用于有符号偏移计算
    pub fn as_i64(self) -> i64 {
        self.0 as i64
    }

    /// 获取原始u64值
    ///
    /// # 返回
    /// 地址的原始u64表示
    pub fn value(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for GuestAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "0x{:x}", self.0)
    }
}

/// 访问类型
///
/// 定义内存访问的类型，用于MMU权限检查和TLB标签。
///
/// # 使用场景
/// - MMU翻译：根据访问类型检查页表权限位
/// - TLB查找：区分ITLB(指令)和DTLB(数据)
/// - 权限验证：R/W/X权限位验证
///
/// # 示例
/// ```
/// use vm_core::AccessType;
///
/// // 读取访问
/// let access = AccessType::Read;
///
/// // 写入访问
/// let access = AccessType::Write;
///
/// // 执行访问（指令获取）
/// let access = AccessType::Execute;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    /// 读取访问
    ///
    /// 检查页表R(Read)位
    Read,
    /// 写入访问
    ///
    /// 检查页表W(Write)位和D(Dirty)位
    Write,
    /// 执行访问
    ///
    /// 检查页表X(Execute)位
    Execute,
    /// 原子操作
    ///
    /// 用于LR/SC等原子指令
    Atomic,
}

/// 故障/异常类型
///
/// 表示虚拟机执行过程中可能发生的各种故障和异常。
/// 这些故障通常由MMU、执行引擎或解码器生成。
///
/// # 使用场景
/// - MMU故障：页面缺失、权限违规
/// - 执行故障：对齐错误、非法指令
/// - 系统调用：Guest请求的异常处理
/// - 中断处理：外部设备和定时器中断
///
/// # 示例
/// ```
/// use vm_core::{Fault, GuestAddr, AccessType};
///
/// // 页面故障
/// let fault = Fault::PageFault {
///     addr: GuestAddr(0x1000),
///     access_type: AccessType::Read,
///     is_write: false,
///     is_user: true,
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// 页面故障
    ///
    /// 虚拟地址无法翻译到物理地址，或者权限不足。
    ///
    /// # 字段
    /// - `addr`: 触发故障的虚拟地址
    /// - `access_type`: 访问类型（读/写/执行）
    /// - `is_write`: 是否是写操作
    /// - `is_user`: 是否是用户模式访问
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
    ///
    /// 通用的保护违规，如权限不足、特权指令等
    GeneralProtection,
    /// 段故障
    ///
    /// 段描述符相关错误（主要用于x86架构）
    SegmentFault,
    /// 对齐故障
    ///
    /// 访问未对齐的内存地址
    AlignmentFault,
    /// 总线错误
    ///
    /// 物理内存访问失败，如访问无效的物理地址
    BusError,
    /// 无效操作码
    ///
    /// 解码器无法识别的指令
    ///
    /// # 字段
    /// - `pc`: 指令地址
    /// - `opcode`: 原始操作码
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
///
/// 表示Guest操作系统的物理地址。这是Guest视角的物理地址，
/// 在虚拟化环境中可能对应Host的虚拟地址。
///
/// # 使用场景
/// - 页表项：页表项存储的是Guest物理地址
/// - 物理内存访问：直接访问物理内存区域
/// - MMIO映射：设备寄存器的物理地址
///
/// # 注意
/// - GuestPhysAddr在虚拟化环境中可能不是真正的Host物理地址
/// - 需要通过EPT/NPT等机制进行二次翻译
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuestPhysAddr(pub u64);

/// 主机地址
///
/// Host视角的地址，通常是Host虚拟地址。
///
/// # 使用场景
/// - 内存映射：Guest物理内存映射到Host地址空间
/// - JIT编译：JIT代码缓冲区的Host地址
/// - 设备映射：MMIO设备在Host的映射地址
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
///
/// 支持的Guest架构类型。
///
/// # 使用场景
/// - 虚拟机配置：指定Guest的CPU架构
/// - 解码器选择：根据架构选择对应的指令解码器
/// - 系统调用：不同架构的系统调用ABI
///
/// # 示例
/// ```
/// use vm_core::GuestArch;
///
/// // 创建RISC-V 64位虚拟机
/// let arch = GuestArch::Riscv64;
/// println!("Architecture: {}", arch.name()); // "riscv64"
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Encode, Decode)]
pub enum GuestArch {
    /// RISC-V 64位架构
    ///
    /// 支持RV64I基础指令集和M/A/F/D/C扩展
    Riscv64,
    /// ARM 64位架构
    ///
    /// 支持AArch64指令集
    Arm64,
    /// x86-64架构
    ///
    /// 支持AMD64指令集
    X86_64,
    /// PowerPC 64位架构
    ///
    /// 支持PowerPC 64位指令集
    PowerPC64,
}

impl GuestArch {
    /// 获取架构名称
    ///
    /// # 返回
    /// 架构的小写字符串名称，如"riscv64"、"arm64"等
    pub fn name(&self) -> &'static str {
        match self {
            GuestArch::Riscv64 => "riscv64",
            GuestArch::Arm64 => "arm64",
            GuestArch::X86_64 => "x86_64",
            GuestArch::PowerPC64 => "powerpc64",
        }
    }
}

impl std::fmt::Display for GuestArch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// 虚拟机配置
///
/// 定义虚拟机的核心配置参数。
///
/// # 使用场景
/// - 虚拟机创建：初始化虚拟机实例
/// - 资源分配：内存大小、CPU数量
/// - 执行模式：选择解释器、JIT或硬件辅助虚拟化
///
/// # 示例
/// ```
/// use vm_core::{VmConfig, GuestArch, ExecMode};
///
/// let config = VmConfig {
///     guest_arch: GuestArch::Riscv64,
///     memory_size: 128 * 1024 * 1024, // 128MB
///     vcpu_count: 1,
///     exec_mode: ExecMode::Interpreter,
///     kernel_path: Some("/path/to/kernel".to_string()),
///     initrd_path: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct VmConfig {
    /// 客户机架构
    ///
    /// 指定Guest的CPU架构类型
    pub guest_arch: GuestArch,
    /// 内存大小（字节）
    ///
    /// 分配给Guest的物理内存大小
    pub memory_size: usize,
    /// 虚拟CPU数量
    ///
    /// 创建的虚拟CPU核心数
    pub vcpu_count: usize,
    /// 执行模式
    ///
    /// 指定虚拟机的执行引擎类型
    pub exec_mode: ExecMode,
    /// 内核文件路径
    ///
    /// Guest内核镜像的文件路径（可选）
    pub kernel_path: Option<String>,
    /// 初始化RAM磁盘路径
    ///
    /// initrd镜像的文件路径（可选）
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
///
/// 定义虚拟机的执行引擎类型。
///
/// # 使用场景
/// - 性能调优：根据需求选择执行模式
/// - 调试：解释器模式便于调试
/// - 生产：JIT模式提供接近原生的性能
///
/// # 模式对比
/// - **Interpreter**: 简单、便携、易调试，性能较低
/// - **JIT**: 高性能，需要编译时间，内存占用较大
/// - **HardwareAssisted**: 最高性能，依赖硬件虚拟化支持
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub enum ExecMode {
    /// 解释器模式
    ///
    /// 逐条或逐块解释执行Guest指令。
    ///
    /// # 特点
    /// - 实现简单，便于调试
    /// - 启动快速，无编译开销
    /// - 性能较低，通常为原生的1-5%
    Interpreter,
    /// JIT编译模式
    ///
    /// 即时编译Guest指令为Host机器码并执行。
    ///
    /// # 特点
    /// - 高性能，可达原生的50-80%
    /// - 需要编译时间
    /// - 内存占用较大（代码缓存）
    JIT,
    /// 硬件辅助虚拟化模式
    ///
    /// 利用硬件虚拟化技术（如Intel VT-x、AMD-V）。
    ///
    /// # 特点
    /// - 最高性能，接近原生
    /// - 依赖硬件支持
    /// - 主要用于系统虚拟化
    HardwareAssisted,
}

// VmState struct has been removed - use VirtualMachineState instead
// The old VmState struct conflicted with the VmState enum defined below

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
/// let insn = decoder.decode_insn(&mut mmu, GuestAddr(0x1000))?;
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
    /// - `mmu`: MMU可变引用，用于读取虚拟内存中的指令
    ///   （需要可变性因为translate可能更新TLB）
    /// - `pc`: 程序计数器，指向要解码的指令地址
    ///
    /// # 返回
    /// 解码后的指令对象
    fn decode_insn(&mut self, mmu: &mut dyn MMU, pc: GuestAddr) -> VmResult<Self::Instruction>;

    /// 解码指令块
    ///
    /// 从指定地址解码一个基本块，直到遇到跳转指令或其他终止指令。
    /// 基本块是指只有一个入口和一个出口的指令序列。
    ///
    /// # 参数
    /// - `mmu`: MMU可变引用，用于读取虚拟内存中的指令
    ///   （需要可变性因为translate可能更新TLB）
    /// - `pc`: 程序计数器，指向基本块起始地址
    ///
    /// # 返回
    /// 解码后的基本块对象
    fn decode(&mut self, mmu: &mut dyn MMU, pc: GuestAddr) -> VmResult<Self::Block>;
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

/// vCPU 退出原因
#[derive(Debug)]
pub enum VcpuExit {
    /// Halt 指令
    Halt,
    /// MMIO 访问
    Mmio {
        addr: GuestAddr,
        is_write: bool,
        size: u8,
        data: u64,
    },
    /// I/O 端口访问（x86）
    Io {
        port: u16,
        is_write: bool,
        size: u8,
        data: u32,
    },
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum VmState {
    #[default]
    Created,
    Running,
    Paused,
    Stopped,
}

/// 虚拟机核心结构
#[allow(dead_code)]
pub struct VirtualMachine<B> {
    /// 配置
    config: VmConfig,
    /// 状态
    state: VmState,
    /// MMU（共享访问）
    mmu: Arc<Mutex<Box<dyn MMU>>>,
    /// vCPU 列表
    vcpus: Vec<Arc<Mutex<dyn ExecutionEngine<B>>>>,
    /// 执行统计
    stats: ExecStats,
    /// 快照管理器
    snapshot_manager: Mutex<snapshot::SnapshotMetadataManager>,
    /// 模板管理器
    template_manager: Mutex<template::TemplateManager>,
}

impl<B: 'static> VirtualMachine<B> {
    /// 使用提供的 MMU 创建 VM
    pub fn with_mmu(config: VmConfig, mmu: Box<dyn MMU>) -> Self {
        Self {
            config,
            state: VmState::Created,
            mmu: Arc::new(Mutex::new(mmu)),
            vcpus: Vec::new(),
            stats: ExecStats::default(),
            snapshot_manager: Mutex::new(snapshot::SnapshotMetadataManager::new()),
            template_manager: Mutex::new(template::TemplateManager::new()),
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
    /// CPU寄存器状态
    pub regs: crate::GuestRegs,
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

/// 执行状态
///
/// 表示虚拟机执行的当前状态。
#[derive(Debug, Clone, PartialEq)]
pub enum ExecStatus {
    /// 继续执行
    ///
    /// 虚拟机正常执行中，可以继续执行下一条指令
    Continue,
    /// 执行完成
    ///
    /// 虚拟机正常退出或执行完成
    Ok,
    /// 执行故障
    ///
    /// 执行过程中发生错误
    ///
    /// # 包含错误类型
    Fault(ExecutionError),
    /// IO请求
    ///
    /// Guest发起IO请求，需要Host处理
    IoRequest,
    /// 中断待处理
    ///
    /// 有外部中断等待处理
    InterruptPending,
}

/// 执行统计信息
///
/// 记录虚拟机执行过程中的各种性能指标。
///
/// # 使用场景
/// - 性能分析：统计指令数、内存访问等
/// - TLB效率：监控TLB命中率
/// - JIT优化：统计编译次数和时间
///
/// # 示例
/// ```
/// use vm_core::ExecStats;
///
/// let stats = ExecStats::default();
/// println!("Instructions: {}", stats.executed_insns);
/// println!("TLB hit rate: {:.2}%",
///     stats.tlb_hits as f64 / (stats.tlb_hits + stats.tlb_misses) as f64 * 100.0);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, Encode, Decode)]
pub struct ExecStats {
    /// 已执行的操作数（遗留字段，用于兼容）
    pub executed_ops: u64,
    /// 已执行的指令数
    ///
    /// 统计从开始到当前执行的总指令数
    pub executed_insns: u64,
    /// 内存访问次数
    ///
    /// 统计所有内存读写操作的次数
    pub mem_accesses: u64,
    /// 执行时间（纳秒）
    ///
    /// 累计执行时间，不包括JIT编译时间
    pub exec_time_ns: u64,
    /// TLB命中次数
    ///
    /// TLB查找成功的次数
    pub tlb_hits: u64,
    /// TLB未命中次数
    ///
    /// TLB查找失败，需要页表遍历的次数
    pub tlb_misses: u64,
    /// JIT编译次数
    ///
    /// JIT编译器被调用的次数
    pub jit_compiles: u64,
    /// JIT编译时间（纳秒）
    ///
    /// 累计的JIT编译时间
    pub jit_compile_time_ns: u64,
}

/// 执行结果结构
///
/// 包含执行后的状态、统计信息和下一条指令地址。
///
/// # 字段
/// - `status`: 执行状态（成功/失败/中断等）
/// - `stats`: 执行统计信息
/// - `next_pc`: 下一条要执行的指令地址
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
        let result = addr1.wrapping_sub(1);
        assert_eq!(result, GuestAddr(0xFFFF_FFFF_FFFF_FFFF));
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
