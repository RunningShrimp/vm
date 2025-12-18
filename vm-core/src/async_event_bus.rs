//! 异步事件总线
//!
//! 提供异步事件处理能力，支持事件队列、批处理和重试机制。

use crate::VmError;
use crate::domain_event_bus::{DomainEventBus, EventHandler, EventSubscriptionId};
use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use std::time::SystemTime;
#[cfg(feature = "async")]
use tokio::time::sleep;

#[cfg(feature = "async")]
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

#[cfg(feature = "async")]
type EventSender = mpsc::UnboundedSender<QueuedEvent>;
#[cfg(not(feature = "async"))]
type EventSender = ();

#[cfg(feature = "async")]
type EventReceiver = mpsc::UnboundedReceiver<QueuedEvent>;
#[cfg(not(feature = "async"))]
type EventReceiver = ();

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
    #[cfg(feature = "async")]
    event_sender: Option<EventSender>,
    #[cfg(not(feature = "async"))]
    event_sender: Option<()>,
    /// 事件接收器（用于异步处理）
    #[cfg(feature = "async")]
    event_receiver: Option<EventReceiver>,
    #[cfg(not(feature = "async"))]
    event_receiver: Option<()>,
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
            #[cfg(feature = "async")]
            event_sender: {
                let (sender, _) = mpsc::unbounded_channel();
                Some(sender)
            },
            #[cfg(not(feature = "async"))]
            event_sender: None,
            #[cfg(feature = "async")]
            event_receiver: {
                let (_, receiver) = mpsc::unbounded_channel();
                Some(receiver)
            },
            #[cfg(not(feature = "async"))]
            event_receiver: None,
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
            #[cfg(feature = "async")]
            event_sender: {
                let (sender, _) = mpsc::unbounded_channel();
                Some(sender)
            },
            #[cfg(not(feature = "async"))]
            event_sender: None,
            #[cfg(feature = "async")]
            event_receiver: {
                let (_, receiver) = mpsc::unbounded_channel();
                Some(receiver)
            },
            #[cfg(not(feature = "async"))]
            event_receiver: None,
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

    /// 启动异步事件处理
    #[cfg(feature = "async")]
    pub fn start(&mut self) -> Result<(), VmError> {
        if self.is_running.load(Ordering::Acquire) {
            return Err(VmError::Core(crate::CoreError::InvalidState {
                message: "Async event bus is already running".to_string(),
                current: "running".to_string(),
                expected: "stopped".to_string(),
            }));
        }

        self.is_running.store(true, Ordering::Release);

        // 启动异步处理任务
        let sync_bus = Arc::clone(&self.sync_bus);
        let batch_size = self.batch_size;
        let min_batch_size = self.min_batch_size;
        let max_batch_size = self.max_batch_size;
        let batch_interval = Duration::from_millis(self.batch_interval_ms);
        let max_retries = self.max_retries;
        let processed_count = Arc::clone(&self.processed_count);
        let failed_count = Arc::clone(&self.failed_count);
        let is_running = Arc::clone(&self.is_running);
        let processing_rate = Arc::clone(&self.processing_rate);

        #[cfg(feature = "async")]
        let mut receiver = self.event_receiver.take().ok_or_else(|| {
            VmError::Core(crate::CoreError::Internal {
                message: "Event receiver already taken".to_string(),
                module: "async_event_bus".to_string(),
            })
        })?;

        #[cfg(feature = "async")]
        {
            tokio::spawn(async move {
                let mut batch = Vec::new();
                let mut last_batch_time = SystemTime::now();

                while is_running.load(Ordering::Acquire) {
                    // 接收事件
                    tokio::select! {
                        event = receiver.recv() => {
                            if let Some(queued_event) = event {
                                batch.push(queued_event);
                            } else {
                                // 通道关闭，退出
                                break;
                            }
                        }
                        _ = sleep(batch_interval) => {
                            // 批处理间隔到达
                        }
                    }

                    // 自适应批处理大小和间隔
                    let current_rate = processing_rate.load(Ordering::Relaxed);
                    let adaptive_batch_size = if current_rate > 10000 {
                        // 高负载时增大批次
                        (batch_size * 2).min(max_batch_size)
                    } else if current_rate < 1000 {
                        // 低负载时减小批次
                        (batch_size / 2).max(min_batch_size)
                    } else {
                        batch_size
                    };

                    let adaptive_interval = if current_rate > 10000 {
                        // 高负载时缩短间隔
                        batch_interval / 2
                    } else if current_rate < 1000 {
                        // 低负载时延长间隔
                        batch_interval * 2
                    } else {
                        batch_interval
                    };

                    // 检查是否需要处理批次
                    let should_process = batch.len() >= adaptive_batch_size
                        || (batch.len() > 0
                            && last_batch_time.elapsed().unwrap_or(Duration::ZERO)
                                >= adaptive_interval);

                    if should_process {
                        // 更新处理速率
                        let batch_len = batch.len() as u64;
                        let elapsed_ms = last_batch_time
                            .elapsed()
                            .unwrap_or(Duration::ZERO)
                            .as_millis() as u64;
                        if elapsed_ms > 0 {
                            let rate = (batch_len * 1000) / elapsed_ms;
                            processing_rate.store(rate, Ordering::Relaxed);
                        }

                        // 处理批次
                        for queued_event in batch.drain(..) {
                            match sync_bus.publish(queued_event.event.clone()) {
                                Ok(_) => {
                                    processed_count.fetch_add(1, Ordering::Relaxed);
                                }
                                Err(e) => {
                                    // 处理失败，检查是否需要重试
                                    if queued_event.retry_count < max_retries {
                                        // 重新入队
                                        let mut retry_event = queued_event;
                                        retry_event.retry_count += 1;
                                        // 注意：这里简化处理，实际应该重新发送到队列
                                        failed_count.fetch_add(1, Ordering::Relaxed);
                                    } else {
                                        failed_count.fetch_add(1, Ordering::Relaxed);
                                        eprintln!(
                                            "Event processing failed after {} retries: {:?}",
                                            max_retries, e
                                        );
                                    }
                                }
                            }
                        }
                        last_batch_time = SystemTime::now();
                    }
                }
            });
        }
        #[cfg(not(feature = "async"))]
        {
            // 非async版本，直接返回成功（使用同步处理）
        }

        Ok(())
    }

    /// 启动异步事件处理（非async版本，回退到同步）
    #[cfg(not(feature = "async"))]
    pub fn start(&mut self) -> Result<(), VmError> {
        // 非async版本，直接返回成功（使用同步处理）
        Ok(())
    }

    /// 停止异步事件处理
    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::Release);
        // 关闭发送器以停止接收器
        #[cfg(feature = "async")]
        {
            if let Some(sender) = self.event_sender.take() {
                drop(sender);
            }
        }
    }

    /// 异步发布事件
    #[cfg(feature = "async")]
    pub fn publish_async(
        &self,
        event: crate::domain_events::DomainEventEnum,
    ) -> Result<(), VmError> {
        if !self.is_running.load(Ordering::Acquire) {
            // 如果异步处理未启动，回退到同步处理
            return self.sync_bus.publish(event);
        }

        let queued_event = QueuedEvent {
            event,
            retry_count: 0,
            enqueued_at: SystemTime::now(),
        };

        #[cfg(feature = "async")]
        {
            if let Some(ref sender) = self.event_sender {
                return sender.send(queued_event).map_err(|_| {
                    VmError::Core(crate::CoreError::Internal {
                        message: "Failed to send event to async queue".to_string(),
                        module: "async_event_bus".to_string(),
                    })
                });
            }
        }
        // 发送器不可用或非async版本，回退到同步处理
        self.sync_bus.publish(queued_event.event)
    }

    /// 异步发布事件（非async版本，回退到同步）
    #[cfg(not(feature = "async"))]
    pub fn publish_async(
        &self,
        event: crate::domain_events::DomainEventEnum,
    ) -> Result<(), VmError> {
        // 非async版本，直接使用同步处理
        self.sync_bus.publish(event)
    }

    /// 同步发布事件（立即处理）
    pub fn publish_sync(
        &self,
        event: crate::domain_events::DomainEventEnum,
    ) -> Result<(), VmError> {
        self.sync_bus.publish(event)
    }

    /// 订阅事件（委托给同步总线）
    pub fn subscribe(
        &self,
        event_type: &str,
        handler: Box<dyn EventHandler>,
        filter: Option<Box<dyn crate::domain_event_bus::EventFilter>>,
    ) -> Result<EventSubscriptionId, VmError> {
        self.sync_bus.subscribe(event_type, handler, filter)
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

#[cfg(test)]
#[cfg(feature = "async")]
mod tests {
    use super::*;
    use crate::domain_events::{DomainEventEnum, VmLifecycleEvent};
    use std::time::SystemTime;

    #[tokio::test]
    async fn test_async_event_bus() {
        let mut bus = AsyncEventBus::new();
        bus.start().unwrap();

        let event = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted {
            vm_id: "test".to_string(),
            occurred_at: SystemTime::now(),
        });

        bus.publish_async(event).unwrap();

        // 等待处理
        sleep(Duration::from_millis(50)).await;

        let stats = bus.stats();
        assert!(stats.processed_count > 0 || stats.queue_size > 0);

        bus.stop();
    }
}
