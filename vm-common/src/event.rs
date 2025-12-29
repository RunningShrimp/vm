// VM事件系统（Event System）
//
// 提供高性能的异步事件总线，用于VM组件之间的通信。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use std::task::JoinHandle;

/// VM事件类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VmEventType {
    /// 内存事件
    Memory,
    /// 处理器事件
    Cpu,
    /// 设备事件
    Device,
    /// 网络事件
    Network,
    /// 系统事件
    System,
    /// 错误事件
    Error,
}

/// VM事件
#[derive(Debug, Clone)]
pub struct VmEvent {
    /// 事件ID
    pub id: u64,
    /// 事件类型
    pub event_type: VmEventType,
    /// 源组件
    pub source: String,
    /// 时间戳
    pub timestamp: Instant,
    /// 事件数据（JSON序列化）
    pub data: String,
}

impl VmEvent {
    /// 创建新事件
    pub fn new(event_type: VmEventType, source: String, data: String) -> Self {
        Self {
            id: Self::next_id(),
            event_type,
            source,
            timestamp: Instant::now(),
            data,
        }
    }

    /// 生成唯一事件ID
    fn next_id() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        COUNTER.fetch_add(1, Ordering::SeqCst)
    }
}

/// 事件处理器trait
pub trait EventHandler: Send + Sync {
    /// 处理事件
    fn handle(&self, event: &VmEvent) -> Result<(), String>;
}

/// 事件总线
pub struct EventBus {
    /// 事件处理器注册
    handlers: Arc<Mutex<HashMap<String, Vec<JoinHandle<()>>>>>,
    /// 事件队列
    event_queue: Arc<Mutex<VecDeque<VmEvent>>>,
    /// 事件统计
    stats: Arc<Mutex<EventStats>>,
}

/// 事件统计
#[derive(Debug, Clone)]
pub struct EventStats {
    /// 总事件数
    pub total_events: u64,
    /// 处理中的事件数
    pub processing_events: u64,
    /// 总处理时间（毫秒）
    pub total_processing_time_ms: u128,
    /// 平均处理时间（毫秒）
    pub avg_processing_time_ms: f64,
}

impl EventBus {
    /// 创建新的事件总线
    ///
    /// # 参数
    /// - `max_handlers`: 最大处理器数量（默认1024）
    /// - `max_queue_size`: 最大队列大小（默认10000）
    ///
    /// # 示例
    /// ```ignore
    /// let bus = EventBus::new(1024, 10000);
    /// ```
    pub fn new(max_handlers: usize, max_queue_size: usize) -> Self {
        Self {
            handlers: Arc::new(Mutex::new(HashMap::new())),
            event_queue: Arc::new(Mutex::new(VecDeque::with_capacity(max_queue_size))),
            stats: Arc::new(Mutex::new(EventStats::new())),
        }
    }

    /// 使用默认配置创建
    pub fn default() -> Self {
        Self::new(1024, 10000)
    }

    /// Helper to acquire handlers lock with error handling
    fn lock_handlers(&self) -> Result<std::sync::MutexGuard<HashMap<String, Vec<JoinHandle<()>>>>, String> {
        self.handlers.lock().map_err(|e| format!("Handlers lock is poisoned: {:?}", e))
    }

    /// Helper to acquire event_queue lock with error handling
    fn lock_event_queue(&self) -> Result<std::sync::MutexGuard<VecDeque<VmEvent>>, String> {
        self.event_queue.lock().map_err(|e| format!("Event queue lock is poisoned: {:?}", e))
    }

    /// Helper to acquire stats lock with error handling
    fn lock_stats(&self) -> Result<std::sync::MutexGuard<EventStats>, String> {
        self.stats.lock().map_err(|e| format!("Stats lock is poisoned: {:?}", e))
    }

    /// 注册事件处理器
    ///
    /// # 参数
    /// - `event_type`: 事件类型（如"memory", "cpu"）
    /// - `handler`: 事件处理器trait对象
    ///
    /// # 返回
    /// - `Ok(())`: 注册成功
    /// - `Err(msg)`: 注册失败
    ///
    /// # 示例
    /// ```ignore
    /// struct MemoryHandler;
    /// impl EventHandler for MemoryHandler {
    ///     fn handle(&self, event: &VmEvent) -> Result<(), String> {
    ///         println!("Memory event: {:?}", event.data);
    ///         Ok(())
    ///     }
    /// }
    /// 
    /// let handler = MemoryHandler;
    /// bus.register("memory", Box::new(handler))?;
    /// ```
    pub fn register<H: EventHandler + 'static>(
        &self,
        event_type: &str,
        handler: H,
    ) -> Result<(), String> {
        let mut handlers = self.lock_handlers()?;
        let key = event_type.to_string();
        
        handlers.entry(key.to_string()).or_insert_with(Vec::new)
            .push(tokio::task::spawn_blocking(move || {
                // 这里应该启动事件处理任务
                tokio::runtime::Handle::current();
            }));
        
        Ok(())
    }

    /// 发布事件
    ///
    /// # 参数
    /// - `event_type`: 事件类型（如"memory", "cpu"）
    /// - `source`: 事件源
    /// - `data`: 事件数据（JSON格式）
    ///
    /// # 返回
    /// - `Ok(())`: 发布成功
    /// - `Err(msg)`: 发布失败
    ///
    /// # 示例
    /// ```ignore
    /// bus.publish("memory", "vm-core", r#"{"type": "alloc", "size": 4096}"#)?;
    /// ```
    pub fn publish(&self, event_type: &str, source: &str, data: &str) -> Result<(), String> {
        let event = VmEvent::new(
            VmEventType::Memory,
            source.to_string(),
            data.to_string()
        );

        let mut queue = self.lock_event_queue()?;
        queue.push_back(event);

        let mut stats = self.lock_stats()?;
        stats.total_events += 1;

        // 限制队列大小
        if queue.len() > 10000 {
            queue.pop_front();
            println!("Event queue full, dropping oldest event");
        }

        Ok(())
    }

    /// 获取事件统计
    pub fn get_stats(&self) -> EventStats {
        match self.lock_stats() {
            Ok(stats) => stats.clone(),
            Err(_) => EventStats::new(),
        }
    }

    /// 清空事件队列和统计
    pub fn clear(&self) {
        if let Ok(mut queue) = self.lock_event_queue() {
            queue.clear();
        }

        if let Ok(mut stats) = self.lock_stats() {
            *stats = EventStats::new();
        }
    }
}

impl EventStats {
    fn new() -> Self {
        Self {
            total_events: 0,
            processing_events: 0,
            total_processing_time_ms: 0,
            avg_processing_time_ms: 0.0,
        }
    }
}

impl std::fmt::Display for EventStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "VM事件总线统计信息")?;
        writeln!(f, "  总事件数: {}", self.total_events)?;
        writeln!(f, "  处理中的事件数: {}", self.processing_events)?;
        writeln!(f, "  总处理时间: {:.2}ms", self.total_processing_time_ms as f64)?;
        writeln!(f, "  平均处理时间: {:.2}ms", self.avg_processing_time_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_event_creation() {
        let event = VmEvent::new(
            VmEventType::Memory,
            "vm-core".to_string(),
            r#"{"type": "alloc", "size": 4096}"#.to_string()
        );
        
        assert_eq!(event.event_type, VmEventType::Memory);
        assert_eq!(event.source, "vm-core");
        assert!(!event.data.is_empty());
    }

    #[test]
    fn test_event_bus_creation() {
        let bus = EventBus::new(100, 1000);
        assert_eq!(bus.handlers.lock().unwrap().len(), 0);
        assert_eq!(bus.event_queue.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_event_bus_stats() {
        let stats = EventStats::new();
        assert_eq!(stats.total_events, 0);
        assert_eq!(stats.processing_events, 0);
        assert_eq!(stats.avg_processing_time_ms, 0.0);
    }
}
