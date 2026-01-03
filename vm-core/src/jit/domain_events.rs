//! 领域事件定义
//!
//! 定义虚拟机运行过程中发生的领域事件。

use std::fmt;

/// 领域事件trait
pub trait DomainEvent: fmt::Debug + Send + Sync {
    /// 获取事件类型
    fn event_type(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

/// VM生命周期事件
#[derive(Debug, Clone)]
pub enum VmLifecycleEvent {
    /// VM启动
    VmStarted {
        vm_id: String,
        occurred_at: std::time::SystemTime,
    },
    /// VM停止
    VmStopped {
        vm_id: String,
        occurred_at: std::time::SystemTime,
    },
    /// VM暂停
    VmPaused {
        vm_id: String,
        occurred_at: std::time::SystemTime,
    },
    /// VM恢复
    VmResumed {
        vm_id: String,
        occurred_at: std::time::SystemTime,
    },
}

impl DomainEvent for VmLifecycleEvent {}

/// JIT编译事件
#[derive(Debug, Clone)]
pub enum JitCompilationEvent {
    /// 开始编译块
    BlockCompilationStarted {
        block_address: u64,
        block_size: usize,
    },
    /// 完成编译块
    BlockCompilationCompleted {
        block_address: u64,
        code_size: usize,
        duration_ms: u64,
    },
    /// 编译失败
    BlockCompilationFailed { block_address: u64, error: String },
}

impl DomainEvent for JitCompilationEvent {}

/// GC事件
#[derive(Debug, Clone)]
pub enum GcEvent {
    /// GC开始
    GcStarted { generation: u32 },
    /// GC完成
    GcCompleted {
        generation: u32,
        duration_ms: u64,
        objects_reclaimed: usize,
    },
}

impl DomainEvent for GcEvent {}

/// 内存事件
#[derive(Debug, Clone)]
pub enum MemoryEvent {
    /// 内存分配
    MemoryAllocated { address: u64, size: usize },
    /// 内存释放
    MemoryFreed { address: u64, size: usize },
    /// 页错误
    PageFault { address: u64, is_write: bool },
}

impl DomainEvent for MemoryEvent {}

/// 执行事件
#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    /// 代码块编译完成
    CodeBlockCompiled {
        vm_id: String,
        pc: u64,
        block_size: usize,
    },
    /// 热点检测
    HotspotDetected {
        vm_id: String,
        pc: u64,
        execution_count: u64,
    },
    /// 代码块缓存命中
    CacheHit { vm_id: String, pc: u64 },
    /// 代码块缓存未命中
    CacheMiss { vm_id: String, pc: u64 },
}

impl DomainEvent for ExecutionEvent {}
