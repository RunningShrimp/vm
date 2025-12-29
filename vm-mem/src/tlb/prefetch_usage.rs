//! TLB预热功能说明文档
//!
//! 本文档说明如何使用MultiLevelTlb的预热功能

use super::{MultiLevelTlb, MultiLevelTlbConfig};
use vm_core::GuestAddr;

/// TLB预热功能使用指南
pub struct TlbPrefetchGuide;

impl TlbPrefetchGuide {
    /// 创建配置（启用预热）
    ///
    /// 创建一个启用了TLB预热功能的配置
    pub fn create_prefetch_config() -> MultiLevelTlbConfig {
        MultiLevelTlbConfig {
            l1_capacity: 64,
            l2_capacity: 256,
            l3_capacity: 1024,
            prefetch_window: 16,          // 预热窗口：16个条目
            prefetch_threshold: 0.8,      // 预热阈值
            adaptive_replacement: true,
            concurrent_optimization: false, // 关闭并发以简化测试
            enable_stats: true,
            enable_prefetch: true,         // 启用TLB预热
        }
    }

    /// 示例1：基本预热使用
    ///
    /// 展示如何预热特定地址（如代码段、数据段）
    ///
    /// # 使用场景
    /// - 应用启动时预热代码段
    /// - 服务启动时预热配置数据
    /// - 工作负载切换时预热新数据
    ///
    /// # 代码示例
    /// ```ignore
    /// // 创建TLB（启用预热）
    /// let config = TlbPrefetchGuide::create_prefetch_config();
    /// let mut tlb = MultiLevelTlb::new(config);
    ///
    /// // 准备预热地址
    /// let warm_addresses = vec![
    ///     GuestAddr(0x1000),  // 代码段起始
    ///     GuestAddr(0x2000),  // 数据段起始
    ///     GuestAddr(0x3000),  // 堆段起始
    /// ];
    ///
    /// // 添加到预取队列
    /// tlb.prefetch_addresses(warm_addresses);
    ///
    /// // 执行预热
    /// tlb.prefetch();
    /// // 输出：TLB预热完成：预热3个条目，耗时XXX
    /// ```
    pub fn example_basic_prefetch() {
        let config = Self::create_prefetch_config();
        let mut tlb = MultiLevelTlb::new(config);

        // 准备预热地址
        let warm_addresses = vec![
            GuestAddr(0x1000),  // 代码段起始（4KB）
            GuestAddr(0x2000),  // 数据段起始（8KB）
            GuestAddr(0x3000),  // 堆段起始（12KB）
        ];

        // 添加到预取队列
        tlb.prefetch_addresses(warm_addresses);

        // 执行预热
        tlb.prefetch();
    }

    /// 示例2：批量预热使用
    ///
    /// 展示如何批量预热大量地址
    ///
    /// # 使用场景
    /// - 系统启动时预热常用库代码
    /// - 大规模数据处理前预热数据缓冲区
    ///
    /// # 代码示例
    /// ```ignore
    /// // 创建TLB
    /// let config = TlbPrefetchGuide::create_prefetch_config();
    /// let mut tlb = MultiLevelTlb::new(config);
    ///
    /// // 批量添加地址
    /// let addresses: Vec<GuestAddr> = (0..100)
    ///     .map(|i| GuestAddr(0x1000 + (i as u64) * 4096))
    ///     .collect();
    ///
    /// tlb.prefetch_addresses(addresses);
    ///
    /// // 执行预热
    /// tlb.prefetch();
    /// // 输出：TLB预热完成：预热16个条目，耗时XXX
    /// ```
    pub fn example_batch_prefetch() {
        let config = Self::create_prefetch_config();
        let mut tlb = MultiLevelTlb::new(config);

        // 批量添加地址（模拟代码段、数据段的多个页面）
        let addresses: Vec<GuestAddr> = (0..100)
            .map(|i| GuestAddr(0x1000 + (i as u64) * 4096))
            .collect();

        println!("准备预热{}个地址", addresses.len());

        // 添加到预取队列
        tlb.prefetch_addresses(addresses);

        // 执行预热
        tlb.prefetch();
    }

    /// 示例3：预热效果验证
    ///
    /// 展示如何验证预热是否生效
    ///
    /// # 验证步骤
    /// 1. 执行预热
    /// 2. 访问预热过的地址
    /// 3. 检查统计信息
    /// 4. 比较访问时间
    ///
    /// # 预期结果
    /// - 预热的地址应该命中L1 TLB
    /// - 访问时间应该更快
    /// - 统计中应该有预热命中记录
    pub fn example_prefetch_verification() {
        let config = Self::create_prefetch_config();
        let mut tlb = MultiLevelTlb::new(config);

        // 预热常用地址
        let warm_addresses = vec![
            GuestAddr(0x1000),
            GuestAddr(0x2000),
            GuestAddr(0x3000),
        ];

        tlb.prefetch_addresses(warm_addresses);
        tlb.prefetch();

        // 访问预热过的地址（应该命中L1）
        let _ = tlb.lookup(GuestAddr(0x1000), 0, crate::AccessType::Read);

        // 访问未预热的地址（应该缺失或进入L2/L3）
        let _ = tlb.lookup(GuestAddr(0x4000), 0, crate::AccessType::Read);

        // 获取统计信息
        let stats = tlb.get_stats();
        println!("预热命中次数：{}", 
                 stats.prefetch_hits.load(std::sync::atomic::Ordering::Relaxed));
    }

    /// 预热策略建议
    ///
    /// # 何时使用预热
    /// 1. **应用启动时**：预热代码段、系统库
    ///    - 减少冷启动时间
    ///    - 提高初始响应速度
    ///
    /// 2. **服务启动时**：预热配置数据、常量表
    ///    - 加快服务初始化
    ///    - 提高第一个请求的响应时间
    ///
    /// 3. **工作负载切换时**：预热新数据集
    ///    - 避免冷数据访问
    ///    - 保持性能稳定
    ///
    /// # 预热窗口设置建议
    /// - **小规模**（<100 MB）：prefetch_window = 8-16
    /// - **中等规模**（100-500 MB）：prefetch_window = 16-32
    /// - **大规模**（>500 MB）：prefetch_window = 32-64
    ///
    /// # 预热地址选择
    /// 优先预热：
    /// 1. 代码段（程序代码）
    /// 2. 常用数据段
    /// 3. 堆栈段
    /// 4. IO缓冲区
    /// 5. 页表项
    pub fn prefetch_strategies() {
        println!("=== TLB预热策略建议 ===");
        println!();
        println!("1. 应用启动时");
        println!("   预热：代码段、系统库");
        println!("   预期收益：减少冷启动时间10-20%");
        println!();
        println!("2. 服务启动时");
        println!("   预热：配置数据、常量表");
        println!("   预期收益：加快服务初始化");
        println!();
        println!("3. 工作负载切换时");
        println!("   预热：新数据集");
        println!("   预期收益：避免冷数据访问");
        println!();
        println!("4. 预热窗口设置建议");
        println!("   小规模（<100 MB）：prefetch_window = 8-16");
        println!("   中等规模（100-500 MB）：prefetch_window = 16-32");
        println!("   大规模（>500 MB）：prefetch_window = 32-64");
        println!();
        println!("5. 预热地址优先级");
        println!("   高优先级：代码段、系统库");
        println!("   中优先级：常用数据段");
        println!("   中优先级：堆栈段");
        println!("   低优先级：其他数据");
        println!("========================================");
    }
}

