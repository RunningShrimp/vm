//! Execution Manager Domain Service
//!
//! This service encapsulates business logic for execution management
//! including execution context management, task scheduling, and
//! performance monitoring.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::domain_event_bus::DomainEventBus;
use crate::domain_services::events::{DomainEventEnum, ExecutionEvent};
use crate::{CoreError, GuestAddr, VmError, VmResult};

/// Execution context state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionState {
    /// Ready to execute
    Ready,
    /// Currently executing
    Running,
    /// Waiting for I/O or resource
    Waiting,
    /// Execution completed
    Completed,
    /// Execution failed
    Failed,
}

impl ExecutionState {
    pub fn is_active(&self) -> bool {
        matches!(self, ExecutionState::Running | ExecutionState::Waiting)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, ExecutionState::Completed | ExecutionState::Failed)
    }
}

/// Execution context
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Context ID
    pub id: u64,
    /// Program counter
    pub pc: GuestAddr,
    /// Stack pointer
    pub sp: GuestAddr,
    /// Current state
    pub state: ExecutionState,
    /// Creation timestamp
    pub created_at: Instant,
    /// Last updated timestamp
    pub updated_at: Instant,
    /// Total execution time
    pub total_execution_time: Duration,
    /// Number of instructions executed
    pub instructions_executed: u64,
}

impl ExecutionContext {
    pub fn new(id: u64, pc: GuestAddr) -> Self {
        let now = Instant::now();
        Self {
            id,
            pc,
            sp: GuestAddr(0),
            state: ExecutionState::Ready,
            created_at: now,
            updated_at: now,
            total_execution_time: Duration::ZERO,
            instructions_executed: 0,
        }
    }

    pub fn update(&mut self, pc: GuestAddr) {
        self.pc = pc;
        self.updated_at = Instant::now();
    }

    pub fn add_execution_time(&mut self, duration: Duration) {
        self.total_execution_time += duration;
    }

    pub fn increment_instructions(&mut self, count: u64) {
        self.instructions_executed += count;
    }
}

/// Execution priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExecutionPriority {
    /// Low priority
    Low = 0,
    /// Normal priority
    Normal = 1,
    /// High priority
    High = 2,
    /// Critical priority
    Critical = 3,
}

impl ExecutionPriority {
    pub fn from_level(level: u8) -> Self {
        match level {
            0 => ExecutionPriority::Low,
            1 => ExecutionPriority::Normal,
            2 => ExecutionPriority::High,
            _ => ExecutionPriority::Critical,
        }
    }

    pub fn as_level(&self) -> u8 {
        *self as u8
    }
}

/// Execution statistics
#[derive(Debug, Clone, Default)]
pub struct ExecutionStatistics {
    /// Total number of executions
    pub total_executions: u64,
    /// Number of successful executions
    pub successful_executions: u64,
    /// Number of failed executions
    pub failed_executions: u64,
    /// Total instructions executed
    pub total_instructions: u64,
    /// Average execution time
    pub avg_execution_time: Duration,
    /// Peak memory usage in bytes
    pub peak_memory_usage: u64,
    /// Current active contexts
    pub active_contexts: u64,
}

impl ExecutionStatistics {
    pub fn success_rate(&self) -> f64 {
        if self.total_executions == 0 {
            0.0
        } else {
            self.successful_executions as f64 / self.total_executions as f64
        }
    }

    pub fn failure_rate(&self) -> f64 {
        if self.total_executions == 0 {
            0.0
        } else {
            self.failed_executions as f64 / self.total_executions as f64
        }
    }

    pub fn avg_instructions_per_execution(&self) -> f64 {
        if self.total_executions == 0 {
            0.0
        } else {
            self.total_instructions as f64 / self.total_executions as f64
        }
    }
}

/// Execution Manager Domain Service
///
/// This service provides high-level business logic for managing execution contexts
/// with priority-based scheduling, state tracking, and performance monitoring.
pub struct ExecutionManagerDomainService {
    /// Event bus for publishing execution events
    event_bus: Arc<DomainEventBus>,
    /// Active execution contexts
    contexts: HashMap<u64, ExecutionContext>,
    /// Ready queue for execution
    ready_queue: Vec<(ExecutionPriority, u64)>,
    /// Execution statistics
    statistics: ExecutionStatistics,
    /// Maximum number of active contexts
    max_active_contexts: usize,
}

impl ExecutionManagerDomainService {
    pub fn new(event_bus: Arc<DomainEventBus>, max_active_contexts: usize) -> Self {
        Self {
            event_bus,
            contexts: HashMap::new(),
            ready_queue: Vec::new(),
            statistics: ExecutionStatistics::default(),
            max_active_contexts,
        }
    }

    /// Create a new execution context
    pub fn create_context(&mut self, id: u64, pc: GuestAddr) -> VmResult<()> {
        if self.contexts.contains_key(&id) {
            return Err(VmError::Core(CoreError::InvalidState {
                message: "Context already exists".to_string(),
                current: "present".to_string(),
                expected: "absent".to_string(),
            }));
        }

        let context = ExecutionContext::new(id, pc);
        self.contexts.insert(id, context);
        self.ready_queue.push((ExecutionPriority::Normal, id));
        self.ready_queue.sort_by(|a, b| b.0.cmp(&a.0));

        self.publish_event(ExecutionEvent::ContextCreated {
            id,
            pc: pc.0,
            priority: ExecutionPriority::Normal,
        });

        Ok(())
    }

    /// Get an execution context
    pub fn get_context(&self, id: u64) -> Option<&ExecutionContext> {
        self.contexts.get(&id)
    }

    /// Get a mutable execution context
    pub fn get_context_mut(&mut self, id: u64) -> Option<&mut ExecutionContext> {
        self.contexts.get_mut(&id)
    }

    /// Delete an execution context
    pub fn delete_context(&mut self, id: u64) -> VmResult<()> {
        if let Some(context) = self.contexts.remove(&id) {
            self.ready_queue.retain(|(_, ctx_id)| *ctx_id != id);

            self.publish_event(ExecutionEvent::ContextDeleted {
                id,
                final_state: context.state,
                execution_time: context.total_execution_time,
                instructions_executed: context.instructions_executed,
            });

            Ok(())
        } else {
            Err(VmError::Core(CoreError::InvalidState {
                message: "Context not found".to_string(),
                current: "absent".to_string(),
                expected: "present".to_string(),
            }))
        }
    }

    /// Schedule a context for execution
    pub fn schedule(&mut self, id: u64, priority: ExecutionPriority) -> VmResult<()> {
        if !self.contexts.contains_key(&id) {
            return Err(VmError::Core(CoreError::InvalidState {
                message: "Context not found".to_string(),
                current: "absent".to_string(),
                expected: "present".to_string(),
            }));
        }

        self.ready_queue.retain(|(_, ctx_id)| *ctx_id != id);
        self.ready_queue.push((priority, id));
        self.ready_queue.sort_by(|a, b| b.0.cmp(&a.0));

        let context = self.contexts.get_mut(&id).ok_or_else(|| {
            VmError::Core(CoreError::InvalidState {
                message: "Context not found".to_string(),
                current: "absent".to_string(),
                expected: "present".to_string(),
            })
        })?;
        context.state = ExecutionState::Ready;

        self.publish_event(ExecutionEvent::ContextScheduled { id, priority });

        Ok(())
    }

    /// Get next context to execute
    pub fn next_context(&mut self) -> Option<u64> {
        let active_count = self.count_active_contexts();
        if active_count as usize >= self.max_active_contexts {
            return None;
        }

        self.ready_queue.pop().map(|(_, id)| {
            if let Some(context) = self.contexts.get_mut(&id) {
                context.state = ExecutionState::Running;
                self.statistics.active_contexts += 1;
                self.publish_event(ExecutionEvent::ContextStarted { id });
            }
            id
        })
    }

    /// Complete execution of a context
    pub fn complete_execution(&mut self, id: u64) -> VmResult<()> {
        let context = self.contexts.get_mut(&id).ok_or_else(|| {
            VmError::Core(CoreError::InvalidState {
                message: "Context not found".to_string(),
                current: "absent".to_string(),
                expected: "present".to_string(),
            })
        })?;
        context.state = ExecutionState::Completed;
        self.statistics.active_contexts = self.statistics.active_contexts.saturating_sub(1);
        self.statistics.successful_executions += 1;
        self.statistics.total_executions += 1;
        self.statistics.total_instructions += context.instructions_executed;

        // Clone values before calling methods that need &mut self
        let total_execution_time = context.total_execution_time;
        let instructions_executed = context.instructions_executed;

        self.update_avg_execution_time(total_execution_time);

        self.publish_event(ExecutionEvent::ContextCompleted {
            id,
            execution_time: total_execution_time,
            instructions_executed,
        });

        Ok(())
    }

    /// Fail execution of a context
    pub fn fail_execution(&mut self, id: u64, error: VmError) -> VmResult<()> {
        let context = self.contexts.get_mut(&id).ok_or_else(|| {
            VmError::Core(CoreError::InvalidState {
                message: "Context not found".to_string(),
                current: "absent".to_string(),
                expected: "present".to_string(),
            })
        })?;
        context.state = ExecutionState::Failed;
        self.statistics.active_contexts = self.statistics.active_contexts.saturating_sub(1);
        self.statistics.failed_executions += 1;
        self.statistics.total_executions += 1;

        self.publish_event(ExecutionEvent::ContextFailed {
            id,
            error: format!("{:?}", error),
        });

        Ok(())
    }

    /// Pause a context
    pub fn pause_context(&mut self, id: u64) -> VmResult<()> {
        let context = self.contexts.get_mut(&id).ok_or_else(|| {
            VmError::Core(CoreError::InvalidState {
                message: "Context not found".to_string(),
                current: "absent".to_string(),
                expected: "present".to_string(),
            })
        })?;
        context.state = ExecutionState::Waiting;

        self.publish_event(ExecutionEvent::ContextPaused { id });

        Ok(())
    }

    /// Resume a paused context
    pub fn resume_context(&mut self, id: u64) -> VmResult<()> {
        let context = self.contexts.get_mut(&id).ok_or_else(|| {
            VmError::Core(CoreError::InvalidState {
                message: "Context not found".to_string(),
                current: "absent".to_string(),
                expected: "present".to_string(),
            })
        })?;
        context.state = ExecutionState::Ready;
        self.ready_queue.push((ExecutionPriority::Normal, id));
        self.ready_queue.sort_by(|a, b| b.0.cmp(&a.0));

        self.publish_event(ExecutionEvent::ContextResumed { id });

        Ok(())
    }

    /// Get statistics
    pub fn get_statistics(&self) -> &ExecutionStatistics {
        &self.statistics
    }

    /// Set maximum active contexts
    pub fn set_max_active_contexts(&mut self, max: usize) {
        self.max_active_contexts = max;
    }

    /// Count active contexts
    fn count_active_contexts(&self) -> u64 {
        self.contexts
            .values()
            .filter(|ctx| ctx.state.is_active())
            .count() as u64
    }

    /// Update average execution time
    fn update_avg_execution_time(&mut self, duration: Duration) {
        let total =
            self.statistics.avg_execution_time.as_nanos() as u64 * self.statistics.total_executions;
        let new_total = total + duration.as_nanos() as u64;
        let count = self.statistics.total_executions + 1;
        self.statistics.avg_execution_time = Duration::from_nanos(new_total / count);
    }

    /// Publish an execution event
    fn publish_event(&self, event: ExecutionEvent) {
        let _ = self.event_bus.publish(&DomainEventEnum::Execution(event));
    }
}

impl Default for ExecutionManagerDomainService {
    fn default() -> Self {
        Self::new(Arc::new(DomainEventBus::new()), 8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_state_is_active() {
        assert!(ExecutionState::Running.is_active());
        assert!(ExecutionState::Waiting.is_active());
        assert!(!ExecutionState::Ready.is_active());
        assert!(!ExecutionState::Completed.is_active());
        assert!(!ExecutionState::Failed.is_active());
    }

    #[test]
    fn test_execution_state_is_terminal() {
        assert!(ExecutionState::Completed.is_terminal());
        assert!(ExecutionState::Failed.is_terminal());
        assert!(!ExecutionState::Ready.is_terminal());
        assert!(!ExecutionState::Running.is_terminal());
        assert!(!ExecutionState::Waiting.is_terminal());
    }

    #[test]
    fn test_execution_priority_conversions() {
        assert_eq!(ExecutionPriority::Low.as_level(), 0);
        assert_eq!(ExecutionPriority::Normal.as_level(), 1);
        assert_eq!(ExecutionPriority::High.as_level(), 2);
        assert_eq!(ExecutionPriority::Critical.as_level(), 3);

        assert_eq!(ExecutionPriority::from_level(0), ExecutionPriority::Low);
        assert_eq!(ExecutionPriority::from_level(1), ExecutionPriority::Normal);
        assert_eq!(ExecutionPriority::from_level(2), ExecutionPriority::High);
        assert_eq!(
            ExecutionPriority::from_level(5),
            ExecutionPriority::Critical
        );
    }

    #[test]
    fn test_execution_priority_ord() {
        assert!(ExecutionPriority::Critical > ExecutionPriority::High);
        assert!(ExecutionPriority::High > ExecutionPriority::Normal);
        assert!(ExecutionPriority::Normal > ExecutionPriority::Low);
    }

    #[test]
    fn test_execution_context_creation() {
        let context = ExecutionContext::new(1, GuestAddr(0x1000));
        assert_eq!(context.id, 1);
        assert_eq!(context.pc, GuestAddr(0x1000));
        assert_eq!(context.state, ExecutionState::Ready);
        assert_eq!(context.instructions_executed, 0);
    }

    #[test]
    fn test_execution_context_update() {
        let mut context = ExecutionContext::new(1, GuestAddr(0x1000));
        context.update(GuestAddr(0x2000));
        assert_eq!(context.pc, GuestAddr(0x2000));
    }

    #[test]
    fn test_execution_statistics_rates() {
        let stats = ExecutionStatistics {
            total_executions: 100,
            successful_executions: 80,
            failed_executions: 20,
            total_instructions: 1000000,
            ..Default::default()
        };
        assert_eq!(stats.success_rate(), 0.8);
        assert_eq!(stats.failure_rate(), 0.2);
        assert_eq!(stats.avg_instructions_per_execution(), 10000.0);
    }

    #[test]
    fn test_execution_statistics_zero_executions() {
        let stats = ExecutionStatistics::default();
        assert_eq!(stats.success_rate(), 0.0);
        assert_eq!(stats.failure_rate(), 0.0);
        assert_eq!(stats.avg_instructions_per_execution(), 0.0);
    }

    #[test]
    fn test_execution_manager_creation() {
        let manager = ExecutionManagerDomainService::default();
        assert_eq!(manager.max_active_contexts, 8);
    }

    #[test]
    fn test_execution_manager_create_context() {
        let mut manager = ExecutionManagerDomainService::default();
        manager
            .create_context(1, GuestAddr(0x1000))
            .expect("Failed to create context");
        assert!(manager.get_context(1).is_some());
    }

    #[test]
    fn test_execution_manager_delete_context() {
        let mut manager = ExecutionManagerDomainService::default();
        manager
            .create_context(1, GuestAddr(0x1000))
            .expect("Failed to create context");
        manager.delete_context(1).expect("Failed to delete context");
        assert!(manager.get_context(1).is_none());
    }

    #[test]
    fn test_execution_manager_schedule() {
        let mut manager = ExecutionManagerDomainService::default();
        manager
            .create_context(1, GuestAddr(0x1000))
            .expect("Failed to create context");
        manager
            .schedule(1, ExecutionPriority::High)
            .expect("Failed to schedule context");
        assert!(manager.next_context().is_some());
    }

    #[test]
    fn test_execution_manager_complete_execution() {
        let mut manager = ExecutionManagerDomainService::default();
        manager
            .create_context(1, GuestAddr(0x1000))
            .expect("Failed to create context");
        manager.next_context().expect("Failed to get next context");
        let ctx = manager.get_context_mut(1).expect("Failed to get context");
        ctx.instructions_executed = 1000;
        manager
            .complete_execution(1)
            .expect("Failed to complete execution");
        assert_eq!(manager.get_statistics().successful_executions, 1);
    }
}
