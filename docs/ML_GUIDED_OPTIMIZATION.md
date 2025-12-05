# ML指导的JIT优化

## 概述

ML指导的JIT优化使用机器学习模型来预测最佳的编译策略和优化参数，基于代码特征和运行时profile数据做出智能决策。

## 主要功能

### 1. 特征提取

从IR块和PGO profile数据中提取特征：

- **块大小**: 代码块的大小（字节数）
- **指令数**: IR指令的数量
- **分支数**: 分支指令的数量
- **内存访问数**: 内存读写操作的数量
- **执行次数**: 代码块的历史执行次数
- **执行时间**: 平均执行时间
- **缓存命中率**: 代码缓存的命中率

### 2. ML模型

实现了线性回归模型用于编译决策预测：

- **模型训练**: 在线学习，根据实际性能反馈调整模型
- **决策预测**: 基于特征预测最佳编译策略
- **模型持久化**: 支持保存和加载训练好的模型

### 3. 在线学习

- **批量更新**: 收集训练样本，批量更新模型
- **性能反馈**: 根据实际性能调整模型权重
- **自适应学习**: 学习率可配置，支持不同学习策略

### 4. 性能验证

- **基准对比**: 对比优化前后的性能
- **改进统计**: 统计性能改进情况
- **性能报告**: 生成详细的性能分析报告

## 使用方法

### 启用ML指导的编译

```rust
use vm_engine_jit::Jit;

let mut jit = Jit::new();

// 启用ML指导
jit.enable_ml_guidance();

// 启用PGO（ML会使用PGO数据增强特征）
jit.enable_pgo(std::time::Duration::from_secs(1));
```

### 获取ML编译决策

```rust
use vm_engine_jit::{Jit, IRBlock};
use vm_core::ExecutionEngine;

let mut jit = Jit::new();
jit.enable_ml_guidance();

let block = IRBlock {
    start_pc: 0x1000,
    ops: vec![],
    term: Terminator::Return,
};

// 获取ML推荐的编译决策
if let Some(decision) = jit.get_ml_decision(&block) {
    match decision {
        CompilationDecision::Skip => {
            // 跳过编译，使用解释器
        }
        CompilationDecision::FastJit => {
            // 快速JIT编译（O0）
        }
        CompilationDecision::StandardJit => {
            // 标准JIT编译（O1）
        }
        CompilationDecision::OptimizedJit => {
            // 优化JIT编译（O2）
        }
        CompilationDecision::Aot => {
            // AOT编译
        }
    }
}
```

### 记录训练样本

```rust
use vm_engine_jit::{Jit, CompilationDecision};

let mut jit = Jit::new();
jit.enable_ml_guidance();

// 执行代码块
let block = IRBlock { /* ... */ };
let result = jit.run(&mut mmu, &block)?;

// 记录训练样本（用于在线学习）
let performance = 1.5; // 性能指标（例如：相对于基准的改进倍数）
jit.record_ml_sample(&block, CompilationDecision::OptimizedJit, performance);
```

### 获取性能报告

```rust
use vm_engine_jit::Jit;

let mut jit = Jit::new();
jit.enable_ml_guidance();

// ... 执行代码 ...

// 获取性能报告
if let Some(report) = jit.get_ml_performance_report() {
    println!("Total blocks: {}", report.total_blocks);
    println!("Improved blocks: {}", report.improved_blocks);
    println!("Average improvement: {:.2}%", report.avg_improvement);
    println!("Max improvement: {:.2}%", report.max_improvement);
}
```

### 直接使用ML模型

```rust
use vm_engine_jit::ml_model::{LinearRegressionModel, MLModel, FeatureExtractor};
use vm_engine_jit::{ExecutionFeatures, CompilationDecision};

// 创建模型
let mut model = LinearRegressionModel::new(0.01);

// 提取特征
let block = IRBlock { /* ... */ };
let features = FeatureExtractor::extract_from_ir_block(&block);

// 预测决策
let decision = model.predict(&features);

// 更新模型（在线学习）
let actual_performance = 1.3; // 实际性能
model.update(&features, decision, actual_performance);

// 保存模型
model.save("model.json")?;

// 加载模型
let loaded_model = LinearRegressionModel::load("model.json")?;
```

### 使用在线学习器

```rust
use vm_engine_jit::ml_model::{LinearRegressionModel, OnlineLearner};
use vm_engine_jit::{ExecutionFeatures, CompilationDecision};
use std::time::Duration;

// 创建模型和学习器
let model = LinearRegressionModel::new(0.01);
let mut learner = OnlineLearner::new(
    Box::new(model),
    10,  // 批量大小
    Duration::from_secs(5), // 更新间隔
);

// 预测
let features = ExecutionFeatures::new(100, 20, 2, 5);
let decision = learner.predict(&features);

// 添加训练样本（会自动触发批量更新）
learner.add_sample(features, decision, 1.2);
```

### 性能验证

```rust
use vm_engine_jit::ml_model::PerformanceValidator;

let mut validator = PerformanceValidator::new();

// 记录基准性能
validator.record_baseline(0x1000, 100.0); // 100ms

// 记录优化后性能
validator.record_optimized(0x1000, 80.0); // 80ms

// 获取性能报告
let report = validator.get_performance_report();
println!("Average improvement: {:.2}%", report.avg_improvement);
```

## 特征提取

### 从IR块提取

```rust
use vm_engine_jit::ml_model::FeatureExtractor;
use vm_engine_jit::IRBlock;

let block = IRBlock {
    start_pc: 0x1000,
    ops: vec![
        IROp::Load { dst: 1, base: 2, offset: 0 },
        IROp::Add { dst: 1, src1: 1, src2: 3 },
        IROp::Store { src: 1, base: 2, offset: 4 },
    ],
    term: Terminator::Return,
};

let features = FeatureExtractor::extract_from_ir_block(&block);
// features.memory_access_count = 2 (Load + Store)
// features.branch_count = 0 (无分支)
```

### 从PGO Profile提取

```rust
use vm_engine_jit::ml_model::FeatureExtractor;
use vm_engine_jit::pgo::ProfileData;

let profile = ProfileData::default();
let features = FeatureExtractor::extract_from_pgo_profile(0x1000, &profile);
```

## ML模型

### 线性回归模型

线性回归模型使用加权特征向量来预测编译决策：

```rust
score = w1 * block_size + w2 * instr_count + w3 * branch_count + 
        w4 * memory_access + w5 * execution_count + w6 * cache_hit_rate

decision = score_to_decision(score)
```

### 模型训练

模型使用梯度下降进行在线学习：

```rust
error = target_score - predicted_score
weight += learning_rate * error * feature
```

### 模型持久化

```rust
use vm_engine_jit::ml_model::LinearRegressionModel;

let model = LinearRegressionModel::new(0.01);

// 训练模型...
// ...

// 保存模型
model.save("trained_model.json")?;

// 加载模型
let loaded_model = LinearRegressionModel::load("trained_model.json")?;
```

## 编译决策

ML模型可以预测以下编译决策：

- **Skip**: 跳过编译，使用解释器执行
- **FastJit**: 快速JIT编译（O0优化级别）
- **StandardJit**: 标准JIT编译（O1优化级别）
- **OptimizedJit**: 优化JIT编译（O2优化级别）
- **Aot**: AOT离线编译

## 与PGO集成

ML指导的优化可以与PGO数据结合使用：

```rust
use vm_engine_jit::Jit;

let mut jit = Jit::new();

// 启用PGO和ML
jit.enable_pgo(std::time::Duration::from_secs(1));
jit.enable_ml_guidance();

// ML会自动使用PGO数据增强特征
let block = IRBlock { /* ... */ };
if let Some(decision) = jit.get_ml_decision(&block) {
    // 使用ML推荐的决策
}
```

## 性能测试

### 基准测试

```rust
use vm_engine_jit::ml_model::PerformanceValidator;

let mut validator = PerformanceValidator::new();

// 测试多个代码块
for block_id in 0x1000..0x2000 {
    // 记录基准性能
    let baseline = measure_performance(block_id);
    validator.record_baseline(block_id, baseline);

    // 应用ML优化
    apply_ml_optimization(block_id);

    // 记录优化后性能
    let optimized = measure_performance(block_id);
    validator.record_optimized(block_id, optimized);
}

// 获取性能报告
let report = validator.get_performance_report();
println!("Average improvement: {:.2}%", report.avg_improvement);
```

## 最佳实践

1. **启用PGO**: ML模型会从PGO数据中受益，建议同时启用
2. **收集足够数据**: 让模型有足够的训练样本
3. **定期保存模型**: 保存训练好的模型以便重用
4. **监控性能**: 定期检查性能改进情况
5. **调整学习率**: 根据实际情况调整学习率
6. **验证决策**: 验证ML推荐的决策是否有效

## 配置参数

### 学习率

学习率控制模型更新的速度：

- **高学习率** (0.1+): 快速适应，但可能不稳定
- **中等学习率** (0.01): 平衡速度和稳定性（推荐）
- **低学习率** (0.001): 稳定但适应慢

### 批量大小

批量大小控制何时更新模型：

- **小批量** (5-10): 频繁更新，适应快
- **中等批量** (10-50): 平衡更新频率和稳定性（推荐）
- **大批量** (50+): 更新少，但更稳定

### 更新间隔

更新间隔控制批量更新的时间间隔：

- **短间隔** (1-5秒): 快速适应变化
- **中等间隔** (5-30秒): 平衡响应速度和稳定性（推荐）
- **长间隔** (30秒+): 减少计算开销

## 示例：完整工作流

```rust
use vm_engine_jit::{Jit, CompilationDecision};
use vm_core::ExecutionEngine;
use vm_mem::SoftMmu;
use vm_ir::IRBlock;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建JIT并启用ML指导
    let mut jit = Jit::new();
    jit.enable_ml_guidance();
    jit.enable_pgo(std::time::Duration::from_secs(1));

    let mut mmu = SoftMmu::new(1024 * 1024 * 1024, false);

    // 2. 执行代码块
    let block = IRBlock {
        start_pc: 0x1000,
        ops: vec![],
        term: Terminator::Return,
    };

    // 3. 获取ML推荐的编译决策
    if let Some(decision) = jit.get_ml_decision(&block) {
        println!("ML recommends: {:?}", decision);
        
        // 4. 执行代码
        let start_time = std::time::Instant::now();
        let result = jit.run(&mut mmu, &block)?;
        let execution_time = start_time.elapsed().as_secs_f64();

        // 5. 记录训练样本
        let performance = 1.0 / execution_time; // 性能指标
        jit.record_ml_sample(&block, decision, performance);
    }

    // 6. 获取性能报告
    if let Some(report) = jit.get_ml_performance_report() {
        println!("Performance Report:");
        println!("  Total blocks: {}", report.total_blocks);
        println!("  Improved blocks: {}", report.improved_blocks);
        println!("  Average improvement: {:.2}%", report.avg_improvement);
    }

    Ok(())
}
```

## 未来改进

1. **更复杂的模型**: 支持神经网络、决策树等更复杂的模型
2. **特征工程**: 自动特征选择和工程
3. **超参数优化**: 自动调整学习率等超参数
4. **分布式训练**: 支持多VM实例的模型训练
5. **模型集成**: 使用多个模型的集成预测
6. **实时监控**: 实时监控模型性能和预测准确率


