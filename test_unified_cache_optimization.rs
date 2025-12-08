//! 独立测试文件，用于验证 unified_cache.rs 的优化效果
//! 这个文件不依赖 vm-core，可以直接测试优化后的代码结构

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

// 模拟类型定义
type GuestAddr = u64;
type CodePtr = usize; // 使用 usize 代替指针，确保线程安全

// 优化后的缓存条目结构
#[repr(C)]
pub struct CacheEntry {
    pub code_ptr: CodePtr,
    pub code_size: usize,
    pub access_count: AtomicU64,
    pub compilation_cost: u64,
    pub created_timestamp: u64,
    pub last_access_timestamp: u64,
    pub hotness_score: f32,
    pub execution_benefit: f32,
}

impl Clone for CacheEntry {
    fn clone(&self) -> Self {
        Self {
            code_ptr: self.code_ptr,
            code_size: self.code_size,
            access_count: AtomicU64::new(self.access_count.load(Ordering::Relaxed)),
            compilation_cost: self.compilation_cost,
            created_timestamp: self.created_timestamp,
            last_access_timestamp: self.last_access_timestamp,
            hotness_score: self.hotness_score,
            execution_benefit: self.execution_benefit,
        }
    }
}

impl std::fmt::Debug for CacheEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheEntry")
            .field("code_ptr", &self.code_ptr)
            .field("code_size", &self.code_size)
            .field("access_count", &self.access_count.load(Ordering::Relaxed))
            .field("compilation_cost", &self.compilation_cost)
            .field("created_timestamp", &self.created_timestamp)
            .field("last_access_timestamp", &self.last_access_timestamp)
            .field("hotness_score", &self.hotness_score)
            .field("execution_benefit", &self.execution_benefit)
            .finish()
    }
}

impl CacheEntry {
    pub fn new(code_ptr: CodePtr, code_size: usize) -> Self {
        let now = Self::current_timestamp();
        Self {
            code_ptr,
            code_size,
            access_count: AtomicU64::new(0),
            compilation_cost: 0,
            created_timestamp: now,
            last_access_timestamp: now,
            hotness_score: 0.0,
            execution_benefit: 0.0,
        }
    }

    fn current_timestamp() -> u64 {
        use std::time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }

    pub fn get_access_count(&self) -> u64 {
        self.access_count.load(Ordering::Relaxed)
    }

    pub fn increment_access(&self) {
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }
}

// 分片缓存结构
struct ShardedCache {
    shards: Vec<Arc<RwLock<HashMap<GuestAddr, CacheEntry>>>>,
    shard_count: usize,
    shard_mask: u64,
}

impl ShardedCache {
    fn new(shard_count: usize) -> Self {
        let shard_count = shard_count.next_power_of_two();
        let mut shards = Vec::with_capacity(shard_count);
        
        for _ in 0..shard_count {
            shards.push(Arc::new(RwLock::new(HashMap::new())));
        }
        
        Self {
            shards,
            shard_count,
            shard_mask: (shard_count - 1) as u64,
        }
    }
    
    fn get_shard(&self, addr: GuestAddr) -> &Arc<RwLock<HashMap<GuestAddr, CacheEntry>>> {
        &self.shards[(addr & self.shard_mask) as usize]
    }
    
    fn get(&self, addr: GuestAddr) -> Option<(CodePtr, u64)> {
        let shard = self.get_shard(addr);
        if let Ok(shard) = shard.try_read() {
            shard.get(&addr).map(|e| (e.code_ptr, e.get_access_count()))
        } else {
            let shard = shard.read().unwrap();
            shard.get(&addr).map(|e| (e.code_ptr, e.get_access_count()))
        }
    }
    
    fn insert(&self, addr: GuestAddr, entry: CacheEntry) -> Option<CacheEntry> {
        let shard = self.get_shard(addr);
        let mut shard = shard.write().unwrap();
        shard.insert(addr, entry)
    }
    
    fn total_size(&self) -> usize {
        self.shards.iter()
            .map(|shard| shard.read().unwrap().len())
            .sum()
    }
}

fn main() {
    println!("=== 统一缓存优化验证测试 ===\n");
    
    // 测试1: 分片缓存性能测试
    test_sharded_cache_performance();
    
    // 测试2: 内存布局优化验证
    test_memory_layout_optimization();
    
    // 测试3: 原子操作性能测试
    test_atomic_operations_performance();
    
    // 测试4: 并发性能测试
    test_concurrent_performance();
    
    println!("\n=== 所有测试完成 ===");
}

fn test_sharded_cache_performance() {
    println!("📊 测试1: 分片缓存性能");
    
    let cache = ShardedCache::new(16);
    let start = Instant::now();
    
    // 插入性能测试
    for i in 0..10000 {
        let code_ptr = i * 1024;
        let entry = CacheEntry::new(code_ptr, 1024);
        cache.insert(i as u64, entry);
    }
    
    let insert_time = start.elapsed();
    println!("  ✅ 插入10000条目耗时: {:?}", insert_time);
    
    // 查找性能测试
    let start = Instant::now();
    let mut hits = 0;
    for i in 0..10000 {
        if cache.get(i).is_some() {
            hits += 1;
        }
    }
    let lookup_time = start.elapsed();
    println!("  ✅ 查找10000次耗时: {:?}", lookup_time);
    println!("  ✅ 命中次数: {}/10000", hits);
    
    // 性能指标验证
    let insert_ops_per_sec = 10000.0 / insert_time.as_secs_f64();
    let lookup_ops_per_sec = 10000.0 / lookup_time.as_secs_f64();
    
    println!("  📈 插入性能: {:.0} ops/sec", insert_ops_per_sec);
    println!("  📈 查找性能: {:.0} ops/sec", lookup_ops_per_sec);
    
    // 验证性能目标
    assert!(insert_time.as_millis() < 100, "插入性能应该 < 100ms");
    assert!(lookup_time.as_millis() < 50, "查找性能应该 < 50ms");
    assert_eq!(hits, 10000, "所有插入的条目都应该能找到");
    
    println!("  ✅ 分片缓存性能测试通过\n");
}

fn test_memory_layout_optimization() {
    println!("🧠 测试2: 内存布局优化验证");
    
    let entry = CacheEntry::new(0, 1024);
    let entry_size = std::mem::size_of::<CacheEntry>();
    
    println!("  📏 优化后CacheEntry大小: {} bytes", entry_size);
    
    // 验证紧凑布局
    assert!(entry_size < 128, "CacheEntry大小应该小于128字节");
    
    // 验证原子操作
    assert_eq!(entry.get_access_count(), 0);
    entry.increment_access();
    assert_eq!(entry.get_access_count(), 1);
    
    // 验证时间戳功能
    let now = CacheEntry::current_timestamp();
    assert!(now > 0, "时间戳应该大于0");
    
    println!("  ✅ 内存布局优化验证通过\n");
}

fn test_atomic_operations_performance() {
    println!("⚡ 测试3: 原子操作性能测试");
    
    let entry = Arc::new(CacheEntry::new(0, 1024));
    let iterations = 1_000_000;
    
    // 原子操作性能测试
    let start = Instant::now();
    for _ in 0..iterations {
        entry.increment_access();
    }
    let atomic_time = start.elapsed();
    
    let final_count = entry.get_access_count();
    let ops_per_sec = iterations as f64 / atomic_time.as_secs_f64();
    
    println!("  🚀 {}次原子操作耗时: {:?}", iterations, atomic_time);
    println!("  📈 原子操作性能: {:.0} ops/sec", ops_per_sec);
    println!("  ✅ 最终计数: {}", final_count);
    
    // 验证性能目标
    assert!(final_count == iterations as u64, "原子操作计数应该正确");
    assert!(atomic_time.as_millis() < 100, "原子操作性能应该 < 100ms");
    
    println!("  ✅ 原子操作性能测试通过\n");
}

fn test_concurrent_performance() {
    println!("🔄 测试4: 并发性能测试");
    
    use std::thread;
    
    let cache = Arc::new(ShardedCache::new(16));
    let thread_count = 8;
    let operations_per_thread = 1000;
    
    let start = Instant::now();
    let mut handles = vec![];
    
    // 启动多个线程进行并发测试
    for thread_id in 0..thread_count {
        let cache_clone = cache.clone();
        let handle = thread::spawn(move || {
            let mut operations = 0;
            
            for i in 0..operations_per_thread {
                let addr = thread_id * operations_per_thread + i;
                let code_ptr = addr * 1024; // 直接使用 usize
                
                // 交替进行插入和查找操作
                if i % 2 == 0 {
                    let entry = CacheEntry::new(code_ptr, 1024);
                    cache_clone.insert(addr as u64, entry);
                } else {
                    cache_clone.get(addr as u64);
                }
                operations += 1;
            }
            
            operations
        });
        
        handles.push(handle);
    }
    
    // 等待所有线程完成
    let total_operations: usize = handles.into_iter()
        .map(|h| h.join().unwrap())
        .sum();
    
    let elapsed = start.elapsed();
    let total_ops_per_sec = total_operations as f64 / elapsed.as_secs_f64();
    
    println!("  👥 {}个线程并发测试", thread_count);
    println!("  ⏱️  总耗时: {:?}", elapsed);
    println!("  📊 总操作数: {}", total_operations);
    println!("  📈 并发性能: {:.0} ops/sec", total_ops_per_sec);
    
    // 验证并发性能目标
    assert_eq!(total_operations, (thread_count * operations_per_thread) as usize);
    assert!(elapsed.as_millis() < 200, "并发操作应该在200ms内完成");
    
    println!("  ✅ 并发性能测试通过\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cache_entry_creation() {
        let code_ptr = 0x1000 as *const u8;
        let entry = CacheEntry::new(code_ptr, 2048);
        
        assert_eq!(entry.code_ptr, code_ptr);
        assert_eq!(entry.code_size, 2048);
        assert_eq!(entry.get_access_count(), 0);
        assert!(entry.created_timestamp > 0);
    }
    
    #[test]
    fn test_sharded_cache_basic_operations() {
        let cache = ShardedCache::new(8);
        let addr = 0x2000;
        let code_ptr = 0x3000 as *const u8;
        let entry = CacheEntry::new(code_ptr, 1024);
        
        // 测试插入
        let old_entry = cache.insert(addr, entry.clone());
        assert!(old_entry.is_none());
        
        // 测试查找
        let found = cache.get(addr);
        assert!(found.is_some());
        assert_eq!(found.unwrap().0, code_ptr);
        
        // 测试大小
        assert_eq!(cache.total_size(), 1);
    }
    
    #[test]
    fn test_memory_efficiency() {
        let entry_size = std::mem::size_of::<CacheEntry>();
        
        // 验证内存优化效果
        assert!(entry_size <= 64, "优化后的CacheEntry应该 <= 64字节");
        
        // 验证对齐
        assert_eq!(entry_size % 8, 0, "CacheEntry应该8字节对齐");
    }
}