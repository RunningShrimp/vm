use serde::{Deserialize, Serialize};

use crate::{VcpuStateContainer, VmConfig};

#[derive(Serialize, Deserialize, Debug)]
pub struct MigrationState {
    pub config: VmConfig,
    pub vcpu_states: Vec<VcpuStateContainer>,
    pub memory_dump: Vec<u8>,
}
