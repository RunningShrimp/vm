use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use vm_core::{GuestAddr, MMU, VmResult};

/// SR-IOV (Single Root I/O Virtualization) support for network devices
///
/// This module provides comprehensive SR-IOV functionality including:
/// - Physical Function (PF) management
/// - Virtual Function (VF) creation and management
/// - VF resource allocation and isolation
/// - VF configuration and state management
/// - VF to PF communication
/// - VF migration support
///
/// SR-IOV capability structure
#[derive(Debug, Clone, Serialize)]
pub struct SrIovCapability {
    /// Total number of VFs supported
    pub total_vfs: u16,
    /// Initial number of VFs
    pub initial_vfs: u16,
    /// Number of VFs currently enabled
    pub num_vfs: u16,
    /// VF device ID
    pub vf_device_id: u16,
    /// Supported link speeds
    pub supported_link_speeds: u32,
    /// VF offset and stride
    pub vf_offset: u16,
    pub vf_stride: u16,
    /// VF migration support
    pub vf_migration: bool,
    /// VF memory space size
    pub vf_memory_size: u32,
}

impl Default for SrIovCapability {
    fn default() -> Self {
        Self {
            total_vfs: 64,
            initial_vfs: 0,
            num_vfs: 0,
            vf_device_id: 0x10fb,        // Intel XL710 VF device ID
            supported_link_speeds: 0x1f, // All speeds
            vf_offset: 0,
            vf_stride: 0x1000,
            vf_migration: true,
            vf_memory_size: 0x10000,
        }
    }
}

/// VF (Virtual Function) configuration
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct VfConfig {
    /// VF ID
    pub vf_id: u16,
    /// VF is enabled
    pub enabled: bool,
    /// VF MAC address
    pub mac_address: [u8; 6],
    /// VF VLAN ID
    pub vlan_id: u16,
    /// VF QoS priority
    pub qos_priority: u8,
    /// VF link state
    pub link_state: VfLinkState,
    /// VF trust mode
    pub trusted: bool,
    /// VF spoof checking
    pub spoof_check: bool,
    /// VF max transmit rate (Mbps)
    pub max_tx_rate: u32,
    /// VF min transmit rate (Mbps)
    pub min_tx_rate: u32,
    /// VF queue configuration
    pub num_rx_queues: u16,
    pub num_tx_queues: u16,
    /// VF memory regions
    pub memory_regions: Vec<VfMemoryRegion>,
}

impl Default for VfConfig {
    fn default() -> Self {
        Self {
            vf_id: 0,
            enabled: false,
            mac_address: [0x02, 0x00, 0x00, 0x00, 0x00, 0x00],
            vlan_id: 0,
            qos_priority: 0,
            link_state: VfLinkState::Auto,
            trusted: false,
            spoof_check: true,
            max_tx_rate: 0,
            min_tx_rate: 0,
            num_rx_queues: 2,
            num_tx_queues: 2,
            memory_regions: Vec::new(),
        }
    }
}

/// VF link state
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum VfLinkState {
    /// Link is forced down
    Down,
    /// Link is forced up
    Up,
    /// Link follows physical function state
    Auto,
}

/// VF memory region
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct VfMemoryRegion {
    /// Region type
    pub region_type: VfMemoryRegionType,
    /// Guest physical address
    pub guest_addr: u64,
    /// Size in bytes
    pub size: u64,
    /// Host virtual address
    pub host_addr: u64,
    /// Region flags
    pub flags: u32,
}

/// VF memory region type
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum VfMemoryRegionType {
    /// MMIO region
    Mmio,
    /// MSI-X table
    MsixTable,
    /// MSI-X PBA
    MsixPba,
    /// Doorbell region
    Doorbell,
    /// Descriptor rings
    DescriptorRings,
}

/// VF statistics
#[derive(Debug, Default)]
pub struct VfStats {
    /// RX statistics
    pub rx_packets: AtomicU64,
    pub rx_bytes: AtomicU64,
    pub rx_errors: AtomicU64,
    pub rx_dropped: AtomicU64,
    /// TX statistics
    pub tx_packets: AtomicU64,
    pub tx_bytes: AtomicU64,
    pub tx_errors: AtomicU64,
    pub tx_dropped: AtomicU64,
    /// Interrupt statistics
    pub interrupts: AtomicU64,
    /// Queue statistics
    pub queue_stats: HashMap<u16, VfQueueStats>,
}

impl Clone for VfStats {
    fn clone(&self) -> Self {
        Self {
            rx_packets: AtomicU64::new(self.rx_packets.load(Ordering::Relaxed)),
            rx_bytes: AtomicU64::new(self.rx_bytes.load(Ordering::Relaxed)),
            rx_errors: AtomicU64::new(self.rx_errors.load(Ordering::Relaxed)),
            rx_dropped: AtomicU64::new(self.rx_dropped.load(Ordering::Relaxed)),
            tx_packets: AtomicU64::new(self.tx_packets.load(Ordering::Relaxed)),
            tx_bytes: AtomicU64::new(self.tx_bytes.load(Ordering::Relaxed)),
            tx_errors: AtomicU64::new(self.tx_errors.load(Ordering::Relaxed)),
            tx_dropped: AtomicU64::new(self.tx_dropped.load(Ordering::Relaxed)),
            interrupts: AtomicU64::new(self.interrupts.load(Ordering::Relaxed)),
            queue_stats: self.queue_stats.clone(),
        }
    }
}

/// VF queue statistics
#[derive(Debug, Default)]
pub struct VfQueueStats {
    pub packets: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    pub desc_used: AtomicU64,
    pub desc_avail: AtomicU64,
}

impl Clone for VfQueueStats {
    fn clone(&self) -> Self {
        Self {
            packets: AtomicU64::new(self.packets.load(Ordering::Relaxed)),
            bytes: AtomicU64::new(self.bytes.load(Ordering::Relaxed)),
            errors: AtomicU64::new(self.errors.load(Ordering::Relaxed)),
            desc_used: AtomicU64::new(self.desc_used.load(Ordering::Relaxed)),
            desc_avail: AtomicU64::new(self.desc_avail.load(Ordering::Relaxed)),
        }
    }
}

/// VF migration state
#[derive(Debug, Clone)]
pub struct VfMigrationState {
    /// Migration is in progress
    pub migrating: bool,
    /// Migration phase
    pub phase: VfMigrationPhase,
    /// Migration data
    pub migration_data: Vec<u8>,
    /// Source VF ID
    pub source_vf_id: Option<u16>,
    /// Target VF ID
    pub target_vf_id: Option<u16>,
}

/// VF migration phase
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VfMigrationPhase {
    /// Not migrating
    None,
    /// Pre-copy phase
    PreCopy,
    /// Stop-and-copy phase
    StopAndCopy,
    /// Resume phase
    Resume,
    /// Completed
    Completed,
    /// Failed
    Failed,
}

/// SR-IOV manager
pub struct SrIovManager {
    /// SR-IOV capability
    capability: SrIovCapability,
    /// VF configurations
    vfs: Arc<Mutex<HashMap<u16, VfConfig>>>,
    /// VF statistics
    vf_stats: Arc<Mutex<HashMap<u16, VfStats>>>,
    /// VF migration states
    vf_migration: Arc<Mutex<HashMap<u16, VfMigrationState>>>,
    /// PF MAC address
    pf_mac_address: [u8; 6],
    /// PF link state
    pf_link_state: AtomicBool,
    /// Global VF ID counter
    next_vf_id: AtomicU16,
    /// MMU reference
    mmu: Arc<Mutex<Box<dyn MMU>>>,
    /// Guest memory
    guest_memory: Option<Arc<vm_mem::PhysicalMemory>>,
}

impl SrIovManager {
    /// Create a new SR-IOV manager
    pub fn new(mmu: Arc<Mutex<Box<dyn MMU>>>) -> Self {
        Self {
            capability: SrIovCapability::default(),
            vfs: Arc::new(Mutex::new(HashMap::new())),
            vf_stats: Arc::new(Mutex::new(HashMap::new())),
            vf_migration: Arc::new(Mutex::new(HashMap::new())),
            pf_mac_address: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
            pf_link_state: AtomicBool::new(true),
            next_vf_id: AtomicU16::new(0),
            mmu,
            guest_memory: None,
        }
    }

    // Helper methods for safe lock acquisition
    fn lock_vfs(&self) -> VmResult<std::sync::MutexGuard<'_, HashMap<u16, VfConfig>>> {
        self.vfs.lock().map_err(|_| {
            vm_core::VmError::Memory(vm_core::MemoryError::PageTableError {
                message: "SR-IOV VFs lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    fn lock_vf_stats(&self) -> VmResult<std::sync::MutexGuard<'_, HashMap<u16, VfStats>>> {
        self.vf_stats.lock().map_err(|_| {
            vm_core::VmError::Memory(vm_core::MemoryError::PageTableError {
                message: "SR-IOV VF stats lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    fn lock_vf_migration(
        &self,
    ) -> VmResult<std::sync::MutexGuard<'_, HashMap<u16, VfMigrationState>>> {
        self.vf_migration.lock().map_err(|_| {
            vm_core::VmError::Memory(vm_core::MemoryError::PageTableError {
                message: "SR-IOV VF migration lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    fn lock_mmu(&self) -> VmResult<std::sync::MutexGuard<'_, Box<dyn MMU>>> {
        self.mmu.lock().map_err(|_| {
            vm_core::VmError::Memory(vm_core::MemoryError::PageTableError {
                message: "SR-IOV MMU lock is poisoned".to_string(),
                level: None,
            })
        })
    }

    /// Initialize SR-IOV capability
    pub fn initialize(&mut self, capability: SrIovCapability) -> VmResult<()> {
        self.capability = capability;

        // Initialize VF configurations
        let mut vfs = self.lock_vfs()?;
        for vf_id in 0..self.capability.total_vfs {
            let mut mac_address = VfConfig::default().mac_address;
            mac_address[5] = vf_id as u8;
            let vf_config = VfConfig {
                vf_id,
                mac_address,
                ..Default::default()
            };
            vfs.insert(vf_id, vf_config);
        }

        // Initialize VF statistics
        let mut vf_stats = self.lock_vf_stats()?;
        for vf_id in 0..self.capability.total_vfs {
            vf_stats.insert(vf_id, VfStats::default());
        }

        // Initialize VF migration states
        let mut vf_migration = self.lock_vf_migration()?;
        for vf_id in 0..self.capability.total_vfs {
            vf_migration.insert(
                vf_id,
                VfMigrationState {
                    migrating: false,
                    phase: VfMigrationPhase::None,
                    migration_data: Vec::new(),
                    source_vf_id: None,
                    target_vf_id: None,
                },
            );
        }

        Ok(())
    }

    /// Enable SR-IOV and create VFs
    pub fn enable_sriov(&mut self, num_vfs: u16) -> VmResult<()> {
        if num_vfs > self.capability.total_vfs {
            return Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "num_vfs".to_string(),
                    value: num_vfs.to_string(),
                    message: format!(
                        "Requested {} VFs exceeds maximum of {}",
                        num_vfs, self.capability.total_vfs
                    ),
                },
            ));
        }

        self.capability.num_vfs = num_vfs;

        // Enable the specified number of VFs
        let mut vfs = self.lock_vfs()?;
        for vf_id in 0..num_vfs {
            if let Some(vf_config) = vfs.get_mut(&vf_id) {
                vf_config.enabled = true;
            }
        }

        Ok(())
    }

    /// Dynamically allocate a new VF ID
    pub fn allocate_vf_id(&self) -> u16 {
        self.next_vf_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// Write VF configuration to guest memory
    pub fn write_vf_config_to_memory(
        &self,
        vf_id: u16,
        guest_addr: vm_core::GuestAddr,
        size: usize,
    ) -> VmResult<()> {
        let vfs = self.lock_vfs()?;
        let vf_config = vfs.get(&vf_id).ok_or_else(|| {
            vm_core::error::VmError::Core(vm_core::error::CoreError::Internal {
                message: format!("VF {} not found", vf_id),
                module: "sriov".to_string(),
            })
        })?;

        // Convert VF config to bytes
        let config_bytes =
            bincode::encode_to_vec(vf_config, bincode::config::standard()).map_err(|_e| {
                vm_core::error::VmError::Core(vm_core::error::CoreError::NotSupported {
                    feature: "bincode serialization".to_string(),
                    module: "sriov".to_string(),
                })
            })?;

        // Ensure the buffer is not larger than the requested size
        if config_bytes.len() > size {
            return Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::Internal {
                    message: format!(
                        "Buffer size too small: requested {}, needed {}",
                        size,
                        config_bytes.len()
                    ),
                    module: "sriov".to_string(),
                },
            ));
        }

        // Use MMU to write the configuration to guest memory
        let mut mmu = self.lock_mmu()?;
        mmu.write_bulk(guest_addr, &config_bytes)?;

        Ok(())
    }

    /// Disable SR-IOV and remove VFs
    pub fn disable_sriov(&mut self) -> VmResult<()> {
        // Disable all VFs
        {
            let mut vfs = self.lock_vfs()?;
            for vf_config in vfs.values_mut() {
                vf_config.enabled = false;
            }
        } // Drop the lock guard here

        self.capability.num_vfs = 0;
        Ok(())
    }

    /// Get VF configuration
    pub fn get_vf_config(&self, vf_id: u16) -> VmResult<VfConfig> {
        let vfs = self.lock_vfs()?;
        vfs.get(&vf_id).cloned().ok_or_else(|| {
            vm_core::error::VmError::Core(vm_core::error::CoreError::InvalidParameter {
                name: "vf_id".to_string(),
                value: vf_id.to_string(),
                message: format!("VF {} not found", vf_id),
            })
        })
    }

    /// Set VF configuration
    pub fn set_vf_config(&mut self, vf_id: u16, config: VfConfig) -> VmResult<()> {
        let mut vfs = self.lock_vfs()?;
        if let Some(vf_config) = vfs.get_mut(&vf_id) {
            *vf_config = config;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Get VF statistics
    pub fn get_vf_stats(&self, vf_id: u16) -> VmResult<VfStats> {
        let vf_stats = self.lock_vf_stats()?;
        vf_stats.get(&vf_id).cloned().ok_or_else(|| {
            vm_core::error::VmError::Core(vm_core::error::CoreError::InvalidParameter {
                name: "vf_id".to_string(),
                value: vf_id.to_string(),
                message: format!("VF {} not found", vf_id),
            })
        })
    }

    /// Reset VF statistics
    pub fn reset_vf_stats(&mut self, vf_id: u16) -> VmResult<()> {
        let mut vf_stats = self.lock_vf_stats()?;
        if let Some(stats) = vf_stats.get_mut(&vf_id) {
            *stats = VfStats::default();
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Set VF MAC address
    pub fn set_vf_mac(&mut self, vf_id: u16, mac_address: [u8; 6]) -> VmResult<()> {
        let mut vfs = self.lock_vfs()?;
        if let Some(vf_config) = vfs.get_mut(&vf_id) {
            vf_config.mac_address = mac_address;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Set VF VLAN
    pub fn set_vf_vlan(&mut self, vf_id: u16, vlan_id: u16) -> VmResult<()> {
        let mut vfs = self.lock_vfs()?;
        if let Some(vf_config) = vfs.get_mut(&vf_id) {
            vf_config.vlan_id = vlan_id;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Set VF QoS priority
    pub fn set_vf_qos(&mut self, vf_id: u16, priority: u8) -> VmResult<()> {
        let mut vfs = self.lock_vfs()?;
        if let Some(vf_config) = vfs.get_mut(&vf_id) {
            vf_config.qos_priority = priority;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Set VF link state
    pub fn set_vf_link_state(&mut self, vf_id: u16, link_state: VfLinkState) -> VmResult<()> {
        let mut vfs = self.lock_vfs()?;
        if let Some(vf_config) = vfs.get_mut(&vf_id) {
            vf_config.link_state = link_state;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Set VF trust mode
    pub fn set_vf_trust(&mut self, vf_id: u16, trusted: bool) -> VmResult<()> {
        let mut vfs = self.lock_vfs()?;
        if let Some(vf_config) = vfs.get_mut(&vf_id) {
            vf_config.trusted = trusted;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Set VF spoof checking
    pub fn set_vf_spoof_check(&mut self, vf_id: u16, spoof_check: bool) -> VmResult<()> {
        let mut vfs = self.lock_vfs()?;
        if let Some(vf_config) = vfs.get_mut(&vf_id) {
            vf_config.spoof_check = spoof_check;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Set VF transmit rate
    pub fn set_vf_tx_rate(&mut self, vf_id: u16, min_rate: u32, max_rate: u32) -> VmResult<()> {
        let mut vfs = self.lock_vfs()?;
        if let Some(vf_config) = vfs.get_mut(&vf_id) {
            vf_config.min_tx_rate = min_rate;
            vf_config.max_tx_rate = max_rate;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Get SR-IOV capability
    pub fn get_capability(&self) -> &SrIovCapability {
        &self.capability
    }

    /// Get PF MAC address
    pub fn get_pf_mac_address(&self) -> [u8; 6] {
        self.pf_mac_address
    }

    /// Set PF MAC address
    pub fn set_pf_mac_address(&mut self, mac_address: [u8; 6]) {
        self.pf_mac_address = mac_address;
    }

    /// Get PF link state
    pub fn get_pf_link_state(&self) -> bool {
        self.pf_link_state.load(Ordering::Acquire)
    }

    /// Set PF link state
    pub fn set_pf_link_state(&self, link_state: bool) {
        self.pf_link_state.store(link_state, Ordering::Release);
    }

    /// Check if VF link is up
    pub fn is_vf_link_up(&self, vf_id: u16) -> VmResult<bool> {
        let vfs = self.lock_vfs()?;
        if let Some(vf_config) = vfs.get(&vf_id) {
            match vf_config.link_state {
                VfLinkState::Up => Ok(true),
                VfLinkState::Down => Ok(false),
                VfLinkState::Auto => Ok(self.pf_link_state.load(Ordering::Acquire)),
            }
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Start VF migration
    pub fn start_vf_migration(&mut self, source_vf_id: u16, target_vf_id: u16) -> VmResult<()> {
        let mut vf_migration = self.lock_vf_migration()?;

        // Check if VFs exist
        let vfs = self.lock_vfs()?;
        if !vfs.contains_key(&source_vf_id) || !vfs.contains_key(&target_vf_id) {
            return Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "source_vf_id/target_vf_id".to_string(),
                    value: format!("{}/{}", source_vf_id, target_vf_id),
                    message: "Source or target VF not found".to_string(),
                },
            ));
        }
        drop(vfs);

        // Check if migration is already in progress
        if let Some(migration_state) = vf_migration.get(&source_vf_id)
            && migration_state.migrating
        {
            return Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: source_vf_id.to_string(),
                    message: "Migration already in progress".to_string(),
                },
            ));
        }

        // Start migration
        vf_migration.insert(
            source_vf_id,
            VfMigrationState {
                migrating: true,
                phase: VfMigrationPhase::PreCopy,
                migration_data: Vec::new(),
                source_vf_id: Some(source_vf_id),
                target_vf_id: Some(target_vf_id),
            },
        );

        Ok(())
    }

    /// Get VF migration state
    pub fn get_vf_migration_state(&self, vf_id: u16) -> VmResult<VfMigrationState> {
        let vf_migration = self.lock_vf_migration()?;
        vf_migration.get(&vf_id).cloned().ok_or_else(|| {
            vm_core::error::VmError::Core(vm_core::error::CoreError::InvalidParameter {
                name: "vf_id".to_string(),
                value: vf_id.to_string(),
                message: format!("VF {} not found", vf_id),
            })
        })
    }

    /// Complete VF migration
    pub fn complete_vf_migration(&mut self, vf_id: u16) -> VmResult<()> {
        let mut vf_migration = self.lock_vf_migration()?;
        if let Some(migration_state) = vf_migration.get_mut(&vf_id) {
            migration_state.phase = VfMigrationPhase::Completed;
            migration_state.migrating = false;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Add VF memory region
    pub fn add_vf_memory_region(&mut self, vf_id: u16, region: VfMemoryRegion) -> VmResult<()> {
        let mut vfs = self.lock_vfs()?;
        if let Some(vf_config) = vfs.get_mut(&vf_id) {
            vf_config.memory_regions.push(region);
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Remove VF memory region
    pub fn remove_vf_memory_region(
        &mut self,
        vf_id: u16,
        region_type: VfMemoryRegionType,
    ) -> VmResult<()> {
        let mut vfs = self.lock_vfs()?;
        if let Some(vf_config) = vfs.get_mut(&vf_id) {
            vf_config
                .memory_regions
                .retain(|r| r.region_type != region_type);
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Get VF memory regions
    pub fn get_vf_memory_regions(&self, vf_id: u16) -> VmResult<Vec<VfMemoryRegion>> {
        let vfs = self.lock_vfs()?;
        if let Some(vf_config) = vfs.get(&vf_id) {
            Ok(vf_config.memory_regions.clone())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Update VF queue statistics
    pub fn update_vf_queue_stats(
        &self,
        vf_id: u16,
        queue_id: u16,
        packets: u64,
        bytes: u64,
        errors: u64,
    ) -> VmResult<()> {
        let mut vf_stats = self.lock_vf_stats()?;
        if let Some(stats) = vf_stats.get_mut(&vf_id) {
            let queue_stats = stats
                .queue_stats
                .entry(queue_id)
                .or_insert_with(VfQueueStats::default);
            queue_stats.packets.fetch_add(packets, Ordering::Relaxed);
            queue_stats.bytes.fetch_add(bytes, Ordering::Relaxed);
            queue_stats.errors.fetch_add(errors, Ordering::Relaxed);
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Update VF RX statistics
    pub fn update_vf_rx_stats(
        &self,
        vf_id: u16,
        packets: u64,
        bytes: u64,
        errors: u64,
        dropped: u64,
    ) -> VmResult<()> {
        let mut vf_stats = self.lock_vf_stats()?;
        if let Some(stats) = vf_stats.get_mut(&vf_id) {
            stats.rx_packets.fetch_add(packets, Ordering::Relaxed);
            stats.rx_bytes.fetch_add(bytes, Ordering::Relaxed);
            stats.rx_errors.fetch_add(errors, Ordering::Relaxed);
            stats.rx_dropped.fetch_add(dropped, Ordering::Relaxed);
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Update VF TX statistics
    pub fn update_vf_tx_stats(
        &self,
        vf_id: u16,
        packets: u64,
        bytes: u64,
        errors: u64,
        dropped: u64,
    ) -> VmResult<()> {
        let mut vf_stats = self.lock_vf_stats()?;
        if let Some(stats) = vf_stats.get_mut(&vf_id) {
            stats.tx_packets.fetch_add(packets, Ordering::Relaxed);
            stats.tx_bytes.fetch_add(bytes, Ordering::Relaxed);
            stats.tx_errors.fetch_add(errors, Ordering::Relaxed);
            stats.tx_dropped.fetch_add(dropped, Ordering::Relaxed);
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Update VF interrupt statistics
    pub fn update_vf_interrupt_stats(&self, vf_id: u16, count: u64) -> VmResult<()> {
        let mut vf_stats = self.lock_vf_stats()?;
        if let Some(stats) = vf_stats.get_mut(&vf_id) {
            stats.interrupts.fetch_add(count, Ordering::Relaxed);
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vf_id".to_string(),
                    value: vf_id.to_string(),
                    message: format!("VF {} not found", vf_id),
                },
            ))
        }
    }

    /// Set guest memory reference
    pub fn set_guest_memory(&mut self, guest_memory: Arc<vm_mem::PhysicalMemory>) {
        self.guest_memory = Some(guest_memory);
    }

    /// Get all enabled VFs
    pub fn get_enabled_vfs(&self) -> Vec<u16> {
        match self.lock_vfs() {
            Ok(vfs) => vfs
                .iter()
                .filter(|(_, config)| config.enabled)
                .map(|(vf_id, _)| *vf_id)
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Get all VF configurations
    pub fn get_all_vf_configs(&self) -> HashMap<u16, VfConfig> {
        match self.lock_vfs() {
            Ok(vfs) => vfs.clone(),
            Err(_) => HashMap::new(),
        }
    }

    /// Get all VF statistics
    pub fn get_all_vf_stats(&self) -> HashMap<u16, VfStats> {
        match self.lock_vf_stats() {
            Ok(vf_stats) => vf_stats.clone(),
            Err(_) => HashMap::new(),
        }
    }
}

/// SR-IOV PCI configuration space helper
pub struct SrIovPciConfig {
    /// SR-IOV capability offset
    pub capability_offset: u16,
    /// SR-IOV control register
    pub control: u16,
    /// SR-IOV status register
    pub status: u16,
    /// Initial VFs
    pub initial_vfs: u16,
    /// Total VFs
    pub total_vfs: u16,
    /// Number VFs
    pub num_vfs: u16,
    /// Function dependency link
    pub function_dependency_link: u8,
    /// VF offset
    pub vf_offset: u16,
    /// VF stride
    pub vf_stride: u16,
    /// VF device ID
    pub vf_device_id: u16,
    /// Supported page sizes
    pub supported_page_sizes: u32,
    /// System page size
    pub system_page_size: u32,
    /// VF BAR0
    pub vf_bar0: u32,
    /// VF BAR1
    pub vf_bar1: u32,
    /// VF BAR2
    pub vf_bar2: u32,
    /// VF BAR3
    pub vf_bar3: u32,
    /// VF BAR4
    pub vf_bar4: u32,
    /// VF BAR5
    pub vf_bar5: u32,
    /// VF migration state offset
    pub vf_migration_state_offset: u32,
}

impl Default for SrIovPciConfig {
    fn default() -> Self {
        Self {
            capability_offset: 0x100,
            control: 0,
            status: 0,
            initial_vfs: 0,
            total_vfs: 64,
            num_vfs: 0,
            function_dependency_link: 0,
            vf_offset: 0,
            vf_stride: 0x1000,
            vf_device_id: 0x10fb,
            supported_page_sizes: 0x55355, // 4K, 8K, 64K, 2M, 4M, 1G
            system_page_size: 0x1000,      // 4K
            vf_bar0: 0,
            vf_bar1: 0,
            vf_bar2: 0,
            vf_bar3: 0,
            vf_bar4: 0,
            vf_bar5: 0,
            vf_migration_state_offset: 0,
        }
    }
}

/// SR-IOV VF device implementation
pub struct SrIovVirtualFunction {
    /// VF ID
    vf_id: u16,
    /// VF configuration
    config: VfConfig,
    /// VF statistics
    stats: VfStats,
    /// VF PCI configuration
    pci_config: SrIovPciConfig,
    /// VF is initialized
    initialized: bool,
    /// VF MMIO regions
    mmio_regions: HashMap<u32, (GuestAddr, u64)>,
    /// VF interrupt vectors
    interrupt_vectors: Vec<bool>,
    /// VF queues
    rx_queues: HashMap<u16, VfQueue>,
    tx_queues: HashMap<u16, VfQueue>,
}

/// VF queue implementation
#[derive(Debug)]
pub struct VfQueue {
    /// Queue ID
    pub queue_id: u16,
    /// Queue size
    pub size: u16,
    /// Queue enabled
    pub enabled: bool,
    /// Descriptor ring address
    pub desc_addr: GuestAddr,
    /// Descriptor ring size
    pub desc_size: u16,
    /// Available ring address
    pub avail_addr: GuestAddr,
    /// Used ring address
    pub used_addr: GuestAddr,
    /// Queue statistics
    pub stats: VfQueueStats,
}

impl SrIovVirtualFunction {
    /// Create a new virtual function
    pub fn new(vf_id: u16, config: VfConfig) -> Self {
        Self {
            vf_id,
            config,
            stats: VfStats::default(),
            pci_config: SrIovPciConfig::default(),
            initialized: false,
            mmio_regions: HashMap::new(),
            interrupt_vectors: Vec::new(),
            rx_queues: HashMap::new(),
            tx_queues: HashMap::new(),
        }
    }

    /// Get PCI configuration
    pub fn pci_config(&self) -> &SrIovPciConfig {
        &self.pci_config
    }

    /// Set PCI configuration
    pub fn set_pci_config(&mut self, pci_config: SrIovPciConfig) {
        self.pci_config = pci_config;
    }

    /// Initialize the virtual function
    pub fn initialize(&mut self) -> VmResult<()> {
        // Initialize RX queues
        for queue_id in 0..self.config.num_rx_queues {
            self.rx_queues.insert(
                queue_id,
                VfQueue {
                    queue_id,
                    size: 256,
                    enabled: false,
                    desc_addr: GuestAddr(0),
                    desc_size: 16,
                    avail_addr: GuestAddr(0),
                    used_addr: GuestAddr(0),
                    stats: VfQueueStats::default(),
                },
            );
        }

        // Initialize TX queues
        for queue_id in 0..self.config.num_tx_queues {
            self.tx_queues.insert(
                queue_id,
                VfQueue {
                    queue_id,
                    size: 256,
                    enabled: false,
                    desc_addr: GuestAddr(0),
                    desc_size: 16,
                    avail_addr: GuestAddr(0),
                    used_addr: GuestAddr(0),
                    stats: VfQueueStats::default(),
                },
            );
        }

        // Initialize interrupt vectors
        self.interrupt_vectors = vec![false; 32];

        self.initialized = true;
        Ok(())
    }

    /// Get VF ID
    pub fn vf_id(&self) -> u16 {
        self.vf_id
    }

    /// Get VF configuration
    pub fn config(&self) -> &VfConfig {
        &self.config
    }

    /// Get VF statistics
    pub fn stats(&self) -> &VfStats {
        &self.stats
    }

    /// Check if VF is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get RX queue
    pub fn get_rx_queue(&self, queue_id: u16) -> Option<&VfQueue> {
        self.rx_queues.get(&queue_id)
    }

    /// Get TX queue
    pub fn get_tx_queue(&self, queue_id: u16) -> Option<&VfQueue> {
        self.tx_queues.get(&queue_id)
    }

    /// Enable RX queue
    pub fn enable_rx_queue(&mut self, queue_id: u16) -> VmResult<()> {
        if let Some(queue) = self.rx_queues.get_mut(&queue_id) {
            queue.enabled = true;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "queue_id".to_string(),
                    value: queue_id.to_string(),
                    message: format!("RX queue {} not found", queue_id),
                },
            ))
        }
    }

    /// Enable TX queue
    pub fn enable_tx_queue(&mut self, queue_id: u16) -> VmResult<()> {
        if let Some(queue) = self.tx_queues.get_mut(&queue_id) {
            queue.enabled = true;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "queue_id".to_string(),
                    value: queue_id.to_string(),
                    message: format!("TX queue {} not found", queue_id),
                },
            ))
        }
    }

    /// Disable RX queue
    pub fn disable_rx_queue(&mut self, queue_id: u16) -> VmResult<()> {
        if let Some(queue) = self.rx_queues.get_mut(&queue_id) {
            queue.enabled = false;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "queue_id".to_string(),
                    value: queue_id.to_string(),
                    message: format!("RX queue {} not found", queue_id),
                },
            ))
        }
    }

    /// Disable TX queue
    pub fn disable_tx_queue(&mut self, queue_id: u16) -> VmResult<()> {
        if let Some(queue) = self.tx_queues.get_mut(&queue_id) {
            queue.enabled = false;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "queue_id".to_string(),
                    value: queue_id.to_string(),
                    message: format!("TX queue {} not found", queue_id),
                },
            ))
        }
    }

    /// Set queue descriptor address
    pub fn set_queue_desc_addr(
        &mut self,
        queue_id: u16,
        is_tx: bool,
        addr: GuestAddr,
    ) -> VmResult<()> {
        let queues = if is_tx {
            &mut self.tx_queues
        } else {
            &mut self.rx_queues
        };
        if let Some(queue) = queues.get_mut(&queue_id) {
            queue.desc_addr = addr;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "queue_id".to_string(),
                    value: queue_id.to_string(),
                    message: format!("Queue {} not found", queue_id),
                },
            ))
        }
    }

    /// Set queue available ring address
    pub fn set_queue_avail_addr(
        &mut self,
        queue_id: u16,
        is_tx: bool,
        addr: GuestAddr,
    ) -> VmResult<()> {
        let queues = if is_tx {
            &mut self.tx_queues
        } else {
            &mut self.rx_queues
        };
        if let Some(queue) = queues.get_mut(&queue_id) {
            queue.avail_addr = addr;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "queue_id".to_string(),
                    value: queue_id.to_string(),
                    message: format!("Queue {} not found", queue_id),
                },
            ))
        }
    }

    /// Set queue used ring address
    pub fn set_queue_used_addr(
        &mut self,
        queue_id: u16,
        is_tx: bool,
        addr: GuestAddr,
    ) -> VmResult<()> {
        let queues = if is_tx {
            &mut self.tx_queues
        } else {
            &mut self.rx_queues
        };
        if let Some(queue) = queues.get_mut(&queue_id) {
            queue.used_addr = addr;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "queue_id".to_string(),
                    value: queue_id.to_string(),
                    message: format!("Queue {} not found", queue_id),
                },
            ))
        }
    }

    /// Trigger interrupt
    pub fn trigger_interrupt(&mut self, vector: u16) -> VmResult<()> {
        if vector < self.interrupt_vectors.len() as u16 {
            self.interrupt_vectors[vector as usize] = true;
            self.stats.interrupts.fetch_add(1, Ordering::Relaxed);
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vector".to_string(),
                    value: vector.to_string(),
                    message: format!("Interrupt vector {} out of range", vector),
                },
            ))
        }
    }

    /// Clear interrupt
    pub fn clear_interrupt(&mut self, vector: u16) -> VmResult<()> {
        if vector < self.interrupt_vectors.len() as u16 {
            self.interrupt_vectors[vector as usize] = false;
            Ok(())
        } else {
            Err(vm_core::error::VmError::Core(
                vm_core::error::CoreError::InvalidParameter {
                    name: "vector".to_string(),
                    value: vector.to_string(),
                    message: format!("Interrupt vector {} out of range", vector),
                },
            ))
        }
    }

    /// Check if interrupt is pending
    pub fn is_interrupt_pending(&self, vector: u16) -> bool {
        if vector < self.interrupt_vectors.len() as u16 {
            self.interrupt_vectors[vector as usize]
        } else {
            false
        }
    }

    /// Add MMIO region
    pub fn add_mmio_region(&mut self, bar: u32, addr: GuestAddr, size: u64) {
        self.mmio_regions.insert(bar, (addr, size));
    }

    /// Remove MMIO region
    pub fn remove_mmio_region(&mut self, bar: u32) {
        self.mmio_regions.remove(&bar);
    }

    /// Get MMIO region
    pub fn get_mmio_region(&self, bar: u32) -> Option<(GuestAddr, u64)> {
        self.mmio_regions.get(&bar).copied()
    }

    /// Get all MMIO regions
    pub fn get_all_mmio_regions(&self) -> &HashMap<u32, (GuestAddr, u64)> {
        &self.mmio_regions
    }
}
