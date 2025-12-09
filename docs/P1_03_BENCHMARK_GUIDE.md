# P1-03: 性能基准框架实现指南

**目标**: 创建全面的性能基准测试框架，量化JIT/AOT/GC/TLB/跨架构的性能

**完成标准**:
- ✅ 基准测试框架设计完成
- ✅ 6个基准测试类别 (JIT/AOT/GC/TLB/CrossArch/Scheduler)
- ✅ 自动数据收集和导出
- ✅ CI/CD集成
- ✅ 性能目标追踪 (<500ns async overhead)
- ✅ 关键性能指标 (KPI) 定义

**时间线**: 1.5周(7个工作日)

---

## 基准框架设计

### 整体架构

```
┌─────────────────────────────────────┐
│   Benchmark Suite (性能测试协调)    │
├──────┬──────┬──────┬──────┬──────┬──┤
│ JIT  │ AOT  │ GC   │ TLB  │Cross │Sch│
├──────┴──────┴──────┴──────┴──────┴──┤
│   Result Collector (结果收集)        │
├──────────────────────────────────────┤
│   Data Export (CSV/JSON/Markdown)    │
├──────────────────────────────────────┤
│   Visualization (Charts/Reports)     │
└──────────────────────────────────────┘
```

### 核心数据结构

```rust
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: u64,
    pub total_time_us: u64,
    pub avg_time_us: f64,
    pub min_time_us: u64,
    pub max_time_us: u64,
    pub stddev_us: f64,
    pub throughput: f64,  // ops/sec
}

pub struct BenchmarkSuite {
    results: Vec<BenchmarkResult>,
}
```

---

## 基准测试类别

### 1. JIT编译基准

**目标指标**:
- 编译延迟: <500µs/块
- 缓存命中率: >90%
- 热点检测延迟: <10µs

**测试用例**:

```rust
// JIT编译延迟测试
async fn bench_compilation_latency(iterations: u64) -> BenchmarkResult {
    let mut times = Vec::new();
    
    for _ in 0..iterations {
        let start = Instant::now();
        
        // 1. IR优化
        let ir_optimized = optimize_ir(&ir_block);
        
        // 2. 机器码生成
        let machine_code = compile_to_machine_code(&ir_optimized);
        
        // 3. 代码缓存
        cache.insert(block_addr, machine_code);
        
        times.push(start.elapsed().as_micros() as u64);
    }
    
    BenchmarkResult::new("JIT Compilation Latency", iterations, times)
}

// 块缓存效率测试
async fn bench_block_caching(iterations: u64) -> BenchmarkResult {
    let mut cache = DashMap::new();
    let mut times = Vec::new();
    
    for i in 0..iterations {
        let start = Instant::now();
        
        let block_id = i % 100; // 100个不同的块
        
        if !cache.contains_key(&block_id) {
            // 缓存未命中：编译
            let code = compile_block(block_id).await;
            cache.insert(block_id, code);
        } else {
            // 缓存命中：直接执行
            let _ = cache.get(&block_id);
        }
        
        times.push(start.elapsed().as_micros() as u64);
    }
    
    // 分析结果
    let hit_rate = calculate_hit_rate(&times);
    println!("Cache hit rate: {:.1}%", hit_rate * 100.0);
    
    BenchmarkResult::new("JIT Block Caching", iterations, times)
}

// 混合执行模式对比
async fn bench_mixed_execution(jit_ratio: f64, iterations: u64) -> BenchmarkResult {
    let mut times = Vec::new();
    let jit_count = (iterations as f64 * jit_ratio) as u64;
    
    for i in 0..iterations {
        let start = Instant::now();
        
        if i < jit_count {
            // JIT执行 (~2µs)
            execute_jit_compiled_block().await;
        } else {
            // 解释器执行 (~5µs)
            execute_interpreted_block().await;
        }
        
        times.push(start.elapsed().as_micros() as u64);
    }
    
    BenchmarkResult::new("Mixed JIT/Interpreter", iterations, times)
}
```

### 2. AOT编译基准

**目标指标**:
- AOT编译: <1ms/函数
- 镜像加载: <100ms
- 启动加速: >3倍相对于JIT

**测试用例**:

```rust
async fn bench_aot_compilation(iterations: u64) -> BenchmarkResult {
    // 全函数AOT编译（包括优化）
}

async fn bench_aot_image_loading(iterations: u64) -> BenchmarkResult {
    // 加载预编译AOT镜像 (mmap)
}

async fn bench_aot_startup_vs_jit(iterations: u64) -> BenchmarkResult {
    // 对比AOT启动 vs JIT启动延迟
}
```

### 3. 垃圾回收基准

**目标指标**:
- GC暂停时间: <100ms
- 吞吐量: >99%
- 并发标记: 支持多线程

**测试用例**:

```rust
async fn bench_gc_mark_phase(heap_size: usize, iterations: u64) {
    // 标记阶段性能 (对象遍历)
}

async fn bench_gc_sweep_phase(iterations: u64) {
    // 清扫阶段性能 (内存回收)
}

async fn bench_gc_pause_time(iterations: u64) {
    // 总体GC暂停时间
    // 目标: <100ms
}

async fn bench_concurrent_gc(iterations: u64) {
    // 并发GC vs STW GC对比
}
```

### 4. TLB基准

**目标指标**:
- TLB命中率: >95%
- 缺失处理: <1µs
- 预取效率: >10%性能提升

**测试用例**:

```rust
async fn bench_tlb_hit_rate(
    iterations: u64,
    working_set_size: usize,
) -> BenchmarkResult {
    // 测试不同工作集大小下的命中率
}

async fn bench_tlb_miss_latency(iterations: u64) {
    // TLB缺失处理延迟
}

async fn bench_address_translation_cache(iterations: u64) {
    // 带缓存的地址转换性能
}
```

### 5. 跨架构基准

**目标指标**:
- x86↔ARM转换: <100ns/指令
- ARM↔RISC-V转换: <100ns/指令
- 混合架构执行: <5%开销

**测试用例**:

```rust
async fn bench_x86_to_arm_translation(iterations: u64) {
    // x86指令转换为ARM
}

async fn bench_arm_to_riscv_translation(iterations: u64) {
    // ARM指令转换为RISC-V
}

async fn bench_cross_arch_execution(iterations: u64) {
    // 混合架构执行（切换开销）
}
```

### 6. 调度器基准

**目标指标**:
- 协程创建: <10µs
- 调度延迟: <100µs
- 负载均衡: <30%差异

**测试用例**:

```rust
async fn bench_coroutine_creation(iterations: u64) {
    // 协程创建延迟
}

async fn bench_scheduling_latency(iterations: u64) {
    // 调度决策延迟
}

async fn bench_load_balancing_overhead(iterations: u64) {
    // 负载均衡计算开销
}
```

---

## 实现步骤

### Day 1-2: 基准框架核心

1. **BenchmarkResult 结构**
```rust
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: u64,
    pub total_time_us: u64,
    pub avg_time_us: f64,
    pub min_time_us: u64,
    pub max_time_us: u64,
    pub stddev_us: f64,
    pub throughput: f64,
    
    // 计算标准差、吞吐量等统计信息
}
```

2. **数据收集**
```rust
pub struct BenchmarkSuite {
    results: Vec<BenchmarkResult>,
    
    pub fn add_result(&mut self, result: BenchmarkResult);
    pub fn display_summary(&self);
    pub fn export_csv(&self, filename: &str);
    pub fn export_json(&self, filename: &str);
}
```

### Day 3-4: 各类别基准实现

按顺序实现：
1. JIT基准 (3个测试)
2. AOT基准 (3个测试)
3. GC基准 (4个测试)
4. TLB基准 (3个测试)
5. 跨架构基准 (3个测试)
6. 调度器基准 (3个测试)

**总计: 19个基准测试**

### Day 5: 数据导出和可视化

```rust
impl BenchmarkSuite {
    pub fn export_csv(&self, filename: &str) -> io::Result<()> {
        // CSV导出
    }
    
    pub fn export_json(&self, filename: &str) -> io::Result<()> {
        // JSON导出
    }
    
    pub fn generate_report(&self) -> String {
        // 生成Markdown报告
    }
}
```

### Day 6: CI/CD集成

```yaml
# .github/workflows/benchmarks.yml
name: Performance Benchmarks

on: [push, pull_request]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
      - run: cargo test --bench performance_suite
      - run: cargo bench --bench performance_suite -- --save-baseline main
      - uses: actions/upload-artifact@v2
        with:
          name: benchmark-results
          path: target/criterion/
```

### Day 7: 性能目标追踪

```rust
#[derive(Debug)]
pub struct PerformanceTarget {
    pub name: String,
    pub metric: String,
    pub target_value: f64,
    pub tolerance: f64, // ±tolerance%
}

pub fn validate_targets(results: &[BenchmarkResult], targets: &[PerformanceTarget]) -> bool {
    // 验证是否满足所有性能目标
    // 输出Fail/Pass报告
}
```

---

## 性能目标 (KPI)

| 类别 | 指标 | 目标 | 验证方法 |
|------|------|------|---------|
| **JIT** | 编译延迟 | <500µs | bench_compilation_latency |
| | 缓存命中 | >90% | bench_block_caching |
| | 混合执行 | <5%开销 | bench_mixed_execution |
| **AOT** | 编译延迟 | <1ms | bench_aot_compilation |
| | 启动加速 | >3倍 | bench_aot_startup_vs_jit |
| **GC** | 暂停时间 | <100ms | bench_gc_pause_time |
| | 吞吐量 | >99% | bench_gc_throughput |
| **TLB** | 命中率 | >95% | bench_tlb_hit_rate |
| | 缺失延迟 | <1µs | bench_tlb_miss_latency |
| **CrossArch** | 转换延迟 | <100ns | bench_*_translation |
| **Scheduler** | 协程创建 | <10µs | bench_coroutine_creation |
| | 调度延迟 | <100µs | bench_scheduling_latency |

---

## 基准运行和报告

### 运行单个基准

```bash
cargo test --lib async_executor::tests::bench_async_execution_latency
```

### 运行所有基准

```bash
cargo test --bench performance_suite -- --nocapture
```

### 生成对比报告

```bash
cargo bench --bench performance_suite -- --save-baseline v1.0
cargo bench --bench performance_suite -- --baseline v1.0
```

### 输出示例

```
======================================================================
VIRTUAL MACHINE PERFORMANCE BENCHMARK REPORT
======================================================================

======================================================================
Benchmark: JIT Compilation Latency
======================================================================
Iterations:              1000
Total time:           451.234 ms
Avg time:             451.234 µs
Min time:             420 µs
Max time:             500 µs
Std dev:               32.150 µs
Throughput:          2215.5 ops/sec
======================================================================

[更多基准结果...]

======================================================================
OVERALL STATISTICS
======================================================================
Total benchmarks: 19
Average throughput: 2451.3 ops/sec
======================================================================
```

---

## 关键文件清单

| 文件 | 行数 | 内容 |
|------|------|------|
| benches/performance_suite.rs | 500+ | 完整基准实现 |
| benches/lib.rs | 50 | 模块组织 |
| docs/P1_03_BENCHMARK_GUIDE.md | 300 | 本文档 |
| .github/workflows/benchmarks.yml | 30 | CI配置 |
| **总计** | **880+** | |

---

## 与P1-01/P1-02的协作

### P1-01（异步执行引擎）验证

```rust
// 验证P1-01的性能目标
#[test]
fn verify_async_overhead() {
    // 目标: 异步开销 <500ns
    let sync_time = bench_sync_execution(1000).await;
    let async_time = bench_async_execution(1000).await;
    let overhead_ns = (async_time.avg_time_us - sync_time.avg_time_us) * 1000.0;
    assert!(overhead_ns < 500.0);
}
```

### P1-02（调度器）验证

```rust
// 验证P1-02的负载均衡
#[test]
fn verify_load_balance() {
    // 目标: 负载差异 <30%
    let utilizations = get_vcpu_utilizations();
    let max = utilizations.iter().fold(0.0, |a, &b| a.max(b));
    let min = utilizations.iter().fold(1.0, |a, &b| a.min(b));
    assert!((max - min) < 0.3);
}
```

---

## 后续和改进

1. **可视化增强**:
   - 集成plotly或gnuplot生成图表
   - 实时性能仪表板

2. **高级分析**:
   - 性能回归检测
   - 趋势分析
   - 异常检测

3. **对标支持**:
   - 与业界VM对标 (V8, JVM, .NET)
   - 相对性能分析

4. **分布追踪**:
   - 详细的调用栈分析
   - 火焰图生成
