use serde::{Serialize, Deserialize};
use crate::{VcpuStateContainer, VmConfig};

#[derive(Serialize, Deserialize, Debug)]
pub struct MigrationState {
    pub config: VmConfig,
    pub vcpu_states: Vec<VcpuStateContainer>,
    pub memory_dump: Vec<u8>,
}
