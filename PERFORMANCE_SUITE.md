# VM 虚拟机 - 性能测试套件

## 1. 基准测试配置

此目录包含性能基准测试，用于持续监控虚拟机的性能。

### 1.1 运行基准测试

```bash
# 运行所有基准测试
cargo bench --all

# 运行特定基准测试
cargo bench --bench jit_compile
cargo bench --bench jit_vs_interp
cargo bench --bench mmu_translate

# 生成详细报告
cargo bench --all -- --verbose --output-format bencher | tee bench_result.txt

# 与基准线比较
cargo bench --all -- --baseline base
```

## 2. 基准测试列表

### 2.1 JIT 编译性能 (vm-engine-jit/benches/jit_compile.rs)

**目的**: 测量 JIT 编译的性能

**指标**:
- 编译延迟 (ms)
- 编译吞吐量 (blocks/sec)
- 代码生成速率 (bytes/sec)

**测试场景**:
- 小块编译 (< 50 指令)
- 中块编译 (50-500 指令)
- 大块编译 (> 500 指令)
- 并行编译

**预期性能**:
- 小块: < 1ms
- 中块: 1-5ms
- 大块: 5-20ms
- 并行: 线性加速

```bash
cargo bench --bench jit_compile -- --verbose
```

### 2.2 JIT vs 解释器 (vm-engine-jit/benches/jit_vs_interp.rs)

**目的**: 比较 JIT 和解释器执行速度

**指标**:
- 执行吞吐量 (insn/sec)
- 执行延迟 (ns/insn)
- 相对性能 (JIT / Interpreter)

**测试场景**:
- 热循环 (重复指令)
- 冷启动 (首次执行)
- 混合工作负载
- 浮点密集

**预期性能**:
- 热循环: 5-20x 加速
- 冷启动: 1-3x 加速
- 浮点: 10x 加速

```bash
cargo bench --bench jit_vs_interp -- --verbose
```

### 2.3 MMU 翻译性能 (vm-mem/benches/mmu_translate.rs)

**目的**: 测量地址翻译性能

**指标**:
- TLB 查找延迟 (ns)
- TLB 命中率 (%)
- 页表遍历延迟 (μs)

**测试场景**:
- TLB 热 (99% 命中率)
- TLB 冷 (0% 命中率)
- 混合访问模式
- 不同页大小

**预期性能**:
- TLB 热: 50-100 ns
- TLB 冷: 1-5 μs
- 命中率: 95%+

```bash
cargo bench --bench mmu_translate -- --verbose
```

## 3. 性能监控指标

### 3.1 关键指标定义

| 指标 | 单位 | 说明 |
|------|------|------|
| 吞吐量 (Throughput) | ops/sec | 每秒完成的操作数 |
| 延迟 (Latency) | ns/ops | 单个操作的时间 |
| 命中率 (Hit Rate) | % | 缓存命中的百分比 |
| 编译时间 | ms | 编译一个块的时间 |
| 代码大小 | bytes | 编译生成的代码大小 |
| 内存使用 | MB | 峰值内存使用量 |

### 3.2 性能基线

**发布版本性能基线** (Release build, Ubuntu 22.04, Intel i7-12700K):

#### JIT 编译性能
```
小块编译 (< 50 insn):  0.5-1.0 ms
中块编译 (50-500):     2-5 ms
大块编译 (> 500):      10-20 ms
平均代码生成率:        500-1000 bytes/ms
```

#### 执行性能
```
解释器吞吐量:          10-50 Minsn/sec
JIT 吞吐量:            100-500 Minsn/sec
JIT 加速比:            5-10x
浮点操作加速:          10x vs 软件
```

#### MMU 性能
```
TLB 查找 (命中):       50-100 ns
页表遍历 (3级):        1-5 μs
TLB 命中率:            95-99%
```

## 4. 性能回归检测

### 4.1 自动检测

每次提交到 main 分支时运行基准测试，自动检测性能回归：

```yaml
# .github/workflows/benchmark.yml
- name: Compare with baseline
  run: |
    cargo bench --all -- --baseline main
    
- name: Check for regressions
  run: |
    python3 scripts/check_regression.py \
      --baseline main \
      --threshold 5%  # 允许 5% 性能下降
```

### 4.2 本地比较

```bash
# 设置基准线
cargo bench --all -- --save-baseline my_baseline

# 与基准线比较
cargo bench --all -- --baseline my_baseline

# 查看详细对比
cat target/criterion/*/base/raw.json
```

## 5. 优化指南

### 5.1 如何改进 JIT 编译性能

1. **减少编译延迟**:
   ```rust
   // 现有优化
   - Cranelift 编译时 O(n)
   - 并行编译多个块
   - 代码缓存避免重新编译
   
   // 可能的优化
   - 增量编译
   - 编译队列优先级
   - 后台编译线程
   ```

2. **提高执行速度**:
   ```rust
   // 现有优化
   - 快速路径缓存
   - 指令融合
   - SIMD 向量化
   
   // 可能的优化
   - 循环展开
   - 内联小函数
   - 分支预测优化
   ```

### 5.2 如何改进 MMU 性能

1. **提高 TLB 命中率**:
   ```rust
   // 现有优化
   - LRU 驱逐策略
   - 全局页表项
   - 复合键 O(1) 查找
   
   // 可能的优化
   - 预取策略
   - 工作集估计
   - 动态 TLB 大小调整
   ```

2. **加快页表遍历**:
   ```rust
   // 现有优化
   - 缓存页表根指针
   - 批量转换
   
   // 可能的优化
   - 并行遍历
   - 页表预取
   - 超级页支持
   ```

## 6. 性能分析工具

### 6.1 使用 Perf 分析

```bash
# 分析 CPU 时间分布
cargo build --release
perf record -g target/release/vm-cli test.bin
perf report

# 分析缓存行为
perf stat -e cache-references,cache-misses target/release/vm-cli test.bin

# 火焰图生成
perf record -g -F 99 target/release/vm-cli test.bin
perf script | FlameGraph/stackcollapse-perf.pl | FlameGraph/flamegraph.pl > out.svg
```

### 6.2 使用 Criterion.rs 分析

```bash
# 运行基准测试并生成 HTML 报告
cargo bench --all

# 报告位置
target/criterion/report/index.html
```

### 6.3 内存分析

```bash
# 使用 Valgrind 检测内存泄漏
valgrind --leak-check=full --show-leak-kinds=all target/release/vm-cli test.bin

# 使用 Heaptrack 分析内存分配
heaptrack target/release/vm-cli test.bin
heaptrack_gui heaptrack.target.pid.gz
```

## 7. 持续性能监控

### 7.1 每日运行

```bash
# 定时运行基准测试
# 在 crontab 中
0 2 * * * cd /path/to/vm && cargo bench --all > /tmp/bench_$(date +%Y%m%d).log 2>&1
```

### 7.2 收集指标

```python
# scripts/collect_metrics.py
import json
import subprocess
from datetime import datetime

results = {}
for bench in ['jit_compile', 'jit_vs_interp', 'mmu_translate']:
    output = subprocess.check_output(
        f'cargo bench --bench {bench} -- --verbose',
        shell=True,
        text=True
    )
    # 解析输出并存储
    results[bench] = parse_output(output)

# 保存到时间序列数据库
with open(f'metrics_{datetime.now().isoformat()}.json', 'w') as f:
    json.dump(results, f)
```

### 7.3 生成报告

```python
# scripts/generate_report.py
import matplotlib.pyplot as plt
import pandas as pd

# 读取历史数据
df = pd.read_csv('metrics.csv')

# 绘制性能趋势
plt.figure(figsize=(12, 6))
plt.plot(df['date'], df['jit_throughput'], label='JIT Throughput')
plt.plot(df['date'], df['interp_throughput'], label='Interpreter Throughput')
plt.xlabel('Date')
plt.ylabel('Throughput (Minsn/sec)')
plt.title('VM Performance Trends')
plt.legend()
plt.savefig('performance_trends.png')
```

## 8. 预期性能里程碑

| 优化 | 阶段 | 目标吞吐量 | 目标延迟 |
|------|------|----------|---------|
| 基础解释器 | Phase 1 | 10 Minsn/sec | - |
| TLB 优化 | Phase 2 | 15 Minsn/sec | 50 ns |
| 批量操作 | Phase 2 | 20 Minsn/sec | - |
| JIT 基础 | Phase 2 | 100 Minsn/sec | - |
| 浮点加速 | Phase 2 | 150 Minsn/sec | - |
| 完整优化 | Phase 2 | 300-500 Minsn/sec | 2-10 ns |

## 9. 故障排除

### 常见性能问题

#### 问题: JIT 编译太慢
**症状**: 编译时间 > 20ms
**可能原因**:
- 块太大 (> 1000 指令)
- 编译队列积压
- 系统负载过高

**解决方案**:
- 增加编译队列优先级
- 分割大块
- 调整编译阈值

#### 问题: TLB 命中率低
**症状**: 命中率 < 80%
**可能原因**:
- TLB 大小太小
- 工作集太大
- 随机访问模式

**解决方案**:
- 增加 TLB 大小
- 启用预取策略
- 优化访问模式

---

**最后更新**: 2025年11月29日
**版本**: 1.0
