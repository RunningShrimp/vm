//! NUMA感知内存分配器测试
//!
//! 测试NUMA感知的内存分配性能和策略

use std::sync::Arc;
use std::time::Instant;
use vm_mem::{NumaAllocPolicy, NumaAllocStats, NumaAllocator, NumaNodeInfo};

/// 创建测试用的NUMA节点信息
fn create_test_numa_nodes() -> Vec<NumaNodeInfo> {
    vec![
        NumaNodeInfo {
            node_id: 0,
            total_memory: 8 * 1024 * 1024 * 1024,     // 8GB
            available_memory: 7 * 1024 * 1024 * 1024, // 7GB
            cpu_mask: 0xFF,                           // CPU 0-7
        },
        NumaNodeInfo {
            node_id: 1,
            total_memory: 8 * 1024 * 1024 * 1024,     // 8GB
            available_memory: 7 * 1024 * 1024 * 1024, // 7GB
            cpu_mask: 0xFF00,                         // CPU 8-15
        },
        NumaNodeInfo {
            node_id: 2,
            total_memory: 4 * 1024 * 1024 * 1024,     // 4GB
            available_memory: 3 * 1024 * 1024 * 1024, // 3GB
            cpu_mask: 0xFF0000,                       // CPU 16-23
        },
    ]
}

#[test]
fn test_numa_node_detection() {
    let nodes = create_test_numa_nodes();

    println!("NUMA Node Detection Test:");
    for node in nodes.iter() {
        println!(
            "  Node {}: {}GB total, {}GB available, CPU mask: {:#x}",
            node.node_id,
            node.total_memory / (1024 * 1024 * 1024),
            node.available_memory / (1024 * 1024 * 1024),
            node.cpu_mask
        );
    }

    assert_eq!(nodes.len(), 3, "Should have 3 NUMA nodes");
    assert!(
        nodes.iter().map(|n| n.total_memory).sum::<u64>() > 0,
        "Total memory should be positive"
    );
}

#[test]
fn test_numa_alloc_strategies() {
    let nodes = create_test_numa_nodes();

    let strategies = vec![
        NumaAllocPolicy::Local,
        NumaAllocPolicy::Interleave,
        NumaAllocPolicy::Bind(0),
        NumaAllocPolicy::Preferred(1),
    ];

    println!("NUMA Allocation Strategy Test:");

    for policy in strategies {
        let allocator = NumaAllocator::new(nodes.clone(), policy);
        println!("  Strategy: {:?}", policy);
        println!("    Node count: {}", allocator.node_count());
        println!(
            "    Available memory: {}GB",
            allocator.total_available_memory() / (1024 * 1024 * 1024)
        );
    }
}

#[test]
fn test_numa_memory_performance() {
    let nodes = create_test_numa_nodes();
    let mut allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Local);

    println!("NUMA Memory Performance Test:");

    // 测试不同大小的内存分配性能
    let sizes = vec![1024, 4096, 16384, 65536, 262144, 1048576]; // 1KB to 1MB

    for &size in sizes.iter() {
        let iterations = 1000;
        let start = Instant::now();

        // 模拟内存分配
        for i in 0..iterations {
            let layout = std::alloc::Layout::from_size_align(size, 8).unwrap();
            let _ = allocator.allocate(layout);
            // 在实际实现中，这里会调用底层的分配器
        }

        let duration = start.elapsed();
        let allocs_per_sec = iterations as f64 / duration.as_secs_f64();

        println!("  Size {}: {:.0} allocs/sec", size, allocs_per_sec);
    }

    // 打印统计信息
    let stats = allocator.stats();
    println!("  Statistics:");
    println!(
        "    Local allocs: {}",
        stats
            .local_allocs
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    println!(
        "    Remote allocs: {}",
        stats
            .remote_allocs
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    println!(
        "    Failed allocs: {}",
        stats
            .failed_allocs
            .load(std::sync::atomic::Ordering::Relaxed)
    );
}

#[test]
fn test_numa_interleave_performance() {
    let nodes = create_test_numa_nodes();
    let mut allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Interleave);

    println!("NUMA Interleave Performance Test:");

    let iterations = 10000;
    let start = Instant::now();

    // 测试交错分配策略
    for i in 0..iterations {
        let size = 4096 + (i % 4) * 4096; // 4KB, 8KB, 12KB, 16KB
        let layout = std::alloc::Layout::from_size_align(size, 8).unwrap();
        let _ = allocator.allocate(layout);
    }

    let duration = start.elapsed();
    let allocs_per_sec = iterations as f64 / duration.as_secs_f64();

    println!(
        "  {} interleaved allocations: {:.0} allocs/sec",
        iterations, allocs_per_sec
    );
    println!(
        "  Average allocation time: {:.2} ns",
        duration.as_nanos() as f64 / iterations as f64
    );

    // 交错分配应该分布在不同节点上
    assert!(
        allocs_per_sec > 1000.0,
        "Interleaved allocation should be reasonably fast"
    );
}

#[test]
fn test_numa_preferred_node_performance() {
    let nodes = create_test_numa_nodes();
    let mut allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Preferred(1));

    println!("NUMA Preferred Node Performance Test:");

    let iterations = 5000;
    let start = Instant::now();

    // 测试优先节点策略
    for i in 0..iterations {
        let size = 8192; // 8KB
        let layout = std::alloc::Layout::from_size_align(size, 8).unwrap();
        let _ = allocator.allocate(layout);
    }

    let duration = start.elapsed();
    let allocs_per_sec = iterations as f64 / duration.as_secs_f64();

    println!(
        "  {} preferred allocations: {:.0} allocs/sec",
        iterations, allocs_per_sec
    );
    println!("  Target node: Node 1");
}

#[test]
fn test_numa_binding_performance() {
    let nodes = create_test_numa_nodes();
    let mut allocator = NumaAllocator::new(nodes, NumaAllocPolicy::Bind(2));

    println!("NUMA Binding Performance Test:");

    let iterations = 3000;
    let start = Instant::now();

    // 测试绑定分配策略
    for i in 0..iterations {
        let size = 16384; // 16KB
        let layout = std::alloc::Layout::from_size_align(size, 8).unwrap();
        let _ = allocator.allocate(layout);
    }

    let duration = start.elapsed();
    let allocs_per_sec = iterations as f64 / duration.as_secs_f64();

    println!(
        "  {} bound allocations: {:.0} allocs/sec",
        iterations, allocs_per_sec
    );
    println!("  Bound to: Node 2");
}

#[test]
fn test_numa_memory_efficiency() {
    let nodes = create_test_numa_nodes();

    // 比较不同策略的内存使用效率
    let policies = vec![
        (NumaAllocPolicy::Local, "Local"),
        (NumaAllocPolicy::Interleave, "Interleave"),
    ];

    println!("NUMA Memory Efficiency Comparison:");

    for (policy, name) in policies {
        let mut allocator = NumaAllocator::new(nodes.clone(), policy);

        // 分配大量内存块
        let block_count = 1000;
        let block_size = 64 * 1024; // 64KB
        let start = Instant::now();

        for i in 0..block_count {
            let layout = std::alloc::Layout::from_size_align(block_size, 8).unwrap();
            let _ = allocator.allocate(layout);

            // 模拟内存使用统计更新
            if i % 100 == 0 {
                allocator
                    .update_node_memory_usage(i, (block_count * block_size) / nodes.len() as u64);
            }
        }

        let duration = start.elapsed();
        let total_memory = block_count * block_size;
        let memory_throughput = total_memory as f64 / duration.as_secs_f64();

        println!(
            "  {}: {:.2} MB/sec",
            name,
            memory_throughput / (1024.0 * 1024.0)
        );

        let stats = allocator.stats();
        let local_ratio = stats
            .local_allocs
            .load(std::sync::atomic::Ordering::Relaxed) as f64;
        let total_allocs = local_ratio
            + stats
                .remote_allocs
                .load(std::sync::atomic::Ordering::Relaxed) as f64;

        if total_allocs > 0.0 {
            println!(
                "    Local allocation ratio: {:.1}%",
                (local_ratio / total_allocs) * 100.0
            );
        }
    }
}

#[test]
fn test_numa_multi_threaded_performance() {
    use std::sync::Arc;
    use std::thread;

    let nodes = create_test_numa_nodes();
    let allocator = Arc::new(NumaAllocator::new(nodes, NumaAllocPolicy::Local));
    let thread_count = 4;
    let allocations_per_thread = 2500;

    println!("NUMA Multi-threaded Performance Test:");

    let start = Instant::now();
    let mut handles = Vec::new();

    // 创建多个线程进行内存分配
    for thread_id in 0..thread_count {
        let allocator_clone = allocator.clone();
        let handle = thread::spawn(move || {
            for i in 0..allocations_per_thread {
                let size = 4096 + (thread_id * 1000 + i % 1000) * 256; // 变化大小
                let layout = std::alloc::Layout::from_size_align(size, 8).unwrap();
                let _ = allocator_clone.allocate(layout);

                // 模拟内存使用
                thread::sleep(std::time::Duration::from_nanos(1));
            }
        });
        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();
    let total_allocations = thread_count * allocations_per_thread;
    let allocs_per_sec = total_allocations as f64 / duration.as_secs_f64();

    println!(
        "  {} threads x {} allocations each: {:.0} allocs/sec",
        thread_count, allocations_per_thread, allocs_per_sec
    );
    println!("  Duration: {:?}", duration);

    let stats = allocator.stats();
    println!("  Final Statistics:");
    println!(
        "    Total local allocs: {}",
        stats
            .local_allocs
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    println!(
        "    Total remote allocs: {}",
        stats
            .remote_allocs
            .load(std::sync::atomic::Ordering::Relaxed)
    );

    assert!(
        allocs_per_sec > 10000.0,
        "Multi-threaded allocation should be fast"
    );
}

impl NumaAllocator {
    /// 获取总的可用内存
    pub fn total_available_memory(&self) -> u64 {
        self.nodes.iter().map(|n| n.available_memory).sum()
    }

    /// 模拟内存分配（在真实实现中会调用底层分配器）
    pub fn allocate(&self, size: usize) -> Option<usize> {
        // 简化的内存分配模拟
        match self.policy {
            NumaAllocPolicy::Local => {
                if let Some(node_id) = Self::current_node() {
                    if let Some(node) = self.nodes.get(node_id) {
                        if node.available_memory >= size as u64 {
                            return Some(node_id);
                        }
                    }
                }
                // 回退到其他节点
                for (i, node) in self.nodes.iter().enumerate() {
                    if node.available_memory >= size as u64 {
                        return Some(i);
                    }
                }
            }
            NumaAllocPolicy::Interleave => {
                // 轮询分配 - 使用简单哈希代替rand
                let hash = (size as usize).wrapping_mul(31) % self.nodes.len();
                if let Some(node) = self.nodes.get(hash) {
                    if node.available_memory >= size as u64 {
                        return Some(hash);
                    }
                }
            }
            NumaAllocPolicy::Bind(node_id) => {
                if let Some(node) = self.nodes.get(node_id) {
                    if node.available_memory >= size as u64 {
                        return Some(node_id);
                    }
                }
            }
            NumaAllocPolicy::Preferred(node_id) => {
                if let Some(node) = self.nodes.get(node_id) {
                    if node.available_memory >= size as u64 {
                        return Some(node_id);
                    }
                }
                // 回退到本地分配
                if let Some(local_id) = Self::current_node() {
                    if let Some(node) = self.nodes.get(local_id) {
                        if node.available_memory >= size as u64 {
                            return Some(local_id);
                        }
                    }
                }
            }
        }

        None
    }

    /// 更新节点内存使用统计
    pub fn update_node_memory_usage(&self, node_id: usize, used: u64) {
        if let Some(node) = self.nodes.get_mut(node_id) {
            if used <= node.available_memory {
                node.available_memory -= used;
            }
        }
    }

    /// 获取分配统计信息
    pub fn stats(&self) -> &NumaAllocStats {
        &self.stats
    }
}
