# P1-03: 性能基准测试框架 - 设计文档

## 概述

为虚拟机实现comprehensive的性能基准测试框架,覆盖JIT、AOT、GC、TLB和跨架构执行。

## 基准测试范围

### 1. JIT编译性能
- **编译速度**: ops/ms (操作每毫秒)
- **代码大小**: bytes per operation
- **缓存效率**: hit rate (%)
- **热点检测**: detection latency (ms)

### 2. AOT编译性能
- **预编译时间**: total compile time (ms)
- **二进制大小**: final executable size
- **启动时间**: cold start latency
- **内存使用**: peak memory during compilation

### 3. GC性能
- **GC延迟**: pause time (ms)
- **吞吐量**: operations between GC (ops)
- **内存利用**: live objects / total heap (%)
- **收集效率**: bytes collected per cycle

### 4. TLB性能
- **命中率**: TLB hit rate (%)
- **翻译延迟**: address translation latency
- **缓存大小**: entries per level
- **失效率**: TLB invalidations per 1M ops

### 5. 跨架构执行
- **转换开销**: x86 to ARM conversion cost
- **执行效率**: native vs translated ratio
- **指令映射**: average instructions mapped per source

## 基准测试框架

```
benches/
├── common/
│   ├── mod.rs              # 共享工具
│   ├── timer.rs            # 高精度计时
│   └── metrics.rs          # 指标收集
├── jit_benchmark.rs        # JIT编译基准
├── aot_benchmark.rs        # AOT编译基准
├── gc_benchmark.rs         # GC性能基准
├── tlb_benchmark.rs        # TLB命中率基准
└── cross_arch_benchmark.rs # 跨架构执行基准
```

## 每个基准的详细设计

### JIT编译基准

```rust
pub struct JitBenchmark {
    executor: JitExecutor,
    block_count: u32,
    operations_per_block: u32,
}

impl JitBenchmark {
    // 热身运行
    pub fn warmup(&mut self) { }
    
    // 编译基准
    pub fn bench_compilation(&mut self) -> Metrics { }
    
    // 缓存命中基准
    pub fn bench_cache_hits(&mut self) -> Metrics { }
    
    // 执行吞吐量
    pub fn bench_throughput(&mut self) -> Metrics { }
}
```

**Metrics输出:**
```
JIT Compilation:
  Total time: 1234.56 ms
  Blocks compiled: 1000
  Compile time per block: 1.23 ms
  Cache hit rate: 95.2%
  Throughput: 810.4 ops/ms
```

### AOT编译基准

```rust
pub struct AotBenchmark {
    builder: AotBuilder,
    module_count: u32,
}

impl AotBenchmark {
    pub fn bench_compilation_speed(&mut self) -> Metrics { }
    pub fn bench_binary_size(&mut self) -> Metrics { }
    pub fn bench_startup_time(&mut self) -> Metrics { }
}
```

**Metrics输出:**
```
AOT Compilation:
  Compilation time: 5678.90 ms
  Binary size: 2.45 MB
  Size per module: 10.2 KB
  Startup latency: 234.5 ms
```

### GC性能基准

```rust
pub struct GcBenchmark {
    heap: GarbageCollector,
    allocation_rate: u64, // bytes/ms
    object_size: u32,
}

impl GcBenchmark {
    pub fn bench_pause_time(&mut self) -> Metrics { }
    pub fn bench_throughput(&mut self) -> Metrics { }
    pub fn bench_memory_efficiency(&mut self) -> Metrics { }
}
```

**Metrics输出:**
```
GC Performance:
  Pause time: 12.3 ms (max)
  Collection rate: 456.7 MB/s
  Live object ratio: 42.3%
  Collections per 1M ops: 12
  Total GC time: 234.5 ms (3.2% of total)
```

### TLB性能基准

```rust
pub struct TlbBenchmark {
    tlb: TlbManager,
    working_set_size: u64, // bytes
}

impl TlbBenchmark {
    pub fn bench_hit_rate(&mut self) -> Metrics { }
    pub fn bench_translation_latency(&mut self) -> Metrics { }
    pub fn bench_invalidation_overhead(&mut self) -> Metrics { }
}
```

**Metrics输出:**
```
TLB Performance:
  L1 TLB hit rate: 98.5%
  L2 TLB hit rate: 96.3%
  Average translation latency: 2.4 ns
  Invalidations per 1M accesses: 45
  Memory saved by TLB: 1.2 GB (vs. full walk)
```

### 跨架构执行基准

```rust
pub struct CrossArchBenchmark {
    executor: HybridExecutor,
    source_arch: Architecture,
    target_arch: Architecture,
}

impl CrossArchBenchmark {
    pub fn bench_x86_to_arm(&mut self) -> Metrics { }
    pub fn bench_x86_to_riscv(&mut self) -> Metrics { }
    pub fn bench_arm_to_x86(&mut self) -> Metrics { }
}
```

**Metrics输出:**
```
Cross-Architecture Execution (x86→ARM):
  Translation overhead: 12.3%
  Instruction ratio: 1.4x (1.4 ARM per x86)
  Average latency: 34.5 ms per block
  Correctness: PASS (all tests)
```

## 共享工具

### 计时器(Timer)

```rust
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn start() -> Self { }
    pub fn elapsed_ns(&self) -> u64 { }
    pub fn elapsed_us(&self) -> u64 { }
    pub fn elapsed_ms(&self) -> f64 { }
}
```

### 指标收集(Metrics)

```rust
#[derive(Debug, Clone)]
pub struct Metrics {
    pub name: String,
    pub total_time_ms: f64,
    pub operations: u64,
    pub throughput_ops_per_ms: f64,
    pub latency_us: f64,
}

impl Metrics {
    pub fn calculate(&self) -> Self { }
    pub fn print(&self) { }
    pub fn to_csv(&self) -> String { }
}
```

## 运行方式

### 使用cargo bench

```bash
# 运行所有基准
cargo bench

# 运行特定基准
cargo bench -- jit
cargo bench -- aot
cargo bench -- gc

# 保存结果
cargo bench -- --save-baseline baseline1
```

### 使用criterion.rs(可选)

```toml
[dev-dependencies]
criterion = "0.5"
```

```bash
cargo bench -- --compare
```

## 性能目标

| 组件 | 指标 | 目标 |
|-----|------|-----|
| JIT | 编译速度 | >1000 ops/ms |
| JIT | 缓存命中率 | >95% |
| AOT | 编译时间 | <10 s/module |
| AOT | 二进制大小 | <10 KB/1000 ops |
| GC | 暂停时间 | <50 ms |
| GC | 吞吐量 | >95% |
| TLB | L1命中率 | >98% |
| TLB | L2命中率 | >95% |
| CrossArch | 开销 | <20% |
| CrossArch | 指令比 | <2.0x |

## 输出格式

### 文本格式

```
╔════════════════════════════════════════════════════════╗
║          Performance Benchmark Results                  ║
╚════════════════════════════════════════════════════════╝

JIT Compilation (1000 blocks, 10000 ops/block)
├─ Compilation time: 1234.56 ms
├─ Throughput: 810.4 ops/ms
├─ Cache hit rate: 95.2%
└─ Status: ✓ PASS

AOT Compilation (10 modules)
├─ Total time: 5678.90 ms
├─ Binary size: 2.45 MB
├─ Per-module size: 245 KB
└─ Status: ✓ PASS

GC Performance (heap size: 1GB)
├─ Max pause time: 12.3 ms
├─ Collection rate: 456.7 MB/s
├─ Live object ratio: 42.3%
└─ Status: ✓ PASS

TLB Performance (4M memory accesses)
├─ L1 hit rate: 98.5%
├─ L2 hit rate: 96.3%
├─ Translation latency: 2.4 ns
└─ Status: ✓ PASS

Cross-Arch Execution (10000 blocks x86→ARM)
├─ Translation overhead: 12.3%
├─ Instruction ratio: 1.4x
├─ Latency: 34.5 ms/block
└─ Status: ✓ PASS

═══════════════════════════════════════════════════════════
Overall: ✓ All benchmarks PASSED
Generated: 2024-12-09 21:30:00 UTC
```

### CSV格式

```csv
benchmark,metric,value,unit,status
JIT,compilation_time,1234.56,ms,PASS
JIT,throughput,810.4,ops/ms,PASS
JIT,cache_hit_rate,95.2,%,PASS
AOT,compilation_time,5678.9,ms,PASS
AOT,binary_size,2.45,MB,PASS
GC,max_pause_time,12.3,ms,PASS
...
```

### JSON格式

```json
{
  "timestamp": "2024-12-09T21:30:00Z",
  "results": {
    "jit": {
      "compilation_time_ms": 1234.56,
      "throughput_ops_per_ms": 810.4,
      "cache_hit_rate_percent": 95.2,
      "status": "PASS"
    },
    ...
  },
  "summary": {
    "total_tests": 5,
    "passed": 5,
    "failed": 0
  }
}
```

## 实现计划

### 第1阶段: 框架搭建(1-2天)

1. **共享工具**
   - Timer类
   - Metrics类
   - 输出格式化

2. **基准基类**
   - Benchmark trait
   - 共同的运行逻辑

### 第2阶段: 各组件基准(3-4天)

1. **JIT基准** - 2个指标
2. **AOT基准** - 2个指标
3. **GC基准** - 3个指标
4. **TLB基准** - 3个指标
5. **CrossArch基准** - 2个指标

### 第3阶段: 集成与报告(1-2天)

1. **性能持续监控**
   - 保存baseline
   - 对比回归

2. **报告生成**
   - HTML报告
   - 趋势分析

## 与现有系统的集成

```
async-executor
      ↓
(执行blocks)
      ↓
coroutine-scheduler
      ↓
(分配vCPU)
      ↓
Benchmark Suite
      ↓
(收集指标)
      ↓
Reports
```

## 成功标准

- ✓ 所有5个基准都能运行
- ✓ 指标都在目标范围内
- ✓ 可以持续监控性能
- ✓ 能检测性能回归
- ✓ 输出清晰易读
