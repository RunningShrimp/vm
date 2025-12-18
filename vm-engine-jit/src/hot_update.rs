//! JIT代码热更新机制
//!
//! 实现了运行时代码更新和版本管理功能，支持无缝代码替换。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vm_core::GuestAddr;
use crate::code_cache::CodeCache;

/// 热更新配置
#[derive(Debug, Clone)]
pub struct HotUpdateConfig {
    /// 启用热更新
    pub enabled: bool,
    /// 更新检查间隔（毫秒）
    pub check_interval_ms: u64,
    /// 最大并发更新数
    pub max_concurrent_updates: usize,
    /// 更新超时时间（毫秒）
    pub update_timeout_ms: u64,
    /// 启用渐进式更新
    pub enable_incremental_update: bool,
    /// 版本历史大小
    pub version_history_size: usize,
}

impl Default for HotUpdateConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_interval_ms: 1000, // 1秒
            max_concurrent_updates: 4,
            update_timeout_ms: 5000, // 5秒
            enable_incremental_update: true,
            version_history_size: 10,
        }
    }
}

/// 代码版本
#[derive(Debug, Clone)]
pub struct CodeVersion {
    /// 版本号
    pub version: u64,
    /// 代码数据
    pub code: Vec<u8>,
    /// 创建时间
    pub created_at: Instant,
    /// 代码大小
    pub size: usize,
    /// 校验和
    pub checksum: u64,
    /// 更新原因
    pub update_reason: UpdateReason,
}

/// 更新原因
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateReason {
    /// 性能优化
    PerformanceOptimization,
    /// 错误修复
    BugFix,
    /// 功能增强
    FeatureEnhancement,
    /// 安全更新
    SecurityUpdate,
    /// 自适应优化
    AdaptiveOptimization,
}

/// 更新状态
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateStatus {
    /// 等待中
    Pending,
    /// 进行中
    InProgress,
    /// 已完成
    Completed,
    /// 失败
    Failed(String),
    /// 已取消
    Cancelled,
}

/// 更新任务
#[derive(Debug)]
pub struct UpdateTask {
    /// 任务ID
    pub task_id: u64,
    /// 目标地址
    pub target_addr: GuestAddr,
    /// 新代码版本
    pub new_version: CodeVersion,
    /// 当前版本
    pub current_version: Option<CodeVersion>,
    /// 更新状态
    pub status: UpdateStatus,
    /// 创建时间
    pub created_at: Instant,
    /// 开始时间
    pub started_at: Option<Instant>,
    /// 完成时间
    pub completed_at: Option<Instant>,
}

/// 热更新管理器
pub struct HotUpdateManager {
    /// 配置
    config: HotUpdateConfig,
    /// 代码缓存
    code_cache: Arc<Mutex<dyn CodeCache>>,
    /// 版本管理
    versions: Arc<Mutex<HashMap<GuestAddr, VecDeque<CodeVersion>>>>,
    /// 活跃版本映射
    active_versions: Arc<Mutex<HashMap<GuestAddr, CodeVersion>>>,
    /// 更新任务
    update_tasks: Arc<Mutex<HashMap<u64, UpdateTask>>>,
    /// 下一个任务ID
    next_task_id: Arc<Mutex<u64>>,
    /// 最后检查时间
    last_check_time: Arc<Mutex<Instant>>,
    /// 更新统计
    update_stats: Arc<Mutex<HotUpdateStats>>,
}

/// 热更新统计
#[derive(Debug, Clone, Default)]
pub struct HotUpdateStats {
    /// 总更新次数
    pub total_updates: u64,
    /// 成功更新次数
    pub successful_updates: u64,
    /// 失败更新次数
    pub failed_updates: u64,
    /// 平均更新时间（毫秒）
    pub avg_update_time_ms: f64,
    /// 最大更新时间（毫秒）
    pub max_update_time_ms: u64,
    /// 最小更新时间（毫秒）
    pub min_update_time_ms: u64,
    /// 回滚次数
    pub rollbacks: u64,
}

impl HotUpdateManager {
    /// 创建新的热更新管理器
    pub fn new(
        config: HotUpdateConfig,
        code_cache: Arc<Mutex<dyn CodeCache>>,
    ) -> Self {
        Self {
            config,
            code_cache,
            versions: Arc::new(Mutex::new(HashMap::new())),
            active_versions: Arc::new(Mutex::new(HashMap::new())),
            update_tasks: Arc::new(Mutex::new(HashMap::new())),
            next_task_id: Arc::new(Mutex::new(1)),
            last_check_time: Arc::new(Mutex::new(Instant::now())),
            update_stats: Arc::new(Mutex::new(HotUpdateStats::default())),
        }
    }
    
    /// 检查是否需要更新
    pub fn check_for_updates(&self) -> Vec<GuestAddr> {
        if !self.config.enabled {
            return Vec::new();
        }
        
        let now = Instant::now();
        let last_check = *self.last_check_time.lock().unwrap();
        
        if now.duration_since(last_check) < Duration::from_millis(self.config.check_interval_ms) {
            return Vec::new();
        }
        
        *self.last_check_time.lock().unwrap() = now;
        
        // 检查所有活跃版本是否需要更新
        let active_versions = self.active_versions.lock().unwrap();
        let mut update_candidates = Vec::new();
        
        for (&addr, version) in active_versions.iter() {
            if self.should_update(addr, version) {
                update_candidates.push(addr);
            }
        }
        
        update_candidates
    }
    
    /// 判断是否需要更新
    fn should_update(&self, addr: GuestAddr, version: &CodeVersion) -> bool {
        // 检查版本年龄
        let age = version.created_at.elapsed();
        if age > Duration::from_secs(300) { // 5分钟
            return true;
        }
        
        // 检查性能指标（这里需要与性能监控系统集成）
        // 简化实现：假设需要定期更新
        false
    }
    
    /// 创建更新任务
    pub fn create_update_task(&self, 
                           target_addr: GuestAddr, 
                           new_code: Vec<u8>, 
                           reason: UpdateReason) -> u64 {
        let task_id = {
            let mut id = self.next_task_id.lock().unwrap();
            let current_id = *id;
            *id += 1;
            current_id
        };
        
        // 获取当前版本
        let current_version = self.active_versions.lock().unwrap()
            .get(&target_addr)
            .cloned();
        
        // 创建新版本
        let new_version = CodeVersion {
            version: current_version.as_ref()
                .map(|v| v.version + 1)
                .unwrap_or(1),
            code: new_code.clone(),
            created_at: Instant::now(),
            size: new_code.len(),
            checksum: self.calculate_checksum(&new_code),
            update_reason: reason,
        };
        
        // 创建更新任务
        let task = UpdateTask {
            task_id,
            target_addr,
            new_version,
            current_version,
            status: UpdateStatus::Pending,
            created_at: Instant::now(),
            started_at: None,
            completed_at: None,
        };
        
        // 添加到任务列表
        let mut tasks = self.update_tasks.lock().unwrap();
        tasks.insert(task_id, task);
        
        task_id
    }
    
    /// 执行更新任务
    pub fn execute_update_task(&self, task_id: u64) -> Result<(), String> {
        let mut tasks = self.update_tasks.lock().unwrap();
        let task = tasks.get_mut(&task_id)
            .ok_or_else(|| "Task not found".to_string())?;
        
        // 检查并发更新限制
        let in_progress_count = tasks.values()
            .filter(|t| t.status == UpdateStatus::InProgress)
            .count();
        
        if in_progress_count >= self.config.max_concurrent_updates {
            return Err("Too many concurrent updates".to_string());
        }
        
        // 开始更新
        task.status = UpdateStatus::InProgress;
        task.started_at = Some(Instant::now());
        
        let start_time = Instant::now();
        
        // 执行实际更新
        let result = self.perform_hot_update(
            task.target_addr,
            &task.new_version,
            task.current_version.as_ref(),
        );
        
        let update_time = start_time.elapsed();
        
        match result {
            Ok(()) => {
                task.status = UpdateStatus::Completed;
                task.completed_at = Some(Instant::now());
                
                // 更新统计
                self.update_success_stats(update_time);
                
                // 更新活跃版本
                let mut active_versions = self.active_versions.lock().unwrap();
                active_versions.insert(task.target_addr, task.new_version.clone());
                
                // 添加到版本历史
                let mut versions = self.versions.lock().unwrap();
                let version_list = versions.entry(task.target_addr)
                    .or_insert_with(VecDeque::new);
                version_list.push_back(task.new_version.clone());
                
                // 保持历史大小
                while version_list.len() > self.config.version_history_size {
                    version_list.pop_front();
                }
                
                Ok(())
            }
            Err(e) => {
                task.status = UpdateStatus::Failed(e.clone());
                task.completed_at = Some(Instant::now());
                
                // 更新统计
                self.update_failure_stats(update_time);
                
                Err(e)
            }
        }
    }
    
    /// 执行热更新
    fn perform_hot_update(&self, 
                        target_addr: GuestAddr, 
                        new_version: &CodeVersion,
                        current_version: Option<&CodeVersion>) -> Result<(), String> {
        // 验证新代码
        if let Err(e) = self.validate_code(&new_version.code) {
            return Err(format!("Code validation failed: {}", e));
        }
        
        // 如果启用渐进式更新，尝试增量更新
        if self.config.enable_incremental_update {
            if let Some(current) = current_version {
                if let Some(diff) = self.compute_diff(&current.code, &new_version.code) {
                    return self.apply_incremental_update(target_addr, diff);
                }
            }
        }
        
        // 执行完整更新
        self.apply_full_update(target_addr, new_version)
    }
    
    /// 验证代码
    fn validate_code(&self, code: &[u8]) -> Result<(), String> {
        // 基本验证
        if code.is_empty() {
            return Err("Empty code".to_string());
        }
        
        // 检查代码大小
        if code.len() > 1024 * 1024 { // 1MB限制
            return Err("Code too large".to_string());
        }
        
        // 检查代码完整性
        let checksum = self.calculate_checksum(code);
        if checksum == 0 {
            return Err("Invalid checksum".to_string());
        }
        
        Ok(())
    }
    
    /// 计算校验和
    fn calculate_checksum(&self, code: &[u8]) -> u64 {
        code.iter().fold(0u64, |acc, &byte| {
            acc.wrapping_mul(31).wrapping_add(byte as u64)
        })
    }
    
    /// 计算代码差异
    fn compute_diff(&self, old_code: &[u8], new_code: &[u8]) -> Option<Vec<u8>> {
        // 简化的差异计算
        // 实际实现可以使用更复杂的差异算法
        if old_code.len() != new_code.len() {
            return None;
        }
        
        let mut diff = Vec::new();
        for (i, (&old_byte, &new_byte)) in old_code.iter().zip(new_code.iter()).enumerate() {
            if old_byte != new_byte {
                diff.push(i as u8);
                diff.push(new_byte);
            }
        }
        
        if diff.is_empty() {
            None
        } else {
            Some(diff)
        }
    }
    
    /// 应用增量更新
    fn apply_incremental_update(&self, target_addr: GuestAddr, diff: Vec<u8>) -> Result<(), String> {
        // 简化的增量更新实现
        // 实际实现需要更复杂的逻辑
        let mut cache = self.code_cache.lock().unwrap();
        
        // 获取当前代码
        let current_code = cache.get(target_addr)
            .ok_or_else(|| "Current code not found".to_string())?;
        
        // 应用差异
        let mut updated_code = current_code;
        let mut i = 0;
        while i < diff.len() {
            let offset = diff[i] as usize;
            if i + 1 < diff.len() {
                if offset < updated_code.len() {
                    updated_code[offset] = diff[i + 1];
                }
                i += 2;
            } else {
                break;
            }
        }
        
        // 更新缓存
        cache.insert(target_addr, updated_code);
        
        Ok(())
    }
    
    /// 应用完整更新
    fn apply_full_update(&self, target_addr: GuestAddr, new_version: &CodeVersion) -> Result<(), String> {
        let mut cache = self.code_cache.lock().unwrap();
        cache.insert(target_addr, new_version.code.clone());
        Ok(())
    }
    
    /// 回滚到指定版本
    pub fn rollback_to_version(&self, target_addr: GuestAddr, version: u64) -> Result<(), String> {
        let versions = self.versions.lock().unwrap();
        let version_list = versions.get(&target_addr)
            .ok_or_else(|| "No version history for address".to_string())?;
        
        let target_version = version_list.iter()
            .find(|v| v.version == version)
            .ok_or_else(|| "Version not found".to_string())?;
        
        // 应用回滚
        self.apply_full_update(target_addr, target_version)?;
        
        // 更新活跃版本
        let mut active_versions = self.active_versions.lock().unwrap();
        active_versions.insert(target_addr, target_version.clone());
        
        // 更新统计
        let mut stats = self.update_stats.lock().unwrap();
        stats.rollbacks += 1;
        
        Ok(())
    }
    
    /// 更新成功统计
    fn update_success_stats(&self, update_time: Duration) {
        let mut stats = self.update_stats.lock().unwrap();
        stats.total_updates += 1;
        stats.successful_updates += 1;
        
        let update_time_ms = update_time.as_millis() as f64;
        stats.avg_update_time_ms = 
            (stats.avg_update_time_ms * (stats.successful_updates - 1) as f64 + update_time_ms) 
            / stats.successful_updates as f64;
        
        stats.max_update_time_ms = stats.max_update_time_ms.max(update_time_ms as u64);
        stats.min_update_time_ms = if stats.min_update_time_ms == 0 {
            update_time_ms as u64
        } else {
            stats.min_update_time_ms.min(update_time_ms as u64)
        };
    }
    
    /// 更新失败统计
    fn update_failure_stats(&self, update_time: Duration) {
        let mut stats = self.update_stats.lock().unwrap();
        stats.total_updates += 1;
        stats.failed_updates += 1;
    }
    
    /// 获取更新统计
    pub fn update_stats(&self) -> HotUpdateStats {
        self.update_stats.lock().unwrap().clone()
    }
    
    /// 获取活跃版本
    pub fn active_version(&self, addr: GuestAddr) -> Option<CodeVersion> {
        self.active_versions.lock().unwrap().get(&addr).cloned()
    }
    
    /// 获取版本历史
    pub fn version_history(&self, addr: GuestAddr) -> Vec<CodeVersion> {
        self.versions.lock().unwrap()
            .get(&addr)
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default()
    }
    
    /// 获取待处理任务
    pub fn pending_tasks(&self) -> Vec<UpdateTask> {
        self.update_tasks.lock().unwrap()
            .values()
            .filter(|t| t.status == UpdateStatus::Pending)
            .cloned()
            .collect()
    }
    
    /// 取消更新任务
    pub fn cancel_update_task(&self, task_id: u64) -> Result<(), String> {
        let mut tasks = self.update_tasks.lock().unwrap();
        let task = tasks.get_mut(&task_id)
            .ok_or_else(|| "Task not found".to_string())?;
        
        match task.status {
            UpdateStatus::Pending => {
                task.status = UpdateStatus::Cancelled;
                task.completed_at = Some(Instant::now());
                Ok(())
            }
            UpdateStatus::InProgress => {
                Err("Cannot cancel in-progress task".to_string())
            }
            _ => {
                Err("Task already completed".to_string())
            }
        }
    }
}