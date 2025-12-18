//! # vm-interface - 虚拟机统一接口规范
//!
//! 提供虚拟机各组件的统一接口定义，遵循SOLID原则，提高代码的可维护性和扩展性。

use serde::{Deserialize, Serialize};
use vm_core::{ExecResult, ExecStats, GuestAddr, VmError};
use vm_ir::IRBlock;

/// 组件状态枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentStatus {
    /// 未初始化
    Uninitialized,
    /// 已初始化
    Initialized,
    /// 正在启动
    Starting,
    /// 运行中
    Running,
    /// 正在停止
    Stopping,
    /// 已停止
    Stopped,
    /// 错误状态
    Error,
}

/// 订阅ID类型
pub type SubscriptionId = u64;

/// 任务ID类型
pub type TaskId = u64;

/// 任务状态
#[derive(Debug, Clone)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// 任务结果
#[derive(Debug)]
pub enum TaskResult {
    Success,
    Failure(VmError),
}

/// 内存序类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryOrder {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

/// 页标志
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFlags(u64);

/// 缓存统计
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

/// 页表统计
#[derive(Debug, Clone, Default)]
pub struct PageStats {
    pub translations: u64,
    pub faults: u64,
    pub flushes: u64,
}

/// 热点统计
#[derive(Debug, Clone, Default)]
pub struct HotStats {
    pub total_executions: u64,
    pub hot_blocks: u64,
    pub compiled_blocks: u64,
}

/// 设备ID类型
pub type DeviceId = u64;

/// 设备类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DeviceType {
    Block,
    Network,
    GPU,
    Input,
    Audio,
    Serial,
    Custom(u32),
}

/// 设备状态
#[derive(Debug, Clone)]
pub enum DeviceStatus {
    Uninitialized,
    Initialized,
    Running,
    Stopped,
    Error(String),
}

/// 事件枚举
#[derive(Debug, Clone)]
pub enum VmEvent {
    ComponentStarted(String),
    ComponentStopped(String),
    ExecutionCompleted(ExecStats),
    MemoryAccess {
        addr: GuestAddr,
        size: u8,
        is_write: bool,
    },
    DeviceInterrupt(DeviceId),
    ErrorOccurred(VmError),
}

// ============================================================================
// 核心Trait定义
// ============================================================================

/// VM组件基础trait，定义生命周期管理
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

/// 配置管理trait
pub trait Configurable {
    type Config;

    /// 更新配置
    fn update_config(&mut self, config: &Self::Config) -> Result<(), VmError>;

    /// 获取当前配置
    fn get_config(&self) -> &Self::Config;

    /// 验证配置
    fn validate_config(config: &Self::Config) -> Result<(), VmError>;
}

/// 状态观察trait
pub trait Observable {
    type State;
    type Event;

    /// 获取当前状态
    fn get_state(&self) -> &Self::State;

    /// 订阅状态变化
    fn subscribe(
        &mut self,
        callback: Box<dyn Fn(&Self::State, &Self::Event) + Send + Sync>,
    ) -> SubscriptionId;

    /// 取消订阅
    fn unsubscribe(&mut self, id: SubscriptionId) -> Result<(), VmError>;
}

// ============================================================================
// 执行引擎接口
// ============================================================================

/// 扩展的执行引擎trait
pub trait ExecutionEngine<I>: VmComponent + Configurable + Observable {
    type State;
    type Stats;

    /// 执行IR块
    fn execute<M: crate::memory::MemoryManager>(&mut self, mmu: &mut M, block: &I) -> ExecResult;

    /// 获取寄存器值
    fn get_register(&self, index: usize) -> u64;

    /// 设置寄存器值
    fn set_register(&mut self, index: usize, value: u64);

    /// 获取程序计数器
    fn get_pc(&self) -> GuestAddr;

    /// 设置程序计数器
    fn set_pc(&mut self, pc: GuestAddr);

    /// 获取执行状态
    fn get_execution_state(&self) -> &<Self as ExecutionEngine<I>>::State;

    /// 获取执行统计
    fn get_execution_stats(&self) -> &Self::Stats;

    /// 重置执行状态
    fn reset(&mut self);

    /// 异步执行版本
    fn execute_async<M: crate::memory::MemoryManager>(
        &mut self,
        mmu: &mut M,
        block: &I,
    ) -> impl std::future::Future<Output = ExecResult> + Send;
}

/// 热编译管理trait（用于JIT和Hybrid引擎）
pub trait HotCompilationManager {
    /// 设置热点阈值
    fn set_hot_threshold(&mut self, min: u64, max: u64);

    /// 获取热点统计
    fn get_hot_stats(&self) -> &HotStats;

    /// 清除热点缓存
    fn clear_hot_cache(&mut self);

    /// 预编译块
    fn precompile_block(&mut self, address: GuestAddr) -> Result<(), VmError>;
}

/// 状态同步trait（用于Hybrid引擎）
pub trait StateSynchronizer {
    /// 从源同步状态到目标
    fn sync_state_to<E: ExecutionEngine<IRBlock>>(&mut self, target: &mut E)
    -> Result<(), VmError>;

    /// 从目标同步状态到源
    fn sync_state_from<E: ExecutionEngine<IRBlock>>(&mut self, source: &E) -> Result<(), VmError>;

    /// 检查状态是否需要同步
    fn needs_sync(&self) -> bool;
}

// ============================================================================
// 内存管理接口
// ============================================================================

/// 统一的内存管理接口
pub trait MemoryManager: VmComponent + Configurable {
    /// 读取内存
    fn read_memory(&self, addr: GuestAddr, size: usize) -> Result<Vec<u8>, VmError>;

    /// 写入内存
    fn write_memory(&mut self, addr: GuestAddr, data: &[u8]) -> Result<(), VmError>;

    /// 原子读取
    fn read_atomic(&self, addr: GuestAddr, size: usize, order: MemoryOrder)
    -> Result<u64, VmError>;

    /// 原子写入
    fn write_atomic(
        &mut self,
        addr: GuestAddr,
        value: u64,
        size: usize,
        order: MemoryOrder,
    ) -> Result<(), VmError>;

    /// 原子比较交换
    fn compare_exchange(
        &mut self,
        addr: GuestAddr,
        expected: u64,
        desired: u64,
        size: usize,
        success: MemoryOrder,
        failure: MemoryOrder,
    ) -> Result<u64, VmError>;

    /// 异步内存操作
    fn read_memory_async(&self, addr: GuestAddr, size: usize) -> impl std::future::Future<Output = Result<Vec<u8>, VmError>> + Send;
    fn write_memory_async(&mut self, addr: GuestAddr, data: Vec<u8>) -> impl std::future::Future<Output = Result<(), VmError>> + Send;
}

/// 缓存管理接口
pub trait CacheManager {
    type Key;
    type Value;

    /// 获取缓存项
    fn get(&self, key: &Self::Key) -> Option<&Self::Value>;

    /// 设置缓存项
    fn set(&mut self, key: Self::Key, value: Self::Value);

    /// 删除缓存项
    fn remove(&mut self, key: &Self::Key) -> Option<Self::Value>;

    /// 清空缓存
    fn clear(&mut self);

    /// 获取缓存统计
    fn get_stats(&self) -> &CacheStats;
}

/// 页表管理接口
pub trait PageTableManager {
    /// 地址翻译
    fn translate(
        &self,
        vaddr: GuestAddr,
        access_type: vm_core::AccessType,
    ) -> Result<vm_core::GuestPhysAddr, VmError>;

    /// 更新页表项
    fn update_entry(
        &mut self,
        vaddr: GuestAddr,
        paddr: vm_core::GuestPhysAddr,
        flags: PageFlags,
    ) -> Result<(), VmError>;

    /// 刷新TLB
    fn flush_tlb(&mut self, vaddr: Option<GuestAddr>);

    /// 获取页表统计
    fn get_page_stats(&self) -> &PageStats;
}

// ============================================================================
// 设备管理接口
// ============================================================================

/// 统一的设备接口
pub trait Device: VmComponent + Configurable + Observable {
    type IoRegion;

    /// 获取设备ID
    fn device_id(&self) -> DeviceId;

    /// 获取设备类型
    fn device_type(&self) -> DeviceType;

    /// 获取I/O区域
    fn io_regions(&self) -> &[Self::IoRegion];

    /// 处理I/O读操作
    fn handle_read(&mut self, offset: u64, size: usize) -> Result<u64, VmError>;

    /// 处理I/O写操作
    fn handle_write(&mut self, offset: u64, value: u64, size: usize) -> Result<(), VmError>;

    /// 处理中断
    fn handle_interrupt(&mut self, vector: u32) -> Result<(), VmError>;

    /// 获取设备状态
    fn device_status(&self) -> DeviceStatus;
}

/// 设备管理器接口
pub trait DeviceManager: VmComponent {
    type Device: Device;

    /// 注册设备
    fn register_device(&mut self, device: Box<Self::Device>) -> Result<DeviceId, VmError>;

    /// 注销设备
    fn unregister_device(&mut self, device_id: DeviceId) -> Result<Box<Self::Device>, VmError>;

    /// 查找设备
    fn find_device(&self, device_id: DeviceId) -> Option<&Self::Device>;

    /// 查找设备（可变）
    fn find_device_mut(&mut self, device_id: DeviceId) -> Option<&mut Self::Device>;

    /// 列出所有设备
    fn list_devices(&self) -> Vec<&Self::Device>;

    /// 路由I/O操作
    fn route_io_read(
        &mut self,
        device_id: DeviceId,
        offset: u64,
        size: usize,
    ) -> Result<u64, VmError>;
    fn route_io_write(
        &mut self,
        device_id: DeviceId,
        offset: u64,
        value: u64,
        size: usize,
    ) -> Result<(), VmError>;
}

/// 设备总线接口
pub trait DeviceBus {
    /// 映射设备到总线地址
    fn map_device(&mut self, device_id: DeviceId, base_addr: u64, size: u64)
    -> Result<(), VmError>;

    /// 取消设备映射
    fn unmap_device(&mut self, device_id: DeviceId) -> Result<(), VmError>;

    /// 地址到设备的翻译
    fn translate_address(&self, addr: u64) -> Option<(DeviceId, u64)>;
}

// ============================================================================
// 异步接口
// ============================================================================

/// 异步执行上下文
#[async_trait::async_trait]
pub trait AsyncExecutionContext {
    /// 获取异步运行时
    fn runtime(&self) -> &tokio::runtime::Runtime;

    /// 获取任务调度器
    fn scheduler(&self) -> &dyn TaskScheduler;

    /// 生成任务ID
    fn generate_task_id(&self) -> TaskId;
}

/// 任务调度器接口
#[async_trait::async_trait]
pub trait TaskScheduler {
    /// 提交任务
    async fn submit_task(&self, task: Box<dyn AsyncTask<Result = TaskResult>>) -> TaskId;

    /// 取消任务
    async fn cancel_task(&self, task_id: TaskId) -> Result<(), VmError>;

    /// 获取任务状态
    async fn get_task_status(&self, task_id: TaskId) -> TaskStatus;

    /// 等待任务完成
    async fn wait_task(&self, task_id: TaskId) -> Result<TaskResult, VmError>;
}

/// 异步任务trait
#[async_trait::async_trait]
pub trait AsyncTask: Send + Sync {
    type Result;

    /// 执行任务
    async fn execute(&mut self) -> Result<Self::Result, VmError>;

    /// 获取任务描述
    fn description(&self) -> &str;
}

// ============================================================================
// 模块声明
// ============================================================================

pub mod async_interface;
pub mod config;
pub mod config_validator;
pub mod core;
pub mod device;
pub mod engine;
pub mod event;
pub mod memory;

pub use async_interface::*;
pub use config::*;
pub use core::*;
pub use device::*;
pub use engine::*;
pub use event::*;
pub use memory::*;
