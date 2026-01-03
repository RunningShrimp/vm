//! P1-02: 协程调度器
//!
//! GMP风格的协程调度器实现,支持work stealing和负载均衡

use std::collections::VecDeque;
use std::sync::Arc;

use parking_lot::Mutex;

/// 协程ID
pub type CoroutineId = u64;

/// 协程状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoroutineState {
    /// 刚创建
    Created,
    /// 就绪
    Ready,
    /// 运行中
    Running,
    /// 等待
    Waiting,
    /// 暂停
    Suspended,
    /// 已完成
    Dead,
}

/// 协程结构
#[derive(Debug, Clone)]
pub struct Coroutine {
    /// 协程ID
    pub id: CoroutineId,
    /// 状态
    pub state: CoroutineState,
    /// 执行计数
    pub execution_count: u64,
    /// 执行时间(微秒)
    pub total_time_us: u64,
}

impl Coroutine {
    /// 创建新协程
    pub fn new(id: CoroutineId) -> Self {
        Self {
            id,
            state: CoroutineState::Created,
            execution_count: 0,
            total_time_us: 0,
        }
    }

    /// 标记为就绪
    pub fn mark_ready(&mut self) {
        self.state = CoroutineState::Ready;
    }

    /// 标记为运行中
    pub fn mark_running(&mut self) {
        self.state = CoroutineState::Running;
    }

    /// 记录执行时间
    pub fn record_execution(&mut self, time_us: u64) {
        self.execution_count += 1;
        self.total_time_us += time_us;
    }

    /// 获取平均执行时间
    pub fn avg_exec_time(&self) -> u64 {
        if self.execution_count == 0 {
            0
        } else {
            self.total_time_us / self.execution_count
        }
    }
}

/// vCPU状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VCPUState {
    /// 空闲
    Idle,
    /// 运行中
    Running,
    /// 等待任务
    WaitingForWork,
    /// 已暂停
    Halted,
}

/// vCPU统计
#[derive(Debug, Clone)]
pub struct VCPUStats {
    /// 执行数
    pub executions: u64,
    /// 上下文切换数
    pub context_switches: u64,
    /// 空闲时间(微秒)
    pub idle_time_us: u64,
    /// 忙碌时间(微秒)
    pub busy_time_us: u64,
}

impl VCPUStats {
    pub fn new() -> Self {
        Self {
            executions: 0,
            context_switches: 0,
            idle_time_us: 0,
            busy_time_us: 0,
        }
    }

    /// 获取利用率(0.0-1.0)
    pub fn utilization(&self) -> f64 {
        let total = self.idle_time_us + self.busy_time_us;
        if total == 0 {
            0.0
        } else {
            self.busy_time_us as f64 / total as f64
        }
    }
}

impl Default for VCPUStats {
    fn default() -> Self {
        Self::new()
    }
}

/// vCPU结构
#[derive(Debug)]
pub struct VCPU {
    /// vCPU ID
    pub id: u32,
    /// 当前协程
    pub current_coroutine: Option<Coroutine>,
    /// 本地就绪队列
    pub local_queue: VecDeque<Coroutine>,
    /// 状态
    pub state: VCPUState,
    /// 统计信息
    pub stats: VCPUStats,
}

impl VCPU {
    /// 创建新的vCPU
    pub fn new(id: u32) -> Self {
        Self {
            id,
            current_coroutine: None,
            local_queue: VecDeque::new(),
            state: VCPUState::Idle,
            stats: VCPUStats::new(),
        }
    }

    /// 入队协程
    pub fn enqueue(&mut self, coro: Coroutine) {
        self.local_queue.push_back(coro);
    }

    /// 出队协程
    pub fn dequeue(&mut self) -> Option<Coroutine> {
        self.local_queue.pop_front()
    }

    /// 设置当前协程
    pub fn set_current(&mut self, coro: Coroutine) {
        self.current_coroutine = Some(coro);
        self.state = VCPUState::Running;
        self.stats.executions += 1;
    }

    /// 清除当前协程
    pub fn clear_current(&mut self) {
        self.current_coroutine = None;
        if self.local_queue.is_empty() {
            self.state = VCPUState::Idle;
        }
    }

    /// 获取队列长度
    pub fn queue_length(&self) -> usize {
        self.local_queue.len()
    }

    /// 记录忙碌时间
    pub fn record_busy_time(&mut self, time_us: u64) {
        self.stats.busy_time_us += time_us;
    }

    /// 记录空闲时间
    pub fn record_idle_time(&mut self, time_us: u64) {
        self.stats.idle_time_us += time_us;
    }
}

/// 全局调度器
pub struct Scheduler {
    /// vCPU列表
    vcpus: Vec<VCPU>,
    /// 全局就绪队列
    global_queue: Arc<Mutex<VecDeque<Coroutine>>>,
    /// 是否启用work stealing
    work_stealing_enabled: bool,
    /// 下一个协程ID
    next_coro_id: Arc<Mutex<CoroutineId>>,
}

impl Scheduler {
    /// 创建新的调度器
    pub fn new(vcpu_count: u32) -> Self {
        let mut vcpus = Vec::new();
        for i in 0..vcpu_count {
            vcpus.push(VCPU::new(i));
        }

        Self {
            vcpus,
            global_queue: Arc::new(Mutex::new(VecDeque::new())),
            work_stealing_enabled: true,
            next_coro_id: Arc::new(Mutex::new(0)),
        }
    }

    /// 创建协程
    pub fn create_coroutine(&self) -> Coroutine {
        let id = {
            let mut next_id = self.next_coro_id.lock();
            let id = *next_id;
            *next_id += 1;
            id
        };
        Coroutine::new(id)
    }

    /// 提交协程到全局队列
    pub fn submit_coroutine(&self, coro: Coroutine) {
        let mut queue = self.global_queue.lock();
        queue.push_back(coro);
    }

    /// 分配协程到vCPU
    pub fn assign_to_vcpu(&mut self, vcpu_id: u32, coro: Coroutine) -> Result<(), String> {
        if vcpu_id as usize >= self.vcpus.len() {
            return Err("Invalid vCPU ID".to_string());
        }
        self.vcpus[vcpu_id as usize].enqueue(coro);
        Ok(())
    }

    /// 获取vCPU的下一个协程
    pub fn next_coroutine(&mut self, vcpu_id: u32) -> Option<Coroutine> {
        if vcpu_id as usize >= self.vcpus.len() {
            return None;
        }

        let vcpu = &mut self.vcpus[vcpu_id as usize];

        // 先尝试本地队列
        if let Some(coro) = vcpu.dequeue() {
            return Some(coro);
        }

        // 如果启用了work stealing,尝试从全局队列窃取
        if self.work_stealing_enabled {
            let mut queue = self.global_queue.lock();
            if let Some(coro) = queue.pop_front() {
                return Some(coro);
            }
        }

        None
    }

    /// 尝试从其他vCPU窃取任务
    pub fn try_steal_work(&mut self, from_vcpu_id: u32) -> Option<Coroutine> {
        if from_vcpu_id as usize >= self.vcpus.len() {
            return None;
        }

        // 轮转其他vCPU寻找可以窃取的任务
        let vcpu_count = self.vcpus.len();
        for offset in 1..vcpu_count {
            let target_id = ((from_vcpu_id as usize + offset) % vcpu_count) as u32;
            if let Some(coro) = self.vcpus[target_id as usize].dequeue() {
                return Some(coro);
            }
        }

        None
    }

    /// 获取vCPU数量
    pub fn vcpu_count(&self) -> u32 {
        self.vcpus.len() as u32
    }

    /// 获取vCPU统计
    pub fn get_vcpu_stats(&self, vcpu_id: u32) -> Option<VCPUStats> {
        if (vcpu_id as usize) < self.vcpus.len() {
            Some(self.vcpus[vcpu_id as usize].stats.clone())
        } else {
            None
        }
    }

    /// 获取全局队列长度
    pub fn global_queue_length(&self) -> usize {
        self.global_queue.lock().len()
    }

    /// 设置work stealing状态
    pub fn set_work_stealing(&mut self, enabled: bool) {
        self.work_stealing_enabled = enabled;
    }

    /// 计算负载均衡指标
    pub fn calculate_load_imbalance(&self) -> f64 {
        if self.vcpus.is_empty() {
            return 0.0;
        }

        let loads: Vec<usize> = self.vcpus.iter().map(|v| v.queue_length()).collect();
        let avg = loads.iter().sum::<usize>() as f64 / loads.len() as f64;
        let variance =
            loads.iter().map(|&l| (l as f64 - avg).powi(2)).sum::<f64>() / loads.len() as f64;

        variance.sqrt()
    }

    /// 获取调度器统计
    pub fn get_stats(&self) -> SchedulerStats {
        SchedulerStats {
            vcpu_count: self.vcpus.len() as u32,
            total_coroutines_created: *self.next_coro_id.lock(),
            global_queue_length: self.global_queue_length(),
            vcpu_stats: self.vcpus.iter().map(|v| v.stats.clone()).collect(),
            load_imbalance: self.calculate_load_imbalance(),
        }
    }
}

/// 调度器统计
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    pub vcpu_count: u32,
    pub total_coroutines_created: u64,
    pub global_queue_length: usize,
    pub vcpu_stats: Vec<VCPUStats>,
    pub load_imbalance: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coroutine_creation() {
        let coro = Coroutine::new(1);
        assert_eq!(coro.id, 1);
        assert_eq!(coro.state, CoroutineState::Created);
        assert_eq!(coro.execution_count, 0);
    }

    #[test]
    fn test_coroutine_state_transitions() {
        let mut coro = Coroutine::new(1);

        coro.mark_ready();
        assert_eq!(coro.state, CoroutineState::Ready);

        coro.mark_running();
        assert_eq!(coro.state, CoroutineState::Running);
    }

    #[test]
    fn test_vcpu_creation() {
        let vcpu = VCPU::new(0);
        assert_eq!(vcpu.id, 0);
        assert_eq!(vcpu.state, VCPUState::Idle);
        assert_eq!(vcpu.queue_length(), 0);
    }

    #[test]
    fn test_vcpu_enqueue_dequeue() {
        let mut vcpu = VCPU::new(0);
        let coro = Coroutine::new(1);

        vcpu.enqueue(coro.clone());
        assert_eq!(vcpu.queue_length(), 1);

        let dequeued = vcpu.dequeue();
        assert!(dequeued.is_some());
        assert_eq!(dequeued.expect("Dequeued coroutine should exist").id, 1);
        assert_eq!(vcpu.queue_length(), 0);
    }

    #[test]
    fn test_scheduler_creation() {
        let scheduler = Scheduler::new(4);
        assert_eq!(scheduler.vcpu_count(), 4);
    }

    #[test]
    fn test_scheduler_create_coroutine() {
        let scheduler = Scheduler::new(4);
        let coro1 = scheduler.create_coroutine();
        let coro2 = scheduler.create_coroutine();

        assert_eq!(coro1.id, 0);
        assert_eq!(coro2.id, 1);
    }

    #[test]
    fn test_scheduler_submit_and_steal() {
        let mut scheduler = Scheduler::new(4);
        let coro = scheduler.create_coroutine();

        scheduler.submit_coroutine(coro);
        assert_eq!(scheduler.global_queue_length(), 1);

        let stolen = scheduler.next_coroutine(0);
        assert!(stolen.is_some());
        assert_eq!(scheduler.global_queue_length(), 0);
    }

    #[test]
    fn test_vcpu_assignment() {
        let mut scheduler = Scheduler::new(4);
        let coro = scheduler.create_coroutine();

        let result = scheduler.assign_to_vcpu(0, coro);
        assert!(result.is_ok());

        let next = scheduler.next_coroutine(0);
        assert!(next.is_some());
    }

    #[test]
    fn test_work_stealing() {
        let mut scheduler = Scheduler::new(2);
        scheduler.set_work_stealing(true);

        // 给vCPU0分配协程
        let coro1 = scheduler.create_coroutine();
        let coro2 = scheduler.create_coroutine();

        let _ = scheduler.assign_to_vcpu(0, coro1);
        let _ = scheduler.assign_to_vcpu(0, coro2);

        // vCPU1从vCPU0窃取
        let stolen = scheduler.try_steal_work(1);
        assert!(stolen.is_some());
    }

    #[test]
    fn test_vcpu_stats() {
        let mut vcpu = VCPU::new(0);
        vcpu.record_busy_time(100);
        vcpu.record_idle_time(50);

        let stats = &vcpu.stats;
        assert_eq!(stats.busy_time_us, 100);
        assert_eq!(stats.idle_time_us, 50);

        let utilization = stats.utilization();
        assert!((utilization - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_load_imbalance_calculation() {
        let mut scheduler = Scheduler::new(4);

        // 不均衡的负载
        let _ = scheduler.assign_to_vcpu(0, scheduler.create_coroutine());
        let _ = scheduler.assign_to_vcpu(0, scheduler.create_coroutine());
        let _ = scheduler.assign_to_vcpu(0, scheduler.create_coroutine());
        let _ = scheduler.assign_to_vcpu(1, scheduler.create_coroutine());

        let imbalance = scheduler.calculate_load_imbalance();
        assert!(imbalance > 0.0);
    }

    #[test]
    fn test_scheduler_stats() {
        let scheduler = Scheduler::new(4);

        for _ in 0..5 {
            let coro = scheduler.create_coroutine();
            scheduler.submit_coroutine(coro);
        }

        let stats = scheduler.get_stats();
        assert_eq!(stats.vcpu_count, 4);
        assert_eq!(stats.total_coroutines_created, 5);
        assert_eq!(stats.global_queue_length, 5);
    }
}
