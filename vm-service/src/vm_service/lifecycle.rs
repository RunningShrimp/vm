//! 虚拟机生命周期管理模块

use std::sync::{Arc, Mutex};
use vm_core::vm_state::VirtualMachineState;
use vm_core::{VmError, VmLifecycleState, VmResult};

/// 启动虚拟机
pub fn start<B: 'static>(state: Arc<Mutex<VirtualMachineState<B>>>) -> VmResult<()> {
    let mut state_guard = state.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to lock state".to_string(),
            module: "LifecycleManager".to_string(),
        })
    })?;

    if state_guard.state() != VmLifecycleState::Created
        && state_guard.state() != VmLifecycleState::Paused
    {
        return Err(VmError::Core(vm_core::CoreError::Config {
            message: "VM not in startable state".to_string(),
            path: None,
        }));
    }

    state_guard.set_state(VmLifecycleState::Running);
    Ok(())
}

/// 暂停虚拟机
pub fn pause<B: 'static>(state: Arc<Mutex<VirtualMachineState<B>>>) -> VmResult<()> {
    let mut state_guard = state.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to lock state".to_string(),
            module: "LifecycleManager".to_string(),
        })
    })?;

    if state_guard.state() != VmLifecycleState::Running {
        return Err(VmError::Core(vm_core::CoreError::Config {
            message: "VM not running".to_string(),
            path: None,
        }));
    }

    state_guard.set_state(VmLifecycleState::Paused);
    Ok(())
}

/// 停止虚拟机
pub fn stop<B: 'static>(state: Arc<Mutex<VirtualMachineState<B>>>) -> VmResult<()> {
    let mut state_guard = state.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to lock state".to_string(),
            module: "LifecycleManager".to_string(),
        })
    })?;

    state_guard.set_state(VmLifecycleState::Stopped);
    Ok(())
}

/// 重置虚拟机
pub fn reset<B: 'static>(state: Arc<Mutex<VirtualMachineState<B>>>) -> VmResult<()> {
    let mut state_guard = state.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to lock state".to_string(),
            module: "LifecycleManager".to_string(),
        })
    })?;

    state_guard.set_state(VmLifecycleState::Created);

    let mmu = state_guard.mmu();
    let mut mmu_guard = mmu.lock().map_err(|_| {
        VmError::Memory(vm_core::MemoryError::MmuLockFailed {
            message: "Failed to acquire MMU lock".to_string(),
        })
    })?;

    mmu_guard.flush_tlb();
    Ok(())
}

/// 请求停止执行
pub fn request_stop(run_flag: &std::sync::Arc<std::sync::atomic::AtomicBool>) {
    run_flag.store(false, std::sync::atomic::Ordering::Relaxed);
}

/// 请求暂停执行
pub fn request_pause(pause_flag: &std::sync::Arc<std::sync::atomic::AtomicBool>) {
    pause_flag.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// 请求恢复执行
pub fn request_resume(pause_flag: &std::sync::Arc<std::sync::atomic::AtomicBool>) {
    pause_flag.store(false, std::sync::atomic::Ordering::Relaxed);
}
