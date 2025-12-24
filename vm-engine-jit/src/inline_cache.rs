use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vm_core::GuestAddr;
use vm_ir::IROp;

pub use config::*;
pub use entry::*;
pub use stats::*;

mod config;
mod entry;
mod stats;

/// 内联缓存
/// 
/// 用于缓存动态方法查找的结果，减少运行时开销。
/// 支持单态缓存（Monomorphic）和多态缓存（Polymorphic）。
pub struct InlineCache {
    /// 缓存配置
    config: InlineCacheConfig,
    /// 缓存条目（按调用点地址索引）
    entries: Arc<Mutex<HashMap<GuestAddr, CacheEntry>>>,
    /// 缓存统计
    stats: Arc<Mutex<InlineCacheStats>>,
}

impl InlineCache {
    pub fn new(config: InlineCacheConfig) -> Self {
        Self {
            config,
            entries: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(InlineCacheStats::default())),
        }
    }

    /// 查找内联缓存条目
    pub fn lookup(&self, call_site: GuestAddr, receiver: u64) -> Option<CacheValue> {
        let mut stats = self.stats.lock().unwrap();
        
        let entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.get(&call_site) {
            match entry {
                CacheEntry::Monomorphic(mono) => {
                    if mono.receiver == receiver {
                        stats.monomorphic_hits += 1;
                        stats.total_hits += 1;
                        return Some(CacheValue::CodePtr(mono.code_ptr));
                    } else {
                        stats.monomorphic_misses += 1;
                    }
                },
                CacheEntry::Polymorphic(poly) => {
                    for entry in &poly.entries {
                        if entry.receiver == receiver {
                            stats.polymorphic_hits += 1;
                            stats.total_hits += 1;
                            return Some(CacheValue::CodePtr(entry.code_ptr));
                        }
                    }
                    stats.polymorphic_misses += 1;
                },
            }
        }
        
        stats.total_misses += 1;
        None
    }

    /// 更新内联缓存条目
    pub fn update(&self, call_site: GuestAddr, receiver: u64, code_ptr: GuestAddr) {
        let mut entries = self.entries.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();
        
        let entry = entries.entry(call_site).or_insert_with(|| {
            stats.megamorphic_transitions += 1;
            CacheEntry::Monomorphic(MonomorphicCache::new(receiver, code_ptr))
        });

        match entry {
            CacheEntry::Monomorphic(mono) => {
                if mono.receiver == receiver {
                    mono.code_ptr = code_ptr;
                    mono.last_access = std::time::Instant::now();
                } else {
                    // 检查是否应该升级到多态缓存
                    let should_upgrade = mono.miss_count >= self.config.polymorphic_threshold;
                    mono.miss_count += 1;
                    stats.monomorphic_to_polymorphic += 1;
                    
                    if should_upgrade {
                        *entry = CacheEntry::Polymorphic(PolymorphicCache::new(
                            mono.receiver,
                            mono.code_ptr,
                            receiver,
                            code_ptr,
                        ));
                    }
                }
            },
            CacheEntry::Polymorphic(poly) => {
                // 查找现有条目
                for entry in &poly.entries {
                    if entry.receiver == receiver {
                        entry.code_ptr = code_ptr;
                        entry.last_access = std::time::Instant::now();
                        return;
                    }
                }
                
                // 添加新条目
                if poly.entries.len() < self.config.max_polymorphic_entries {
                    poly.entries.push(PolymorphicEntry::new(receiver, code_ptr));
                    poly.last_access = std::time::Instant::now();
                } else {
                    // 达到最大条目数，驱逐最旧的条目
                    if let Some(oldest_idx) = poly.entries.iter()
                        .enumerate()
                        .min_by_key(|(_, e)| e.last_access)
                        .map(|(i, _)| i)
                    {
                        poly.entries[oldest_idx] = PolymorphicEntry::new(receiver, code_ptr);
                    }
                }
            },
        }
    }

    /// 使缓存失效
    pub fn invalidate(&self, call_site: GuestAddr) {
        let mut entries = self.entries.lock().unwrap();
        entries.remove(&call_site);
        
        let mut stats = self.stats.lock().unwrap();
        stats.invalidations += 1;
    }

    /// 清空所有缓存
    pub fn clear(&self) {
        let mut entries = self.entries.lock().unwrap();
        entries.clear();
        
        let mut stats = self.stats.lock().unwrap();
        stats.clears += 1;
    }

    /// 获取缓存统计
    pub fn stats(&self) -> InlineCacheStats {
        self.stats.lock().unwrap().clone()
    }

    /// 检查调用点是否是单态的
    pub fn is_monomorphic(&self, call_site: GuestAddr) -> bool {
        let entries = self.entries.lock().unwrap();
        matches!(entries.get(&call_site), Some(CacheEntry::Monomorphic(_)))
    }

    /// 获取调用点的多态性信息
    pub fn get_polymorphism_info(&self, call_site: GuestAddr) -> Option<PolymorphismInfo> {
        let entries = self.entries.lock().unwrap();
        match entries.get(&call_site) {
            Some(CacheEntry::Monomorphic(mono)) => Some(PolymorphismInfo {
                entry_type: CacheEntryType::Monomorphic,
                unique_receivers: 1,
                total_lookups: mono.hit_count + mono.miss_count,
            }),
            Some(CacheEntry::Polymorphic(poly)) => Some(PolymorphismInfo {
                entry_type: CacheEntryType::Polymorphic,
                unique_receivers: poly.entries.len(),
                total_lookups: poly.entries.iter().map(|e| e.hit_count + e.miss_count).sum(),
            }),
            None => None,
        }
    }

    /// 使所有与指定接收者相关的缓存失效
    pub fn invalidate_by_receiver(&self, receiver: u64) {
        let mut entries = self.entries.lock().unwrap();
        let mut removed = 0;
        
        entries.retain(|_, entry| {
            let should_remove = match entry {
                CacheEntry::Monomorphic(mono) => mono.receiver == receiver,
                CacheEntry::Polymorphic(poly) => {
                    poly.entries.iter().any(|e| e.receiver == receiver)
                }
            };
            if should_remove {
                removed += 1;
            }
            !should_remove
        });
        
        let mut stats = self.stats.lock().unwrap();
        stats.invalidations += removed as u64;
    }

    /// 预热缓存（用于已知热点调用点）
    pub fn warm_up(&self, call_site: GuestAddr, receivers: Vec<(u64, GuestAddr)>) {
        let mut entries = self.entries.lock().unwrap();
        
        if receivers.len() == 1 {
            let (receiver, code_ptr) = receivers[0];
            entries.insert(call_site, CacheEntry::Monomorphic(MonomorphicCache::new(receiver, code_ptr)));
        } else if receivers.len() > 1 {
            let mut poly_entries = Vec::new();
            for (receiver, code_ptr) in receivers {
                poly_entries.push(PolymorphicEntry::new(receiver, code_ptr));
            }
            entries.insert(call_site, CacheEntry::Polymorphic(PolymorphicCache::new_with_entries(poly_entries)));
        }
    }

    /// 获取缓存大小
    pub fn size(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    /// 获取缓存命中率
    pub fn hit_rate(&self) -> f64 {
        let stats = self.stats.lock().unwrap();
        if stats.total_hits + stats.total_misses == 0 {
            0.0
        } else {
            stats.total_hits as f64 / (stats.total_hits + stats.total_misses) as f64
        }
    }
}

impl Default for InlineCache {
    fn default() -> Self {
        Self::new(InlineCacheConfig::default())
    }
}

/// 缓存值
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheValue {
    /// 代码指针
    CodePtr(GuestAddr),
    /// 直接值（用于常量）
    DirectValue(u64),
}

/// 多态性信息
#[derive(Debug, Clone)]
pub struct PolymorphismInfo {
    /// 缓存条目类型
    pub entry_type: CacheEntryType,
    /// 唯一接收者数量
    pub unique_receivers: usize,
    /// 总查找次数
    pub total_lookups: u64,
}

/// 缓存条目类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheEntryType {
    /// 单态
    Monomorphic,
    /// 多态
    Polymorphic,
}

/// 内联缓存编译器
/// 
/// 负责在编译时生成内联缓存相关的代码。
pub struct InlineCacheCompiler {
    /// 缓存配置
    config: InlineCacheConfig,
}

impl InlineCacheCompiler {
    pub fn new(config: InlineCacheConfig) -> Self {
        Self { config }
    }

    /// 生成内联缓存查找代码
    pub fn generate_lookup_code(&self, call_site: GuestAddr) -> Vec<u8> {
        let mut code = Vec::new();
        
        // 检查接收者类型是否匹配
        code.extend_from_slice(&[0x48, 0x39, 0xF8]); // cmp rdi, rax
        code.extend_from_slice(&[0x75, 0x10]); // jne miss
        
        // 命中：直接跳转到缓存的目标
        code.extend_from_slice(&[0xFF, 0xE0]); // jmp rax
        
        // 未命中：回退到慢速路径
        code.extend_from_slice(&[0x48, 0xB8]); // movabs rax, call_site
        code.extend_from_slice(&call_site.0.to_le_bytes());
        code.extend_from_slice(&[0xFF, 0xD0]); // call rax
        
        code
    }

    /// 生成内联缓存更新代码
    pub fn generate_update_code(&self, call_site: GuestAddr) -> Vec<u8> {
        let mut code = Vec::new();
        
        // 保存寄存器
        code.extend_from_slice(&[0x50, 0x51, 0x52]); // push rax, rcx, rdx
        
        // 调用缓存更新函数
        code.extend_from_slice(&[0x48, 0xB8]); // movabs rax, call_site
        code.extend_from_slice(&call_site.0.to_le_bytes());
        code.extend_from_slice(&[0xFF, 0xD0]); // call rax
        
        // 恢复寄存器
        code.extend_from_slice(&[0x5A, 0x59, 0x58]); // pop rdx, rcx, rax
        
        code
    }

    /// 为虚拟方法调用生成内联缓存桩代码
    pub fn generate_ic_stub(&self, call_site: GuestAddr, method_name: &str) -> Vec<u8> {
        let mut code = Vec::new();
        
        // 内联缓存查找
        code.extend_from_slice(&self.generate_lookup_code(call_site));
        
        // 慢速路径：查找方法表
        code.extend_from_slice(&self.generate_slow_path(call_site, method_name));
        
        code
    }

    /// 生成慢速路径代码
    fn generate_slow_path(&self, call_site: GuestAddr, _method_name: &str) -> Vec<u8> {
        let mut code = Vec::new();
        
        // 调用方法表查找
        code.extend_from_slice(&[0x48, 0xB8]); // movabs rax, slow_path
        code.extend_from_slice(&call_site.0.to_le_bytes());
        code.extend_from_slice(&[0xFF, 0xE0]); // jmp rax
        
        code
    }
}

impl Default for InlineCacheCompiler {
    fn default() -> Self {
        Self::new(InlineCacheConfig::default())
    }
}
