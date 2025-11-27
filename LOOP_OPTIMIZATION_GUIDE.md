# JIT 循环优化使用指南

## 快速开始

### 1. 基本使用

```rust
use vm_engine_jit::LoopOptimizer;
use vm_ir::IRBlock;

// 创建默认配置的优化器
let optimizer = LoopOptimizer::default();

// 优化 IR 块
let mut block = /* 你的 IR 块 */;
optimizer.optimize(&mut block);
```

### 2. 自定义配置

```rust
use vm_engine_jit::{LoopOptimizer, LoopOptConfig};

// 创建自定义配置
let config = LoopOptConfig {
    max_unroll_factor: 8,           // 最大展开因子
    enable_licm: true,              // 启用不变量外提
    enable_strength_reduction: true, // 启用强度削弱
    enable_unrolling: true,         // 启用循环展开
};

let optimizer = LoopOptimizer::new(config);
let mut block = /* 你的 IR 块 */;
optimizer.optimize(&mut block);
```

### 3. 在 JIT 中自动应用

在 `vm-engine-jit` 的 `compile()` 方法中，循环优化已经自动集成：

```rust
impl ExecutionEngine<IRBlock> for Jit {
    fn compile(&mut self, block: &IRBlock) {
        // 循环优化自动应用
        let mut optimized_block = block.clone();
        self.loop_optimizer.optimize(&mut optimized_block);
        
        // 使用优化后的 IR 编译
        // ...
    }
}
```

## 优化详解

### 循环检测 (Loop Detection)

优化器自动检测以下循环模式：

```
1. 无条件回边:
   Jmp { target: 之前的地址 }

2. 条件回边 (含跳出):
   CondJmp { 
       cond, 
       target_true: 之前的地址,  // 循环继续
       target_false: 之前的地址   // 循环退出
   }
```

### 不变量外提 (LICM)

优化器识别循环中不会改变的操作：

```rust
// 这个操作不依赖循环变量，可被外提
let invariant_op = IROp::Add {
    dst: 5,
    src1: 6,  // 常数 (不被循环修改)
    src2: 7,  // 常数 (不被循环修改)
};

// 这个操作依赖循环变量，不能外提
let loop_op = IROp::Add {
    dst: 0,
    src1: 0,  // 循环计数器 (被修改)
    src2: 1,
};
```

### 归纳变量识别

优化器检测形如 `reg = reg + const` 的模式：

```rust
// 检测: i = i + 1
IROp::Add { dst: 1, src1: 1, src2: 2 }
-> InductionVar { 
    reg: 1, 
    step: 1, 
    update_idx: 某个指令索引
}
```

### 循环展开 (Unrolling)

根据循环体大小自动确定展开因子：

```
展开因子计算:
    unroll_factor = min(config.max_unroll_factor, 100 / loop_size)

示例:
    - 循环体 4 条指令, max=8 => 展开 8 倍
    - 循环体 25 条指令, max=8 => 展开 4 倍
    - 循环体 50 条指令, max=8 => 展开 2 倍
```

## 性能调优建议

### 何时优化工作最好

✅ **最有效的场景**:
- 循环执行次数多 (>100 迭代)
- 循环体较小 (<10 条指令)
- 循环中有明显的不变量或归纳变量

❌ **效果有限的场景**:
- 一次性执行的循环 (<5 迭代)
- 循环体很大 (>100 条指令)
- 循环中充满指针别名问题 (难以分析)

### 调整展开因子

```rust
// 保守策略 - 减少代码膨胀
LoopOptConfig {
    max_unroll_factor: 2,  // 最多展开 2 倍
    ..Default::default()
}

// 激进策略 - 最大化性能
LoopOptConfig {
    max_unroll_factor: 8,  // 最多展开 8 倍
    ..Default::default()
}

// 禁用某个优化
LoopOptConfig {
    enable_unrolling: false,  // 仅做 LICM
    ..Default::default()
}
```

## 诊断和调试

### 检查循环是否被识别

```rust
let optimizer = LoopOptimizer::default();
if let Some(loop_info) = optimizer.detect_loop(&block) {
    println!("循环头: 0x{:x}", loop_info.header_pc);
    println!("循环体指令数: {}", loop_info.body_indices.len());
    println!("不变量数: {}", loop_info.invariants.len());
    println!("归纳变量数: {}", loop_info.induction_vars.len());
}
```

### 优化前后对比

```rust
// 原始 IR 块大小
let orig_size = block.ops.len();

// 应用优化
optimizer.optimize(&mut block);

// 优化后大小
let opt_size = block.ops.len();

println!("代码膨胀: {} -> {} ({:.1}%)",
    orig_size, opt_size, 
    (opt_size as f64 / orig_size as f64) * 100.0);
```

## API 参考

### LoopOptimizer

```rust
pub struct LoopOptimizer {
    config: LoopOptConfig,
}

impl LoopOptimizer {
    // 创建新优化器
    pub fn new(config: LoopOptConfig) -> Self
    pub fn default() -> Self  // 默认配置
    
    // 核心优化方法
    pub fn optimize(&self, block: &mut IRBlock)
    
    // 内部诊断方法 (pub 用于测试)
    pub fn detect_loop(&self, block: &IRBlock) -> Option<LoopInfo>
    pub fn find_invariants(&self, block: &IRBlock, indices: &[usize]) -> HashSet<usize>
    pub fn find_induction_vars(&self, block: &IRBlock, indices: &[usize]) -> HashMap<RegId, InductionVar>
}
```

### LoopInfo

```rust
pub struct LoopInfo {
    pub header_pc: GuestAddr,           // 循环开始 PC
    pub body_indices: Vec<usize>,       // 循环体指令索引
    pub back_edge_target: GuestAddr,    // 回边目标
    pub invariants: HashSet<usize>,     // 不变量指令索引集
    pub induction_vars: HashMap<RegId, InductionVar>,  // 归纳变量
}
```

### LoopOptConfig

```rust
pub struct LoopOptConfig {
    pub max_unroll_factor: usize,       // 默认: 4
    pub enable_licm: bool,              // 默认: true
    pub enable_strength_reduction: bool,// 默认: true
    pub enable_unrolling: bool,         // 默认: true
}

impl Default for LoopOptConfig {
    fn default() -> Self {
        // 均衡的默认配置
        Self {
            max_unroll_factor: 4,
            enable_licm: true,
            enable_strength_reduction: true,
            enable_unrolling: true,
        }
    }
}
```

## 集成示例

### 完整的优化管道

```rust
use vm_engine_jit::{LoopOptimizer, LoopOptConfig};
use vm_ir::IRBlock;

fn optimize_ir_blocks(blocks: Vec<IRBlock>) -> Vec<IRBlock> {
    let config = LoopOptConfig {
        max_unroll_factor: 4,
        enable_licm: true,
        enable_strength_reduction: true,
        enable_unrolling: true,
    };
    
    let optimizer = LoopOptimizer::new(config);
    
    blocks.into_iter().map(|mut block| {
        optimizer.optimize(&mut block);
        block
    }).collect()
}
```

### 选择性优化

```rust
use vm_engine_jit::{LoopOptimizer, LoopOptConfig};

// 只对热点块优化
fn selective_optimize(blocks: Vec<(bool, IRBlock)>) -> Vec<IRBlock> {
    let optimizer = LoopOptimizer::default();
    
    blocks.into_iter().filter_map(|(is_hot, mut block)| {
        if is_hot {
            // 只优化热点循环
            optimizer.optimize(&mut block);
        }
        Some(block)
    }).collect()
}
```

## 已知限制

1. **别名分析**: 当前不执行深度的指针别名分析
2. **条件依赖**: 复杂的数据流依赖可能被保守处理
3. **嵌套循环**: 目前的实现针对单层循环优化
4. **动态循环**: 循环范围在编译时已知才能优化

## 未来改进

- [ ] 支持嵌套循环优化
- [ ] 更精确的别名分析
- [ ] 向量化支持
- [ ] 循环归纳变量消除 (IVE)
- [ ] 自适应展开因子基于热点数据

---

*JIT 循环优化文档 v1.0*  
*最后更新: 2025-11-29*
