use std::collections::VecDeque;
/// 异步I/O缓冲池实现
///
/// 提供高效的缓冲区管理，支持异步操作和自动扩展。
/// 使用tokio的Semaphore进行资源限制，Arc<Mutex>保护内部状态。
use std::sync::Arc;

use tokio::sync::{Mutex, Semaphore};

/// 缓冲池配置
#[derive(Clone, Debug)]
pub struct BufferPoolConfig {
    /// 单个缓冲区大小（字节）
    pub buffer_size: usize,
    /// 初始缓冲区数量
    pub initial_pool_size: usize,
    /// 最大缓冲区数量
    pub max_pool_size: usize,
    /// 最大待处理操作数
    pub max_pending_ops: usize,
}

impl Default for BufferPoolConfig {
    fn default() -> Self {
        Self {
            buffer_size: 4096, // 一个扇区的标准大小
            initial_pool_size: 32,
            max_pool_size: 256,
            max_pending_ops: 128,
        }
    }
}

/// 缓冲池统计信息
#[derive(Clone, Debug, Default)]
pub struct BufferPoolStats {
    /// 当前缓冲区数量
    pub total_buffers: usize,
    /// 可用缓冲区数量
    pub available_buffers: usize,
    /// 使用中的缓冲区数量
    pub in_use_buffers: usize,
    /// 总的分配请求数
    pub total_allocations: u64,
    /// 缓冲池命中数
    pub pool_hits: u64,
    /// 缓冲池未命中数（新分配）
    pub pool_misses: u64,
    /// 释放的缓冲区数
    pub total_releases: u64,
}

impl BufferPoolStats {
    /// 计算缓冲池命中率
    pub fn hit_rate(&self) -> f64 {
        if self.total_allocations == 0 {
            0.0
        } else {
            self.pool_hits as f64 / self.total_allocations as f64
        }
    }

    /// 计算缓冲区利用率
    pub fn utilization_rate(&self) -> f64 {
        if self.total_buffers == 0 {
            0.0
        } else {
            self.in_use_buffers as f64 / self.total_buffers as f64
        }
    }
}

/// 缓冲区包装
pub struct PoolBuffer {
    /// 缓冲区数据
    pub data: Vec<u8>,
    /// 返回时的回调（自动释放）
    release: Option<Arc<dyn Fn(Vec<u8>) + Send + Sync>>,
}

impl PoolBuffer {
    /// 创建新的池缓冲区
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            release: None,
        }
    }

    /// 设置释放回调
    pub fn with_release(mut self, release: Arc<dyn Fn(Vec<u8>) + Send + Sync + 'static>) -> Self {
        self.release = Some(release);
        self
    }
}

impl Drop for PoolBuffer {
    fn drop(&mut self) {
        if let Some(release) = &self.release {
            release(std::mem::take(&mut self.data));
        }
    }
}

/// 异步I/O缓冲池
pub struct AsyncBufferPool {
    /// 可用缓冲区队列
    available: Arc<tokio::sync::Mutex<VecDeque<Vec<u8>>>>,
    /// 资源信号量（控制并发操作数）
    semaphore: Arc<Semaphore>,
    /// 配置信息
    config: BufferPoolConfig,
    /// 统计信息
    stats: Arc<tokio::sync::Mutex<BufferPoolStats>>,
}

impl AsyncBufferPool {
    /// 创建新的缓冲池
    pub fn new(config: BufferPoolConfig) -> Self {
        let mut pool = VecDeque::new();

        // 预分配初始缓冲区
        for _ in 0..config.initial_pool_size {
            pool.push_back(vec![0u8; config.buffer_size]);
        }

        let stats = BufferPoolStats {
            total_buffers: config.initial_pool_size,
            available_buffers: config.initial_pool_size,
            in_use_buffers: 0,
            total_allocations: 0,
            pool_hits: 0,
            pool_misses: 0,
            total_releases: 0,
        };

        Self {
            available: Arc::new(tokio::sync::Mutex::new(pool)),
            semaphore: Arc::new(Semaphore::new(config.max_pending_ops)),
            config,
            stats: Arc::new(tokio::sync::Mutex::new(stats)),
        }
    }

    /// 从池中获取一个缓冲区（异步）
    pub async fn acquire(&self) -> Result<PoolBuffer, String> {
        // 等待资源可用
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| format!("Failed to acquire semaphore permit: {}", e))?;

        let mut available = self.available.lock().await;
        let can_expand = self.can_expand(&available);
        let buffer = if let Some(buf) = available.pop_front() {
            // 从池中复用缓冲区
            {
                let mut stats = self.stats.lock().await;
                stats.pool_hits += 1;
                stats.in_use_buffers += 1;
                stats.available_buffers -= 1;
            }
            buf
        } else if can_expand {
            // 池中没有可用缓冲区，但可以扩展
            {
                let mut stats = self.stats.lock().await;
                stats.pool_misses += 1;
                stats.total_buffers += 1;
                stats.in_use_buffers += 1;
                stats.total_allocations += 1;
            }
            vec![0u8; self.config.buffer_size]
        } else {
            // 等待直到有缓冲区可用（放弃许可权，重新获取）
            drop(available);
            drop(_permit);

            // 再次尝试获取
            let _permit = self
                .semaphore
                .acquire()
                .await
                .map_err(|e| format!("Failed to reacquire semaphore permit: {}", e))?;

            available = self.available.lock().await;
            match available.pop_front() {
                Some(buf) => {
                    {
                        let mut stats = self.stats.lock().await;
                        stats.pool_hits += 1;
                        stats.in_use_buffers += 1;
                        stats.available_buffers -= 1;
                    }
                    buf
                }
                None => return Err("Failed to acquire buffer after waiting".to_string()),
            }
        };

        {
            let mut stats = self.stats.lock().await;
            stats.total_allocations += 1;
        }

        // 创建自动释放回调
        let pool = self.clone_pool();
        let release_fn = Arc::new(move |buf: Vec<u8>| {
            tokio::task::block_in_place(|| {
                let mut available = pool.available.blocking_lock();
                available.push_back(buf);

                let mut stats = pool.stats.blocking_lock();
                stats.total_releases += 1;
                stats.in_use_buffers = stats.in_use_buffers.saturating_sub(1);
                stats.available_buffers += 1;
            });
        });

        Ok(PoolBuffer::new(buffer).with_release(release_fn))
    }

    /// 同步获取缓冲区（如果立即可用）
    pub async fn try_acquire(&self) -> Option<PoolBuffer> {
        if self.semaphore.try_acquire().is_ok() {
            let mut available = self.available.lock().await;
            if let Some(buf) = available.pop_front() {
                let mut stats = self.stats.lock().await;
                stats.pool_hits += 1;
                stats.in_use_buffers += 1;
                stats.available_buffers -= 1;
                stats.total_allocations += 1;

                let pool = self.clone_pool();
                let release_fn = Arc::new(move |buf: Vec<u8>| {
                    // 在同步上下文中获取锁
                    if let Ok(mut available) = pool.available.try_lock() {
                        available.push_back(buf);

                        if let Ok(mut stats) = pool.stats.try_lock() {
                            stats.total_releases += 1;
                            stats.in_use_buffers = stats.in_use_buffers.saturating_sub(1);
                            stats.available_buffers += 1;
                        }
                    }
                });

                return Some(PoolBuffer::new(buf).with_release(release_fn));
            }
        }
        None
    }

    /// 手动释放缓冲区到池中
    pub fn release(&self, buffer: Vec<u8>) {
        if buffer.len() == self.config.buffer_size
            && let Ok(mut available) = self.available.try_lock()
        {
            available.push_back(buffer);

            if let Ok(mut stats) = self.stats.try_lock() {
                stats.total_releases += 1;
                stats.in_use_buffers = stats.in_use_buffers.saturating_sub(1);
                stats.available_buffers += 1;
            }
        }
    }

    /// 获取统计信息（异步版本）
    pub async fn get_stats(&self) -> BufferPoolStats {
        self.stats.lock().await.clone()
    }

    /// 获取统计信息（同步版本）
    pub fn get_stats_sync(&self) -> BufferPoolStats {
        if let Ok(stats) = self.stats.try_lock() {
            stats.clone()
        } else {
            BufferPoolStats::default()
        }
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        if let Ok(mut stats) = self.stats.try_lock() {
            stats.total_allocations = 0;
            stats.pool_hits = 0;
            stats.pool_misses = 0;
            stats.total_releases = 0;
        }
    }

    /// 检查是否可以扩展缓冲池
    fn can_expand(&self, available: &VecDeque<Vec<u8>>) -> bool {
        if let Ok(stats) = self.stats.try_lock() {
            available.is_empty() && stats.total_buffers < self.config.max_pool_size
        } else {
            false
        }
    }

    /// 克隆池引用（用于回调）
    fn clone_pool(&self) -> AsyncBufferPoolRef {
        AsyncBufferPoolRef {
            available: Arc::clone(&self.available),
            stats: Arc::clone(&self.stats),
        }
    }

    /// 清空缓冲池
    pub fn clear(&self) {
        if let Ok(mut available) = self.available.try_lock() {
            available.clear();
        }

        if let Ok(mut stats) = self.stats.try_lock() {
            stats.total_buffers = 0;
            stats.available_buffers = 0;
            stats.in_use_buffers = 0;
            stats.pool_hits = 0;
            stats.pool_misses = 0;
            stats.total_allocations = 0;
            stats.total_releases = 0;
        }
    }

    /// 预热缓冲池（预分配指定数量的缓冲区）
    pub async fn warmup(&self, count: usize) {
        let mut available = self.available.lock().await;
        let mut stats = self.stats.lock().await;

        let to_allocate = std::cmp::min(count, self.config.max_pool_size - stats.total_buffers);

        for _ in 0..to_allocate {
            available.push_back(vec![0u8; self.config.buffer_size]);
            stats.total_buffers += 1;
            stats.available_buffers += 1;
        }
    }
}

impl Clone for AsyncBufferPool {
    fn clone(&self) -> Self {
        Self {
            available: Arc::clone(&self.available),
            semaphore: Arc::clone(&self.semaphore),
            config: self.config.clone(),
            stats: Arc::clone(&self.stats),
        }
    }
}

/// 缓冲池内部引用（用于释放回调）
struct AsyncBufferPoolRef {
    available: Arc<Mutex<VecDeque<Vec<u8>>>>,
    stats: Arc<Mutex<BufferPoolStats>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool_creation() {
        let config = BufferPoolConfig::default();
        let pool = AsyncBufferPool::new(config.clone());

        let stats = pool.get_stats_sync();
        assert_eq!(stats.total_buffers, config.initial_pool_size);
        assert_eq!(stats.available_buffers, config.initial_pool_size);
        assert_eq!(stats.in_use_buffers, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_buffer_acquire_and_release() {
        let pool = AsyncBufferPool::new(BufferPoolConfig::default());

        let buf = pool.acquire().await.expect("Failed to acquire buffer");
        assert_eq!(buf.data.len(), 4096);

        let stats = pool.get_stats().await;
        assert_eq!(stats.in_use_buffers, 1);
        assert_eq!(stats.pool_hits, 1);

        drop(buf); // 自动释放

        let stats = pool.get_stats().await;
        assert_eq!(stats.in_use_buffers, 0);
        assert_eq!(stats.total_releases, 1);
    }

    #[tokio::test]
    async fn test_try_acquire() {
        let pool = AsyncBufferPool::new(BufferPoolConfig::default());

        let buf = pool.try_acquire().await;
        assert!(buf.is_some());

        let stats = pool.get_stats().await;
        assert_eq!(stats.pool_hits, 1);
    }

    #[test]
    fn test_buffer_pool_stats() {
        let pool = AsyncBufferPool::new(BufferPoolConfig::default());

        let initial_stats = pool.get_stats_sync();
        assert_eq!(initial_stats.hit_rate(), 0.0);
        assert_eq!(initial_stats.utilization_rate(), 0.0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_buffer_reuse() {
        let pool = AsyncBufferPool::new(BufferPoolConfig::default());
        let initial_count = pool.get_stats().await.total_buffers;

        let buf1 = match pool.acquire().await {
            Ok(buf) => buf,
            Err(e) => panic!("Failed to acquire buffer: {}", e),
        };
        drop(buf1);

        let _buf2 = match pool.acquire().await {
            Ok(buf) => buf,
            Err(e) => panic!("Failed to acquire buffer: {}", e),
        };

        let stats = pool.get_stats().await;
        assert_eq!(stats.total_buffers, initial_count); // 没有分配新缓冲区
        assert_eq!(stats.pool_hits, 2); // 两次都命中
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_warmup() {
        let pool = AsyncBufferPool::new(BufferPoolConfig {
            initial_pool_size: 10,
            max_pool_size: 100,
            ..Default::default()
        });

        pool.warmup(20).await;
        let stats = pool.get_stats().await;
        assert_eq!(stats.total_buffers, 30);
    }
}
