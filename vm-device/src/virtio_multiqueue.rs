//! VirtIO 多队列支持
//!
//! 实现 VirtIO 设备的多队列功能，提升并行处理能力

use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

/// VirtIO 队列
#[derive(Clone)]
pub struct VirtQueue {
    /// 队列索引
    pub index: u16,
    /// 队列大小
    pub size: u16,
    /// Descriptor table 地址
    pub desc_table: u64,
    /// Available ring 地址
    pub avail_ring: u64,
    /// Used ring 地址
    pub used_ring: u64,
    /// 队列是否启用
    pub enabled: bool,
    /// 上次处理的索引
    pub last_avail_idx: u16,
    /// 已使用的索引
    pub used_idx: u16,
}

impl VirtQueue {
    pub fn new(index: u16, size: u16) -> Self {
        Self {
            index,
            size,
            desc_table: 0,
            avail_ring: 0,
            used_ring: 0,
            enabled: false,
            last_avail_idx: 0,
            used_idx: 0,
        }
    }

    /// 检查队列是否有可用的描述符
    pub fn has_available(&self) -> bool {
        // 简化实现
        self.enabled && self.last_avail_idx < self.size
    }

    /// 获取下一个可用的描述符索引
    pub fn pop_available(&mut self) -> Option<u16> {
        if !self.has_available() {
            return None;
        }

        let idx = self.last_avail_idx;
        self.last_avail_idx = (self.last_avail_idx + 1) % self.size;
        Some(idx)
    }

    /// 将已使用的描述符放回队列
    pub fn push_used(&mut self, _idx: u16, _len: u32) {
        // 简化实现：更新 used_idx
        self.used_idx = (self.used_idx + 1) % self.size;
    }
}

/// 多队列管理器
pub struct MultiQueueManager {
    /// 队列列表
    queues: Vec<Arc<Mutex<VirtQueue>>>,
    /// 队列数量
    num_queues: usize,
}

impl MultiQueueManager {
    /// 创建新的多队列管理器
    pub fn new(num_queues: usize, queue_size: u16) -> Self {
        let mut queues = Vec::new();
        for i in 0..num_queues {
            queues.push(Arc::new(Mutex::new(VirtQueue::new(i as u16, queue_size))));
        }

        Self {
            queues,
            num_queues,
        }
    }

    /// 获取队列数量
    pub fn num_queues(&self) -> usize {
        self.num_queues
    }

    /// 获取指定索引的队列
    pub fn get_queue(&self, index: usize) -> Option<Arc<Mutex<VirtQueue>>> {
        if index < self.num_queues {
            Some(self.queues[index].clone())
        } else {
            None
        }
    }

    /// 启用队列
    pub fn enable_queue(&mut self, index: usize) -> Result<(), String> {
        if index >= self.num_queues {
            return Err(format!("Queue index {} out of range", index));
        }

        if let Ok(mut queue) = self.queues[index].lock() {
            queue.enabled = true;
            Ok(())
        } else {
            Err("Failed to lock queue".to_string())
        }
    }

    /// 禁用队列
    pub fn disable_queue(&mut self, index: usize) -> Result<(), String> {
        if index >= self.num_queues {
            return Err(format!("Queue index {} out of range", index));
        }

        if let Ok(mut queue) = self.queues[index].lock() {
            queue.enabled = false;
            Ok(())
        } else {
            Err("Failed to lock queue".to_string())
        }
    }

    /// 处理所有队列的请求
    pub fn process_all_queues<F>(&self, mut handler: F)
    where
        F: FnMut(u16, &mut VirtQueue),
    {
        for (i, queue) in self.queues.iter().enumerate() {
            if let Ok(mut q) = queue.lock() {
                if q.enabled {
                    handler(i as u16, &mut q);
                }
            }
        }
    }
}

/// VirtIO 网络设备多队列支持
pub struct VirtioNetMultiQueue {
    /// 接收队列
    rx_queues: MultiQueueManager,
    /// 发送队列
    tx_queues: MultiQueueManager,
    /// 控制队列
    ctrl_queue: Arc<Mutex<VirtQueue>>,
}

impl VirtioNetMultiQueue {
    /// 创建新的多队列网络设备
    pub fn new(num_queue_pairs: usize, queue_size: u16) -> Self {
        Self {
            rx_queues: MultiQueueManager::new(num_queue_pairs, queue_size),
            tx_queues: MultiQueueManager::new(num_queue_pairs, queue_size),
            ctrl_queue: Arc::new(Mutex::new(VirtQueue::new(num_queue_pairs as u16 * 2, queue_size))),
        }
    }

    /// 获取接收队列
    pub fn get_rx_queue(&self, index: usize) -> Option<Arc<Mutex<VirtQueue>>> {
        self.rx_queues.get_queue(index)
    }

    /// 获取发送队列
    pub fn get_tx_queue(&self, index: usize) -> Option<Arc<Mutex<VirtQueue>>> {
        self.tx_queues.get_queue(index)
    }

    /// 获取控制队列
    pub fn get_ctrl_queue(&self) -> Arc<Mutex<VirtQueue>> {
        self.ctrl_queue.clone()
    }

    /// 处理接收队列
    pub fn process_rx_queues<F>(&self, handler: F)
    where
        F: FnMut(u16, &mut VirtQueue),
    {
        self.rx_queues.process_all_queues(handler);
    }

    /// 处理发送队列
    pub fn process_tx_queues<F>(&self, handler: F)
    where
        F: FnMut(u16, &mut VirtQueue),
    {
        self.tx_queues.process_all_queues(handler);
    }
}

/// VirtIO 块设备多队列支持
pub struct VirtioBlockMultiQueue {
    /// I/O 队列
    io_queues: MultiQueueManager,
}

impl VirtioBlockMultiQueue {
    /// 创建新的多队列块设备
    pub fn new(num_queues: usize, queue_size: u16) -> Self {
        Self {
            io_queues: MultiQueueManager::new(num_queues, queue_size),
        }
    }

    /// 获取 I/O 队列
    pub fn get_io_queue(&self, index: usize) -> Option<Arc<Mutex<VirtQueue>>> {
        self.io_queues.get_queue(index)
    }

    /// 处理所有 I/O 队列
    pub fn process_io_queues<F>(&self, handler: F)
    where
        F: FnMut(u16, &mut VirtQueue),
    {
        self.io_queues.process_all_queues(handler);
    }
}

/// 队列调度策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueSchedulingPolicy {
    /// 轮询调度
    RoundRobin,
    /// 优先级调度
    Priority,
    /// 负载均衡
    LoadBalancing,
}

/// 队列调度器
pub struct QueueScheduler {
    policy: QueueSchedulingPolicy,
    current_queue: usize,
    queue_weights: Vec<u32>,
}

impl QueueScheduler {
    pub fn new(policy: QueueSchedulingPolicy, num_queues: usize) -> Self {
        Self {
            policy,
            current_queue: 0,
            queue_weights: vec![1; num_queues],
        }
    }

    /// 选择下一个要处理的队列
    pub fn select_next_queue(&mut self) -> usize {
        match self.policy {
            QueueSchedulingPolicy::RoundRobin => {
                let queue = self.current_queue;
                self.current_queue = (self.current_queue + 1) % self.queue_weights.len();
                queue
            }
            QueueSchedulingPolicy::Priority => {
                // 简化实现：选择权重最高的队列
                self.queue_weights
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, &w)| w)
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            }
            QueueSchedulingPolicy::LoadBalancing => {
                // 简化实现：轮询
                let queue = self.current_queue;
                self.current_queue = (self.current_queue + 1) % self.queue_weights.len();
                queue
            }
        }
    }

    /// 设置队列权重
    pub fn set_queue_weight(&mut self, queue: usize, weight: u32) {
        if queue < self.queue_weights.len() {
            self.queue_weights[queue] = weight;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtqueue() {
        let mut queue = VirtQueue::new(0, 256);
        assert!(!queue.enabled);
        
        queue.enabled = true;
        assert!(queue.has_available());
        
        let idx = queue.pop_available();
        assert_eq!(idx, Some(0));
    }

    #[test]
    fn test_multiqueue_manager() {
        let manager = MultiQueueManager::new(4, 256);
        assert_eq!(manager.num_queues(), 4);
        
        let queue = manager.get_queue(0);
        assert!(queue.is_some());
    }

    #[test]
    fn test_queue_scheduler() {
        let mut scheduler = QueueScheduler::new(QueueSchedulingPolicy::RoundRobin, 4);
        
        assert_eq!(scheduler.select_next_queue(), 0);
        assert_eq!(scheduler.select_next_queue(), 1);
        assert_eq!(scheduler.select_next_queue(), 2);
        assert_eq!(scheduler.select_next_queue(), 3);
        assert_eq!(scheduler.select_next_queue(), 0);
    }
}
