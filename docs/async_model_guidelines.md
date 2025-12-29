# 异步模型准则

## 概述

本文档定义了VM项目中异步锁和同步锁的统一使用准则，以确保代码的一致性和性能优化。

## 锁类型选择

### 异步锁 (`tokio::sync`)

**使用场景：**
- ✅ **I/O密集型操作**：网络请求、文件读写、数据库访问
- ✅ **长时间持有锁**：锁持有时间可能超过100μs
- ✅ **跨await点持有锁**：需要在异步操作期间保持锁
- ✅ **异步上下文**：在`async fn`或`.await`调用的代码路径中
- ✅ **高并发I/O场景**：大量并发网络或磁盘操作

**示例：**
```rust
use tokio::sync::{Mutex as AsyncMutex, RwLock as AsyncRwLock};

#[async_trait]
pub trait AsyncVmService {
    async fn handle_request(&self) -> Result<(), Error>;
}

pub struct AsyncService {
    data: AsyncRwLock<Vec<u8>>,
}

impl AsyncService {
    pub async fn write_data(&self, bytes: Vec<u8>) -> Result<(), Error> {
        let mut data = self.data.write().await;
        *data = bytes;
        Ok(())
    }
}
```

### 同步锁 (`parking_lot`)

**使用场景：**
- ✅ **内存密集型操作**：快速内存访问、计算密集型任务
- ✅ **短时间持有锁**：锁持有时间<100μs
- ✅ **无异步上下文**：同步代码路径
- ✅ **性能关键路径**：需要最小化锁开销
- ✅ **高并发内存访问**：多线程共享内存结构

**示例：**
```rust
use parking_lot::{Mutex, RwLock};

pub struct CoreMemory {
    data: RwLock<Vec<u8>>,
}

impl CoreMemory {
    pub fn read_u8(&self, addr: u64) -> Result<u8, MemoryError> {
        let data = self.data.read();
        data.get(addr as usize).copied().ok_or(MemoryError::InvalidAddress)
    }

    pub fn write_u8(&self, addr: u64, value: u8) -> Result<(), MemoryError> {
        let mut data = self.data.write();
        if let Some(ptr) = data.get_mut(addr as usize) {
            *ptr = value;
            Ok(())
        } else {
            Err(MemoryError::InvalidAddress)
        }
    }
}
```

## 当前项目使用统计

基于代码扫描结果：
- **tokio::sync 使用**: 62处
- **parking_lot 使用**: 70处

## 迁移准则

### 何时迁移到异步锁

**需要迁移的情况：**
1. 在异步函数中使用`parking_lot`锁，并且有`.await`调用
2. 锁持有时间超过100μs，且在异步上下文中
3. 需要与I/O操作并发执行的场景

**示例（需要迁移）：**
```rust
// ❌ 错误：在异步上下文中使用同步锁
pub async fn process_request(&self) -> Result<(), Error> {
    let data = self.lock.lock(); // 可能阻塞异步运行时
    self.async_operation().await?; // 死锁风险
    Ok(())
}

// ✅ 正确：使用异步锁
pub async fn process_request(&self) -> Result<(), Error> {
    let data = self.lock.lock().await;
    self.async_operation().await?;
    Ok(())
}
```

### 何时保留同步锁

**应该保留的情况：**
1. 纯内存操作，锁持有时间<10μs
2. 热路径代码，需要最小化开销
3. 无异步上下文
4. 性能测试显示同步锁更快

**示例（应该保留）：**
```rust
// ✅ 正确：快速内存操作使用同步锁
pub fn fast_memory_access(&self, addr: u64) -> u8 {
    let data = self.data.read(); // parking_lot RwLock
    data[addr as usize]
}
```

## 迁移步骤

### 第一步：审计
```bash
# 查找所有混合使用异步和同步锁的文件
grep -r "parking_lot::" --include="*.rs" | grep -v "test"
grep -r "tokio::sync::" --include="*.rs" | grep -v "test"
```

### 第二步：识别问题模式
```rust
// 问题模式1：锁+await
let guard = self.sync_lock.lock();
self.async_fn().await; // ❌

// 问题模式2：长时间持有锁
let guard = self.lock.lock();
std::thread::sleep(Duration::from_millis(100)); // ❌
```

### 第三步：应用修复
```rust
// 修复1：使用异步锁
let guard = self.async_lock.lock().await;
self.async_fn().await; // ✅

// 修复2：减少锁持有时间
let result = {
    let guard = self.lock.lock();
    guard.compute_result() // 快速操作
}; // 锁在这里释放
self.async_fn().await; // ✅
```

### 第四步：验证
```bash
# 运行测试
cargo test --all

# 运行clippy检查
cargo clippy --workspace --all-targets

# 性能基准测试
cargo bench --all
```

## 推荐的模块配置

### vm-engine (JIT编译器)
```rust
// ✅ 推荐：使用异步锁
use tokio::sync::{Mutex, RwLock};

pub struct JitCompiler {
    cache: Arc<Mutex<HashMap<BlockId, CompiledCode>>>,
}
```

### vm-runtime (运行时)
```rust
// ✅ 推荐：混合使用
use parking_lot::{Mutex, RwLock}; // 内存管理
use tokio::sync::{Mutex as AsyncMutex}; // I/O操作

pub struct Runtime {
    memory: RwLock<Vec<u8>>,      // 快速访问
    io_handler: AsyncMutex<IoHandler>, // 可能阻塞
}
```

### vm-core (核心)
```rust
// ✅ 推荐：主要使用同步锁
use parking_lot::{Mutex, RwLock};

pub struct CoreMemory {
    data: RwLock<Vec<u8>>,
    mmu: Mutex<Mmu>,
}
```

## 性能考虑

### 同步锁优势
- **低开销**: `parking_lot::Mutex`比`std::sync::Mutex`快5-10倍
- **无运行时依赖**: 不依赖tokio运行时
- **适合内存操作**: 零拷贝访问

### 异步锁优势
- **非阻塞**: 不会阻塞异步运行时
- **可组合**: 与`.await`良好配合
- **公平性**: 内置FIFO等待队列

## 检查清单

在代码审查时，检查以下项目：

- [ ] 异步函数中使用异步锁
- [ ] 同步函数中使用同步锁
- [ ] 锁持有时间<100μs（同步）或>100μs（异步）
- [ ] 没有跨`.await`持有同步锁
- [ ] I/O操作使用异步锁
- [ ] 内存密集型操作使用同步锁

## 常见陷阱

### 陷阱1：异步上下文中的死锁
```rust
// ❌ 死锁
async fn bad_example() {
    let guard1 = lock1.lock().await;
    let guard2 = lock2.lock().await; // 可能死锁
}

// ✅ 正确：使用try_lock或超时
async fn good_example() {
    let guard1 = lock1.lock().await;
    if let Ok(guard2) = lock2.try_lock() {
        // 处理
    }
}
```

### 陷阱2：过度使用异步锁
```rust
// ❌ 不必要的异步锁
fn fast_operation(data: &AsyncMutex<u32>) -> u32 {
    let guard = data.blocking_lock(); // 阻塞调用
    *guard
}

// ✅ 使用同步锁
fn fast_operation(data: &Mutex<u32>) -> u32 {
    let guard = data.lock();
    *guard
}
```

## 参考资料

- [Tokio Mutex文档](https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html)
- [parking_lot文档](https://docs.rs/parking_lot/latest/parking_lot/)
- [Rust异步编程指南](https://rust-lang.github.io/async-book/)

## 版本历史

- v1.0 (2024-12-29): 初始版本
- 基于当前代码库审计创建
