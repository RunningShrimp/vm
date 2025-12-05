use parking_lot::RwLock;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
/// 异步中断处理系统
///
/// 提供异步中断队列、优先级管理和异步中断处理器
use std::sync::Arc;
use tokio::sync::mpsc;

/// 中断优先级
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterruptPriority {
    /// 低优先级 (例如定时器)
    Low = 1,
    /// 中优先级 (例如I/O)
    Normal = 2,
    /// 高优先级 (例如外部中断)
    High = 3,
    /// 最高优先级 (例如系统故障)
    Critical = 4,
}

impl PartialOrd for InterruptPriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        (*self as i32).partial_cmp(&(*other as i32))
    }
}

impl Ord for InterruptPriority {
    fn cmp(&self, other: &Self) -> Ordering {
        (*self as i32).cmp(&(*other as i32))
    }
}

/// 中断类型
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterruptType {
    /// 系统调用
    Syscall(u32),
    /// 定时器中断
    Timer,
    /// I/O中断
    IoInterrupt(u32),
    /// 外部中断
    External(u32),
    /// 页面故障
    PageFault(u64),
    /// 权限错误
    PermissionError(u64),
    /// 通用异常
    Exception(String),
}

/// 中断请求
#[derive(Clone, Debug)]
pub struct Interrupt {
    /// 中断类型
    pub intr_type: InterruptType,
    /// 优先级
    pub priority: InterruptPriority,
    /// 时间戳（纳秒）
    pub timestamp_ns: u64,
    /// 上下文信息
    pub context: Option<Vec<u8>>,
}

impl Interrupt {
    /// 创建新中断
    pub fn new(intr_type: InterruptType, priority: InterruptPriority) -> Self {
        Self {
            intr_type,
            priority,
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            context: None,
        }
    }
}

/// 实现排序特性（用于优先级队列）
impl PartialEq for Interrupt {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp_ns == other.timestamp_ns
    }
}

impl Eq for Interrupt {}

impl PartialOrd for Interrupt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // 反向比较，优先级高的排在前面
        Some(
            other
                .priority
                .cmp(&self.priority)
                .then_with(|| self.timestamp_ns.cmp(&other.timestamp_ns)),
        )
    }
}

impl Ord for Interrupt {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// 中断处理器结果
#[derive(Clone, Debug)]
pub enum InterruptHandlerResult {
    /// 中断已处理
    Handled,
    /// 中断未处理，传递给下一个处理器
    NotHandled,
    /// 处理中发生错误
    Error(String),
}

/// 异步中断处理器
pub type AsyncInterruptHandler = Box<
    dyn Fn(
            Interrupt,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = InterruptHandlerResult> + Send>>
        + Send
        + Sync,
>;

/// 异步中断队列
pub struct AsyncInterruptQueue {
    /// 中断优先级队列
    queue: Arc<parking_lot::Mutex<BinaryHeap<Interrupt>>>,
    /// 异步通道发送端
    tx: mpsc::UnboundedSender<Interrupt>,
    /// 异步通道接收端
    rx: Arc<parking_lot::Mutex<mpsc::UnboundedReceiver<Interrupt>>>,
    /// 注册的中断处理器
    handlers: Arc<RwLock<Vec<(InterruptType, Box<dyn Fn(Interrupt) + Send + Sync>)>>>,
    /// 统计信息
    stats: Arc<parking_lot::Mutex<InterruptStats>>,
}

/// 中断队列统计
#[derive(Clone, Debug, Default)]
pub struct InterruptStats {
    /// 处理的中断总数
    pub total_handled: u64,
    /// 未处理的中断数
    pub total_unhandled: u64,
    /// 处理错误数
    pub errors: u64,
    /// 平均处理延迟（纳秒）
    pub avg_latency_ns: u64,
}

impl AsyncInterruptQueue {
    /// 创建新的异步中断队列
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        Self {
            queue: Arc::new(parking_lot::Mutex::new(BinaryHeap::new())),
            tx,
            rx: Arc::new(parking_lot::Mutex::new(rx)),
            handlers: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(parking_lot::Mutex::new(InterruptStats::default())),
        }
    }

    /// 投递中断到队列
    pub async fn dispatch_interrupt(&self, interrupt: Interrupt) -> Result<(), String> {
        let start = std::time::Instant::now();

        // 添加到优先级队列
        {
            let mut queue = self.queue.lock();
            queue.push(interrupt.clone());
        }

        // 通过通道通知
        self.tx
            .send(interrupt)
            .map_err(|e| format!("Failed to send interrupt: {}", e))?;

        // 更新统计
        let elapsed = start.elapsed().as_nanos() as u64;
        let mut stats = self.stats.lock();
        if stats.avg_latency_ns == 0 {
            stats.avg_latency_ns = elapsed;
        } else {
            stats.avg_latency_ns = (stats.avg_latency_ns + elapsed) / 2;
        }

        Ok(())
    }

    /// 获取下一个待处理的中断
    pub fn peek_next(&self) -> Option<Interrupt> {
        let queue = self.queue.lock();
        queue.peek().cloned()
    }

    /// 弹出下一个待处理的中断
    pub fn pop_next(&self) -> Option<Interrupt> {
        let mut queue = self.queue.lock();
        queue.pop()
    }

    /// 处理所有待处理的中断
    pub async fn handle_pending_interrupts(&self) -> Result<(), String> {
        loop {
            let interrupt = self.pop_next();
            match interrupt {
                Some(intr) => {
                    let result = self.process_interrupt(intr).await;
                    match result {
                        InterruptHandlerResult::Handled => {
                            let mut stats = self.stats.lock();
                            stats.total_handled += 1;
                        }
                        InterruptHandlerResult::NotHandled => {
                            let mut stats = self.stats.lock();
                            stats.total_unhandled += 1;
                        }
                        InterruptHandlerResult::Error(e) => {
                            let mut stats = self.stats.lock();
                            stats.errors += 1;
                            eprintln!("Interrupt processing error: {}", e);
                        }
                    }
                }
                None => break, // 队列为空
            }

            // 让出控制权
            tokio::task::yield_now().await;
        }

        Ok(())
    }

    /// 处理单个中断
    async fn process_interrupt(&self, interrupt: Interrupt) -> InterruptHandlerResult {
        let handlers = self.handlers.read();

        for (_intr_type, handler) in handlers.iter() {
            handler(interrupt.clone());
        }

        InterruptHandlerResult::Handled
    }

    /// 注册中断处理器
    pub fn register_handler<F>(&self, intr_type: InterruptType, handler: F) -> Result<(), String>
    where
        F: Fn(Interrupt) + Send + Sync + 'static,
    {
        let mut handlers = self.handlers.write();
        handlers.push((intr_type, Box::new(handler)));
        Ok(())
    }

    /// 清空所有待处理的中断
    pub fn clear(&self) {
        let mut queue = self.queue.lock();
        queue.clear();
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> InterruptStats {
        self.stats.lock().clone()
    }

    /// 获取队列长度
    pub fn queue_length(&self) -> usize {
        self.queue.lock().len()
    }
}

impl Default for AsyncInterruptQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interrupt_priority() {
        assert!(InterruptPriority::Critical > InterruptPriority::High);
        assert!(InterruptPriority::High > InterruptPriority::Normal);
        assert!(InterruptPriority::Normal > InterruptPriority::Low);
    }

    #[test]
    fn test_interrupt_creation() {
        let intr = Interrupt::new(InterruptType::Timer, InterruptPriority::Normal);

        assert_eq!(intr.intr_type, InterruptType::Timer);
        assert_eq!(intr.priority, InterruptPriority::Normal);
        assert!(intr.timestamp_ns > 0);
    }

    #[tokio::test]
    async fn test_interrupt_queue_creation() {
        let queue = AsyncInterruptQueue::new();
        assert_eq!(queue.queue_length(), 0);
    }

    #[tokio::test]
    async fn test_dispatch_interrupt() {
        let queue = AsyncInterruptQueue::new();

        let intr = Interrupt::new(InterruptType::Timer, InterruptPriority::Normal);

        let result = queue.dispatch_interrupt(intr.clone()).await;
        assert!(result.is_ok());
        assert_eq!(queue.queue_length(), 1);
    }

    #[tokio::test]
    async fn test_interrupt_priority_queue() {
        let queue = AsyncInterruptQueue::new();

        // 投递不同优先级的中断
        queue
            .dispatch_interrupt(Interrupt::new(InterruptType::Timer, InterruptPriority::Low))
            .await
            .unwrap();

        queue
            .dispatch_interrupt(Interrupt::new(
                InterruptType::External(1),
                InterruptPriority::Critical,
            ))
            .await
            .unwrap();

        queue
            .dispatch_interrupt(Interrupt::new(
                InterruptType::IoInterrupt(0),
                InterruptPriority::Normal,
            ))
            .await
            .unwrap();

        // 验证优先级顺序
        let first = queue.pop_next().unwrap();
        assert_eq!(first.priority, InterruptPriority::Critical);

        let second = queue.pop_next().unwrap();
        assert_eq!(second.priority, InterruptPriority::Normal);

        let third = queue.pop_next().unwrap();
        assert_eq!(third.priority, InterruptPriority::Low);
    }

    #[tokio::test]
    async fn test_interrupt_stats() {
        let queue = AsyncInterruptQueue::new();

        queue
            .dispatch_interrupt(Interrupt::new(
                InterruptType::Timer,
                InterruptPriority::Normal,
            ))
            .await
            .unwrap();

        let stats = queue.get_stats();
        assert!(stats.avg_latency_ns > 0);
    }
}
