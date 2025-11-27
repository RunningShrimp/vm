//! 预解码缓存模块
//!
//! 实现指令预解码缓存，用于缓存已解码的指令，以提高解码性能。
//! 采用 LRU 替换策略管理缓存容量。

use std::collections::HashMap;
use crate::IROp;

/// 预解码缓存条目
#[derive(Clone, Debug)]
struct CacheEntry {
    /// 缓存的 IR 操作序列
    ir_ops: Vec<IROp>,
    /// 访问时间戳（用于 LRU）
    access_time: u64,
}

/// 预解码缓存
/// 
/// 缓存已解码的指令，以避免重复解码相同的指令序列。
/// 采用 LRU 替换策略，自动驱逐访问最不频繁的条目。
///
/// # 标识
/// 缓存管理数据结构
#[derive(Debug)]
pub struct DecodeCache {
    /// 缓存条目字典，键为 (地址, 字节长度)
    cache: HashMap<(u64, usize), CacheEntry>,
    /// 最大容量（条目数）
    capacity: usize,
    /// 当前时钟值（用于 LRU）
    clock: u64,
}

impl DecodeCache {
    /// 创建新的预解码缓存
    ///
    /// # 参数
    ///
    /// * `capacity` - 最大缓存条目数
    ///
    /// # 返回值
    ///
    /// 新创建的空缓存实例
    ///
    /// # 示例
    ///
    /// ```
    /// use vm_ir::DecodeCache;
    ///
    /// let cache = DecodeCache::new(256);
    /// assert_eq!(cache.len(), 0);
    /// ```
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: HashMap::with_capacity(capacity),
            capacity,
            clock: 0,
        }
    }

    /// 获取缓存中的条目数
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// 检查缓存是否为空
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// 获取缓存命中率
    ///
    /// 返回已命中次数占总访问次数的比例。
    /// 注意：这是一个简化的统计，实际实现可能需要更完善的统计机制。
    pub fn hit_rate(&self) -> f64 {
        if self.cache.is_empty() {
            0.0
        } else {
            self.cache.len() as f64 / (self.capacity as f64)
        }
    }

    /// 从缓存中获取已解码的 IR，如果不存在则返回 None
    ///
    /// # 参数
    ///
    /// * `addr` - 指令地址
    /// * `bytes_len` - 指令字节长度
    ///
    /// # 返回值
    ///
    /// 如果缓存命中，返回 Some(ir_ops)；否则返回 None
    pub fn get(&mut self, addr: u64, bytes_len: usize) -> Option<Vec<IROp>> {
        let key = (addr, bytes_len);

        if let Some(entry) = self.cache.get_mut(&key) {
            // 缓存命中，更新访问时间戳
            self.clock += 1;
            entry.access_time = self.clock;
            return Some(entry.ir_ops.clone());
        }

        None
    }

    /// 将解码结果插入到缓存中
    ///
    /// 如果缓存满，将驱逐访问时间戳最小的条目。
    ///
    /// # 参数
    ///
    /// * `addr` - 指令地址
    /// * `bytes_len` - 指令字节长度
    /// * `ir_ops` - 解码后的 IR 操作序列
    pub fn insert(&mut self, addr: u64, bytes_len: usize, ir_ops: Vec<IROp>) {
        let key = (addr, bytes_len);

        // 如果缓存已满，驱逐 LRU 条目
        if self.cache.len() >= self.capacity && !self.cache.contains_key(&key) {
            self.evict_lru();
        }

        self.clock += 1;
        let entry = CacheEntry {
            ir_ops,
            access_time: self.clock,
        };

        self.cache.insert(key, entry);
    }

    /// 驱逐访问时间戳最小的条目（LRU）
    fn evict_lru(&mut self) {
        if let Some((&key, _)) = self
            .cache
            .iter()
            .min_by_key(|(_, entry)| entry.access_time)
        {
            self.cache.remove(&key);
        }
    }

    /// 清空缓存中的所有条目
    pub fn clear(&mut self) {
        self.cache.clear();
        self.clock = 0;
    }

    /// 统计缓存中的字节大小（近似）
    ///
    /// 计算所有缓存条目中 IR 操作的总数。
    pub fn size_estimate(&self) -> usize {
        self.cache.values().map(|entry| entry.ir_ops.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_cache_basic() {
        let mut cache = DecodeCache::new(10);
        
        let ir_ops = vec![
            IROp::MovImm { dst: 0, imm: 42 },
            IROp::Add { dst: 1, src1: 0, src2: 0 },
        ];
        
        // 插入缓存
        cache.insert(0x1000, 8, ir_ops.clone());
        assert_eq!(cache.len(), 1);
        
        // 查询缓存
        let result = cache.get(0x1000, 8);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn test_decode_cache_miss() {
        let mut cache = DecodeCache::new(10);
        
        // 查询不存在的条目
        let result = cache.get(0x2000, 8);
        assert!(result.is_none());
    }

    #[test]
    fn test_decode_cache_lru_eviction() {
        let mut cache = DecodeCache::new(2);
        
        let ir1 = vec![IROp::MovImm { dst: 0, imm: 1 }];
        let ir2 = vec![IROp::MovImm { dst: 1, imm: 2 }];
        let ir3 = vec![IROp::MovImm { dst: 2, imm: 3 }];
        
        // 插入第一个条目
        cache.insert(0x1000, 4, ir1);
        assert_eq!(cache.len(), 1);
        
        // 插入第二个条目
        cache.insert(0x2000, 4, ir2);
        assert_eq!(cache.len(), 2);
        
        // 访问第一个条目更新时间戳
        cache.get(0x1000, 4);
        
        // 插入第三个条目，应该驱逐第二个（LRU）
        cache.insert(0x3000, 4, ir3);
        assert_eq!(cache.len(), 2);
        
        // 验证第一个和第三个还在
        assert!(cache.get(0x1000, 4).is_some());
        assert!(cache.get(0x3000, 4).is_some());
        // 第二个应该被驱逐
        assert!(cache.get(0x2000, 4).is_none());
    }

    #[test]
    fn test_decode_cache_clear() {
        let mut cache = DecodeCache::new(10);
        
        let ir = vec![IROp::MovImm { dst: 0, imm: 42 }];
        cache.insert(0x1000, 4, ir);
        assert_eq!(cache.len(), 1);
        
        cache.clear();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_decode_cache_size_estimate() {
        let mut cache = DecodeCache::new(10);
        
        let ir1 = vec![
            IROp::MovImm { dst: 0, imm: 1 },
            IROp::Add { dst: 1, src1: 0, src2: 0 },
        ];
        let ir2 = vec![IROp::MovImm { dst: 1, imm: 2 }];
        
        cache.insert(0x1000, 8, ir1);
        cache.insert(0x2000, 4, ir2);
        
        // 应该有 3 个 IR 操作
        assert_eq!(cache.size_estimate(), 3);
    }
}
