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
use alloc::{boxed::Box, vec::Vec, string::String, sync::Arc};
#[cfg(not(feature = "no_std"))]
use std::sync::{Arc, Mutex};

// 模块定义
mod regs;
pub mod domain;
pub mod syscall;
pub mod snapshot;
pub mod template;
pub mod migration;
pub mod gdb;
pub use regs::GuestRegs;
pub use domain::{TlbManager, TlbEntry, PageTableWalker, ExecutionManager};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GuestArch {
    Riscv64,
    Arm64,
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

/// 执行引擎模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    DeviceError { msg: String },
    Halt,
    Shutdown,
    TrapRiscv { cause: RiscvTrapCause, pc: GuestAddr },
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

/// VM 配置结构
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
pub trait MmioDevice: Send + Sync {
    fn read(&self, offset: u64, size: u8) -> u64;
    fn write(&mut self, offset: u64, val: u64, size: u8);
    fn notify(&mut self, _mmu: &mut dyn MMU, _offset: u64) {}
    fn poll(&mut self, _mmu: &mut dyn MMU) {}
    fn notify(&mut self, _mmu: &mut dyn MMU, _offset: u64) {}
    fn poll(&mut self, _mmu: &mut dyn MMU) {}
    fn reset(&mut self) {}
}

/// 内存管理单元 Trait
pub trait MMU: Send + 'static {
pub trait MMU: Send + 'static {
    /// 地址翻译：GVA -> GPA
    fn translate(&mut self, va: GuestAddr, access: AccessType) -> Result<GuestPhysAddr, Fault>;
    
    /// 取指令
    fn fetch_insn(&self, pc: GuestAddr) -> Result<u64, Fault>;
    
    /// 读内存
    fn read(&self, pa: GuestAddr, size: u8) -> Result<u64, Fault>;

    /// Load Reserved (LR) - 读取并设置保留位
    fn load_reserved(&mut self, pa: GuestAddr, size: u8) -> Result<u64, Fault> {
        self.read(pa, size)
    }
    
    /// 写内存
    fn write(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<(), Fault>;

    /// Store Conditional (SC) - 条件写入
    fn store_conditional(&mut self, pa: GuestAddr, val: u64, size: u8) -> Result<bool, Fault> {
        // 默认实现总是失败（保守行为）
        let _ = (pa, val, size);
        Ok(false)
    }

    /// 显式清除保留位（供 JIT 或外部调用）
    fn invalidate_reservation(&mut self, _pa: GuestAddr, _size: u8) {}

    /// 批量读内存
    fn read_bulk(&self, pa: GuestAddr, buf: &mut [u8]) -> Result<(), Fault> {
        for (i, byte) in buf.iter_mut().enumerate() {
            *byte = self.read(pa + i as u64, 1)? as u8;
        }
        Ok(())
    }

    /// 批量写内存
    fn write_bulk(&mut self, pa: GuestAddr, buf: &[u8]) -> Result<(), Fault> {
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

/// vCPU 状态的完整表示
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct VcpuStateContainer {
    pub regs: [u64; 32],
    pub pc: GuestAddr,
}

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

    /// 获取完整的 vCPU 状态
    fn get_vcpu_state(&self) -> VcpuStateContainer;

    /// 设置完整的 vCPU 状态
    fn set_vcpu_state(&mut self, state: &VcpuStateContainer);
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
        let mut mmu = self.mmu.lock().map_err(|_| VmError::Memory("MMU lock poisoned".into()))?;
        
        mmu.write_bulk(load_addr, data)
            .map_err(|f| VmError::Execution(f))?;
        mmu.write_bulk(load_addr, data)
            .map_err(|f| VmError::Execution(f))?;
        
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

    /// 创建快照
    pub fn create_snapshot(&mut self, name: String, description: String) -> VmResult<String> {
        let mmu = self.mmu.lock().map_err(|_| VmError::Memory("MMU lock poisoned".into()))?;
        let memory_dump = mmu.dump_memory();
        let id = uuid::Uuid::new_v4().to_string();
        let memory_dump_path = format!("/tmp/{}.memsnap", id);
        std::fs::write(&memory_dump_path, memory_dump).map_err(|e| VmError::Io(e.to_string()))?;

        let mut snapshot_manager = self.snapshot_manager.lock()
            .map_err(|_| VmError::Config("Failed to lock snapshot manager".into()))?;
        let snapshot_id = snapshot_manager.create_snapshot(name, description, memory_dump_path);
        Ok(snapshot_id)
    }

    /// 恢复快照
    pub fn restore_snapshot(&mut self, id: &str) -> VmResult<()> {
        let mut snapshot_manager = self.snapshot_manager.lock()
            .map_err(|_| VmError::Config("Failed to lock snapshot manager".into()))?;
        let snapshot = snapshot_manager.snapshots.get(id).ok_or_else(|| VmError::Config("Snapshot not found".to_string()))?.clone();
        let memory_dump = std::fs::read(&snapshot.memory_dump_path).map_err(|e| VmError::Io(e.to_string()))?;

        let mut mmu = self.mmu.lock().map_err(|_| VmError::Memory("MMU lock poisoned".into()))?;
        mmu.restore_memory(&memory_dump).map_err(VmError::Memory)?;

        snapshot_manager.restore_snapshot(id).map_err(VmError::Config)
    }

    /// 列出所有快照
    pub fn list_snapshots(&self) -> VmResult<Vec<snapshot::Snapshot>> {
        let snapshot_manager = self.snapshot_manager.lock()
            .map_err(|_| VmError::Config("Failed to lock snapshot manager".into()))?;
        Ok(snapshot_manager.get_snapshot_tree().into_iter().cloned().collect())
    }

    /// 创建模板
    pub fn create_template(&mut self, name: String, description: String, base_snapshot_id: String) -> VmResult<String> {
        let mut template_manager = self.template_manager.lock()
            .map_err(|_| VmError::Config("Failed to lock template manager".into()))?;
        let id = template_manager.create_template(name, description, base_snapshot_id);
        Ok(id)
    }

    /// 列出所有模板
    pub fn list_templates(&self) -> VmResult<Vec<template::VmTemplate>> {
        let template_manager = self.template_manager.lock()
            .map_err(|_| VmError::Config("Failed to lock template manager".into()))?;
        Ok(template_manager.list_templates().into_iter().cloned().collect())
    }

    /// 序列化虚拟机状态以进行迁移
    pub fn serialize_state(&self) -> VmResult<Vec<u8>> {
        let mmu = self.mmu.lock().map_err(|_| VmError::Memory("MMU lock poisoned".into()))?;
        let memory_dump = mmu.dump_memory();

        let mut vcpu_states = Vec::new();
        for vcpu in &self.vcpus {
            let vcpu = vcpu.lock()
                .map_err(|_| VmError::Memory("Failed to lock vCPU".into()))?;
            vcpu_states.push(vcpu.get_vcpu_state());
        }

        let state = migration::MigrationState {
            config: self.config.clone(),
            vcpu_states,
            memory_dump,
        };

        bincode::serialize(&state).map_err(|e| VmError::Io(e.to_string()))
    }

    /// 从序列化数据中反序列化并恢复虚拟机状态
    pub fn deserialize_state(&mut self, data: &[u8]) -> VmResult<()> {
        let state: migration::MigrationState = bincode::deserialize(data).map_err(|e| VmError::Io(e.to_string()))?;

        self.config = state.config;

        let mut mmu = self.mmu.lock().map_err(|_| VmError::Memory("MMU lock poisoned".into()))?;
        mmu.restore_memory(&state.memory_dump).map_err(VmError::Memory)?;

        for (i, vcpu_state) in state.vcpu_states.iter().enumerate() {
            if let Some(vcpu) = self.vcpus.get_mut(i) {
                let mut vcpu = vcpu.lock()
                    .map_err(|_| VmError::Memory("Failed to lock vCPU during restore".into()))?;
                vcpu.set_vcpu_state(vcpu_state);
            }
        }

        Ok(())
    }
}
