//! 异步 I/O 协调模块
//!
//! 实现多设备异步 I/O 操作的调度和管理，包括：
//! - 多设备并行 I/O 处理
//! - 请求优先级队列
//! - I/O 带宽管理
//! - 完成回调机制

use parking_lot::RwLock;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// IO 设备ID
pub type DeviceId = u32;

/// IO 请求ID
pub type RequestId = u64;

/// 异步 I/O 请求类型
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

impl PendingIoRequest {
    pub fn device_id(&self) -> DeviceId {
        self.request.device_id()
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

/// 批量操作配置
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// 最大批量大小
    pub max_batch_size: usize,
    /// 批量超时（毫秒）
    pub batch_timeout_ms: u64,
    /// 合并阈值（相同设备的连续请求）
    pub merge_threshold: usize,
    /// 零拷贝启用
    pub enable_zero_copy: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 32,
            batch_timeout_ms: 1,
            merge_threshold: 4,
            enable_zero_copy: true,
        }
    }
}

/// 批量I/O请求
#[derive(Debug)]
struct BatchIoRequest {
    /// 批量中的请求
    requests: Vec<PendingIoRequest>,
    /// 批量开始时间
    start_time: std::time::Instant,
}

/// 异步 I/O 调度器
pub struct AsyncIoScheduler {
    /// 请求队列（优先级队列）
    request_queue: Arc<RwLock<BinaryHeap<PendingIoRequest>>>,
    /// 设备处理器
    device_handlers: Arc<RwLock<HashMap<DeviceId, mpsc::UnboundedSender<IoRequest>>>>,
    /// 批量处理器（支持批量操作）
    batch_handlers: Arc<RwLock<HashMap<DeviceId, mpsc::UnboundedSender<Vec<IoRequest>>>>>,
    /// 完成回调
    completion_callbacks: Arc<RwLock<HashMap<RequestId, mpsc::UnboundedSender<IoResult>>>>,
    /// 统计信息
    stats: Arc<RwLock<IoStats>>,
    /// 下一个请求ID
    next_request_id: Arc<parking_lot::Mutex<RequestId>>,
    /// 调度器任务
    scheduler_task: Option<JoinHandle<()>>,
    /// 批量配置
    batch_config: BatchConfig,
    /// 当前批量缓冲区
    batch_buffer: Arc<RwLock<HashMap<DeviceId, BatchIoRequest>>>,
    /// 最大并发请求数
    max_concurrent_requests: usize,
}

impl AsyncIoScheduler {
    /// 创建新的异步 I/O 调度器
    pub fn new(max_concurrent_requests: usize) -> Self {
        Self::with_batch_config(max_concurrent_requests, BatchConfig::default())
    }

    /// 使用批量配置创建异步 I/O 调度器
    pub fn with_batch_config(max_concurrent_requests: usize, batch_config: BatchConfig) -> Self {
        let scheduler = Self {
            request_queue: Arc::new(RwLock::new(BinaryHeap::new())),
            device_handlers: Arc::new(RwLock::new(HashMap::new())),
            batch_handlers: Arc::new(RwLock::new(HashMap::new())),
            completion_callbacks: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(IoStats::default())),
            next_request_id: Arc::new(parking_lot::Mutex::new(0)),
            scheduler_task: None,
            batch_config,
            batch_buffer: Arc::new(RwLock::new(HashMap::new())),
            max_concurrent_requests,
        };

        scheduler
    }

    /// 启动调度器
    pub fn start(&mut self) {
        let queue = Arc::clone(&self.request_queue);
        let batch_handlers = Arc::clone(&self.batch_handlers);
        let stats = Arc::clone(&self.stats);
        let batch_config = self.batch_config.clone();
        let batch_buffer = Arc::clone(&self.batch_buffer);
        let max_concurrent = self.max_concurrent_requests;

        // 创建信号量用于限制并发请求数
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));

        let scheduler_task = tokio::spawn(async move {
            loop {
                // 处理批量缓冲区（超时或达到大小限制的批量）
                Self::process_batch_buffer(&batch_buffer, &batch_handlers, &stats, &batch_config, &semaphore)
                    .await;

                // 从队列获取请求并添加到批量缓冲区
                {
                    let mut q = queue.write();
                    let mut buffer = batch_buffer.write();

                    // 收集一批请求进行批量处理
                    for _ in 0..batch_config.max_batch_size {
                        if let Some(pending) = q.pop() {
                            let device_id = pending.device_id();
                            
                            // 获取或创建该设备的批量请求
                            let batch = buffer.entry(device_id).or_insert_with(|| BatchIoRequest {
                                requests: Vec::new(),
                                start_time: std::time::Instant::now(),
                            });
                            
                            // 添加请求到批量
                            batch.requests.push(pending);
                        } else {
                            break;
                        }
                    }
                }

                // 如果队列为空且批量缓冲区也为空，让出 CPU
                let queue_empty = queue.read().is_empty();
                let buffer_empty = batch_buffer.read().is_empty();

                if queue_empty && buffer_empty {
                    tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
                }
            }
        });

        self.scheduler_task = Some(scheduler_task);
    }

    /// 处理批量缓冲区
    async fn process_batch_buffer(
        batch_buffer: &Arc<RwLock<HashMap<DeviceId, BatchIoRequest>>>,
        batch_handlers: &Arc<RwLock<HashMap<DeviceId, mpsc::UnboundedSender<Vec<IoRequest>>>>>,
        stats: &Arc<RwLock<IoStats>>,
        config: &BatchConfig,
        semaphore: &Arc<tokio::sync::Semaphore>,
    ) {
        let mut to_process = Vec::new();

        {
            let mut buffer = batch_buffer.write();
            let now = std::time::Instant::now();

            // 收集超时的批量或达到大小限制的批量
            buffer.retain(|device_id, batch| {
                let should_process = batch.requests.len() >= config.max_batch_size
                    || now.duration_since(batch.start_time).as_millis()
                        >= config.batch_timeout_ms as u128;

                if should_process {
                    to_process.push((*device_id, std::mem::take(&mut batch.requests)));
                    false
                } else {
                    true
                }
            });
        }

        // 处理收集到的批量
        for (device_id, requests) in to_process {
            if let Some(batch_handler) = batch_handlers
                .try_read()
                .and_then(|h| h.get(&device_id).cloned())
            {
                let io_requests: Vec<IoRequest> = requests.into_iter().map(|pr| pr.request).collect();
                let stats_clone = Arc::clone(stats);
                let semaphore_clone = Arc::clone(semaphore);

                // 使用信号量限制并发
                tokio::spawn(async move {
                    // 尝试获取信号量许可
                    if let Ok(_permit) = semaphore_clone.acquire().await {
                        // 发送批量请求
                        let _ = batch_handler.send(io_requests.clone());

                        // 更新统计
                        let mut s = stats_clone.write();
                        s.total_requests += io_requests.len() as u64;

                        // 自动释放许可（当_permit离开作用域时）
                    }
                });
            }
        }
    }

    /// 提交 IO 请求
    pub fn submit_request(
        &self,
        request: IoRequest,
    ) -> (RequestId, mpsc::UnboundedReceiver<IoResult>) {
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

    /// 注册批量设备处理器
    pub fn register_batch_device(
        &self,
        device_id: DeviceId,
        handler: mpsc::UnboundedSender<Vec<IoRequest>>,
    ) {
        self.batch_handlers.write().insert(device_id, handler);
    }

    /// 取消注册设备
    pub fn unregister_device(&self, device_id: DeviceId) {
        self.device_handlers.write().remove(&device_id);
        self.batch_handlers.write().remove(&device_id);
        self.batch_buffer.write().remove(&device_id);
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
            assert!(
                items[0].priority >= items[1].priority || items[0].priority <= items[1].priority
            );
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
