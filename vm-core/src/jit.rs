//! JIT-related types and events
//!
//! This module provides domain events and types used by JIT compilation
//! and execution engines.

pub mod domain_events;

// Re-export commonly used types for convenience
pub use domain_events::{
    DomainEvent, ExecutionEvent, GcEvent, JitCompilationEvent, MemoryEvent, VmLifecycleEvent,
};
