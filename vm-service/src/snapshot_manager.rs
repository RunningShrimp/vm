//! 快照管理模块

use vm_core::{MemoryError, VmError, VmResult};
use vm_core::vm_state::VirtualMachineState;
use std::sync::{Arc, Mutex};

/// 创建快照
pub fn create_snapshot<B>(
    state: Arc<Mutex<VirtualMachineState<B>>>,
    name: String,
    description: String,
) -> VmResult<String> {
        let state_guard = state.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock state".to_string(),
                module: "SnapshotManager".to_string(),
            })
        })?;

        let mmu = state_guard.mmu();
        let mmu_guard = mmu.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire MMU lock".to_string(),
            })
        })?;

        let memory_dump = mmu_guard.dump_memory();
        let id = uuid::Uuid::new_v4().to_string();
        let memory_dump_path = format!("/tmp/{}.memsnap", id);

        // 使用同步文件I/O（保留用于向后兼容）
        std::fs::write(&memory_dump_path, memory_dump).map_err(|e| VmError::Io(e.to_string()))?;

        let snapshot_manager = state_guard.snapshot_manager();
        let mut manager_guard = snapshot_manager.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock snapshot manager".to_string(),
                module: "SnapshotManager".to_string(),
            })
        })?;

        let snapshot_id = manager_guard.create_snapshot(name, description, memory_dump_path);
        Ok(snapshot_id)
}

/// 恢复快照
pub fn restore_snapshot<B>(
    state: Arc<Mutex<VirtualMachineState<B>>>,
    id: &str,
) -> VmResult<()> {
        let state_guard = state.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock state".to_string(),
                module: "SnapshotManager".to_string(),
            })
        })?;

        let snapshot_manager = state_guard.snapshot_manager();
        let mut manager_guard = snapshot_manager.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock snapshot manager".to_string(),
                module: "SnapshotManager".to_string(),
            })
        })?;

        let snapshot = manager_guard
            .snapshots
            .get(id)
            .ok_or_else(|| {
                VmError::Core(vm_core::CoreError::Config {
                    message: "Snapshot not found".to_string(),
                    path: None,
                })
            })?
            .clone();

        // 使用同步文件I/O（保留用于向后兼容）
        let memory_dump =
            std::fs::read(&snapshot.memory_dump_path).map_err(|e| VmError::Io(e.to_string()))?;

        let mmu = state_guard.mmu();
        let mut mmu_guard = mmu.lock().map_err(|_| {
            VmError::Memory(MemoryError::MmuLockFailed {
                message: "Failed to acquire MMU lock".to_string(),
            })
        })?;

        mmu_guard.restore_memory(&memory_dump).map_err(|s| {
            VmError::Memory(MemoryError::MappingFailed {
                message: s,
                src: None,
                dst: None,
            })
        })?;

        manager_guard.restore_snapshot(id).map_err(|s| {
            VmError::Core(vm_core::CoreError::Config {
                message: s,
                path: None,
            })
        })
}

/// 列出所有快照
pub fn list_snapshots<B>(
    state: Arc<Mutex<VirtualMachineState<B>>>,
) -> VmResult<Vec<vm_core::snapshot::Snapshot>> {
        let state_guard = state.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock state".to_string(),
                module: "SnapshotManager".to_string(),
            })
        })?;

        let snapshot_manager = state_guard.snapshot_manager();
        let manager_guard = snapshot_manager.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock snapshot manager".to_string(),
                module: "SnapshotManager".to_string(),
            })
        })?;

        Ok(manager_guard
            .get_snapshot_tree()
            .into_iter()
            .cloned()
            .collect())
}

/// 创建模板
pub fn create_template<B>(
    state: Arc<Mutex<VirtualMachineState<B>>>,
    name: String,
    description: String,
    base_snapshot_id: String,
) -> VmResult<String> {
    let state_guard = state.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to lock state".to_string(),
            module: "SnapshotManager".to_string(),
        })
    })?;

    let template_manager = state_guard.template_manager();
    let mut manager_guard = template_manager.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to lock template manager".to_string(),
            module: "SnapshotManager".to_string(),
        })
    })?;

    let id = manager_guard.create_template(name, description, base_snapshot_id);
    Ok(id)
}

/// 列出所有模板
pub fn list_templates<B>(
    state: Arc<Mutex<VirtualMachineState<B>>>,
) -> VmResult<Vec<vm_core::template::VmTemplate>> {
    let state_guard = state.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to lock state".to_string(),
            module: "SnapshotManager".to_string(),
        })
    })?;

    let template_manager = state_guard.template_manager();
    let manager_guard = template_manager.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to lock template manager".to_string(),
            module: "SnapshotManager".to_string(),
        })
    })?;

    Ok(manager_guard
        .list_templates()
        .into_iter()
        .cloned()
        .collect())
}

/// 序列化虚拟机状态以进行迁移
pub fn serialize_state<B>(state: Arc<Mutex<VirtualMachineState<B>>>) -> VmResult<Vec<u8>> {
    let state_guard = state.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to lock state".to_string(),
            module: "SnapshotManager".to_string(),
        })
    })?;

    let mmu = state_guard.mmu();
    let mmu_guard = mmu.lock().map_err(|_| {
        VmError::Memory(MemoryError::MmuLockFailed {
            message: "Failed to acquire MMU lock".to_string(),
        })
    })?;

    let memory_dump = mmu_guard.dump_memory();

    let mut vcpu_states = Vec::new();
    for vcpu in &state_guard.vcpus {
        let vcpu_guard = vcpu.lock().map_err(|_| {
            VmError::Core(vm_core::CoreError::Internal {
                message: "Failed to lock vCPU".to_string(),
                module: "SnapshotManager".to_string(),
            })
        })?;
        vcpu_states.push(vcpu_guard.get_vcpu_state());
    }

    let migration_state = vm_core::migration::MigrationState {
        config: state_guard.config().clone(),
        vcpu_states,
        memory_dump,
    };

    bincode::serde::encode_to_vec(&migration_state, bincode::config::standard()).map_err(|e| {
        VmError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()).to_string())
    })
}

/// 从序列化数据中反序列化并恢复虚拟机状态
pub fn deserialize_state<B>(
    state: Arc<Mutex<VirtualMachineState<B>>>,
    data: &[u8],
) -> VmResult<()> {
    let (migration_state, _): (vm_core::migration::MigrationState, usize) =
        bincode::serde::decode_from_slice(data, bincode::config::standard()).map_err(|e| {
            VmError::Io(
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string()).to_string(),
            )
        })?;

    let mut state_guard = state.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to lock state".to_string(),
            module: "SnapshotManager".to_string(),
        })
    })?;

    let mmu = state_guard.mmu();
    let mut mmu_guard = mmu.lock().map_err(|_| {
        VmError::Memory(MemoryError::MmuLockFailed {
            message: "Failed to acquire MMU lock".to_string(),
        })
    })?;

    mmu_guard
        .restore_memory(&migration_state.memory_dump)
        .map_err(|s| {
            VmError::Memory(MemoryError::MappingFailed {
                message: s,
                src: None,
                dst: None,
            })
        })?;

    for (i, vcpu_state) in migration_state.vcpu_states.iter().enumerate() {
        if let Some(vcpu) = state_guard.vcpus.get(i) {
            let mut vcpu_guard = vcpu.lock().map_err(|_| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: "Failed to lock vCPU during restore".to_string(),
                    module: "SnapshotManager".to_string(),
                })
            })?;
            vcpu_guard.set_vcpu_state(vcpu_state);
        }
    }

    Ok(())
}

/// 异步创建快照
#[cfg(feature = "async")]
pub async fn create_snapshot_async<B>(
    state: Arc<tokio::sync::Mutex<VirtualMachineState<B>>>,
    name: String,
    description: String,
) -> VmResult<String> {
    use vm_mem::async_mmu::async_file_io;
    use vm_mem::AsyncMMU;
    use vm_mem::AsyncMmuWrapper;
    
    let state_guard = state.lock().await;
    
    // 获取MMU并转换为异步MMU
    let mmu_arc = state_guard.mmu();
    // 注意：这里需要将同步Mutex转换为异步Mutex
    // 为了简化，我们使用tokio::task::spawn_blocking来处理同步MMU操作
    let memory_dump = tokio::task::spawn_blocking(move || {
        let mmu_guard = mmu_arc.lock().ok()?;
        Some(mmu_guard.dump_memory())
    })
    .await
    .map_err(|e| VmError::Io(format!("Failed to dump memory: {}", e)))?
    .ok_or_else(|| VmError::Memory(MemoryError::MmuLockFailed {
        message: "Failed to acquire MMU lock".to_string(),
    }))?;
    
    let id = uuid::Uuid::new_v4().to_string();
    let memory_dump_path = format!("/tmp/{}.memsnap", id);
    
    // 使用异步文件I/O
    async_file_io::write_memory_to_file(&memory_dump_path, &memory_dump).await?;
    
    let snapshot_manager = state_guard.snapshot_manager();
    let mut manager_guard = snapshot_manager.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to lock snapshot manager".to_string(),
            module: "SnapshotManager".to_string(),
        })
    })?;
    
    let snapshot_id = manager_guard.create_snapshot(name, description, memory_dump_path);
    Ok(snapshot_id)
}

/// 异步恢复快照
#[cfg(feature = "async")]
pub async fn restore_snapshot_async<B>(
    state: Arc<tokio::sync::Mutex<VirtualMachineState<B>>>,
    id: &str,
) -> VmResult<()> {
    use vm_mem::async_mmu::async_file_io;
    
    let state_guard = state.lock().await;
    
    let snapshot_manager = state_guard.snapshot_manager();
    let mut manager_guard = snapshot_manager.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to lock snapshot manager".to_string(),
            module: "SnapshotManager".to_string(),
        })
    })?;
    
    let snapshot = manager_guard
        .snapshots
        .get(id)
        .ok_or_else(|| {
            VmError::Core(vm_core::CoreError::Config {
                message: "Snapshot not found".to_string(),
                path: None,
            })
        })?
        .clone();
    
    drop(state_guard);
    
    // 使用异步文件I/O读取快照
    let memory_dump = async_file_io::read_file_to_memory(&snapshot.memory_dump_path).await?;
    
    // 恢复内存（使用spawn_blocking处理同步MMU操作）
    let mmu_arc = {
        let state_guard = state.lock().await;
        state_guard.mmu()
    };
    
    tokio::task::spawn_blocking(move || {
        let mut mmu_guard = mmu_arc.lock().ok()?;
        mmu_guard.restore_memory(&memory_dump).ok()?;
        Some(())
    })
    .await
    .map_err(|e| VmError::Io(format!("Failed to restore memory: {}", e)))?
    .ok_or_else(|| VmError::Memory(MemoryError::MmuLockFailed {
        message: "Failed to acquire MMU lock".to_string(),
    }))?;
    
    let state_guard = state.lock().await;
    let snapshot_manager = state_guard.snapshot_manager();
    let mut manager_guard = snapshot_manager.lock().map_err(|_| {
        VmError::Core(vm_core::CoreError::Internal {
            message: "Failed to lock snapshot manager".to_string(),
            module: "SnapshotManager".to_string(),
        })
    })?;
    
    manager_guard.restore_snapshot(id).map_err(|s| {
        VmError::Core(vm_core::CoreError::Config {
            message: s,
            path: None,
        })
    })
}

