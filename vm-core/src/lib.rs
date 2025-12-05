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
#[cfg(not(feature = "no_std"))]
use std::sync::{Arc, Mutex};

// 模块定义
pub mod aggregate_root;
pub mod async_event_bus;
pub mod async_execution_engine;
pub mod async_mmu;
pub mod domain;
pub mod domain_event_bus;
pub mod domain_events;
pub mod event_store;
pub mod gdb;
pub mod lockfree;
pub mod migration;
pub mod mmu_traits;
#[cfg(test)]
pub mod parallel_execution; // 仅用于测试和基准测试
pub mod parallel;
mod regs;
pub mod repository;
pub use repository::{
    AggregateRepository, EventRepository, SnapshotRepository,
    RepositoryFactory, RepositorySuite,
    InMemoryAggregateRepository, InMemoryEventRepository, InMemorySnapshotRepository,
};
pub mod snapshot;
pub mod syscall;
pub mod template;
pub mod tlb_async;
pub mod value_objects;
pub mod vm_state;
#[cfg(feature = "async")]
pub use async_execution_engine::{AsyncExecutionEngine, ExecutionEngineAdapter};
pub use async_mmu::{AsyncMMU, AsyncTLB, TLBEntry, TLBStats};
pub use domain::{ExecutionManager, PageTableWalker, TlbEntry, TlbManager};
pub use lockfree::{LockFreeCounter, LockFreeQueue, LockFreeStack};
pub use parallel::{LoadBalancePolicy, MultiVcpuExecutor, ParallelExecutorConfig, VcpuLoadBalancer};
pub use regs::GuestRegs;
pub use tlb_async::{AsyncTLBCache, AsyncTlbAdapter, ConcurrentTLBManager, TLBCacheStats};

// ============================================================================
// 基础类型定义
// ============================================================================

/// Guest 虚拟地址（GVA）
///
/// 表示虚拟机内部程序看到的虚拟地址。
/// 在有 MMU 的系统中，这个地址会通过页表转换到物理地址。
pub type GuestAddr = u64;

/// Guest 物理地址（GPA）
///
/// 表示虚拟机内部的物理地址（从虚拟机角度）。
/// 这个地址由宿主机的 EPT/IOMMU 再次转换到宿主机物理地址。
pub type GuestPhysAddr = u64;

/// Host 虚拟地址（HVA）
///
/// 表示宿主机进程的虚拟地址。
pub type HostAddr = usize;

// ============================================================================
// Guest 架构枚举
// ============================================================================

/// 支持的 Guest 架构
///
/// 定义虚拟机支持的 ISA（指令集架构），每种架构有对应的前端解码器。
///
/// # 支持的架构
/// - `Riscv64`: RISC-V 64-bit，由 `vm-frontend-riscv64` 支持
/// - `Arm64`: ARM 64-bit (ARMv8)，由 `vm-frontend-arm64` 支持
/// - `X86_64`: x86-64，由 `vm-frontend-x86_64` 支持
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GuestArch {
    /// RISC-V 64-bit 架构
    Riscv64,
    /// ARM 64-bit (ARMv8) 架构
    Arm64,
    /// x86-64 架构
    X86_64,
}

impl GuestArch {
    pub fn name(&self) -> &str {
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

/// 虚拟机执行模式
///
/// 定义虚拟机使用哪种执行引擎来执行客户代码。
///
/// # 模式说明
/// - `Interpreter`: 纯解释执行，性能最低但实现最简单
/// - `Jit`: 仅 JIT 编译执行，需要编译开销但执行快速
/// - `Accelerated`: 使用硬件虚拟化（KVM/HVF/WHPX），性能最好
/// - `Hybrid`: 混合模式，热点代码 JIT 编译，冷代码解释执行
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum ExecMode {
    /// 纯解释执行模式
    #[default]
    Interpreter,
    /// JIT 编译执行模式
    Jit,
    /// 硬件虚拟化加速模式（KVM/HVF/WHPX）
    Accelerated,
    /// 混合模式：解释器 + JIT 热点编译
    Hybrid,
}

// ============================================================================
// 访问类型与错误
// ============================================================================

// ============================================================================
// 访问类型与错误
// ============================================================================

/// 内存访问类型
///
/// 表示对内存进行的操作类型，用于 TLB 和访问控制等。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessType {
    /// 读操作
    Read,
    /// 写操作
    Write,
    /// 执行操作（取指）
    Exec,
}

/// 虚拟机故障/异常类型
///
/// 表示虚拟机在执行过程中可能遇到的各种异常情况。
///
/// # 故障类型
/// - `PageFault`: 页表缺失
/// - `AccessViolation`: 访问权限违反
/// - `InvalidOpcode`: 非法指令
/// - `AlignmentFault`: 对齐错误
/// - `DeviceError`: 设备错误
/// - `Halt`: 主机停止指令
/// - `Shutdown`: 虚拟机关闭
/// - `TrapRiscv`: RISC-V 陷阱
#[derive(Debug, Clone)]
pub enum Fault {
    PageFault {
        addr: GuestAddr,
        access: AccessType,
    },
    AccessViolation {
        addr: GuestAddr,
        access: AccessType,
    },
    InvalidOpcode {
        pc: GuestAddr,
        opcode: u32,
    },
    AlignmentFault {
        addr: GuestAddr,
        size: u8,
    },
    DeviceError {
        msg: String,
    },
    Halt,
    Shutdown,
    TrapRiscv {
        cause: RiscvTrapCause,
        pc: GuestAddr,
    },
}

pub mod error;
pub use aggregate_root::{AggregateRoot, VirtualMachineAggregate};
#[cfg(feature = "async")]
pub use async_event_bus::{AsyncEventBus, AsyncEventBusStats};
pub use domain_event_bus::{DomainEventBus, EventFilter, EventHandler, EventSubscriptionId};
pub use event_store::{EventStore, InMemoryEventStore, StoredEvent};
pub use domain_events::{
    DeviceEvent, DomainEvent, DomainEventEnum, ExecutionEvent, MemoryEvent, SnapshotEvent,
    VmLifecycleEvent,
};
pub use error::{
    CoreError, DeviceError, ErrorContext, ExecutionError, MemoryError, PlatformError, VmError,
};
pub use repository::{InMemoryVmStateRepository, VmStateRepository, VmStateSnapshot};
pub use value_objects::{DeviceId, MemorySize, PortNumber, VcpuCount, VmId};

pub type VmResult<T> = Result<T, VmError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiscvTrapCause {
    InstructionAddressMisaligned,
    InstructionAccessFault,
    IllegalInstruction,
    Breakpoint,
    LoadAddressMisaligned,
    LoadAccessFault,
    StoreAddressMisaligned,
    StoreAccessFault,
    EnvironmentCallFromU,
    EnvironmentCallFromS,
    EnvironmentCallFromM,
    InstructionPageFault,
    LoadPageFault,
    StorePageFault,
    UserSoftwareInterrupt,
    SupervisorSoftwareInterrupt,
    MachineSoftwareInterrupt,
    UserTimerInterrupt,
    SupervisorTimerInterrupt,
    MachineTimerInterrupt,
    UserExternalInterrupt,
    SupervisorExternalInterrupt,
    MachineExternalInterrupt,
}

// ============================================================================
// VM 配置
// ============================================================================

/// VirtIO 设备配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NetMode {
    /// 用户态 NAT（smoltcp）
    User,
    /// TAP 桥接
    Tap(String),
}

/// AOT 编译配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AotConfig {
    /// 是否启用 AOT 编译
    pub enable_aot: bool,
    /// AOT 镜像文件路径（如果提供，将在启动时加载）
    pub aot_image_path: Option<String>,
    /// 是否启用 AOT 优先策略（优先使用 AOT 代码）
    pub aot_priority: bool,
    /// AOT 代码块最小热度阈值（超过此阈值才考虑 AOT 编译）
    pub aot_hotspot_threshold: u32,
}

impl Default for AotConfig {
    fn default() -> Self {
        Self {
            enable_aot: false,
            aot_image_path: None,
            aot_priority: true,
            aot_hotspot_threshold: 1000,
        }
    }
}

/// 异步执行引擎配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AsyncExecutorConfig {
    /// 是否启用异步执行引擎
    pub enable_async_executor: bool,
    /// Yield 间隔（每 N 条指令进行一次 yield，0 表示禁用 yield）
    pub yield_interval: u64,
    /// 是否启用异步 I/O（使用 tokio::fs 等）
    pub enable_async_io: bool,
    /// 异步执行的最大并发任务数
    pub max_concurrent_tasks: usize,
}

impl Default for AsyncExecutorConfig {
    fn default() -> Self {
        Self {
            enable_async_executor: false,
            yield_interval: 1000,
            enable_async_io: true,
            max_concurrent_tasks: 4,
        }
    }
}

/// 虚拟机配置结构
///
/// 包含虚拟机的所有初始化配置参数，如架构、内存大小、执行模式等。
///
/// # 字段说明
/// - `guest_arch`: 客户机架构（RISC-V64/ARM64/x86_64）
/// - `memory_size`: 虚拟机内存大小（字节）
/// - `vcpu_count`: 虚拟 CPU 数量
/// - `exec_mode`: 执行模式（Interpreter/JIT/Accelerated/Hybrid）
/// - `enable_accel`: 是否启用硬件虚拟化加速
/// - `kernel_path`: 内核或 BIOS 文件路径
/// - `initrd_path`: 初始化 RAM 磁盘路径
/// - `cmdline`: 传递给内核的命令行参数
/// - `virtio`: VirtIO 设备配置
/// - `debug_trace`: 是否启用调试跟踪
/// - `jit_threshold`: JIT 编译的热点执行次数阈值
/// - `aot`: AOT 编译配置
/// - `async_executor`: 异步执行引擎配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    /// AOT 编译配置
    pub aot: AotConfig,
    /// 异步执行引擎配置
    pub async_executor: AsyncExecutorConfig,
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
            aot: AotConfig::default(),
            async_executor: AsyncExecutorConfig::default(),
        }
    }
}

// ============================================================================
// 内存与设备接口
// ============================================================================

/// MMIO（内存映射 I/O）设备接口
///
/// 所有内存映射设备都应实现此 trait，以支持虚拟机对其进行读写操作。
pub trait MmioDevice: Send + Sync {
    /// 从设备读取数据
    ///
    /// # 参数
    /// - `offset`: 设备内部偏移地址
    /// - `size`: 读取大小（1/2/4/8 字节）
    fn read(&self, offset: u64, size: u8) -> u64;

    /// 向设备写入数据
    ///
    /// # 参数
    /// - `offset`: 设备内部偏移地址
    /// - `val`: 要写入的值
    /// - `size`: 写入大小（1/2/4/8 字节）
    fn write(&mut self, offset: u64, val: u64, size: u8);

    /// 设备通知（可选）
    fn notify(&mut self, _mmu: &mut dyn MMU, _offset: u64) {}

    /// 轮询操作（可选）
    fn poll(&mut self, _mmu: &mut dyn MMU) {}

    /// 重置设备
    fn reset(&mut self) {}
}

/// 内存管理单元（MMU）Trait
///
/// 负责虚拟地址翻译、内存读写、指令取指等核心功能。
/// 每种架构可有不同的 MMU 实现（如带 TLB 的 SoftMmu）。
pub trait MMU: Send + 'static {
    /// 虚拟地址翻译
    ///
    /// 将虚拟地址（GVA）翻译到物理地址（GPA）。
    /// 这通常涉及 TLB 查找或页表遍历。
    fn translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestPhysAddr, VmError>;

    /// 从给定 PC 取出指令
    ///
    /// 自动处理地址翻译和访问控制。
    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, VmError>;

    /// 从给定物理地址读取内存
    ///
    /// # 参数
    /// - `pa`: 物理地址
    /// - `size`: 读取大小（1/2/4/8 字节）
    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, VmError>;

    /// 原子性的读取与保留（LR 指令）
    ///
    /// 用于原子操作的实现，通常配合 store_conditional 使用。
    fn load_reserved(&mut self, pa: GuestAddr, size: u8) -> Result<u64, VmError> {
        self.read(pa, size)
    }

    /// 向给定物理地址写入内存
    ///
    /// # 参数
    /// - `pa`: 物理地址
    /// - `val`: 要写入的值
    /// - `size`: 写入大小（1/2/4/8 字节）
    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), VmError>;

    /// 条件存储（SC 指令）
    ///
    /// 用于原子操作，仅在之前 load_reserved 的地址未被修改时写入。
    /// 返回 true 表示成功，false 表示失败。
    fn store_conditional(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<bool, VmError> {
        // 默认实现总是失败（保守行为）
        let _ = (pa, val, size);
        Ok(false)
    }

    /// 清除保留位
    ///
    /// 当 LR 地址被其他 CPU 修改或其他情况下调用，用于清除保留状态。
    fn invalidate_reservation(&mut self, _pa: GuestAddr, _size: u8) {}

    /// 批量读内存
    fn read_bulk(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), VmError> {
        for (i, byte) in buf.iter_mut().enumerate() {
            *byte = self.read(pa + i as u64, 1)? as u8;
        }
        Ok(())
    }

    /// 批量写内存
    fn write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), VmError> {
        for (i, &byte) in buf.iter().enumerate() {
            self.write(pa + i as u64, byte as u64, 1)?;
        }
        Ok(())
    }

    /// 映射 MMIO 设备
    fn map_mmio(&mut self, base: GuestAddr, size: u64, device: Box<dyn MmioDevice>);

    /// TLB 刷新
    fn flush_tlb(&mut self);

    /// 获取物理内存大小
    fn memory_size(&self) -> usize;

    /// 转储整个物理内存内容
    fn dump_memory(&self) -> Vec<u8>;

    /// 从转储中恢复物理内存
    fn restore_memory(&mut self, data: &[u8]) -> Result<(), String>;
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// 设备轮询（用于异步 I/O 驱动）
    fn poll_devices(&mut self) {}
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
    Fault(VmError),
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
// 核心执行引擎与指令接口
// ============================================================================

/// vCPU 状态完整表示
///
/// 包含虚拟 CPU 的所有寄存器和程序计数器状态，用于状态保存和恢复。
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct VcpuStateContainer {
    /// 通用寄存器数组（32个）
    pub regs: [u64; 32],
    /// 程序计数器
    pub pc: GuestAddr,
}

/// 执行引擎接口
///
/// 所有执行模式（Interpreter/JIT/Accelerated/Hybrid）都应实现此 trait。
/// 负责实际执行客户指令并管理 vCPU 状态。
///
/// # 类型参数
/// - `B`: 基本块类型（通常是 IRBlock）
pub trait ExecutionEngine<B>: Send {
    /// 执行单个基本块
    ///
    /// # 参数
    /// - `mmu`: 内存管理单元
    /// - `block`: 要执行的基本块
    fn run(&mut self, mmu: &mut dyn MMU, block: &B) -> ExecResult;

    /// 获取指定编号的寄存器值
    fn get_reg(&self, idx: usize) -> u64;

    /// 设置指定编号的寄存器值
    fn set_reg(&mut self, idx: usize, val: u64);

    /// 获取程序计数器（PC）
    fn get_pc(&self) -> GuestAddr;

    /// 设置程序计数器（PC）
    fn set_pc(&mut self, pc: GuestAddr);

    /// 获取完整的 vCPU 状态
    fn get_vcpu_state(&self) -> VcpuStateContainer;

    /// 设置完整的 vCPU 状态
    fn set_vcpu_state(&mut self, state: &VcpuStateContainer);
}

/// 统一的指令接口
///
/// 所有前端（x86_64/ARM64/RISC-V64）解码器都应产生实现此 trait 的指令类型。
/// 此 trait 提供了统一的接口来访问解码后的指令信息。
pub trait Instruction: Send + Sync {
    /// 获取指令执行后的下一个地址
    ///
    /// 等于 PC + instruction_size，用于顺序执行时的下一条指令地址。
    fn next_pc(&self) -> GuestAddr;

    /// 获取指令长度（字节数）
    ///
    /// - x86_64: 1-15 字节（变长编码）
    /// - ARM64: 4 字节（固定）
    /// - RISC-V: 2 或 4 字节
    fn size(&self) -> u8;

    /// 获取操作数数量
    fn operand_count(&self) -> usize;

    /// 获取操作码/助记符的字符串表示
    ///
    /// 例如："mov", "add", "jmp" 等
    fn mnemonic(&self) -> &str;

    /// 是否是控制流指令（分支/跳转/调用）
    ///
    /// 用于 JIT 的热点追踪和基本块切割
    fn is_control_flow(&self) -> bool;

    /// 是否是内存访问指令
    ///
    /// 用于优化和内存访问追踪
    fn is_memory_access(&self) -> bool;
}

/// 指令解码器Trait
///
/// 所有架构的解码器都应实现此 trait，负责将机器码解码为指令或 IR 中间表示。
/// 支持的实现包括：
/// - `X86Decoder`: x86-64 架构
/// - `Arm64Decoder`: ARM 64-bit (ARMv8) 架构
/// - `RiscvDecoder`: RISC-V 64-bit 架构
pub trait Decoder: Send {
    /// 关联的指令类型
    ///
    /// 每个解码器产生的指令类型，需实现 Instruction trait
    type Instruction: Instruction;

    /// 关联的基本块类型
    ///
    /// 通常是 `IRBlock`（中间表示的基本块）
    type Block;

    /// 解码单个指令
    ///
    /// 从给定的 PC 地址处解码单个指令，返回解码后的指令对象。
    ///
    /// # 参数
    /// - `mmu`: 内存管理单元，用于取指
    /// - `pc`: 指令地址（Program Counter）
    ///
    /// # 返回
    /// - `Ok(Instruction)`: 成功解码的指令
    /// - `Err(VmError)`: 解码过程中发生的故障（如非法指令）
    fn decode_insn(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Instruction, VmError>;

    /// 解码基本块
    ///
    /// 从给定的 PC 地址处开始解码一个完整的基本块，通常返回 IR 中间表示。
    /// 基本块在以下情况结束：
    /// - 遇到无条件跳转/分支
    /// - 遇到系统调用/异常
    /// - 达到指定的最大指令数
    ///
    /// # 参数
    /// - `mmu`: 内存管理单元，用于取指
    /// - `pc`: 基本块的起始地址
    ///
    /// # 返回
    /// - `Ok(Block)`: 成功解码的基本块
    /// - `Err(VmError)`: 解码过程中发生的故障
    fn decode(&mut self, mmu: &dyn MMU, pc: GuestAddr) -> Result<Self::Block, VmError>;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VmState {
    Created,
    Running,
    Paused,
    Stopped,
}

/// 虚拟机核心结构
#[cfg(not(feature = "no_std"))]
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
    snapshot_manager: Mutex<snapshot::SnapshotManager>,
    /// 模板管理器
    template_manager: Mutex<template::TemplateManager>,
}

#[cfg(not(feature = "no_std"))]
impl<B: 'static> VirtualMachine<B> {
    /// 使用提供的 MMU 创建 VM
    pub fn with_mmu(config: VmConfig, mmu: Box<dyn MMU>) -> Self {
        Self {
            config,
            state: VmState::Created,
            mmu: Arc::new(Mutex::new(mmu)),
            vcpus: Vec::new(),
            stats: ExecStats::default(),
            snapshot_manager: Mutex::new(snapshot::SnapshotManager::new()),
            template_manager: Mutex::new(template::TemplateManager::new()),
        }
    }

    pub fn add_vcpu(&mut self, vcpu: Arc<Mutex<dyn ExecutionEngine<B>>>) {
        self.vcpus.push(vcpu);
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
        let mut mmu = self.mmu.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire MMU lock".to_string(),
            })
        })?;

        mmu.write_bulk(load_addr, data)
            .map_err(|f| VmError::from(f))?;

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
            return Err(VmError::Core(CoreError::Config {
                message: "VM not in startable state".to_string(),
                path: None,
            }));
        }
        self.state = VmState::Running;
        Ok(())
    }

    /// 暂停 VM
    pub fn pause(&mut self) -> VmResult<()> {
        if self.state != VmState::Running {
            return Err(VmError::Core(CoreError::Config {
                message: "VM not running".to_string(),
                path: None,
            }));
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
        let mut mmu = self.mmu.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire MMU lock".to_string(),
            })
        })?;
        mmu.flush_tlb();
        Ok(())
    }

    /// 创建快照
    pub fn create_snapshot(&mut self, name: String, description: String) -> VmResult<String> {
        let mmu = self.mmu.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire MMU lock".to_string(),
            })
        })?;
        let memory_dump = mmu.dump_memory();
        let id = uuid::Uuid::new_v4().to_string();
        let memory_dump_path = format!("/tmp/{}.memsnap", id);
        std::fs::write(&memory_dump_path, memory_dump).map_err(|e| VmError::Io(e.to_string()))?;

        let mut snapshot_manager = self.snapshot_manager.lock().map_err(|_| {
            VmError::Core(CoreError::Internal {
                message: "Failed to lock snapshot manager".to_string(),
                module: "VirtualMachine".to_string(),
            })
        })?;
        let snapshot_id = snapshot_manager.create_snapshot(name, description, memory_dump_path);
        Ok(snapshot_id)
    }

    /// 恢复快照
    pub fn restore_snapshot(&mut self, id: &str) -> VmResult<()> {
        let mut snapshot_manager = self.snapshot_manager.lock().map_err(|_| {
            VmError::Core(CoreError::Internal {
                message: "Failed to lock snapshot manager".to_string(),
                module: "VirtualMachine".to_string(),
            })
        })?;
        let snapshot = snapshot_manager
            .snapshots
            .get(id)
            .ok_or_else(|| {
                VmError::Core(CoreError::Config {
                    message: "Snapshot not found".to_string(),
                    path: None,
                })
            })?
            .clone();
        let memory_dump =
            std::fs::read(&snapshot.memory_dump_path).map_err(|e| VmError::Io(e.to_string()))?;

        let mut mmu = self.mmu.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire MMU lock".to_string(),
            })
        })?;
        mmu.restore_memory(&memory_dump).map_err(|s| {
            VmError::Memory(MemoryError::MappingFailed {
                message: s,
                src: None,
                dst: None,
            })
        })?;

        snapshot_manager.restore_snapshot(id).map_err(|s| {
            VmError::Core(CoreError::Config {
                message: s,
                path: None,
            })
        })
    }

    /// 列出所有快照
    pub fn list_snapshots(&self) -> VmResult<Vec<snapshot::Snapshot>> {
        let snapshot_manager = self.snapshot_manager.lock().map_err(|_| {
            VmError::Core(CoreError::Internal {
                message: "Failed to lock snapshot manager".to_string(),
                module: "VirtualMachine".to_string(),
            })
        })?;
        Ok(snapshot_manager
            .get_snapshot_tree()
            .into_iter()
            .cloned()
            .collect())
    }

    /// 创建模板
    pub fn create_template(
        &mut self,
        name: String,
        description: String,
        base_snapshot_id: String,
    ) -> VmResult<String> {
        let mut template_manager = self.template_manager.lock().map_err(|_| {
            VmError::Core(CoreError::Internal {
                message: "Failed to lock template manager".to_string(),
                module: "VirtualMachine".to_string(),
            })
        })?;
        let id = template_manager.create_template(name, description, base_snapshot_id);
        Ok(id)
    }

    /// 列出所有模板
    pub fn list_templates(&self) -> VmResult<Vec<template::VmTemplate>> {
        let template_manager = self.template_manager.lock().map_err(|_| {
            VmError::Core(CoreError::Internal {
                message: "Failed to lock template manager".to_string(),
                module: "VirtualMachine".to_string(),
            })
        })?;
        Ok(template_manager
            .list_templates()
            .into_iter()
            .cloned()
            .collect())
    }

    /// 序列化虚拟机状态以进行迁移
    pub fn serialize_state(&self) -> VmResult<Vec<u8>> {
        let mmu = self.mmu.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire MMU lock".to_string(),
            })
        })?;
        let memory_dump = mmu.dump_memory();

        let mut vcpu_states = Vec::new();
        for vcpu in &self.vcpus {
            let vcpu = vcpu.lock().map_err(|_| {
                VmError::Core(CoreError::Internal {
                    message: "Failed to lock vCPU".to_string(),
                    module: "VirtualMachine".to_string(),
                })
            })?;
            vcpu_states.push(vcpu.get_vcpu_state());
        }

        let state = migration::MigrationState {
            config: self.config.clone(),
            vcpu_states,
            memory_dump,
        };

        // bincode 2.x removed the convenience `serialize`/`deserialize` helpers.
        // Use the serde helpers and an explicit configuration instead.
        bincode::serde::encode_to_vec(&state, bincode::config::standard()).map_err(|e| {
            VmError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()).to_string())
        })
    }

    /// 从序列化数据中反序列化并恢复虚拟机状态
    pub fn deserialize_state(&mut self, data: &[u8]) -> VmResult<()> {
        // decode_from_slice returns (T, usize) where usize is the number of bytes consumed.
        let (state, _): (migration::MigrationState, usize) =
            bincode::serde::decode_from_slice(data, bincode::config::standard()).map_err(|e| {
                VmError::Io(
                    std::io::Error::new(std::io::ErrorKind::Other, e.to_string()).to_string(),
                )
            })?;

        self.config = state.config;

        let mut mmu = self.mmu.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire MMU lock".to_string(),
            })
        })?;
        mmu.restore_memory(&state.memory_dump).map_err(|s| {
            VmError::Memory(MemoryError::MappingFailed {
                message: s,
                src: None,
                dst: None,
            })
        })?;

        for (i, vcpu_state) in state.vcpu_states.iter().enumerate() {
            if let Some(vcpu) = self.vcpus.get_mut(i) {
                let mut vcpu = vcpu.lock().map_err(|_| {
                    VmError::Core(CoreError::Internal {
                        message: "Failed to lock vCPU during restore".to_string(),
                        module: "VirtualMachine".to_string(),
                    })
                })?;
                vcpu.set_vcpu_state(vcpu_state);
            }
        }

        Ok(())
    }
}
