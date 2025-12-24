use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use vm_ir::{IRBlock, RegId};
use crate::common::OptimizationStats;

pub struct HotUpdateManager {
    config: HotUpdateConfig,
    code_cache: Arc<RwLock<HashMap<u64, CachedCode>>>,
    hotspots: VecDeque<Hotspot>,
    update_queue: VecDeque<UpdateRequest>,
    stats: OptimizationStats,
    last_cleanup: Instant,
}

#[derive(Debug, Clone)]
pub struct HotUpdateConfig {
    pub max_cache_size: usize,
    pub hotspot_threshold: u64,
    pub update_interval: Duration,
    pub enable_background_update: bool,
    pub max_hotspots: usize,
    pub update_batch_size: usize,
}

impl Default for HotUpdateConfig {
    fn default() -> Self {
        Self {
            max_cache_size: 1024,
            hotspot_threshold: 100,
            update_interval: Duration::from_secs(5),
            enable_background_update: true,
            max_hotspots: 100,
            update_batch_size: 10,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CachedCode {
    pub block_id: u64,
    pub code: Vec<u8>,
    pub optimization_level: u8,
    pub execution_count: u64,
    pub last_used: Instant,
    pub size_bytes: usize,
    pub checksum: u64,
}

impl CachedCode {
    pub fn new(block_id: u64, code: Vec<u8>, optimization_level: u8) -> Self {
        let checksum = Self::compute_checksum(&code);
        Self {
            block_id,
            code,
            optimization_level,
            execution_count: 0,
            last_used: Instant::now(),
            size_bytes: 0,
            checksum,
        }
    }

    fn compute_checksum(code: &[u8]) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in code {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    pub fn verify_checksum(&self) -> bool {
        self.checksum == Self::compute_checksum(&self.code)
    }
}

#[derive(Debug, Clone)]
pub struct Hotspot {
    pub block_id: u64,
    pub execution_count: u64,
    pub last_update: Instant,
    pub priority: HotspotPriority,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HotspotPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
pub struct UpdateRequest {
    pub block_id: u64,
    pub old_optimization: u8,
    pub new_optimization: u8,
    pub timestamp: Instant,
    pub reason: UpdateReason,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateReason {
    HotspotDetected,
    ExecutionCountThreshold,
    PerformanceRegression,
    UserRequested,
}

#[derive(Debug, Clone)]
pub struct UpdateResult {
    pub block_id: u64,
    pub success: bool,
    pub old_size: usize,
    pub new_size: usize,
    pub update_time: Duration,
    pub reason: UpdateReason,
}

impl HotUpdateManager {
    pub fn new(config: HotUpdateConfig) -> Self {
        Self {
            config,
            code_cache: Arc::new(RwLock::new(HashMap::new())),
            hotspots: VecDeque::new(),
            update_queue: VecDeque::new(),
            stats: OptimizationStats::default(),
            last_cleanup: Instant::now(),
        }
    }

    pub fn add_block(&mut self, block_id: u64, code: Vec<u8>, optimization_level: u8) {
        let cached = CachedCode::new(block_id, code, optimization_level);
        
        let mut cache = self.code_cache.write().unwrap();
        cache.insert(block_id, cached);
        
        if cache.len() > self.config.max_cache_size {
            self.evict_oldest(&mut cache);
        }
    }

    pub fn record_execution(&mut self, block_id: u64) {
        let mut cache = self.code_cache.write().unwrap();
        if let Some(cached) = cache.get_mut(&block_id) {
            cached.execution_count += 1;
            cached.last_used = Instant::now();
            
            if cached.execution_count >= self.config.hotspot_threshold {
                drop(cache);
                self.add_hotspot(block_id);
            }
        }
    }

    fn add_hotspot(&mut self, block_id: u64) {
        let hotspot = Hotspot {
            block_id,
            execution_count: self.get_execution_count(block_id),
            last_update: Instant::now(),
            priority: self.determine_priority(block_id),
        };

        self.hotspots.push_back(hotspot);
        
        if self.hotspots.len() > self.config.max_hotspots {
            self.hotspots.pop_front();
        }

        self.queue_update(block_id, UpdateReason::HotspotDetected);
    }

    fn get_execution_count(&self, block_id: u64) -> u64 {
        let cache = self.code_cache.read().unwrap();
        cache.get(&block_id)
            .map(|c| c.execution_count)
            .unwrap_or(0)
    }

    fn determine_priority(&self, block_id: u64) -> HotspotPriority {
        let count = self.get_execution_count(block_id);
        let hotspot_threshold = self.config.hotspot_threshold;
        match count {
            c if c >= hotspot_threshold * 4 => HotspotPriority::Critical,
            c if c >= hotspot_threshold * 2 => HotspotPriority::High,
            c if c >= hotspot_threshold + hotspot_threshold / 2 => HotspotPriority::Medium,
            _ => HotspotPriority::Low,
        }
    }

    fn queue_update(&mut self, block_id: u64, reason: UpdateReason) {
        let cache = self.code_cache.read().unwrap();
        if let Some(cached) = cache.get(&block_id) {
            let request = UpdateRequest {
                block_id,
                old_optimization: cached.optimization_level,
                new_optimization: (cached.optimization_level + 1).min(3),
                timestamp: Instant::now(),
                reason,
            };
            drop(cache);
            self.update_queue.push_back(request);
        }
    }

    pub fn process_updates(&mut self, blocks: &HashMap<u64, IRBlock>) -> Vec<UpdateResult> {
        let mut results = Vec::new();
        let batch_size = self.config.update_batch_size.min(self.update_queue.len());
        
        for _ in 0..batch_size {
            if let Some(request) = self.update_queue.pop_front() {
                let start = Instant::now();
                
                if let Some(block) = blocks.get(&request.block_id) {
                    let old_size = self.get_cached_size(request.block_id);
                    
                    if self.perform_update(request.block_id, block, request.new_optimization) {
                        self.stats.blocks_optimized += 1;
                        
                        results.push(UpdateResult {
                            block_id: request.block_id,
                            success: true,
                            old_size,
                            new_size: self.get_cached_size(request.block_id),
                            update_time: start.elapsed(),
                            reason: request.reason.clone(),
                        });
                    } else {
                        results.push(UpdateResult {
                            block_id: request.block_id,
                            success: false,
                            old_size,
                            new_size: 0,
                            update_time: start.elapsed(),
                            reason: request.reason,
                        });
                    }
                }
            }
        }
        
        results
    }

    fn perform_update(&mut self, block_id: u64, block: &IRBlock, new_opt_level: u8) -> bool {
        let mut cache = self.code_cache.write().unwrap();
        if let Some(cached) = cache.get_mut(&block_id) {
            let mut new_code = cached.code.clone();
            
            self.apply_optimizations(&mut new_code, new_opt_level);
            
            cached.code = new_code;
            cached.optimization_level = new_opt_level;
            cached.last_used = Instant::now();
            cached.checksum = CachedCode::compute_checksum(&cached.code);
            
            true
        } else {
            false
        }
    }

    fn apply_optimizations(&self, code: &mut Vec<u8>, level: u8) {
        match level {
            0 => {}
            1 => self.basic_optimization(code),
            2 => self.aggressive_optimization(code),
            3 => self.maximum_optimization(code),
            _ => {}
        }
    }

    fn basic_optimization(&self, code: &mut Vec<u8>) {
        let mut i = 0;
        while i < code.len() {
            if i + 2 < code.len() {
                if code[i] == 0x90 && code[i + 1] == 0x90 && code[i + 2] == 0x90 {
                    code[i] = 0x0F;
                    code[i + 1] = 0x1F;
                    code[i + 2] = 0x00;
                    i += 3;
                    continue;
                }
            }
            i += 1;
        }
    }

    fn aggressive_optimization(&self, code: &mut Vec<u8>) {
        self.basic_optimization(code);
        self.reorder_instructions(code);
    }

    fn maximum_optimization(&self, code: &mut Vec<u8>) {
        self.aggressive_optimization(code);
        self.inline_functions(code);
        self.loop_unrolling(code);
    }

    fn reorder_instructions(&self, code: &mut Vec<u8>) {
        if code.len() > 16 {
            let mid = code.len() / 2;
            let first_half: Vec<_> = code[..mid].to_vec();
            let second_half: Vec<_> = code[mid..].to_vec();
            
            code.clear();
            code.extend_from_slice(&second_half);
            code.extend_from_slice(&first_half);
        }
    }

    fn inline_functions(&self, code: &mut Vec<u8>) {
        while code.len() > 0 && code.len() < code.capacity() / 2 {
            code.push(0xCC);
        }
    }

    fn loop_unrolling(&self, code: &mut Vec<u8>) {
        if code.len() < 64 {
            let copy: Vec<u8> = code.clone();
            code.extend_from_slice(&copy);
        }
    }

    fn get_cached_size(&self, block_id: u64) -> usize {
        let cache = self.code_cache.read().unwrap();
        cache.get(&block_id)
            .map(|c| c.code.len())
            .unwrap_or(0)
    }

    fn evict_oldest(&self, cache: &mut HashMap<u64, CachedCode>) {
        let mut oldest_block_id: Option<u64> = None;
        let mut oldest_time = Instant::now();

        for (block_id, cached) in cache.iter() {
            if cached.last_used < oldest_time {
                oldest_time = cached.last_used;
                oldest_block_id = Some(*block_id);
            }
        }

        if let Some(block_id) = oldest_block_id {
            cache.remove(&block_id);
        }
    }

    pub fn get_cached_code(&self, block_id: u64) -> Option<Vec<u8>> {
        let cache = self.code_cache.read().unwrap();
        cache.get(&block_id)
            .filter(|c| c.verify_checksum())
            .map(|c| c.code.clone())
    }

    pub fn get_hotspots(&self) -> Vec<Hotspot> {
        self.hotspots.iter().cloned().collect()
    }

    pub fn get_pending_updates(&self) -> usize {
        self.update_queue.len()
    }

    pub fn get_cache_stats(&self) -> CacheStats {
        let cache = self.code_cache.read().unwrap();
        CacheStats {
            total_blocks: cache.len(),
            total_size: cache.values().map(|c| c.code.len()).sum(),
            avg_execution_count: if cache.is_empty() {
                0
            } else {
                cache.values().map(|c| c.execution_count).sum::<u64>() as usize / cache.len()
            },
        }
    }

    pub fn cleanup_expired(&mut self) {
        if self.last_cleanup.elapsed() < self.config.update_interval {
            return;
        }

        let mut cache = self.code_cache.write().unwrap();
        let now = Instant::now();
        
        let expired: Vec<u64> = cache
            .iter()
            .filter(|(_, cached)| now.duration_since(cached.last_used) > Duration::from_secs(300))
            .map(|(id, _)| *id)
            .collect();

        for id in expired {
            cache.remove(&id);
        }

        self.hotspots.retain(|h| now.duration_since(h.last_update) < Duration::from_secs(600));
        self.last_cleanup = now;
    }

    pub fn get_stats(&self) -> &OptimizationStats {
        &self.stats
    }

    pub fn reset_stats(&mut self) {
        self.stats = OptimizationStats::default();
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_blocks: usize,
    pub total_size: usize,
    pub avg_execution_count: usize,
}

pub struct DefaultHotUpdateManager {
    inner: HotUpdateManager,
}

impl DefaultHotUpdateManager {
    pub fn new() -> Self {
        let config = HotUpdateConfig::default();
        Self {
            inner: HotUpdateManager::new(config),
        }
    }

    pub fn add_block(&mut self, block_id: u64, code: Vec<u8>, optimization_level: u8) {
        self.inner.add_block(block_id, code, optimization_level);
    }

    pub fn record_execution(&mut self, block_id: u64) {
        self.inner.record_execution(block_id);
    }

    pub fn process_updates(&mut self, blocks: &HashMap<u64, IRBlock>) -> Vec<UpdateResult> {
        self.inner.process_updates(blocks)
    }

    pub fn get_hotspots(&self) -> Vec<Hotspot> {
        self.inner.get_hotspots()
    }
}

impl Default for DefaultHotUpdateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hot_update_manager_creation() {
        let config = HotUpdateConfig::default();
        let manager = HotUpdateManager::new(config);
        assert_eq!(manager.get_pending_updates(), 0);
    }

    #[test]
    fn test_add_and_get_block() {
        let config = HotUpdateConfig::default();
        let mut manager = HotUpdateManager::new(config);
        
        manager.add_block(1, vec![1, 2, 3, 4], 0);
        let code = manager.get_cached_code(1);
        assert_eq!(code, Some(vec![1, 2, 3, 4]));
    }

    #[test]
    fn test_checksum_verification() {
        let cached = CachedCode::new(1, vec![1, 2, 3, 4], 0);
        assert!(cached.verify_checksum());
    }

    #[test]
    fn test_hotspot_detection() {
        let config = HotUpdateConfig::default();
        let mut manager = HotUpdateManager::new(config);
        
        manager.add_block(1, vec![1, 2, 3, 4], 0);
        for _ in 0..110 {
            manager.record_execution(1);
        }
        
        let hotspots = manager.get_hotspots();
        assert!(!hotspots.is_empty());
    }
}
