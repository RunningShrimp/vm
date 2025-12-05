//! 执行引擎事件处理器
//!
//! 处理执行引擎相关的领域事件，将指令执行、代码编译等操作集成到事件驱动架构。

use log::{debug, info};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use vm_core::{
    GuestAddr, VmError, VmResult,
    domain_event_bus::{DomainEventBus, EventHandler, SimpleEventHandler},
    domain_events::{DomainEvent, DomainEventEnum, ExecutionEvent},
};

/// 执行引擎事件统计
#[derive(Debug, Clone, Default)]
pub struct ExecutionEventStats {
    /// 执行的指令总数
    pub total_instructions: u64,
    /// 编译的代码块数
    pub total_compiled_blocks: u64,
    /// 检测到的热点数
    pub total_hotspots: u64,
    /// vCPU退出次数
    pub total_vcpu_exits: u64,
    /// 热点PC到执行次数的映射
    pub hotspot_counts: HashMap<GuestAddr, u64>,
}

/// 执行引擎事件处理器
///
/// 监听执行引擎相关事件并执行相应的操作。
pub struct ExecutionEventHandler {
    /// VM ID
    vm_id: String,
    /// 事件总线
    event_bus: Arc<DomainEventBus>,
    /// 统计信息
    stats: Arc<Mutex<ExecutionEventStats>>,
    /// 订阅ID（用于取消订阅）
    subscription_ids: Arc<Mutex<Vec<vm_core::domain_event_bus::EventSubscriptionId>>>,
}

impl ExecutionEventHandler {
    /// 创建新的执行引擎事件处理器
    pub fn new(vm_id: String, event_bus: Arc<DomainEventBus>) -> Self {
        Self {
            vm_id,
            event_bus,
            stats: Arc::new(Mutex::new(ExecutionEventStats::default())),
            subscription_ids: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> ExecutionEventStats {
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

        // 订阅指令执行事件
        let handler_instruction_executed = SimpleEventHandler::new({
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "execution.instruction_executed" {
                    let mut s = stats.lock().unwrap();
                    s.total_instructions += 1;
                }
                Ok(())
            }
        });
        let id = self.event_bus.subscribe(
            "execution.instruction_executed",
            Box::new(handler_instruction_executed),
            None,
        )?;
        ids.push(id);

        // 订阅代码块编译事件
        let handler_code_block_compiled = SimpleEventHandler::new({
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "execution.code_block_compiled" {
                    let mut s = stats.lock().unwrap();
                    s.total_compiled_blocks += 1;
                    info!("Code block compiled: total={}", s.total_compiled_blocks);
                }
                Ok(())
            }
        });
        let id = self.event_bus.subscribe(
            "execution.code_block_compiled",
            Box::new(handler_code_block_compiled),
            None,
        )?;
        ids.push(id);

        // 订阅热点检测事件
        let handler_hotspot_detected = SimpleEventHandler::new({
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "execution.hotspot_detected" {
                    let mut s = stats.lock().unwrap();
                    s.total_hotspots += 1;
                    info!("Hotspot detected: total={}", s.total_hotspots);
                    // 注意：这里无法直接提取PC和执行次数，因为DomainEvent trait的限制
                    // 实际实现中应该使用类型匹配
                }
                Ok(())
            }
        });
        let id = self.event_bus.subscribe(
            "execution.hotspot_detected",
            Box::new(handler_hotspot_detected),
            None,
        )?;
        ids.push(id);

        // 订阅vCPU退出事件
        let handler_vcpu_exited = SimpleEventHandler::new({
            let stats = Arc::clone(&self.stats);
            move |event: &dyn DomainEvent| -> VmResult<()> {
                if event.event_type() == "execution.vcpu_exited" {
                    let mut s = stats.lock().unwrap();
                    s.total_vcpu_exits += 1;
                    debug!("vCPU exited: total={}", s.total_vcpu_exits);
                }
                Ok(())
            }
        });
        let id = self.event_bus
            .subscribe("execution.vcpu_exited", Box::new(handler_vcpu_exited), None)?;
        ids.push(id);

        Ok(())
    }
}

/// 发布指令执行事件
pub fn publish_instruction_executed(
    event_bus: &Arc<DomainEventBus>,
    vm_id: &str,
    vcpu_id: usize,
    pc: GuestAddr,
    instruction: u64,
) -> VmResult<()> {
    let event = DomainEventEnum::Execution(ExecutionEvent::InstructionExecuted {
        vm_id: vm_id.to_string(),
        vcpu_id,
        pc,
        instruction,
        occurred_at: SystemTime::now(),
    });
    event_bus.publish(event)
}

/// 发布代码块编译事件
pub fn publish_code_block_compiled(
    event_bus: &Arc<DomainEventBus>,
    vm_id: &str,
    pc: GuestAddr,
    block_size: usize,
) -> VmResult<()> {
    let event = DomainEventEnum::Execution(ExecutionEvent::CodeBlockCompiled {
        vm_id: vm_id.to_string(),
        pc,
        block_size,
        occurred_at: SystemTime::now(),
    });
    event_bus.publish(event)
}

/// 发布热点检测事件
pub fn publish_hotspot_detected(
    event_bus: &Arc<DomainEventBus>,
    vm_id: &str,
    pc: GuestAddr,
    execution_count: u64,
) -> VmResult<()> {
    let event = DomainEventEnum::Execution(ExecutionEvent::HotspotDetected {
        vm_id: vm_id.to_string(),
        pc,
        execution_count,
        occurred_at: SystemTime::now(),
    });
    event_bus.publish(event)
}

/// 发布vCPU退出事件
pub fn publish_vcpu_exited(
    event_bus: &Arc<DomainEventBus>,
    vm_id: &str,
    vcpu_id: usize,
    reason: String,
) -> VmResult<()> {
    let event = DomainEventEnum::Execution(ExecutionEvent::VcpuExited {
        vm_id: vm_id.to_string(),
        vcpu_id,
        reason,
        occurred_at: SystemTime::now(),
    });
    event_bus.publish(event)
}
