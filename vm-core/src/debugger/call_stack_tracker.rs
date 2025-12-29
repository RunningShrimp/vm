//! Call stack tracking system
//!
//! This module provides comprehensive call stack tracking including
//! function entry/exit, stack frame reconstruction, and local variable inspection.

#![cfg(feature = "debug")]

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};
use crate::{GuestAddr, VmError, VmResult};

/// Stack frame information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackFrame {
    /// Frame ID
    pub id: u64,
    /// Function name (if available from symbols)
    pub function_name: Option<String>,
    /// Function start address
    pub function_address: GuestAddr,
    /// Return address
    pub return_address: GuestAddr,
    /// Current instruction pointer within function
    pub instruction_pointer: GuestAddr,
    /// Stack pointer at frame entry
    pub stack_pointer: GuestAddr,
    /// Frame pointer (if available)
    pub frame_pointer: Option<GuestAddr>,
    /// Frame size in bytes
    pub frame_size: usize,
    /// Local variables (if available from debug info)
    pub local_variables: HashMap<String, LocalVariable>,
    /// Parameters (if available from debug info)
    pub parameters: Vec<LocalVariable>,
    /// Frame creation timestamp
    pub created_at: std::time::SystemTime,
    /// Architecture-specific data
    pub arch_specific: HashMap<String, String>,
}

/// Local variable information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalVariable {
    /// Variable name
    pub name: String,
    /// Variable type
    pub var_type: String,
    /// Variable location (register, stack offset, etc.)
    pub location: VariableLocation,
    /// Current value (if available)
    pub value: Option<VariableValue>,
    /// Variable scope (local, parameter, global)
    pub scope: VariableScope,
}

/// Variable location types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariableLocation {
    /// Stored in a register
    Register { register: String, offset: Option<i32> },
    /// Stored on stack at offset from frame pointer
    StackOffset { offset: i32 },
    /// Stored at absolute memory address
    Memory { address: GuestAddr },
    /// Stored in multiple locations (e.g., split across registers)
    Multiple { locations: Vec<VariableLocation> },
}

/// Variable value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariableValue {
    /// 8-bit signed integer
    I8(i8),
    /// 16-bit signed integer
    I16(i16),
    /// 32-bit signed integer
    I32(i32),
    /// 64-bit signed integer
    I64(i64),
    /// 8-bit unsigned integer
    U8(u8),
    /// 16-bit unsigned integer
    U16(u16),
    /// 32-bit unsigned integer
    U32(u32),
    /// 64-bit unsigned integer
    U64(u64),
    /// 32-bit floating point
    F32(f32),
    /// 64-bit floating point
    F64(f64),
    /// Pointer value
    Pointer(GuestAddr),
    /// Array of values
    Array(Vec<VariableValue>),
    /// Struct/aggregate type
    Struct(HashMap<String, VariableValue>),
    /// Unknown or unsupported type
    Unknown(Vec<u8>),
}

/// Variable scope
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VariableScope {
    /// Local variable
    Local,
    /// Function parameter
    Parameter,
    /// Global variable
    Global,
    /// Static variable
    Static,
}

/// Call stack event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CallStackEvent {
    /// Function call
    FunctionCall {
        frame_id: u64,
        function_address: GuestAddr,
        return_address: GuestAddr,
        parameters: Vec<VariableValue>,
        timestamp: std::time::SystemTime,
    },
    /// Function return
    FunctionReturn {
        frame_id: u64,
        return_value: Option<VariableValue>,
        timestamp: std::time::SystemTime,
    },
    /// Exception during function call
    Exception {
        frame_id: u64,
        exception_type: String,
        exception_address: GuestAddr,
        timestamp: std::time::SystemTime,
    },
    /// Stack overflow/underflow
    StackError {
        error_type: StackErrorType,
        stack_pointer: GuestAddr,
        frame_pointer: Option<GuestAddr>,
        timestamp: std::time::SystemTime,
    },
}

/// Stack error types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StackErrorType {
    /// Stack overflow
    Overflow,
    /// Stack underflow
    Underflow,
    /// Corrupted frame pointer
    CorruptedFramePointer,
    /// Invalid stack alignment
    InvalidAlignment,
}

/// Call stack configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallStackConfig {
    /// Maximum stack depth to track
    pub max_depth: usize,
    /// Enable automatic frame detection
    pub auto_frame_detection: bool,
    /// Enable variable tracking
    pub enable_variable_tracking: bool,
    /// Stack alignment requirement
    pub stack_alignment: usize,
    /// Enable stack overflow detection
    pub overflow_detection: bool,
    /// Maximum stack size in bytes
    pub max_stack_size: usize,
}

/// Statistics for call stack
#[derive(Debug, Clone)]
pub struct CallStackStats {
    pub max_depth: usize,
    pub current_depth: usize,
    pub total_calls: u64,
}

impl Default for CallStackStats {
    fn default() -> Self {
        Self {
            max_depth: 0,
            current_depth: 0,
            total_calls: 0,
        }
    }
}

impl Default for CallStackConfig {
    fn default() -> Self {
        Self {
            max_depth: 1000,
            auto_frame_detection: true,
            enable_variable_tracking: true,
            stack_alignment: 16, // Common for x86-64
            overflow_detection: true,
            max_stack_size: 1024 * 1024, // 1MB
        }
    }
}

/// Enhanced call stack tracker
pub struct CallStackTracker {
    /// Configuration
    config: CallStackConfig,
    /// Current call stack
    stack: Arc<RwLock<VecDeque<StackFrame>>>,
    /// Stack frames by ID
    frames_by_id: Arc<RwLock<HashMap<u64, StackFrame>>>,
    /// Next frame ID
    next_frame_id: Arc<RwLock<u64>>,
    /// Call stack events
    events: Arc<RwLock<Vec<CallStackEvent>>>,
    /// Current stack pointer
    stack_pointer: Arc<RwLock<GuestAddr>>,
    /// Current frame pointer
    frame_pointer: Arc<RwLock<Option<GuestAddr>>>,
    /// Stack base address
    stack_base: GuestAddr,
    /// Stack limits
    stack_limits: Arc<RwLock<StackLimits>>,
}

/// Stack limits for overflow detection
#[derive(Debug, Clone, Copy)]
struct StackLimits {
    /// Minimum stack pointer value
    min_sp: GuestAddr,
    /// Maximum stack pointer value
    max_sp: GuestAddr,
}

impl CallStackTracker {
    /// Helper method to lock stack for reading
    fn lock_stack(&self) -> Result<std::sync::RwLockReadGuard<'_, VecDeque<StackFrame>>, VmError> {
        self.stack.read().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire stack read lock: {}", e),
            operation: "lock_stack_read".to_string(),
        })
    }

    /// Helper method to lock stack for writing
    fn lock_stack_mut(&self) -> Result<std::sync::RwLockWriteGuard<'_, VecDeque<StackFrame>>, VmError> {
        self.stack.write().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire stack write lock: {}", e),
            operation: "lock_stack_write".to_string(),
        })
    }

    /// Helper method to lock frames_by_id for reading
    fn lock_frames(&self) -> Result<std::sync::RwLockReadGuard<'_, HashMap<u64, StackFrame>>, VmError> {
        self.frames_by_id.read().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire frames read lock: {}", e),
            operation: "lock_frames_read".to_string(),
        })
    }

    /// Helper method to lock frames_by_id for writing
    fn lock_frames_mut(&self) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<u64, StackFrame>>, VmError> {
        self.frames_by_id.write().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire frames write lock: {}", e),
            operation: "lock_frames_write".to_string(),
        })
    }

    /// Helper method to lock events for reading
    fn lock_events(&self) -> Result<std::sync::RwLockReadGuard<'_, Vec<CallStackEvent>>, VmError> {
        self.events.read().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire events read lock: {}", e),
            operation: "lock_events_read".to_string(),
        })
    }

    /// Helper method to lock events for writing
    fn lock_events_mut(&self) -> Result<std::sync::RwLockWriteGuard<'_, Vec<CallStackEvent>>, VmError> {
        self.events.write().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire events write lock: {}", e),
            operation: "lock_events_write".to_string(),
        })
    }

    /// Helper method to lock stack_pointer for reading
    fn lock_stack_pointer(&self) -> Result<std::sync::RwLockReadGuard<'_, GuestAddr>, VmError> {
        self.stack_pointer.read().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire stack_pointer read lock: {}", e),
            operation: "lock_stack_pointer_read".to_string(),
        })
    }

    /// Helper method to lock stack_pointer for writing
    fn lock_stack_pointer_mut(&self) -> Result<std::sync::RwLockWriteGuard<'_, GuestAddr>, VmError> {
        self.stack_pointer.write().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire stack_pointer write lock: {}", e),
            operation: "lock_stack_pointer_write".to_string(),
        })
    }

    /// Helper method to lock frame_pointer for reading
    fn lock_frame_pointer(&self) -> Result<std::sync::RwLockReadGuard<'_, Option<GuestAddr>>, VmError> {
        self.frame_pointer.read().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire frame_pointer read lock: {}", e),
            operation: "lock_frame_pointer_read".to_string(),
        })
    }

    /// Helper method to lock frame_pointer for writing
    fn lock_frame_pointer_mut(&self) -> Result<std::sync::RwLockWriteGuard<'_, Option<GuestAddr>>, VmError> {
        self.frame_pointer.write().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire frame_pointer write lock: {}", e),
            operation: "lock_frame_pointer_write".to_string(),
        })
    }

    /// Helper method to lock next_frame_id for writing
    fn lock_next_frame_id_mut(&self) -> Result<std::sync::RwLockWriteGuard<'_, u64>, VmError> {
        self.next_frame_id.write().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire next_frame_id write lock: {}", e),
            operation: "lock_next_frame_id_write".to_string(),
        })
    }

    /// Helper method to lock stack_limits for reading
    fn lock_stack_limits(&self) -> Result<std::sync::RwLockReadGuard<'_, StackLimits>, VmError> {
        self.stack_limits.read().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire stack_limits read lock: {}", e),
            operation: "lock_stack_limits_read".to_string(),
        })
    }

    /// Helper method to lock stack_limits for writing
    fn lock_stack_limits_mut(&self) -> Result<std::sync::RwLockWriteGuard<'_, StackLimits>, VmError> {
        self.stack_limits.write().map_err(|e| VmError::Core(crate::error::CoreError::Concurrency {
            message: format!("Failed to acquire stack_limits write lock: {}", e),
            operation: "lock_stack_limits_write".to_string(),
        })
    }

    /// Create a new call stack tracker
    pub fn new(config: CallStackConfig, stack_base: GuestAddr) -> Self {
        Self {
            config,
            stack: Arc::new(RwLock::new(VecDeque::new())),
            frames_by_id: Arc::new(RwLock::new(HashMap::new())),
            next_frame_id: Arc::new(RwLock::new(1)),
            events: Arc::new(RwLock::new(Vec::new())),
            stack_pointer: Arc::new(RwLock::new(stack_base)),
            frame_pointer: Arc::new(RwLock::new(None)),
            stack_base,
            stack_limits: Arc::new(RwLock::new(StackLimits {
                min_sp: stack_base,
                max_sp: stack_base,
            })),
        }
    }

    /// Record a function call
    pub fn record_function_call(
        &self,
        function_address: GuestAddr,
        return_address: GuestAddr,
        parameters: Vec<VariableValue>,
        registers: &HashMap<String, u64>,
    ) -> VmResult<u64> {
        // Check stack depth
        {
            let stack = self.lock_stack()?;
            if stack.len() >= self.config.max_depth {
                return Err(VmError::Core(crate::error::CoreError::InvalidState {
                    message: format!("Maximum stack depth {} exceeded", self.config.max_depth),
                    current: format!("{}", stack.len()),
                    expected: format!("<= {}", self.config.max_depth),
                }));
            }
        }

        // Get current stack and frame pointers
        let current_sp = *self.lock_stack_pointer()?;
        let current_fp = *self.lock_frame_pointer()?;

        // Generate frame ID
        let mut next_id = self.lock_next_frame_id_mut()?;
        let frame_id = *next_id;
        *next_id += 1;

        // Create new stack frame
        let frame = StackFrame {
            id: frame_id,
            function_name: None, // Will be filled by symbol resolution
            function_address,
            return_address,
            instruction_pointer: function_address,
            stack_pointer: current_sp,
            frame_pointer: current_fp,
            frame_size: 0, // Will be calculated as we track the frame
            local_variables: HashMap::new(),
            parameters: self.create_parameter_variables(parameters),
            created_at: std::time::SystemTime::now(),
            arch_specific: HashMap::new(),
        };

        // Add to stack
        {
            let mut stack = self.lock_stack_mut()?;
            stack.push_front(frame.clone());
        }

        // Add to frames by ID
        {
            let mut frames_by_id = self.lock_frames_mut()?;
            frames_by_id.insert(frame_id, frame);
        }

        // Record event
        {
            let mut events = self.lock_events_mut()?;
            events.push(CallStackEvent::FunctionCall {
                frame_id,
                function_address,
                return_address,
                parameters,
                timestamp: std::time::SystemTime::now(),
            });
        }

        // Update frame pointer (architecture-specific)
        self.update_frame_pointer(registers);

        Ok(frame_id)
    }

    /// Record a function return
    pub fn record_function_return(
        &self,
        frame_id: u64,
        return_value: Option<VariableValue>,
    ) -> VmResult<StackFrame> {
        // Remove frame from stack
        let mut stack = self.lock_stack_mut()?;
        let frame = if let Some((index, frame)) = stack.iter()
            .enumerate()
            .find(|(_, f)| f.id == frame_id) {
            let frame = frame.clone();
            stack.remove(index);
            frame
        } else {
            return Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: format!("Frame {} not found in call stack", frame_id),
                current: "N/A".to_string(),
                expected: format!("Frame {} to exist", frame_id),
            }));
        };

        // Update stack pointer to previous frame
        if let Some(prev_frame) = stack.front() {
            *self.lock_stack_pointer_mut()? = prev_frame.stack_pointer;
            *self.lock_frame_pointer_mut()? = prev_frame.frame_pointer;
        } else {
            // No more frames, reset to base
            *self.lock_stack_pointer_mut()? = self.stack_base;
            *self.lock_frame_pointer_mut()? = None;
        }

        // Remove from frames by ID
        {
            let mut frames_by_id = self.lock_frames_mut()?;
            frames_by_id.remove(&frame_id);
        }

        // Record event
        {
            let mut events = self.lock_events_mut()?;
            events.push(CallStackEvent::FunctionReturn {
                frame_id,
                return_value,
                timestamp: std::time::SystemTime::now(),
            });
        }

        Ok(frame)
    }

    /// Record an exception
    pub fn record_exception(
        &self,
        frame_id: u64,
        exception_type: String,
        exception_address: GuestAddr,
    ) -> VmResult<()> {
        let mut events = self.lock_events_mut()?;
        events.push(CallStackEvent::Exception {
            frame_id,
            exception_type,
            exception_address,
            timestamp: std::time::SystemTime::now(),
        });

        Ok(())
    }

    /// Get current call stack
    pub fn get_call_stack(&self) -> Vec<StackFrame> {
        match self.lock_stack() {
            Ok(stack) => stack.iter().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Get current stack depth
    pub fn get_stack_depth(&self) -> usize {
        match self.lock_stack() {
            Ok(stack) => stack.len(),
            Err(_) => 0,
        }
    }

    /// Get frame by ID
    pub fn get_frame(&self, frame_id: u64) -> Option<StackFrame> {
        match self.lock_frames() {
            Ok(frames_by_id) => frames_by_id.get(&frame_id).cloned(),
            Err(_) => None,
        }
    }

    /// Get top frame (current execution context)
    pub fn get_top_frame(&self) -> Option<StackFrame> {
        match self.lock_stack() {
            Ok(stack) => stack.front().cloned(),
            Err(_) => None,
        }
    }

    /// Update instruction pointer for current frame
    pub fn update_instruction_pointer(&self, ip: GuestAddr) -> VmResult<()> {
        let mut stack = self.lock_stack_mut()?;
        if let Some(frame) = stack.front_mut() {
            frame.instruction_pointer = ip;
            Ok(())
        } else {
            Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: "No frames in call stack".to_string(),
                current: "Empty".to_string(),
                expected: "At least one frame".to_string(),
            }))
        }
    }

    /// Update stack pointer
    pub fn update_stack_pointer(&self, sp: GuestAddr) -> VmResult<()> {
        // Check for stack overflow/underflow
        if self.config.overflow_detection {
            let mut limits = self.lock_stack_limits_mut()?;

            if sp < limits.min_sp {
                limits.min_sp = sp;
            }
            if sp > limits.max_sp {
                limits.max_sp = sp;
            }

            // Check if we've exceeded maximum stack size
            let stack_usage = if sp >= self.stack_base {
                sp - self.stack_base
            } else {
                self.stack_base - sp
            };

            if stack_usage > self.config.max_stack_size as u64 {
                return Err(VmError::Core(crate::error::CoreError::InvalidState {
                    message: format!("Stack overflow: usage {} exceeds maximum {}",
                        stack_usage, self.config.max_stack_size),
                    current: format!("{}", stack_usage),
                    expected: format!("<= {}", self.config.max_stack_size),
                }));
            }
        }

        *self.lock_stack_pointer_mut()? = sp;
        Ok(())
    }

    /// Add local variable to current frame
    pub fn add_local_variable(
        &self,
        name: String,
        var_type: String,
        location: VariableLocation,
        scope: VariableScope,
    ) -> VmResult<()> {
        let mut stack = self.lock_stack_mut()?;
        if let Some(frame) = stack.front_mut() {
            frame.local_variables.insert(name.clone(), LocalVariable {
                name,
                var_type,
                location,
                value: None,
                scope,
            });
            Ok(())
        } else {
            Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: "No frames in call stack".to_string(),
                current: "Empty".to_string(),
                expected: "At least one frame".to_string(),
            }))
        }
    }

    /// Update local variable value
    pub fn update_local_variable(
        &self,
        frame_id: u64,
        name: &str,
        value: VariableValue,
    ) -> VmResult<()> {
        let mut frames_by_id = self.lock_frames_mut()?;
        if let Some(frame) = frames_by_id.get_mut(&frame_id) {
            if let Some(variable) = frame.local_variables.get_mut(name) {
                variable.value = Some(value);
                Ok(())
            } else {
                Err(VmError::Core(crate::error::CoreError::InvalidState {
                    message: format!("Variable '{}' not found in frame {}", name, frame_id),
                    current: "N/A".to_string(),
                    expected: format!("Variable '{}' to exist", name),
                }))
            }
        } else {
            Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: format!("Frame {} not found", frame_id),
                current: "N/A".to_string(),
                expected: format!("Frame {} to exist", frame_id),
            }))
        }
    }

    /// Get local variable value
    pub fn get_local_variable(
        &self,
        frame_id: u64,
        name: &str,
    ) -> VmResult<Option<VariableValue>> {
        let frames_by_id = self.lock_frames()?;
        if let Some(frame) = frames_by_id.get(&frame_id) {
            Ok(frame.local_variables.get(name).and_then(|v| v.value.clone()))
        } else {
            Err(VmError::Core(crate::error::CoreError::InvalidState {
                message: format!("Frame {} not found", frame_id),
                current: "N/A".to_string(),
                expected: format!("Frame {} to exist", frame_id),
            }))
        }
    }

    /// Get call stack events
    pub fn get_events(&self) -> Vec<CallStackEvent> {
        match self.lock_events() {
            Ok(events) => events.clone(),
            Err(_) => Vec::new(),
        }
    }

    /// Clear call stack events
    pub fn clear_events(&self) {
        if let Ok(mut events) = self.lock_events_mut() {
            events.clear();
        }
    }

    /// Get call stack statistics
    pub fn get_statistics(&self) -> CallStackStatistics {
        let (stack, events, limits, stack_pointer) = match (
            self.lock_stack(),
            self.lock_events(),
            self.lock_stack_limits(),
            self.lock_stack_pointer(),
        ) {
            (Ok(stack), Ok(events), Ok(limits), Ok(stack_pointer)) => {
                (Some(stack), Some(events), Some(limits), Some(*stack_pointer))
            }
            _ => (None, None, None, None),
        };

        let mut function_calls = 0;
        let mut function_returns = 0;
        let mut exceptions = 0;
        let mut stack_errors = 0;

        if let Some(ref events) = events {
            for event in events.iter() {
                match event {
                    CallStackEvent::FunctionCall { .. } => function_calls += 1,
                    CallStackEvent::FunctionReturn { .. } => function_returns += 1,
                    CallStackEvent::Exception { .. } => exceptions += 1,
                    CallStackEvent::StackError { .. } => stack_errors += 1,
                }
            }
        }

        let current_stack_usage = match stack_pointer {
            Some(sp) if sp >= self.stack_base => sp - self.stack_base,
            Some(sp) => self.stack_base - sp,
            None => 0,
        };

        CallStackStatistics {
            current_depth: stack.as_ref().map_or(0, |s| s.len()),
            max_depth_reached: match (limits, stack_pointer) {
                (Some(limits), Some(_sp)) => limits.max_sp - limits.min_sp,
                _ => 0,
            },
            current_stack_usage,
            max_stack_usage: self.config.max_stack_size,
            function_calls,
            function_returns,
            exceptions,
            stack_errors,
        }
    }

    /// Create parameter variables from values
    fn create_parameter_variables(&self, parameters: Vec<VariableValue>) -> Vec<LocalVariable> {
        parameters
            .into_iter()
            .enumerate()
            .map(|(i, value)| LocalVariable {
                name: format!("param_{}", i),
                var_type: "auto".to_string(), // Would be determined from debug info
                location: VariableLocation::Register { 
                    register: format!("arg_{}", i), 
                    offset: None 
                },
                value: Some(value),
                scope: VariableScope::Parameter,
            })
            .collect()
    }

    /// Update frame pointer based on architecture and registers
    fn update_frame_pointer(&self, registers: &HashMap<String, u64>) {
        // This is architecture-specific
        // For x86-64, typically RBP or RBP is used
        // For ARM64, x29 (FP) is used
        // For RISC-V, s0/fp is used

        let fp = if let Some(&rbp) = registers.get("rbp") {
            Some(rbp as GuestAddr)
        } else if let Some(&fp) = registers.get("fp") {
            Some(fp as GuestAddr)
        } else if let Some(&s0) = registers.get("s0") {
            Some(s0 as GuestAddr)
        } else {
            None
        };

        if let Ok(mut frame_pointer) = self.lock_frame_pointer_mut() {
            *frame_pointer = fp;
        }
    }
}

/// Call stack statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallStackStatistics {
    /// Current stack depth
    pub current_depth: usize,
    /// Maximum depth reached
    pub max_depth_reached: u64,
    /// Current stack usage in bytes
    pub current_stack_usage: u64,
    /// Maximum stack size in bytes
    pub max_stack_usage: usize,
    /// Total function calls recorded
    pub function_calls: u64,
    /// Total function returns recorded
    pub function_returns: u64,
    /// Total exceptions recorded
    pub exceptions: u64,
    /// Total stack errors recorded
    pub stack_errors: u64,
}

impl Default for crate::debugger::call_stack_tracker::CallStackTracker {
    fn default() -> Self {
        Self::new(crate::debugger::call_stack_tracker::CallStackConfig::default(), 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_frame_creation() {
        let frame = StackFrame {
            id: 1,
            function_name: Some("test_function".to_string()),
            function_address: 0x1000,
            return_address: 0x2000,
            instruction_pointer: 0x1000,
            stack_pointer: 0x7FFF0000,
            frame_pointer: Some(0x7FFF1000),
            frame_size: 64,
            local_variables: HashMap::new(),
            parameters: Vec::new(),
            created_at: std::time::SystemTime::now(),
            arch_specific: HashMap::new(),
        };

        assert_eq!(frame.id, 1);
        assert_eq!(frame.function_name, Some("test_function".to_string()));
        assert_eq!(frame.function_address, 0x1000);
        assert_eq!(frame.return_address, 0x2000);
    }

    #[test]
    fn test_call_stack_tracker() {
        let config = CallStackConfig::default();
        let tracker = CallStackTracker::new(config, 0x80000000);

        // Record function call
        let frame_id = tracker.record_function_call(
            0x1000,
            0x2000,
            vec![VariableValue::I32(42)],
            &HashMap::new(),
        ).expect("Failed to record function call");

        assert_eq!(frame_id, 1);
        assert_eq!(tracker.get_stack_depth(), 1);

        // Check top frame
        let top_frame = tracker.get_top_frame().expect("No top frame");
        assert_eq!(top_frame.id, 1);
        assert_eq!(top_frame.function_address, 0x1000);
        assert_eq!(top_frame.return_address, 0x2000);

        // Record function return
        let returned_frame = tracker.record_function_return(frame_id, Some(VariableValue::I32(100))).expect("Failed to record function return");
        assert_eq!(returned_frame.id, 1);
        assert_eq!(tracker.get_stack_depth(), 0);
    }

    #[test]
    fn test_local_variables() {
        let config = CallStackConfig::default();
        let tracker = CallStackTracker::new(config, 0x80000000);

        let frame_id = tracker.record_function_call(
            0x1000,
            0x2000,
            vec![VariableValue::I32(42)],
            &HashMap::new(),
        ).expect("Failed to record function call");

        // Add local variable
        tracker.add_local_variable(
            "local_var".to_string(),
            "int".to_string(),
            VariableLocation::StackOffset { offset: -8 },
            VariableScope::Local,
        ).expect("Failed to add local variable");

        // Update variable value
        tracker.update_local_variable(frame_id, "local_var", VariableValue::I32(123)).expect("Failed to update local variable");

        // Get variable value
        let value = tracker.get_local_variable(frame_id, "local_var").expect("Failed to get local variable");
        assert!(value.is_some());
        match value.unwrap() {
            VariableValue::I32(v) => assert_eq!(v, 123),
            _ => panic!("Expected I32 value"),
        }
    }

    #[test]
    fn test_stack_overflow_detection() {
        let config = CallStackConfig {
            max_stack_size: 1024,
            overflow_detection: true,
            ..Default::default()
        };
        let tracker = CallStackTracker::new(config, 0x80000000);

        // Try to exceed stack size
        let result = tracker.update_stack_pointer(0x80000000 + 2000);
        assert!(result.is_err());
    }
}
