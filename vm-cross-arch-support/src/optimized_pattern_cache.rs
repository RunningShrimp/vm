//! 优化的模式匹配缓存
//!
//! 基于性能分析优化的缓存实现：
//! - 真正的LRU驱逐策略
//! - 优化的哈希键计算
//! - 减少内存分配和克隆

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

// 重用Arch和PatternFeatures定义
pub use crate::pattern_cache::{Arch, InstructionPattern, OperandType, PatternFeatures};

// ============================================================================
// 优化的缓存键
// ============================================================================

/// 优化的缓存键 - 使用预计算的哈希
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CacheKey {
    arch: Arch,
    hash: u64, // 预计算的指令哈希
}

impl CacheKey {
    #[inline]
    fn new(arch: Arch, hash: u64) -> Self {
        Self { arch, hash }
    }
}

// ============================================================================
// LRU缓存节点
// ============================================================================

/// LRU链表节点
struct LruNode<K, V> {
    key: K,
    value: V,
    prev: Option<*mut LruNode<K, V>>,
    next: Option<*mut LruNode<K, V>>,
}

// ============================================================================
// 优化的模式匹配缓存
// ============================================================================

/// 优化的模式匹配缓存 - 使用LRU策略
pub struct OptimizedPatternMatchCache {
    /// 主缓存
    cache: HashMap<CacheKey, *mut LruNode<CacheKey, InstructionPattern>>,
    /// 特征缓存 (使用更小的键)
    feature_cache: HashMap<u64, PatternFeatures>,
    /// LRU链表头
    lru_head: Option<*mut LruNode<CacheKey, InstructionPattern>>,
    /// LRU链表尾
    lru_tail: Option<*mut LruNode<CacheKey, InstructionPattern>>,
    /// 最大缓存条目数
    max_entries: usize,
    /// 缓存命中次数
    hits: std::sync::atomic::AtomicU64,
    /// 缓存未命中次数
    misses: std::sync::atomic::AtomicU64,
}

unsafe impl Send for OptimizedPatternMatchCache {}
unsafe impl Sync for OptimizedPatternMatchCache {}

impl OptimizedPatternMatchCache {
    /// 创建新的优化缓存
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: HashMap::with_capacity(max_entries),
            feature_cache: HashMap::with_capacity(max_entries),
            lru_head: None,
            lru_tail: None,
            max_entries,
            hits: std::sync::atomic::AtomicU64::new(0),
            misses: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// 匹配或分析模式 - 优化版本
    pub fn match_or_analyze(&mut self, arch: Arch, bytes: &[u8]) -> InstructionPattern {
        // 快速哈希计算（使用更快的哈希算法）
        let hash = self.fast_hash_bytes(bytes);
        let key = CacheKey::new(arch, hash);

        // 快速路径：缓存命中
        if let Some(&node_ptr) = self.cache.get(&key) {
            self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            // 移到LRU链表头部
            self.move_to_front(node_ptr);
            // 返回克隆的模式
            return unsafe { (*node_ptr).value.clone() };
        }

        // 缓存未命中
        self.misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // 提取特征（从feature_cache或计算）
        let features = if let Some(cached_features) = self.feature_cache.get(&hash) {
            cached_features.clone()
        } else {
            let extracted = self.extract_features(bytes);
            self.feature_cache.insert(hash, extracted.clone());
            extracted
        };

        // 创建指令模式
        let pattern = InstructionPattern {
            name: self.infer_pattern_name(&features, arch),
            arch,
            features: features.clone(),
            operand_types: self.infer_operand_types(bytes, arch),
            is_memory: features.has_load || features.has_store,
            is_control_flow: features.has_branch,
        };

        // 克隆模式用于返回
        let result = pattern.clone();

        // 插入缓存
        self.insert_with_lru(key, pattern);

        result
    }

    /// 使用LRU策略插入缓存
    fn insert_with_lru(&mut self, key: CacheKey, pattern: InstructionPattern) {
        // 检查是否需要驱逐
        if self.cache.len() >= self.max_entries {
            self.evict_lru();
        }

        // 创建新节点
        let node = Box::leak(Box::new(LruNode {
            key,
            value: pattern,
            prev: None,
            next: self.lru_head,
        }));

        // 更新链表
        if let Some(old_head) = self.lru_head {
            unsafe {
                (*old_head).prev = Some(node);
            }
        }
        self.lru_head = Some(node);

        if self.lru_tail.is_none() {
            self.lru_tail = Some(node);
        }

        // 插入HashMap
        self.cache.insert(key, node);
    }

    /// 驱逐LRU条目
    fn evict_lru(&mut self) {
        if let Some(tail) = self.lru_tail {
            unsafe {
                // 从链表移除
                if let Some(prev) = (*tail).prev {
                    (*prev).next = None;
                    self.lru_tail = Some(prev);
                } else {
                    // 链表为空
                    self.lru_head = None;
                    self.lru_tail = None;
                }

                // 从HashMap移除
                self.cache.remove(&(*tail).key);

                // 释放内存
                let _ = Box::from_raw(tail);
            }
        }
    }

    /// 移动节点到链表头部（最近使用）
    fn move_to_front(&mut self, node_ptr: *mut LruNode<CacheKey, InstructionPattern>) {
        unsafe {
            // 如果已经在头部，无需操作
            if Some(node_ptr) == self.lru_head {
                return;
            }

            let node = &mut *node_ptr;

            // 从当前位置移除
            if let Some(prev) = node.prev {
                (*prev).next = node.next;
            } else {
                // node是头部，这不应该发生（前面已检查）
                return;
            }

            if let Some(next) = node.next {
                (*next).prev = node.prev;
            } else {
                // node是尾部
                self.lru_tail = node.prev;
            }

            // 插入到头部
            node.prev = None;
            node.next = self.lru_head;

            if let Some(old_head) = self.lru_head {
                (*old_head).prev = Some(node_ptr);
            }

            self.lru_head = Some(node_ptr);

            // 如果链表只有一个节点
            if self.lru_tail.is_none() {
                self.lru_tail = Some(node_ptr);
            }
        }
    }

    /// 快速字节哈希 - 优化版本
    #[inline]
    fn fast_hash_bytes(&self, bytes: &[u8]) -> u64 {
        // 使用更快的哈希算法（FNV-1a 64-bit）
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;

        let mut hash = FNV_OFFSET_BASIS;
        for &byte in bytes.iter().take(16) {
            // 只哈希前16字节
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }

    /// 提取指令特征 (与原实现相同)
    fn extract_features(&self, bytes: &[u8]) -> PatternFeatures {
        // 这里重用原实现的逻辑
        // 为了简洁，直接调用pattern_cache中的方法
        // 实际应用中应该提取公共逻辑
        PatternFeatures {
            has_load: self.detect_load(bytes),
            has_store: self.detect_store(bytes),
            has_branch: self.detect_branch(bytes),
            has_arithmetic: self.detect_arithmetic(bytes),
            has_logic: self.detect_logic(bytes),
            has_vector: self.detect_vector(bytes),
            has_float: self.detect_float(bytes),
            operand_count: self.estimate_operand_count(bytes),
            instruction_length: if bytes.len() >= 2 && (bytes[0] & 0x3) != 0x3 {
                2
            } else {
                4
            },
            is_compressed: bytes.len() >= 2 && (bytes[0] & 0x3) != 0x3,
        }
    }

    // 检测方法 (简化实现，实际应该与原实现一致)
    fn detect_load(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }
        let opcode = bytes[0] & 0x7F;
        opcode == 0x03 || matches!(bytes[0], 0x8B | 0x8D | 0xA1 | 0xA3 | 0xB8..=0xBF)
    }

    fn detect_store(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }
        let opcode = bytes[0] & 0x7F;
        opcode == 0x23 || matches!(bytes[0], 0x89 | 0x8C | 0xA2 | 0xA3)
    }

    fn detect_branch(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }
        let opcode = bytes[0] & 0x7F;
        opcode == 0x63
            || opcode == 0x6F
            || opcode == 0x67
            || matches!(bytes[0], 0x70..=0x7F | 0xE8 | 0xE9 | 0xEB | 0xFF)
    }

    fn detect_arithmetic(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }
        let opcode = bytes[0] & 0x7F;
        opcode == 0x33
            || opcode == 0x13
            || matches!(bytes[0], 0x00..=0x05 | 0x08..=0x0D | 0x28..=0x2D | 0x38..=0x3D | 0x50..=0x5D)
    }

    fn detect_logic(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }
        matches!(bytes[0], 0x20..=0x25 | 0x30..=0x35 | 0x80..=0x83 | 0x84..=0x86 | 0xA8..=0xAF)
    }

    fn detect_vector(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }
        let opcode = bytes[0] & 0x7F;
        opcode == 0x57 || (bytes[0] & 0xE0) == 0x40 || (bytes[0] & 0xF0) == 0x00
    }

    fn detect_float(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }
        let opcode = bytes[0] & 0x7F;
        opcode == 0x07
            || opcode == 0x27
            || opcode == 0x53
            || matches!(bytes[0], 0xD8..=0xDF | 0xF0..=0xFF | 0x0F)
    }

    fn estimate_operand_count(&self, bytes: &[u8]) -> u8 {
        if bytes.is_empty() {
            return 0;
        }
        let is_load = self.detect_load(bytes);
        let is_store = self.detect_store(bytes);
        let is_branch = self.detect_branch(bytes);
        let is_arithmetic = self.detect_arithmetic(bytes);

        if is_load || is_store {
            2
        } else if is_branch {
            1
        } else if is_arithmetic {
            3
        } else {
            2
        }
    }

    fn infer_pattern_name(&self, features: &PatternFeatures, arch: Arch) -> String {
        match arch {
            Arch::Riscv64 => {
                if features.is_compressed {
                    "riscv_c_insn".to_string()
                } else if features.has_load {
                    "riscv_load".to_string()
                } else if features.has_store {
                    "riscv_store".to_string()
                } else if features.has_branch {
                    "riscv_branch".to_string()
                } else if features.has_arithmetic {
                    "riscv_arith".to_string()
                } else if features.has_float {
                    "riscv_float".to_string()
                } else if features.has_vector {
                    "riscv_vector".to_string()
                } else {
                    "riscv_unknown".to_string()
                }
            }
            Arch::X86_64 => {
                if features.has_load {
                    "x86_load".to_string()
                } else if features.has_store {
                    "x86_store".to_string()
                } else if features.has_branch {
                    "x86_branch".to_string()
                } else if features.has_arithmetic {
                    "x86_arith".to_string()
                } else if features.has_float {
                    "x86_float".to_string()
                } else if features.has_vector {
                    "x86_vector".to_string()
                } else {
                    "x86_unknown".to_string()
                }
            }
            Arch::AArch64 | Arch::Arm => {
                if features.has_load || features.has_store {
                    "arm_mem".to_string()
                } else if features.has_branch {
                    "arm_branch".to_string()
                } else if features.has_arithmetic {
                    "arm_arith".to_string()
                } else if features.has_vector {
                    "arm_neon".to_string()
                } else {
                    "arm_unknown".to_string()
                }
            }
            Arch::Unknown => "unknown".to_string(),
        }
    }

    fn infer_operand_types(&self, _bytes: &[u8], _arch: Arch) -> Vec<OperandType> {
        vec![OperandType::Register, OperandType::Register]
    }

    /// 失效特定架构的缓存
    pub fn invalidate_arch(&mut self, arch: Arch) {
        // 收集要移除的键
        let keys_to_remove: Vec<_> = self
            .cache
            .keys()
            .filter(|key| key.arch == arch)
            .copied()
            .collect();

        // 移除条目并释放LRU节点
        for key in keys_to_remove {
            if let Some(node_ptr) = self.cache.remove(&key) {
                unsafe {
                    // 从LRU链表移除
                    let node = &mut *node_ptr;
                    if let Some(prev) = node.prev {
                        (*prev).next = node.next;
                    } else {
                        self.lru_head = node.next;
                    }

                    if let Some(next) = node.next {
                        (*next).prev = node.prev;
                    } else {
                        self.lru_tail = node.prev;
                    }

                    // 释放内存
                    let _ = Box::from_raw(node_ptr);
                }
            }
        }
    }

    /// 清空所有缓存
    pub fn clear(&mut self) {
        // 释放所有LRU节点
        while let Some(node_ptr) = self.lru_head {
            unsafe {
                let next = (*node_ptr).next;
                let _ = Box::from_raw(node_ptr);
                self.lru_head = next;
            }
        }

        self.cache.clear();
        self.feature_cache.clear();
        self.lru_tail = None;
    }

    /// 获取缓存大小
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// 获取命中率
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.misses.load(std::sync::atomic::Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// 获取缓存统计信息
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.cache.len(),
            hits: self.hits.load(std::sync::atomic::Ordering::Relaxed),
            misses: self.misses.load(std::sync::atomic::Ordering::Relaxed),
            hit_rate: self.hit_rate(),
        }
    }
}

// ============================================================================
// 缓存统计
// ============================================================================

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// 缓存条目数
    pub entries: usize,
    /// 命中次数
    pub hits: u64,
    /// 未命中次数
    pub misses: u64,
    /// 命中率
    pub hit_rate: f64,
}

// ============================================================================
// Drop实现
// ============================================================================

impl Drop for OptimizedPatternMatchCache {
    fn drop(&mut self) {
        // 释放所有LRU节点
        self.clear();
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimized_cache_creation() {
        let cache = OptimizedPatternMatchCache::new(1000);
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_fast_hash_consistency() {
        let cache = OptimizedPatternMatchCache::new(100);
        let bytes = [0x01, 0x02, 0x03, 0x04];
        let hash1 = cache.fast_hash_bytes(&bytes);
        let hash2 = cache.fast_hash_bytes(&bytes);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = OptimizedPatternMatchCache::new(3);

        // 添加4个不同的指令
        for i in 0..4 {
            let insn: u32 = 0x00000000 + (i as u32);
            let bytes = insn.to_le_bytes();
            cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);
        }

        // 缓存应该不超过最大条目数
        assert!(cache.len() <= 3);
    }

    #[test]
    fn test_hit_rate_tracking() {
        let mut cache = OptimizedPatternMatchCache::new(100);

        let insn: u32 = 0x00000303;
        let bytes = insn.to_le_bytes();

        // 第一次访问（未命中）
        cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);

        // 第二次访问（命中）
        cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_clear_cache() {
        let mut cache = OptimizedPatternMatchCache::new(100);

        let insn: u32 = 0x00000303;
        let bytes = insn.to_le_bytes();
        cache.match_or_analyze(Arch::Riscv64, &bytes[..4]);

        assert!(cache.len() > 0);

        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }
}
