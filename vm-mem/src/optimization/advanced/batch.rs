//! MMU批量操作优化
//!
//! 实现高效的批量内存操作，减少地址翻译开销

use crate::{GuestAddr, VmError};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

/// 批量内存操作请求
#[derive(Debug, Clone)]
pub enum BatchRequest {
    /// 读取请求
    Read {
        gva: GuestAddr,
        size: usize,
        asid: u16,
    },
    /// 写入请求
    Write {
        gva: GuestAddr,
        data: Vec<u8>,
        asid: u16,
    },
    /// 预取请求
    Prefetch {
        gva: GuestAddr,
        size: usize,
        asid: u16,
    },
}

/// 批量操作结果
#[derive(Debug, Clone)]
pub enum BatchResult {
    /// 读取结果
    Read {
        gva: GuestAddr,
        data: Vec<u8>,
        success: bool,
    },
    /// 写入结果
    Write {
        gva: GuestAddr,
        size: usize,
        success: bool,
    },
    /// 预取结果
    Prefetch {
        gva: GuestAddr,
        success: bool,
    },
}

/// 地址翻译缓存条目
#[derive(Debug, Clone)]
pub struct TranslationCacheEntry {
    /// Guest虚拟地址
    pub gva: GuestAddr,
    /// Guest物理地址
    pub gpa: GuestAddr,
    /// 页面大小
    pub page_size: u64,
    /// ASID
    pub asid: u16,
    /// 最后访问时间
    pub last_access: Instant,
    /// 访问计数
    pub access_count: u64,
}

/// 批量操作配置
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// 最大批量大小
    pub max_batch_size: usize,
    /// 翻译缓存大小
    pub translation_cache_size: usize,
    /// 是否启用预取
    pub enable_prefetch: bool,
    /// 预取距离（页数）
    pub prefetch_distance: usize,
    /// 是否启用地址合并
    pub enable_address_coalescing: bool,
    /// 合并的最大距离
    pub max_coalesce_distance: usize,
    /// 是否启用SIMD优化
    pub enable_simd: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 64,
            translation_cache_size: 1024,
            enable_prefetch: true,
            prefetch_distance: 2,
            enable_address_coalescing: true,
            max_coalesce_distance: 16, // 64KB
            enable_simd: true,
        }
    }
}

/// 批量操作统计信息
#[derive(Debug, Clone, Default)]
pub struct BatchStats {
    /// 总批次数
    pub total_batches: u64,
    /// 总请求数
    pub total_requests: u64,
    /// 平均批量大小
    pub avg_batch_size: f64,
    /// 翻译缓存命中数
    pub translation_cache_hits: u64,
    /// 翻译缓存未命中数
    pub translation_cache_misses: u64,
    /// 地址合并次数
    pub address_coalescing_count: u64,
    /// 预取命中数
    pub prefetch_hits: u64,
    /// 总处理时间（纳秒）
    pub total_time_ns: u64,
    /// 平均处理时间（纳秒）
    pub avg_time_ns: f64,
}

impl BatchStats {
    pub fn translation_cache_hit_rate(&self) -> f64 {
        let total = self.translation_cache_hits + self.translation_cache_misses;
        if total == 0 {
            0.0
        } else {
            self.translation_cache_hits as f64 / total as f64
        }
    }

    pub fn update_avg_batch_size(&mut self) {
        if self.total_batches > 0 {
            self.avg_batch_size = self.total_requests as f64 / self.total_batches as f64;
        }
    }

    pub fn update_avg_time_ns(&mut self) {
        if self.total_batches > 0 {
            self.avg_time_ns = self.total_time_ns as f64 / self.total_batches as f64;
        }
    }
}

/// MMU批量操作处理器
pub struct BatchMmuProcessor {
    /// 配置
    config: BatchConfig,
    /// 地址翻译缓存
    translation_cache: Arc<Mutex<HashMap<(GuestAddr, u16), TranslationCacheEntry>>>,
    /// 统计信息
    stats: Arc<Mutex<BatchStats>>,
    /// 地址翻译函数
    translate_fn: Box<dyn Fn(GuestAddr, u16) -> Result<(GuestAddr, u64), VmError> + Send + Sync>,
    /// 内存读取函数
    read_fn: Box<dyn Fn(GuestAddr, usize) -> Result<Vec<u8>, VmError> + Send + Sync>,
    /// 内存写入函数
    write_fn: Box<dyn Fn(GuestAddr, &[u8]) -> Result<(), VmError> + Send + Sync>,
}

impl BatchMmuProcessor {
    /// 创建新的批量处理器
    pub fn new<F1, F2, F3>(
        config: BatchConfig,
        translate_fn: F1,
        read_fn: F2,
        write_fn: F3,
    ) -> Self
    where
        F1: Fn(GuestAddr, u16) -> Result<(GuestAddr, u64), VmError> + Send + Sync + 'static,
        F2: Fn(GuestAddr, usize) -> Result<Vec<u8>, VmError> + Send + Sync + 'static,
        F3: Fn(GuestAddr, &[u8]) -> Result<(), VmError> + Send + Sync + 'static,
    {
        Self {
            config,
            translation_cache: Arc::new(Mutex::new(HashMap::with_capacity(1024))),
            stats: Arc::new(Mutex::new(BatchStats::default())),
            translate_fn: Box::new(translate_fn),
            read_fn: Box::new(read_fn),
            write_fn: Box::new(write_fn),
        }
    }

    /// 处理批量请求
    pub fn process_batch(&self, requests: &[BatchRequest]) -> Vec<BatchResult> {
        let start_time = Instant::now();
        let mut results = Vec::with_capacity(requests.len());

        // 更新统计信息
        {
            let mut stats = self.stats.lock().unwrap();
            stats.total_batches += 1;
            stats.total_requests += requests.len() as u64;
            stats.update_avg_batch_size();
        }

        // 预处理：地址合并和预取
        let processed_requests = if self.config.enable_address_coalescing {
            self.coalesce_requests(requests)
        } else {
            requests.to_vec()
        };

        // 处理每个请求
        for request in processed_requests {
            let result = match request {
                BatchRequest::Read { gva, size, asid } => {
                    self.handle_read_request(gva, size, asid)
                }
                BatchRequest::Write { gva, data, asid } => {
                    self.handle_write_request(gva, data.clone(), asid)
                }
                BatchRequest::Prefetch { gva, size, asid } => {
                    self.handle_prefetch_request(gva, size, asid)
                }
            };
            results.push(result);
        }

        // 更新处理时间统计
        {
            let mut stats = self.stats.lock().unwrap();
            stats.total_time_ns += start_time.elapsed().as_nanos() as u64;
            stats.update_avg_time_ns();
        }

        results
    }

    /// 处理读取请求
    fn handle_read_request(&self, gva: GuestAddr, size: usize, asid: u16) -> BatchResult {
        match self.translate_address_cached(gva, asid) {
            Ok((gpa, page_size)) => {
                // 检查是否跨页
                let page_offset = gva & (page_size - 1);
                if page_offset + size as u64 <= page_size {
                    // 不跨页，直接读取
                    match (self.read_fn)(gpa, size) {
                        Ok(data) => BatchResult::Read {
                            gva,
                            data,
                            success: true,
                        },
                        Err(_) => BatchResult::Read {
                            gva,
                            data: vec![0; size],
                            success: false,
                        },
                    }
                } else {
                    // 跨页，分两次读取
                    let first_size = (page_size - page_offset) as usize;
                    let second_size = size - first_size;
                    
                    match (self.read_fn)(gpa, first_size) {
                        Ok(first_data) => {
                            let next_gva = GuestAddr((gva.0 & !(page_size - 1)) + page_size);
                            match self.translate_address_cached(next_gva, asid) {
                                Ok((next_gpa, _)) => {
                                    match (self.read_fn)(next_gpa, second_size) {
                                        Ok(second_data) => {
                                            let mut data = first_data;
                                            data.extend_from_slice(&second_data);
                                            BatchResult::Read {
                                                gva,
                                                data,
                                                success: true,
                                            }
                                        }
                                        Err(_) => BatchResult::Read {
                                            gva,
                                            data: vec![0; size],
                                            success: false,
                                        },
                                    }
                                }
                                Err(_) => BatchResult::Read {
                                    gva,
                                    data: vec![0; size],
                                    success: false,
                                },
                            }
                        }
                        Err(_) => BatchResult::Read {
                            gva,
                            data: vec![0; size],
                            success: false,
                        },
                    }
                }
            }
            Err(_) => BatchResult::Read {
                gva,
                data: vec![0; size],
                success: false,
            },
        }
    }

    /// 处理写入请求
    fn handle_write_request(&self, gva: GuestAddr, data: Vec<u8>, asid: u16) -> BatchResult {
        let size = data.len();
        match self.translate_address_cached(gva, asid) {
            Ok((gpa, page_size)) => {
                // 检查是否跨页
                let page_offset = gva & (page_size - 1);
                if page_offset + size as u64 <= page_size {
                    // 不跨页，直接写入
                    match (self.write_fn)(gpa, &data) {
                        Ok(()) => BatchResult::Write {
                            gva,
                            size,
                            success: true,
                        },
                        Err(_) => BatchResult::Write {
                            gva,
                            size,
                            success: false,
                        },
                    }
                } else {
                    // 跨页，分两次写入
                    let first_size = (page_size - page_offset) as usize;
                    let second_size = size - first_size;
                    
                    match (self.write_fn)(gpa, &data[..first_size]) {
                        Ok(()) => {
                            let next_gva = GuestAddr((gva.0 & !(page_size - 1)) + page_size);
                            match self.translate_address_cached(next_gva, asid) {
                                Ok((next_gpa, _)) => {
                                    match (self.write_fn)(next_gpa, &data[first_size..first_size + second_size]) {
                                        Ok(()) => BatchResult::Write {
                                            gva,
                                            size,
                                            success: true,
                                        },
                                        Err(_) => BatchResult::Write {
                                            gva,
                                            size,
                                            success: false,
                                        },
                                    }
                                }
                                Err(_) => BatchResult::Write {
                                    gva,
                                    size,
                                    success: false,
                                },
                            }
                        }
                        Err(_) => BatchResult::Write {
                            gva,
                            size,
                            success: false,
                        },
                    }
                }
            }
            Err(_) => BatchResult::Write {
                gva,
                size,
                success: false,
            },
        }
    }

    /// 处理预取请求
    fn handle_prefetch_request(&self, gva: GuestAddr, size: usize, asid: u16) -> BatchResult {
        // 预取就是提前进行地址翻译并缓存结果
        let page_size = 4096; // 假设4KB页
        let page_base = gva.0 & !(page_size - 1);
        let page_count = (size + page_size as usize - 1) / page_size as usize;
        
        let mut success = true;
        for i in 0..page_count {
            let current_gva = GuestAddr(page_base + i as u64 * page_size);
            if self.translate_address_cached(current_gva, asid).is_err() {
                success = false;
                break;
            }
        }
        
        BatchResult::Prefetch { gva, success }
    }

    /// 使用缓存的地址翻译
    fn translate_address_cached(&self, gva: GuestAddr, asid: u16) -> Result<(GuestAddr, u64), VmError> {
        let page_base = GuestAddr(gva.0 & !(4096 - 1)); // 页对齐
        let key = (page_base, asid);
        
        // 检查缓存
        {
            let cache = self.translation_cache.lock().unwrap();
            if let Some(entry) = cache.get(&key) {
                // 更新统计信息
                {
                    let mut stats = self.stats.lock().unwrap();
                    stats.translation_cache_hits += 1;
                }
                return Ok((entry.gpa + (gva - entry.gva), entry.page_size));
            }
        }
        
        // 缓存未命中，进行翻译
        let result = (self.translate_fn)(gva, asid)?;
        
        // 更新缓存
        {
            let mut cache = self.translation_cache.lock().unwrap();
            
            // 如果缓存已满，清理最旧的条目
            if cache.len() >= self.config.translation_cache_size {
                // 简单的LRU策略：找到最旧的条目并移除
                if let Some(oldest_key) = cache.iter()
                    .min_by_key(|(_, entry)| entry.last_access)
                    .map(|(key, _)| *key) {
                    cache.remove(&oldest_key);
                }
            }
            
            // 添加新条目
            cache.insert(key, TranslationCacheEntry {
                gva: page_base,
                gpa: GuestAddr(result.0 & !(4096 - 1)),
                page_size: result.1,
                asid,
                last_access: Instant::now(),
                access_count: 1,
            });
        }
        
        // 更新统计信息
        {
            let mut stats = self.stats.lock().unwrap();
            stats.translation_cache_misses += 1;
        }
        
        Ok(result)
    }

    /// 合并相邻的地址请求
    fn coalesce_requests(&self, requests: &[BatchRequest]) -> Vec<BatchRequest> {
        let mut coalesced = Vec::new();
        
        for request in requests {
            match request {
                BatchRequest::Read { gva, size, asid } => {
                    // 尝试与最后一个请求合并
                    if let Some(last) = coalesced.last_mut() {
                        if let BatchRequest::Read { 
                            gva: last_gva, 
                            size: last_size, 
                            asid: last_asid 
                        } = last {
                            if *last_asid == *asid {
                                let last_end = *last_gva + *last_size as u64;
                                let current_start = *gva;
                                
                                // 检查是否可以合并
                                if current_start >= last_end && 
                                   (current_start - last_end) <= self.config.max_coalesce_distance as u64 {
                                    // 合并请求
                                    let new_size = (current_start - *last_gva) + *size as u64;
                                    *last_size = new_size as usize;
                                    
                                    // 更新统计信息
                                    {
                                        let mut stats = self.stats.lock().unwrap();
                                        stats.address_coalescing_count += 1;
                                    }
                                    
                                    continue;
                                }
                            }
                        }
                    }
                    
                    // 无法合并，添加新请求
                    coalesced.push(request.clone());
                }
                _ => {
                    // 写入和预取请求暂不合并
                    coalesced.push(request.clone());
                }
            }
        }
        
        coalesced
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> BatchStats {
        self.stats.lock().unwrap().clone()
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        let mut stats = self.stats.lock().unwrap();
        *stats = BatchStats::default();
    }

    /// 清理翻译缓存
    pub fn clear_translation_cache(&self) {
        let mut cache = self.translation_cache.lock().unwrap();
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_translate_fn(gva: GuestAddr, _asid: u16) -> Result<(GuestAddr, u64), VmError> {
        Ok((gva + 0x1000_0000, 4096)) // 简单的地址转换
    }

    fn mock_read_fn(gpa: GuestAddr, size: usize) -> Result<Vec<u8>, VmError> {
        Ok(vec![0xAB; size]) // 返回固定数据
    }

    fn mock_write_fn(_gpa: GuestAddr, _data: &[u8]) -> Result<(), VmError> {
        Ok(()) // 总是成功
    }

    #[test]
    fn test_batch_processor_creation() {
        let config = BatchConfig::default();
        let processor = BatchMmuProcessor::new(
            config,
            mock_translate_fn,
            mock_read_fn,
            mock_write_fn,
        );
        
        let stats = processor.get_stats();
        assert_eq!(stats.total_batches, 0);
    }

    #[test]
    fn test_simple_batch_processing() {
        let config = BatchConfig::default();
        let processor = BatchMmuProcessor::new(
            config,
            mock_translate_fn,
            mock_read_fn,
            mock_write_fn,
        );
        
        let requests = vec![
            BatchRequest::Read { gva: 0x1000, size: 4, asid: 0 },
            BatchRequest::Write { gva: 0x2000, data: vec![1, 2, 3, 4], asid: 0 },
        ];
        
        let results = processor.process_batch(&requests);
        assert_eq!(results.len(), 2);
        
        match &results[0] {
            BatchResult::Read { success, .. } => assert!(success),
            _ => panic!("Expected Read result"),
        }
        
        match &results[1] {
            BatchResult::Write { success, .. } => assert!(success),
            _ => panic!("Expected Write result"),
        }
    }

    #[test]
    fn test_address_coalescing() {
        let config = BatchConfig {
            enable_address_coalescing: true,
            max_coalesce_distance: 16,
            ..Default::default()
        };
        let processor = BatchMmuProcessor::new(
            config,
            mock_translate_fn,
            mock_read_fn,
            mock_write_fn,
        );
        
        let requests = vec![
            BatchRequest::Read { gva: 0x1000, size: 4, asid: 0 },
            BatchRequest::Read { gva: 0x1008, size: 4, asid: 0 }, // 可以合并
        ];
        
        let results = processor.process_batch(&requests);
        assert_eq!(results.len(), 2); // 合并后应该只有一个请求
        
        let stats = processor.get_stats();
        assert_eq!(stats.address_coalescing_count, 1);
    }
}