//! GC statistics and profiling

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// GC statistics
///
/// Tracks various metrics about garbage collection performance
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GcStats {
    /// Total number of collections
    pub collections: u64,

    /// Total time spent in GC (across all collections)
    pub total_collection_time: Duration,

    /// Total bytes allocated
    pub total_allocated: u64,

    /// Total bytes freed
    pub total_freed: u64,

    /// Current heap size
    pub current_heap_size: usize,

    /// Peak heap size
    pub peak_heap_size: usize,

    /// Number of collection cycles
    pub cycles: u64,

    /// Bytes collected in last cycle
    pub last_collected: u64,
}

impl GcStats {
    /// Create new statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a collection cycle
    pub fn record_collection(&mut self, duration: Duration, bytes_collected: u64) {
        self.collections += 1;
        self.cycles += 1;
        self.total_collection_time += duration;
        self.last_collected = bytes_collected;
        self.total_freed += bytes_collected;
    }

    /// Record allocation
    pub fn record_allocation(&mut self, bytes: u64) {
        self.total_allocated += bytes;
        self.current_heap_size += bytes as usize;
        if self.current_heap_size > self.peak_heap_size {
            self.peak_heap_size = self.current_heap_size;
        }
    }

    /// Get average collection time
    pub fn avg_collection_time(&self) -> Duration {
        if self.collections == 0 {
            return Duration::ZERO;
        }
        self.total_collection_time / self.collections as u32
    }

    /// Get allocation rate (bytes per second)
    pub fn allocation_rate(&self) -> f64 {
        let total_time = self.total_collection_time.as_secs_f64();
        if total_time == 0.0 {
            return 0.0;
        }
        self.total_allocated as f64 / total_time
    }

    /// Get collection rate (bytes per second)
    pub fn collection_rate(&self) -> f64 {
        let total_time = self.total_collection_time.as_secs_f64();
        if total_time == 0.0 {
            return 0.0;
        }
        self.total_freed as f64 / total_time
    }

    /// Get heap utilization ratio (0.0 - 1.0)
    pub fn heap_utilization(&self) -> f64 {
        if self.peak_heap_size == 0 {
            return 0.0;
        }
        self.current_heap_size as f64 / self.peak_heap_size as f64
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_stats_default() {
        let stats = GcStats::default();
        assert_eq!(stats.collections, 0);
        assert_eq!(stats.total_allocated, 0);
    }

    #[test]
    fn test_record_collection() {
        let mut stats = GcStats::default();
        stats.record_collection(Duration::from_millis(100), 1024);

        assert_eq!(stats.collections, 1);
        assert_eq!(stats.total_freed, 1024);
        assert_eq!(stats.last_collected, 1024);
    }

    #[test]
    fn test_record_allocation() {
        let mut stats = GcStats::default();
        stats.record_allocation(2048);

        assert_eq!(stats.total_allocated, 2048);
        assert_eq!(stats.current_heap_size, 2048);
        assert_eq!(stats.peak_heap_size, 2048);
    }

    #[test]
    fn test_avg_collection_time() {
        let mut stats = GcStats::default();
        stats.record_collection(Duration::from_millis(100), 0);
        stats.record_collection(Duration::from_millis(200), 0);

        assert_eq!(stats.avg_collection_time(), Duration::from_millis(150));
    }

    #[test]
    fn test_heap_utilization() {
        let mut stats = GcStats::default();
        stats.record_allocation(512);
        stats.record_allocation(512); // current: 1024, peak: 1024

        assert_eq!(stats.heap_utilization(), 1.0);

        stats.current_heap_size = 512; // simulate some collection
        assert_eq!(stats.heap_utilization(), 0.5);
    }
}
