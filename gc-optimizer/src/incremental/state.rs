use std::time::Instant;

/// 增量式 GC 状态
#[derive(Debug, Clone)]
pub struct IncrementalState {
    /// 总回收次数
    pub total_collections: u64,
    /// 总暂停时间（微秒）
    pub total_pause_time_us: u64,
    /// 标记步骤数
    pub marking_steps: u64,
    /// 清除步骤数
    pub sweeping_steps: u64,
    /// 标记的对象数
    pub objects_marked: u64,
    /// 清除的对象数
    pub objects_swept: u64,
    /// 收集的字节数
    pub bytes_collected: u64,
    /// 当前回收开始时间
    pub collection_start: Option<Instant>,
    /// 当前回收是否活跃
    pub collection_active: bool,
}

impl IncrementalState {
    /// 创建新的状态
    pub fn new() -> Self {
        Self {
            total_collections: 0,
            total_pause_time_us: 0,
            marking_steps: 0,
            sweeping_steps: 0,
            objects_marked: 0,
            objects_swept: 0,
            bytes_collected: 0,
            collection_start: None,
            collection_active: false,
        }
    }

    /// 开始新的回收周期
    pub fn start_collection(&mut self) {
        self.collection_active = true;
        self.collection_start = Some(Instant::now());
        self.marking_steps = 0;
        self.sweeping_steps = 0;
        self.objects_marked = 0;
        self.objects_swept = 0;
        self.bytes_collected = 0;
    }

    /// 完成回收周期
    pub fn finish_collection(&mut self) {
        if let Some(start) = self.collection_start {
            let pause_us = start.elapsed().as_micros() as u64;
            self.total_pause_time_us += pause_us;
        }
        self.total_collections += 1;
        self.collection_active = false;
        self.collection_start = None;
    }

    /// 记录标记步骤
    pub fn record_marking_step(&mut self, objects: u64, bytes: u64) {
        self.marking_steps += 1;
        self.objects_marked += objects;
    }

    /// 记录清除步骤
    pub fn record_sweeping_step(&mut self, objects: u64, bytes: u64) {
        self.sweeping_steps += 1;
        self.objects_swept += objects;
        self.bytes_collected += bytes;
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.total_collections = 0;
        self.total_pause_time_us = 0;
        self.marking_steps = 0;
        self.sweeping_steps = 0;
        self.objects_marked = 0;
        self.objects_swept = 0;
        self.bytes_collected = 0;
    }

    /// 获取当前回收的暂停时间
    pub fn current_pause_us(&self) -> Option<u64> {
        self.collection_start.map(|start| start.elapsed().as_micros() as u64)
    }
}

impl Default for IncrementalState {
    fn default() -> Self {
        Self::new()
    }
}
