# P1-01: 异步执行引擎实现指南

**目标**: 为虚拟机执行引擎添加async/await支持，集成tokio异步运行时

**完成标准**:
- ✅ AsyncExecutionEngine trait 定义完成
- ✅ JIT/解释器/混合执行器的异步实现
- ✅ 异步MMU接口
- ✅ 单元测试覆盖 (>80%)
- ✅ 性能基准测试 (<500ns异步开销)
- ✅ 与现有代码无缝集成

**时间线**: 2周(10个工作日)

---

## 设计概述

### 架构模式

```
┌─────────────────────────────────────────┐
│  Application Layer (tokio runtime)       │
├─────────────────────────────────────────┤
│  AsyncExecutionEngine Trait             │
├──────────────┬──────────────┬───────────┤
│ AsyncJIT     │ AsyncInterp. │ AsyncHybr.│
├──────────────┴──────────────┴───────────┤
│  AsyncMmu + TLB Integration             │
├─────────────────────────────────────────┤
│  Physical Memory + Device Layer          │
└─────────────────────────────────────────┘
```

### 核心接口

```rust
#[async_trait]
pub trait AsyncExecutionEngine: Send + Sync {
    async fn run_async(&mut self, block: &IrBlock, mmu: &mut dyn AsyncMmu) 
        -> Result<ExecutionResult, ExecutionError>;
    
    async fn query_and_compile(&mut self, addr: u64) -> Result<bool, ExecutionError>;
    
    fn get_stats(&self) -> ExecutionStats;
}

#[async_trait]
pub trait AsyncMmu: Send + Sync {
    async fn read_async(&self, addr: u64, size: usize) -> Result<Vec<u8>, MemoryError>;
    async fn write_async(&mut self, addr: u64, data: &[u8]) -> Result<(), MemoryError>;
    async fn translate_async(&self, addr: u64) -> Result<u64, MemoryError>;
}
```

---

## 实现步骤

### 第一阶段：准备工作 (Day 1)

**1.1 更新Cargo.toml依赖**

```toml
[dependencies]
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
futures = "0.3"
```

**1.2 模块结构创建**

```
vm-core/src/
  ├── async_executor.rs          # 已创建 ✓
  ├── async_mmu.rs               # 新建
  ├── async_hybrid.rs            # 新建
  └── lib.rs                      # 更新导出
```

**1.3 向后兼容性设计**

- 保留现有 ExecutionEngine trait
- 新增 AsyncExecutionEngine trait
- 统一错误类型（ExecutionError）
- 共享ExecutionStats结构

### 第二阶段：MMU异步实现 (Day 2-3)

**2.1 AsyncMmu实现**

```rust
// vm-core/src/async_mmu.rs

pub struct AsyncMmuImpl {
    tlb: Arc<AsyncTlbManager>,      // 来自P0-03
    memory: Arc<SharedMemory>,
    io_manager: Arc<IoManager>,
}

#[async_trait]
impl AsyncMmu for AsyncMmuImpl {
    async fn read_async(&self, addr: u64, size: usize) -> Result<Vec<u8>, MemoryError> {
        // 1. 异步TLB查询
        let phys_addr = self.tlb.query_async(addr).await?;
        
        // 2. 并发内存读取
        let chunk_size = 64; // 缓存行大小
        let mut results = Vec::new();
        
        for offset in (0..size).step_by(chunk_size) {
            let current_size = std::cmp::min(chunk_size, size - offset);
            let data = self.read_chunk_async(phys_addr + offset as u64, current_size).await?;
            results.extend(data);
        }
        
        Ok(results)
    }
    
    async fn write_async(&mut self, addr: u64, data: &[u8]) -> Result<(), MemoryError> {
        let phys_addr = self.tlb.query_async(addr).await?;
        
        // 并发写入
        for (offset, chunk) in data.chunks(64).enumerate() {
            self.write_chunk_async(phys_addr + (offset * 64) as u64, chunk).await?;
        }
        
        Ok(())
    }
    
    async fn translate_async(&self, addr: u64) -> Result<u64, MemoryError> {
        self.tlb.query_async(addr).await
    }
}
```

**2.2 异步TLB集成**

现有TLB实现中添加异步方法：

```rust
// 在AsyncTlbManager中
pub async fn query_async(&self, addr: u64) -> Result<u64, MemoryError> {
    // 快速路径：内存中的TLB条目
    if let Some(entry) = self.fast_lookup(addr) {
        return Ok(entry);
    }
    
    // 慢速路径：异步页表遍历
    self.slow_walk_async(addr).await
}

async fn slow_walk_async(&self, addr: u64) -> Result<u64, MemoryError> {
    // 使用tokio::spawn进行并发页表遍历
    tokio::task::spawn_blocking(move || {
        // 同步页表遍历逻辑
    }).await
}
```

### 第三阶段：异步执行器实现 (Day 4-7)

**3.1 AsyncJitExecutor**

```rust
// vm-core/src/async_executor.rs (扩展)

pub struct AsyncJitExecutor {
    compiler: Arc<JitCompiler>,
    compiled_cache: Arc<DashMap<u64, CompiledCode>>,
    hotspot_detector: Arc<HotspotDetector>,
    stats: Arc<Mutex<ExecutionStats>>,
}

#[async_trait]
impl AsyncExecutionEngine for AsyncJitExecutor {
    async fn run_async(
        &mut self,
        block: &IrBlock,
        mmu: &mut dyn AsyncMmu,
    ) -> Result<ExecutionResult, ExecutionError> {
        let start_time = std::time::Instant::now();
        
        // 1. 并发编译和执行准备
        let compile_task = async {
            if !self.compiled_cache.contains_key(&block.start_addr) {
                self.compile_async(block).await
            } else {
                Ok(())
            }
        };
        
        let mmu_prefetch = async {
            // 预加载可能的内存访问
            self.prefetch_addresses(block, mmu).await
        };
        
        // 等待两个任务并发完成
        tokio::join!(compile_task, mmu_prefetch);
        
        // 2. 执行编译代码
        let code = self.compiled_cache.get(&block.start_addr)
            .ok_or(ExecutionError::CompilationError("Not compiled".to_string()))?;
        
        let cycles = code.execute_async(mmu).await?;
        
        let elapsed = start_time.elapsed();
        let mut stats = self.stats.lock().await;
        stats.total_cycles += cycles;
        stats.async_operations += 1;
        
        Ok(ExecutionResult {
            pc: block.end_addr,
            cycles,
            success: true,
        })
    }
    
    async fn query_and_compile(&mut self, addr: u64) -> Result<bool, ExecutionError> {
        let compiled = self.compiled_cache.contains_key(&addr);
        
        if !compiled && self.hotspot_detector.is_hot(addr) {
            // 异步启动后台编译任务
            let compiler = self.compiler.clone();
            let cache = self.compiled_cache.clone();
            
            tokio::spawn(async move {
                // 后台编译，不阻塞主线程
                let _ = compiler.compile_async(addr).await;
            });
        }
        
        Ok(compiled)
    }
    
    fn get_stats(&self) -> ExecutionStats {
        // 需要使用block_on或者改为异步获取
        // 暂时返回最后一次的快照
        ExecutionStats {
            total_instructions: 0,
            total_cycles: 0,
            jit_compilations: 0,
            async_operations: 0,
        }
    }
}

impl AsyncJitExecutor {
    async fn compile_async(&mut self, block: &IrBlock) -> Result<(), ExecutionError> {
        // 异步编译流程
        let ir_optimized = self.optimize_async(block).await?;
        let machine_code = self.compiler.compile_async(&ir_optimized).await?;
        
        self.compiled_cache.insert(
            block.start_addr,
            CompiledCode::new(machine_code),
        );
        
        Ok(())
    }
    
    async fn optimize_async(&self, block: &IrBlock) -> Result<IrBlock, ExecutionError> {
        // 异步优化过程
        tokio::task::spawn_blocking({
            let block = block.clone();
            move || {
                // 同步优化逻辑在线程池中执行
                optimize_ir(block)
            }
        }).await
        .map_err(|e| ExecutionError::AsyncError(e.to_string()))?
    }
    
    async fn prefetch_addresses(&self, block: &IrBlock, mmu: &mut dyn AsyncMmu) {
        // 并发预加载内存地址
        let mut tasks = Vec::new();
        
        for instr in &block.instructions {
            if let IrOp::Load { addr, size, .. } = instr {
                let addr = *addr as u64;
                let size = *size as usize;
                let task = mmu.read_async(addr, size);
                tasks.push(task);
            }
        }
        
        // 并发执行所有预加载
        futures::future::join_all(tasks).await;
    }
}
```

**3.2 AsyncInterpreterExecutor**

```rust
pub struct AsyncInterpreterExecutor {
    stats: Arc<Mutex<ExecutionStats>>,
    max_concurrent_ops: usize,
}

#[async_trait]
impl AsyncExecutionEngine for AsyncInterpreterExecutor {
    async fn run_async(
        &mut self,
        block: &IrBlock,
        mmu: &mut dyn AsyncMmu,
    ) -> Result<ExecutionResult, ExecutionError> {
        // 使用Semaphore限制并发内存操作数
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.max_concurrent_ops));
        
        let mut total_cycles = 0u64;
        
        for instr in &block.instructions {
            let permit = semaphore.acquire().await
                .map_err(|e| ExecutionError::AsyncError(e.to_string()))?;
            
            let cycles = match instr {
                IrOp::Load { addr, size, .. } => {
                    let _data = mmu.read_async(*addr as u64, *size as usize).await?;
                    drop(permit);
                    5
                },
                IrOp::Store { addr, value: _, size } => {
                    let _data = vec![0u8; *size as usize];
                    mmu.write_async(*addr as u64, &_data).await?;
                    drop(permit);
                    5
                },
                _ => {
                    drop(permit);
                    2
                }
            };
            
            total_cycles += cycles;
        }
        
        let mut stats = self.stats.lock().await;
        stats.total_instructions += block.instructions.len() as u64;
        stats.total_cycles += total_cycles;
        
        Ok(ExecutionResult {
            pc: block.end_addr,
            cycles: total_cycles,
            success: true,
        })
    }
    
    async fn query_and_compile(&mut self, _addr: u64) -> Result<bool, ExecutionError> {
        Ok(false)
    }
    
    fn get_stats(&self) -> ExecutionStats {
        // 返回统计快照
        ExecutionStats {
            total_instructions: 0,
            total_cycles: 0,
            jit_compilations: 0,
            async_operations: 0,
        }
    }
}
```

**3.3 AsyncHybridExecutor**

```rust
// vm-core/src/async_hybrid.rs

pub struct AsyncHybridExecutor {
    jit: Arc<AsyncJitExecutor>,
    interpreter: Arc<AsyncInterpreterExecutor>,
    hotspot_threshold: u32,
    execution_count: Arc<DashMap<u64, u32>>,
}

#[async_trait]
impl AsyncExecutionEngine for AsyncHybridExecutor {
    async fn run_async(
        &mut self,
        block: &IrBlock,
        mmu: &mut dyn AsyncMmu,
    ) -> Result<ExecutionResult, ExecutionError> {
        // 更新执行计数
        let count = self.execution_count
            .entry(block.start_addr)
            .or_insert(0);
        *count += 1;
        
        // 基于热点选择执行器
        if *count > self.hotspot_threshold {
            // 使用JIT执行
            self.jit.run_async(block, mmu).await
        } else {
            // 使用解释器执行
            self.interpreter.run_async(block, mmu).await
        }
    }
    
    async fn query_and_compile(&mut self, addr: u64) -> Result<bool, ExecutionError> {
        self.jit.query_and_compile(addr).await
    }
    
    fn get_stats(&self) -> ExecutionStats {
        // 合并两个执行器的统计
        ExecutionStats {
            total_instructions: 0,
            total_cycles: 0,
            jit_compilations: 0,
            async_operations: 0,
        }
    }
}
```

### 第四阶段：测试 (Day 8-9)

**4.1 单元测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_async_jit_basic() {
        // 测试基本JIT异步执行
    }
    
    #[tokio::test]
    async fn test_async_interpreter_memory_ops() {
        // 测试解释器异步内存操作
    }
    
    #[tokio::test]
    async fn test_async_hybrid_hotspot_detection() {
        // 测试混合执行器的热点检测
    }
    
    #[tokio::test]
    async fn test_concurrent_mmu_access() {
        // 并发MMU访问测试
    }
    
    #[tokio::test]
    async fn test_async_compilation_background() {
        // 后台异步编译测试
    }
}
```

**4.2 性能测试**

```rust
#[cfg(test)]
mod benchmarks {
    use criterion::*;
    
    fn async_execution_latency(c: &mut Criterion) {
        // 测试异步执行的延迟 (<500ns)
    }
    
    fn concurrent_memory_throughput(c: &mut Criterion) {
        // 并发内存操作吞吐量
    }
    
    fn hotspot_compilation_overhead(c: &mut Criterion) {
        // 热点编译的额外开销
    }
}
```

### 第五阶段：集成与优化 (Day 10)

**5.1 与现有代码集成**

```rust
// vm-runtime/src/lib.rs 中集成

pub struct VirtualMachine {
    executor: Box<dyn AsyncExecutionEngine>,
    runtime: tokio::runtime::Runtime,
    // ...
}

impl VirtualMachine {
    pub async fn run_async(&mut self, block: &IrBlock) -> Result<ExecutionResult, ExecutionError> {
        self.executor.run_async(block, &mut self.mmu).await
    }
}
```

**5.2 性能优化**

- TLB预热：初始化时预加载常用地址
- 编译缓存：使用DashMap支持无锁并发
- 内存池：复用分配减少堆压力
- 调度优化：在tokio task中平衡工作负载

---

## 验收标准

| 指标 | 目标 | 验证方法 |
|------|------|---------|
| 异步接口完整 | 100% | 编译通过 |
| 单元测试覆盖 | >80% | cargo test |
| 异步开销 | <500ns | criterion benchmark |
| 并发操作支持 | >100 vCPU | stress test |
| 向后兼容 | 100% | 现有测试通过 |

---

## 关键文件清单

| 文件 | 行数 | 状态 | 
|------|------|------|
| vm-core/src/async_executor.rs | 240 | ✅ 已创建 |
| vm-core/src/async_mmu.rs | 180 | 待创建 |
| vm-core/src/async_hybrid.rs | 150 | 待创建 |
| tests/async_execution_tests.rs | 300 | 待创建 |
| docs/ASYNC_EXECUTION_GUIDE.md | 200 | 待创建 |
| **总计** | **1,070** | |

---

## 风险和缓解措施

| 风险 | 可能性 | 影响 | 缓解 |
|------|--------|------|------|
| 异步编译死锁 | 中 | 高 | 使用超时 + 死锁检测 |
| 内存泄漏 | 低 | 高 | Arc智能指针 + MIRI检查 |
| 性能倒退 | 低 | 中 | 基准测试 + CI集成 |

---

## 后续步骤

完成P1-01后：

1. **P1-02**: 协程调度器集成 (建立在async基础上)
2. **P1-03**: 性能基准框架 (测试异步带来的改进)
3. **P2-01**: 分层编译 (3层/4层编译的异步支持)
