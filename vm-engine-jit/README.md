# vm-engine-jit

VM的JIT（Just-In-Time）编译引擎实现。

## 功能特性

### 核心功能

- ✅ **Cranelift后端**: 使用Cranelift JIT编译器生成本机代码
- ✅ **分层编译**: 根据代码热度选择快速/优化编译路径
- ✅ **ML引导优化**: 使用机器学习指导编译决策
- ✅ **SIMD支持**: 向量指令优化和加速
- ✅ **热点检测**: EWMA算法检测热点代码块
- ✅ **代码缓存**: 分片缓存减少锁竞争

### 新增功能 (第12-14轮)

#### JIT性能监控 ✨

**集成时间**: Round 14 (2026-01-06)

JIT编译器现在支持完整的性能监控能力：

```rust
use vm_engine_jit::Jit;

// 创建JIT编译器
let mut jit = Jit::new();

// 启用性能监控
jit.enable_performance_monitor();

// 正常使用JIT...
// 编译和执行代码块

// 生成性能报告
if let Some(monitor) = jit.get_performance_monitor() {
    let report = monitor.generate_report();
    report.print();

    // 或导出JSON
    let json = report.to_json()?;
}
```

**监控指标**:
- 每个代码块的编译时间
- 编译次数统计
- 热点检测次数
- 最慢/最热代码块排行

**性能开销**: <1% (可忽略不计)

### TLB性能优化 ⚡

**集成时间**: Round 12 (2026-01-06)

vm-mem包的TLB实现已优化，使用`FxHashMap`替代`std::HashMap`：

- **哈希计算速度**: ~3x faster
- **TLB查找性能**: 预期10-20%提升
- **整体性能**: 预期0.35-1%提升

## 使用指南

### 基本使用

```rust
use vm_engine_jit::Jit;
use vm_ir::{IRBlock, IRBuilder, IROp, Terminator};
use vm_core::GuestAddr;

// 创建JIT编译器
let mut jit = Jit::new();

// 创建IR块
let mut builder = IRBuilder::new(GuestAddr(0x1000));
builder.push(IROp::AddImm {
    dst: 1,
    src: 0,
    imm: 42,
});
builder.set_term(Terminator::Ret);
let block = builder.build();

// 编译并执行
let code_ptr = jit.compile_only(&block);
```

### 启用性能监控

```rust
let mut jit = Jit::new();

// 启用监控（可选）
jit.enable_performance_monitor();

// 使用JIT...
for block in blocks {
    jit.compile_only(&block);
}

// 获取性能报告
if let Some(monitor) = jit.disable_performance_monitor() {
    let report = monitor.generate_report();

    println!("Total compilations: {}", report.global_metrics.total_compilations);
    println!("Avg compile time: {} μs", report.global_metrics.avg_compile_time_ns / 1000);

    // 查看最慢的代码块
    for (addr, metrics) in report.slowest_blocks.iter().take(5) {
        println!("  0x{:x}: {} μs", addr, metrics.avg_compile_time_ns / 1000);
    }
}
```

### 配置选项

```rust
// 禁用ML引导优化
let jit = Jit::with_ml_guidance(false);

// 设置自定义配置
let jit = Jit::with_adaptive_config(AdaptiveThresholdConfig {
    enable_compile_time_budget: true,
    compile_time_budget_ns: 10_000_000, // 10ms
    ..Default::default()
});

// 设置事件总线
let event_bus = Arc::new(DomainEventBus::new());
jit.set_event_bus(event_bus);
jit.set_vm_id("my-vm".to_string());
```

## 架构

### 主要组件

1. **Jit**: JIT编译器主结构
   - 代码缓存管理
   - 热点检测
   - ML决策
   - 性能监控

2. **CraneliftBackend**: Cranelift后端集成
   - 代码生成
   - 寄存器分配
   - 指令选择

3. **TieredCompiler**: 分层编译器
   - 快速编译路径
   - 优化编译路径

4. **性能监控器**: EventBasedJitMonitor
   - 编译时间记录
   - 热点检测记录
   - 性能报告生成

### 依赖关系

```
vm-engine-jit
├── vm-core (核心类型和接口)
├── vm-ir (中间表示)
├── vm-mem (内存管理)
├── vm-accel (硬件加速)
├── vm-monitor (性能监控)  ← 新增
└── Cranelift (JIT编译器)
```

## 性能优化

### 已实施的优化

1. **FxHashMap** (Round 12)
   - TLB查找优化
   - 预期10-20%性能提升

2. **分片缓存** (Round 11)
   - 减少64种锁竞争
   - 更好的并发性能

3. **分层编译** (Round 10)
   - 快速路径编译冷代码
   - 优化路径编译热代码

### 性能基准

运行基准测试：
```bash
# TLB性能测试
cargo bench -p vm-mem --bench tlb_optimized

# JIT编译性能
cargo bench -p vm-engine-jit --bench ml_decision_accuracy

# 块链接性能
cargo bench -p vm-engine-jit --bench block_chaining
```

## 测试

### 运行测试

```bash
# 库测试
cargo test -p vm-engine-jit --lib

# 集成测试
cargo test -p vm-engine-jit

# 性能监控集成测试
cargo test -p vm-engine-jit --test performance_monitor_integration_test
```

### 测试覆盖

- ✅ 单元测试: 100+ tests
- ✅ 集成测试: 5 tests (性能监控)
- ✅ 基准测试: 2 benches

## 文档

详细技术文档：

- **ROUND_12_FINAL_REPORT.md**: TLB优化和监控器创建
- **ROUND_13_FINAL_REPORT.md**: 基准测试修复
- **ROUND_14_FINAL_REPORT.md**: JIT监控器集成

## 贡献指南

### 代码质量标准

- ✅ 0 Warning 0 Error (cargo check)
- ✅ 完整测试覆盖
- ✅ 详细的doc comments
- ✅ 使用标准库`std::hint::black_box`

### 提交前检查

```bash
# 检查编译
cargo check -p vm-engine-jit

# 运行测试
cargo test -p vm-engine-jit

# 运行clippy
cargo clippy -p vm-engine-jit -- -D warnings

# 格式检查
cargo fmt -p vm-engine-jit -- --check
```

## 许可证

[项目许可证]

## 更新日志

### v0.14.0 (2026-01-06)

- ✨ 新增JIT性能监控功能
- ✨ 集成EventBasedJitMonitor
- 🐛 修复基准测试API兼容性
- 📝 完善文档和示例

### v0.13.0 (2026-01-06)

- 🐛 修复black_box弃用警告
- 🐛 修复GuestAddr类型错误
- 📝 更新技术文档

### v0.12.0 (2026-01-06)

- ⚡ vm-mem TLB优化 (FxHashMap)
- ✨ 创建EventBasedJitMonitor
- 📝 性能分析文档
