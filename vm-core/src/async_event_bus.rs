//! 异步事件总线
//!
//! 提供异步事件处理能力，支持事件队列、批处理和重试机制。

#![cfg(feature = "async")]

use crate::VmError;
use crate::domain_event_bus::{DomainEventBus, EventHandler, EventSubscriptionId};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::time::SystemTime;

use tokio::time::sleep;
use tokio::sync::mpsc;

/// 队列中的事件
#[derive(Debug, Clone)]
struct QueuedEvent {
    /// 事件
    event: crate::domain_events::DomainEventEnum,
    /// 重试次数
    retry_count: u32,
    /// 入队时间
    enqueued_at: SystemTime,
}

type EventSender = mpsc::UnboundedSender<QueuedEvent>;

type EventReceiver = mpsc::UnboundedReceiver<QueuedEvent>;

/// 异步事件总线
///
/// 提供异步事件处理能力，支持：
/// - 事件队列
/// - 批处理
/// - 重试机制
pub struct AsyncEventBus {
    /// 底层同步事件总线
    sync_bus: Arc<DomainEventBus>,
    /// 事件队列
    event_queue: Arc<Mutex<VecDeque<QueuedEvent>>>,
    /// 事件发送器（用于异步处理）
    event_sender: Option<EventSender>,
    /// 事件接收器（用于异步处理）
    event_receiver: Option<EventReceiver>,
    /// 批处理大小（自适应）
    batch_size: usize,
    /// 最小批处理大小
    min_batch_size: usize,
    /// 最大批处理大小
    max_batch_size: usize,
    /// 批处理间隔（毫秒，自适应）
    batch_interval_ms: u64,
    /// 最小批处理间隔（毫秒）
    min_batch_interval_ms: u64,
    /// 最大批处理间隔（毫秒）
    max_batch_interval_ms: u64,
    /// 事件处理速率（事件/秒）
    processing_rate: Arc<AtomicU64>,
    /// 最大重试次数
    max_retries: u32,
    /// 是否正在运行
    is_running: Arc<AtomicBool>,
    /// 已处理事件数
    processed_count: Arc<AtomicU64>,
    /// 失败事件数
    failed_count: Arc<AtomicU64>,
}

impl AsyncEventBus {
    /// 创建新的异步事件总线
    pub fn new() -> Self {
        Self {
            sync_bus: Arc::new(DomainEventBus::new()),
            event_queue: Arc::new(Mutex::new(VecDeque::new())),
            event_sender: {
                let (sender, _) = mpsc::unbounded_channel();
                Some(sender)
            },
            event_receiver: {
                let (_, receiver) = mpsc::unbounded_channel();
                Some(receiver)
            },
            batch_size: 100,
            min_batch_size: 10,
            max_batch_size: 1000,
            batch_interval_ms: 10,
            min_batch_interval_ms: 1,
            max_batch_interval_ms: 100,
            max_retries: 3,
            processing_rate: Arc::new(AtomicU64::new(0)),
            is_running: Arc::new(AtomicBool::new(false)),
            processed_count: Arc::new(AtomicU64::new(0)),
            failed_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 创建带配置的异步事件总线
    pub fn with_config(batch_size: usize, batch_interval_ms: u64, max_retries: u32) -> Self {
        Self {
            sync_bus: Arc::new(DomainEventBus::new()),
            event_queue: Arc::new(Mutex::new(VecDeque::new())),
            event_sender: {
                let (sender, _) = mpsc::unbounded_channel();
                Some(sender)
            },
            event_receiver: {
                let (_, receiver) = mpsc::unbounded_channel();
                Some(receiver)
            },
            batch_size,
            min_batch_size: 10,
            max_batch_size: 1000,
            batch_interval_ms,
            min_batch_interval_ms: 1,
            max_batch_interval_ms: 100,
            max_retries,
            processing_rate: Arc::new(AtomicU64::new(0)),
            is_running: Arc::new(AtomicBool::new(false)),
            processed_count: Arc::new(AtomicU64::new(0)),
            failed_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> AsyncEventBusStats {
        AsyncEventBusStats {
            processed_count: self.processed_count.load(Ordering::Relaxed),
            failed_count: self.failed_count.load(Ordering::Relaxed),
            queue_size: self.event_queue.lock().map(|q| q.len()).unwrap_or(0),
            is_running: self.is_running.load(Ordering::Acquire),
        }
    }

    /// 获取底层同步事件总线
    pub fn sync_bus(&self) -> &Arc<DomainEventBus> {
        &self.sync_bus
    }
}

impl Default for AsyncEventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// 异步事件总线统计
#[derive(Debug, Clone)]
pub struct AsyncEventBusStats {
    /// 已处理事件数
    pub processed_count: u64,
    /// 失败事件数
    pub failed_count: u64,
    /// 队列大小
    pub queue_size: usize,
    /// 是否正在运行
    pub is_running: bool,
}
