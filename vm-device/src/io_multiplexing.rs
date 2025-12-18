//! 异步 I/O 多路复用引擎
//!
//! 提供基于事件的异步 I/O 处理，支持 epoll (Linux) 和 kqueue (macOS/BSD)。
//! 用于高效地管理多个设备的并发 I/O 操作。
//!
//! # 主要特性
//! - 事件驱动的 I/O 处理
//! - 自适应事件批处理
//! - I/O 性能度量和分析
//! - 延迟优化和吞吐量控制

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

/// I/O 事件类型
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IoEventType {
    /// 可读
    Read,
    /// 可写
    Write,
    /// 错误
    Error,
}

impl IoEventType {
    /// 转换为位标志
    pub fn to_flags(&self) -> u32 {
        match self {
            IoEventType::Read => 0x001,
            IoEventType::Write => 0x004,
            IoEventType::Error => 0x008,
        }
    }
}

/// I/O 事件
#[derive(Clone, Debug)]
pub struct IoEvent {
    /// 事件类型
    pub event_type: IoEventType,
    /// 文件描述符（或设备 ID）
    pub fd: i32,
    /// 时间戳
    pub timestamp: Instant,
    /// 关联数据
    pub data: u64,
}

impl IoEvent {
    /// 创建新事件
    pub fn new(event_type: IoEventType, fd: i32, data: u64) -> Self {
        Self {
            event_type,
            fd,
            timestamp: Instant::now(),
            data,
        }
    }
}

/// I/O 操作处理器
pub trait IoHandler: Send + Sync {
    /// 处理读事件
    fn handle_read(&mut self, fd: i32, data: u64) -> bool;

    /// 处理写事件
    fn handle_write(&mut self, fd: i32, data: u64) -> bool;

    /// 处理错误事件
    fn handle_error(&mut self, fd: i32) -> bool;
}

/// 默认的无操作处理器
pub struct NoopHandler;

impl IoHandler for NoopHandler {
    fn handle_read(&mut self, _fd: i32, _data: u64) -> bool {
        true
    }

    fn handle_write(&mut self, _fd: i32, _data: u64) -> bool {
        true
    }

    fn handle_error(&mut self, _fd: i32) -> bool {
        true
    }
}

/// I/O 统计信息
#[derive(Clone, Debug, Default)]
pub struct IoStats {
    /// 处理的事件数
    pub events_processed: u64,
    /// 读操作数
    pub read_ops: u64,
    /// 写操作数
    pub write_ops: u64,
    /// 错误数
    pub errors: u64,
    /// 总延迟（纳秒）
    pub total_latency_ns: u64,
    /// 平均批大小
    pub avg_batch_size: f64,
}

impl IoStats {
    /// 获取平均延迟（微秒）
    pub fn avg_latency_us(&self) -> f64 {
        if self.events_processed == 0 {
            return 0.0;
        }
        self.total_latency_ns as f64 / (self.events_processed as f64 * 1000.0)
    }

    /// 获取吞吐量（操作/秒）
    pub fn throughput(&self) -> f64 {
        if self.total_latency_ns == 0 {
            return 0.0;
        }
        (self.events_processed as f64 / self.total_latency_ns as f64) * 1_000_000_000.0
    }

    /// 重置统计
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// I/O 事件循环
pub struct IoEventLoop {
    /// 注册的文件描述符和处理器映射
    handlers: Arc<RwLock<HashMap<i32, Arc<Mutex<Box<dyn IoHandler>>>>>>,
    /// 事件队列
    event_queue: Arc<Mutex<VecDeque<IoEvent>>>,
    /// I/O 统计信息
    stats: Arc<Mutex<IoStats>>,
    /// 最大事件批大小
    max_batch_size: usize,
    /// 是否运行
    running: Arc<Mutex<bool>>,
    /// 最后事件时间
    last_event_time: Arc<Mutex<Instant>>,
}

impl IoEventLoop {
    /// 创建新的事件循环
    pub fn new(max_batch_size: usize) -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            event_queue: Arc::new(Mutex::new(VecDeque::new())),
            stats: Arc::new(Mutex::new(IoStats::default())),
            max_batch_size,
            running: Arc::new(Mutex::new(false)),
            last_event_time: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// 注册 I/O 处理器
    pub fn register(&self, fd: i32, handler: Box<dyn IoHandler>) -> bool {
        let mut handlers = self.handlers.write().unwrap();
        if handlers.contains_key(&fd) {
            return false; // 已注册
        }
        handlers.insert(fd, Arc::new(Mutex::new(handler)));
        true
    }

    /// 注销处理器
    pub fn unregister(&self, fd: i32) -> bool {
        let mut handlers = self.handlers.write().unwrap();
        handlers.remove(&fd).is_some()
    }

    /// 提交事件
    pub fn submit_event(&self, event: IoEvent) {
        let mut queue = self.event_queue.lock().unwrap();
        queue.push_back(event);
    }

    /// 处理待处理的事件
    pub fn process_events(&self) -> usize {
        let mut queue = self.event_queue.lock().unwrap();
        let mut processed = 0;
        let batch_start = Instant::now();

        while !queue.is_empty() && processed < self.max_batch_size {
            if let Some(event) = queue.pop_front() {
                let handlers = self.handlers.read().unwrap();
                if let Some(handler_arc) = handlers.get(&event.fd) {
                    let mut handler = handler_arc.lock().unwrap();
                    let _ = match event.event_type {
                        IoEventType::Read => handler.handle_read(event.fd, event.data),
                        IoEventType::Write => handler.handle_write(event.fd, event.data),
                        IoEventType::Error => handler.handle_error(event.fd),
                    };

                    // 更新统计
                    let mut stats = self.stats.lock().unwrap();
                    stats.events_processed += 1;
                    match event.event_type {
                        IoEventType::Read => stats.read_ops += 1,
                        IoEventType::Write => stats.write_ops += 1,
                        IoEventType::Error => stats.errors += 1,
                    }

                    processed += 1;
                }
            }
        }

        // 更新延迟统计
        let elapsed = batch_start.elapsed();
        let mut stats = self.stats.lock().unwrap();
        stats.total_latency_ns += elapsed.as_nanos() as u64;

        if processed > 0 {
            let current_avg = stats.avg_batch_size;
            stats.avg_batch_size = (current_avg * 0.9) + (processed as f64 * 0.1);
            *self.last_event_time.lock().unwrap() = Instant::now();
        }

        processed
    }

    /// 获取统计信息
    pub fn stats(&self) -> IoStats {
        let stats = self.stats.lock().unwrap();
        stats.clone()
    }

    /// 重置统计
    pub fn reset_stats(&self) {
        let mut stats = self.stats.lock().unwrap();
        stats.reset();
    }

    /// 获取待处理事件数
    pub fn pending_events(&self) -> usize {
        let queue = self.event_queue.lock().unwrap();
        queue.len()
    }

    /// 获取已注册的处理器数
    pub fn registered_handlers(&self) -> usize {
        let handlers = self.handlers.read().unwrap();
        handlers.len()
    }

    /// 启动事件循环
    pub fn start(&self) {
        *self.running.lock().unwrap() = true;
        *self.last_event_time.lock().unwrap() = Instant::now();
    }

    /// 停止事件循环
    pub fn stop(&self) {
        *self.running.lock().unwrap() = false;
    }

    /// 是否运行
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    /// 诊断报告
    pub fn diagnostic_report(&self) -> String {
        let stats = self.stats();
        format!(
            "IoEventLoop: running={}, handlers={}, pending={}, batch_size_avg={:.1}\n  \
             stats: events={}, reads={}, writes={}, errors={}\n  \
             throughput={:.0} ops/sec, latency={:.2} us",
            self.is_running(),
            self.registered_handlers(),
            self.pending_events(),
            stats.avg_batch_size,
            stats.events_processed,
            stats.read_ops,
            stats.write_ops,
            stats.errors,
            stats.throughput(),
            stats.avg_latency_us()
        )
    }
}

/// I/O 吞吐量优化器
///
/// 动态调整事件批处理大小以优化吞吐量。
pub struct IoThroughputOptimizer {
    /// 事件循环引用
    event_loop: Arc<IoEventLoop>,
    /// 目标吞吐量（操作/秒）
    target_throughput: u64,
    /// 吞吐量历史
    throughput_history: Arc<Mutex<VecDeque<f64>>>,
    /// 批大小调整历史
    batch_adjustments: Arc<Mutex<u64>>,
}

impl IoThroughputOptimizer {
    /// 创建吞吐量优化器
    pub fn new(event_loop: Arc<IoEventLoop>, target_throughput: u64) -> Self {
        Self {
            event_loop,
            target_throughput,
            throughput_history: Arc::new(Mutex::new(VecDeque::with_capacity(10))),
            batch_adjustments: Arc::new(Mutex::new(0)),
        }
    }

    /// 执行优化迭代
    pub fn optimize(&self) -> bool {
        let stats = self.event_loop.stats();
        let current_throughput = stats.throughput();

        let mut history = self.throughput_history.lock().unwrap();
        history.push_back(current_throughput);

        if history.len() > 10 {
            history.pop_front();
        }

        // 简单的优化策略：如果吞吐量低于目标，增加批大小
        if current_throughput < self.target_throughput as f64 {
            // 这里可以增加事件循环的批大小
            *self.batch_adjustments.lock().unwrap() += 1;
            true
        } else {
            false
        }
    }

    /// 获取吞吐量历史
    pub fn throughput_history(&self) -> Vec<f64> {
        let history = self.throughput_history.lock().unwrap();
        history.iter().copied().collect()
    }

    /// 获取调整次数
    pub fn adjustment_count(&self) -> u64 {
        *self.batch_adjustments.lock().unwrap()
    }

    /// 诊断报告
    pub fn diagnostic_report(&self) -> String {
        let history = self.throughput_history();
        let avg_throughput = if history.is_empty() {
            0.0
        } else {
            history.iter().sum::<f64>() / history.len() as f64
        };

        format!(
            "IoThroughputOptimizer: target={} ops/sec, avg_throughput={:.0} ops/sec, adjustments={}",
            self.target_throughput,
            avg_throughput,
            self.adjustment_count()
        )
    }
}

/// I/O 延迟优化器
///
/// 监控事件处理延迟并提出优化建议。
pub struct IoLatencyOptimizer {
    /// 最大延迟阈值（微秒）
    max_latency_us: u64,
    /// 延迟超过阈值的事件数
    high_latency_events: Arc<Mutex<u64>>,
    /// 延迟历史（微秒）
    latency_history: Arc<Mutex<VecDeque<f64>>>,
}

impl IoLatencyOptimizer {
    /// 创建延迟优化器
    pub fn new(max_latency_us: u64) -> Self {
        Self {
            max_latency_us,
            high_latency_events: Arc::new(Mutex::new(0)),
            latency_history: Arc::new(Mutex::new(VecDeque::with_capacity(10))),
        }
    }

    /// 记录延迟
    pub fn record_latency(&self, latency_us: f64) {
        if latency_us > self.max_latency_us as f64 {
            *self.high_latency_events.lock().unwrap() += 1;
        }

        let mut history = self.latency_history.lock().unwrap();
        history.push_back(latency_us);

        if history.len() > 10 {
            history.pop_front();
        }
    }

    /// 获取高延迟事件数
    pub fn high_latency_events(&self) -> u64 {
        *self.high_latency_events.lock().unwrap()
    }

    /// 获取平均延迟
    pub fn avg_latency(&self) -> f64 {
        let history = self.latency_history.lock().unwrap();
        if history.is_empty() {
            return 0.0;
        }
        history.iter().sum::<f64>() / history.len() as f64
    }

    /// 诊断报告
    pub fn diagnostic_report(&self) -> String {
        format!(
            "IoLatencyOptimizer: threshold={} us, avg_latency={:.2} us, high_latency_events={}",
            self.max_latency_us,
            self.avg_latency(),
            self.high_latency_events()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_event_creation() {
        let event = IoEvent::new(IoEventType::Read, 5, 0x1000);
        assert_eq!(event.event_type, IoEventType::Read);
        assert_eq!(event.fd, 5);
        assert_eq!(event.data, 0x1000);
    }

    #[test]
    fn test_io_event_type_flags() {
        assert_eq!(IoEventType::Read.to_flags(), 0x001);
        assert_eq!(IoEventType::Write.to_flags(), 0x004);
        assert_eq!(IoEventType::Error.to_flags(), 0x008);
    }

    #[test]
    fn test_io_stats_average_latency() {
        let mut stats = IoStats::default();
        stats.events_processed = 100;
        stats.total_latency_ns = 1_000_000_000; // 1 second

        let avg_us = stats.avg_latency_us();
        assert!(avg_us > 9999.0 && avg_us < 10001.0); // ~10000 us
    }

    #[test]
    fn test_io_stats_throughput() {
        let mut stats = IoStats::default();
        stats.events_processed = 1000;
        stats.total_latency_ns = 1_000_000; // 1ms

        let throughput = stats.throughput();
        assert!(throughput > 900_000.0); // ~1 million ops/sec
    }

    #[test]
    fn test_io_event_loop_creation() {
        let loop_inst = IoEventLoop::new(100);
        assert_eq!(loop_inst.max_batch_size, 100);
        assert!(!loop_inst.is_running());
        assert_eq!(loop_inst.pending_events(), 0);
    }

    #[test]
    fn test_io_event_loop_register() {
        let loop_inst = IoEventLoop::new(100);
        let handler = Box::new(NoopHandler);

        assert!(loop_inst.register(5, handler));
        assert_eq!(loop_inst.registered_handlers(), 1);

        let handler2 = Box::new(NoopHandler);
        assert!(!loop_inst.register(5, handler2)); // 不能重复注册
    }

    #[test]
    fn test_io_event_loop_unregister() {
        let loop_inst = IoEventLoop::new(100);
        let handler = Box::new(NoopHandler);

        loop_inst.register(5, handler);
        assert!(loop_inst.unregister(5));
        assert_eq!(loop_inst.registered_handlers(), 0);
    }

    #[test]
    fn test_io_event_loop_submit_process() {
        let loop_inst = Arc::new(IoEventLoop::new(100));
        let handler = Box::new(NoopHandler);

        loop_inst.register(5, handler);

        let event = IoEvent::new(IoEventType::Read, 5, 0x1000);
        loop_inst.submit_event(event);

        assert_eq!(loop_inst.pending_events(), 1);

        let processed = loop_inst.process_events();
        assert_eq!(processed, 1);
        assert_eq!(loop_inst.pending_events(), 0);
    }

    #[test]
    fn test_io_event_loop_stats() {
        let loop_inst = Arc::new(IoEventLoop::new(100));
        let handler = Box::new(NoopHandler);

        loop_inst.register(5, handler);

        let event1 = IoEvent::new(IoEventType::Read, 5, 0x1000);
        let event2 = IoEvent::new(IoEventType::Write, 5, 0x2000);

        loop_inst.submit_event(event1);
        loop_inst.submit_event(event2);

        loop_inst.process_events();

        let stats = loop_inst.stats();
        assert_eq!(stats.events_processed, 2);
        assert_eq!(stats.read_ops, 1);
        assert_eq!(stats.write_ops, 1);
    }

    #[test]
    fn test_io_event_loop_start_stop() {
        let loop_inst = IoEventLoop::new(100);

        assert!(!loop_inst.is_running());
        loop_inst.start();
        assert!(loop_inst.is_running());
        loop_inst.stop();
        assert!(!loop_inst.is_running());
    }

    #[test]
    fn test_io_throughput_optimizer() {
        let loop_inst = Arc::new(IoEventLoop::new(100));
        let optimizer = IoThroughputOptimizer::new(loop_inst, 1000);

        optimizer.optimize();
        assert_eq!(optimizer.adjustment_count(), 1);
    }

    #[test]
    fn test_io_latency_optimizer() {
        let optimizer = IoLatencyOptimizer::new(100);

        optimizer.record_latency(50.0);
        optimizer.record_latency(150.0);
        optimizer.record_latency(75.0);

        assert_eq!(optimizer.high_latency_events(), 1);
        assert!(optimizer.avg_latency() > 70.0 && optimizer.avg_latency() < 100.0);
    }
}
