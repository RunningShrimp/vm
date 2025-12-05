use crate::{VcpuStateContainer, VmConfig};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct MigrationState {
    pub config: VmConfig,
    pub vcpu_states: Vec<VcpuStateContainer>,
    pub memory_dump: Vec<u8>,
}
