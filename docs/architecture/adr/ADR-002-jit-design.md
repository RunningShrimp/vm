# ADR-002: JIT编译器设计

## 状态
已接受 (2024-12-31)

## 上下文
VM项目需要支持多种执行模式：
- 解释器模式：便于调试，性能低
- JIT模式：高性能，编译开销
- 混合模式：平衡启动和性能

## 决策
采用分层JIT设计，支持热点检测和渐进式优化。

```
冷代码(解释器) → 温代码(基础JIT) → 热代码(优化JIT)
```

## 架构设计

### 执行模式分层

```rust
pub enum ExecutionMode {
    Interpreter,     // 冷代码
    BaselineJIT,     // 温代码
    OptimizingJIT,   // 热代码
}
```

### 热点检测

```rust
pub struct HotspotDetector {
    execution_counts: HashMap<GuestAddr, u64>,
    cold_threshold: u64,    // 默认100
    hot_threshold: u64,     // 默认1000
}

impl HotspotDetector {
    pub fn record_execution(&mut self, addr: GuestAddr) {
        *self.execution_counts.entry(addr).or_insert(0) += 1;
    }

    pub fn get_optimization_level(&self, addr: GuestAddr) -> ExecutionMode {
        let count = *self.execution_counts.get(&addr).unwrap_or(&0);

        if count < self.cold_threshold {
            ExecutionMode::Interpreter
        } else if count < self.hot_threshold {
            ExecutionMode::BaselineJIT
        } else {
            ExecutionMode::OptimizingJIT
        }
    }
}
```

### JIT编译器

```rust
pub struct JITEngine {
    baseline_compiler: BaselineCompiler,
    optimizing_compiler: OptimizingCompiler,
    code_cache: HashMap<GuestAddr, CompiledCode>,
}
```

## 理由

### 优势

1. **自适应性能**:
   - 冷代码快速启动
   - 热代码充分优化
   - 无需手动配置

2. **内存效率**:
   - 只编译热点
   - LRU淘汰冷代码

3. **开发效率**:
   - 解释器便于调试
   - JIT提供生产性能

### 劣势及缓解

1. **编译开销**:
   - 缓解：代码缓存
   - 缓解：后台编译

2. **代码复杂度**:
   - 缓解：清晰的抽象
   - 缓解：模块化设计

## 替代方案

### 纯解释器
**优势**: 简单
**劣势**: 性能差（1-5%原生）

### 纯JIT
**优势**: 性能高（50-80%原生）
**劣势**: 启动慢，内存大

### AOT编译
**优势**: 无运行时开销
**劣势**: 静态编译，不适合动态代码

## 后果

### 短期
- ✅ 提供灵活的执行模式
- ✅ 满足调试和生产需求
- ⚠️ 增加代码复杂度

### 长期
- ✅ 可扩展到更多优化等级
- ✅ 支持OSR（栈上替换）
- ✅ 可集成profiling反馈

## 实现

### 阶段1 (已完成)
- ✅ 基础解释器
- ✅ 简单JIT编译器

### 阶段2 (进行中)
- 🔄 分层编译
- 🔄 热点检测

### 阶段3 (计划中)
- ⏳ 高级优化
- ⏳ OSR支持

---
**创建时间**: 2024-12-31
**作者**: VM开发团队
