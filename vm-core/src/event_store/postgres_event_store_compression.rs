//! PostgreSQL event store compression
//!
//! This module provides event data compression and decompression functionality
//! for the PostgreSQL event store to optimize storage usage.

use std::io::{Read, Write};
use std::collections::HashMap;
use zstd::stream::{Encoder, Decoder};
use miniz_oxide::deflate::compress_to_vec;
use miniz_oxide::inflate::decompress_to_vec;
use serde::{Serialize, Deserialize};
use tracing::{info, warn, error};
use super::postgres_event_store_types::{EventMetadata};

/// Compression manager for event data
pub struct CompressionManager {
    /// Compression configuration
    config: CompressionConfig,
    /// Compression statistics
    stats: CompressionStats,
    /// Compression method cache
    method_cache: HashMap<String, CompressionMethod>,
}

/// Compression configuration
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Default compression method
    pub default_method: CompressionMethod,
    /// Enable compression
    pub enabled: bool,
    /// Compression level (zstd: 1-22, gzip: 1-9)
    pub compression_level: i32,
    /// Minimum size to compress (bytes)
    pub min_size_threshold: usize,
    /// Maximum compression ratio (0.0-1.0)
    pub max_compression_ratio: f64,
    /// Enable compression statistics
    pub enable_stats: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            default_method: CompressionMethod::Zstd,
            enabled: true,
            compression_level: 3, // Balanced speed/compression
            min_size_threshold: 1024, // 1KB
            max_compression_ratio: 0.9, // 90% compression ratio
            enable_stats: true,
        }
    }
}

/// Compression methods
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionMethod {
    /// No compression
    None,
    /// Zstandard compression
    Zstd,
    /// Gzip compression
    Gzip,
    /// Brotli compression (future)
    Brotli,
}

impl CompressionMethod {
    /// Get compression method name
    pub fn name(&self) -> &'static str {
        match self {
            CompressionMethod::None => "none",
            CompressionMethod::Zstd => "zstd",
            CompressionMethod::Gzip => "gzip",
            CompressionMethod::Brotli => "brotli",
        }
    }
}

/// Compression statistics
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    /// Total events compressed
    pub events_compressed: u64,
    /// Total events decompressed
    pub events_decompressed: u64,
    /// Total bytes compressed
    pub bytes_compressed: u64,
    /// Total bytes decompressed
    pub bytes_decompressed: u64,
    /// Compression ratio savings
    pub compression_ratio_savings: f64,
    /// Average compression ratio
    pub average_compression_ratio: f64,
    /// Compression time (ms)
    pub total_compression_time_ms: u64,
    /// Decompression time (ms)
    pub total_decompression_time_ms: u64,
    /// Failed compressions
    pub failed_compressions: u64,
    /// Failed decompressions
    pub failed_decompressions: u64,
}

impl CompressionManager {
    /// Create a new compression manager
    pub fn new(config: CompressionConfig) -> Self {
        Self {
            config,
            stats: CompressionStats::default(),
            method_cache: HashMap::new(),
        }
    }

    /// Compress event data
    pub fn compress_event(
        &mut self,
        event_data: &[u8],
        method: Option<CompressionMethod>,
    ) -> Result<CompressedEvent, CompressionError> {
        if !self.config.enabled {
            return Ok(CompressedEvent {
                method: CompressionMethod::None,
                compressed_data: event_data.to_vec(),
                original_size: event_data.len(),
                compressed_size: event_data.len(),
                checksum: self.calculate_checksum(event_data),
            });
        }

        // Skip compression for small events
        if event_data.len() < self.config.min_size_threshold {
            info!("Skipping compression for small event ({} bytes)", event_data.len());
            return Ok(CompressedEvent {
                method: CompressionMethod::None,
                compressed_data: event_data.to_vec(),
                original_size: event_data.len(),
                compressed_size: event_data.len(),
                checksum: self.calculate_checksum(event_data),
            });
        }

        let compression_method = method.unwrap_or(self.config.default_method);
        let start_time = std::time::Instant::now();

        if compression_method == CompressionMethod::None {
            return Ok(CompressedEvent {
                method: CompressionMethod::None,
                compressed_data: event_data.to_vec(),
                original_size: event_data.len(),
                compressed_size: event_data.len(),
                checksum: self.calculate_checksum(event_data),
            });
        }

        let compressed_data = self.compress_data(event_data, compression_method)?;
        let compression_time = start_time.elapsed().as_millis() as u64;

        // Check compression ratio
        let compression_ratio = compressed_data.len() as f64 / event_data.len() as f64;
        if compression_ratio > self.config.max_compression_ratio {
            warn!(
                "Compression ratio {} too high for {} bytes event, skipping compression",
                compression_ratio, event_data.len()
            );

            return Ok(CompressedEvent {
                method: CompressionMethod::None,
                compressed_data: event_data.to_vec(),
                original_size: event_data.len(),
                compressed_size: event_data.len(),
                checksum: self.calculate_checksum(event_data),
            });
        }

        // Update statistics
        if self.config.enable_stats {
            self.stats.events_compressed += 1;
            self.stats.bytes_compressed += event_data.len() as u64;
            self.stats.bytes_decompressed += compressed_data.len() as u64;
            self.stats.total_compression_time_ms += compression_time;
            self.stats.compression_ratio_savings += 1.0 - compression_ratio;
            self.stats.average_compression_ratio =
                (self.stats.average_compression_ratio * (self.stats.events_compressed - 1) as f64 + compression_ratio) / self.stats.events_compressed as f64;
        }

        info!(
            "Compressed {} bytes to {} bytes (ratio: {:.2}%, method: {}, {}ms)",
            event_data.len(),
            compressed_data.len(),
            (1.0 - compression_ratio) * 100.0,
            compression_method.name(),
            compression_time
        );

        Ok(CompressedEvent {
            method: compression_method,
            compressed_data,
            original_size: event_data.len(),
            compressed_size: compressed_data.len(),
            checksum: self.calculate_checksum(event_data),
        })
    }

    /// Decompress event data
    pub fn decompress_event(
        &mut self,
        compressed_event: &CompressedEvent,
    ) -> Result<Vec<u8>, CompressionError> {
        if compressed_event.method == CompressionMethod::None {
            return Ok(compressed_event.compressed_data.clone());
        }

        let start_time = std::time::Instant::now();

        let original_data = self.decompress_data(&compressed_event.compressed_data, compressed_event.method)?;
        let decompression_time = start_time.elapsed().as_millis() as u64;

        // Verify checksum
        let calculated_checksum = self.calculate_checksum(&original_data);
        if calculated_checksum != compressed_event.checksum {
            error!("Checksum mismatch during decompression");
            return Err(CompressionError::ChecksumMismatch);
        }

        // Verify size
        if original_data.len() != compressed_event.original_size {
            error!("Size mismatch during decompression");
            return Err(CompressionError::SizeMismatch);
        }

        // Update statistics
        if self.config.enable_stats {
            self.stats.events_decompressed += 1;
            self.stats.bytes_compressed += compressed_event.compressed_data.len() as u64;
            self.stats.bytes_decompressed += compressed_event.original_size as u64;
            self.stats.total_decompression_time_ms += decompression_time;
        }

        info!(
            "Decompressed {} bytes to {} bytes (method: {}, {}ms)",
            compressed_event.compressed_data.len(),
            compressed_event.original_size,
            compressed_event.method.name(),
            decompression_time
        );

        Ok(original_data)
    }

    /// Compress data using specified method
    fn compress_data(&self, data: &[u8], method: CompressionMethod) -> Result<Vec<u8>, CompressionError> {
        match method {
            CompressionMethod::Zstd => {
                let encoder = Encoder::new(data, self.config.compression_level)
                    .map_err(|e| CompressionError::CompressionFailed(format!("Zstd encoder: {}", e)))?;
                let mut compressed = Vec::new();
                encoder.write_all(data)
                    .map_err(|e| CompressionError::CompressionFailed(format!("Zstd write: {}", e)))?;
                encoder.finish()
                    .map_err(|e| CompressionError::CompressionFailed(format!("Zstd finish: {}", e)))?
            }
            CompressionMethod::Gzip => {
                compress_to_vec(data, self.config.compression_level.max(1).min(9))
            }
            CompressionMethod::None | CompressionMethod::Brotli => {
                data.to_vec()
            }
        }
        .map_err(|e| CompressionError::CompressionFailed(e.to_string()))
    }

    /// Decompress data using specified method
    fn decompress_data(&self, data: &[u8], method: CompressionMethod) -> Result<Vec<u8>, CompressionError> {
        match method {
            CompressionMethod::Zstd => {
                let mut decoder = Decoder::new(data)
                    .map_err(|e| CompressionError::DecompressionFailed(format!("Zstd decoder: {}", e)))?;
                let mut decompressed = Vec::new();
                decoder.read_to_end(&mut decompressed)
                    .map_err(|e| CompressionError::DecompressionFailed(format!("Zstd read: {}", e)))?;
                Ok(decompressed)
            }
            CompressionMethod::Gzip => {
                decompress_to_vec(data)
                    .map_err(|e| CompressionError::DecompressionFailed(e.to_string()))
            }
            CompressionMethod::None | CompressionMethod::Brotli => {
                Ok(data.to_vec())
            }
        }
    }

    /// Calculate checksum (simple CRC32)
    fn calculate_checksum(&self, data: &[u8]) -> String {
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(data);
        format!("{:08x}", hasher.finalize())
    }

    /// Get compression statistics
    pub fn get_stats(&self) -> CompressionStats {
        self.stats.clone()
    }

    /// Reset compression statistics
    pub fn reset_stats(&mut self) {
        self.stats = CompressionStats::default();
    }

    /// Check if compression is beneficial
    pub fn is_compression_beneficial(&self, original_size: usize, compressed_size: usize) -> bool {
        if original_size == 0 {
            return false;
        }
        let ratio = compressed_size as f64 / original_size as f64;
        ratio <= self.config.max_compression_ratio
    }

    /// Get recommended compression method
    pub fn get_recommended_method(&self, data_size: usize) -> CompressionMethod {
        if !self.config.enabled {
            return CompressionMethod::None;
        }

        if data_size < self.config.min_size_threshold {
            return CompressionMethod::None;
        }

        // For small data, use faster compression
        if data_size < 1024 * 1024 { // 1MB
            return CompressionMethod::Gzip;
        }

        // For larger data, use better compression
        CompressionMethod::Zstd
    }

    /// Batch compress events
    pub fn batch_compress_events(
        &mut self,
        events: Vec<CompressibleEvent>,
    ) -> Result<Vec<CompressedEvent>, CompressionError> {
        let mut results = Vec::new();
        let mut errors = Vec::new();

        for event in events {
            match self.compress_event(&event.data, None) {
                Ok(compressed) => results.push(compressed),
                Err(e) => {
                    errors.push(format!("Failed to compress event for VM {}: {}", event.vm_id, e));
                    // Add original as fallback
                    let fallback = CompressedEvent {
                        method: CompressionMethod::None,
                        compressed_data: event.data,
                        original_size: event.data.len(),
                        compressed_size: event.data.len(),
                        checksum: self.calculate_checksum(&event.data),
                    };
                    results.push(fallback);
                }
            }
        }

        if !errors.is_empty() {
            warn!("Batch compression completed with {} errors", errors.len());
        }

        Ok(results)
    }
}

/// Compressed event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedEvent {
    /// Compression method used
    pub method: CompressionMethod,
    /// Compressed data
    pub compressed_data: Vec<u8>,
    /// Original size in bytes
    pub original_size: usize,
    /// Compressed size in bytes
    pub compressed_size: usize,
    /// Checksum for integrity verification
    pub checksum: String,
}

impl CompressedEvent {
    /// Get compression ratio
    pub fn compression_ratio(&self) -> f64 {
        if self.original_size == 0 {
            0.0
        } else {
            self.compressed_size as f64 / self.original_size as f64
        }
    }

    /// Get space savings percentage
    pub fn space_savings_percent(&self) -> f64 {
        (1.0 - self.compression_ratio()) * 100.0
    }
}

/// Event that can be compressed
#[derive(Debug, Clone)]
pub struct CompressibleEvent {
    /// Virtual machine ID
    pub vm_id: String,
    /// Sequence number
    pub sequence_number: u64,
    /// Event data
    pub data: Vec<u8>,
    /// Event type
    pub event_type: String,
}

impl CompressibleEvent {
    pub fn new(
        vm_id: String,
        sequence_number: u64,
        data: Vec<u8>,
        event_type: String,
    ) -> Self {
        Self {
            vm_id,
            sequence_number,
            data,
            event_type,
        }
    }
}

/// Compression errors
#[derive(Debug, thiserror::Error)]
pub enum CompressionError {
    #[error("Compression failed: {0}")]
    CompressionFailed(String),
    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),
    #[error("Checksum mismatch")]
    ChecksumMismatch,
    #[error("Size mismatch")]
    SizeMismatch,
    #[error("Invalid compression method: {0}")]
    InvalidMethod(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_method_names() {
        assert_eq!(CompressionMethod::None.name(), "none");
        assert_eq!(CompressionMethod::Zstd.name(), "zstd");
        assert_eq!(CompressionMethod::Gzip.name(), "gzip");
    }

    #[test]
    fn test_compressed_event_ratio() {
        let event = CompressedEvent {
            method: CompressionMethod::Zstd,
            compressed_data: vec![1, 2, 3],
            original_size: 100,
            compressed_size: 50,
            checksum: "test".to_string(),
        };

        assert_eq!(event.compression_ratio(), 0.5);
        assert_eq!(event.space_savings_percent(), 50.0);
    }

    #[test]
    fn test_compressible_event_creation() {
        let event = CompressibleEvent::new(
            "vm1".to_string(),
            1,
            vec![1, 2, 3],
            "TestEvent".to_string(),
        );

        assert_eq!(event.vm_id, "vm1");
        assert_eq!(event.sequence_number, 1);
        assert_eq!(event.data, vec![1, 2, 3]);
    }

    #[test]
    fn test_compression_config_default() {
        let config = CompressionConfig::default();
        assert_eq!(config.default_method, CompressionMethod::Zstd);
        assert!(config.enabled);
        assert_eq!(config.compression_level, 3);
        assert_eq!(config.min_size_threshold, 1024);
        assert_eq!(config.max_compression_ratio, 0.9);
    }

    #[tokio::test]
    async fn test_compression_manager() {
        let config = CompressionConfig::default();
        let mut manager = CompressionManager::new(config);

        let test_data = b"This is a test event data that should be compressed".repeat(10);

        // Test compression
        let compressed = manager.compress_event(&test_data, None).unwrap();
        assert_ne!(compressed.original_size, compressed.compressed_size);

        // Test decompression
        let decompressed = manager.decompress_event(&compressed).unwrap();
        assert_eq!(decompressed, test_data);
    }
}