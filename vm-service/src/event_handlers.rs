//! 增强的事件处理器
//!
//! 提供完善的事件处理功能，包括过滤、路由、重试和统计。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use vm_core::{
    VmError, VmResult,
    domain_event_bus::{DomainEventBus, EventFilter, EventHandler, EventSubscriptionId},
    domain_events::{DomainEvent, DomainEventEnum},
};

/// 事件处理统计
#[derive(Debug, Clone, Default)]
pub struct EventHandlerStats {
    /// 处理的事件总数
    pub total_processed: u64,
    /// 成功处理的事件数
    pub total_succeeded: u64,
    /// 失败的事件数
    pub total_failed: u64,
    /// 被过滤的事件数
    pub total_filtered: u64,
    /// 重试的事件数
    pub total_retried: u64,
    /// 平均处理时间（纳秒）
    pub avg_processing_time_ns: u64,
}

/// 事件路由规则
#[derive(Debug, Clone)]
pub struct EventRoute {
    /// 目标事件类型
    pub target_type: String,
    /// 条件（可选）
    pub condition: Option<String>,
}

/// 带重试的事件处理器
pub struct RetryEventHandler {
    /// 底层处理器
    handler: Box<dyn EventHandler>,
    /// 最大重试次数
    max_retries: u32,
    /// 重试延迟（毫秒）
    retry_delay_ms: u64,
    /// 统计信息
    stats: Arc<Mutex<EventHandlerStats>>,
}

impl RetryEventHandler {
    pub fn new(
        handler: Box<dyn EventHandler>,
        max_retries: u32,
        retry_delay_ms: u64,
    ) -> Self {
        Self {
            handler,
            max_retries,
            retry_delay_ms,
            stats: Arc::new(Mutex::new(EventHandlerStats::default())),
        }
    }

    pub fn stats(&self) -> EventHandlerStats {
        self.stats.lock().unwrap().clone()
    }
}

impl EventHandler for RetryEventHandler {
    fn handle(&self, event: &dyn DomainEvent) -> Result<(), VmError> {
        let start_time = SystemTime::now();
        let mut retries = 0;

        loop {
            match self.handler.handle(event) {
                Ok(()) => {
                    let elapsed = start_time.elapsed().unwrap_or_default();
                    let mut stats = self.stats.lock().unwrap();
                    stats.total_processed += 1;
                    stats.total_succeeded += 1;
                    stats.avg_processing_time_ns = (stats.avg_processing_time_ns
                        + elapsed.as_nanos() as u64)
                        / 2;
                    return Ok(());
                }
                Err(e) => {
                    if retries < self.max_retries {
                        retries += 1;
                        let mut stats = self.stats.lock().unwrap();
                        stats.total_retried += 1;
                        drop(stats);
                        std::thread::sleep(Duration::from_millis(self.retry_delay_ms));
                        continue;
                    } else {
                        let mut stats = self.stats.lock().unwrap();
                        stats.total_processed += 1;
                        stats.total_failed += 1;
                        return Err(e);
                    }
                }
            }
        }
    }

    fn priority(&self) -> u32 {
        self.handler.priority()
    }
}

/// VM ID过滤器
pub struct VmIdFilter {
    vm_id: String,
}

impl VmIdFilter {
    pub fn new(vm_id: String) -> Self {
        Self { vm_id }
    }
}

impl EventFilter for VmIdFilter {
    fn matches(&self, event: &dyn DomainEvent) -> bool {
        // 尝试从事件中提取VM ID
        // 这是一个简化实现，实际应该根据事件类型提取
        match event.event_type() {
            t if t.starts_with("vm.") => true, // VM生命周期事件总是匹配
            _ => {
                // 对于其他事件，需要检查事件内容
                // 这里简化处理，实际应该使用类型匹配
                true
            }
        }
    }
}

/// 地址范围过滤器（用于内存事件）
pub struct AddressRangeFilter {
    start_addr: u64,
    end_addr: u64,
}

impl AddressRangeFilter {
    pub fn new(start_addr: u64, end_addr: u64) -> Self {
        Self { start_addr, end_addr }
    }
}

impl EventFilter for AddressRangeFilter {
    fn matches(&self, event: &dyn DomainEvent) -> bool {
        // 检查事件类型是否为内存事件
        match event.event_type() {
            "memory.allocated" | "memory.freed" | "memory.mapped" | "memory.unmapped"
            | "memory.page_fault" => {
                // 简化实现：对于内存事件，总是匹配
                // 实际应该从事件中提取地址并检查范围
                true
            }
            _ => false,
        }
    }
}

/// 事件路由器
pub struct EventRouter {
    /// 路由规则
    routes: HashMap<String, Vec<EventRoute>>,
    /// 事件总线
    event_bus: Arc<DomainEventBus>,
}

impl EventRouter {
    pub fn new(event_bus: Arc<DomainEventBus>) -> Self {
        Self {
            routes: HashMap::new(),
            event_bus,
        }
    }

    /// 添加路由规则
    pub fn add_route(&mut self, source_type: String, route: EventRoute) {
        self.routes
            .entry(source_type)
            .or_insert_with(Vec::new)
            .push(route);
    }

    /// 路由事件
    pub fn route(&self, event: &dyn DomainEvent) -> VmResult<()> {
        let event_type = event.event_type();
        if let Some(routes) = self.routes.get(event_type) {
            for route in routes {
                // 创建新事件并发布到目标类型
                // 这里简化实现，实际应该根据路由规则转换事件
                let _ = self.event_bus.publish(event);
            }
        }
        Ok(())
    }
}

/// 统计事件处理器
pub struct StatsEventHandler {
    /// 底层处理器
    handler: Box<dyn EventHandler>,
    /// 统计信息
    stats: Arc<Mutex<EventHandlerStats>>,
}

impl StatsEventHandler {
    pub fn new(handler: Box<dyn EventHandler>) -> Self {
        Self {
            handler,
            stats: Arc::new(Mutex::new(EventHandlerStats::default())),
        }
    }

    pub fn stats(&self) -> EventHandlerStats {
        self.stats.lock().unwrap().clone()
    }
}

impl EventHandler for StatsEventHandler {
    fn handle(&self, event: &dyn DomainEvent) -> Result<(), VmError> {
        let start_time = SystemTime::now();
        let result = self.handler.handle(event);
        let elapsed = start_time.elapsed().unwrap_or_default();

        let mut stats = self.stats.lock().unwrap();
        stats.total_processed += 1;
        match &result {
            Ok(_) => stats.total_succeeded += 1,
            Err(_) => stats.total_failed += 1,
        }
        stats.avg_processing_time_ns = (stats.avg_processing_time_ns + elapsed.as_nanos() as u64) / 2;

        result
    }

    fn priority(&self) -> u32 {
        self.handler.priority()
    }
}

/// 组合过滤器（AND逻辑）
pub struct AndFilter {
    filters: Vec<Box<dyn EventFilter>>,
}

impl AndFilter {
    pub fn new(filters: Vec<Box<dyn EventFilter>>) -> Self {
        Self { filters }
    }
}

impl EventFilter for AndFilter {
    fn matches(&self, event: &dyn DomainEvent) -> bool {
        self.filters.iter().all(|f| f.matches(event))
    }
}

/// 组合过滤器（OR逻辑）
pub struct OrFilter {
    filters: Vec<Box<dyn EventFilter>>,
}

impl OrFilter {
    pub fn new(filters: Vec<Box<dyn EventFilter>>) -> Self {
        Self { filters }
    }
}

impl EventFilter for OrFilter {
    fn matches(&self, event: &dyn DomainEvent) -> bool {
        self.filters.iter().any(|f| f.matches(event))
    }
}

/// 事件类型过滤器
pub struct EventTypeFilter {
    event_types: Vec<String>,
}

impl EventTypeFilter {
    pub fn new(event_types: Vec<String>) -> Self {
        Self { event_types }
    }
}

impl EventFilter for EventTypeFilter {
    fn matches(&self, event: &dyn DomainEvent) -> bool {
        self.event_types.contains(&event.event_type().to_string())
    }
}


