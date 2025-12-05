# 分层编译策略实现文档

## 概述

分层编译策略根据代码块的执行次数选择不同的编译策略，在编译时间和代码质量之间取得平衡。

## 实现原理

### 编译路径

1. **快速编译路径**（执行次数 < 200）：
   - 使用基础优化（`speed_and_size`）
   - 跳过循环优化等耗时优化Pass
   - 目标：快速编译，减少启动延迟

2. **优化编译路径**（执行次数 ≥ 200）：
   - 使用完整优化（`speed`）
   - 应用循环优化等所有优化Pass
   - 目标：最大化运行时性能

### 阈值配置

默认阈值：
- `fast_path_threshold`: 200次
- `optimized_path_threshold`: 200次

可以通过 `TieredCompilationConfig` 自定义阈值。

## 使用方法

### 基本使用

```rust
use vm_engine_jit::{Jit, TieredCompiler, TieredCompilationConfig};

// 创建配置
let config = TieredCompilationConfig {
    fast_path_threshold: 200,
    optimized_path_threshold: 200,
    enabled: true,
};

// 创建分层编译器
let tiered_compiler = TieredCompiler::new(config);

// 在JIT编译时使用
let mut jit = Jit::new();
let execution_count = 150; // 假设执行了150次

// 选择编译路径
let tier = tiered_compiler.select_tier(execution_count);
match tier {
    CompilationTier::FastPath => {
        println!("使用快速编译路径");
    }
    CompilationTier::OptimizedPath => {
        println!("使用优化编译路径");
    }
}

// 编译代码块
let (code_ptr, used_tier) = tiered_compiler.compile(&block, execution_count, &mut jit);
```

### 集成到JIT编译器

当前实现已经集成到 `Jit::compile` 方法中：

```rust
// 在 compile 方法中
let execution_count = self.hot_counts.get(&block.start_pc)
    .map(|s| s.exec_count)
    .unwrap_or(0);

// 快速编译路径（执行次数 < 200）：使用基础优化
let use_fast_path = execution_count < 200;

// 应用循环优化（仅在优化路径）
let mut optimized_block = block.clone();
if !use_fast_path {
    self.loop_optimizer.optimize(&mut optimized_block);
}
```

### 获取统计信息

```rust
let stats = tiered_compiler.stats();
println!("快速路径编译次数: {}", stats.fast_path_compiles);
println!("优化路径编译次数: {}", stats.optimized_path_compiles);
println!("快速路径平均编译时间: {} ns", stats.fast_path_avg_time_ns);
println!("优化路径平均编译时间: {} ns", stats.optimized_path_avg_time_ns);
```

## 技术细节

### Cranelift优化级别

Cranelift的优化级别是在ISA创建时设置的，不能在运行时动态改变。因此：

1. **快速路径**：虽然ISA使用 `speed_and_size`，但实际控制通过跳过优化Pass实现
2. **优化路径**：ISA使用 `speed`，应用所有优化Pass

### 优化Pass控制

当前实现通过以下方式控制优化：

1. **循环优化**：仅在优化路径应用（`if !use_fast_path`）
2. **其他优化**：通过Cranelift的优化级别控制

### 性能考虑

- **快速路径**：编译时间短，但代码质量较低
- **优化路径**：编译时间长，但代码质量高

选择策略：
- 执行次数少的代码块：使用快速路径，减少编译开销
- 执行次数多的代码块：使用优化路径，最大化运行时性能

## 配置选项

### TieredCompilationConfig

```rust
pub struct TieredCompilationConfig {
    /// 快速路径阈值（执行次数）
    pub fast_path_threshold: u64,
    /// 优化路径阈值（执行次数）
    pub optimized_path_threshold: u64,
    /// 是否启用分层编译
    pub enabled: bool,
}
```

### 自定义配置

```rust
let config = TieredCompilationConfig {
    fast_path_threshold: 100,  // 降低阈值，更多代码使用快速路径
    optimized_path_threshold: 500,  // 提高阈值，只有热点代码使用优化路径
    enabled: true,
};
```

## 统计信息

### TieredCompilationStats

```rust
pub struct TieredCompilationStats {
    pub fast_path_compiles: u64,
    pub optimized_path_compiles: u64,
    pub fast_path_total_time_ns: u64,
    pub optimized_path_total_time_ns: u64,
    pub fast_path_avg_time_ns: u64,
    pub optimized_path_avg_time_ns: u64,
}
```

### 使用统计信息

```rust
let stats = tiered_compiler.stats();

// 计算编译时间节省
let total_fast_time = stats.fast_path_total_time_ns;
let total_optimized_time = stats.optimized_path_total_time_ns;
let time_saved = total_optimized_time.saturating_sub(total_fast_time);

println!("快速路径节省的编译时间: {} ns", time_saved);

// 计算平均编译时间
println!("快速路径平均编译时间: {} ns", stats.fast_path_avg_time_ns);
println!("优化路径平均编译时间: {} ns", stats.optimized_path_avg_time_ns);
```

## 性能影响

### 预期效果

1. **启动延迟**：快速路径减少编译时间，降低启动延迟
2. **运行时性能**：优化路径提高代码质量，提升运行时性能
3. **总体性能**：在编译时间和运行时性能之间取得平衡

### 监控指标

- 快速路径编译次数 vs 优化路径编译次数
- 快速路径平均编译时间 vs 优化路径平均编译时间
- 代码块执行次数分布

## 未来改进

1. **自适应阈值**：根据编译时间和运行时性能动态调整阈值
2. **多级分层**：支持更多级别的编译策略（如3级或4级）
3. **Profile引导**：使用PGO数据指导分层编译决策
4. **ML引导**：使用机器学习模型预测最佳编译策略

## 相关模块

- `enhanced_compiler.rs`: 增强的编译器，包含寄存器分配和指令调度
- `loop_opt.rs`: 循环优化器
- `optimization_passes.rs`: 优化Pass管理器
- `ml_guided_jit.rs`: ML引导的JIT编译决策


