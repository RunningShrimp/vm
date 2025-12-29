//! TLB预热机制使用示例
//!
//! 本示例展示如何使用MultiLevelTlb的预热功能：
//! - 基本预热
//! - 批量预热
//! - 预热效果验证

use super::MultiLevelTlb;
use vm_core::{AccessType, GuestAddr};

/// TLB预热使用示例
pub struct TlbPrefetchExample {
    tlb: MultiLevelTlb,
}

impl TlbPrefetchExample {
    /// 创建新的示例实例
    pub fn new() -> Self {
        // 创建配置（启用预热）
        let config = super::MultiLevelTlbConfig {
            l1_capacity: 64,
            l2_capacity: 256,
            l3_capacity: 1024,
            prefetch_window: 16,          // 预热16个条目
            prefetch_threshold: 0.8,
            adaptive_replacement: true,
            concurrent_optimization: false,
            enable_stats: true,
            enable_prefetch: true,        // 启用预热
        };

        Self {
            tlb: MultiLevelTlb::new(config),
        }
    }

    /// 示例1：基本预热
    ///
    /// 预热代码段和数据段
    pub fn example_basic_prefetch(&mut self) {
        println!("=== 示例1：基本预热 ===");

        // 准备预热地址（代码段、数据段、堆段）
        let addresses = vec![
            GuestAddr(0x1000),  // 代码段起始（4KB）
            GuestAddr(0x2000),  // 数据段起始（8KB）
            GuestAddr(0x3000),  // 堆段起始（12KB）
            GuestAddr(0x4000),  // 栈段起始（16KB）
        ];

        // 添加到预取队列
        self.tlb.prefetch_addresses(addresses.clone());
        println!("已添加{}个地址到预取队列", addresses.len());

        // 执行预热
        self.tlb.prefetch();
        println!("预热完成！");

        // 获取统计信息
        let stats = self.tlb.get_stats();
        let prefetch_hits = stats.prefetch_hits.load(std::sync::atomic::Ordering::Relaxed);
        println!("预热命中次数：{}", prefetch_hits);

        // 验证L1 TLB使用情况
        let (l1, l2, l3) = self.tlb.get_usage();
        println!("L1 TLB使用率：{}/{} ({:.1}%)", 
                 l1, 64, l1 as f64 * 100.0 / 64.0);
        println!("L2 TLB使用率：{}/{} ({:.1}%)", 
                 l2, 256, l2 as f64 * 100.0 / 256.0);
        println!("L3 TLB使用率：{}/{} ({:.1}%)", 
                 l3, 1024, l3 as f64 * 100.0 / 1024.0);
        println!();
    }

    /// 示例2：批量预热
    ///
    /// 预热大量地址（模拟实际工作负载）
    pub fn example_batch_prefetch(&mut self) {
        println!("=== 示例2：批量预热 ===");

        // 生成32个地址（模拟代码段、数据段的多个页面）
        let addresses: Vec<GuestAddr> = (0..32)
            .map(|i| GuestAddr(0x1000 + (i as u64) * 4096))
            .collect();

        println!("准备预热{}个地址", addresses.len());

        // 批量添加到预取队列
        self.tlb.prefetch_addresses(addresses);

        // 验证预取队列限制
        let prefetch_queue_len = self.tlb.prefetch_queue.len();
        println!("预取队列大小：{}（限制：32）", prefetch_queue_len);
        assert!(prefetch_queue_len <= 64, "预取队列大小应该被限制");

        // 执行预热（只预热prefetch_window个）
        self.tlb.prefetch();

        let stats = self.tlb.get_stats();
        let prefetch_hits = stats.prefetch_hits.load(std::sync::atomic::Ordering::Relaxed);
        println!("实际预热条目：{}", prefetch_hits);
        println!("预期预热数量：{}", self.tlb.config.prefetch_window);
        println!();
    }

    /// 示例3：预热效果验证
    ///
    /// 比较预热前后的访问性能
    pub fn example_prefetch_effect(&mut self) {
        println!("=== 示例3：预热效果验证 ===");

        // 场景1：未预热，直接访问
        let addr1 = GuestAddr(0x5000);  // 未预热的地址

        // 预热常用地址
        let warm_addresses = vec![
            GuestAddr(0x1000),
            GuestAddr(0x2000),
            GuestAddr(0x3000),
        ];

        self.tlb.prefetch_addresses(warm_addresses);
        self.tlb.prefetch();

        // 访问预热的地址（应该命中L1）
        let start1 = std::time::Instant::now();
        let _result1 = self.tlb.lookup(addr1, 0, AccessType::Read);
        let elapsed1 = start1.elapsed();
        println!("访问预热地址（{}）耗时：{:?}", addr1, elapsed1);

        // 访问未预热的地址（应该缺失）
        let addr2 = GuestAddr(0x4000);  // 未预热的地址
        let start2 = std::time::Instant::now();
        let _result2 = self.tlb.lookup(addr2, 0, AccessType::Read);
        let elapsed2 = start2.elapsed();
        println!("访问未预热地址（{}）耗时：{:?}", addr2, elapsed2);

        // 获取统计信息
        let stats = self.tlb.get_stats();
        let l1_hits = stats.l1_hits.load(std::sync::atomic::Ordering::Relaxed);
        let total_lookups = stats.total_lookups.load(std::sync::atomic::Ordering::Relaxed);
        
        println!();
        println!("=== 性能对比 ===");
        println!("总查找次数：{}", total_lookups);
        println!("L1 TLB命中次数：{}", l1_hits);
        println!("L1 TLB命中率：{:.2}%", 
                 l1_hits as f64 / total_lookups as f64 * 100.0);
        println!("预热地址访问更快：{}", elapsed1 < elapsed2);
        println!();
    }

    /// 示例4：预热与正常访问混合
    ///
    /// 模拟实际工作负载：部分预热，部分不预热
    pub fn example_mixed_workload(&mut self) {
        println!("=== 示例4：预热与正常访问混合 ===");

        // 预热前半部分地址
        let warm_addresses: Vec<GuestAddr> = (0..16)
            .map(|i| GuestAddr(0x1000 + (i as u64) * 4096))
            .collect();

        self.tlb.prefetch_addresses(warm_addresses);
        self.tlb.prefetch();

        println!("已预热{}个地址", warm_addresses.len());

        // 访问混合地址
        let (warm_hits, cold_misses) = (0..32)
            .filter_map(|i| {
                let addr = if i < 16 {
                    GuestAddr(0x1000 + (i as u64) * 4096)  // 预热
                } else {
                    GuestAddr(0x1000 + (i as u64) * 4096)  // 未预热
                };

                let _result = self.tlb.lookup(addr, 0, AccessType::Read);
                let stats = self.tlb.get_stats();
                let l1_hits = stats.l1_hits.load(std::sync::atomic::Ordering::Relaxed);

                let _start = std::time::Instant::now();
                let hit = if let Some(_) = _result {
                    let _elapsed = _start.elapsed();
                    _elapsed.as_nanos() < 1000  // 命中且快
                } else {
                    false
                };

                Some((hit, l1_hits))
            })
            .fold((0, 0), |acc, (hit, count)| {
                if hit {
                    (acc.0 + 1, acc.1 + count as u64)
                } else {
                    (acc.0, acc.1 + 1)
                }
            });

        let stats = self.tlb.get_stats();
        let total_lookups = stats.total_lookups.load(std::sync::atomic::Ordering::Relaxed);

        println!("总查找次数：{}", total_lookups);
        println!("预热地址命中次数：{}", warm_hits);
        println!("未预热地址缺失次数：{}", cold_misses);
        println!("预热地址命中率：{:.2}%", 
                 warm_hits as f64 / (warm_hits + cold_misses) as f64 * 100.0);
        println!();
    }

    /// 示例5：预热统计详细分析
    pub fn example_prefetch_stats_analysis(&mut self) {
        println!("=== 示例5：预热统计详细分析 ===");

        // 执行一次预热
        let addresses = vec![
            GuestAddr(0x1000),
            GuestAddr(0x2000),
            GuestAddr(0x3000),
        ];

        self.tlb.prefetch_addresses(addresses);
        self.tlb.prefetch();

        // 获取详细统计
        let stats = self.tlb.get_stats();
        println!("=== TLB统计信息 ===");
        println!("总查找次数：{}", 
                 stats.total_lookups.load(std::sync::atomic::Ordering::Relaxed));
        println!("L1 TLB命中：{}", 
                 stats.l1_hits.load(std::sync::atomic::Ordering::Relaxed));
        println!("L2 TLB命中：{}", 
                 stats.l2_hits.load(std::sync::atomic::Ordering::Relaxed));
        println!("L3 TLB命中：{}", 
                 stats.l3_hits.load(std::sync::atomic::Ordering::Relaxed));
        println!("总缺失：{}", 
                 stats.total_misses.load(std::sync::atomic::Ordering::Relaxed));
        println!("预热命中：{}", 
                 stats.prefetch_hits.load(std::sync::atomic::Ordering::Relaxed));
        println!("预热完成：{}", 
                 if self.tlb.prefetch_done { "是" } else { "否" });
        println!();

        // 计算命中率
        let total_hits = stats.total_lookups.load(std::sync::atomic::Ordering::Relaxed) 
            - stats.total_misses.load(std::sync::atomic::Ordering::Relaxed);
        if total_hits > 0 {
            println!("TLB总命中率：{:.2}%", 
                     total_hits as f64 / stats.total_lookups.load(std::sync::atomic::Ordering::Relaxed) as f64 * 100.0);
        }

        println!();
    }

    /// 主示例函数
    pub fn run_all_examples(&mut self) {
        println!("========================================");
        println!("  TLB预热机制使用示例");
        println!("========================================");
        println!();

        self.example_basic_prefetch();
        println!();

        self.example_batch_prefetch();
        println!();

        self.example_prefetch_effect();
        println!();

        self.example_mixed_workload();
        println!();

        self.example_prefetch_stats_analysis();
        println!();

        println!("========================================");
        println!("  示例运行完成");
        println!("========================================");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_creation() {
        let example = TlbPrefetchExample::new();
        assert!(!example.tlb.prefetch_done, "初始状态应该未预热");
    }
}

