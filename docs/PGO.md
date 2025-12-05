# Profile-Guided Optimization (PGO) 支持

## 概述

PGO（Profile-Guided Optimization）模块提供了运行时profile数据收集、序列化和基于profile的优化决策功能。通过收集代码执行频率、分支预测、内存访问模式等数据，可以指导JIT和AOT编译器进行更精准的优化。

## 功能特性

### 1. Profile数据收集

`ProfileCollector`收集以下类型的profile数据：

- **代码块执行频率**：记录每个代码块的执行次数和执行时间
- **分支预测信息**：记录分支指令的跳转概率和目标地址
- **内存访问模式**：检测顺序/随机/混合访问模式
- **函数调用频率**：记录函数调用次数和调用关系
- **循环迭代次数**：记录循环的平均、最大、最小迭代次数

### 2. Profile数据序列化

- **JSON格式**：使用JSON格式序列化profile数据
- **文件保存/加载**：支持将profile数据保存到文件和从文件加载
- **数据合并**：支持合并多个profile数据文件

### 3. Profile分析

`ProfileAnalyzer`分析profile数据并生成优化建议：

- **热点代码识别**：识别执行频率高的代码块
- **分支优化建议**：针对高预测性分支提出优化建议
- **循环优化建议**：针对适合展开的循环提出建议
- **函数内联建议**：识别适合内联的函数
- **内存预取建议**：针对顺序访问模式提出预取建议

## 使用方法

### 启用PGO收集

```rust
use vm_engine_jit::Jit;
use std::time::Duration;

let mut jit = Jit::new();

// 启用PGO收集（每5秒收集一次）
jit.enable_pgo(Duration::from_secs(5));

// 或者手动设置Profile收集器
use vm_engine_jit::pgo::ProfileCollector;
use std::sync::Arc;

let collector = Arc::new(ProfileCollector::new(Duration::from_secs(5)));
jit.set_profile_collector(Arc::clone(&collector));
```

### 执行代码并收集Profile数据

```rust
use vm_engine_jit::Jit;
use vm_core::ExecutionEngine;
use vm_mem::SoftMmu;
use vm_ir::{IRBlock, IROp, Terminator};

let mut jit = Jit::new();
jit.enable_pgo(Duration::from_secs(1));

let mut mmu = SoftMmu::new(1024 * 1024 * 1024, false);

// 执行代码块（会自动收集profile数据）
let block = IRBlock {
    start_pc: 0x1000,
    ops: vec![IROp::MovImm { dst: 1, imm: 42 }],
    term: Terminator::Return,
};

for _ in 0..1000 {
    jit.run(&mut mmu, &block)?;
}
```

### 保存Profile数据

```rust
use vm_engine_jit::Jit;

let mut jit = Jit::new();
jit.enable_pgo(Duration::from_secs(1));

// ... 执行代码 ...

// 保存profile数据到文件
jit.save_profile_data("profile.json")?;
```

### 加载并使用Profile数据

```rust
use vm_engine_jit::pgo::{ProfileCollector, ProfileAnalyzer};

// 从文件加载profile数据
let profile = ProfileCollector::deserialize_from_file("profile.json")?;

// 分析profile数据
let analyzer = ProfileAnalyzer;
let suggestions = analyzer.analyze(&profile);

// 打印优化建议
for suggestion in suggestions {
    println!("PC: 0x{:x}, Type: {:?}, Priority: {}, Expected improvement: {:.1}%",
        suggestion.pc,
        suggestion.optimization_type,
        suggestion.priority,
        suggestion.expected_improvement
    );
    println!("  Reason: {}", suggestion.reason);
}

// 获取热点代码块
let hot_blocks = analyzer.get_hot_blocks(&profile, 1000);
println!("Hot blocks: {:?}", hot_blocks);
```

### 合并多个Profile数据

```rust
use vm_engine_jit::pgo::ProfileCollector;
use std::sync::Arc;

let collector = Arc::new(ProfileCollector::new(Duration::from_secs(1)));

// 加载多个profile文件
let profile1 = ProfileCollector::deserialize_from_file("profile1.json")?;
let profile2 = ProfileCollector::deserialize_from_file("profile2.json")?;

// 合并到collector
collector.merge_profile_data(&profile1);
collector.merge_profile_data(&profile2);

// 保存合并后的数据
collector.serialize_to_file("merged_profile.json")?;
```

### 基于Profile的优化决策

```rust
use vm_engine_jit::pgo::{ProfileAnalyzer, OptimizationType};

let analyzer = ProfileAnalyzer;
let suggestions = analyzer.analyze(&profile);

// 根据优化建议调整编译策略
for suggestion in suggestions {
    match suggestion.optimization_type {
        OptimizationType::InlineFunction => {
            // 应用函数内联优化
            println!("Inline function at 0x{:x}", suggestion.pc);
        }
        OptimizationType::UnrollLoop => {
            // 应用循环展开优化
            println!("Unroll loop at 0x{:x}", suggestion.pc);
        }
        OptimizationType::OptimizeBranch => {
            // 应用分支优化
            println!("Optimize branch at 0x{:x}", suggestion.pc);
        }
        OptimizationType::PrefetchMemory => {
            // 应用内存预取
            println!("Prefetch memory at 0x{:x}", suggestion.pc);
        }
        _ => {}
    }
}
```

## Profile数据结构

### BlockProfile（代码块Profile）

```rust
pub struct BlockProfile {
    pub pc: GuestAddr,                    // 代码块地址
    pub execution_count: u64,             // 执行次数
    pub avg_execution_time_ns: u64,       // 平均执行时间
    pub total_execution_time_ns: u64,     // 总执行时间
    pub last_execution_time: Option<u64>, // 最后执行时间
    pub callers: HashSet<GuestAddr>,      // 调用者集合
    pub callees: HashSet<GuestAddr>,      // 被调用者集合
}
```

### BranchProfile（分支Profile）

```rust
pub struct BranchProfile {
    pub pc: GuestAddr,                    // 分支指令地址
    pub total_branches: u64,              // 总分支次数
    pub taken_count: u64,                 // 跳转次数
    pub not_taken_count: u64,             // 不跳转次数
    pub target_counts: HashMap<GuestAddr, u64>, // 目标地址 -> 跳转次数
    pub taken_probability: f64,           // 跳转概率
}
```

### MemoryAccessProfile（内存访问Profile）

```rust
pub struct MemoryAccessProfile {
    pub pc: GuestAddr,                    // 代码块地址
    pub access_count: u64,                // 访问次数
    pub min_address: u64,                 // 最小地址
    pub max_address: u64,                 // 最大地址
    pub access_pattern: AccessPattern,    // 访问模式
    pub cache_hit_rate: Option<f64>,      // 缓存命中率
}
```

## 优化建议类型

- `InlineFunction`: 函数内联优化
- `UnrollLoop`: 循环展开优化
- `OptimizeBranch`: 分支预测优化
- `PrefetchMemory`: 内存预取优化
- `OptimizeRegisterAllocation`: 寄存器分配优化
- `OptimizeInstructionScheduling`: 指令调度优化

## 集成到AOT编译

```rust
use aot_builder::{AotBuilder, CompilationOptions};
use vm_engine_jit::pgo::{ProfileCollector, ProfileAnalyzer};

// 加载profile数据
let profile = ProfileCollector::deserialize_from_file("profile.json")?;

// 分析profile数据
let analyzer = ProfileAnalyzer;
let suggestions = analyzer.analyze(&profile);

// 创建AOT构建器
let mut builder = AotBuilder::new();

// 根据profile数据优化编译
for suggestion in suggestions {
    if suggestion.priority > 70 {
        // 高优先级优化
        match suggestion.optimization_type {
            OptimizationType::InlineFunction => {
                // 标记函数为内联候选
            }
            OptimizationType::UnrollLoop => {
                // 标记循环为展开候选
            }
            _ => {}
        }
    }
}

// 构建AOT镜像
let image = builder.build()?;
```

## 性能影响

### 开销

- **内存开销**：Profile数据占用内存，但可以通过定期保存和重置来管理
- **CPU开销**：Profile收集的开销很小（O(1)操作），对性能影响可忽略不计
- **存储开销**：Profile文件大小取决于收集的数据量，通常几MB到几十MB

### 收益

- **编译优化**：基于profile的优化可以提升5-20%的性能
- **代码布局**：优化代码布局可以提升缓存命中率
- **分支预测**：优化分支预测可以提升10-30%的性能

## 最佳实践

1. **收集足够的数据**：运行代表性工作负载足够长时间以收集有意义的profile数据
2. **定期保存**：定期保存profile数据，避免数据丢失
3. **合并多个profile**：合并多个工作负载的profile数据以获得更全面的视图
4. **验证优化效果**：应用优化后验证性能提升
5. **增量更新**：对于长期运行的应用，可以增量更新profile数据

## 示例：完整工作流

```rust
use vm_engine_jit::Jit;
use vm_engine_jit::pgo::{ProfileCollector, ProfileAnalyzer};
use vm_core::ExecutionEngine;
use vm_mem::SoftMmu;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建JIT并启用PGO
    let mut jit = Jit::new();
    jit.enable_pgo(Duration::from_secs(1));

    // 2. 运行代表性工作负载
    let mut mmu = SoftMmu::new(1024 * 1024 * 1024, false);
    // ... 执行代码 ...

    // 3. 保存profile数据
    jit.save_profile_data("profile.json")?;

    // 4. 分析profile数据
    let profile = ProfileCollector::deserialize_from_file("profile.json")?;
    let analyzer = ProfileAnalyzer;
    let suggestions = analyzer.analyze(&profile);

    // 5. 应用优化建议
    for suggestion in suggestions {
        if suggestion.priority > 70 {
            println!("Apply optimization: {:?} at 0x{:x}", 
                suggestion.optimization_type, suggestion.pc);
        }
    }

    // 6. 使用优化后的配置重新编译
    // ...

    Ok(())
}
```

## 未来改进

1. **实时优化**：根据实时profile数据动态调整优化策略
2. **机器学习集成**：使用ML模型预测最佳优化策略
3. **多维度分析**：分析更多维度（如数据依赖、控制流图等）
4. **可视化工具**：提供profile数据的可视化工具
5. **自动优化应用**：自动应用优化建议，无需手动干预


