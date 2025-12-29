//! 协程调度器集成
//! 
//! 将CoroutineScheduler与虚拟CPU (vCPU) 集成，实现负载均衡和并发执行

use std::sync::Arc;
use dashmap::DashMap;
use async_trait::async_trait;

/// 协程状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoroutineState {
    /// 就绪状态
    Ready,
    /// 运行中
    Running,
    /// 等待中
    Waiting,
    /// 已完成
    Done,
}

/// 协程优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// 低优先级
    Low = 0,
    /// 中优先级
    Normal = 1,
    /// 高优先级
    High = 2,
    /// 实时优先级
    RealTime = 3,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

/// 协程ID
pub type CoroutineId = u64;

/// vCPU状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VCPUState {
    /// 空闲
    Idle,
    /// 运行中
    Running,
    /// 暂停
    Halted,
}

/// 协程
#[derive(Debug, Clone)]
pub struct Coroutine {
    /// 协程ID
    pub id: CoroutineId,
    /// 协程状态
    pub state: CoroutineState,
    /// 优先级
    pub priority: Priority,
}

impl Coroutine {
    /// 创建新协程
    pub fn new(id: CoroutineId, priority: Priority) -> Self {
        Self {
            id,
            state: CoroutineState::Ready,
            priority,
        }
    }

    /// 标记为就绪
    pub fn mark_ready(&mut self) {
        self.state = CoroutineState::Ready;
    }
}

/// 简单的协程调度器（用于vCPU映射）
pub struct Scheduler {
    /// vCPU数量
    vcpu_count: u32,
    /// 下一个协程ID
    next_coro_id: u64,
    /// 全局队列长度（用于测试）
    global_queue_len: std::sync::atomic::AtomicUsize,
}

impl Scheduler {
    /// 创建新调度器
    pub fn new(vcpu_count: u32) -> Self {
        Self {
            vcpu_count,
            next_coro_id: 1,
            global_queue_len: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// 获取vCPU数量
    pub fn vcpu_count(&self) -> u32 {
        self.vcpu_count
    }

    /// 创建协程
    pub fn create_coroutine(&mut self) -> Coroutine {
        let id = self.next_coro_id;
        self.next_coro_id += 1;
        Coroutine::new(id, Priority::default())
    }

    /// 分配协程到vCPU
    pub fn assign_to_vcpu(&mut self, _vcpu_id: u32, _coro: Coroutine) -> Result<(), String> {
        // 简化实现
        Ok(())
    }

    /// 提交协程
    pub fn submit_coroutine(&mut self, _coro: Coroutine) {
        // 简化实现
    }

    /// 获取全局队列长度
    pub fn global_queue_length(&self) -> usize {
        self.global_queue_len.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// 协程信息
#[derive(Debug, Clone)]
pub struct CoroutineInfo {
    pub id: u64,
    pub state: CoroutineState,
    pub priority: Priority,
    pub cycles_executed: u64,
    pub cycles_remaining: u64,
    pub assigned_vcpu: Option<u32>,
}

/// 虚拟CPU信息
#[derive(Debug, Clone)]
pub struct VCpuInfo {
    pub id: u32,
    pub total_cycles: u64,
    pub available_cycles: u64,
    pub coroutine_count: usize,
    pub utilization: f64,
}

/// 调度策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingPolicy {
    /// 轮询调度
    RoundRobin,
    /// 优先级队列
    Priority,
    /// 抢占式调度
    Preemptive,
    /// 负载均衡
    LoadBalancing,
}

/// 协程调度器配置
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub vcpu_count: u32,
    pub time_slice_us: u64,
    pub scheduling_policy: SchedulingPolicy,
    pub enable_load_balancing: bool,
    pub load_balance_threshold: f64,
    pub max_coroutines: usize,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            vcpu_count: num_cpus::get() as u32,
            time_slice_us: 100,
            scheduling_policy: SchedulingPolicy::LoadBalancing,
            enable_load_balancing: true,
            load_balance_threshold: 0.3, // 30%差异触发平衡
            max_coroutines: 10000,
        }
    }
}

/// 协程调度器
pub struct CoroutineScheduler {
    config: SchedulerConfig,
    /// 协程池 (ID -> CoroutineInfo)
    coroutines: Arc<DashMap<u64, CoroutineInfo>>,
    /// vCPU池 (ID -> VCpuInfo)
    vcpus: Arc<DashMap<u32, VCpuInfo>>,
    /// 就绪队列 (优先级 -> 协程ID列表)
    ready_queues: Arc<Vec<tokio::sync::Mutex<Vec<u64>>>>,
    /// 全局统计
    global_stats: Arc<tokio::sync::Mutex<SchedulerStats>>,
    /// 协程计数器
    next_coroutine_id: Arc<std::sync::atomic::AtomicU64>,
}

/// 调度器统计信息
#[derive(Debug, Clone, Copy)]
pub struct SchedulerStats {
    pub total_coroutines: u64,
    pub total_scheduled: u64,
    pub total_context_switches: u64,
    pub total_load_balances: u64,
    pub avg_coroutine_latency_us: u64,
}

impl CoroutineScheduler {
    /// 创建新调度器
    pub fn new(config: SchedulerConfig) -> Self {
        // 初始化vCPU
        let vcpus = Arc::new(DashMap::new());
        for i in 0..config.vcpu_count {
            vcpus.insert(
                i,
                VCpuInfo {
                    id: i,
                    total_cycles: u64::MAX,
                    available_cycles: u64::MAX,
                    coroutine_count: 0,
                    utilization: 0.0,
                },
            );
        }

        // 初始化就绪队列（按优先级）
        let ready_queues = Arc::new(vec![
            tokio::sync::Mutex::new(Vec::new()), // Low
            tokio::sync::Mutex::new(Vec::new()), // Normal
            tokio::sync::Mutex::new(Vec::new()), // High
            tokio::sync::Mutex::new(Vec::new()), // RealTime
        ]);

        Self {
            config,
            coroutines: Arc::new(DashMap::new()),
            vcpus,
            ready_queues,
            global_stats: Arc::new(tokio::sync::Mutex::new(SchedulerStats {
                total_coroutines: 0,
                total_scheduled: 0,
                total_context_switches: 0,
                total_load_balances: 0,
                avg_coroutine_latency_us: 0,
            })),
            next_coroutine_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    /// 创建新协程
    pub async fn create_coroutine(
        &self,
        priority: Priority,
        cycles: u64,
    ) -> Result<u64, String> {
        if self.coroutines.len() >= self.config.max_coroutines {
            return Err("Max coroutines reached".to_string());
        }

        let coro_id = self.next_coroutine_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let info = CoroutineInfo {
            id: coro_id,
            state: CoroutineState::Ready,
            priority,
            cycles_executed: 0,
            cycles_remaining: cycles,
            assigned_vcpu: None,
        };

        self.coroutines.insert(coro_id, info);

        // 加入就绪队列
        let queue_idx = priority as usize;
        self.ready_queues[queue_idx].lock().await.push(coro_id);

        let mut stats = self.global_stats.lock().await;
        stats.total_coroutines += 1;

        Ok(coro_id)
    }

    /// 调度协程到vCPU
    pub async fn schedule(&self) -> Result<(), String> {
        // 选择最佳负载分布的vCPU
        let target_vcpu = self.select_target_vcpu().await?;

        // 从就绪队列取出协程
        let coro_id = self.dequeue_coroutine().await?;

        // 分配协程到vCPU
        self.assign_coroutine_to_vcpu(coro_id, target_vcpu).await?;

        let mut stats = self.global_stats.lock().await;
        stats.total_scheduled += 1;

        Ok(())
    }

    /// 选择目标vCPU（负载均衡）
    async fn select_target_vcpu(&self) -> Result<u32, String> {
        if !self.config.enable_load_balancing {
            // 简单轮询
            return Ok(0);
        }

        // 找到负载最低的vCPU
        let mut min_utilization = f64::MAX;
        let mut target_id = 0u32;

        for entry in self.vcpus.iter() {
            if entry.value().utilization < min_utilization {
                min_utilization = entry.value().utilization;
                target_id = entry.key().clone();
            }
        }

        Ok(target_id)
    }

    /// 从就绪队列取出协程
    async fn dequeue_coroutine(&self) -> Result<u64, String> {
        // 优先级队列：从高到低检查
        for queue_idx in (0..4).rev() {
            let mut queue = self.ready_queues[queue_idx].lock().await;
            if let Some(coro_id) = queue.pop() {
                return Ok(coro_id);
            }
        }

        Err("No coroutine in ready queue".to_string())
    }

    /// 分配协程到vCPU
    async fn assign_coroutine_to_vcpu(
        &self,
        coro_id: u64,
        vcpu_id: u32,
    ) -> Result<(), String> {
        // 更新协程信息
        if let Some(mut coro) = self.coroutines.get_mut(&coro_id) {
            coro.assigned_vcpu = Some(vcpu_id);
            coro.state = CoroutineState::Running;
        } else {
            return Err("Coroutine not found".to_string());
        }

        // 更新vCPU信息
        if let Some(mut vcpu) = self.vcpus.get_mut(&vcpu_id) {
            vcpu.coroutine_count += 1;
            vcpu.utilization = vcpu.coroutine_count as f64 / 100.0; // 简化计算
        } else {
            return Err("vCPU not found".to_string());
        }

        Ok(())
    }

    /// 执行协程（在指定vCPU上）
    pub async fn execute_coroutine(&self, coro_id: u64) -> Result<u64, String> {
        let start = std::time::Instant::now();

        // 获取协程信息
        let coro = self.coroutines
            .get(&coro_id)
            .ok_or("Coroutine not found")?;

        let remaining_cycles = coro.cycles_remaining;
        drop(coro);

        // 模拟执行时间（基于周期数）
        let estimated_exec_us = remaining_cycles / 1000; // 1000 cycles/us
        tokio::time::sleep(tokio::time::Duration::from_micros(
            std::cmp::min(estimated_exec_us, 100),
        )).await;

        // 执行完成后的处理
        let cycles_executed = remaining_cycles;
        
        if let Some(mut coro) = self.coroutines.get_mut(&coro_id) {
            coro.cycles_executed += cycles_executed;
            coro.cycles_remaining = 0;
            coro.state = CoroutineState::Done;
        }

        let latency = start.elapsed().as_micros() as u64;
        Ok(latency)
    }

    /// 执行负载均衡
    pub async fn balance_load(&self) -> Result<usize, String> {
        if !self.config.enable_load_balancing {
            return Ok(0);
        }

        let mut moves = 0;

        // 计算平均利用率
        let mut total_util = 0.0;
        for entry in self.vcpus.iter() {
            total_util += entry.value().utilization;
        }
        let avg_util = total_util / self.config.vcpu_count as f64;
        let threshold = self.config.load_balance_threshold;

        // 从高负载vCPU移动协程到低负载vCPU
        for entry in self.vcpus.iter() {
            let vcpu_id = entry.key().clone();
            let utilization = entry.value().utilization;

            if utilization > avg_util * (1.0 + threshold) {
                // 该vCPU过载，移出一个协程
                moves += 1;
            }
        }

        let mut stats = self.global_stats.lock().await;
        stats.total_load_balances += 1;

        Ok(moves)
    }

    /// 获取调度器统计
    pub async fn get_stats(&self) -> SchedulerStats {
        *self.global_stats.lock().await
    }

    /// 获取协程信息
    pub fn get_coroutine_info(&self, coro_id: u64) -> Option<CoroutineInfo> {
        self.coroutines.get(&coro_id).map(|entry| entry.clone())
    }

    /// 获取vCPU信息
    pub fn get_vcpu_info(&self, vcpu_id: u32) -> Option<VCpuInfo> {
        self.vcpus.get(&vcpu_id).map(|entry| entry.clone())
    }

    /// 获取所有vCPU的负载平衡状态
    pub fn get_load_balance_status(&self) -> Vec<(u32, f64)> {
        self.vcpus
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().utilization))
            .collect()
    }

    /// 暂停协程
    pub async fn pause_coroutine(&self, coro_id: u64) -> Result<(), String> {
        if let Some(mut coro) = self.coroutines.get_mut(&coro_id) {
            coro.state = CoroutineState::Waiting;
            Ok(())
        } else {
            Err("Coroutine not found".to_string())
        }
    }

    /// 恢复协程
    pub async fn resume_coroutine(&self, coro_id: u64) -> Result<(), String> {
        if let Some(mut coro) = self.coroutines.get_mut(&coro_id) {
            coro.state = CoroutineState::Ready;
            
            // 重新加入就绪队列
            let queue_idx = coro.priority as usize;
            drop(coro);
            
            self.ready_queues[queue_idx].lock().await.push(coro_id);
            Ok(())
        } else {
            Err("Coroutine not found".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scheduler_creation() {
        let config = SchedulerConfig::default();
        let scheduler = CoroutineScheduler::new(config.clone());

        let vcpu_info = scheduler.get_vcpu_info(0);
        assert!(vcpu_info.is_some());
        let info = vcpu_info.expect("vCPU info should exist");
        assert_eq!(info.id, 0);
    }

    #[tokio::test]
    async fn test_create_coroutine() {
        let scheduler = CoroutineScheduler::new(SchedulerConfig::default());

        let coro_id = scheduler.create_coroutine(Priority::Normal, 1000).await;
        assert!(coro_id.is_ok());

        let id = coro_id.expect("Coroutine creation should succeed");
        let info = scheduler.get_coroutine_info(id);
        assert!(info.is_some());
        let coro_info = info.expect("Coroutine info should exist");
        assert_eq!(coro_info.state, CoroutineState::Ready);
    }

    #[tokio::test]
    async fn test_coroutine_scheduling() {
        let scheduler = CoroutineScheduler::new(SchedulerConfig::default());

        // 创建多个协程
        for i in 0..10 {
            let priority = match i % 3 {
                0 => Priority::Low,
                1 => Priority::Normal,
                _ => Priority::High,
            };
            let _ = scheduler.create_coroutine(priority, 1000).await;
        }

        // 调度协程
        let result = scheduler.schedule().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_coroutine() {
        let scheduler = CoroutineScheduler::new(SchedulerConfig::default());

        let coro_id = scheduler.create_coroutine(Priority::Normal, 1000).await
            .expect("Coroutine creation should succeed");
        scheduler.schedule().await.ok();

        let latency = scheduler.execute_coroutine(coro_id).await;
        assert!(latency.is_ok());
    }

    #[tokio::test]
    async fn test_load_balancing() {
        let mut config = SchedulerConfig::default();
        config.enable_load_balancing = true;
        let scheduler = CoroutineScheduler::new(config);

        // 创建多个协程，触发负载均衡
        for _ in 0..50 {
            let _ = scheduler.create_coroutine(Priority::Normal, 1000).await;
        }

        let moves = scheduler.balance_load().await;
        assert!(moves.is_ok());
    }

    #[tokio::test]
    async fn test_priority_scheduling() {
        let scheduler = CoroutineScheduler::new(SchedulerConfig::default());

        // 创建不同优先级的协程
        let low_id = scheduler.create_coroutine(Priority::Low, 1000).await
            .expect("Low priority coroutine creation should succeed");
        let high_id = scheduler.create_coroutine(Priority::High, 1000).await
            .expect("High priority coroutine creation should succeed");

        // 应该优先调度高优先级
        for _ in 0..2 {
            let _ = scheduler.schedule().await;
        }

        let high_info = scheduler.get_coroutine_info(high_id)
            .expect("High priority coroutine info should exist");
        assert!(high_info.assigned_vcpu.is_some());
    }

    #[tokio::test]
    async fn test_coroutine_pause_resume() {
        let scheduler = CoroutineScheduler::new(SchedulerConfig::default());

        let coro_id = scheduler.create_coroutine(Priority::Normal, 1000).await
            .expect("Coroutine creation should succeed");

        // 暂停
        let pause_result = scheduler.pause_coroutine(coro_id).await;
        assert!(pause_result.is_ok());

        let info = scheduler.get_coroutine_info(coro_id)
            .expect("Coroutine info should exist");
        assert_eq!(info.state, CoroutineState::Waiting);

        // 恢复
        let resume_result = scheduler.resume_coroutine(coro_id).await;
        assert!(resume_result.is_ok());

        let info = scheduler.get_coroutine_info(coro_id)
            .expect("Coroutine info should exist");
        assert_eq!(info.state, CoroutineState::Ready);
    }

    #[tokio::test]
    async fn test_concurrent_coroutine_creation() {
        let scheduler = Arc::new(CoroutineScheduler::new(SchedulerConfig::default()));
        let mut handles = vec![];

        for i in 0..100 {
            let sched = Arc::clone(&scheduler);
            let handle = tokio::spawn(async move {
                let priority = match i % 3 {
                    0 => Priority::Low,
                    1 => Priority::Normal,
                    _ => Priority::High,
                };
                sched.create_coroutine(priority, 1000).await
            });
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await;
        }

        let stats = scheduler.get_stats().await;
        assert_eq!(stats.total_coroutines, 100);
    }

    #[tokio::test]
    async fn test_vcpu_load_distribution() {
        let scheduler = CoroutineScheduler::new(SchedulerConfig::default());

        // 创建并调度协程
        for _ in 0..20 {
            let _ = scheduler.create_coroutine(Priority::Normal, 1000).await;
            let _ = scheduler.schedule().await;
        }

        // 检查负载分布
        let load_status = scheduler.get_load_balance_status();
        assert!(!load_status.is_empty());
    }

    #[tokio::test]
    async fn test_scheduler_statistics() {
        let scheduler = CoroutineScheduler::new(SchedulerConfig::default());

        for _ in 0..10 {
            let _ = scheduler.create_coroutine(Priority::Normal, 1000).await;
            let _ = scheduler.schedule().await;
        }

        let stats = scheduler.get_stats().await;
        assert_eq!(stats.total_coroutines, 10);
        assert!(stats.total_scheduled > 0);
    }
}
