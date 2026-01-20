//! Debugger integration with VirtualMachine
//!
//! This module provides integration between the VirtualMachine and the debugger
//! subsystem, allowing for seamless debugging of VM execution.

use std::sync::{Arc, Mutex};
use crate::{VmConfig, VmError, VmResult, GuestAddr, ExecResult, ExecStatus, VcpuState};
use crate::jit::debugger::{
    UnifiedDebugger, UnifiedDebuggerConfig, DebuggerState, DebuggerEvent,
    BreakpointType, BreakpointCondition, MemoryAccessType
};
use crate::jit::mmu_traits::MMU;
use crate::ExecutionEngine;

/// Debugger integration configuration
#[derive(Debug, Clone)]
pub struct DebuggerIntegrationConfig {
    /// Enable debugger integration
    pub enable_debugger: bool,
    /// Debugger configuration
    pub debugger_config: UnifiedDebuggerConfig,
    /// Auto-start debugger on VM creation
    pub auto_start: bool,
    /// Break on entry point
    pub break_on_entry: bool,
    /// Entry point address
    pub entry_point: Option<GuestAddr>,
}

impl Default for DebuggerIntegrationConfig {
    fn default() -> Self {
        Self {
            enable_debugger: false,
            debugger_config: UnifiedDebuggerConfig::default(),
            auto_start: false,
            break_on_entry: false,
            entry_point: None,
        }
    }
}

/// Debugger integration manager
/// 
/// This struct manages the integration between the VirtualMachine and the debugger,
/// handling events, state synchronization, and debugging operations.
pub struct DebuggerIntegration {
    /// Unified debugger instance
    debugger: Option<Arc<UnifiedDebugger>>,
    /// Integration configuration
    config: DebuggerIntegrationConfig,
    /// Current VM state for debugging
    vm_state: Arc<Mutex<VmDebuggerState>>,
}

/// VM debugger state
#[derive(Debug, Clone)]
pub struct VmDebuggerState {
    /// Current execution status
    pub execution_status: ExecStatus,
    /// Last execution result
    pub last_result: Option<ExecResult>,
    /// Current thread ID (for single-threaded VMs, this is 0)
    pub current_thread: u32,
    /// Whether the VM is paused for debugging
    pub is_paused: bool,
    /// Breakpoint hit flag
    pub breakpoint_hit: bool,
    /// Last breakpoint ID hit
    pub last_breakpoint_id: Option<u64>,
}

impl Default for VmDebuggerState {
    fn default() -> Self {
        Self {
            execution_status: ExecStatus::Continue,
            last_result: None,
            current_thread: 0,
            is_paused: false,
            breakpoint_hit: false,
            last_breakpoint_id: None,
        }
    }
}

impl DebuggerIntegration {
    /// Create a new debugger integration
    pub fn new(config: DebuggerIntegrationConfig) -> VmResult<Self> {
        let debugger = if config.enable_debugger {
            Some(Arc::new(UnifiedDebugger::new(config.debugger_config.clone())?))
        } else {
            None
        };

        Ok(Self {
            debugger,
            config,
            vm_state: Arc::new(Mutex::new(VmDebuggerState::default())),
        })
    }

    /// Initialize the debugger integration
    pub fn initialize(&mut self, vm_config: &VmConfig) -> VmResult<()> {
        if let Some(debugger) = &self.debugger {
            // Register the main VM thread
            debugger.register_thread(0, "main")?;
            
            // Set entry point breakpoint if configured
            if self.config.break_on_entry {
                if let Some(entry_point) = self.config.entry_point {
                    debugger.set_breakpoint(
                        entry_point,
                        BreakpointType::Execution,
                        None,
                        Some(0)
                    )?;
                }
            }
            
            // Auto-start debugger if configured
            if self.config.auto_start {
                debugger.start()?;
            }
        }
        
        Ok(())
    }

    /// Handle pre-execution hook
    /// 
    /// This should be called before executing each instruction or basic block.
    pub fn pre_execution_hook<B>(
        &self,
        engine: &dyn ExecutionEngine<B>,
        mmu: &mut dyn MMU,
    ) -> VmResult<bool> {
        if let Some(debugger) = &self.debugger {
            let pc = engine.get_pc();
            let mut vm_state = self.vm_state.lock().map_err(|_| {
                VmError::Core(crate::error::CoreError::Internal {
                    message: "Failed to lock VM state".to_string(),
                    module: "DebuggerIntegration".to_string(),
                })
            })?;

            // Check for breakpoints at current PC
            let breakpoints = debugger.breakpoint_manager.get_breakpoints_at(pc);
            if !breakpoints.is_empty() {
                vm_state.breakpoint_hit = true;
                vm_state.last_breakpoint_id = Some(breakpoints[0].id);
                vm_state.is_paused = true;
                
                // Notify debugger of breakpoint hit
                debugger.notify_breakpoint_hit(breakpoints[0].id, pc, Some(0))?;
                
                return Ok(false); // Don't execute
            }

            // Check if VM is paused
            if vm_state.is_paused {
                return Ok(false); // Don't execute
            }
        }
        
        Ok(true) // Continue execution
    }

    /// Handle post-execution hook
    /// 
    /// This should be called after executing each instruction or basic block.
    pub fn post_execution_hook<B>(
        &self,
        engine: &mut dyn ExecutionEngine<B>,
        result: &ExecResult,
    ) -> VmResult<()> {
        if let Some(debugger) = &self.debugger {
            let mut vm_state = self.vm_state.lock().map_err(|_| {
                VmError::Core(crate::error::CoreError::Internal {
                    message: "Failed to lock VM state".to_string(),
                    module: "DebuggerIntegration".to_string(),
                })
            })?;

            // Update VM state
            vm_state.execution_status = result.status.clone();
            vm_state.last_result = Some(result.clone());

            // Update thread state in debugger
            let vcpu_state = engine.get_vcpu_state();
            let mut registers = std::collections::HashMap::new();
            
            // Add general purpose registers
            for (i, &reg) in vcpu_state.regs.iter().enumerate() {
                registers.insert(format!("r{}", i), reg);
            }
            
            // Add PC
            registers.insert("pc".to_string(), vcpu_state.pc);
            
            debugger.update_thread_registers(0, &registers)?;

            // Handle execution status
            match &result.status {
                ExecStatus::Continue => {
                    // Normal continuation
                }
                ExecStatus::Ok => {
                    // Execution completed
                    debugger.stop("VM execution completed".to_string())?;
                }
                ExecStatus::Fault(error) => {
                    // Execution fault
                    debugger.notify_exception(
                        format!("Execution fault: {:?}", error),
                        engine.get_pc(),
                        Some(0)
                    )?;
                }
                ExecStatus::IoRequest => {
                    // I/O request
                }
                ExecStatus::InterruptPending => {
                    // Interrupt pending
                }
            }
        }
        
        Ok(())
    }

    /// Handle memory access
    /// 
    /// This should be called for all memory accesses to support watchpoints.
    pub fn handle_memory_access(
        &self,
        address: GuestAddr,
        size: u8,
        access_type: MemoryAccessType,
        value: Option<u64>,
    ) -> VmResult<()> {
        if let Some(debugger) = &self.debugger {
            // Check for watchpoints
            let watchpoints = debugger.breakpoint_manager.get_watchpoints_at(address, size, access_type);
            if !watchpoints.is_empty() {
                let mut vm_state = self.vm_state.lock().map_err(|_| {
                    VmError::Core(crate::error::CoreError::Internal {
                        message: "Failed to lock VM state".to_string(),
                        module: "DebuggerIntegration".to_string(),
                    })
                })?;

                vm_state.breakpoint_hit = true;
                vm_state.last_breakpoint_id = Some(watchpoints[0].id);
                vm_state.is_paused = true;

                // Notify debugger of watchpoint hit
                debugger.notify_watchpoint_hit(
                    watchpoints[0].id,
                    address,
                    size,
                    access_type,
                    value,
                    Some(0)
                )?;
            }
        }
        
        Ok(())
    }

    /// Continue execution
    pub fn continue_execution(&self) -> VmResult<()> {
        if let Some(debugger) = &self.debugger {
            let mut vm_state = self.vm_state.lock().map_err(|_| {
                VmError::Core(crate::error::CoreError::Internal {
                    message: "Failed to lock VM state".to_string(),
                    module: "DebuggerIntegration".to_string(),
                })
            })?;

            vm_state.is_paused = false;
            vm_state.breakpoint_hit = false;
            vm_state.last_breakpoint_id = None;

            debugger.continue_execution(Some(0))?;
        }
        
        Ok(())
    }

    /// Step execution
    pub fn step_execution(&self) -> VmResult<()> {
        if let Some(debugger) = &self.debugger {
            let mut vm_state = self.vm_state.lock().map_err(|_| {
                VmError::Core(crate::error::CoreError::Internal {
                    message: "Failed to lock VM state".to_string(),
                    module: "DebuggerIntegration".to_string(),
                })
            })?;

            vm_state.is_paused = false;
            vm_state.breakpoint_hit = false;
            vm_state.last_breakpoint_id = None;

            debugger.step(Some(0))?;
        }
        
        Ok(())
    }

    /// Get debugger instance
    pub fn debugger(&self) -> Option<Arc<UnifiedDebugger>> {
        self.debugger.clone()
    }

    /// Get VM state
    pub fn vm_state(&self) -> Arc<Mutex<VmDebuggerState>> {
        Arc::clone(&self.vm_state)
    }

    /// Check if VM is paused for debugging
    pub fn is_paused(&self) -> VmResult<bool> {
        let vm_state = self.vm_state.lock().map_err(|_| {
            VmError::Core(crate::error::CoreError::Internal {
                message: "Failed to lock VM state".to_string(),
                module: "DebuggerIntegration".to_string(),
            })
        })?;
        
        Ok(vm_state.is_paused)
    }

    /// Check if breakpoint was hit
    pub fn is_breakpoint_hit(&self) -> VmResult<bool> {
        let vm_state = self.vm_state.lock().map_err(|_| {
            VmError::Core(crate::error::CoreError::Internal {
                message: "Failed to lock VM state".to_string(),
                module: "DebuggerIntegration".to_string(),
            })
        })?;
        
        Ok(vm_state.breakpoint_hit)
    }

    /// Get last breakpoint ID
    pub fn last_breakpoint_id(&self) -> VmResult<Option<u64>> {
        let vm_state = self.vm_state.lock().map_err(|_| {
            VmError::Core(crate::error::CoreError::Internal {
                message: "Failed to lock VM state".to_string(),
                module: "DebuggerIntegration".to_string(),
            })
        })?;
        
        Ok(vm_state.last_breakpoint_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debugger_integration_creation() {
        let config = DebuggerIntegrationConfig::default();
        let integration = DebuggerIntegration::new(config).unwrap();
        
        assert!(integration.debugger.is_none());
    }

    #[test]
    fn test_debugger_integration_enabled() {
        let mut config = DebuggerIntegrationConfig::default();
        config.enable_debugger = true;
        
        let integration = DebuggerIntegration::new(config).unwrap();
        
        assert!(integration.debugger.is_some());
    }

    #[test]
    fn test_vm_state_default() {
        let state = VmDebuggerState::default();
        
        assert!(matches!(state.execution_status, ExecStatus::Continue));
        assert!(state.last_result.is_none());
        assert_eq!(state.current_thread, 0);
        assert!(!state.is_paused);
        assert!(!state.breakpoint_hit);
        assert!(state.last_breakpoint_id.is_none());
    }
}
