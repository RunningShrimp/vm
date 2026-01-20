use std::io::Result as IoResult;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::Mutex;

/// 异步块设备实现
///
/// 提供真正的异步I/O支持，使用tokio::fs和缓冲池实现高效的块设备操作。
use crate::async_buffer_pool::{AsyncBufferPool, BufferPoolConfig, BufferPoolStats, PoolBuffer};

/// 异步I/O操作统计
#[derive(Clone, Debug, Default)]
pub struct AsyncIoStats {
    /// 读操作数
    pub read_ops: u64,
    /// 写操作数
    pub write_ops: u64,
    /// 刷新操作数
    pub flush_ops: u64,
    /// 总字节读取
    pub bytes_read: u64,
    /// 总字节写入
    pub bytes_written: u64,
    /// I/O错误数
    pub io_errors: u64,
    /// 平均读取延迟（微秒）
    pub avg_read_latency_us: u64,
    /// 平均写入延迟（微秒）
    pub avg_write_latency_us: u64,
}

impl AsyncIoStats {
    /// 计算平均吞吐量（MB/s）
    pub fn throughput_mbps(&self) -> f64 {
        let total_bytes = self.bytes_read + self.bytes_written;
        if total_bytes == 0 {
            return 0.0;
        }
        total_bytes as f64 / 1024.0 / 1024.0 // 简化计算，实际需要时间信息
    }

    /// 计算I/O错误率
    pub fn error_rate(&self) -> f64 {
        let total_ops = self.read_ops + self.write_ops + self.flush_ops;
        if total_ops == 0 {
            return 0.0;
        }
        self.io_errors as f64 / total_ops as f64
    }
}

/// 异步块设备
pub struct AsyncBlockDevice {
    /// 文件路径
    file_path: String,
    /// 打开的文件句柄
    file: Arc<Mutex<Option<tokio::fs::File>>>,
    /// 缓冲池
    buffer_pool: Arc<AsyncBufferPool>,
    /// 设备配置（容量、扇区大小等）
    config: BlockDeviceConfig,
    /// 统计信息
    stats: Arc<Mutex<AsyncIoStats>>,
}

/// 块设备配置
#[derive(Clone, Debug)]
pub struct BlockDeviceConfig {
    /// 设备容量（扇区数）
    pub capacity_sectors: u64,
    /// 扇区大小（字节）
    pub sector_size: u32,
    /// 是否只读
    pub read_only: bool,
}

impl Default for BlockDeviceConfig {
    fn default() -> Self {
        Self {
            capacity_sectors: 1024 * 1024, // 1M扇区
            sector_size: 512,
            read_only: false,
        }
    }
}

impl AsyncBlockDevice {
    /// 创建新的异步块设备
    pub async fn new<P: AsRef<Path>>(
        path: P,
        config: BlockDeviceConfig,
        buffer_pool_config: BufferPoolConfig,
    ) -> IoResult<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();

        // 以适当的模式打开文件
        let file = if config.read_only {
            tokio::fs::OpenOptions::new()
                .read(true)
                .open(path.as_ref())
                .await?
        } else {
            tokio::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(path.as_ref())
                .await?
        };

        Ok(Self {
            file_path: path_str,
            file: Arc::new(Mutex::new(Some(file))),
            buffer_pool: Arc::new(AsyncBufferPool::new(buffer_pool_config)),
            config,
            stats: Arc::new(Mutex::new(AsyncIoStats::default())),
        })
    }

    /// 创建基于内存的块设备（测试用）
    pub fn new_memory(capacity_sectors: u64, buffer_pool_config: BufferPoolConfig) -> Self {
        Self {
            file_path: "<memory>".to_string(),
            file: Arc::new(Mutex::new(None)),
            buffer_pool: Arc::new(AsyncBufferPool::new(buffer_pool_config)),
            config: BlockDeviceConfig {
                capacity_sectors,
                sector_size: 512,
                read_only: false,
            },
            stats: Arc::new(Mutex::new(AsyncIoStats::default())),
        }
    }

    /// 异步读操作
    pub async fn read_async(&self, sector: u64, buffer: &mut [u8]) -> IoResult<usize> {
        // 验证参数
        let sectors_to_read = (buffer.len() as u64).div_ceil(self.config.sector_size as u64);

        if sector + sectors_to_read > self.config.capacity_sectors {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Read beyond device capacity",
            ));
        }

        let start_time = std::time::Instant::now();
        let bytes_requested = buffer.len();

        // 获取文件引用（使用async lock）
        let file_guard = self.file.lock().await;
        let file_result = match file_guard.as_ref() {
            Some(f) => Some(f.try_clone().await),
            None => None,
        };

        match file_result {
            Some(Ok(mut file)) => {
                // 计算偏移量
                let offset = sector * self.config.sector_size as u64;

                // 执行异步读操作
                tokio::io::AsyncSeekExt::seek(&mut file, std::io::SeekFrom::Start(offset))
                    .await
                    .map_err(|e| std::io::Error::other(format!("Seek failed: {}", e)))?;

                tokio::io::AsyncReadExt::read(&mut file, buffer)
                    .await
                    .map_err(|e| std::io::Error::other(format!("Read failed: {}", e)))
            }
            Some(Err(_)) => Err(std::io::Error::other("Failed to clone file")),
            None => {
                // 内存模式 - 填充零
                buffer.fill(0);
                Ok(bytes_requested)
            }
        }
        .inspect(|&bytes| {
            // 更新统计信息
            let elapsed_us = start_time.elapsed().as_micros() as u64;
            tokio::task::block_in_place(|| {
                let mut stats = self.stats.blocking_lock();
                stats.read_ops += 1;
                stats.bytes_read += bytes as u64;
                if stats.avg_read_latency_us == 0 {
                    stats.avg_read_latency_us = elapsed_us;
                } else {
                    stats.avg_read_latency_us = (stats.avg_read_latency_us + elapsed_us) / 2;
                }
            });
        })
        .inspect_err(|_e| {
            tokio::task::block_in_place(|| {
                let mut stats = self.stats.blocking_lock();
                stats.io_errors += 1;
            });
        })
    }

    /// 异步写操作
    pub async fn write_async(&self, sector: u64, buffer: &[u8]) -> IoResult<usize> {
        // 检查只读标志
        if self.config.read_only {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Device is read-only",
            ));
        }

        // 验证参数
        let sectors_to_write = (buffer.len() as u64).div_ceil(self.config.sector_size as u64);

        if sector + sectors_to_write > self.config.capacity_sectors {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Write beyond device capacity",
            ));
        }

        let start_time = std::time::Instant::now();
        let bytes_requested = buffer.len();

        // 获取文件引用（使用async lock）
        let file_result = {
            let file_guard = self.file.lock().await;
            match file_guard.as_ref() {
                Some(f) => Some(f.try_clone().await),
                None => None,
            }
        };

        match file_result {
            Some(Ok(mut file)) => {
                // 计算偏移量
                let offset = sector * self.config.sector_size as u64;

                // 执行异步写操作
                tokio::io::AsyncSeekExt::seek(&mut file, std::io::SeekFrom::Start(offset))
                    .await
                    .map_err(|e| std::io::Error::other(format!("Seek failed: {}", e)))?;

                tokio::io::AsyncWriteExt::write(&mut file, buffer)
                    .await
                    .map_err(|e| std::io::Error::other(format!("Write failed: {}", e)))
            }
            Some(Err(_)) => Err(std::io::Error::other("Failed to clone file")),
            None => {
                // 内存模式 - 无操作
                Ok(bytes_requested)
            }
        }
        .inspect(|&bytes| {
            // 更新统计信息
            let elapsed_us = start_time.elapsed().as_micros() as u64;
            tokio::task::block_in_place(|| {
                let mut stats = self.stats.blocking_lock();
                stats.write_ops += 1;
                stats.bytes_written += bytes as u64;
                if stats.avg_write_latency_us == 0 {
                    stats.avg_write_latency_us = elapsed_us;
                } else {
                    stats.avg_write_latency_us = (stats.avg_write_latency_us + elapsed_us) / 2;
                }
            });
        })
        .inspect_err(|_e| {
            tokio::task::block_in_place(|| {
                let mut stats = self.stats.blocking_lock();
                stats.io_errors += 1;
            });
        })
    }

    /// 异步刷新操作
    pub async fn flush_async(&self) -> IoResult<()> {
        if self.config.read_only {
            return Ok(());
        }

        let start_time = std::time::Instant::now();

        // 获取文件引用（使用async lock）
        let file_result = {
            let file_guard = self.file.lock().await;
            match file_guard.as_ref() {
                Some(f) => Some(f.try_clone().await),
                None => None,
            }
        };

        let result = match file_result {
            Some(Ok(mut file)) => {
                // 执行异步刷新操作
                tokio::io::AsyncWriteExt::flush(&mut file)
                    .await
                    .map_err(|e| std::io::Error::other(format!("Flush failed: {}", e)))
            }
            Some(Err(_)) => Err(std::io::Error::other("Failed to clone file")),
            None => {
                // 内存模式 - 无操作
                Ok(())
            }
        };

        result
            .map(|_| {
                let elapsed_us = start_time.elapsed().as_micros() as u64;
                tokio::task::block_in_place(|| {
                    let mut stats = self.stats.blocking_lock();
                    stats.flush_ops += 1;
                    if stats.avg_write_latency_us == 0 {
                        stats.avg_write_latency_us = elapsed_us;
                    } else {
                        stats.avg_write_latency_us = (stats.avg_write_latency_us + elapsed_us) / 2;
                    }
                });
            })
            .inspect_err(|_e| {
                tokio::task::block_in_place(|| {
                    let mut stats = self.stats.blocking_lock();
                    stats.io_errors += 1;
                });
            })
    }

    /// 获取缓冲区进行直接I/O
    pub async fn acquire_buffer(&self) -> Result<PoolBuffer, String> {
        self.buffer_pool.acquire().await
    }

    /// 尝试立即获取缓冲区
    pub async fn try_acquire_buffer(&self) -> Option<PoolBuffer> {
        self.buffer_pool.try_acquire().await
    }

    /// 获取I/O统计信息
    pub async fn get_io_stats(&self) -> AsyncIoStats {
        let stats = self.stats.lock().await;
        stats.clone()
    }

    /// 获取缓冲池统计信息
    pub fn get_buffer_stats(&self) -> BufferPoolStats {
        self.buffer_pool.get_stats_sync()
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        tokio::task::block_in_place(|| {
            let mut stats = self.stats.blocking_lock();
            *stats = AsyncIoStats::default();
        });
        self.buffer_pool.reset_stats();
    }

    /// 预热缓冲池
    pub async fn warmup_buffers(&self, count: usize) {
        self.buffer_pool.warmup(count).await;
    }

    /// 获取设备容量（字节）
    pub fn capacity_bytes(&self) -> u64 {
        self.config.capacity_sectors * self.config.sector_size as u64
    }

    /// 获取设备配置
    pub fn config(&self) -> &BlockDeviceConfig {
        &self.config
    }
}

impl Clone for AsyncBlockDevice {
    fn clone(&self) -> Self {
        Self {
            file_path: self.file_path.clone(),
            file: Arc::clone(&self.file),
            buffer_pool: Arc::clone(&self.buffer_pool),
            config: self.config.clone(),
            stats: Arc::clone(&self.stats),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_block_device_creation() {
        let device = AsyncBlockDevice::new_memory(1024, BufferPoolConfig::default());

        assert_eq!(device.capacity_bytes(), 1024 * 512);
        assert!(!device.config().read_only);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_read_operation() {
        let device = AsyncBlockDevice::new_memory(1024, BufferPoolConfig::default());

        let mut buffer = vec![0u8; 512];
        let bytes_read = match device.read_async(0, &mut buffer).await {
            Ok(bytes) => bytes,
            Err(e) => panic!("Read operation failed: {}", e),
        };

        assert_eq!(bytes_read, 512);
        assert_eq!(buffer.len(), 512);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_write_operation() {
        let device = AsyncBlockDevice::new_memory(1024, BufferPoolConfig::default());

        let buffer = vec![0xAAu8; 512];
        let bytes_written = match device.write_async(0, &buffer).await {
            Ok(bytes) => bytes,
            Err(e) => panic!("Write operation failed: {}", e),
        };

        assert_eq!(bytes_written, 512);
    }

    #[tokio::test]
    async fn test_read_only_write_fails() {
        let config = BlockDeviceConfig {
            capacity_sectors: 1024,
            sector_size: 512,
            read_only: true,
        };

        let device = AsyncBlockDevice {
            file_path: "<memory>".to_string(),
            file: Arc::new(Mutex::new(None)),
            buffer_pool: Arc::new(AsyncBufferPool::new(BufferPoolConfig::default())),
            config,
            stats: Arc::new(Mutex::new(AsyncIoStats::default())),
        };

        let buffer = vec![0xAAu8; 512];
        let result = device.write_async(0, &buffer).await;

        assert!(result.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_flush_operation() {
        let device = AsyncBlockDevice::new_memory(1024, BufferPoolConfig::default());

        match device.flush_async().await {
            Ok(_) => {}
            Err(e) => panic!("Flush operation failed: {}", e),
        }

        let stats = device.get_io_stats().await;
        assert_eq!(stats.flush_ops, 1);
    }

    #[tokio::test]
    async fn test_io_stats() {
        let device = AsyncBlockDevice::new_memory(1024, BufferPoolConfig::default());

        let stats = device.get_io_stats().await;
        assert_eq!(stats.read_ops, 0);
        assert_eq!(stats.write_ops, 0);
        assert_eq!(stats.error_rate(), 0.0);
    }
}
