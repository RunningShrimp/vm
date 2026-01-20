//! Unified debugger interface
//!
//! This module provides a comprehensive unified debugger interface that integrates
//! all debugging capabilities including breakpoints, call stack tracking, symbol table,
//! and multi-threading support.

#![cfg(feature = "debug")]

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::{GuestAddr, VmError, VmResult};

// Import all debugging modules
use super::enhanced_breakpoints::{
    BreakpointManager, Breakpoint, BreakpointType, 
    BreakpointCondition, BreakpointGroup
};
use super::call_stack_tracker::{
    CallStackTracker, StackFrame, LocalVariable, 
    VariableLocation, VariableValue, VariableScope
};
use super::symbol_table::{
    SymbolTable, Symbol, SymbolType, SymbolScope,
    FunctionInfo, SourceLocation, LineInfo
};
use super::multi_thread_debug::{
    MultiThreadDebugger, ThreadState, ThreadExecutionState,
    ThreadEvent, ThreadBreakpoint
};

/// Unified debugger configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedDebuggerConfig {
    /// Breakpoint manager configuration
    pub breakpoint_config: crate::debugger::enhanced_breakpoints::BreakpointBuilder,
    /// Call stack tracker configuration
    pub call_stack_config: crate::debugger::call_stack_tracker::CallStackBuilder,
    /// Symbol table configuration
    pub symbol_table_config: crate::debugger::symbol_table::SymbolTableBuilder,
    /// Multi-threading configuration
    pub multi_thread_config: crate::debugger::multi_thread_debug::MultiThreadDebugConfig,
    /// Enable source-level debugging
    pub enable_source_level_debugging: bool,
    /// Enable performance monitoring
    pub enable_performance_monitoring: bool,
    /// Enable memory tracking
    pub enable_memory_tracking: bool,
    /// Enable instruction tracing
    pub enable_instruction_tracing: bool,
    /// Maximum trace buffer size
    pub max_trace_buffer_size: usize,
    /// Enable auto-breakpoint on exceptions
    pub auto_break_on_exception: bool,
    /// Enable step-over optimization
    pub enable_step_over_optimization: bool,
}

impl Default for UnifiedDebuggerConfig {
    fn default() -> Self {
        Self {
            breakpoint_config: crate::debugger::enhanced_breakpoints::BreakpointBuilder::default(),
            call_stack_config: crate::debugger::call_stack_tracker::CallStackBuilder::default(),
            symbol_table_config: crate::debugger::symbol_table::SymbolTableBuilder::default(),
            multi_thread_config: crate::debugger::multi_thread_debug::MultiThreadDebugConfig::default(),
            enable_source_level_debugging: true,
            enable_performance_monitoring: true,
            enable_memory_tracking: true,
            enable_instruction_tracing: true,
            max_trace_buffer_size: 10000,
            auto_break_on_exception: true,
            enable_step_over_optimization: true,
        }
    }
}

/// Debugger execution state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebuggerState {
    /// Debugger is inactive
    Inactive,
    /// Debugger is active and VM is running
    Running,
    /// Debugger is active and VM is paused
    Paused,
    /// Debugger is stepping through instructions
    Stepping,
    /// Debugger is in error state
    Error,
}

/// Debugger event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DebuggerEvent {
    /// Debugger started
    DebuggerStarted {
        timestamp: std::time::SystemTime,
    },
    /// Debugger stopped
    DebuggerStopped {
        timestamp: std::time::SystemTime,
        reason: String,
    },
    /// Breakpoint hit
    BreakpointHit {
        breakpoint_id: u64,
        thread_id: u32,
        address: GuestAddr,
        timestamp: std::time::SystemTime,
    },
    /// Step completed
    StepCompleted {
        thread_id: u32,
        address: GuestAddr,
        instruction: Vec<u8>,
        timestamp: std::time::SystemTime,
    },
    /// Exception occurred
    Exception {
        thread_id: u32,
        exception_type: String,
        exception_address: GuestAddr,
        timestamp: std::time::SystemTime,
    },
    /// Thread event
    ThreadEvent(ThreadEvent),
    /// Call stack event
    CallStackEvent(crate::debugger::call_stack_tracker::CallStackEvent),
    /// Performance event
    Performance {
        thread_id: u32,
        metric: String,
        value: f64,
        timestamp: std::time::SystemTime,
    },
}

/// Instruction trace entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionTrace {
    /// Thread ID
    pub thread_id: u32,
    /// Instruction address
    pub address: GuestAddr,
    /// Instruction bytes
    pub instruction: Vec<u8>,
    /// Instruction disassembly
    pub disassembly: String,
    /// Registers before instruction
    pub registers_before: HashMap<String, u64>,
    /// Registers after instruction
    pub registers_after: HashMap<String, u64>,
    /// Memory accesses performed by instruction
    pub memory_accesses: Vec<MemoryAccess>,
    /// Execution time in nanoseconds
    pub execution_time_ns: u64,
    /// Timestamp
    pub timestamp: std::time::SystemTime,
}

/// Memory access information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAccess {
    /// Memory address
    pub address: GuestAddr,
    /// Access type (read/write)
    pub access_type: MemoryAccessType,
    /// Size of access in bytes
    pub size: usize,
    /// Value read or written
    pub value: Option<Vec<u8>>,
    /// Whether access was successful
    pub success: bool,
}

/// Memory access types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryAccessType {
    /// Read access
    Read,
    /// Write access
    Write,
    /// Atomic read-modify-write
    AtomicRMW,
    /// Cache prefetch
    Prefetch,
    /// Memory fence
    Fence,
}

/// Unified debugger interface
/// 
/// This provides a comprehensive debugging interface that integrates all debugging
/// capabilities into a single, easy-to-use API.
pub struct UnifiedDebugger {
    /// Configuration
    config: UnifiedDebuggerConfig,
    /// Current debugger state
    state: Arc<RwLock<DebuggerState>>,
    /// Breakpoint manager
    breakpoint_manager: Arc<BreakpointManager>,
    /// Call stack tracker
    call_stack_tracker: Arc<CallStackTracker>,
    /// Symbol table
    symbol_table: Arc<SymbolTable>,
    /// Multi-threading debugger
    multi_thread_debugger: Arc<MultiThreadDebugger>,
    /// Debugger events
    events: Arc<RwLock<Vec<DebuggerEvent>>>,
    /// Instruction trace buffer
    instruction_trace: Arc<RwLock<VecDeque<InstructionTrace>>>,
    /// Current thread being debugged
    current_thread: Arc<RwLock<Option<u32>>>,
    /// Step over information
    step_over_info: Arc<RwLock<Option<StepOverInfo>>>,
    /// Performance statistics
    performance_stats: Arc<RwLock<DebuggerPerformanceStats>>,
}

/// Step over information for function call stepping
#[derive(Debug, Clone)]
struct StepOverInfo {
    /// Thread ID being stepped
    thread_id: u32,
    /// Original breakpoint at function entry
    entry_breakpoint_id: u64,
    /// Temporary breakpoint at function return
    return_breakpoint_id: u64,
    /// Function return address
    return_address: GuestAddr,
    /// Stack depth at step start
    stack_depth: usize,
}

/// Debugger performance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebuggerPerformanceStats {
    /// Total instructions executed
    pub total_instructions: u64,
    /// Total breakpoints hit
    pub total_breakpoints_hit: u64,
    /// Total steps taken
    pub total_steps: u64,
    /// Total exceptions handled
    pub total_exceptions: u64,
    /// Average instruction execution time
    pub avg_instruction_time_ns: f64,
    /// Total debugging time
    pub total_debug_time_ns: u64,
    /// Memory access statistics
    pub memory_access_stats: MemoryAccessStats,
    /// Last update timestamp
    pub last_update: std::time::SystemTime,
}

/// Memory access statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAccessStats {
    /// Total memory reads
    pub total_reads: u64,
    /// Total memory writes
    pub total_writes: u64,
    /// Total bytes read
    pub total_bytes_read: u64,
    /// Total bytes written
    pub total_bytes_written: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
}

// Lock helper methods for UnifiedDebugger
impl UnifiedDebugger {
    /// Helper method to acquire state write lock
    fn lock_state_write(&self) -> Result<std::sync::RwLockWriteGuard<'_, DebuggerState>, VmError> {
        self.state.write().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire state write lock: {}", e),
            operation: "lock_state_write".to_string(),
        })
    }

    /// Helper method to acquire state read lock
    fn lock_state_read(&self) -> Result<std::sync::RwLockReadGuard<'_, DebuggerState>, VmError> {
        self.state.read().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire state read lock: {}", e),
            operation: "lock_state_read".to_string(),
        })
    }

    /// Helper method to acquire events write lock
    fn lock_events_write(&self) -> Result<std::sync::RwLockWriteGuard<'_, Vec<DebuggerEvent>>, VmError> {
        self.events.write().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire events write lock: {}", e),
            operation: "lock_events_write".to_string(),
        })
    }

    /// Helper method to acquire events read lock
    fn lock_events_read(&self) -> Result<std::sync::RwLockReadGuard<'_, Vec<DebuggerEvent>>, VmError> {
        self.events.read().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire events read lock: {}", e),
            operation: "lock_events_read".to_string(),
        })
    }

    /// Helper method to acquire current_thread write lock
    fn lock_current_thread_write(&self) -> Result<std::sync::RwLockWriteGuard<'_, Option<u32>>, VmError> {
        self.current_thread.write().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire current_thread write lock: {}", e),
            operation: "lock_current_thread_write".to_string(),
        })
    }

    /// Helper method to acquire current_thread read lock
    fn lock_current_thread_read(&self) -> Result<std::sync::RwLockReadGuard<'_, Option<u32>>, VmError> {
        self.current_thread.read().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire current_thread read lock: {}", e),
            operation: "lock_current_thread_read".to_string(),
        })
    }

    /// Helper method to acquire step_over_info write lock
    fn lock_step_over_info_write(&self) -> Result<std::sync::RwLockWriteGuard<'_, Option<StepOverInfo>>, VmError> {
        self.step_over_info.write().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire step_over_info write lock: {}", e),
            operation: "lock_step_over_info_write".to_string(),
        })
    }

    /// Helper method to acquire step_over_info read lock
    fn lock_step_over_info_read(&self) -> Result<std::sync::RwLockReadGuard<'_, Option<StepOverInfo>>, VmError> {
        self.step_over_info.read().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire step_over_info read lock: {}", e),
            operation: "lock_step_over_info_read".to_string(),
        })
    }

    /// Helper method to acquire instruction_trace write lock
    fn lock_instruction_trace_write(&self) -> Result<std::sync::RwLockWriteGuard<'_, VecDeque<InstructionTrace>>, VmError> {
        self.instruction_trace.write().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire instruction_trace write lock: {}", e),
            operation: "lock_instruction_trace_write".to_string(),
        })
    }

    /// Helper method to acquire instruction_trace read lock
    fn lock_instruction_trace_read(&self) -> Result<std::sync::RwLockReadGuard<'_, VecDeque<InstructionTrace>>, VmError> {
        self.instruction_trace.read().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire instruction_trace read lock: {}", e),
            operation: "lock_instruction_trace_read".to_string(),
        })
    }

    /// Helper method to acquire performance_stats write lock
    fn lock_performance_stats_write(&self) -> Result<std::sync::RwLockWriteGuard<'_, DebuggerPerformanceStats>, VmError> {
        self.performance_stats.write().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire performance_stats write lock: {}", e),
            operation: "lock_performance_stats_write".to_string(),
        })
    }

    /// Helper method to acquire performance_stats read lock
    fn lock_performance_stats_read(&self) -> Result<std::sync::RwLockReadGuard<'_, DebuggerPerformanceStats>, VmError> {
        self.performance_stats.read().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire performance_stats read lock: {}", e),
            operation: "lock_performance_stats_read".to_string(),
        })
    }
}

impl UnifiedDebugger {
    /// Create a new unified debugger
    pub fn new(config: UnifiedDebuggerConfig) -> VmResult<Self> {
        // Create component debuggers
        let breakpoint_manager = Arc::new(BreakpointManager::new());
        let call_stack_tracker = Arc::new(CallStackTracker::new(
            config.call_stack_config.clone(),
            0, // Default stack base
        ));
        let symbol_table = Arc::new(SymbolTable::new(config.symbol_table_config.clone()));
        let multi_thread_debugger = Arc::new(MultiThreadDebugger::new(config.multi_thread_config.clone()));

        Ok(Self {
            config,
            state: Arc::new(RwLock::new(DebuggerState::Inactive)),
            breakpoint_manager,
            call_stack_tracker,
            symbol_table,
            multi_thread_debugger,
            events: Arc::new(RwLock::new(Vec::new())),
            instruction_trace: Arc::new(RwLock::new(VecDeque::new())),
            current_thread: Arc::new(RwLock::new(None)),
            step_over_info: Arc::new(RwLock::new(None)),
            performance_stats: Arc::new(RwLock::new(DebuggerPerformanceStats {
                total_instructions: 0,
                total_breakpoints_hit: 0,
                total_steps: 0,
                total_exceptions: 0,
                avg_instruction_time_ns: 0.0,
                total_debug_time_ns: 0,
                memory_access_stats: MemoryAccessStats {
                    total_reads: 0,
                    total_writes: 0,
                    total_bytes_read: 0,
                    total_bytes_written: 0,
                    cache_hits: 0,
                    cache_misses: 0,
                },
                last_update: std::time::SystemTime::now(),
            })),
        })
    }

    /// Start the debugger
    pub fn start(&self) -> VmResult<()> {
        let mut state = self.lock_state_write()?;
        *state = DebuggerState::Running;

        // Record debugger started event
        let mut events = self.lock_events_write()?;
        events.push(DebuggerEvent::DebuggerStarted {
            timestamp: std::time::SystemTime::now(),
        });

        Ok(())
    }

    /// Stop the debugger
    pub fn stop(&self, reason: String) -> VmResult<()> {
        let mut state = self.lock_state_write()?;
        *state = DebuggerState::Inactive;

        // Record debugger stopped event
        let mut events = self.lock_events_write()?;
        events.push(DebuggerEvent::DebuggerStopped {
            timestamp: std::time::SystemTime::now(),
            reason,
        });

        Ok(())
    }

    /// Pause execution
    pub fn pause(&self) -> VmResult<()> {
        let mut state = self.lock_state_write()?;
        *state = DebuggerState::Paused;
        Ok(())
    }

    /// Resume execution
    pub fn resume(&self) -> VmResult<()> {
        let mut state = self.lock_state_write()?;
        *state = DebuggerState::Running;
        Ok(())
    }

    /// Set a breakpoint
    pub fn set_breakpoint(
        &self,
        address: GuestAddr,
        breakpoint_type: BreakpointType,
        condition: Option<BreakpointCondition>,
        thread_id: Option<u32>,
    ) -> VmResult<u64> {
        let mut breakpoint = match breakpoint_type {
            BreakpointType::Execution => {
                // Get original bytes at address (would need VM interface)
                let original_bytes = vec![0xCC, 0xCC]; // Placeholder
                Breakpoint::new_execution(0, address, original_bytes)
            }
            BreakpointType::Read => {
                Breakpoint::new_read_watchpoint(0, address, 4)
            }
            BreakpointType::Write => {
                Breakpoint::new_write_watchpoint(0, address, 4)
            }
            _ => {
                return Err(VmError::Core(crate::error::CoreError::UnsupportedOperation {
                    operation: format!("Breakpoint type {:?}", breakpoint_type),
                    reason: "Not yet implemented".to_string(),
                }));
            }
        };

        // Set condition if provided
        if let Some(cond) = condition {
            breakpoint.condition = cond;
        }

        // Set thread ID if provided
        if let Some(tid) = thread_id {
            breakpoint.thread_id = tid;
        }

        self.breakpoint_manager.add_breakpoint(breakpoint)
    }

    /// Remove a breakpoint
    pub fn remove_breakpoint(&self, breakpoint_id: u64) -> VmResult<()> {
        self.breakpoint_manager.remove_breakpoint(breakpoint_id)?;
        Ok(())
    }

    /// Enable or disable a breakpoint
    pub fn set_breakpoint_enabled(&self, breakpoint_id: u64, enabled: bool) -> VmResult<()> {
        self.breakpoint_manager.set_breakpoint_enabled(breakpoint_id, enabled)?;
        Ok(())
    }

    /// Single step execution
    pub fn step(&self, thread_id: Option<u32>) -> VmResult<()> {
        let current_thread = thread_id.or(*self.lock_current_thread_read()?);

        if let Some(tid) = current_thread {
            // Check if we're in step-over mode
            {
                let step_over_info = self.lock_step_over_info_read()?;
                if let Some(ref info) = *step_over_info {
                    if info.thread_id == tid {
                        return self.step_over_continue(tid);
                    }
                }
            }

            // Set stepping state
            self.multi_thread_debugger.update_thread_state(
                tid,
                ThreadExecutionState::Stepping,
            )?;

            // Update debugger state
            let mut state = self.lock_state_write()?;
            *state = DebuggerState::Stepping;

            // Update performance stats
            self.update_performance_stats(|stats| {
                stats.total_steps += 1;
            });

            Ok(())
        } else {
            Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: "No current thread selected".to_string(),
                current: "None".to_string(),
                expected: "Thread ID".to_string(),
            }))
        }
    }

    /// Step over function call
    pub fn step_over(&self, thread_id: Option<u32>) -> VmResult<()> {
        let current_thread = thread_id.or(*self.lock_current_thread_read()?);

        if let Some(tid) = current_thread {
            // Get current call stack
            let call_stack = self.call_stack_tracker.get_call_stack();

            if call_stack.len() < 2 {
                // Not in a function call, just do regular step
                return self.step(Some(tid));
            }

            // Get current frame and caller frame
            let current_frame = &call_stack[0];
            let caller_frame = &call_stack[1];

            // Set breakpoint at function return address
            let return_breakpoint_id = self.breakpoint_manager.add_breakpoint(
                Breakpoint::new_execution(
                    0,
                    caller_frame.return_address,
                    vec![0x90, 0x90], // NOP instructions
                )
            )?;

            // Set step over info
            {
                let mut step_over_info = self.lock_step_over_info_write()?;
                *step_over_info = Some(StepOverInfo {
                    thread_id: tid,
                    entry_breakpoint_id: 0, // Would be current BP if exists
                    return_breakpoint_id,
                    return_address: caller_frame.return_address,
                    stack_depth: call_stack.len(),
                });
            }

            // Continue execution
            self.resume()
        } else {
            Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: "No current thread selected".to_string(),
                current: "None".to_string(),
                expected: "Thread ID".to_string(),
            }))
        }
    }

    /// Continue execution
    pub fn continue_execution(&self, thread_id: Option<u32>) -> VmResult<()> {
        let current_thread = thread_id.or(*self.lock_current_thread_read()?);

        if let Some(tid) = current_thread {
            // Clear step over info
            {
                let mut step_over_info = self.lock_step_over_info_write()?;
                if let Some(ref info) = *step_over_info {
                    // Remove temporary return breakpoint
                    self.breakpoint_manager.remove_breakpoint(info.return_breakpoint_id)?;
                }
                *step_over_info = None;
            }

            // Set running state
            self.multi_thread_debugger.update_thread_state(
                tid,
                ThreadExecutionState::Running,
            )?;

            // Update debugger state
            let mut state = self.lock_state_write()?;
            *state = DebuggerState::Running;

            Ok(())
        } else {
            Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: "No current thread selected".to_string(),
                current: "None".to_string(),
                expected: "Thread ID".to_string(),
            }))
        }
    }

    /// Read memory
    pub fn read_memory(&self, address: GuestAddr, size: usize) -> VmResult<Vec<u8>> {
        // This would interface with the VM's memory system
        // For now, return dummy data
        Ok(vec![0; size])
    }

    /// Write memory
    pub fn write_memory(&self, address: GuestAddr, data: &[u8]) -> VmResult<()> {
        // This would interface with the VM's memory system
        // For now, just record the access
        if self.config.enable_memory_tracking {
            self.record_memory_access(address, MemoryAccessType::Write, data.len(), Some(data.to_vec()), true);
        }
        Ok(())
    }

    /// Read register
    pub fn read_register(&self, thread_id: Option<u32>, register: &str) -> VmResult<u64> {
        let current_thread = thread_id.or(*self.lock_current_thread_read()?);

        if let Some(tid) = current_thread {
            let thread_state = self.multi_thread_debugger.get_thread_state(tid)?;
            if let Some(state) = thread_state {
                Ok(state.registers.get(register).copied().unwrap_or(0))
            } else {
                Err(VmError::Core(crate::error::CoreError::InvalidState {
                    message: format!("Thread {} not found", tid),
                    current: "N/A".to_string(),
                    expected: format!("Thread {} to exist", tid),
                }))
            }
        } else {
            Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: "No current thread selected".to_string(),
                current: "None".to_string(),
                expected: "Thread ID".to_string(),
            }))
        }
    }

    /// Write register
    pub fn write_register(&self, thread_id: Option<u32>, register: &str, value: u64) -> VmResult<()> {
        let current_thread = thread_id.or(*self.lock_current_thread_read()?);

        if let Some(tid) = current_thread {
            let mut registers = HashMap::new();
            {
                let thread_state = self.multi_thread_debugger.get_thread_state(tid)?;
                if let Some(state) = thread_state {
                    registers = state.registers.clone();
                } else {
                    return Err(VmError::Core(crate::error::CoreError::InvalidState {
                        message: format!("Thread {} not found", tid),
                        current: "N/A".to_string(),
                        expected: format!("Thread {} to exist", tid),
                    }));
                }
            }

            registers.insert(register.to_string(), value);
            self.multi_thread_debugger.update_thread_registers(tid, registers)?;
            Ok(())
        } else {
            Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: "No current thread selected".to_string(),
                current: "None".to_string(),
                expected: "Thread ID".to_string(),
            }))
        }
    }

    /// Get current call stack
    pub fn get_call_stack(&self, thread_id: Option<u32>) -> VmResult<Vec<StackFrame>> {
        // For now, return global call stack
        // In a real implementation, this would be thread-specific
        Ok(self.call_stack_tracker.get_call_stack())
    }

    /// Get local variable value
    pub fn get_local_variable(
        &self,
        frame_id: u64,
        variable_name: &str,
    ) -> VmResult<Option<VariableValue>> {
        self.call_stack_tracker.get_local_variable(frame_id, variable_name)
    }

    /// Set local variable value
    pub fn set_local_variable(
        &self,
        frame_id: u64,
        variable_name: &str,
        value: VariableValue,
    ) -> VmResult<()> {
        self.call_stack_tracker.update_local_variable(frame_id, variable_name, value)
    }

    /// Get source location for address
    pub fn get_source_location(&self, address: GuestAddr) -> VmResult<Option<SourceLocation>> {
        if self.config.enable_source_level_debugging {
            self.symbol_table.get_source_location(address)
        } else {
            Ok(None)
        }
    }

    /// Resolve symbol
    pub fn resolve_symbol(&self, name: &str) -> VmResult<Option<Symbol>> {
        self.symbol_table.find_symbol(name)
    }

    /// Resolve address to symbol
    pub fn resolve_address(&self, address: GuestAddr) -> VmResult<Option<Symbol>> {
        self.symbol_table.find_symbol_by_address(address)
    }

    /// Load symbols from file
    pub fn load_symbols(&self, file_path: &Path) -> VmResult<()> {
        self.symbol_table.load_from_file(file_path)
    }

    /// Get debugger events
    pub fn get_events(&self) -> Vec<DebuggerEvent> {
        match self.lock_events_read() {
            Ok(events) => events.clone(),
            Err(_) => Vec::new(),
        }
    }

    /// Get instruction trace
    pub fn get_instruction_trace(&self, thread_id: Option<u32>) -> Vec<InstructionTrace> {
        match self.lock_instruction_trace_read() {
            Ok(trace) => {
                if let Some(tid) = thread_id {
                    trace.iter()
                        .filter(|entry| entry.thread_id == tid)
                        .cloned()
                        .collect()
                } else {
                    trace.iter().cloned().collect()
                }
            }
            Err(_) => Vec::new(),
        }
    }

    /// Clear instruction trace
    pub fn clear_instruction_trace(&self) {
        if let Ok(mut trace) = self.lock_instruction_trace_write() {
            trace.clear();
        }
    }

    /// Get debugger statistics
    pub fn get_statistics(&self) -> DebuggerStatistics {
        let state = match self.lock_state_read() {
            Ok(guard) => *guard,
            Err(_) => DebuggerState::Error,
        };
        let breakpoint_stats = self.breakpoint_manager.get_statistics();
        let call_stack_stats = self.call_stack_tracker.get_statistics();
        let symbol_stats = self.symbol_table.get_statistics();
        let multi_thread_stats = self.multi_thread_debugger.get_statistics();
        let performance_stats = match self.lock_performance_stats_read() {
            Ok(stats) => stats.clone(),
            Err(_) => DebuggerPerformanceStats {
                total_instructions: 0,
                total_breakpoints_hit: 0,
                total_steps: 0,
                total_exceptions: 0,
                avg_instruction_time_ns: 0.0,
                total_debug_time_ns: 0,
                memory_access_stats: MemoryAccessStats {
                    total_reads: 0,
                    total_writes: 0,
                    total_bytes_read: 0,
                    total_bytes_written: 0,
                    cache_hits: 0,
                    cache_misses: 0,
                },
                last_update: std::time::SystemTime::now(),
            },
        };

        DebuggerStatistics {
            state,
            breakpoint_stats,
            call_stack_stats,
            symbol_stats,
            multi_thread_stats,
            performance_stats,
        }
    }

    /// Continue step over execution
    fn step_over_continue(&self, thread_id: u32) -> VmResult<()> {
        // Set running state
        self.multi_thread_debugger.update_thread_state(
            thread_id,
            ThreadExecutionState::Running,
        )?;

        // Update debugger state
        let mut state = self.lock_state_write()?;
        *state = DebuggerState::Running;

        Ok(())
    }

    /// Record memory access
    fn record_memory_access(
        &self,
        address: GuestAddr,
        access_type: MemoryAccessType,
        size: usize,
        value: Option<Vec<u8>>,
        success: bool,
    ) {
        let access = MemoryAccess {
            address,
            access_type,
            size,
            value,
            success,
        };

        // Add to instruction trace
        if self.config.enable_instruction_tracing {
            if let Ok(mut trace) = self.lock_instruction_trace_write() {
                if let Ok(current_thread_guard) = self.lock_current_thread_read() {
                    if let Some(current_thread) = *current_thread_guard {
                        trace.push_back(InstructionTrace {
                            thread_id: current_thread,
                            address: 0, // Would be current IP
                            instruction: vec![], // Would be current instruction
                            disassembly: String::new(),
                            registers_before: HashMap::new(),
                            registers_after: HashMap::new(),
                            memory_accesses: vec![access],
                            execution_time_ns: 0,
                            timestamp: std::time::SystemTime::now(),
                        });

                        // Limit trace buffer size
                        if trace.len() > self.config.max_trace_buffer_size {
                            trace.pop_front();
                        }
                    }
                }
            }
        }

        // Update performance stats
        self.update_performance_stats(|stats| {
            match access_type {
                MemoryAccessType::Read => {
                    stats.memory_access_stats.total_reads += 1;
                    stats.memory_access_stats.total_bytes_read += size as u64;
                }
                MemoryAccessType::Write => {
                    stats.memory_access_stats.total_writes += 1;
                    stats.memory_access_stats.total_bytes_written += size as u64;
                }
                _ => {}
            }
        });
    }

    /// Update performance statistics
    fn update_performance_stats<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut DebuggerPerformanceStats),
    {
        if let Ok(mut stats) = self.lock_performance_stats_write() {
            update_fn(&mut stats);
            stats.last_update = std::time::SystemTime::now();
        }
    }
}

/// Debugger statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebuggerStatistics {
    /// Current debugger state
    pub state: DebuggerState,
    /// Breakpoint statistics
    pub breakpoint_stats: crate::debugger::enhanced_breakpoints::BreakpointStats,
    /// Call stack statistics
    pub call_stack_stats: crate::debugger::call_stack_tracker::CallStackStats,
    /// Symbol table statistics
    pub symbol_stats: crate::debugger::symbol_table::SymbolTableStatistics,
    /// Multi-threading statistics
    pub multi_thread_stats: crate::debugger::multi_thread_debug::MultiThreadDebugStatistics,
    /// Performance statistics
    pub performance_stats: DebuggerPerformanceStats,
}

impl Default for UnifiedDebugger {
    fn default() -> Self {
        Self::new(UnifiedDebuggerConfig::default()).unwrap_or_else(|e| {
            panic!("Failed to create UnifiedDebugger with default config: {:?}", e)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_debugger_creation() {
        let config = UnifiedDebuggerConfig::default();
        let debugger = UnifiedDebugger::new(config).expect("Failed to create UnifiedDebugger");

        assert_eq!(debugger.config.enable_source_level_debugging, true);
        assert_eq!(debugger.config.enable_performance_monitoring, true);
        assert_eq!(debugger.config.enable_memory_tracking, true);
        assert_eq!(debugger.config.enable_instruction_tracing, true);
    }

    #[test]
    fn test_debugger_state_transitions() {
        let config = UnifiedDebuggerConfig::default();
        let debugger = UnifiedDebugger::new(config).expect("Failed to create UnifiedDebugger");

        // Start debugger
        debugger.start().expect("Failed to start debugger");
        assert_eq!(*debugger.lock_state_read().expect("Failed to lock state"), DebuggerState::Running);

        // Pause debugger
        debugger.pause().expect("Failed to pause debugger");
        assert_eq!(*debugger.lock_state_read().expect("Failed to lock state"), DebuggerState::Paused);

        // Resume debugger
        debugger.resume().expect("Failed to resume debugger");
        assert_eq!(*debugger.lock_state_read().expect("Failed to lock state"), DebuggerState::Running);

        // Stop debugger
        debugger.stop("Test stop".to_string()).expect("Failed to stop debugger");
        assert_eq!(*debugger.lock_state_read().expect("Failed to lock state"), DebuggerState::Inactive);
    }

    #[test]
    fn test_breakpoint_operations() {
        let config = UnifiedDebuggerConfig::default();
        let debugger = UnifiedDebugger::new(config).expect("Failed to create UnifiedDebugger");

        // Set breakpoint
        let bp_id = debugger.set_breakpoint(
            0x1000,
            BreakpointType::Execution,
            None,
            None,
        ).expect("Failed to set breakpoint");

        assert!(bp_id > 0);

        // Check breakpoint exists
        let bp = debugger.breakpoint_manager.get_breakpoint(bp_id).expect("Failed to get breakpoint");
        assert_eq!(bp.address, 0x1000);
        assert_eq!(bp.breakpoint_type, BreakpointType::Execution);

        // Remove breakpoint
        debugger.remove_breakpoint(bp_id).expect("Failed to remove breakpoint");

        // Check breakpoint is gone
        let result = debugger.breakpoint_manager.get_breakpoint(bp_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_step_operations() {
        let config = UnifiedDebuggerConfig::default();
        let debugger = UnifiedDebugger::new(config).expect("Failed to create UnifiedDebugger");

        // Register a thread
        let native_id = std::thread::current().id();
        let thread_id = debugger.multi_thread_debugger.register_thread(
            native_id,
            Some("test_thread".to_string()),
            crate::debugger::multi_thread_debug::ThreadPriority::Normal,
        ).expect("Failed to register thread");

        // Set current thread
        *debugger.lock_current_thread_write().expect("Failed to lock current thread") = Some(thread_id);

        // Step execution
        debugger.step(Some(thread_id)).expect("Failed to step");
        assert_eq!(*debugger.lock_state_read().expect("Failed to lock state"), DebuggerState::Stepping);

        // Continue execution
        debugger.continue_execution(Some(thread_id)).expect("Failed to continue execution");
        assert_eq!(*debugger.lock_state_read().expect("Failed to lock state"), DebuggerState::Running);
    }
}
