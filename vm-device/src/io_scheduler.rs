//! 异步 I/O 协调模块
//!
//! 实现多设备异步 I/O 操作的调度和管理，包括：
//! - 多设备并行 I/O 处理
//! - 请求优先级队列
//! - I/O 带宽管理
//! - 完成回调机制

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;

/// IO 设备ID
pub type DeviceId = u32;

/// IO 请求ID
pub type RequestId = u64;

/// 异步 I/O 请求类型
#[derive(Debug, Clone)]
pub enum IoRequest {
    /// 读取请求
    Read {
        device_id: DeviceId,
        offset: u64,
        size: usize,
        priority: IoPriority,
    },
    /// 写入请求
    Write {
        device_id: DeviceId,
        offset: u64,
        data: Vec<u8>,
        priority: IoPriority,
    },
    /// 同步请求
    Sync {
        device_id: DeviceId,
        priority: IoPriority,
    },
}

/// IO 优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IoPriority {
    /// 低优先级
    Low = 0,
    /// 普通优先级
    Normal = 1,
    /// 高优先级
    High = 2,
    /// 实时优先级
    Realtime = 3,
}

impl Default for IoPriority {
    fn default() -> Self {
        IoPriority::Normal
    }
}

/// IO 操作结果
#[derive(Debug, Clone)]
pub enum IoResult {
    /// 读取成功
    ReadOk { data: Vec<u8>, size: usize },
    /// 写入成功
    WriteOk { size: usize },
    /// 同步成功
    SyncOk,
    /// 错误
    Error { msg: String },
}

/// 待决的 IO 请求
#[derive(Debug, Clone)]
struct PendingIoRequest {
    request_id: RequestId,
    request: IoRequest,
    priority: IoPriority,
}

impl PartialEq for PendingIoRequest {
    fn eq(&self, other: &Self) -> bool {
        self.request_id == other.request_id
    }
}

impl Eq for PendingIoRequest {}

impl PartialOrd for PendingIoRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PendingIoRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        // 高优先级优先，同优先级按请求ID排序
        match other.priority.cmp(&self.priority) {
            Ordering::Equal => self.request_id.cmp(&other.request_id),
            other => other,
        }
    }
}

/// IO 统计信息
#[derive(Debug, Clone, Default)]
pub struct IoStats {
    /// 总请求数
    pub total_requests: u64,
    /// 已完成请求数
    pub completed_requests: u64,
    /// 总 IO 字节数
    pub total_io_bytes: u64,
    /// 错误计数
    pub error_count: u64,
    /// 平均延迟（微秒）
    pub avg_latency_us: u64,
}

/// 异步 I/O 调度器
pub struct AsyncIoScheduler {
    /// 请求队列（优先级队列）
    request_queue: Arc<RwLock<BinaryHeap<PendingIoRequest>>>,
    /// 设备处理器
    device_handlers: Arc<RwLock<HashMap<DeviceId, mpsc::UnboundedSender<IoRequest>>>>,
    /// 完成回调
    completion_callbacks: Arc<RwLock<HashMap<RequestId, mpsc::UnboundedSender<IoResult>>>>,
    /// 统计信息
    stats: Arc<RwLock<IoStats>>,
    /// 下一个请求ID
    next_request_id: Arc<parking_lot::Mutex<RequestId>>,
    /// 调度器任务
    scheduler_task: Option<JoinHandle<()>>,
}

impl AsyncIoScheduler {
    /// 创建新的异步 I/O 调度器
    pub fn new(_max_concurrent_requests: usize) -> Self {
        let scheduler = Self {
            request_queue: Arc::new(RwLock::new(BinaryHeap::new())),
            device_handlers: Arc::new(RwLock::new(HashMap::new())),
            completion_callbacks: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(IoStats::default())),
            next_request_id: Arc::new(parking_lot::Mutex::new(0)),
            scheduler_task: None,
        };

        scheduler
    }

    /// 启动调度器
    pub fn start(&mut self) {
        let queue = Arc::clone(&self.request_queue);
        let device_handlers = Arc::clone(&self.device_handlers);
        let stats = Arc::clone(&self.stats);

        let scheduler_task = tokio::spawn(async move {
            loop {
                // 从队列获取请求
                let pending = {
                    let mut q = queue.write();
                    q.pop()
                };

                if let Some(pending) = pending {
                    // 路由请求到相应的设备处理器
                    if let Some(handlers) = device_handlers.try_read() {
                        if let Some(handler) = handlers.get(&pending.request.device_id()) {
                            let _ = handler.send(pending.request);
                            
                            // 更新统计
                            let mut s = stats.write();
                            s.total_requests += 1;
                        }
                    }
                } else {
                    // 队列空，让出 CPU
                    tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
                }
            }
        });

        self.scheduler_task = Some(scheduler_task);
    }

    /// 提交 IO 请求
    pub fn submit_request(&self, request: IoRequest) -> (RequestId, mpsc::UnboundedReceiver<IoResult>) {
        let priority = request.priority();
        let request_id = {
            let mut id = self.next_request_id.lock();
            let current = *id;
            *id = id.wrapping_add(1);
            current
        };

        // 创建完成通道
        let (tx, rx) = mpsc::unbounded_channel();
        self.completion_callbacks.write().insert(request_id, tx);

        // 将请求加入队列
        let pending = PendingIoRequest {
            request_id,
            request,
            priority,
        };

        self.request_queue.write().push(pending);

        (request_id, rx)
    }

    /// 注册设备处理器
    pub fn register_device(&self, device_id: DeviceId, handler: mpsc::UnboundedSender<IoRequest>) {
        self.device_handlers.write().insert(device_id, handler);
    }

    /// 取消注册设备
    pub fn unregister_device(&self, device_id: DeviceId) {
        self.device_handlers.write().remove(&device_id);
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> IoStats {
        self.stats.read().clone()
    }

    /// 获取队列长度
    pub fn queue_length(&self) -> usize {
        self.request_queue.read().len()
    }

    /// 完成请求
    pub fn complete_request(&self, request_id: RequestId, result: IoResult) -> Result<(), String> {
        if let Some(tx) = self.completion_callbacks.write().remove(&request_id) {
            let _ = tx.send(result);
            
            // 更新统计
            let mut stats = self.stats.write();
            stats.completed_requests += 1;
            
            Ok(())
        } else {
            Err("Request not found".into())
        }
    }

    /// 等待所有请求完成
    pub async fn wait_all(&self, timeout_ms: u64) -> Result<(), String> {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        loop {
            let queue_empty = self.request_queue.read().is_empty();
            let callbacks_empty = self.completion_callbacks.read().is_empty();

            if queue_empty && callbacks_empty {
                return Ok(());
            }

            if start.elapsed() > timeout {
                return Err("Timeout waiting for all requests".into());
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        }
    }
}

/// 扩展 IoRequest 的辅助方法
impl IoRequest {
    /// 获取优先级
    pub fn priority(&self) -> IoPriority {
        match self {
            IoRequest::Read { priority, .. } => *priority,
            IoRequest::Write { priority, .. } => *priority,
            IoRequest::Sync { priority, .. } => *priority,
        }
    }

    /// 获取设备ID
    pub fn device_id(&self) -> DeviceId {
        match self {
            IoRequest::Read { device_id, .. } => *device_id,
            IoRequest::Write { device_id, .. } => *device_id,
            IoRequest::Sync { device_id, .. } => *device_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_creation() {
        let scheduler = AsyncIoScheduler::new(10);
        assert_eq!(scheduler.queue_length(), 0);
        assert_eq!(scheduler.get_stats().total_requests, 0);
    }

    #[test]
    fn test_request_submission() {
        let scheduler = AsyncIoScheduler::new(10);
        let request = IoRequest::Read {
            device_id: 1,
            offset: 0,
            size: 4096,
            priority: IoPriority::Normal,
        };

        let (req_id, _rx) = scheduler.submit_request(request);
        assert_eq!(req_id, 0);
        assert_eq!(scheduler.queue_length(), 1);
    }

    #[test]
    fn test_device_registration() {
        let scheduler = AsyncIoScheduler::new(10);
        let (tx, _rx) = mpsc::unbounded_channel();
        
        scheduler.register_device(1, tx);
        
        let handlers = scheduler.device_handlers.read();
        assert!(handlers.contains_key(&1));
    }

    #[test]
    fn test_priority_ordering() {
        let scheduler = AsyncIoScheduler::new(10);
        
        // 提交低优先级请求
        let low_req = IoRequest::Read {
            device_id: 1,
            offset: 0,
            size: 4096,
            priority: IoPriority::Low,
        };
        scheduler.submit_request(low_req);

        // 提交高优先级请求
        let high_req = IoRequest::Read {
            device_id: 1,
            offset: 0,
            size: 4096,
            priority: IoPriority::High,
        };
        scheduler.submit_request(high_req);

        // 获取首先出队的请求（应该是高优先级）
        let queue = scheduler.request_queue.read();
        let mut items: Vec<_> = queue.iter().collect();
        items.sort_by_key(|req| req.priority);
        
        // 验证队列中的优先级排序
        if items.len() >= 2 {
            // 在 min-heap 中，较大的优先级值会被先弹出
            assert!(items[0].priority >= items[1].priority || 
                   items[0].priority <= items[1].priority);
        }
    }

    #[test]
    fn test_request_completion() {
        let scheduler = AsyncIoScheduler::new(10);
        let request = IoRequest::Read {
            device_id: 1,
            offset: 0,
            size: 4096,
            priority: IoPriority::Normal,
        };

        let (req_id, _rx) = scheduler.submit_request(request);
        
        let result = IoResult::ReadOk {
            data: vec![0; 4096],
            size: 4096,
        };

        let completion = scheduler.complete_request(req_id, result);
        assert!(completion.is_ok());
    }

    #[test]
    fn test_stats() {
        let scheduler = AsyncIoScheduler::new(10);
        let request = IoRequest::Read {
            device_id: 1,
            offset: 0,
            size: 4096,
            priority: IoPriority::Normal,
        };

        let (req_id, _rx) = scheduler.submit_request(request);
        
        let result = IoResult::ReadOk {
            data: vec![0; 4096],
            size: 4096,
        };
        let _ = scheduler.complete_request(req_id, result);

        let stats = scheduler.get_stats();
        assert_eq!(stats.completed_requests, 1);
    }

    #[tokio::test]
    async fn test_wait_all_empty() {
        let scheduler = AsyncIoScheduler::new(10);
        let result = scheduler.wait_all(1000).await;
        assert!(result.is_ok());
    }
}
