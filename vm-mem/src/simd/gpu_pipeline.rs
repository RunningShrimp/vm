//! GPU 计算流水线 - 异步执行与负载均衡
//!
//! 提供：
//! - 异步命令提交
//! - 事件同步机制
//! - 流管理与优先级
//! - 动态负载均衡
//! - 多 GPU 任务调度

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;

/// GPU 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// GPU 任务优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// GPU 计算任务
#[derive(Debug, Clone)]
pub struct GpuTask {
    pub id: u32,
    pub device_id: u32,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub submit_time: Instant,
    pub start_time: Option<Instant>,
    pub completion_time: Option<Instant>,
    pub computation_time_us: u32,
    pub data_volume: u64,
}

impl GpuTask {
    pub fn new(
        id: u32,
        device_id: u32,
        priority: TaskPriority,
        computation_time_us: u32,
        data_volume: u64,
    ) -> Self {
        Self {
            id,
            device_id,
            priority,
            status: TaskStatus::Pending,
            submit_time: Instant::now(),
            start_time: None,
            completion_time: None,
            computation_time_us,
            data_volume,
        }
    }

    pub fn start_execution(&mut self) {
        self.status = TaskStatus::Running;
        self.start_time = Some(Instant::now());
    }

    pub fn complete(&mut self) {
        self.status = TaskStatus::Completed;
        self.completion_time = Some(Instant::now());
    }

    pub fn latency_us(&self) -> Option<u64> {
        self.completion_time
            .map(|ct| ct.duration_since(self.submit_time).as_micros() as u64)
    }

    pub fn throughput_gbps(&self) -> Option<f64> {
        self.latency_us().map(|lat| {
            let data_mb = self.data_volume as f64 / (1024.0 * 1024.0);
            let time_s = lat as f64 / 1_000_000.0;
            data_mb / time_s / 1024.0
        })
    }
}

/// GPU 事件
#[derive(Debug, Clone)]
pub struct GpuEvent {
    pub id: u32,
    pub name: String,
    pub triggered_time: Option<Instant>,
    pub is_record_event: bool,
    pub waiting_tasks: Arc<RwLock<Vec<u32>>>,
}

impl GpuEvent {
    pub fn new_record(id: u32, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            triggered_time: None,
            is_record_event: true,
            waiting_tasks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn record(&mut self) {
        self.triggered_time = Some(Instant::now());
    }

    pub fn wait(&self, task_id: u32) {
        self.waiting_tasks.write().push(task_id);
    }

    pub fn is_triggered(&self) -> bool {
        self.triggered_time.is_some()
    }
}

/// GPU 流管理器
pub struct GpuStreamManager {
    device_id: u32,
    streams: Arc<RwLock<Vec<Arc<GpuCommandStream>>>>,
    events: Arc<RwLock<HashMap<u32, GpuEvent>>>,
}

/// GPU 命令流
pub struct GpuCommandStream {
    pub id: u32,
    pub device_id: u32,
    pub priority: TaskPriority,
    task_queue: Arc<RwLock<VecDeque<GpuTask>>>,
    executed_tasks: Arc<RwLock<Vec<GpuTask>>>,
    last_update: Arc<RwLock<Instant>>,
}

impl GpuCommandStream {
    pub fn new(id: u32, device_id: u32, priority: TaskPriority) -> Self {
        Self {
            id,
            device_id,
            priority,
            task_queue: Arc::new(RwLock::new(VecDeque::new())),
            executed_tasks: Arc::new(RwLock::new(Vec::new())),
            last_update: Arc::new(RwLock::new(Instant::now())),
        }
    }

    pub fn enqueue_task(&self, task: GpuTask) {
        self.task_queue.write().push_back(task);
        *self.last_update.write() = Instant::now();
    }

    pub fn dequeue_task(&self) -> Option<GpuTask> {
        self.task_queue.write().pop_front()
    }

    pub fn get_queue_length(&self) -> usize {
        self.task_queue.read().len()
    }

    pub fn get_completed_tasks(&self) -> Vec<GpuTask> {
        self.executed_tasks.read().clone()
    }

    pub fn record_completed(&self, task: GpuTask) {
        self.executed_tasks.write().push(task);
    }

    pub fn get_stats(&self) -> (usize, usize) {
        let pending = self.task_queue.read().len();
        let completed = self.executed_tasks.read().len();
        (pending, completed)
    }
}

impl GpuStreamManager {
    pub fn new(device_id: u32) -> Self {
        Self {
            device_id,
            streams: Arc::new(RwLock::new(Vec::new())),
            events: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 创建命令流
    pub fn create_stream(&self, priority: TaskPriority) -> Arc<GpuCommandStream> {
        let stream_id = self.streams.read().len() as u32;
        let stream = Arc::new(GpuCommandStream::new(stream_id, self.device_id, priority));
        self.streams.write().push(stream.clone());
        stream
    }

    /// 创建事件
    pub fn create_event(&self, name: &str) -> u32 {
        let event_id = self.events.read().len() as u32;
        let event = GpuEvent::new_record(event_id, name);
        self.events.write().insert(event_id, event);
        event_id
    }

    /// 记录事件
    pub fn record_event(&self, event_id: u32) {
        if let Some(event) = self.events.write().get_mut(&event_id) {
            event.record();
        }
    }

    /// 等待事件
    pub fn wait_event(&self, event_id: u32) -> bool {
        self.events
            .read()
            .get(&event_id)
            .map(|e| e.is_triggered())
            .unwrap_or(false)
    }

    /// 获取流统计
    pub fn get_stream_stats(&self) -> Vec<(u32, TaskPriority, usize, usize)> {
        self.streams
            .read()
            .iter()
            .map(|s| {
                let (pending, completed) = s.get_stats();
                (s.id, s.priority, pending, completed)
            })
            .collect()
    }
}

/// 动态负载均衡器
pub struct LoadBalancer {
    streams: Arc<RwLock<Vec<Arc<GpuCommandStream>>>>,
    device_weights: Arc<RwLock<HashMap<u32, f32>>>,
    last_rebalance: Arc<RwLock<Instant>>,
    rebalance_interval_ms: u64,
}

impl Default for LoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadBalancer {
    pub fn new() -> Self {
        Self {
            streams: Arc::new(RwLock::new(Vec::new())),
            device_weights: Arc::new(RwLock::new(HashMap::new())),
            last_rebalance: Arc::new(RwLock::new(Instant::now())),
            rebalance_interval_ms: 100,
        }
    }

    pub fn register_stream(&self, stream: Arc<GpuCommandStream>) {
        let device_id = stream.device_id;
        self.streams.write().push(stream);
        self.device_weights.write().insert(device_id, 1.0);
    }

    /// 选择最优流进行任务调度
    pub fn select_stream(&self, task: &GpuTask) -> Option<Arc<GpuCommandStream>> {
        let streams = self.streams.read();
        if streams.is_empty() {
            return None;
        }

        // 按优先级过滤
        let available: Vec<_> = streams
            .iter()
            .filter(|s| s.device_id == task.device_id || task.device_id == u32::MAX)
            .collect();

        if available.is_empty() {
            return None;
        }

        // 选择队列最短的流
        let best = available
            .iter()
            .min_by_key(|s| s.get_queue_length())
            .copied();
        best.cloned()
    }

    /// 动态调整设备权重
    pub fn adjust_weights(&self) -> HashMap<u32, f32> {
        let now = Instant::now();
        let last = *self.last_rebalance.read();

        if now.duration_since(last).as_millis() < self.rebalance_interval_ms as u128 {
            return self.device_weights.read().clone();
        }

        let streams = self.streams.read();
        let mut device_loads: HashMap<u32, (usize, usize)> = HashMap::new();

        for stream in streams.iter() {
            let (pending, _) = stream.get_stats();
            device_loads.entry(stream.device_id).or_insert((0, 0)).0 += pending;
        }

        for stream in streams.iter() {
            device_loads.entry(stream.device_id).or_insert((0, 0)).1 += 1;
        }

        let mut new_weights = self.device_weights.write();
        let max_load = device_loads
            .values()
            .map(|(l, _)| l)
            .max()
            .copied()
            .unwrap_or(1);

        for (device_id, (load, stream_count)) in device_loads.iter() {
            let _avg_load = *load as f32 / (*stream_count as f32).max(1.0);
            let weight = if max_load > 0 {
                (*load as f32 / max_load as f32).max(0.1)
            } else {
                1.0
            };
            new_weights.insert(*device_id, weight);
        }

        *self.last_rebalance.write() = now;
        new_weights.clone()
    }

    pub fn get_weights(&self) -> HashMap<u32, f32> {
        self.device_weights.read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_task_creation() {
        let task = GpuTask::new(1, 0, TaskPriority::Normal, 1000, 1024 * 1024);
        assert_eq!(task.id, 1);
        assert_eq!(task.device_id, 0);
        assert_eq!(task.priority, TaskPriority::Normal);
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.computation_time_us, 1000);
    }

    #[test]
    fn test_gpu_task_execution() {
        let mut task = GpuTask::new(1, 0, TaskPriority::Normal, 1000, 1024 * 1024);

        task.start_execution();
        assert_eq!(task.status, TaskStatus::Running);

        std::thread::sleep(std::time::Duration::from_millis(10));
        task.complete();
        assert_eq!(task.status, TaskStatus::Completed);

        let latency = task.latency_us();
        assert!(latency.is_some());
        let latency_val = latency.expect("Latency should be available");
        assert!(latency_val >= 10000);
    }

    #[test]
    fn test_gpu_event_creation() {
        let event = GpuEvent::new_record(1, "kernel_complete");
        assert_eq!(event.id, 1);
        assert!(!event.is_triggered());

        let mut event = event;
        event.record();
        assert!(event.is_triggered());
    }

    #[test]
    fn test_command_stream_operations() {
        let stream = GpuCommandStream::new(0, 0, TaskPriority::Normal);

        let task1 = GpuTask::new(1, 0, TaskPriority::Normal, 1000, 512 * 1024);
        let task2 = GpuTask::new(2, 0, TaskPriority::High, 2000, 1024 * 1024);

        stream.enqueue_task(task1);
        stream.enqueue_task(task2);

        assert_eq!(stream.get_queue_length(), 2);

        let dequeued = stream.dequeue_task();
        assert!(dequeued.is_some());
        let task = dequeued.expect("Dequeued task should be available");
        assert_eq!(task.id, 1);
        assert_eq!(stream.get_queue_length(), 1);
    }

    #[test]
    fn test_stream_manager() {
        let manager = GpuStreamManager::new(0);

        let stream1 = manager.create_stream(TaskPriority::Normal);
        let stream2 = manager.create_stream(TaskPriority::High);

        assert_eq!(stream1.id, 0);
        assert_eq!(stream2.id, 1);

        let event_id = manager.create_event("test_event");
        assert!(!manager.wait_event(event_id));

        manager.record_event(event_id);
        assert!(manager.wait_event(event_id));
    }

    #[test]
    fn test_load_balancer() {
        let lb = LoadBalancer::new();

        let stream1 = Arc::new(GpuCommandStream::new(0, 0, TaskPriority::Normal));
        let stream2 = Arc::new(GpuCommandStream::new(1, 0, TaskPriority::Normal));

        lb.register_stream(stream1.clone());
        lb.register_stream(stream2.clone());

        let task = GpuTask::new(1, 0, TaskPriority::Normal, 1000, 512 * 1024);
        let selected = lb.select_stream(&task);
        assert!(selected.is_some());

        // Add tasks to stream1
        for i in 0..5 {
            let t = GpuTask::new(i, 0, TaskPriority::Normal, 1000, 512 * 1024);
            stream1.enqueue_task(t);
        }

        // Next selection should prefer stream2
        let selected2 = lb.select_stream(&task);
        assert!(selected2.is_some());
        let stream2 = selected2.expect("Selected stream should be available");
        assert_eq!(stream2.id, 1);
    }

    #[test]
    fn test_dynamic_weight_adjustment() {
        let lb = LoadBalancer::new();

        let stream1 = Arc::new(GpuCommandStream::new(0, 0, TaskPriority::Normal));
        let stream2 = Arc::new(GpuCommandStream::new(1, 0, TaskPriority::Normal));

        lb.register_stream(stream1.clone());
        lb.register_stream(stream2.clone());

        // Add load to stream1
        for i in 0..10 {
            let task = GpuTask::new(i, 0, TaskPriority::Normal, 1000, 512 * 1024);
            stream1.enqueue_task(task);
        }

        let weights = lb.adjust_weights();
        assert_eq!(weights.len(), 1); // Only device 0 has streams
    }

    #[test]
    fn test_task_throughput_calculation() {
        let mut task = GpuTask::new(1, 0, TaskPriority::Normal, 1000, 64 * 1024 * 1024);
        task.start_execution();
        std::thread::sleep(std::time::Duration::from_millis(10));
        task.complete();

        let throughput = task.throughput_gbps();
        assert!(throughput.is_some());
        let throughput_val = throughput.expect("Throughput should be available");
        assert!(throughput_val > 0.0);
    }
}
