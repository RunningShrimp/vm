//! 事件存储模块
//!
//! 实现事件溯源（Event Sourcing）模式，提供事件的持久化和回放功能。
//!
//! ## 主要功能
//!
//! - **事件存储**: 持久化所有领域事件
//! - **事件回放**: 从事件流重建聚合根状态
//! - **事件查询**: 按VM ID、事件类型、时间范围查询事件
//! - **快照支持**: 支持快照机制以加速回放
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use vm_core::{EventStore, InMemoryEventStore, DomainEventEnum};
//!
//! // 创建事件存储
//! let store = InMemoryEventStore::new();
//!
//! // 保存事件
//! let event = DomainEventEnum::VmLifecycle(/* ... */);
//! store.append("vm-001", 1, event)?;
//!
//! // 回放事件
//! let events = store.load_events("vm-001", None, None)?;
//! ```

use crate::{VmError, VmResult};
use crate::domain_events::DomainEventEnum;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

/// 存储的事件记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    /// 事件序号（从1开始，每个VM独立）
    pub sequence_number: u64,
    /// VM ID
    pub vm_id: String,
    /// 事件数据
    pub event: DomainEventEnum,
    /// 存储时间戳
    pub stored_at: SystemTime,
}

/// 事件存储trait
///
/// 定义事件存储的接口，支持事件的追加、查询和回放。
pub trait EventStore: Send + Sync {
    /// 追加事件到事件流
    ///
    /// # 参数
    /// - `vm_id`: VM ID
    /// - `sequence_number`: 事件序号（从1开始，每个VM独立）
    /// - `event`: 要存储的事件
    ///
    /// # 返回
    /// 返回实际存储的序号（可能与传入的序号不同，如果存储自动分配序号）
    fn append(
        &self,
        vm_id: &str,
        sequence_number: Option<u64>,
        event: DomainEventEnum,
    ) -> VmResult<u64>;

    /// 加载指定VM的所有事件
    ///
    /// # 参数
    /// - `vm_id`: VM ID
    /// - `from_sequence`: 起始序号（包含），None表示从开始
    /// - `to_sequence`: 结束序号（包含），None表示到结束
    ///
    /// # 返回
    /// 按序号排序的事件列表
    fn load_events(
        &self,
        vm_id: &str,
        from_sequence: Option<u64>,
        to_sequence: Option<u64>,
    ) -> VmResult<Vec<StoredEvent>>;

    /// 获取指定VM的最后一个事件序号
    ///
    /// # 返回
    /// 最后一个事件序号，如果没有事件则返回0
    fn get_last_sequence_number(&self, vm_id: &str) -> VmResult<u64>;

    /// 获取指定VM的事件总数
    fn get_event_count(&self, vm_id: &str) -> VmResult<usize>;

    /// 列出所有有事件的VM ID
    fn list_vm_ids(&self) -> VmResult<Vec<String>>;

    /// 删除指定VM的所有事件
    fn delete_events(&self, vm_id: &str) -> VmResult<()>;
}

/// 内存事件存储实现
///
/// 用于测试和开发，事件存储在内存中。
pub struct InMemoryEventStore {
    /// 事件存储：vm_id -> Vec<StoredEvent>
    events: Arc<RwLock<HashMap<String, Vec<StoredEvent>>>>,
    /// 每个VM的当前序号：vm_id -> sequence_number
    sequences: Arc<RwLock<HashMap<String, u64>>>,
}

impl InMemoryEventStore {
    /// 创建新的内存事件存储
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(HashMap::new())),
            sequences: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryEventStore {
    fn default() -> Self {
        Self::new()
    }
}

impl EventStore for InMemoryEventStore {
    fn append(
        &self,
        vm_id: &str,
        sequence_number: Option<u64>,
        event: DomainEventEnum,
    ) -> VmResult<u64> {
        let mut events = self.events.write().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire events write lock".to_string(),
                operation: "append".to_string(),
            })
        })?;

        let mut sequences = self.sequences.write().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire sequences write lock".to_string(),
                operation: "append".to_string(),
            })
        })?;

        // 确定序号
        let seq = match sequence_number {
            Some(seq) => {
                // 检查序号是否已存在
                let current_seq = sequences.get(vm_id).copied().unwrap_or(0);
                if seq <= current_seq {
                    return Err(VmError::Core(crate::CoreError::InvalidState {
                        message: format!(
                            "Sequence number {} is not greater than current {}",
                            seq, current_seq
                        ),
                        current: format!("{}", seq),
                        expected: format!("> {}", current_seq),
                    }));
                }
                seq
            }
            None => {
                // 自动分配序号
                
                sequences.get(vm_id).copied().unwrap_or(0) + 1
            }
        };

        // 创建存储的事件
        let stored_event = StoredEvent {
            sequence_number: seq,
            vm_id: vm_id.to_string(),
            event,
            stored_at: SystemTime::now(),
        };

        // 追加到事件流
        events
            .entry(vm_id.to_string())
            .or_insert_with(Vec::new)
            .push(stored_event);

        // 更新序号
        sequences.insert(vm_id.to_string(), seq);

        Ok(seq)
    }

    fn load_events(
        &self,
        vm_id: &str,
        from_sequence: Option<u64>,
        to_sequence: Option<u64>,
    ) -> VmResult<Vec<StoredEvent>> {
        let events = self.events.read().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire events read lock".to_string(),
                operation: "load_events".to_string(),
            })
        })?;

        let vm_events = events.get(vm_id).cloned().unwrap_or_default();

        // 过滤事件
        let filtered: Vec<StoredEvent> = vm_events
            .into_iter()
            .filter(|e| {
                let seq = e.sequence_number;
                let from_ok = from_sequence.is_none_or(|f| seq >= f);
                let to_ok = to_sequence.is_none_or(|t| seq <= t);
                from_ok && to_ok
            })
            .collect();

        Ok(filtered)
    }

    fn get_last_sequence_number(&self, vm_id: &str) -> VmResult<u64> {
        let sequences = self.sequences.read().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire sequences read lock".to_string(),
                operation: "get_last_sequence_number".to_string(),
            })
        })?;

        Ok(sequences.get(vm_id).copied().unwrap_or(0))
    }

    fn get_event_count(&self, vm_id: &str) -> VmResult<usize> {
        let events = self.events.read().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire events read lock".to_string(),
                operation: "get_event_count".to_string(),
            })
        })?;

        Ok(events.get(vm_id).map(|e| e.len()).unwrap_or(0))
    }

    fn list_vm_ids(&self) -> VmResult<Vec<String>> {
        let events = self.events.read().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire events read lock".to_string(),
                operation: "list_vm_ids".to_string(),
            })
        })?;

        Ok(events.keys().cloned().collect())
    }

    fn delete_events(&self, vm_id: &str) -> VmResult<()> {
        let mut events = self.events.write().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire events write lock".to_string(),
                operation: "delete_events".to_string(),
            })
        })?;

        let mut sequences = self.sequences.write().map_err(|_| {
            VmError::Core(crate::CoreError::Concurrency {
                message: "Failed to acquire sequences write lock".to_string(),
                operation: "delete_events".to_string(),
            })
        })?;

        events.remove(vm_id);
        sequences.remove(vm_id);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_events::{VmLifecycleEvent, DomainEventEnum};
    use std::time::SystemTime;

    #[test]
    fn test_append_and_load() {
        let store = InMemoryEventStore::new();

        let event1 = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated {
            vm_id: "vm-001".to_string(),
            config: crate::VmConfigSnapshot::default(),
            occurred_at: SystemTime::now(),
        });

        let event2 = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmStarted {
            vm_id: "vm-001".to_string(),
            occurred_at: SystemTime::now(),
        });

        // 追加事件
        let seq1 = store.append("vm-001", None, event1.clone()).unwrap();
        assert_eq!(seq1, 1);

        let seq2 = store.append("vm-001", None, event2.clone()).unwrap();
        assert_eq!(seq2, 2);

        // 加载事件
        let events = store.load_events("vm-001", None, None).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence_number, 1);
        assert_eq!(events[1].sequence_number, 2);

        // 测试范围查询
        let events_range = store.load_events("vm-001", Some(1), Some(1)).unwrap();
        assert_eq!(events_range.len(), 1);
        assert_eq!(events_range[0].sequence_number, 1);
    }

    #[test]
    fn test_get_last_sequence_number() {
        let store = InMemoryEventStore::new();

        assert_eq!(store.get_last_sequence_number("vm-001").unwrap(), 0);

        let event = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated {
            vm_id: "vm-001".to_string(),
            config: crate::VmConfigSnapshot::default(),
            occurred_at: SystemTime::now(),
        });

        store.append("vm-001", None, event).unwrap();
        assert_eq!(store.get_last_sequence_number("vm-001").unwrap(), 1);
    }

    #[test]
    fn test_get_event_count() {
        let store = InMemoryEventStore::new();

        assert_eq!(store.get_event_count("vm-001").unwrap(), 0);

        let event = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated {
            vm_id: "vm-001".to_string(),
            config: crate::VmConfigSnapshot::default(),
            occurred_at: SystemTime::now(),
        });

        store.append("vm-001", None, event).unwrap();
        assert_eq!(store.get_event_count("vm-001").unwrap(), 1);
    }

    #[test]
    fn test_list_vm_ids() {
        let store = InMemoryEventStore::new();

        let event1 = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated {
            vm_id: "vm-001".to_string(),
            config: crate::VmConfigSnapshot::default(),
            occurred_at: SystemTime::now(),
        });

        let event2 = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated {
            vm_id: "vm-002".to_string(),
            config: crate::VmConfigSnapshot::default(),
            occurred_at: SystemTime::now(),
        });

        store.append("vm-001", None, event1).unwrap();
        store.append("vm-002", None, event2).unwrap();

        let vm_ids = store.list_vm_ids().unwrap();
        assert_eq!(vm_ids.len(), 2);
        assert!(vm_ids.contains(&"vm-001".to_string()));
        assert!(vm_ids.contains(&"vm-002".to_string()));
    }

    #[test]
    fn test_delete_events() {
        let store = InMemoryEventStore::new();

        let event = DomainEventEnum::VmLifecycle(VmLifecycleEvent::VmCreated {
            vm_id: "vm-001".to_string(),
            config: crate::VmConfigSnapshot::default(),
            occurred_at: SystemTime::now(),
        });

        store.append("vm-001", None, event).unwrap();
        assert_eq!(store.get_event_count("vm-001").unwrap(), 1);

        store.delete_events("vm-001").unwrap();
        assert_eq!(store.get_event_count("vm-001").unwrap(), 0);
        assert_eq!(store.get_last_sequence_number("vm-001").unwrap(), 0);
    }
}

