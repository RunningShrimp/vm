//! Debugger module
//!
//! This module provides comprehensive debugging capabilities for the VM system,
//! including enhanced breakpoints, call stack tracking, symbol table support,
//! multi-threading debugging, and a unified debugging interface.

pub mod enhanced_breakpoints;
pub mod call_stack_tracker;
pub mod symbol_table;
pub mod multi_thread_debug;
pub mod unified_debugger;
pub mod enhanced_gdb_server;
pub mod integration;

// Re-export all debugging functionality
pub use enhanced_breakpoints::{
    BreakpointManager, Breakpoint, BreakpointType, BreakpointCondition,
    BreakpointGroup, BreakpointStatistics
};

pub use call_stack_tracker::{
    CallStackTracker, StackFrame, LocalVariable, VariableValue,
    VariableLocation, VariableScope, CallStackEvent, CallStackStatistics
};

pub use symbol_table::{
    SymbolTable, Symbol, SymbolType, SymbolScope, SymbolVisibility,
    FunctionInfo, ParameterInfo, LocalVariableInfo, SourceLocation,
    LineInfo, SymbolTableConfig, SymbolTableStatistics
};

pub use multi_thread_debug::{
    MultiThreadDebugger, ThreadState, ThreadExecutionState, ThreadPriority,
    ThreadStatus, ThreadEvent, ThreadBreakpoint, ThreadCondition,
    MultiThreadDebugConfig, ThreadPerformanceStats, MultiThreadDebugStatistics
};

pub use unified_debugger::{
    UnifiedDebugger, UnifiedDebuggerConfig, DebuggerState, DebuggerEvent,
    InstructionTrace, MemoryAccess, MemoryAccessType, DebuggerStatistics,
    DebuggerPerformanceStats, MemoryAccessStats
};

pub use enhanced_gdb_server::{
    EnhancedGdbServer, EnhancedGdbServerConfig,
    GdbResponse, GdbPacket, GdbErrorCode, GdbSignal, GdbStopReason,
    GdbRegisterInfo, GdbThreadInfo, GdbMemoryMap, GdbMemoryRegion, GdbBreakpointInfo,
    GdbWatchpointInfo, GdbServerStatistics, GdbConnectionStats
};

pub use integration::{
    DebuggerIntegration, DebuggerIntegrationConfig, VmDebuggerState
};