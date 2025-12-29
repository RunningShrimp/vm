//! SMMU (IOMMU) device management module
//!
//! Consolidates all SMMU-related functionality under a single "smmu" feature flag.
//! This includes device attachment, DMA translation, and statistics tracking.

use std::sync::Arc;

use vm_core::{GuestAddr, VmError, VmResult};

#[cfg(feature = "smmu")]
use vm_accel::SmmuManager;
#[cfg(feature = "smmu")]
use vm_device::smmu_device::SmmuDeviceManager;

/// SMMU device management context
///
/// Holds all SMMU-related state including the manager and device manager.
/// This structure is only available when the "smmu" feature is enabled.
#[cfg(feature = "smmu")]
pub struct SmmuContext {
    /// SMMU manager for DMA translation
    pub smmu_manager: Option<Arc<SmmuManager>>,
    /// SMMU device manager for device attachment
    pub smmu_device_manager: Option<Arc<SmmuDeviceManager>>,
}

#[cfg(feature = "smmu")]
impl SmmuContext {
    /// Create a new SMMU context
    pub fn new() -> Self {
        Self {
            smmu_manager: None,
            smmu_device_manager: None,
        }
    }

    /// Initialize SMMU support
    ///
    /// Creates and initializes SMMU manager and device manager.
    pub fn init(&mut self) -> VmResult<()> {
        log::info!("Initializing SMMU support");

        // Create SMMU manager
        let smmu_manager = Arc::new(SmmuManager::new());
        smmu_manager.init().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to initialize SMMU: {:?}", e),
                module: "vm-service".to_string(),
            })
        })?;

        // Create SMMU device manager
        let smmu_device_manager = Arc::new(SmmuDeviceManager::new(smmu_manager.clone()));

        self.smmu_manager = Some(smmu_manager);
        self.smmu_device_manager = Some(smmu_device_manager);

        log::info!("SMMU support initialized successfully");
        Ok(())
    }

    /// Attach device to SMMU
    ///
    /// Allocates SMMU Stream ID for PCIe device and configures DMA address space.
    ///
    /// # Arguments
    ///
    /// * `bdf` - PCIe BDF identifier (format: "BBBB:DD:F.F")
    /// * `dma_start` - DMA address space start address
    /// * `dma_size` - DMA address space size
    ///
    /// # Returns
    ///
    /// Returns allocated Stream ID on success, error on failure.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let stream_id = smmu_context.attach_device("0000:01:00.0", 0x1000, 0x10000)?;
    /// ```
    pub fn attach_device(&self, bdf: &str, dma_start: u64, dma_size: u64) -> VmResult<u16> {
        log::info!(
            "Attaching device {} to SMMU (DMA range: 0x{:x}-0x{:x})",
            bdf,
            dma_start,
            dma_start + dma_size
        );

        let device_manager = self.smmu_device_manager.as_ref().ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: "SMMU not initialized. Call init() first.".to_string(),
                current: "not_initialized".to_string(),
                expected: "initialized".to_string(),
            })
        })?;

        let stream_id = device_manager
            .assign_device(bdf, dma_start, dma_size)
            .map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("Failed to assign device to SMMU: {}", e),
                    module: "vm-service".to_string(),
                })
            })?;

        log::info!(
            "Device {} attached to SMMU with stream ID {}",
            bdf,
            stream_id
        );
        Ok(stream_id)
    }

    /// Detach device from SMMU
    ///
    /// # Arguments
    ///
    /// * `bdf` - PCIe BDF identifier
    pub fn detach_device(&self, bdf: &str) -> VmResult<()> {
        log::info!("Detaching device {} from SMMU", bdf);

        let device_manager = self.smmu_device_manager.as_ref().ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: "SMMU not initialized. Call init() first.".to_string(),
                current: "not_initialized".to_string(),
                expected: "initialized".to_string(),
            })
        })?;

        device_manager.unassign_device(bdf).map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to detach device from SMMU: {:?}", e),
                module: "vm-service".to_string(),
            })
        })?;

        log::info!("Device {} detached from SMMU", bdf);
        Ok(())
    }

    /// Translate device DMA address
    ///
    /// # Arguments
    ///
    /// * `bdf` - PCIe BDF identifier
    /// * `guest_addr` - Guest physical address
    /// * `size` - Access size
    ///
    /// # Returns
    ///
    /// Returns translated physical address on success, error on failure.
    pub fn translate_dma(&self, bdf: &str, guest_addr: GuestAddr, size: u64) -> VmResult<u64> {
        log::trace!(
            "Translating DMA addr for device {}: GPA 0x{:x}, size {}",
            bdf,
            guest_addr.0,
            size
        );

        let smmu_manager = self.smmu_manager.as_ref().ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: "SMMU not initialized. Call init() first.".to_string(),
                current: "not_initialized".to_string(),
                expected: "initialized".to_string(),
            })
        })?;

        let device_id = format!("pci-{}", bdf);
        let translated = smmu_manager
            .translate_dma_addr(&device_id, guest_addr, size)
            .map_err(|e| {
                VmError::Core(vm_core::CoreError::Internal {
                    message: format!("DMA translation failed: {:?}", e),
                    module: "vm-service".to_string(),
                })
            })?;

        log::trace!(
            "DMA addr translated: GPA 0x{:x} -> 0x{:x}",
            guest_addr.0,
            translated
        );
        Ok(translated)
    }

    /// List all devices attached to SMMU
    ///
    /// # Returns
    ///
    /// Returns list of device BDF identifiers.
    pub fn list_devices(&self) -> VmResult<Vec<String>> {
        let device_manager = self.smmu_device_manager.as_ref().ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: "SMMU not initialized. Call init() first.".to_string(),
                current: "not_initialized".to_string(),
                expected: "initialized".to_string(),
            })
        })?;

        let devices = device_manager.list_devices();
        Ok(devices)
    }

    /// Get SMMU statistics
    ///
    /// # Returns
    ///
    /// Returns SMMU statistics including translation count, cache hit rate, etc.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let stats = smmu_context.get_stats()?;
    /// println!("Total translations: {}", stats.total_translations);
    /// ```
    pub fn get_stats(&self) -> VmResult<vm_smmu::SmmuStats> {
        let smmu_manager = self.smmu_manager.as_ref().ok_or_else(|| {
            VmError::Core(vm_core::CoreError::InvalidState {
                message: "SMMU not initialized. Call init() first.".to_string(),
                current: "not_initialized".to_string(),
                expected: "initialized".to_string(),
            })
        })?;

        smmu_manager.get_stats().map_err(|e| {
            VmError::Core(vm_core::CoreError::Internal {
                message: format!("Failed to get SMMU stats: {:?}", e),
                module: "vm-service".to_string(),
            })
        })
    }
}

#[cfg(feature = "smmu")]
impl Default for SmmuContext {
    fn default() -> Self {
        Self::new()
    }
}
