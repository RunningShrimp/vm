# 性能基准测试文档

**创建日期**: 2026-01-03
**Rust版本**: 1.92.0
**目的**: 建立和跟踪VM项目的关键性能指标

---

## 📊 性能基准概览

### 测试环境
- **平台**: macOS (Darwin 25.2.0)
- **CPU架构**: 待记录
- **内存**: 待记录
- **Rust版本**: 1.92.0
- **编译模式**: dev (debug) / release (optimized)

---

## 🎯 关键性能指标 (KPIs)

### 1. MMU性能指标

#### 1.1 页表翻译性能
**基准文件**: `vm-mem/benches/mmu_translate.rs`

**指标**:
- **Bare模式翻译延迟**: < 10ns
- **SV39模式翻译延迟**: < 50ns (TLB命中)
- **TLB未命中惩罚**: < 500ns
- **页表遍历时间**: < 200ns (4级页表)

**测试命令**:
```bash
cargo bench -p vm-mem --bench mmu_translate
```

#### 1.2 TLB性能
**基准文件**: `vm-mem/benches/tlb_optimized.rs`

**指标**:
- **L1 DTLB命中率**: > 95%
- **L1 ITLB命中率**: > 95%
- **L2 TLB命中率**: > 90%
- **TLB刷新开销**: < 1ms (全TLB刷新)

**测试命令**:
```bash
cargo bench -p vm-mem --bench tlb_optimized
```

#### 1.3 并发TLB性能
**基准文件**: `vm-mem/benches/lockfree_tlb.rs`

**指标**:
- **4线程并发翻译吞吐**: > 10M ops/sec
- **8线程并发翻译吞吐**: > 15M ops/sec
- **锁竞争开销**: < 5%

**测试命令**:
```bash
cargo bench -p vm-mem --bench lockfree_tlb
```

---

### 2. JIT编译性能

#### 2.1 基本块编译
**基准文件**: `vm-engine/benches/jit_compilation_bench.rs`

**指标**:
- **简单指令编译速度**: > 1M instr/sec
- **复杂指令编译速度**: > 100K instr/sec
- **平均基本块编译时间**: < 100μs
- **编译缓存命中率**: > 80%

**测试命令**:
```bash
cargo bench -p vm-engine --bench jit_compilation_bench
```

#### 2.2 分支预测性能
**基准文件**: `vm-engine-jit/benches/block_chaining_bench.rs`

**指标**:
- **分支预测准确率**: > 85%
- **间接分支预测准确率**: > 70%
- **块链优化性能提升**: > 30%

**测试命令**:
```bash
cargo bench -p vm-engine-jit --bench block_chaining_bench
```

---

### 3. 跨架构翻译性能

#### 3.1 指令翻译
**基准文件**: `vm-cross-arch-support/benches/cross_arch_translation_bench.rs`

**指标**:
- **x86_64 → ARM64翻译速度**: > 500K instr/sec
- **ARM64 → RISC-V翻译速度**: > 500K instr/sec
- **翻译缓存命中率**: > 90%
- **翻译准确率**: 100%

**测试命令**:
```bash
cargo bench -p vm-cross-arch-support --bench cross_arch_translation_bench
```

#### 3.2 寄存器映射性能
**指标**:
- **寄存器映射查找时间**: < 10ns
- **热寄存器识别准确率**: > 80%
- **映射缓存命中率**: > 95%

---

### 4. 内存管理性能

#### 4.1 内存分配
**基准文件**: `vm-optimizers/benches/memory_allocation_bench.rs`

**指标**:
- **小对象分配速度**: > 10M allocs/sec
- **大对象分配速度**: > 1M allocs/sec
- **分配延迟 (P50)**: < 100ns
- **分配延迟 (P99)**: < 1μs

**测试命令**:
```bash
cargo bench -p vm-optimizers --bench memory_allocation_bench
```

#### 4.2 SIMD内存操作
**基准文件**: `vm-mem/benches/simd_memcpy.rs`

**指标**:
- **内存拷贝速度**: > 10GB/s (AVX2)
- **内存填充速度**: > 10GB/s (AVX2)
- **比较操作速度**: > 5GB/s
- **SIMD加速比**: > 4x (vs 标准库)

**测试命令**:
```bash
cargo bench -p vm-mem --bench simd_memcpy
```

---

### 5. GC性能

#### 5.1 GC停顿时间
**基准文件**: `vm-optimizers/benches/gc_bench.rs`

**指标**:
- **Minor GC停顿**: < 10ms (1GB堆)
- **Major GC停顿**: < 100ms (1GB堆)
- **GC吞吐量**: > 1GB/sec
- **GC开销**: < 5% (总运行时间)

**测试命令**:
```bash
cargo bench -p vm-optimizers --bench gc_bench
```

---

## 📈 性能回归检测

### 回归阈值
- **严重回归**: 性能下降 > 10%
- **警告回归**: 性能下降 5-10%
- **正常波动**: 性能变化 < 5%

### CI/CD集成
**工作流**: `.github/workflows/performance.yml`
- **触发条件**:
  - Push到master/main分支
  - Pull Request
  - 每日定时任务 (凌晨2点 UTC)
  - 手动触发

**监控指标**:
- MMU翻译性能
- JIT编译性能
- 跨架构翻译性能
- 内存管理性能
- GC性能

---

## 🚀 运行完整基准测试

### 快速测试 (关键指标)
```bash
# 运行关键基准测试 (约5分钟)
./scripts/run_benchmarks.sh --quick
```

### 完整测试 (所有指标)
```bash
# 运行所有基准测试 (约30-60分钟)
./scripts/run_benchmarks.sh --all

# 或使用bench.sh
./scripts/bench.sh
```

### 更新Baseline
```bash
# 运行基准测试并更新baseline
./scripts/run_benchmarks.sh --all --update-baseline
```

### 生成性能报告
```bash
# 生成性能报告
python3 scripts/generate_benchmark_report.py
```

---

## 📊 性能Baseline数据

### 待建立 (首次运行后填写)

### MMU性能Baseline (2026-01-03建立)
```
平台: macOS (Darwin 25.2.0)
Rust版本: 1.92.0
编译模式: release (optimized)

#### 1. 地址翻译性能
Bare模式翻译: 1 ns/iter
TLB性能 (不同页面数):
  - 1页面: 1 ns/iter (TLB完全命中)
  - 10页面: 13 ns/iter
  - 64页面: 83 ns/iter
  - 128页面: 169 ns/iter
  - 256页面: 346 ns/iter (TLB未命中增加)

#### 2. 内存读取性能
不同大小读取:
  - 1字节: 5 ns/iter
  - 2字节: 5 ns/iter
  - 4字节: 5 ns/iter
  - 8字节: 4 ns/iter

#### 3. 内存写入性能
不同大小写入:
  - 1字节: 6 ns/iter
  - 2字节: 6 ns/iter
  - 4字节: 6 ns/iter
  - 8字节: 6 ns/iter

#### 4. 批量访问性能
顺序读取 (1K次): 4,876 ns/iter (4.876 μs)
随机读取 (1K次): 4,726 ns/iter (4.726 μs)

#### 性能分析
✅ TLB命中率极高 (小页面数)
✅ 内存读写延迟极低 (4-6 ns)
✅ 批量吞吐量: ~205K ops/sec (1K次操作)
```

#### JIT编译Baseline
```
简单指令编译: XXX instr/sec
复杂指令编译: XXX instr/sec
基本块编译时间: XX μs
```

#### 跨架构翻译Baseline
```
x86_64 → ARM64: XXX instr/sec
ARM64 → RISC-V: XXX instr/sec
翻译缓存命中率: XX%
```

---

## 🔍 性能分析工具

### Criterion
- **用途**: 统计显著的基准测试框架
- **输出**: HTML报告，包含置信区间
- **安装**: 已集成在项目中

### critcmp
- **用途**: 比较不同benchmark运行结果
- **安装**: `cargo install critcmp`
- **使用**: `critcmp main new`

### Flamegraph
- **用途**: CPU性能分析
- **安装**: `cargo install flamegraph`
- **使用**: `cargo flamegraph --bench <bench_name>`

---

## 📝 性能测试最佳实践

### 1. 测试环境一致性
- 使用相同的硬件配置
- 禁用电源管理和Turbo Boost
- 关闭不必要的后台进程
- 使用release模式进行测试

### 2. 测试稳定性
- 每个benchmark运行至少10次
- 使用统计显著性检验 (Criterion自动处理)
- 记录环境信息 (CPU、内存、OS)
- 保存原始数据用于回溯分析

### 3. 回归分析
- 建立性能baseline作为参考
- 设置合理的回归阈值
- 使用CI/CD自动检测回归
- 对于异常结果进行人工审查

### 4. 性能优化循环
1. 运行baseline测试
2. 识别性能瓶颈
3. 实施优化
4. 验证性能提升
5. 更新baseline
6. 持续监控

---

## 🎯 下一步行动

- [ ] 运行完整基准测试建立初始baseline
- [ ] 配置CI/CD性能监控阈值
- [ ] 创建性能趋势仪表板
- [ ] 建立性能回归应急流程
- [ ] 定期审查和更新KPI

---

## 📚 相关文档

- [Feature规范化计划](../FEATURE_NORMALIZATION_PLAN.md)
- [CI/CD质量门禁](./QUALITY_GATES_QUICK_REFERENCE.md)
- [性能监控工作流](../.github/workflows/performance.yml)
- [Benchmark运行脚本](../scripts/run_benchmarks.sh)

---

*文档维护: 每次重大性能变更后更新*
*最后更新: 2026-01-03*
