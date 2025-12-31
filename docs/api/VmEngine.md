# vm-engine API 参考

## 概述

`vm-engine` 提供虚拟机的执行引擎实现，包括解释器、JIT编译器和执行器。该模块是VM执行子系统的核心。

## 主要功能

- **解释执行**: Interpreter 提供基本的指令解释
- **JIT编译**: JITCompiler 实现即时编译优化
- **异步执行**: AsyncExecutionContext 支持异步执行模式
- **执行控制**: 灵活的执行流程控制

## 主要类型

### JITCompiler

JIT编译器，负责将Guest指令编译为Host机器码。

#### 配置

##### `JITConfig`

JIT编译器配置。

**字段**:
- `optimization_level: u8` - 优化级别（0-3）
- `code_cache_size: usize` - 代码缓存大小
- `enable_inlining: bool` - 是否启用内联
- `enable_vectorization: bool` - 是否启用向量化

#### 方法

##### `new(config: JITConfig) -> Self`

创建新的JIT编译器。

**参数**:
- `config`: JIT配置

**返回**:
- JIT编译器实例

**示例**:
```rust
use vm_engine::{JITCompiler, JITConfig};

let config = JITConfig {
    optimization_level: 2,
    code_cache_size: 64 * 1024 * 1024, // 64MB
    enable_inlining: true,
    enable_vectorization: false,
};

let jit = JITCompiler::new(config);
```

##### `compile_block(&mut self, block: &IRBlock) -> Result<CompiledBlock, VmError>`

编译基本块。

**参数**:
- `block`: IR基本块

**返回**:
- 编译后的基本块

##### `execute_compiled(&mut self, compiled: &CompiledBlock, mmu: &mut dyn MMU) -> ExecResult`

执行编译后的代码。

**参数**:
- `compiled`: 编译后的基本块
- `mmu`: MMU引用

**返回**:
- 执行结果

### Interpreter

解释器，逐条解释执行Guest指令。

#### 方法

##### `new() -> Self`

创建新的解释器。

**返回**:
- 解释器实例

**示例**:
```rust
use vm_engine::Interpreter;

let interpreter = Interpreter::new();
```

##### `execute_instruction(&mut self, insn: &Instruction) -> Result<(), VmError>`

执行单条指令。

**参数**:
- `insn`: 要执行的指令

**返回**:
- 成功返回Ok(())，失败返回错误

##### `execute_block(&mut self, block: &IRBlock, mmu: &mut dyn MMU) -> ExecResult`

执行基本块。

**参数**:
- `block`: 要执行的基本块
- `mmu`: MMU引用

**返回**:
- 执行结果

### ExecutorType

执行器类型枚举。

#### 变体

##### `Interpreter`

解释器模式。

##### `JIT`

JIT编译模式。

##### `Hybrid`

混合模式（解释器+JIT）。

### AsyncExecutionContext

异步执行上下文。

#### 方法

##### `new(executor_type: ExecutorType) -> Self`

创建异步执行上下文。

**参数**:
- `executor_type`: 执行器类型

**返回**:
- 异步执行上下文实例

##### `execute_async(&mut self, block: IRBlock, mmu: &mut dyn MMU) -> impl Future<Output = ExecResult>`

异步执行基本块。

**参数**:
- `block`: 要执行的基本块
- `mmu`: MMU引用

**返回**:
- 异步执行结果的Future

**示例**:
```rust
use vm_engine::{AsyncExecutionContext, ExecutorType};

let mut ctx = AsyncExecutionContext::new(ExecutorType::Hybrid);

async fn run_vm(ctx: &mut AsyncExecutionContext, mmu: &mut dyn MMU) {
    let block = parse_block();
    let result = ctx.execute_async(block, mmu).await;
    match result.status {
        vm_core::ExecStatus::Ok => println!("Execution completed"),
        _ => println!("Execution status: {:?}", result.status),
    }
}
```

## 使用示例

### 基本解释器用法

```rust
use vm_engine::Interpreter;
use vm_core::{Instruction, ExecResult};
use vm_ir::IRBlock;

let mut interpreter = Interpreter::new();

// 执行单条指令
let insn = Instruction {
    opcode: 0x13,  // addi
    operands: vec![1, 0, 0x10],  // x1 = x0 + 16
    length: 4,
};

interpreter.execute_instruction(&insn)?;

// 执行基本块
let block = IRBlock {
    instructions: vec![insn],
    ..Default::default()
};

let mut mmu = setup_mmu();
let result = interpreter.execute_block(&block, &mut mmu)?;
```

### JIT编译器用法

```rust
use vm_engine::{JITCompiler, JITConfig};
use vm_ir::IRBlock;

let config = JITConfig {
    optimization_level: 2,
    code_cache_size: 64 * 1024 * 1024,
    enable_inlining: true,
    enable_vectorization: false,
};

let mut jit = JITCompiler::new(config);

// 编译基本块
let block = IRBlock {
    instructions: vec![/* ... */],
    ..Default::default()
};

let compiled = jit.compile_block(&block)?;

// 执行编译后的代码
let mut mmu = setup_mmu();
let result = jit.execute_compiled(&compiled, &mut mmu)?;
```

### 异步执行用法

```rust
use vm_engine::{AsyncExecutionContext, ExecutorType};
use vm_ir::IRBlock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ctx = AsyncExecutionContext::new(ExecutorType::Hybrid);
    let mut mmu = setup_mmu();

    let block = parse_block()?;

    // 异步执行
    let result = ctx.execute_async(block, &mut mmu).await;

    match result.status {
        vm_core::ExecStatus::Ok => println!("OK"),
        _ => println!("Status: {:?}", result.status),
    }

    Ok(())
}
```

### 混合模式执行

```rust
use vm_engine::{AsyncExecutionContext, ExecutorType};

let mut ctx = AsyncExecutionContext::new(ExecutorType::Hybrid);

// 混合模式会自动选择：
// - 热点代码使用JIT
// - 冷代码使用解释器
// - 根据执行统计动态调整

let result = ctx.execute_async(block, &mut mmu).await;
```

## 性能考虑

### 执行模式选择

**解释器**:
- 启动快速
- 内存占用小
- 性能较低（1-5%原生）

**JIT**:
- 启动较慢（需要编译）
- 内存占用较大（代码缓存）
- 性能较高（50-80%原生）

**混合**:
- 自动平衡性能和启动时间
- 热点检测和自适应优化
- 推荐用于大多数场景

### 优化级别

JIT优化级别：

**Level 0**:
- 无优化
- 快速编译
- 适合调试

**Level 1**:
- 基本优化
- 寄存器分配
- 适合一般用途

**Level 2**:
- 中等优化
- 内联小函数
- 基本循环优化
- 推荐级别

**Level 3**:
- 激进优化
- 向量化
- 高级循环优化
- 可能增加编译时间

### 代码缓存

合理设置代码缓存大小：
- 过小：频繁驱逐，增加编译开销
- 过大：内存浪费
- 推荐：64MB-256MB

```rust
let config = JITConfig {
    code_cache_size: 128 * 1024 * 1024, // 128MB
    ..Default::default()
};
```

## 错误处理

所有执行函数都返回`Result<T, VmError>`：

```rust
use vm_core::VmError;

match interpreter.execute_instruction(&insn) {
    Ok(()) => println!("Instruction executed"),
    Err(VmError::Execution(vm_core::ExecutionError::Fault(fault))) => {
        eprintln!("Execution fault: {:?}", fault);
    }
    Err(e) => eprintln!("Error: {:?}", e),
}
```

常见错误：
- `ExecutionError::Fault(Fault::InvalidOpcode)` - 无效指令
- `ExecutionError::Fault(Fault::PageFault)` - 页面故障
- `ExecutionError::Fault(Fault::AlignmentFault)` - 对齐错误

## 注意事项

### 线程安全

- `Interpreter` 本身不是线程安全的
- 每个线程应使用独立的实例
- 或者使用外部同步

### JIT编译

- JIT编译产物是特定于架构的
- 不能在不同Host架构间共享编译结果
- 代码缓存不需要手动管理

### 异步执行

- 异步执行需要在Tokio运行时中
- 使用`.await`等待执行完成
- 可以并发执行多个VM实例

## 相关API

- [VmCore API](./VmCore.md) - ExecutionEngine trait定义
- [VmInterface API](./VmInterface.md) - 执行引擎接口规范
- [InstructionSet API](./InstructionSet.md) - 指令集支持
