# GC参数自适应调整

## 概述

GC参数自适应调整模块（`gc_adaptive`）根据运行时数据动态调整GC参数，以优化GC性能和内存使用效率。该模块实现了以下功能：

1. **基于分配速率的GC触发**：根据内存分配速率智能触发GC，避免内存浪费和频繁GC
2. **年轻代比例自适应调整**：根据对象存活率动态调整年轻代大小比例
3. **晋升阈值自适应调整**：根据对象存活模式动态调整晋升到老年代的阈值

## 功能特性

### 1. 分配速率跟踪

`AllocationRateTracker`跟踪最近一段时间内的内存分配速率（字节/秒），用于：

- 检测高分配速率，提前触发GC
- 避免内存快速耗尽
- 优化GC触发时机

**配置参数：**
- `window_size_sec`: 跟踪窗口大小（秒），默认5秒

### 2. 年轻代比例自适应调整

`YoungGenRatioAdjuster`根据GC后的对象存活率动态调整年轻代大小比例：

- **高存活率**（>目标值）：增加年轻代比例，让更多对象在年轻代，更频繁的minor GC
- **低存活率**（<目标值）：减少年轻代比例，减少minor GC频率

**配置参数：**
- `target_survival_rate`: 目标存活率（0.0-1.0），默认0.1（10%）
- `min_ratio`: 最小年轻代比例，默认0.1（10%）
- `max_ratio`: 最大年轻代比例，默认0.5（50%）

**调整策略：**
- 每次GC后根据平均存活率调整
- 调整步长：±5%
- 限制在最小和最大比例之间

### 3. 晋升阈值自适应调整

`PromotionThresholdAdjuster`根据对象存活次数分布动态调整晋升阈值：

- **晋升比例太高**（>目标值*1.2）：提高阈值，让对象在年轻代停留更久
- **晋升比例太低**（<目标值*0.8）：降低阈值，让对象更快晋升到老年代

**配置参数：**
- `target_promotion_ratio`: 目标晋升比例（0.0-1.0），默认0.2（20%）
- `min_threshold`: 最小晋升阈值，默认1
- `max_threshold`: 最大晋升阈值，默认10

**调整策略：**
- 每次GC后根据存活次数分布调整
- 调整步长：±1
- 限制在最小和最大阈值之间

## 使用方法

### 启用自适应调整

在`UnifiedGcConfig`中启用自适应调整：

```rust
use vm_engine_jit::unified_gc::{UnifiedGC, UnifiedGcConfig};

let mut config = UnifiedGcConfig::default();
config.enable_adaptive_adjustment = true;
config.allocation_trigger_threshold = 10 * 1024 * 1024; // 10MB/秒

let gc = UnifiedGC::new(config);
```

### 记录分配

在对象分配时调用`record_allocation`：

```rust
// 分配对象时
gc.record_allocation(object_size);
```

### 检查是否应该触发GC

使用`should_trigger_gc`检查是否应该触发GC：

```rust
if gc.should_trigger_gc() {
    let roots = get_roots();
    let cycle_start = gc.start_gc(&roots);
    // ... 执行GC ...
    gc.finish_gc(cycle_start);
}
```

### 获取当前参数

```rust
// 获取当前年轻代比例
let young_gen_ratio = gc.get_young_gen_ratio();

// 获取当前晋升阈值
let promotion_threshold = gc.get_promotion_threshold();

// 获取当前分配速率
if let Some(rate) = gc.get_allocation_rate() {
    println!("Allocation rate: {} bytes/sec", rate);
}
```

## 配置参数

### UnifiedGcConfig新增字段

```rust
pub struct UnifiedGcConfig {
    // ... 其他字段 ...
    
    /// 启用自适应GC参数调整
    pub enable_adaptive_adjustment: bool,
    
    /// 基于分配速率的GC触发阈值（字节/秒）
    pub allocation_trigger_threshold: u64,
}
```

**默认值：**
- `enable_adaptive_adjustment`: `true`
- `allocation_trigger_threshold`: `10 * 1024 * 1024` (10MB/秒)

## GC触发逻辑

`should_trigger_gc`使用以下逻辑决定是否触发GC：

1. **分配速率超过阈值**：如果分配速率 > `allocation_trigger_threshold`，触发GC
2. **堆使用率很高**：如果堆使用率 > 80%，触发GC
3. **分配速率高且堆使用率中等**：如果分配速率 > 阈值/2 且堆使用率 > 50%，触发GC

## 性能影响

### 优势

1. **减少GC暂停时间**：通过优化触发时机，避免在关键时刻触发GC
2. **提高内存利用率**：根据实际使用模式调整参数，减少内存浪费
3. **自适应工作负载**：自动适应不同的分配模式和对象存活模式

### 开销

1. **分配跟踪开销**：每次分配需要记录，但开销很小（O(1)）
2. **参数调整开销**：GC后需要计算和调整参数，但开销可忽略不计
3. **内存开销**：需要存储分配历史和存活次数分布，但内存占用很小

## 最佳实践

1. **启用自适应调整**：对于大多数应用，建议启用自适应调整
2. **设置合理的触发阈值**：根据应用的内存分配模式设置`allocation_trigger_threshold`
3. **监控调整效果**：定期检查年轻代比例和晋升阈值的变化，确保调整合理
4. **性能测试**：在生产环境前进行性能测试，验证自适应调整的效果

## 示例

```rust
use vm_engine_jit::unified_gc::{UnifiedGC, UnifiedGcConfig};

fn main() {
    // 配置GC
    let mut config = UnifiedGcConfig::default();
    config.enable_adaptive_adjustment = true;
    config.allocation_trigger_threshold = 20 * 1024 * 1024; // 20MB/秒
    
    let gc = UnifiedGC::new(config);
    
    // 模拟分配
    for i in 0..1000 {
        let size = 1024 * (i % 100 + 1);
        gc.record_allocation(size);
        
        // 检查是否应该触发GC
        if gc.should_trigger_gc() {
            let roots = vec![0x1000, 0x2000]; // 示例根节点
            let cycle_start = gc.start_gc(&roots);
            
            // 执行增量标记
            loop {
                let (complete, _) = gc.incremental_mark();
                if complete {
                    break;
                }
            }
            
            gc.terminate_marking();
            
            // 执行增量清扫
            loop {
                let (complete, _) = gc.incremental_sweep();
                if complete {
                    break;
                }
            }
            
            gc.finish_gc(cycle_start);
            
            // 打印当前参数
            println!("Young gen ratio: {:.2}", gc.get_young_gen_ratio());
            println!("Promotion threshold: {}", gc.get_promotion_threshold());
            if let Some(rate) = gc.get_allocation_rate() {
                println!("Allocation rate: {} bytes/sec", rate);
            }
        }
    }
}
```

## 技术细节

### 分配速率计算

分配速率使用滑动窗口计算：
- 维护最近N秒内的分配历史
- 计算窗口内的总分配量
- 除以时间窗口得到分配速率

### 存活率计算

存活率 = 标记的对象数 / 总对象数（估算）

### 晋升比例计算

晋升比例 = 存活次数 >= 阈值的对象数 / 总对象数

## 未来改进

1. **更精确的存活率计算**：使用实际对象计数而不是估算
2. **多级调整策略**：根据不同的工作负载模式使用不同的调整策略
3. **机器学习优化**：使用ML模型预测最佳参数
4. **实时监控和告警**：当参数调整异常时发出告警


