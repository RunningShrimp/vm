//! Multi-threading debugging support
//!
//! This module provides comprehensive multi-threading debugging capabilities including
//! thread state management, thread-specific breakpoints, and concurrent execution control.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock, Mutex};
use std::thread::ThreadId;
use serde::{Deserialize, Serialize};
use crate::{GuestAddr, VmError, VmResult};

/// Thread state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadState {
    /// Thread ID
    pub thread_id: u32,
    /// Native thread ID
    pub native_thread_id: ThreadId,
    /// Thread name (if available)
    pub name: Option<String>,
    /// Current execution state
    pub execution_state: ThreadExecutionState,
    /// Current instruction pointer
    pub instruction_pointer: GuestAddr,
    /// Current stack pointer
    pub stack_pointer: GuestAddr,
    /// Current frame pointer
    pub frame_pointer: Option<GuestAddr>,
    /// Thread registers
    pub registers: HashMap<String, u64>,
    /// Thread creation timestamp
    pub created_at: std::time::SystemTime,
    /// Last activity timestamp
    pub last_activity: std::time::SystemTime,
    /// Thread priority
    pub priority: ThreadPriority,
    /// Thread status
    pub status: ThreadStatus,
    /// CPU affinity (if applicable)
    pub cpu_affinity: Option<Vec<u32>>,
    /// Architecture-specific thread data
    pub arch_specific: HashMap<String, String>,
}

/// Thread execution states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThreadExecutionState {
    /// Thread is running
    Running,
    /// Thread is stopped at breakpoint
    Stopped,
    /// Thread is stepping
    Stepping,
    /// Thread is waiting for resource
    Waiting,
    /// Thread is terminated
    Terminated,
    /// Thread is suspended
    Suspended,
    /// Thread is in system call
    InSyscall,
    /// Thread is in exception handling
    InException,
}

/// Thread priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThreadPriority {
    /// Idle priority
    Idle,
    /// Low priority
    Low,
    /// Normal priority
    Normal,
    /// High priority
    High,
    /// Real-time priority
    Realtime,
}

/// Thread status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThreadStatus {
    /// Thread is active
    Active,
    /// Thread is sleeping
    Sleeping,
    /// Thread is blocked
    Blocked,
    /// Thread is zombie (terminated but not yet cleaned up)
    Zombie,
    /// Thread is detached
    Detached,
}

/// Thread event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThreadEvent {
    /// Thread creation
    ThreadCreated {
        thread_id: u32,
        native_thread_id: ThreadId,
        name: Option<String>,
        timestamp: std::time::SystemTime,
    },
    /// Thread termination
    ThreadTerminated {
        thread_id: u32,
        exit_code: i32,
        timestamp: std::time::SystemTime,
    },
    /// Thread state change
    ThreadStateChanged {
        thread_id: u32,
        old_state: ThreadExecutionState,
        new_state: ThreadExecutionState,
        timestamp: std::time::SystemTime,
    },
    /// Thread priority change
    ThreadPriorityChanged {
        thread_id: u32,
        old_priority: ThreadPriority,
        new_priority: ThreadPriority,
        timestamp: std::time::SystemTime,
    },
    /// Thread context switch
    ThreadContextSwitch {
        from_thread_id: u32,
        to_thread_id: u32,
        timestamp: std::time::SystemTime,
    },
    /// Thread synchronization event
    ThreadSynchronization {
        thread_id: u32,
        sync_type: SynchronizationType,
        sync_object: String,
        timestamp: std::time::SystemTime,
    },
}

/// Synchronization event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SynchronizationType {
    /// Mutex lock
    MutexLock,
    /// Mutex unlock
    MutexUnlock,
    /// Condition variable wait
    CondvarWait,
    /// Condition variable signal
    CondvarSignal,
    /// Semaphore acquire
    SemaphoreAcquire,
    /// Semaphore release
    SemaphoreRelease,
    /// Read/write lock acquire
    RwLockAcquire,
    /// Read/write lock release
    RwLockRelease,
}

/// Thread breakpoint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadBreakpoint {
    /// Breakpoint ID
    pub id: u64,
    /// Thread ID this breakpoint applies to
    pub thread_id: u32,
    /// Breakpoint address
    pub address: GuestAddr,
    /// Breakpoint type
    pub breakpoint_type: crate::debugger::enhanced_breakpoints::BreakpointType,
    /// Whether breakpoint is enabled
    pub enabled: bool,
    /// Hit count for this thread
    pub hit_count: u64,
    /// Thread-specific condition
    pub thread_condition: Option<ThreadCondition>,
}

/// Thread-specific breakpoint conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThreadCondition {
    /// Trigger when thread is in specific state
    ThreadState { state: ThreadExecutionState },
    /// Trigger when thread priority matches
    ThreadPriority { priority: ThreadPriority },
    /// Trigger when thread is on specific CPU
    CpuAffinity { cpu_id: u32 },
    /// Complex condition (AND of multiple conditions)
    And { conditions: Vec<ThreadCondition> },
    /// Complex condition (OR of multiple conditions)
    Or { conditions: Vec<ThreadCondition> },
}

/// Multi-threading debugging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiThreadDebugConfig {
    /// Maximum number of threads to track
    pub max_threads: usize,
    /// Enable thread state tracking
    pub enable_state_tracking: bool,
    /// Enable thread-specific breakpoints
    pub enable_thread_breakpoints: bool,
    /// Enable context switch tracking
    pub enable_context_switch_tracking: bool,
    /// Enable synchronization event tracking
    pub enable_sync_tracking: bool,
    /// Thread state history size
    pub state_history_size: usize,
    /// Enable thread performance monitoring
    pub enable_performance_monitoring: bool,
}

impl Default for MultiThreadDebugConfig {
    fn default() -> Self {
        Self {
            max_threads: 256,
            enable_state_tracking: true,
            enable_thread_breakpoints: true,
            enable_context_switch_tracking: true,
            enable_sync_tracking: true,
            state_history_size: 100,
            enable_performance_monitoring: true,
        }
    }
}

/// Multi-threading debugger
pub struct MultiThreadDebugger {
    /// Configuration
    config: MultiThreadDebugConfig,
    /// Thread states by ID
    thread_states: Arc<RwLock<HashMap<u32, ThreadState>>>,
    /// Thread states by native thread ID
    thread_states_by_native: Arc<RwLock<HashMap<ThreadId, ThreadState>>>,
    /// Thread-specific breakpoints
    thread_breakpoints: Arc<RwLock<HashMap<u64, ThreadBreakpoint>>>,
    /// Thread events
    thread_events: Arc<RwLock<Vec<ThreadEvent>>>,
    /// Next thread ID
    next_thread_id: Arc<RwLock<u32>>,
    /// Next breakpoint ID
    next_breakpoint_id: Arc<RwLock<u64>>,
    /// Current thread (thread being debugged)
    current_thread: Arc<RwLock<Option<u32>>>,
    /// Thread state history
    state_history: Arc<RwLock<HashMap<u32, VecDeque<ThreadExecutionState>>>>,
    /// Performance statistics
    performance_stats: Arc<RwLock<HashMap<u32, ThreadPerformanceStats>>>,
}

/// Thread performance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadPerformanceStats {
    /// Thread ID
    pub thread_id: u32,
    /// Total execution time in nanoseconds
    pub total_execution_time_ns: u64,
    /// Total waiting time in nanoseconds
    pub total_wait_time_ns: u64,
    /// Number of context switches
    pub context_switches: u64,
    /// Number of breakpoints hit
    pub breakpoints_hit: u64,
    /// Number of system calls made
    pub syscalls: u64,
    /// CPU usage percentage
    pub cpu_usage_percent: f64,
    /// Last update timestamp
    pub last_update: std::time::SystemTime,
}

impl MultiThreadDebugger {
    /// Create a new multi-threading debugger
    pub fn new(config: MultiThreadDebugConfig) -> Self {
        Self {
            config,
            thread_states: Arc::new(RwLock::new(HashMap::new())),
            thread_states_by_native: Arc::new(RwLock::new(HashMap::new())),
            thread_breakpoints: Arc::new(RwLock::new(HashMap::new())),
            thread_events: Arc::new(RwLock::new(Vec::new())),
            next_thread_id: Arc::new(RwLock::new(1)),
            next_breakpoint_id: Arc::new(RwLock::new(1)),
            current_thread: Arc::new(RwLock::new(None)),
            state_history: Arc::new(RwLock::new(HashMap::new())),
            performance_stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new thread
    pub fn register_thread(
        &self,
        native_thread_id: ThreadId,
        name: Option<String>,
        priority: ThreadPriority,
    ) -> VmResult<u32> {
        // Check thread limit
        {
            let thread_states = self.thread_states.read().unwrap();
            if thread_states.len() >= self.config.max_threads {
                return Err(VmError::Core(crate::error::CoreError::InvalidState {
                    message: format!("Maximum thread count {} exceeded", self.config.max_threads),
                    current: format!("{}", thread_states.len()),
                    expected: format!("<= {}", self.config.max_threads),
                }));
            }
        }

        // Generate thread ID
        let mut next_id = self.next_thread_id.write().unwrap();
        let thread_id = *next_id;
        *next_id += 1;

        // Create thread state
        let thread_state = ThreadState {
            thread_id,
            native_thread_id,
            name,
            execution_state: ThreadExecutionState::Running,
            instruction_pointer: 0,
            stack_pointer: 0,
            frame_pointer: None,
            registers: HashMap::new(),
            created_at: std::time::SystemTime::now(),
            last_activity: std::time::SystemTime::now(),
            priority,
            status: ThreadStatus::Active,
            cpu_affinity: None,
            arch_specific: HashMap::new(),
        };

        // Add to thread states
        {
            let mut thread_states = self.thread_states.write().unwrap();
            thread_states.insert(thread_id, thread_state.clone());
        }

        {
            let mut thread_states_by_native = self.thread_states_by_native.write().unwrap();
            thread_states_by_native.insert(native_thread_id, thread_state.clone());
        }

        // Initialize state history
        if self.config.enable_state_tracking {
            let mut state_history = self.state_history.write().unwrap();
            state_history.insert(thread_id, VecDeque::new());
        }

        // Initialize performance stats
        if self.config.enable_performance_monitoring {
            let mut performance_stats = self.performance_stats.write().unwrap();
            performance_stats.insert(thread_id, ThreadPerformanceStats {
                thread_id,
                total_execution_time_ns: 0,
                total_wait_time_ns: 0,
                context_switches: 0,
                breakpoints_hit: 0,
                syscalls: 0,
                cpu_usage_percent: 0.0,
                last_update: std::time::SystemTime::now(),
            });
        }

        // Record thread creation event
        {
            let mut thread_events = self.thread_events.write().unwrap();
            thread_events.push(ThreadEvent::ThreadCreated {
                thread_id,
                native_thread_id,
                name: thread_state.name.clone(),
                timestamp: std::time::SystemTime::now(),
            });
        }

        Ok(thread_id)
    }

    /// Unregister a thread
    pub fn unregister_thread(&self, thread_id: u32, exit_code: i32) -> VmResult<ThreadState> {
        // Get thread state
        let thread_state = {
            let mut thread_states = self.thread_states.write().unwrap();
            thread_states.remove(&thread_id)
                .ok_or_else(|| VmError::Core(crate::error::CoreError::InvalidState {
                    message: format!("Thread {} not found", thread_id),
                    current: "N/A".to_string(),
                    expected: format!("Thread {} to exist", thread_id),
                }))?
        };

        // Remove from native thread mapping
        {
            let mut thread_states_by_native = self.thread_states_by_native.write().unwrap();
            thread_states_by_native.remove(&thread_state.native_thread_id);
        }

        // Remove thread-specific breakpoints
        if self.config.enable_thread_breakpoints {
            let mut thread_breakpoints = self.thread_breakpoints.write().unwrap();
            thread_breakpoints.retain(|_, bp| bp.thread_id != thread_id);
        }

        // Remove state history
        if self.config.enable_state_tracking {
            let mut state_history = self.state_history.write().unwrap();
            state_history.remove(&thread_id);
        }

        // Remove performance stats
        if self.config.enable_performance_monitoring {
            let mut performance_stats = self.performance_stats.write().unwrap();
            performance_stats.remove(&thread_id);
        }

        // Record thread termination event
        {
            let mut thread_events = self.thread_events.write().unwrap();
            thread_events.push(ThreadEvent::ThreadTerminated {
                thread_id,
                exit_code,
                timestamp: std::time::SystemTime::now(),
            });
        }

        // Clear current thread if this was it
        {
            let mut current_thread = self.current_thread.write().unwrap();
            if *current_thread == Some(thread_id) {
                *current_thread = None;
            }
        }

        Ok(thread_state)
    }

    /// Get thread state by ID
    pub fn get_thread_state(&self, thread_id: u32) -> VmResult<Option<ThreadState>> {
        let thread_states = self.thread_states.read().unwrap();
        Ok(thread_states.get(&thread_id).cloned())
    }

    /// Get thread state by native thread ID
    pub fn get_thread_state_by_native(&self, native_thread_id: ThreadId) -> VmResult<Option<ThreadState>> {
        let thread_states_by_native = self.thread_states_by_native.read().unwrap();
        Ok(thread_states_by_native.get(&native_thread_id).cloned())
    }

    /// Get all thread states
    pub fn get_all_threads(&self) -> VmResult<Vec<ThreadState>> {
        let thread_states = self.thread_states.read().unwrap();
        Ok(thread_states.values().cloned().collect())
    }

    /// Set current thread for debugging
    pub fn set_current_thread(&self, thread_id: Option<u32>) -> VmResult<()> {
        if let Some(id) = thread_id {
            // Verify thread exists
            let thread_states = self.thread_states.read().unwrap();
            if !thread_states.contains_key(&id) {
                return Err(VmError::Core(crate::error::CoreError::InvalidState {
                    message: format!("Thread {} not found", id),
                    current: "N/A".to_string(),
                    expected: format!("Thread {} to exist", id),
                }));
            }
        }

        let mut current_thread = self.current_thread.write().unwrap();
        *current_thread = thread_id;
        Ok(())
    }

    /// Get current thread
    pub fn get_current_thread(&self) -> Option<u32> {
        *self.current_thread.read().unwrap()
    }

    /// Update thread execution state
    pub fn update_thread_state(
        &self,
        thread_id: u32,
        new_state: ThreadExecutionState,
    ) -> VmResult<()> {
        let old_state = {
            let mut thread_states = self.thread_states.write().unwrap();
            if let Some(thread_state) = thread_states.get_mut(&thread_id) {
                let old_state = thread_state.execution_state;
                thread_state.execution_state = new_state;
                thread_state.last_activity = std::time::SystemTime::now();
                old_state
            } else {
                return Err(VmError::Core(crate::error::CoreError::InvalidState {
                    message: format!("Thread {} not found", thread_id),
                    current: "N/A".to_string(),
                    expected: format!("Thread {} to exist", thread_id),
                }));
            }
        };

        // Update state history
        if self.config.enable_state_tracking {
            let mut state_history = self.state_history.write().unwrap();
            if let Some(history) = state_history.get_mut(&thread_id) {
                history.push_back(new_state);
                if history.len() > self.config.state_history_size {
                    history.pop_front();
                }
            }
        }

        // Record state change event
        if self.config.enable_context_switch_tracking {
            let mut thread_events = self.thread_events.write().unwrap();
            thread_events.push(ThreadEvent::ThreadStateChanged {
                thread_id,
                old_state,
                new_state,
                timestamp: std::time::SystemTime::now(),
            });
        }

        Ok(())
    }

    /// Update thread registers
    pub fn update_thread_registers(
        &self,
        thread_id: u32,
        registers: HashMap<String, u64>,
    ) -> VmResult<()> {
        let mut thread_states = self.thread_states.write().unwrap();
        if let Some(thread_state) = thread_states.get_mut(&thread_id) {
            thread_state.registers = registers;
            thread_state.last_activity = std::time::SystemTime::now();
            Ok(())
        } else {
            Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: format!("Thread {} not found", thread_id),
                current: "N/A".to_string(),
                expected: format!("Thread {} to exist", thread_id),
            }))
        }
    }

    /// Update thread instruction pointer
    pub fn update_thread_instruction_pointer(
        &self,
        thread_id: u32,
        ip: GuestAddr,
    ) -> VmResult<()> {
        let mut thread_states = self.thread_states.write().unwrap();
        if let Some(thread_state) = thread_states.get_mut(&thread_id) {
            thread_state.instruction_pointer = ip;
            thread_state.last_activity = std::time::SystemTime::now();
            Ok(())
        } else {
            Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: format!("Thread {} not found", thread_id),
                current: "N/A".to_string(),
                expected: format!("Thread {} to exist", thread_id),
            }))
        }
    }

    /// Add thread-specific breakpoint
    pub fn add_thread_breakpoint(
        &self,
        thread_id: u32,
        address: GuestAddr,
        breakpoint_type: crate::debugger::enhanced_breakpoints::BreakpointType,
        thread_condition: Option<ThreadCondition>,
    ) -> VmResult<u64> {
        if !self.config.enable_thread_breakpoints {
            return Err(VmError::Core(crate::error::CoreError::UnsupportedOperation {
                operation: "Thread-specific breakpoints".to_string(),
                reason: "Thread breakpoints are disabled".to_string(),
            }));
        }

        // Generate breakpoint ID
        let mut next_id = self.next_breakpoint_id.write().unwrap();
        let breakpoint_id = *next_id;
        *next_id += 1;

        // Create thread breakpoint
        let thread_breakpoint = ThreadBreakpoint {
            id: breakpoint_id,
            thread_id,
            address,
            breakpoint_type,
            enabled: true,
            hit_count: 0,
            thread_condition,
        };

        // Add to breakpoints
        {
            let mut thread_breakpoints = self.thread_breakpoints.write().unwrap();
            thread_breakpoints.insert(breakpoint_id, thread_breakpoint);
        }

        Ok(breakpoint_id)
    }

    /// Remove thread-specific breakpoint
    pub fn remove_thread_breakpoint(&self, breakpoint_id: u64) -> VmResult<ThreadBreakpoint> {
        let mut thread_breakpoints = self.thread_breakpoints.write().unwrap();
        thread_breakpoints.remove(&breakpoint_id)
            .ok_or_else(|| VmError::Core(crate::error::CoreError::InvalidState {
                message: format!("Thread breakpoint {} not found", breakpoint_id),
                current: "N/A".to_string(),
                expected: format!("Thread breakpoint {} to exist", breakpoint_id),
            }))
    }

    /// Check thread-specific breakpoints
    pub fn check_thread_breakpoints(
        &self,
        thread_id: u32,
        address: GuestAddr,
    ) -> Vec<ThreadBreakpoint> {
        if !self.config.enable_thread_breakpoints {
            return Vec::new();
        }

        let thread_breakpoints = self.thread_breakpoints.read().unwrap();
        let mut triggered_breakpoints = Vec::new();

        for breakpoint in thread_breakpoints.values() {
            if breakpoint.thread_id == thread_id && 
               breakpoint.address == address && 
               breakpoint.enabled {
                
                // Check thread-specific condition
                let should_trigger = if let Some(ref condition) = breakpoint.thread_condition {
                    self.evaluate_thread_condition(condition, thread_id)
                } else {
                    true
                };

                if should_trigger {
                    triggered_breakpoints.push(breakpoint.clone());
                }
            }
        }

        triggered_breakpoints
    }

    /// Record thread synchronization event
    pub fn record_sync_event(
        &self,
        thread_id: u32,
        sync_type: SynchronizationType,
        sync_object: String,
    ) -> VmResult<()> {
        if !self.config.enable_sync_tracking {
            return Ok(());
        }

        let mut thread_events = self.thread_events.write().unwrap();
        thread_events.push(ThreadEvent::ThreadSynchronization {
            thread_id,
            sync_type,
            sync_object,
            timestamp: std::time::SystemTime::now(),
        });

        Ok(())
    }

    /// Get thread events
    pub fn get_thread_events(&self) -> Vec<ThreadEvent> {
        let thread_events = self.thread_events.read().unwrap();
        thread_events.clone()
    }

    /// Get thread state history
    pub fn get_thread_state_history(&self, thread_id: u32) -> Vec<ThreadExecutionState> {
        if !self.config.enable_state_tracking {
            return Vec::new();
        }

        let state_history = self.state_history.read().unwrap();
        if let Some(history) = state_history.get(&thread_id) {
            history.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Get thread performance statistics
    pub fn get_thread_performance_stats(&self, thread_id: u32) -> Option<ThreadPerformanceStats> {
        if !self.config.enable_performance_monitoring {
            return None;
        }

        let performance_stats = self.performance_stats.read().unwrap();
        performance_stats.get(&thread_id).cloned()
    }

    /// Get multi-threading statistics
    pub fn get_statistics(&self) -> MultiThreadDebugStatistics {
        let thread_states = self.thread_states.read().unwrap();
        let thread_events = self.thread_events.read().unwrap();
        let thread_breakpoints = self.thread_breakpoints.read().unwrap();
        let performance_stats = self.performance_stats.read().unwrap();

        let total_threads = thread_states.len();
        let active_threads = thread_states.values()
            .filter(|t| t.status == ThreadStatus::Active)
            .count();
        let stopped_threads = thread_states.values()
            .filter(|t| t.execution_state == ThreadExecutionState::Stopped)
            .count();
        let total_breakpoints = thread_breakpoints.len();
        let enabled_breakpoints = thread_breakpoints.values()
            .filter(|bp| bp.enabled)
            .count();

        // Calculate performance statistics
        let total_execution_time: u64 = performance_stats.values()
            .map(|stats| stats.total_execution_time_ns)
            .sum();
        let total_wait_time: u64 = performance_stats.values()
            .map(|stats| stats.total_wait_time_ns)
            .sum();
        let total_context_switches: u64 = performance_stats.values()
            .map(|stats| stats.context_switches)
            .sum();

        MultiThreadDebugStatistics {
            total_threads,
            active_threads,
            stopped_threads,
            total_breakpoints,
            enabled_breakpoints,
            total_execution_time_ns: total_execution_time,
            total_wait_time_ns: total_wait_time,
            total_context_switches,
            average_cpu_usage: if total_threads > 0 {
                performance_stats.values()
                    .map(|stats| stats.cpu_usage_percent)
                    .sum::<f64>() / total_threads as f64
            } else {
                0.0
            },
        }
    }

    /// Evaluate thread condition
    fn evaluate_thread_condition(&self, condition: &ThreadCondition, thread_id: u32) -> bool {
        let thread_states = self.thread_states.read().unwrap();
        
        if let Some(thread_state) = thread_states.get(&thread_id) {
            match condition {
                ThreadCondition::ThreadState { state } => thread_state.execution_state == *state,
                ThreadCondition::ThreadPriority { priority } => thread_state.priority == *priority,
                ThreadCondition::CpuAffinity { cpu_id } => {
                    thread_state.cpu_affinity
                        .as_ref()
                        .map_or(false, |affinity| affinity.contains(cpu_id))
                }
                ThreadCondition::And { conditions } => {
                    conditions.iter().all(|c| self.evaluate_thread_condition(c, thread_id))
                }
                ThreadCondition::Or { conditions } => {
                    conditions.iter().any(|c| self.evaluate_thread_condition(c, thread_id))
                }
            }
        } else {
            false
        }
    }
}

/// Multi-threading debugging statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiThreadDebugStatistics {
    /// Total number of threads
    pub total_threads: usize,
    /// Number of active threads
    pub active_threads: usize,
    /// Number of stopped threads
    pub stopped_threads: usize,
    /// Total thread-specific breakpoints
    pub total_breakpoints: usize,
    /// Number of enabled breakpoints
    pub enabled_breakpoints: usize,
    /// Total execution time in nanoseconds
    pub total_execution_time_ns: u64,
    /// Total wait time in nanoseconds
    pub total_wait_time_ns: u64,
    /// Total context switches
    pub total_context_switches: u64,
    /// Average CPU usage across all threads
    pub average_cpu_usage: f64,
}

impl Default for MultiThreadDebugger {
    fn default() -> Self {
        Self::new(MultiThreadDebugConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_thread_registration() {
        let config = MultiThreadDebugConfig::default();
        let debugger = MultiThreadDebugger::new(config);

        // Register a thread
        let native_id = thread::current().id();
        let thread_id = debugger.register_thread(
            native_id,
            Some("test_thread".to_string()),
            ThreadPriority::Normal,
        ).unwrap();

        assert_eq!(thread_id, 1);

        // Check thread state
        let thread_state = debugger.get_thread_state(thread_id).unwrap();
        assert!(thread_state.is_some());
        assert_eq!(thread_state.unwrap().thread_id, thread_id);
        assert_eq!(thread_state.unwrap().name, Some("test_thread".to_string()));
    }

    #[test]
    fn test_thread_state_updates() {
        let config = MultiThreadDebugConfig::default();
        let debugger = MultiThreadDebugger::new(config);

        let native_id = thread::current().id();
        let thread_id = debugger.register_thread(
            native_id,
            Some("test_thread".to_string()),
            ThreadPriority::Normal,
        ).unwrap();

        // Update thread state
        debugger.update_thread_state(thread_id, ThreadExecutionState::Stopped).unwrap();

        // Check state history
        let history = debugger.get_thread_state_history(thread_id);
        assert_eq!(history.len(), 2); // Initial Running + Stopped
        assert_eq!(history[1], ThreadExecutionState::Stopped);
    }

    #[test]
    fn test_thread_breakpoints() {
        let config = MultiThreadDebugConfig::default();
        let debugger = MultiThreadDebugger::new(config);

        let native_id = thread::current().id();
        let thread_id = debugger.register_thread(
            native_id,
            Some("test_thread".to_string()),
            ThreadPriority::Normal,
        ).unwrap();

        // Add thread-specific breakpoint
        let bp_id = debugger.add_thread_breakpoint(
            thread_id,
            0x1000,
            crate::debugger::enhanced_breakpoints::BreakpointType::Execution,
            Some(ThreadCondition::ThreadState { 
                state: ThreadExecutionState::Running 
            }),
        ).unwrap();

        // Check breakpoints
        let triggered_bps = debugger.check_thread_breakpoints(thread_id, 0x1000);
        assert_eq!(triggered_bps.len(), 1);
        assert_eq!(triggered_bps[0].id, bp_id);
    }

    #[test]
    fn test_thread_performance_stats() {
        let config = MultiThreadDebugConfig::default();
        let debugger = MultiThreadDebugger::new(config);

        let native_id = thread::current().id();
        let thread_id = debugger.register_thread(
            native_id,
            Some("test_thread".to_string()),
            ThreadPriority::Normal,
        ).unwrap();

        // Get performance stats
        let stats = debugger.get_thread_performance_stats(thread_id);
        assert!(stats.is_some());
        assert_eq!(stats.unwrap().thread_id, thread_id);
    }
}
