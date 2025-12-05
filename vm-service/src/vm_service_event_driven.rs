//! 事件驱动的虚拟机服务
//!
//! 使用聚合根和事件总线实现事件驱动的虚拟机服务。

#[cfg(not(feature = "no_std"))]
use crate::device_event_handler::DeviceEventHandler;
#[cfg(not(feature = "no_std"))]
use crate::execution_event_handler::ExecutionEventHandler;
#[cfg(not(feature = "no_std"))]
use crate::memory_event_handler::MemoryEventHandler;
use crate::vm_service::VirtualMachineService;
use log::{debug, info};
use std::sync::{Arc, Mutex};
use vm_core::vm_state::VirtualMachineState;
use vm_core::{
    AggregateRoot, ExecResult, ExecStats, ExecStatus, GuestAddr, VirtualMachineAggregate, VmConfig,
    VmError, VmId, VmResult,
    domain_event_bus::{DomainEventBus, EventHandler, SimpleEventHandler},
    domain_events::{DomainEvent, DomainEventEnum, MemoryEvent, VmLifecycleEvent},
    event_store::{EventStore, InMemoryEventStore},
};

/// 事件驱动的虚拟机服务
///
/// 使用聚合根和事件总线实现事件驱动的架构。
/// 所有状态变更都通过聚合根进行，并自动发布领域事件。
/// 支持事件溯源：所有事件都会被存储，可以通过事件回放重建状态。
#[cfg(not(feature = "no_std"))]
pub struct EventDrivenVmService<B> {
    /// 虚拟机聚合根
    aggregate: Arc<Mutex<VirtualMachineAggregate>>,
    /// 事件总线
    event_bus: Arc<DomainEventBus>,
    /// 事件存储（用于事件溯源）
    event_store: Arc<dyn EventStore>,
    /// 底层状态（用于向后兼容）
    state: Arc<Mutex<VirtualMachineState<B>>>,
    /// 传统服务（用于向后兼容）
    legacy_service: Arc<VirtualMachineService<B>>,
}

#[cfg(not(feature = "no_std"))]
impl<B: 'static> EventDrivenVmService<B> {
    /// 创建新的事件驱动VM服务（领域服务方法）
    ///
    /// 封装事件驱动VM服务的创建业务逻辑
    pub fn new(vm_id: VmId, config: VmConfig, state: VirtualMachineState<B>) -> VmResult<Self> {
        // 业务逻辑：验证VM ID
        if vm_id.as_str().trim().is_empty() {
            return Err(VmError::InvalidArgument {
                field: "vm_id".to_string(),
                message: "VM ID cannot be empty".to_string(),
            });
        }

        // 业务逻辑：验证配置
        if config.memory_size == 0 {
            return Err(VmError::InvalidArgument {
                field: "config.memory_size".to_string(),
                message: "Memory size cannot be zero".to_string(),
            });
        }

        // 业务逻辑：验证虚拟机状态
        if state.is_running() {
            return Err(VmError::InvalidState {
                message: "Cannot create event-driven service for running VM".to_string(),
                current: "running".to_string(),
                expected: "stopped".to_string(),
            });
        }

        // 调用基础设施层创建服务
        Self::create_event_driven_service_infrastructure(
            vm_id,
            config,
            state,
            Arc::new(InMemoryEventStore::new()),
        )
    }

    /// 基础设施层：实际的事件驱动VM服务创建
    fn create_event_driven_service_infrastructure(
        vm_id: VmId,
        config: VmConfig,
        state: VirtualMachineState<B>,
        event_store: Arc<dyn EventStore>,
    ) -> VmResult<Self> {
        let event_bus = Arc::new(DomainEventBus::new());
        let vm_id_str = vm_id.as_str().to_string();

        // 基础设施：尝试从事件存储回放事件重建聚合
        let aggregate = {
            let events = event_store.load_events(&vm_id_str, None, None)?;
            if events.is_empty() {
                // 基础设施：没有历史事件，创建新的聚合
                Arc::new(Mutex::new(VirtualMachineAggregate::with_event_bus(
                    vm_id_str.clone(),
                    config.clone(),
                    event_bus.clone(),
                )))
            } else {
                // 基础设施：从事件回放重建聚合
                Arc::new(Mutex::new(VirtualMachineAggregate::from_events(
                    vm_id_str.clone(),
                    config.clone(),
                    events,
                )))
            }
        };

        let state_arc = Arc::new(Mutex::new(state));

        // 基础设施：设置事件处理器
        Self::setup_event_handlers(&event_bus, &state_arc, &event_store, &vm_id_str)?;

        // 基础设施：设置各种事件处理器
        let memory_handler = MemoryEventHandler::new(vm_id_str.clone(), event_bus.clone());
        memory_handler.register_handlers()?;

        let device_handler = DeviceEventHandler::new(vm_id_str.clone(), event_bus.clone());
        device_handler.register_handlers()?;

        let execution_handler =
            ExecutionEventHandler::new(vm_id_str.clone(), event_bus.clone());
        execution_handler.register_handlers()?;

        let state_for_service = {
            let state_guard = state_arc.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Concurrency {
                    message: "Failed to acquire state lock".to_string(),
                    operation: "create_event_driven_service".to_string(),
                })
            })?;
            state_guard.clone()
        };

        let legacy_service = Arc::new(VirtualMachineService::new(state_for_service));

        Ok(Self {
            aggregate,
            event_bus,
            event_store,
            state: state_arc,
            legacy_service,
        })
    }

    /// 基础设施层：设置事件处理器
    fn setup_event_handlers_infrastructure(
        event_bus: &Arc<DomainEventBus>,
        state_arc: &Arc<Mutex<VirtualMachineState<B>>>,
        event_store: &Arc<dyn EventStore>,
        vm_id_str: &str,
    ) -> VmResult<()> {
        // 基础设施：设置事件存储处理器
        let event_store_handler = SimpleEventHandler::new(
            Arc::clone(event_store),
            vm_id_str.to_string(),
        );
        event_bus.register_handler(event_store_handler)?;

        Ok(())
    }

    /// 使用指定的事件存储创建事件驱动VM服务
    pub fn with_event_store(
        vm_id: VmId,
        config: VmConfig,
        state: VirtualMachineState<B>,
        event_store: Arc<dyn EventStore>,
    ) -> VmResult<Self> {
        let event_bus = Arc::new(DomainEventBus::new());
        let vm_id_str = vm_id.as_str().to_string();
        
        // 尝试从事件存储回放事件重建聚合
        let aggregate = {
            let events = event_store.load_events(&vm_id_str, None, None)?;
            if events.is_empty() {
                // 没有历史事件，创建新的聚合
                Arc::new(Mutex::new(VirtualMachineAggregate::with_event_bus(
                    vm_id_str.clone(),
                    config.clone(),
                    event_bus.clone(),
                )))
            } else {
                // 从事件回放重建聚合
                Arc::new(Mutex::new(VirtualMachineAggregate::from_events(
                    vm_id_str.clone(),
                    config.clone(),
                    events,
                )))
            }
        };

        let state_arc = Arc::new(Mutex::new(state));

        // 设置事件处理器（包括事件存储处理器）
        Self::setup_event_handlers(&event_bus, &state_arc, &event_store, &vm_id_str)?;

        // 设置内存事件处理器
        let memory_handler = MemoryEventHandler::new(vm_id_str.clone(), event_bus.clone());
        memory_handler.register_handlers()?;

        // 设置设备事件处理器
        let device_handler = DeviceEventHandler::new(vm_id_str.clone(), event_bus.clone());
        device_handler.register_handlers()?;

        // 设置执行引擎事件处理器
        let execution_handler =
            ExecutionEventHandler::new(vm_id_str.clone(), event_bus.clone());
        execution_handler.register_handlers()?;

        let state_for_service = {
            let state_guard = state_arc.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Concurrency {
                    message: "Failed to acquire state lock".to_string(),
                    operation: "new".to_string(),
                })
            })?;
            // 创建新的状态用于legacy服务
            VirtualMachineState::new(
                state_guard.config().clone(),
                Box::new(vm_mem::SoftMmu::new(
                    state_guard.config().memory_size,
                    false,
                )),
            )
        };
        let legacy_service = Arc::new(VirtualMachineService::new(state_for_service));

        Ok(Self {
            aggregate,
            event_bus,
            event_store,
            state: state_arc,
            legacy_service,
        })
    }

    /// 设置事件处理器
    fn setup_event_handlers(
        event_bus: &Arc<DomainEventBus>,
        state: &Arc<Mutex<VirtualMachineState<B>>>,
        event_store: &Arc<dyn EventStore>,
        vm_id: &str,
    ) -> VmResult<()> {
        let state_clone = Arc::clone(state);

        // 订阅VM生命周期事件并更新状态
        let state_for_started = state_clone.clone();
        let handler_started: SimpleEventHandler<_> =
            SimpleEventHandler::new(move |event: &dyn DomainEvent| {
                if event.event_type() == "vm.started" {
                    if let Ok(mut s) = state_for_started.lock() {
                        s.set_state(vm_core::VmState::Running);
                    }
                }
                Ok(())
            });
        event_bus.subscribe("vm.started", Box::new(handler_started), None)?;

        let state_for_paused = state_clone.clone();
        let handler_paused: SimpleEventHandler<_> =
            SimpleEventHandler::new(move |event: &dyn DomainEvent| {
                if event.event_type() == "vm.paused" {
                    if let Ok(mut s) = state_for_paused.lock() {
                        s.set_state(vm_core::VmState::Paused);
                    }
                }
                Ok(())
            });
        event_bus.subscribe("vm.paused", Box::new(handler_paused), None)?;

        let state_for_stopped = state_clone.clone();
        let handler_stopped: SimpleEventHandler<_> =
            SimpleEventHandler::new(move |event: &dyn DomainEvent| {
                if event.event_type() == "vm.stopped" {
                    if let Ok(mut s) = state_for_stopped.lock() {
                        s.set_state(vm_core::VmState::Stopped);
                    }
                }
                Ok(())
            });
        event_bus.subscribe("vm.stopped", Box::new(handler_stopped), None)?;

        Ok(())
    }


    /// 启动虚拟机
    pub fn start(&self) -> VmResult<()> {
        let mut aggregate = self.aggregate.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Concurrency {
                message: "Failed to acquire aggregate lock".to_string(),
                operation: "start".to_string(),
            })
        })?;

        // 通过聚合根启动（会自动发布事件）
        aggregate.start()?;

        // 获取未提交的事件
        let events = aggregate.uncommitted_events();
        let vm_id_str = aggregate.vm_id().to_string();

        // 提交事件到事件总线
        aggregate.commit_events()?;

        // 存储事件到事件存储
        for event in events {
            let last_seq = self.event_store.get_last_sequence_number(&vm_id_str)?;
            self.event_store.append(&vm_id_str, Some(last_seq + 1), event)?;
        }

        info!("VM {} started", aggregate.vm_id());
        Ok(())
    }

    /// 暂停虚拟机
    pub fn pause(&self) -> VmResult<()> {
        let mut aggregate = self.aggregate.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Concurrency {
                message: "Failed to acquire aggregate lock".to_string(),
                operation: "pause".to_string(),
            })
        })?;

        aggregate.pause()?;

        // 提交事件到事件总线并存储
        let events = aggregate.uncommitted_events();
        aggregate.commit_events()?;

        let vm_id_str = aggregate.vm_id().to_string();
        for event in events {
            let last_seq = self.event_store.get_last_sequence_number(&vm_id_str)?;
            self.event_store.append(&vm_id_str, Some(last_seq + 1), event)?;
        }

        info!("VM {} paused", aggregate.vm_id());
        Ok(())
    }

    /// 恢复虚拟机
    pub fn resume(&self) -> VmResult<()> {
        let mut aggregate = self.aggregate.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Concurrency {
                message: "Failed to acquire aggregate lock".to_string(),
                operation: "resume".to_string(),
            })
        })?;

        aggregate.resume()?;

        // 提交事件到事件总线并存储
        let events = aggregate.uncommitted_events();
        aggregate.commit_events()?;

        let vm_id_str = aggregate.vm_id().to_string();
        for event in events {
            let last_seq = self.event_store.get_last_sequence_number(&vm_id_str)?;
            self.event_store.append(&vm_id_str, Some(last_seq + 1), event)?;
        }

        info!("VM {} resumed", aggregate.vm_id());
        Ok(())
    }

    /// 停止虚拟机
    pub fn stop(&self, reason: String) -> VmResult<()> {
        let mut aggregate = self.aggregate.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Concurrency {
                message: "Failed to acquire aggregate lock".to_string(),
                operation: "stop".to_string(),
            })
        })?;

        aggregate.stop(reason.clone())?;

        // 提交事件到事件总线并存储
        let events = aggregate.uncommitted_events();
        aggregate.commit_events()?;

        let vm_id_str = aggregate.vm_id().to_string();
        for event in events {
            let last_seq = self.event_store.get_last_sequence_number(&vm_id_str)?;
            self.event_store.append(&vm_id_str, Some(last_seq + 1), event)?;
        }

        info!("VM {} stopped: {}", aggregate.vm_id(), reason);
        Ok(())
    }

    /// 获取虚拟机ID
    pub fn vm_id(&self) -> String {
        self.aggregate
            .lock()
            .map(|a| a.vm_id().to_string())
            .unwrap_or_default()
    }

    /// 获取当前状态
    pub fn state(&self) -> vm_core::VmState {
        self.aggregate
            .lock()
            .map(|a| a.state())
            .unwrap_or(vm_core::VmState::Stopped)
    }

    /// 获取事件总线
    pub fn event_bus(&self) -> &Arc<DomainEventBus> {
        &self.event_bus
    }

    /// 获取事件存储
    pub fn event_store(&self) -> &Arc<dyn EventStore> {
        &self.event_store
    }

    /// 回放事件重建聚合状态
    ///
    /// 从事件存储加载所有事件并重建聚合根状态。
    pub fn replay_events(&self) -> VmResult<()> {
        let vm_id_str = self.vm_id();
        let events = self.event_store.load_events(vm_id_str, None, None)?;

        if events.is_empty() {
            return Ok(());
        }

        // 获取当前配置
        let config = {
            let aggregate = self.aggregate.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Concurrency {
                    message: "Failed to acquire aggregate lock".to_string(),
                    operation: "replay_events".to_string(),
                })
            })?;
            aggregate.config().clone()
        };

        // 从事件重建聚合
        let new_aggregate = VirtualMachineAggregate::from_events(
            vm_id_str.to_string(),
            config,
            events,
        );

        // 替换聚合
        let mut aggregate = self.aggregate.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Concurrency {
                message: "Failed to acquire aggregate lock".to_string(),
                operation: "replay_events".to_string(),
            })
        })?;
        *aggregate = new_aggregate;

        info!("Replayed {} events for VM {}", self.event_store.get_event_count(vm_id_str)?, vm_id_str);
        Ok(())
    }

    /// 获取底层状态（用于向后兼容）
    pub fn state_ref(&self) -> &Arc<Mutex<VirtualMachineState<B>>> {
        &self.state
    }

    /// 获取传统服务（用于向后兼容）
    pub fn legacy_service(&self) -> &Arc<VirtualMachineService<B>> {
        &self.legacy_service
    }

    /// 执行代码块（事件驱动）
    ///
    /// 执行代码块并发布执行引擎事件
    /// 注意：VirtualMachineService 没有直接的 execute_block 方法
    /// 这个方法作为示例，实际使用时需要根据具体实现调整
    pub fn execute_block(&self, _pc: GuestAddr) -> VmResult<ExecResult> {
        use crate::execution_event_handler::publish_code_block_compiled;

        // 注意：VirtualMachineService 没有 execute_block 方法
        // 这里返回一个示例结果，实际使用时需要调用正确的方法
        // 例如：self.legacy_service.run(pc) 或其他执行方法

        // 发布代码块编译事件（示例）
        // publish_code_block_compiled(&self.event_bus, &self.vm_id(), pc, 0)?;

        Ok(ExecResult {
            status: ExecStatus::Ok,
            stats: ExecStats::default(),
            next_pc: _pc,
        })
    }

    /// 加载内核（事件驱动）
    ///
    /// 加载内核并发布内存映射事件
    pub fn load_kernel(&self, data: &[u8], load_addr: GuestAddr) -> VmResult<()> {
        use crate::memory_event_handler::publish_memory_mapped;

        // 使用传统服务加载内核
        self.legacy_service.load_kernel(data, load_addr)?;

        // 发布内存映射事件
        publish_memory_mapped(
            &self.event_bus,
            &self.vm_id(),
            load_addr,
            0, // host_addr 未知
            data.len() as u64,
        )?;

        info!(
            "Kernel loaded at 0x{:x}, size: {} bytes",
            load_addr,
            data.len()
        );
        Ok(())
    }

    /// 加载内核文件（事件驱动）
    pub fn load_kernel_file(&self, path: &str, load_addr: GuestAddr) -> VmResult<()> {
        use std::fs;
        let data = fs::read(path).map_err(|e| VmError::Io(e.to_string()))?;
        self.load_kernel(&data, load_addr)
    }

    /// 创建快照（事件驱动）
    pub fn create_snapshot(&self, name: String, description: String) -> VmResult<String> {
        use std::time::SystemTime;
        use vm_core::domain_events::{DomainEventEnum, SnapshotEvent};

        // 使用传统服务创建快照
        let snapshot_id = self
            .legacy_service
            .create_snapshot(name.clone(), description.clone())?;

        // 获取快照大小
        let snapshot_size = {
            let state = self.state.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Concurrency {
                    message: "Failed to acquire state lock".to_string(),
                    operation: "create_snapshot".to_string(),
                })
            })?;
            let snapshot_manager = state.snapshot_manager();
            let manager = snapshot_manager.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: "Failed to lock snapshot manager".to_string(),
                    module: "EventDrivenVmService".to_string(),
                })
            })?;
            if let Some(snapshot) = manager.snapshots.get(&snapshot_id) {
                // 尝试读取文件大小
                std::fs::metadata(&snapshot.memory_dump_path)
                    .map(|m| m.len())
                    .unwrap_or(0)
            } else {
                0
            }
        };

        // 发布快照创建事件
        let event = DomainEventEnum::Snapshot(SnapshotEvent::SnapshotCreated {
            vm_id: self.vm_id(),
            snapshot_id: snapshot_id.clone(),
            snapshot_size,
            occurred_at: SystemTime::now(),
        });
        self.event_bus.publish(event)?;

        info!("Snapshot created: {} ({})", name, snapshot_id);
        Ok(snapshot_id)
    }

    /// 恢复快照（事件驱动）
    pub fn restore_snapshot(&self, snapshot_id: &str) -> VmResult<()> {
        use std::time::SystemTime;
        use vm_core::domain_events::{DomainEventEnum, SnapshotEvent};

        // 使用传统服务恢复快照
        self.legacy_service.restore_snapshot(snapshot_id)?;

        // 发布快照恢复事件
        let event = DomainEventEnum::Snapshot(SnapshotEvent::SnapshotRestored {
            vm_id: self.vm_id(),
            snapshot_id: snapshot_id.to_string(),
            occurred_at: SystemTime::now(),
        });
        self.event_bus.publish(event)?;

        info!("Snapshot restored: {}", snapshot_id);
        Ok(())
    }

    /// 删除快照（事件驱动）
    pub fn delete_snapshot(&self, snapshot_id: &str) -> VmResult<()> {
        use std::time::SystemTime;
        use vm_core::domain_events::{DomainEventEnum, SnapshotEvent};

        // 检查快照是否存在
        {
            let state = self.state.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Concurrency {
                    message: "Failed to acquire state lock".to_string(),
                    operation: "delete_snapshot".to_string(),
                })
            })?;
            let snapshot_manager = state.snapshot_manager();
            let manager = snapshot_manager.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: "Failed to lock snapshot manager".to_string(),
                    module: "EventDrivenVmService".to_string(),
                })
            })?;

            if !manager.snapshots.contains_key(snapshot_id) {
                return Err(VmError::Core(vm_core::CoreError::Config {
                    message: format!("Snapshot not found: {}", snapshot_id),
                    path: Some("snapshot_id".to_string()),
                }));
            }
        }

        // 删除快照文件和从管理器删除快照
        {
            let state = self.state.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Concurrency {
                    message: "Failed to acquire state lock".to_string(),
                    operation: "delete_snapshot".to_string(),
                })
            })?;
            let snapshot_manager = state.snapshot_manager();
            let mut manager = snapshot_manager.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: "Failed to lock snapshot manager".to_string(),
                    module: "EventDrivenVmService".to_string(),
                })
            })?;

            if let Some(snapshot) = manager.snapshots.get(snapshot_id) {
                // 删除快照文件
                let _ = std::fs::remove_file(&snapshot.memory_dump_path);
            }

            // 从管理器删除快照
            manager.snapshots.remove(snapshot_id);
        }

        // 发布快照删除事件
        let event = DomainEventEnum::Snapshot(SnapshotEvent::SnapshotDeleted {
            vm_id: self.vm_id(),
            snapshot_id: snapshot_id.to_string(),
            occurred_at: SystemTime::now(),
        });
        self.event_bus.publish(event)?;

        info!("Snapshot deleted: {}", snapshot_id);
        Ok(())
    }

    /// 列出所有快照
    pub fn list_snapshots(&self) -> VmResult<Vec<vm_core::snapshot::Snapshot>> {
        self.legacy_service.list_snapshots()
    }
}
