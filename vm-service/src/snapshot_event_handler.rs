//! 快照事件处理器
//!
//! 处理快照相关的领域事件，将快照操作集成到事件驱动架构。

use log::{debug, info};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use vm_core::{
    VmError, VmResult,
    domain_event_bus::{DomainEventBus, EventHandler, SimpleEventHandler},
    domain_events::{DomainEvent, DomainEventEnum, SnapshotEvent},
};

/// 快照事件统计
#[derive(Debug, Clone, Default)]
pub struct SnapshotEventStats {
    /// 创建的快照数
    pub snapshots_created: u64,
    /// 恢复的快照数
    pub snapshots_restored: u64,
    /// 删除的快照数
    pub snapshots_deleted: u64,
    /// 总快照大小（字节）
    pub total_snapshot_size: u64,
}

/// 快照事件处理器
///
/// 监听快照相关事件并执行相应的操作。
pub struct SnapshotEventHandler {
    /// VM ID
    vm_id: String,
    /// 事件总线
    event_bus: Arc<DomainEventBus>,
    /// 统计信息
    stats: Arc<Mutex<SnapshotEventStats>>,
    /// 订阅ID（用于取消订阅）
    subscription_ids: Arc<Mutex<Vec<vm_core::domain_event_bus::EventSubscriptionId>>>,
}

impl SnapshotEventHandler {
    /// 创建新的快照事件处理器
    pub fn new(vm_id: String, event_bus: Arc<DomainEventBus>) -> Self {
        Self {
            vm_id,
            event_bus,
            stats: Arc::new(Mutex::new(SnapshotEventStats::default())),
            subscription_ids: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> SnapshotEventStats {
        self.stats.lock().unwrap().clone()
    }

    /// 取消所有订阅
    pub fn unregister_handlers(&self) -> VmResult<()> {
        let ids = self.subscription_ids.lock().unwrap();
        for id in ids.iter() {
            let _ = self.event_bus.unsubscribe_by_id(*id);
        }
        Ok(())
    }

    /// 注册事件处理器
    pub fn register_handlers(&self) -> VmResult<()> {
        let mut ids = self.subscription_ids.lock().unwrap();

        // 订阅快照创建事件
        let handler_created = SimpleEventHandler::new({
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "snapshot.created" {
                    let mut s = stats.lock().unwrap();
                    s.snapshots_created += 1;
                    info!("Snapshot created: total={}", s.snapshots_created);
                    // 注意：这里无法直接提取快照大小，因为DomainEvent trait的限制
                }
                Ok(())
            }
        });
        let id = self.event_bus
            .subscribe("snapshot.created", Box::new(handler_created), None)?;
        ids.push(id);

        // 订阅快照恢复事件
        let handler_restored = SimpleEventHandler::new({
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "snapshot.restored" {
                    let mut s = stats.lock().unwrap();
                    s.snapshots_restored += 1;
                    info!("Snapshot restored: total={}", s.snapshots_restored);
                }
                Ok(())
            }
        });
        let id = self.event_bus
            .subscribe("snapshot.restored", Box::new(handler_restored), None)?;
        ids.push(id);

        // 订阅快照删除事件
        let handler_deleted = SimpleEventHandler::new({
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "snapshot.deleted" {
                    let mut s = stats.lock().unwrap();
                    s.snapshots_deleted += 1;
                    info!("Snapshot deleted: total={}", s.snapshots_deleted);
                }
                Ok(())
            }
        });
        let id = self.event_bus
            .subscribe("snapshot.deleted", Box::new(handler_deleted), None)?;
        ids.push(id);

        Ok(())
    }
}

/// 发布快照创建事件
pub fn publish_snapshot_created(
    event_bus: &Arc<DomainEventBus>,
    vm_id: &str,
    snapshot_id: &str,
    snapshot_size: u64,
) -> VmResult<()> {
    let event = DomainEventEnum::Snapshot(SnapshotEvent::SnapshotCreated {
        vm_id: vm_id.to_string(),
        snapshot_id: snapshot_id.to_string(),
        snapshot_size,
        occurred_at: SystemTime::now(),
    });
    event_bus.publish(event)
}

/// 发布快照恢复事件
pub fn publish_snapshot_restored(
    event_bus: &Arc<DomainEventBus>,
    vm_id: &str,
    snapshot_id: &str,
) -> VmResult<()> {
    let event = DomainEventEnum::Snapshot(SnapshotEvent::SnapshotRestored {
        vm_id: vm_id.to_string(),
        snapshot_id: snapshot_id.to_string(),
        occurred_at: SystemTime::now(),
    });
    event_bus.publish(event)
}

/// 发布快照删除事件
pub fn publish_snapshot_deleted(
    event_bus: &Arc<DomainEventBus>,
    vm_id: &str,
    snapshot_id: &str,
) -> VmResult<()> {
    let event = DomainEventEnum::Snapshot(SnapshotEvent::SnapshotDeleted {
        vm_id: vm_id.to_string(),
        snapshot_id: snapshot_id.to_string(),
        occurred_at: SystemTime::now(),
    });
    event_bus.publish(event)
}


