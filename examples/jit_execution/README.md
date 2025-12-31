# JIT编译执行示例

这个示例展示如何使用JIT(Just-In-Time)编译器来提升VM性能。

## 功能特性

1. **JIT编译** - 将RISC-V指令编译为本机代码
2. **性能对比** - 比较JIT和解释器性能
3. **配置选项** - 调整JIT优化级别和编译阈值
4. **基准测试** - 多种场景的性能测试

## 运行示例

```bash
cargo run --example jit_execution
```

## 预期输出

```
=== JIT编译执行示例 ===

--- 算术运算基准测试 ---

解释器: 45.23ms (10000 次迭代)
JIT:      3.12ms (10000 次迭代)
加速比:   14.50x

--- 循环基准测试 ---

解释器: 234.56ms (1000 次迭代)
JIT:      12.34ms (1000 次迭代)
加速比:   19.01x

--- 内存访问基准测试 ---

解释器: 345.67ms (1000 次迭代)
JIT:      28.90ms (1000 次迭代)
加速比:   11.96x

=== 所有测试完成 ===
JIT编译的优势:
  - 显著提升执行速度(10-100倍)
  - 降低CPU开销
  - 更好的内存局部性
  - 支持激进的优化
```

## JIT工作原理

### 1. 解释执行

```
RISC-V指令 → 解码 → 执行 → 下一条指令
              ↓          ↓
          每次重复    每次重复
```

**优点**: 实现简单,启动快速
**缺点**: 性能较差,重复开销大

### 2. JIT编译

```
RISC-V指令 → 热点检测 → 编译为本机代码 → 执行
                                     ↓
                              直接执行,无需解码
```

**优点**: 性能接近原生,可深度优化
**缺点**: 编译开销,内存占用

### 3. 热点检测

```rust
// vm-engine/src/jit/HotSpotDetector.rs
pub struct HotSpotDetector {
    execution_counts: HashMap<u64, usize>,  // 地址 -> 执行次数
    threshold: usize,                        // 编译阈值
}

impl HotSpotDetector {
    pub fn record_execution(&mut self, addr: u64) {
        let count = self.execution_counts.entry(addr).or_insert(0);
        *count += 1;

        if *count == self.threshold {
            // 触发JIT编译
            self.compile_block(addr);
        }
    }
}
```

## JIT配置选项

### EngineConfig

```rust
pub struct EngineConfig {
    /// 是否启用JIT
    pub enable_jit: bool,

    /// 触发JIT编译的执行次数阈值
    pub jit_threshold: usize,

    /// 优化级别 (0-3)
    pub optimization_level: u8,

    /// 是否内联函数
    pub inline_functions: bool,

    /// JIT代码缓存大小
    pub cache_size: usize,
}
```

### 优化级别

| 级别 | 说明             | 编译时间 | 运行性能 |
|------|------------------|----------|----------|
| 0    | 无优化           | 快       | 慢       |
| 1    | 基础优化         | 中       | 中       |
| 2    | 标准优化(推荐)   | 慢       | 快       |
| 3    | 激进优化         | 很慢     | 很快     |

### 编译阈值

```rust
// 保守策略 - 更快启动,较低性能
EngineConfig {
    jit_threshold: 1000,
    optimization_level: 1,
}

// 平衡策略 - 推荐
EngineConfig {
    jit_threshold: 100,
    optimization_level: 2,
}

// 激进策略 - 更慢启动,最高性能
EngineConfig {
    jit_threshold: 10,
    optimization_level: 3,
}
```

## 性能分析

### 不同场景的性能提升

#### 1. 算术运算
- **解释器**: 每条指令需要解码和分发
- **JIT**: 编译为直接的本机算术指令
- **加速比**: 10-20x

#### 2. 循环代码
- **解释器**: 重复解码相同的指令
- **JIT**: 循环体只编译一次,重复执行
- **加速比**: 20-50x

#### 3. 内存访问
- **解释器**: 每次访问都检查边界和权限
- **JIT**: 编译时优化,移除冗余检查
- **加速比**: 5-15x

### 性能调优建议

#### 1. 选择合适的阈值

```rust
// 短生命周期程序
jit_threshold: 10    // 快速编译

// 长生命周期程序
jit_threshold: 1000  // 只编译真正的热点
```

#### 2. 选择优化级别

```rust
// 开发/调试
optimization_level: 0  // 快速编译

// 生产环境
optimization_level: 2  // 平衡性能

// 性能关键应用
optimization_level: 3  // 最大性能
```

#### 3. 利用编译缓存

```rust
EngineConfig {
    cache_size: 64 * 1024 * 1024,  // 64MB缓存
}
```

## JIT编译流程

### 1. IR生成

```rust
// RISC-V指令
let inst = 0x003101b3;  // add x3, x1, x2

// 解码为IR
let ir = IrInstruction {
    opcode: IrOpcode::Add,
    operands: vec![
        IrOperand::Register(3),  // x3
        IrOperand::Register(1),  // x1
        IrOperand::Register(2),  // x2
    ],
};
```

### 2. 优化

```rust
// 优化前
add x1, x2, x3
add x4, x1, x5
mv  x6, x4

// 优化后(常量传播,死代码消除)
add x4, x2, x3
add x4, x4, x5
mv  x6, x4
```

### 3. 代码生成

```rust
// 使用Cranelift生成本机代码
let mut codegen = CraneliftCodeGenerator::new();
codegen.compile_ir(&ir)?;
let native_code = codegen.finalize()?;
```

### 4. 代码缓存

```rust
pub struct CodeCache {
    blocks: HashMap<u64, CompiledBlock>,
    lru: LruCache<u64>,
}

impl CodeCache {
    pub fn get_or_compile(&mut self, addr: u64) -> Option<&CompiledBlock> {
        if let Some(block) = self.blocks.get(&addr) {
            return Some(block);
        }

        let block = self.compile(addr)?;
        self.blocks.insert(addr, block);
        self.blocks.get(&addr)
    }
}
```

## 高级特性

### 1. 分层编译 (Tiered Compilation)

```rust
// 第一层: 快速生成无优化代码
if execution_count == threshold {
    compile_at_level(addr, 0);
}

// 第二层: 进一步优化
if execution_count == threshold * 10 {
    compile_at_level(addr, 2);
}
```

### 2. 内联优化

```rust
// 内联前
jal x1, function_name
add x2, x1, x3

// 内联后
# function body directly here
add x2, x1, x3
```

### 3. 寄存器分配

```rust
// 寄存器分配算法
pub struct RegisterAllocator {
    // 线性扫描寄存器分配
    active: Vec<LiveInterval>,
    registers: Vec<Register>,
}
```

## 扩展阅读

### JIT编译技术

1. **方法JIT** - 编译整个方法/函数
2. **跟踪JIT** - 编译热执行路径
3. **分层JIT** - 结合两者优势

### 相关论文

- [HotSpot VM论文](https://www.oracle.com/java/technologies/hotspot.html)
- [LuaJIT架构](https://luajit.org/architecture.html)
- [Cranelift JIT](https://cranelift.dev/)

### 其他VM的JIT实现

- **V8 (JavaScript)**: TurboFan优化编译器
- **PyPy (Python)**: 追踪JIT
- **GraalVM**: 多语言JIT编译器

## 故障排除

### JIT未生效

检查:
1. `enable_jit`是否为true
2. 执行次数是否达到阈值
3. 日志中是否有编译信息

### 性能未提升

可能原因:
- 程序太短,编译开销超过收益
- 阈值太高,未触发编译
- 优化级别设置不当

### 内存占用过高

解决:
- 减小JIT缓存大小
- 增加编译阈值
- 降低优化级别

## 相关文档

- [VM引擎架构](../../docs/tutorials/ADVANCED_USAGE.md)
- [性能优化指南](../../docs/tutorials/PERFORMANCE.md)
- [Fibonacci示例](../fibonacci/)
- [自定义设备示例](../custom_device/)
