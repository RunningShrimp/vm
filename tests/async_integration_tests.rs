/// 异步块设备集成测试
///
/// 测试异步Block设备与缓冲池的集成，包括并发操作和性能对比。

#[cfg(test)]
mod async_block_device_integration {
    use vm_device::async_buffer_pool::{AsyncBufferPool, BufferPoolConfig};
    use vm_device::async_block_device::{AsyncBlockDevice, BlockDeviceConfig};
    use std::sync::Arc;
    use tokio::sync::Barrier;

    #[tokio::test]
    async fn test_async_read_write_sequence() {
        let device = AsyncBlockDevice::new_memory(
            1024,
            BufferPoolConfig::default(),
        );

        // 写入数据
        let write_buffer = vec![0xDEu8; 512];
        let written = device.write_async(0, &write_buffer).await.unwrap();
        assert_eq!(written, 512);

        // 读取数据
        let mut read_buffer = vec![0u8; 512];
        let read = device.read_async(0, &mut read_buffer).await.unwrap();
        assert_eq!(read, 512);

        // 验证统计信息
        let stats = device.get_io_stats();
        assert_eq!(stats.read_ops, 1);
        assert_eq!(stats.write_ops, 1);
        assert_eq!(stats.bytes_read, 512);
        assert_eq!(stats.bytes_written, 512);
    }

    #[tokio::test]
    async fn test_concurrent_reads() {
        let device = Arc::new(AsyncBlockDevice::new_memory(
            10000,
            BufferPoolConfig::default(),
        ));

        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for i in 0..10 {
            let device_clone = Arc::clone(&device);
            let barrier_clone = Arc::clone(&barrier);

            let handle = tokio::spawn(async move {
                barrier_clone.wait().await; // 同时启动所有任务

                let mut buffer = vec![0u8; 512];
                let sector = i * 100;
                
                let result = device_clone.read_async(sector as u64, &mut buffer).await;
                assert!(result.is_ok());
                result.unwrap()
            });

            handles.push(handle);
        }

        let mut total_bytes = 0;
        for handle in handles {
            total_bytes += handle.await.unwrap();
        }

        assert_eq!(total_bytes, 512 * 10);

        let stats = device.get_io_stats();
        assert_eq!(stats.read_ops, 10);
        assert_eq!(stats.bytes_read, 512 * 10);
    }

    #[tokio::test]
    async fn test_concurrent_writes() {
        let device = Arc::new(AsyncBlockDevice::new_memory(
            10000,
            BufferPoolConfig::default(),
        ));

        let barrier = Arc::new(Barrier::new(10));
        let mut handles = vec![];

        for i in 0..10 {
            let device_clone = Arc::clone(&device);
            let barrier_clone = Arc::clone(&barrier);

            let handle = tokio::spawn(async move {
                barrier_clone.wait().await;

                let buffer = vec![i as u8; 512];
                let sector = i * 100;
                
                let result = device_clone.write_async(sector as u64, &buffer).await;
                assert!(result.is_ok());
                result.unwrap()
            });

            handles.push(handle);
        }

        let mut total_bytes = 0;
        for handle in handles {
            total_bytes += handle.await.unwrap();
        }

        assert_eq!(total_bytes, 512 * 10);

        let stats = device.get_io_stats();
        assert_eq!(stats.write_ops, 10);
        assert_eq!(stats.bytes_written, 512 * 10);
    }

    #[tokio::test]
    async fn test_concurrent_mixed_operations() {
        let device = Arc::new(AsyncBlockDevice::new_memory(
            10000,
            BufferPoolConfig::default(),
        ));

        let barrier = Arc::new(Barrier::new(20));
        let mut handles = vec![];

        // 10个读操作
        for i in 0..10 {
            let device_clone = Arc::clone(&device);
            let barrier_clone = Arc::clone(&barrier);

            let handle = tokio::spawn(async move {
                barrier_clone.wait().await;

                let mut buffer = vec![0u8; 512];
                let sector = i * 100;
                
                device_clone.read_async(sector as u64, &mut buffer).await.unwrap()
            });

            handles.push(handle);
        }

        // 10个写操作
        for i in 10..20 {
            let device_clone = Arc::clone(&device);
            let barrier_clone = Arc::clone(&barrier);

            let handle = tokio::spawn(async move {
                barrier_clone.wait().await;

                let buffer = vec![(i % 256) as u8; 512];
                let sector = i * 100;
                
                device_clone.write_async(sector as u64, &buffer).await.unwrap()
            });

            handles.push(handle);
        }

        let mut total_bytes = 0;
        for handle in handles {
            total_bytes += handle.await.unwrap();
        }

        assert_eq!(total_bytes, 512 * 20);

        let stats = device.get_io_stats();
        assert_eq!(stats.read_ops + stats.write_ops, 20);
        assert_eq!(stats.bytes_read + stats.bytes_written, 512 * 20);
    }

    #[tokio::test]
    async fn test_buffer_pool_with_device() {
        let device = AsyncBlockDevice::new_memory(
            1024,
            BufferPoolConfig::default(),
        );

        // 获取缓冲区进行I/O
        let buf = device.acquire_buffer().await.unwrap();
        assert_eq!(buf.data.len(), 4096);

        let initial_stats = device.get_buffer_stats();
        assert_eq!(initial_stats.in_use_buffers, 1);

        drop(buf); // 释放缓冲区

        let final_stats = device.get_buffer_stats();
        assert_eq!(final_stats.in_use_buffers, 0);
        assert_eq!(final_stats.total_releases, 1);
    }

    #[tokio::test]
    async fn test_read_beyond_capacity() {
        let device = AsyncBlockDevice::new_memory(
            100,  // 只有100个扇区
            BufferPoolConfig::default(),
        );

        let mut buffer = vec![0u8; 512];
        let result = device.read_async(99, &mut buffer).await;
        
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_write_beyond_capacity() {
        let device = AsyncBlockDevice::new_memory(
            100,
            BufferPoolConfig::default(),
        );

        let buffer = vec![0xAAu8; 512];
        let result = device.write_async(99, &buffer).await;
        
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_only_device() {
        let config = BlockDeviceConfig {
            capacity_sectors: 1024,
            sector_size: 512,
            read_only: true,
        };

        let device = AsyncBlockDevice {
            file_path: "<memory>".to_string(),
            file: std::sync::Arc::new(parking_lot::RwLock::new(None)),
            buffer_pool: std::sync::Arc::new(AsyncBufferPool::new(BufferPoolConfig::default())),
            config,
            stats: std::sync::Arc::new(parking_lot::RwLock::new(
                vm_device::async_block_device::AsyncIoStats::default()
            )),
        };

        // 读应该成功
        let mut buffer = vec![0u8; 512];
        assert!(device.read_async(0, &mut buffer).await.is_ok());

        // 写应该失败
        let buffer = vec![0xAAu8; 512];
        assert!(device.write_async(0, &buffer).await.is_err());

        // 刷新应该成功（空操作）
        assert!(device.flush_async().await.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_sequential_operations() {
        let device = Arc::new(AsyncBlockDevice::new_memory(
            10000,
            BufferPoolConfig::default(),
        ));

        // 执行100个连续的读写操作
        for i in 0..100 {
            let write_buf = vec![(i % 256) as u8; 512];
            device.write_async(i as u64, &write_buf).await.unwrap();

            let mut read_buf = vec![0u8; 512];
            device.read_async(i as u64, &mut read_buf).await.unwrap();
        }

        let stats = device.get_io_stats();
        assert_eq!(stats.write_ops, 100);
        assert_eq!(stats.read_ops, 100);
        assert_eq!(stats.bytes_written, 512 * 100);
        assert_eq!(stats.bytes_read, 512 * 100);
    }

    #[tokio::test]
    async fn test_buffer_pool_reuse() {
        let device = AsyncBlockDevice::new_memory(
            1024,
            BufferPoolConfig {
                buffer_size: 4096,
                initial_pool_size: 5,
                max_pool_size: 10,
                max_pending_ops: 5,
            },
        );

        // 连续获取和释放缓冲区
        for _ in 0..10 {
            let buf = device.acquire_buffer().await.unwrap();
            assert_eq!(buf.data.len(), 4096);
            drop(buf);
        }

        let stats = device.get_buffer_stats();
        // 缓冲区应该被复用，总数不应该增加很多
        assert!(stats.total_buffers <= 10);
        assert!(stats.pool_hits > 0);
    }

    #[tokio::test]
    async fn test_flush_clears_write_buffer() {
        let device = AsyncBlockDevice::new_memory(
            1024,
            BufferPoolConfig::default(),
        );

        // 执行几个写操作
        for i in 0..5 {
            let buf = vec![i as u8; 512];
            device.write_async(i as u64, &buf).await.unwrap();
        }

        // 刷新
        device.flush_async().await.unwrap();

        let stats = device.get_io_stats();
        assert_eq!(stats.write_ops, 5);
        assert_eq!(stats.flush_ops, 1);
    }
}

#[cfg(test)]
mod async_buffer_pool_integration {
    use vm_device::async_buffer_pool::{AsyncBufferPool, BufferPoolConfig};
    use std::sync::Arc;
    use tokio::sync::Barrier;

    #[tokio::test]
    async fn test_concurrent_buffer_acquisition() {
        let pool = Arc::new(AsyncBufferPool::new(
            BufferPoolConfig {
                buffer_size: 4096,
                initial_pool_size: 10,
                max_pool_size: 50,
                max_pending_ops: 20,
            }
        ));

        let barrier = Arc::new(Barrier::new(20));
        let mut handles = vec![];

        for _ in 0..20 {
            let pool_clone = Arc::clone(&pool);
            let barrier_clone = Arc::clone(&barrier);

            let handle = tokio::spawn(async move {
                barrier_clone.wait().await;
                
                let buf = pool_clone.acquire().await;
                assert!(buf.is_ok());
                
                let buf = buf.unwrap();
                assert_eq!(buf.data.len(), 4096);
                
                // 缓冲区在drop时自动释放
                drop(buf);
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let stats = pool.get_stats();
        assert_eq!(stats.total_allocations, 20);
        assert!(stats.pool_hits > 0);
    }

    #[tokio::test]
    async fn test_buffer_pool_expansion() {
        let pool = Arc::new(AsyncBufferPool::new(
            BufferPoolConfig {
                buffer_size: 4096,
                initial_pool_size: 5,
                max_pool_size: 20,
                max_pending_ops: 10,
            }
        ));

        // 尝试获取超过初始大小的缓冲区
        let mut buffers = vec![];
        for _ in 0..8 {
            let buf = pool.acquire().await.unwrap();
            buffers.push(buf);
        }

        let stats = pool.get_stats();
        // 应该进行了扩展
        assert!(stats.total_buffers >= 8);

        // 释放所有缓冲区
        drop(buffers);

        let stats = pool.get_stats();
        assert_eq!(stats.in_use_buffers, 0);
    }

    #[test]
    fn test_try_acquire_non_blocking() {
        let pool = Arc::new(AsyncBufferPool::new(
            BufferPoolConfig {
                buffer_size: 4096,
                initial_pool_size: 5,
                max_pool_size: 10,
                max_pending_ops: 5,
            }
        ));

        // 非阻塞获取应该立即返回
        let start = std::time::Instant::now();
        for _ in 0..5 {
            let buf = pool.try_acquire();
            assert!(buf.is_some());
        }
        let elapsed = start.elapsed();
        
        // 应该非常快速（< 100ms）
        assert!(elapsed.as_millis() < 100);

        // 第6次应该失败（所有缓冲区都在使用中）
        let buf = pool.try_acquire();
        assert!(buf.is_none());
    }
}
