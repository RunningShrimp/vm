# 性能调优指南

## 概述

本文档提供详细的性能调优指南，帮助用户优化虚拟机系统的性能。

## JIT编译参数调优

### 热点阈值调整

```rust
use vm_engine_jit::{Jit, AdaptiveThresholdConfig};

let config = AdaptiveThresholdConfig {
    min_threshold: 50,      // 最小阈值：50次执行
    max_threshold: 500,     // 最大阈值：500次执行
    sample_window: 1000,     // 采样窗口：1000次执行
    compile_time_weight: 0.3,
    exec_benefit_weight: 0.7,
    compile_time_budget_ns: 10_000_000, // 10ms编译时间预算
    enable_compile_time_budget: true,
};

let jit = Jit::with_adaptive_config(config);
```

### 分层编译策略

系统自动实现分层编译：
- **快速编译路径**（执行次数 < 200）：使用基础优化，快速生成代码
- **优化编译路径**（执行次数 >= 200）：使用完整优化，最大化性能

### 代码缓存配置

```rust
use vm_engine_jit::{UnifiedCodeCache, CacheConfig, EvictionPolicy};

let cache_config = CacheConfig {
    max_entries: 10000,
    max_memory_bytes: 128 * 1024 * 1024, // 128MB
    eviction_policy: EvictionPolicy::LRU_LFU,
    cleanup_interval_secs: 60,
    hotness_decay_factor: 0.95,
    warmup_size: 100,
};

let cache = UnifiedCodeCache::new(cache_config, Default::default());
```

## GC参数调优

### 基础GC配置

```rust
use vm_engine_jit::{UnifiedGC, UnifiedGcConfig};

let gc_config = UnifiedGcConfig {
    heap_size_limit: 256 * 1024 * 1024, // 256MB
    mark_quota_us: 1000,                 // 1ms标记配额
    sweep_quota_us: 500,                 // 0.5ms清扫配额
    adaptive_quota: true,                 // 启用自适应配额
    concurrent_marking: true,             // 启用并发标记
    write_barrier_shards: 8,              // 8个写屏障分片
    ..Default::default()
};

let gc = UnifiedGC::new(gc_config);
```

### GC触发阈值

- **堆使用率阈值**：默认80%，超过此值触发GC
- **目标占用率**：默认70%，GC后堆的目标占用率
- **增量步长**：默认100，每次增量GC处理的对象数

### GC性能优化建议

1. **启用并发标记**：减少GC暂停时间
2. **调整配额**：根据应用特性调整mark_quota_us和sweep_quota_us
3. **启用自适应配额**：让系统自动调整GC配额
4. **增加写屏障分片**：高并发场景下增加write_barrier_shards

## 内存管理调优

### MMU配置

```rust
use vm_mem::{UnifiedMmu, UnifiedMmuConfig, MmuOptimizationStrategy};

let mmu_config = UnifiedMmuConfig {
    strategy: MmuOptimizationStrategy::Hybrid, // 混合策略
    multilevel_tlb_config: Default::default(),
    concurrent_tlb_config: Default::default(),
    enable_prefetch: true,
    prefetch_history_window: 100,
    prefetch_distance: 4,
    strict_align: false,
    ..Default::default()
};

let mmu = UnifiedMmu::new(256 * 1024 * 1024, false, mmu_config);
```

### TLB优化策略

- **MultiLevel**：适合单线程场景，多级TLB缓存
- **Concurrent**：适合多线程场景，分片锁优化
- **Hybrid**：混合策略，自动选择最优方案

### 内存预取配置

- **prefetch_history_window**：预取历史窗口大小，默认100
- **prefetch_distance**：预取距离，默认4页
- **enable_prefetch**：是否启用预取，默认true

## 执行引擎调优

### 协程池配置

```rust
use vm_runtime::CoroutinePool;

let pool = CoroutinePool::new(
    num_cpus::get() * 2  // CPU核心数 * 2
);
```

### 并行执行配置

- **vCPU数量**：根据工作负载调整
- **协程池大小**：建议为vCPU数量的2倍
- **使用协程而非线程**：减少上下文切换开销

## 性能监控

### 启用性能监控

```rust
use vm_monitor::PerformanceMonitor;

let monitor = PerformanceMonitor::new(Default::default());
monitor.start_collection();

// 获取性能报告
let report = monitor.generate_report();
```

### 关键指标

- **JIT编译时间**：应 < 10ms
- **GC暂停时间**：应 < 1ms
- **TLB命中率**：应 > 90%
- **缓存命中率**：应 > 85%

## 性能调优检查清单

- [ ] 调整JIT热点阈值
- [ ] 启用分层编译
- [ ] 配置代码缓存大小和策略
- [ ] 调优GC参数
- [ ] 配置MMU和TLB策略
- [ ] 启用内存预取
- [ ] 配置协程池大小
- [ ] 启用性能监控
- [ ] 分析性能瓶颈
- [ ] 验证性能改进

## 常见性能问题

### JIT编译时间过长

**原因**：
- 热点阈值过低
- 编译优化级别过高

**解决方案**：
- 提高热点阈值
- 使用分层编译（快速路径）
- 启用编译时间预算

### GC暂停时间过长

**原因**：
- GC配额设置不当
- 堆大小过大

**解决方案**：
- 启用并发标记
- 启用自适应配额
- 减小堆大小
- 增加增量步长

### TLB命中率低

**原因**：
- TLB容量不足
- 访问模式不规律

**解决方案**：
- 使用多级TLB
- 启用预取
- 增加TLB容量

### 内存使用过高

**原因**：
- 代码缓存过大
- GC触发频率低

**解决方案**：
- 减小代码缓存大小
- 降低GC触发阈值
- 使用更激进的淘汰策略

## 性能基准

### 目标性能指标

| 指标 | 目标值 |
|------|--------|
| JIT编译时间 | < 10ms |
| GC暂停时间 | < 1ms |
| TLB命中率 | > 90% |
| 缓存命中率 | > 85% |
| 执行性能 | > 70% native |

## 最佳实践

1. **渐进式优化**：先优化最耗时的部分
2. **性能监控**：持续监控关键指标
3. **A/B测试**：对比不同配置的效果
4. **文档记录**：记录调优过程和结果
5. **回归测试**：确保优化不引入性能回归


