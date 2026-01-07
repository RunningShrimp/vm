use serde::{Deserialize, Serialize};

use crate::{VcpuStateContainer, VmConfig};

/// VM迁移状态
///
/// 用于保存和恢复VM完整状态，支持实时迁移。
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MigrationState {
    pub config: VmConfig,
    pub vcpu_states: Vec<VcpuStateContainer>,
    pub memory_dump: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_state_empty() {
        let state = MigrationState {
            config: VmConfig::default(),
            vcpu_states: vec![],
            memory_dump: vec![],
        };
        assert_eq!(state.vcpu_states.len(), 0);
        assert_eq!(state.memory_dump.len(), 0);
    }

    #[test]
    fn test_migration_state_with_memory() {
        let memory = vec![42u8; 1024];
        let state = MigrationState {
            config: VmConfig::default(),
            vcpu_states: vec![],
            memory_dump: memory.clone(),
        };
        assert_eq!(state.memory_dump.len(), 1024);
        assert_eq!(state.memory_dump[0], 42);
    }

    #[test]
    fn test_migration_state_debug() {
        let state = MigrationState {
            config: VmConfig::default(),
            vcpu_states: vec![],
            memory_dump: vec![1, 2, 3],
        };
        // Just test that Debug trait works
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("MigrationState"));
    }

    #[test]
    fn test_migration_state_fields() {
        let state = MigrationState {
            config: VmConfig::default(),
            vcpu_states: vec![],
            memory_dump: vec![1, 2, 3, 4, 5],
        };
        // Test that fields are accessible
        assert_eq!(state.memory_dump.len(), 5);
        assert_eq!(state.memory_dump[0], 1);
        assert_eq!(state.memory_dump[4], 5);
    }
}
