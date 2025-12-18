//! Snapshot module
//!
//! This module provides comprehensive snapshot management for the VM system,
//! including metadata management, data persistence, and snapshot-based replay optimization.

pub mod enhanced_snapshot;

// Re-export the original snapshot types for backward compatibility
pub use crate::snapshot_legacy::{
    Snapshot, SnapshotMetadataManager
};

// Re-export enhanced snapshot functionality
#[cfg(feature = "enhanced-event-sourcing")]
pub use enhanced_snapshot::{
    SnapshotStore, FileSnapshotStore, SnapshotManager, SnapshotConfig,
    SnapshotData, SnapshotMetadata, SnapshotStats, SnapshotStoreBuilder
};

// SnapshotConfig is exported conditionally with other enhanced snapshot functionality