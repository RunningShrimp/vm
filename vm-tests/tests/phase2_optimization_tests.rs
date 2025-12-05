//! 集成测试：Phase 2 优化功能
//!
//! 验证零复制 I/O、DMA、mmap 和异步 I/O 调度器的集成功能

#[cfg(test)]
mod integration_tests {
    use vm_device::dma::{DmaDescriptor, DmaFlags, DmaManager};
    use vm_device::io_scheduler::{AsyncIoScheduler, IoPriority, IoRequest};
    use vm_device::mmap_io::MmapDeviceIo;

    #[test]
    fn test_dma_integration() {
        let dma = DmaManager::new(4096);

        // 创建多个 DMA 映射
        for i in 0..10 {
            let desc = DmaDescriptor {
                guest_addr: 0x1000 * (i + 1) as u64,
                host_addr: Some(0x2000 * (i + 1)),
                len: 512,
                flags: DmaFlags {
                    readable: true,
                    writable: i % 2 == 0,
                    coherent: true,
                },
            };
            assert!(dma.register_mapping(desc).is_ok());
        }

        // 验证统计信息
        let stats = dma.get_stats().unwrap();
        assert_eq!(stats.total_mappings, 10);
        assert_eq!(stats.total_size, 5120);
        assert_eq!(stats.mapped_count, 10);
    }

    #[test]
    fn test_io_scheduler_integration() {
        use vm_device::io_scheduler::IoResult;

        let scheduler = AsyncIoScheduler::new(100);

        // 提交不同优先级的请求
        let mut request_ids = Vec::new();
        for priority in [IoPriority::Low, IoPriority::Normal, IoPriority::High].iter() {
            for i in 0..5 {
                let request = IoRequest::Read {
                    device_id: 1,
                    offset: (i * 4096) as u64,
                    size: 4096,
                    priority: *priority,
                };
                let (req_id, _rx) = scheduler.submit_request(request);
                request_ids.push(req_id);
            }
        }

        // 验证队列长度（在提交后）
        let initial_queue_len = scheduler.queue_length();
        assert_eq!(initial_queue_len, 15);

        // 模拟请求完成
        let mut completed_count = 0;
        for req_id in request_ids.iter() {
            let result = IoResult::ReadOk {
                data: vec![0u8; 4096],
                size: 4096,
            };
            if scheduler.complete_request(*req_id, result).is_ok() {
                completed_count += 1;
            }
        }

        // 验证所有请求都被完成
        assert_eq!(completed_count, 15);
        let final_stats = scheduler.get_stats();
        assert_eq!(final_stats.completed_requests, 15);
    }

    #[test]
    fn test_dma_and_scheduler_cooperation() {
        use vm_device::io_scheduler::IoResult;

        let dma = DmaManager::new(8192);
        let scheduler = AsyncIoScheduler::new(50);

        // 为多个设备注册 DMA 映射
        for device_id in 1..=3 {
            for i in 0..5 {
                let desc = DmaDescriptor {
                    guest_addr: 0x10000 + (device_id as u64 * 0x100000) + (i as u64 * 0x1000),
                    host_addr: Some(0x20000 + (device_id * 0x100000) + (i * 0x1000)),
                    len: 4096,
                    flags: DmaFlags {
                        readable: true,
                        writable: true,
                        coherent: false,
                    },
                };
                assert!(dma.register_mapping(desc).is_ok());
            }
        }

        // 为每个设备提交 I/O 请求
        let mut request_ids = Vec::new();
        for device_id in 1..=3 {
            for i in 0..5 {
                let request = IoRequest::Write {
                    device_id: device_id as u32,
                    offset: (i * 4096) as u64,
                    data: vec![0xAB; 4096],
                    priority: IoPriority::Normal,
                };
                let (req_id, _rx) = scheduler.submit_request(request);
                request_ids.push(req_id);
            }
        }

        // 验证 DMA 统计
        let dma_stats = dma.get_stats().unwrap();
        assert_eq!(dma_stats.total_mappings, 15);
        assert_eq!(dma_stats.total_size, 61440); // 15 * 4096

        // 验证调度器队列长度
        assert_eq!(scheduler.queue_length(), 15);

        // 完成所有请求
        for req_id in request_ids {
            let result = IoResult::WriteOk { size: 4096 };
            let _ = scheduler.complete_request(req_id, result);
        }

        // 验证所有请求都完成了
        let scheduler_stats = scheduler.get_stats();
        assert_eq!(scheduler_stats.completed_requests, 15);
    }

    #[test]
    fn test_mmap_device_io_basic() {
        let mmap_io = MmapDeviceIo::new();
        assert!(mmap_io.page_size() > 0);

        // 获取所有区域（应该为空）
        let regions = mmap_io.get_regions().unwrap();
        assert_eq!(regions.len(), 0);
    }

    #[test]
    fn test_priority_based_scheduling() {
        let scheduler = AsyncIoScheduler::new(100);

        // 按优先级提交请求
        let mut request_ids = Vec::new();
        for priority in [
            IoPriority::Realtime,
            IoPriority::Low,
            IoPriority::High,
            IoPriority::Normal,
        ]
        .iter()
        {
            for i in 0..3 {
                let request = IoRequest::Read {
                    device_id: 1,
                    offset: (i * 4096) as u64,
                    size: 4096,
                    priority: *priority,
                };
                let (req_id, _rx) = scheduler.submit_request(request);
                request_ids.push((*priority, req_id));
            }
        }

        // 验证队列长度
        assert_eq!(scheduler.queue_length(), 12);

        // 验证所有请求都被接受
        assert_eq!(request_ids.len(), 12);
    }

    #[test]
    fn test_multi_device_scheduling() {
        let scheduler = AsyncIoScheduler::new(100);

        // 为 5 个设备提交请求
        let mut device_request_counts = vec![0; 5];
        let mut all_request_ids = Vec::new();

        for device_id in 0..5 {
            for i in 0..10 {
                let request = if i % 2 == 0 {
                    IoRequest::Read {
                        device_id: device_id as u32,
                        offset: (i * 4096) as u64,
                        size: 4096,
                        priority: IoPriority::Normal,
                    }
                } else {
                    IoRequest::Write {
                        device_id: device_id as u32,
                        offset: (i * 4096) as u64,
                        data: vec![0x55; 4096],
                        priority: IoPriority::High,
                    }
                };
                let (req_id, _rx) = scheduler.submit_request(request);
                all_request_ids.push(req_id);
                device_request_counts[device_id] += 1;
            }
        }

        // 验证队列长度
        assert_eq!(scheduler.queue_length(), 50);

        // 完成所有请求
        use vm_device::io_scheduler::IoResult;
        for req_id in all_request_ids {
            let result = IoResult::ReadOk {
                data: vec![0u8; 4096],
                size: 4096,
            };
            let _ = scheduler.complete_request(req_id, result);
        }

        // 验证最终统计
        let stats = scheduler.get_stats();
        assert_eq!(stats.completed_requests, 50);
    }
}
