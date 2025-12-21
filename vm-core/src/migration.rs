use crate::{VcpuStateContainer, VmConfig};
use serde::{Deserialize, Serialize};

#[cfg(feature = "no_std")]
use alloc::vec::Vec;
#[cfg(not(feature = "no_std"))]
use std::vec::Vec;

#[derive(Serialize, Deserialize, Debug)]
pub struct MigrationState {
    pub config: VmConfig,
    pub vcpu_states: Vec<VcpuStateContainer>,
    pub memory_dump: Vec<u8>,
}
