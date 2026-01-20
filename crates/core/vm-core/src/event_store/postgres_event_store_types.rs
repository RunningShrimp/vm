//! PostgreSQL event store types
//!
//! This module defines custom types and enums used by the PostgreSQL event store,
//! including event status, metadata, and query parameters.

use std::fmt;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Event status in the store
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventStatus {
    /// Event is active and visible
    Active,
    /// Event is archived (soft delete)
    Archived,
    /// Event is being processed
    Processing,
    /// Event processing completed
    Completed,
    /// Event has been rejected
    Rejected,
}

impl fmt::Display for EventStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventStatus::Active => write!(f, "active"),
            EventStatus::Archived => write!(f, "archived"),
            EventStatus::Processing => write!(f, "processing"),
            EventStatus::Completed => write!(f, "completed"),
            EventStatus::Rejected => write!(f, "rejected"),
        }
    }
}

impl std::str::FromStr for EventStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(EventStatus::Active),
            "archived" => Ok(EventStatus::Archived),
            "processing" => Ok(EventStatus::Processing),
            "completed" => Ok(EventStatus::Completed),
            "rejected" => Ok(EventStatus::Rejected),
            _ => Err(format!("Invalid event status: {}", s)),
        }
    }
}

/// Event metadata for PostgreSQL storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Event sequence number
    pub sequence_number: u64,
    /// Virtual machine ID
    pub vm_id: String,
    /// Event type name
    pub event_type: String,
    /// Event version
    pub event_version: i32,
    /// Event status
    pub status: EventStatus,
    /// Timestamp when event was created
    pub created_at: DateTime<Utc>,
    /// Timestamp when event was last updated
    pub updated_at: DateTime<Utc>,
    /// Event size in bytes
    pub event_size: u64,
    /// Compressed event size (if applicable)
    pub compressed_size: Option<u64>,
    /// Checksum of the event data
    pub checksum: Option<String>,
    /// Custom metadata as JSON
    pub custom_metadata: Option<serde_json::Value>,
}

impl EventMetadata {
    /// Create new event metadata
    pub fn new(
        sequence_number: u64,
        vm_id: String,
        event_type: String,
        event_version: i32,
    ) -> Self {
        let now = Utc::now();
        Self {
            sequence_number,
            vm_id,
            event_type,
            event_version,
            status: EventStatus::Active,
            created_at: now,
            updated_at: now,
            event_size: 0,
            compressed_size: None,
            checksum: None,
            custom_metadata: None,
        }
    }

    /// Set custom metadata
    pub fn with_custom_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.custom_metadata = Some(metadata);
        self
    }

    /// Update timestamp
    pub fn update_timestamp(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Mark as processing
    pub fn mark_processing(&mut self) {
        self.status = EventStatus::Processing;
        self.update_timestamp();
    }

    /// Mark as completed
    pub fn mark_completed(&mut self) {
        self.status = EventStatus::Completed;
        self.update_timestamp();
    }

    /// Mark as archived
    pub fn mark_archived(&mut self) {
        self.status = EventStatus::Archived;
        self.update_timestamp();
    }

    /// Mark as rejected
    pub fn mark_rejected(&mut self) {
        self.status = EventStatus::Rejected;
        self.update_timestamp();
    }
}

/// Query parameters for event filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventQueryParams {
    /// Virtual machine ID (required)
    pub vm_id: String,
    /// Starting sequence number (inclusive)
    pub from_sequence: Option<u64>,
    /// Ending sequence number (inclusive)
    pub to_sequence: Option<u64>,
    /// Event types to filter by
    pub event_types: Option<Vec<String>>,
    /// Event statuses to filter by
    pub statuses: Option<Vec<EventStatus>>,
    /// Start time filter
    pub created_after: Option<DateTime<Utc>>,
    /// End time filter
    pub created_before: Option<DateTime<Utc>>,
    /// Limit number of results
    pub limit: Option<u64>,
    /// Offset for pagination
    pub offset: Option<u64>,
    /// Sort order
    pub sort_order: SortOrder,
}

impl EventQueryParams {
    /// Create new query parameters
    pub fn new(vm_id: String) -> Self {
        Self {
            vm_id,
            from_sequence: None,
            to_sequence: None,
            event_types: None,
            statuses: None,
            created_after: None,
            created_before: None,
            limit: None,
            offset: None,
            sort_order: SortOrder::Asc,
        }
    }

    /// Set sequence range
    pub fn with_sequence_range(mut self, from: Option<u64>, to: Option<u64>) -> Self {
        self.from_sequence = from;
        self.to_sequence = to;
        self
    }

    /// Set event types filter
    pub fn with_event_types(mut self, event_types: Vec<String>) -> Self {
        self.event_types = Some(event_types);
        self
    }

    /// Set statuses filter
    pub fn with_statuses(mut self, statuses: Vec<EventStatus>) -> Self {
        self.statuses = Some(statuses);
        self
    }

    /// Set time range
    pub fn with_time_range(mut self, after: Option<DateTime<Utc>>, before: Option<DateTime<Utc>>) -> Self {
        self.created_after = after;
        self.created_before = before;
        self
    }

    /// Set pagination
    pub fn with_pagination(mut self, limit: Option<u64>, offset: Option<u64>) -> Self {
        self.limit = limit;
        self.offset = offset;
        self
    }

    /// Set sort order
    pub fn with_sort_order(mut self, order: SortOrder) -> Self {
        self.sort_order = order;
        self
    }
}

/// Sort order for query results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortOrder {
    /// Ascending order
    Asc,
    /// Descending order
    Desc,
}

impl Default for SortOrder {
    fn default() -> Self {
        SortOrder::Asc
    }
}

/// Event batch operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    /// Number of events processed
    pub processed_count: u64,
    /// Number of successful operations
    pub success_count: u64,
    /// Number of failed operations
    pub failure_count: u64,
    /// Processing duration
    pub duration_ms: u64,
    /// Error messages for failed operations
    pub errors: Vec<String>,
}

impl BatchResult {
    /// Create a new batch result
    pub fn new(processed_count: u64, success_count: u64, failure_count: u64, duration_ms: u64) -> Self {
        Self {
            processed_count,
            success_count,
            failure_count,
            duration_ms,
            errors: Vec::new(),
        }
    }

    /// Add an error message
    pub fn add_error(mut self, error: String) -> Self {
        self.errors.push(error);
        self
    }

    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.processed_count == 0 {
            0.0
        } else {
            self.success_count as f64 / self.processed_count as f64
        }
    }
}

/// Event store statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStoreStats {
    /// Total number of events
    pub total_events: u64,
    /// Number of active events
    pub active_events: u64,
    /// Number of archived events
    pub archived_events: u64,
    /// Number of VMs
    pub vm_count: u64,
    /// Total storage size in bytes
    pub total_storage_bytes: u64,
    /// Average event size
    pub average_event_size: u64,
    /// Oldest event timestamp
    pub oldest_event: Option<DateTime<Utc>>,
    /// Newest event timestamp
    pub newest_event: Option<DateTime<Utc>>,
    /// Number of events processed today
    pub events_today: u64,
    /// Number of events processed yesterday
    pub events_yesterday: u64,
}

impl EventStoreStats {
    /// Check if store is empty
    pub fn is_empty(&self) -> bool {
        self.total_events == 0
    }

    /// Calculate growth rate (vs yesterday)
    pub fn growth_rate(&self) -> Option<f64> {
        if self.events_yesterday == 0 {
            None
        } else {
            Some((self.events_today as f64 - self.events_yesterday as f64) / self.events_yesterday as f64)
        }
    }
}

/// Migration status for event store schema
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationStatus {
    /// No migration needed
    UpToDate,
    /// Migration pending
    Pending,
    /// Migration in progress
    InProgress,
    /// Migration completed successfully
    Completed,
    /// Migration failed
    Failed,
}

impl fmt::Display for MigrationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrationStatus::UpToDate => write!(f, "up_to_date"),
            MigrationStatus::Pending => write!(f, "pending"),
            MigrationStatus::InProgress => write!(f, "in_progress"),
            MigrationStatus::Completed => write!(f, "completed"),
            MigrationStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Migration information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationInfo {
    /// Migration status
    pub status: MigrationStatus,
    /// Current schema version
    pub current_version: i32,
    /// Target schema version
    pub target_version: i32,
    /// Migration message
    pub message: Option<String>,
    /// Last migration timestamp
    pub last_migrated_at: Option<DateTime<Utc>>,
}

impl MigrationInfo {
    /// Create new migration info
    pub fn new(current_version: i32, target_version: i32) -> Self {
        Self {
            status: MigrationStatus::Pending,
            current_version,
            target_version,
            message: None,
            last_migrated_at: None,
        }
    }

    /// Set migration in progress
    pub fn start_migration(&mut self) {
        self.status = MigrationStatus::InProgress;
        self.last_migrated_at = Some(Utc::now());
    }

    /// Mark migration as completed
    pub fn complete_migration(&mut self) {
        self.status = MigrationStatus::Completed;
        self.last_migrated_at = Some(Utc::now());
        self.message = Some("Migration completed successfully".to_string());
    }

    /// Mark migration as failed
    pub fn fail_migration(&mut self, message: String) {
        self.status = MigrationStatus::Failed;
        self.message = Some(message);
        self.last_migrated_at = Some(Utc::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_status_display() {
        assert_eq!(EventStatus::Active.to_string(), "active");
        assert_eq!(EventStatus::Archived.to_string(), "archived");
    }

    #[test]
    fn test_event_status_from_str() {
        assert_eq!("active".parse::<EventStatus>().unwrap(), EventStatus::Active);
        assert_eq!("archived".parse::<EventStatus>().unwrap(), EventStatus::Archived);
    }

    #[test]
    fn test_event_metadata() {
        let mut metadata = EventMetadata::new(1, "vm1", "TestEvent", 1);
        metadata.mark_processing();
        assert_eq!(metadata.status, EventStatus::Processing);
        metadata.mark_completed();
        assert_eq!(metadata.status, EventStatus::Completed);
    }

    #[test]
    fn test_event_query_params() {
        let params = EventQueryParams::new("vm1".to_string())
            .with_sequence_range(Some(1), Some(10))
            .with_event_types(vec!["TestEvent".to_string()])
            .with_sort_order(SortOrder::Desc);

        assert_eq!(params.vm_id, "vm1");
        assert_eq!(params.from_sequence, Some(1));
        assert_eq!(params.to_sequence, Some(10));
        assert_eq!(params.sort_order, SortOrder::Desc);
    }

    #[test]
    fn test_batch_result() {
        let result = BatchResult::new(10, 8, 2, 100);
        assert_eq!(result.success_count, 8);
        assert_eq!(result.failure_count, 2);
        assert_eq!(result.success_rate(), 0.8);
    }
}