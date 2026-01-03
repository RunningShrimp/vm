//! TLB Management Domain Service
//!
//! This service encapsulates business logic for Translation Lookaside Buffer (TLB) management
//! including TLB invalidation strategies, multi-level TLB coordination, ASID-based management,
//! and TLB statistics tracking.
//!
//! **DDD Architecture**:
//! - Uses `TlbManager` trait from domain layer (dependency inversion)
//! - Delegates TLB operations to infrastructure layer implementation
//! - Focuses on business logic: event publishing, coordination, statistics aggregation
//!
//! **Migration Status**:
//! - Infrastructure implementation created: ✅ (vm-mem/src/tlb/management/multilevel.rs)
//! - Domain service refactored to use trait: ✅ (Refactored to use TlbManager trait)

use std::sync::Arc;

use crate::domain_services::events::{DomainEventEnum, TlbEvent};
use crate::domain_event_bus::DomainEventBus;
use crate::domain::TlbManager;
use crate::{AccessType, GuestAddr, TlbEntry, VmResult};

/// TLB level (ITLB, DTLB, L2 TLB, etc.)
/// Used for event publishing and coordination
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TlbLevel {
    /// Instruction TLB (L1)
    ITlb,
    /// Data TLB (L1)
    DTlb,
    /// Unified L2 TLB
    L2Tlb,
    /// L3 TLB (if present)
    L3Tlb,
}

impl TlbLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            TlbLevel::ITlb => "ITLB",
            TlbLevel::DTlb => "DTLB",
            TlbLevel::L2Tlb => "L2TLB",
            TlbLevel::L3Tlb => "L3TLB",
        }
    }
}

/// TLB Management Domain Service
///
/// This service provides high-level business logic for managing Translation Lookaside Buffers
/// across different levels (ITLB, DTLB, L2TLB, etc.) with coordinated invalidation
/// and statistics tracking.
///
/// **Refactored Architecture**:
/// - Uses `TlbManager` trait from domain layer (dependency inversion)
/// - Delegates TLB operations to infrastructure layer implementation
/// - Focuses on business logic: event publishing, coordination, statistics aggregation
pub struct TlbManagementDomainService {
    /// Event bus for publishing TLB events
    event_bus: Arc<DomainEventBus>,
    /// TLB manager (infrastructure layer implementation via trait)
    tlb_manager: Arc<std::sync::Mutex<dyn TlbManager>>,
}

impl TlbManagementDomainService {
    /// Create a new TLB management domain service
    ///
    /// # 参数
    /// - `event_bus`: Event bus for publishing domain events
    /// - `tlb_manager`: TLB manager implementation (from infrastructure layer)
    pub fn new(
        event_bus: Arc<DomainEventBus>,
        tlb_manager: Arc<std::sync::Mutex<dyn TlbManager>>,
    ) -> Self {
        Self {
            event_bus,
            tlb_manager,
        }
    }

    /// Lookup TLB entry
    ///
    /// Delegates to the infrastructure layer implementation.
    pub async fn lookup(&mut self, addr: GuestAddr, asid: u16, access: AccessType) -> Option<TlbEntry> {
        let mut manager = self.tlb_manager.lock().unwrap();
        manager.lookup(addr, asid, access)
    }

    /// Insert or update TLB entry
    ///
    /// Delegates to the infrastructure layer implementation.
    pub async fn update(&mut self, entry: TlbEntry) -> VmResult<()> {
        let mut manager = self.tlb_manager.lock().unwrap();
        manager.update(entry);

        // Publish domain event
        self.publish_event(TlbEvent::EntryInserted {
            level: TlbLevel::L2Tlb, // Default level, actual level from infrastructure
            va: entry.guest_addr.0,
            asid: entry.asid,
        });

        Ok(())
    }

    /// Flush all TLB entries
    ///
    /// Delegates to the infrastructure layer implementation.
    pub async fn flush(&mut self) -> VmResult<()> {
        let mut manager = self.tlb_manager.lock().unwrap();
        manager.flush();

        // Publish domain event
        self.publish_event(TlbEvent::FlushAll { level: TlbLevel::L2Tlb });

        Ok(())
    }

    /// Flush entries for a specific ASID
    ///
    /// Delegates to the infrastructure layer implementation.
    pub async fn flush_asid(&mut self, asid: u16) -> VmResult<()> {
        let mut manager = self.tlb_manager.lock().unwrap();
        manager.flush_asid(asid);

        // Publish domain event
        self.publish_event(TlbEvent::FlushAsid { asid });

        Ok(())
    }

    /// Flush a specific page
    ///
    /// Delegates to the infrastructure layer implementation.
    pub async fn flush_page(&mut self, va: GuestAddr) -> VmResult<()> {
        let mut manager = self.tlb_manager.lock().unwrap();
        manager.flush_page(va);

        // Publish domain event
        self.publish_event(TlbEvent::EntryFlushed {
            level: TlbLevel::L2Tlb, // Default level
            va: va.0,
            asid: 0, // ASID not available at this level
        });

        Ok(())
    }

    /// Get TLB statistics
    ///
    /// Delegates to the infrastructure layer implementation.
    pub async fn get_statistics(&self) -> Option<crate::domain::TlbStats> {
        let manager = self.tlb_manager.lock().unwrap();
        manager.get_stats()
    }

    /// Publish domain event
    fn publish_event(&self, event: TlbEvent) {
        let _ = self.event_bus.publish(&DomainEventEnum::Tlb(event));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tlb_level_conversions() {
        assert_eq!(TlbLevel::ITlb.as_str(), "ITLB");
        assert_eq!(TlbLevel::DTlb.as_str(), "DTLB");
        assert_eq!(TlbLevel::L2Tlb.as_str(), "L2TLB");
        assert_eq!(TlbLevel::L3Tlb.as_str(), "L3TLB");
    }
}
