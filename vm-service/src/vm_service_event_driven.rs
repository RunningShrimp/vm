//! 事件驱动的虚拟机服务
//!
//! 使用聚合根和事件总线实现事件驱动的虚拟机服务。


use crate::vm_service::VirtualMachineService;
use log::{debug, info};
use std::sync::{Arc, Mutex};
use vm_core::vm_state::VirtualMachineState;
use vm_core::{
    VmConfig,
    VmError,
    VmResult,
};

// NOTE: EventDrivenVmService implementation was removed due to type mismatches and incomplete implementation.
// The commented code occupied approximately 650 lines and contained:
// - EventDrivenVmService struct definition
// - Event sourcing and aggregate root integration
// - VM lifecycle management methods (start, pause, resume, stop)
// - Snapshot management methods
// - Event handler setup
//
// This code can be restored and fixed when needed for event-driven architecture features.
