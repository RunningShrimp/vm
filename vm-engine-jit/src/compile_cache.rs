//! 编译缓存
//!
//! 缓存编译后的代码，避免重复编译相同的IR块。

use crate::compiler_backend::CompilerError;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use vm_ir::IRBlock;

/// 编译缓存
pub struct CompileCache {
    /// 缓存：hash -> compiled code
    cache: HashMap<u64, Vec<u8>>,
    /// 最大缓存大小
    max_size: usize,
    /// 当前缓存大小
    current_size: usize,
    /// 缓存命中次数
    hits: u64,
    /// 缓存未命中次数
    misses: u64,
}

impl CompileCache {
    /// 创建新的编译缓存
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: HashMap::new(),
            max_size,
            current_size: 0,
            hits: 0,
            misses: 0,
        }
    }

    /// 获取或编译代码
    pub fn get_or_compile<F>(
        &mut self,
        block: &IRBlock,
        compile_fn: F,
    ) -> Result<Vec<u8>, CompilerError>
    where
        F: FnOnce(&IRBlock) -> Result<Vec<u8>, CompilerError>,
    {
        let hash = self.calculate_hash(block);

        if let Some(code) = self.cache.get(&hash) {
            self.hits += 1;
            return Ok(code.clone());
        }

        self.misses += 1;
        let code = compile_fn(block)?;
        self.insert(hash, code.clone());
        Ok(code)
    }

    /// 插入缓存
    pub fn insert(&mut self, hash: u64, code: Vec<u8>) {
        // 检查缓存是否已满
        let code_len = code.len();
        if self.current_size + code_len > self.max_size {
            self.evict_lru();
        }

        self.cache.insert(hash, code);
        self.current_size += code_len;
    }

    /// 计算IR块的哈希值
    fn calculate_hash(&self, block: &IRBlock) -> u64 {
        let mut hasher = DefaultHasher::new();

        // 哈希块地址
        block.start_pc.hash(&mut hasher);

        // 哈希操作
        for op in &block.ops {
            // 简化实现：使用操作类型的哈希
            std::mem::discriminant(op).hash(&mut hasher);
        }

        // 哈希终止符
        std::mem::discriminant(&block.term).hash(&mut hasher);

        hasher.finish()
    }

    /// 使用LRU策略驱逐缓存项
    fn evict_lru(&mut self) {
        // 简化实现：清除一半缓存
        let keys_to_remove: Vec<u64> = self
            .cache
            .keys()
            .take(self.cache.len() / 2)
            .cloned()
            .collect();

        for key in keys_to_remove {
            if let Some(code) = self.cache.remove(&key) {
                self.current_size -= code.len();
            }
        }
    }

    /// 清空缓存
    pub fn clear(&mut self) {
        self.cache.clear();
        self.current_size = 0;
    }

    /// 获取缓存命中率
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// 获取缓存统计信息
    pub fn get_stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            hit_rate: self.hit_rate(),
            entries: self.cache.len(),
            current_size: self.current_size,
            max_size: self.max_size,
        }
    }

    /// 检查缓存中是否存在指定的哈希值
    pub fn contains_key(&self, hash: &u64) -> bool {
        self.cache.contains_key(hash)
    }

    /// 获取指定哈希值的编译代码
    pub fn get(&self, hash: &u64) -> Option<&Vec<u8>> {
        self.cache.get(hash)
    }

    /// 获取缓存中的条目数量
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// 检查缓存是否为空
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// 缓存命中次数
    pub hits: u64,
    /// 缓存未命中次数
    pub misses: u64,
    /// 缓存命中率
    pub hit_rate: f64,
    /// 缓存条目数
    pub entries: usize,
    /// 当前缓存大小
    pub current_size: usize,
    /// 最大缓存大小
    pub max_size: usize,
}

impl CacheStats {
    /// 生成统计报告
    pub fn report(&self) -> String {
        format!(
            r#"Cache Statistics:
  - Hits: {}
  - Misses: {}
  - Hit Rate: {:.2}%
  - Entries: {}
  - Size: {} / {} bytes ({:.1}% used)
"#,
            self.hits,
            self.misses,
            self.hit_rate * 100.0,
            self.entries,
            self.current_size,
            self.max_size,
            (self.current_size as f64 / self.max_size as f64) * 100.0
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_cache() {
        let mut cache = CompileCache::new(1024);

        // 创建测试块
        let block = IRBlock {
            start_pc: vm_ir::GuestAddr(0x1000),
            ops: vec![],
            term: vm_ir::Terminator::Ret,
        };

        // 第一次编译（缓存未命中）
        let result1 = cache.get_or_compile(&block, |_| Ok(vec![1, 2, 3]));
        assert!(result1.is_ok());
        assert_eq!(result1.unwrap(), vec![1, 2, 3]);

        // 第二次获取相同块（缓存命中）
        let result2 = cache.get_or_compile(&block, |_| panic!("Should not compile"));
        assert!(result2.is_ok());
        assert_eq!(result2.unwrap(), vec![1, 2, 3]);

        // 验证统计信息
        let stats = cache.get_stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_rate, 0.5);
    }
}
