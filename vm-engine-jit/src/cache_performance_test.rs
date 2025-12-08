//! 统一缓存性能测试

use std::time::Instant;
use std::collections::HashMap;

// 简化版缓存条目
#[derive(Debug, Clone)]
pub struct SimpleCacheEntry {
    pub code_ptr: *const u8,
    pub code_size: usize,
    pub access_count: u64,
}

// 简化版缓存
pub struct SimpleCache {
    cache: HashMap<u64, SimpleCacheEntry>,
    hits: u64,
    misses: u64,
}

impl SimpleCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            hits: 0,
            misses: 0,
        }
    }

    pub fn lookup(&mut self, addr: u64) -> Option<*const u8> {
        if let Some(entry) = self.cache.get_mut(&addr) {
            entry.access_count += 1;
            self.hits += 1;
            Some(entry.code_ptr)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn insert(&mut self, addr: u64, code_ptr: *const u8, code_size: usize) {
        let entry = SimpleCacheEntry {
            code_ptr,
            code_size,
            access_count: 0,
        };
        self.cache.insert(addr, entry);
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

fn main() {
    println!("=== 缓存性能测试 ===\n");

    // 测试参数
    const NUM_ENTRIES: usize = 10000;
    const NUM_LOOKUPS: usize = 100000;
    const HIT_RATE_TARGET: f64 = 0.8; // 80% 命中率目标

    // 创建测试数据
    let mut test_data = Vec::new();
    for i in 0..NUM_ENTRIES {
        let code = vec![i as u8; 100]; // 每个条目100字节
        test_data.push((i as u64, code.as_ptr(), code.len()));
    }

    // 创建缓存
    let mut cache = SimpleCache::new();

    // 阶段1: 填充缓存
    println!("阶段1: 填充缓存 ({} 条目)", NUM_ENTRIES);
    let start_time = Instant::now();
    
    for &(addr, ptr, size) in &test_data {
        cache.insert(addr, ptr, size);
    }
    
    let fill_time = start_time.elapsed();
    println!("填充完成，耗时: {:?}", fill_time);
    println!("填充速度: {:.2} 条目/秒\n", NUM_ENTRIES as f64 / fill_time.as_secs_f64());

    // 阶段2: 随机查找测试
    println!("阶段2: 随机查找测试 ({} 次查找)", NUM_LOOKUPS);
    let start_time = Instant::now();
    
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    for i in 0..NUM_LOOKUPS {
        // 使用简单的哈希来生成"随机"但可重现的访问模式
        let mut hasher = DefaultHasher::new();
        i.hash(&mut hasher);
        let hash = hasher.finish();
        
        // 80% 的时间访问已存在的条目，20% 访问不存在的条目
        let addr = if (hash % 100) < 80 {
            test_data[(hash as usize) % NUM_ENTRIES].0
        } else {
            NUM_ENTRIES as u64 + (hash % 1000)
        };
        
        cache.lookup(addr);
    }
    
    let lookup_time = start_time.elapsed();
    println!("查找完成，耗时: {:?}", lookup_time);
    println!("查找速度: {:.2} 查找/秒", NUM_LOOKUPS as f64 / lookup_time.as_secs_f64());
    println!("命中率: {:.2}% (目标: {:.2}%)", cache.hit_rate() * 100.0, HIT_RATE_TARGET * 100.0);
    
    // 性能评估
    let avg_lookup_time_ns = lookup_time.as_nanos() as f64 / NUM_LOOKUPS as f64;
    println!("平均查找时间: {:.2} 纳秒", avg_lookup_time_ns);
    
    // 评估结果
    println!("\n=== 性能评估 ===");
    
    let hit_rate_ok = cache.hit_rate() >= HIT_RATE_TARGET * 0.95; // 允许5%的误差
    let lookup_speed_ok = avg_lookup_time_ns < 1000.0; // 目标: 小于1微秒
    
    println!("命中率测试: {}", if hit_rate_ok { "✓ 通过" } else { "✗ 失败" });
    println!("查找速度测试: {}", if lookup_speed_ok { "✓ 通过" } else { "✗ 失败" });
    
    if hit_rate_ok && lookup_speed_ok {
        println!("\n🎉 缓存性能测试通过！");
    } else {
        println!("\n⚠️  缓存性能需要优化");
    }

    // 阶段3: 热点访问模式测试
    println!("\n阶段3: 热点访问模式测试");
    let mut hotspot_cache = SimpleCache::new();
    
    // 插入1000个条目
    for i in 0..1000 {
        let code = vec![i as u8; 50];
        hotspot_cache.insert(i, code.as_ptr(), code.len());
    }
    
    // 模拟热点访问：80%的访问集中在20%的条目上
    let start_time = Instant::now();
    for i in 0..50000 {
        let addr = if i % 100 < 80 {
            // 热点区域：前200个条目
            (i % 200) as u64
        } else {
            // 冷门区域：后800个条目
            200 + (i % 800) as u64
        };
        
        hotspot_cache.lookup(addr);
    }
    
    let hotspot_time = start_time.elapsed();
    println!("热点访问完成，耗时: {:?}", hotspot_time);
    println!("热点命中率: {:.2}%", hotspot_cache.hit_rate() * 100.0);
    
    // 热点访问应该有更高的命中率
    let hotspot_hit_rate_ok = hotspot_cache.hit_rate() > 0.9; // 90%以上
    println!("热点命中率测试: {}", if hotspot_hit_rate_ok { "✓ 通过" } else { "✗ 失败" });
    
    println!("\n=== 测试总结 ===");
    let all_tests_passed = hit_rate_ok && lookup_speed_ok && hotspot_hit_rate_ok;
    
    if all_tests_passed {
        println!("🎉 所有性能测试通过！缓存实现满足性能要求。");
    } else {
        println!("⚠️  部分测试未通过，需要进一步优化缓存实现。");
    }
}