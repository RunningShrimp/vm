//! Asynchronous TLB implementation module.

#![cfg(feature = "async")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::{AccessType, TlbEntry, VmError};

/// Trait defining the asynchronous interface for a Translation Lookaside Buffer (TLB).
///
/// This trait provides a unified interface for all TLB implementations across different
/// architectures, allowing them to be used interchangeably in the virtual machine.
#[async_trait]
pub trait AsyncTranslationLookasideBuffer {
    /// Asynchronously translate a virtual address to a physical address.
    ///
    /// # Arguments
    /// - `va`: The virtual address to translate.
    /// - `access`: The type of access (read, write, execute).
    ///
    /// # Returns
    /// - `Ok(paddr)`: The translated physical address if the translation was successful.
    /// - `Err(VmError)`: An error if the translation failed (e.g., page fault).
    async fn translate(&mut self, va: u64, access: AccessType) -> Result<u64, VmError>;

    /// Asynchronously update the TLB with a new translation entry.
    ///
    /// # Arguments
    /// - `va`: The virtual address to associate with the entry.
    /// - `entry`: The TLB entry containing the physical address and other metadata.
    ///
    /// # Returns
    /// - `Ok(())`: If the entry was successfully added to the TLB.
    /// - `Err(VmError)`: An error if the entry could not be added.
    async fn update(&mut self, va: u64, entry: TlbEntry) -> Result<(), VmError>;

    /// Asynchronously flush a specific TLB entry by virtual address.
    ///
    /// # Arguments
    /// - `va`: The virtual address of the entry to flush.
    ///
    /// # Returns
    /// - `Ok(())`: If the entry was successfully flushed.
    /// - `Err(VmError)`: An error if the entry could not be flushed.
    async fn flush(&mut self, va: u64) -> Result<(), VmError>;

    /// Asynchronously flush all TLB entries.
    ///
    /// # Returns
    /// - `Ok(())`: If all entries were successfully flushed.
    /// - `Err(VmError)`: An error if the TLB could not be flushed.
    async fn flush_all(&mut self) -> Result<(), VmError>;

    /// Asynchronously flush TLB entries by ASID.
    ///
    /// # Arguments
    /// - `asid`: The Address Space Identifier (ASID) of the entries to flush.
    ///
    /// # Returns
    /// - `Ok(())`: If the entries were successfully flushed.
    /// - `Err(VmError)`: An error if the entries could not be flushed.
    async fn flush_asid(&mut self, asid: u16) -> Result<(), VmError>;

    /// Asynchronously flush TLB entries by virtual page range.
    ///
    /// # Arguments
    /// - `start_va`: The start of the virtual address range to flush.
    /// - `end_va`: The end of the virtual address range to flush.
    ///
    /// # Returns
    /// - `Ok(())`: If the entries were successfully flushed.
    /// - `Err(VmError)`: An error if the entries could not be flushed.
    async fn flush_range(&mut self, start_va: u64, end_va: u64) -> Result<(), VmError>;
}

/// A virtual page key used for TLB lookups.
///
/// This combines the virtual address and ASID to form a unique key for TLB entries.
#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub struct VirtPageKey {
    pub va: u64,
    pub asid: u16,
}

impl VirtPageKey {
    /// Create a new VirtPageKey.
    ///
    /// # Arguments
    /// - `va`: The virtual address (page-aligned).
    /// - `asid`: The Address Space Identifier.
    pub fn new(va: u64, asid: u16) -> Self {
        // Ensure the address is page-aligned.
        let page_aligned_va = va & !(0x1000 - 1);
        Self {
            va: page_aligned_va,
            asid,
        }
    }
}

impl From<u64> for VirtPageKey {
    /// Create a VirtPageKey from a virtual address with default ASID 0.
    ///
    /// # Arguments
    /// - `va`: The virtual address.
    fn from(va: u64) -> Self {
        Self::new(va, 0)
    }
}

/// A simple asynchronous TLB implementation.
pub struct AsyncSimpleTlb {
    entries: Arc<Mutex<HashMap<VirtPageKey, TlbEntry>>>,
}

impl AsyncSimpleTlb {
    /// Create a new simple asynchronous TLB.
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Acquire the lock with proper error handling.
    ///
    /// Returns a guard for the entries HashMap, or an error if the lock is poisoned.
    fn lock_entries(&self) -> Result<std::sync::MutexGuard<HashMap<VirtPageKey, TlbEntry>>, VmError> {
        self.entries.lock().map_err(|_| VmError::Memory(crate::MemoryError::PageTableError {
            message: "TLB lock is poisoned".to_string(),
            level: None,
        }))
    }
}

#[async_trait]
impl AsyncTranslationLookasideBuffer for AsyncSimpleTlb {
    async fn translate(&mut self, va: u64, access: AccessType) -> Result<u64, VmError> {
        let entry = self
            .lock_entries()?
            .get(&VirtPageKey::from(va))
            .cloned();

        match entry {
            Some(entry) => {
                // Check access permissions.
                // Note: For simplicity, we'll assume all entries have full permissions.
                // In a real implementation, you'd check the flags field.

                // Calculate the physical address.
                let offset = va - (va & !(0x1000 - 1));
                Ok(entry.phys_addr + offset)
            }
            None => {
                // Return a page not found error.
                Err(crate::VmError::Memory(crate::MemoryError::PageTableError {
                    message: "Page not found in TLB".to_string(),
                    level: None,
                }))
            }
        }
    }

    async fn update(&mut self, va: u64, entry: TlbEntry) -> Result<(), VmError> {
        let key = VirtPageKey::new(entry.guest_addr, entry.asid);
        self.lock_entries()?.insert(key, entry);
        Ok(())
    }

    async fn flush(&mut self, va: u64) -> Result<(), VmError> {
        self.lock_entries()?.remove(&VirtPageKey::from(va));
        Ok(())
    }

    async fn flush_all(&mut self) -> Result<(), VmError> {
        self.lock_entries()?.clear();
        Ok(())
    }

    async fn flush_asid(&mut self, asid: u16) -> Result<(), VmError> {
        self.lock_entries()?
            .retain(|key, _| key.asid != asid);
        Ok(())
    }

    async fn flush_range(&mut self, start_va: u64, end_va: u64) -> Result<(), VmError> {
        let start_key = VirtPageKey::from(start_va);
        let end_key = VirtPageKey::from(end_va);

        self.lock_entries()?
            .retain(|key, _| key.va < start_key.va || key.va > end_key.va);

        Ok(())
    }
}

/// Architecture-specific TLB implementations can be added here.
/// For example:
/// - X86_64AsyncTlb
/// - Arm64AsyncTlb
/// - Riscv64AsyncTlb

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AccessType;

    #[tokio::test]
    async fn test_async_simple_tlb_translate() {
        // Create a new asynchronous TLB.
        let mut tlb = AsyncSimpleTlb::new();

        // Define a test translation entry.
        let va = 0x12345000;
        let entry = TlbEntry {
            guest_addr: va,
            phys_addr: 0x56789000,
            flags: 0x00000003, // Read/Write permissions.
            asid: 0,
        };

        // Update the TLB with the entry.
        tlb.update(va, entry).await.expect("Failed to update TLB");

        // Translate the virtual address.
        let pa = tlb.translate(va, AccessType::Read).await.expect("Failed to translate address");

        // Verify the translation result.
        assert_eq!(pa, 0x56789000);
    }

    #[tokio::test]
    async fn test_async_simple_tlb_flush() {
        // Create a new asynchronous TLB.
        let mut tlb = AsyncSimpleTlb::new();

        // Define a test translation entry.
        let va = 0x12345000;
        let entry = TlbEntry {
            guest_addr: va,
            phys_addr: 0x56789000,
            flags: 0x00000003, // Read/Write permissions.
            asid: 0,
        };

        // Update the TLB with the entry.
        tlb.update(va, entry).await.expect("Failed to update TLB");

        // Flush the entry.
        tlb.flush(va).await.expect("Failed to flush TLB entry");

        // Attempt to translate the virtual address.
        let result = tlb.translate(va, AccessType::Read).await;

        // Verify that the translation fails.
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_async_simple_tlb_flush_all() {
        // Create a new asynchronous TLB.
        let mut tlb = AsyncSimpleTlb::new();

        // Define multiple test translation entries.
        let entries = vec![
            (0x12345000, 0x56789000, 0),
            (0x23456000, 0x67890000, 0),
            (0x34567000, 0x78901000, 1),
        ];

        // Update the TLB with the entries.
        for (va, pa, asid) in entries {
            let entry = TlbEntry {
                guest_addr: va,
                phys_addr: pa,
                flags: 0x00000003, // Read/Write permissions.
                asid,
            };
            tlb.update(va, entry).await.expect("Failed to update TLB");
        }

        // Flush all entries.
        tlb.flush_all().await.expect("Failed to flush all TLB entries");

        // Attempt to translate the virtual addresses.
        for (va, _, _) in entries {
            let result = tlb.translate(va, AccessType::Read).await;
            // Verify that the translation fails.
            assert!(result.is_err());
        }
    }
}
