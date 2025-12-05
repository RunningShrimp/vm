# 异步执行引擎接口文档

## 概述

`AsyncExecutionEngine` trait提供了异步版本的执行引擎接口，支持异步执行和异步内存访问。这是对同步`ExecutionEngine` trait的补充，适用于需要高并发和异步I/O的场景。

## 设计目标

1. **非阻塞执行**：使用异步内存访问，避免阻塞线程
2. **高并发支持**：支持并发执行多个基本块
3. **向后兼容**：与同步`ExecutionEngine`接口保持一致
4. **性能优化**：减少上下文切换，提高吞吐量

## 接口定义

### AsyncExecutionEngine Trait

```rust
#[async_trait]
pub trait AsyncExecutionEngine<B>: Send + Sync
where
    B: Send + Sync,
{
    /// 异步执行单个基本块
    async fn run_async(
        &mut self,
        mmu: &mut dyn AsyncMMU,
        block: &B,
    ) -> Result<ExecResult, VmError>;

    /// 异步获取寄存器值
    async fn get_reg_async(&self, idx: usize) -> u64;

    /// 异步设置寄存器值
    async fn set_reg_async(&mut self, idx: usize, val: u64);

    /// 异步获取PC
    async fn get_pc_async(&self) -> GuestAddr;

    /// 异步设置PC
    async fn set_pc_async(&mut self, pc: GuestAddr);

    /// 异步批量执行（默认实现）
    async fn run_many_async(
        &mut self,
        mmu: &mut dyn AsyncMMU,
        blocks: &[B],
    ) -> Result<Vec<ExecResult>, VmError>;
}
```

## 使用场景

### 1. 异步I/O操作
当执行引擎需要执行I/O操作（如设备访问）时，使用异步版本可以避免阻塞。

### 2. 协程池执行
在协程池中执行多个vCPU时，异步版本可以更好地利用协程资源。

### 3. 高并发场景
当需要同时执行大量基本块时，异步版本可以并行执行，提高吞吐量。

## 实现指南

### 基本实现

```rust
use vm_core::{AsyncExecutionEngine, AsyncMMU};
use vm_ir::IRBlock;

struct MyAsyncEngine {
    // 引擎状态
}

#[async_trait]
impl AsyncExecutionEngine<IRBlock> for MyAsyncEngine {
    async fn run_async(
        &mut self,
        mmu: &mut dyn AsyncMMU,
        block: &IRBlock,
    ) -> Result<ExecResult, VmError> {
        // 使用异步MMU进行内存访问
        // 执行基本块
        // 返回结果
    }
    
    // 实现其他方法...
}
```

### 适配器模式

如果已有同步`ExecutionEngine`实现，可以使用`ExecutionEngineAdapter`：

```rust
use vm_core::{ExecutionEngineAdapter, AsyncExecutionEngine};

let sync_engine = MySyncEngine::new();
let mut async_engine = ExecutionEngineAdapter::new(sync_engine);

// 使用异步接口
let result = async_engine.run_async(&mut mmu, &block).await?;
```

**注意**：适配器使用`spawn_blocking`在线程池中执行同步代码，可能增加延迟。如果可能，应该直接实现`AsyncExecutionEngine`。

## 性能考虑

### 优势
- **非阻塞**：避免阻塞异步运行时
- **并发**：支持并行执行多个基本块
- **资源利用**：更好地利用协程资源

### 注意事项
- **适配器开销**：使用适配器会增加线程池开销
- **内存访问**：需要确保MMU支持异步访问
- **错误处理**：异步错误处理需要特别注意

## 与同步版本的关系

| 特性 | ExecutionEngine | AsyncExecutionEngine |
|------|----------------|---------------------|
| MMU类型 | `&mut dyn MMU` | `&mut dyn AsyncMMU` |
| 执行方式 | 同步阻塞 | 异步非阻塞 |
| 并发支持 | 有限 | 良好 |
| 适用场景 | 一般场景 | 高并发、I/O密集型 |

## 迁移指南

### 从同步迁移到异步

1. **实现AsyncExecutionEngine trait**
   ```rust
   #[async_trait]
   impl AsyncExecutionEngine<IRBlock> for MyEngine {
       // 实现异步方法
   }
   ```

2. **使用AsyncMMU**
   ```rust
   async fn run_async(
       &mut self,
       mmu: &mut dyn AsyncMMU,  // 使用AsyncMMU
       block: &IRBlock,
   ) -> Result<ExecResult, VmError> {
       // 使用异步内存访问
       let data = mmu.read_async(addr).await?;
   }
   ```

3. **更新调用代码**
   ```rust
   // 旧代码（同步）
   let result = engine.run(&mut mmu, &block);
   
   // 新代码（异步）
   let result = engine.run_async(&mut mmu, &block).await?;
   ```

## 未来改进

1. **批量执行优化**：实现真正的并行批量执行
2. **自适应选择**：根据场景自动选择同步或异步版本
3. **性能监控**：添加异步执行的性能监控

## 相关文档

- [ExecutionEngine接口文档](../vm-core/src/lib.rs)
- [AsyncMMU接口文档](../vm-core/src/async_mmu.rs)
- [协程池文档](../vm-runtime/src/coroutine_pool.rs)

