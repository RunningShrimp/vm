//! vCPU协程映射器模块
//!
//! 实现vCPU到协程的映射关系管理

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use parking_lot::Mutex;

use crate::coroutine_scheduler::{Coroutine, CoroutineId, Scheduler, VCPUState};

/// vCPU协程映射器
pub struct VcpuCoroutineMapper {
    /// 协程调度器
    scheduler: Arc<Mutex<Scheduler>>,
    /// vCPU到协程的映射 (vCPU ID -> Coroutine ID)
    vcpu_to_coro: HashMap<u32, CoroutineId>,
    /// 协程到vCPU的映射 (Coroutine ID -> vCPU ID)
    coro_to_vcpu: HashMap<CoroutineId, u32>,
    /// 挂起的协程队列
    suspended_coroutines: VecDeque<Coroutine>,
}

impl VcpuCoroutineMapper {
    /// 创建新的vCPU协程映射器
    pub fn new(vcpu_count: u32) -> Self {
        Self {
            scheduler: Arc::new(Mutex::new(Scheduler::new(vcpu_count))),
            vcpu_to_coro: HashMap::new(),
            coro_to_vcpu: HashMap::new(),
            suspended_coroutines: VecDeque::new(),
        }
    }

    /// 创建协程并映射到vCPU
    pub fn create_and_map_coroutine(&mut self, vcpu_id: u32) -> Result<Coroutine, String> {
        let mut scheduler = self.scheduler.lock();
        
        // 检查vCPU是否存在
        if vcpu_id >= scheduler.vcpu_count() {
            return Err(format!("Invalid vCPU ID: {}", vcpu_id));
        }

        // 创建协程
        let coro = scheduler.create_coroutine();
        
        // 分配协程到vCPU
        scheduler.assign_to_vcpu(vcpu_id, coro.clone())?;
        
        // 记录映射关系
        self.vcpu_to_coro.insert(vcpu_id, coro.id);
        self.coro_to_vcpu.insert(coro.id, vcpu_id);
        
        Ok(coro)
    }

    /// 查找vCPU对应的协程
    pub fn get_coroutine_for_vcpu(&self, vcpu_id: u32) -> Option<CoroutineId> {
        self.vcpu_to_coro.get(&vcpu_id).cloned()
    }

    /// 查找协程对应的vCPU
    pub fn get_vcpu_for_coroutine(&self, coro_id: CoroutineId) -> Option<u32> {
        self.coro_to_vcpu.get(&coro_id).cloned()
    }

    /// 将协程从vCPU解绑
    pub fn unmap_coroutine(&mut self, coro_id: CoroutineId) -> Result<(), String> {
        if let Some(vcpu_id) = self.coro_to_vcpu.remove(&coro_id) {
            self.vcpu_to_coro.remove(&vcpu_id);
            Ok(())
        } else {
            Err(format!("Coroutine {} not mapped", coro_id))
        }
    }

    /// 将协程从vCPU解绑并重新分配到新vCPU
    pub fn remap_coroutine(&mut self, coro_id: CoroutineId, new_vcpu_id: u32) -> Result<(), String> {
        let mut scheduler = self.scheduler.lock();
        
        // 检查新vCPU是否存在
        if new_vcpu_id >= scheduler.vcpu_count() {
            return Err(format!("Invalid vCPU ID: {}", new_vcpu_id));
        }

        // 获取当前vCPU
        if let Some(current_vcpu) = self.coro_to_vcpu.get(&coro_id) {
            // 解绑当前映射
            self.vcpu_to_coro.remove(current_vcpu);
        }
        
        // 更新映射关系
        self.vcpu_to_coro.insert(new_vcpu_id, coro_id);
        self.coro_to_vcpu.insert(coro_id, new_vcpu_id);
        
        Ok(())
    }

    /// 获取协程调度器
    pub fn scheduler(&self) -> Arc<Mutex<Scheduler>> {
        self.scheduler.clone()
    }

    /// 暂停协程
    pub fn suspend_coroutine(&mut self, coro: Coroutine) {
        self.suspended_coroutines.push_back(coro);
    }

    /// 恢复暂停的协程
    pub fn resume_coroutines(&mut self) {
        let mut scheduler = self.scheduler.lock();
        
        while let Some(mut coro) = self.suspended_coroutines.pop_front() {
            coro.mark_ready();
            scheduler.submit_coroutine(coro);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vcpu_coroutine_mapper_basic() {
        let mut mapper = VcpuCoroutineMapper::new(4);
        
        // 创建并映射协程到vCPU 0
        let coro1 = mapper.create_and_map_coroutine(0).unwrap();
        assert_eq!(mapper.get_coroutine_for_vcpu(0), Some(coro1.id));
        assert_eq!(mapper.get_vcpu_for_coroutine(coro1.id), Some(0));
        
        // 创建并映射协程到vCPU 1
        let coro2 = mapper.create_and_map_coroutine(1).unwrap();
        assert_eq!(mapper.get_coroutine_for_vcpu(1), Some(coro2.id));
    }

    #[test]
    fn test_vcpu_coroutine_remap() {
        let mut mapper = VcpuCoroutineMapper::new(4);
        
        // 创建并映射协程到vCPU 0
        let coro = mapper.create_and_map_coroutine(0).unwrap();
        assert_eq!(mapper.get_coroutine_for_vcpu(0), Some(coro.id));
        
        // 重新映射到vCPU 1
        mapper.remap_coroutine(coro.id, 1).unwrap();
        assert_eq!(mapper.get_coroutine_for_vcpu(0), None);
        assert_eq!(mapper.get_coroutine_for_vcpu(1), Some(coro.id));
    }

    #[test]
    fn test_vcpu_coroutine_unmap() {
        let mut mapper = VcpuCoroutineMapper::new(4);
        
        // 创建并映射协程到vCPU 0
        let coro = mapper.create_and_map_coroutine(0).unwrap();
        assert_eq!(mapper.get_coroutine_for_vcpu(0), Some(coro.id));
        
        // 解绑协程
        mapper.unmap_coroutine(coro.id).unwrap();
        assert_eq!(mapper.get_coroutine_for_vcpu(0), None);
        assert_eq!(mapper.get_vcpu_for_coroutine(coro.id), None);
    }

    #[test]
    fn test_suspend_resume_coroutines() {
        let mut mapper = VcpuCoroutineMapper::new(4);
        
        // 创建并映射协程到vCPU 0
        let coro = mapper.create_and_map_coroutine(0).unwrap();
        
        // 暂停协程
        mapper.suspend_coroutine(coro);
        
        // 恢复协程
        mapper.resume_coroutines();
        
        // 检查协程是否已经提交到调度器
        let scheduler = mapper.scheduler().lock();
        assert_eq!(scheduler.global_queue_length(), 0); // 因为协程已经分配到vCPU本地队列
    }
}