# 代码风格指南

本文档定义了 Rust 虚拟机项目的代码风格和最佳实践。

## 目录

- [Rust 代码风格](#rust-代码风格)
- [命名约定](#命名约定)
- [文档规范](#文档规范)
- [错误处理](#错误处理)
- [并发安全](#并发安全)
- [性能优化](#性能优化)
- [测试规范](#测试规范)
- [模块组织](#模块组织)

---

## Rust 代码风格

### 基本原则

1. **遵循 Rust 官方风格指南**
   - 使用 `cargo fmt` 自动格式化代码
   - 参考官方 [Rust API 指南](https://rust-lang.github.io/api-guidelines/)
   - 遵循 [Rust 编码规范](https://rust-lang.github.io/rust-clippy/)

2. **使用 `rustfmt`**
   ```bash
   # 检查格式
   cargo fmt -- --check

   # 自动格式化
   cargo fmt
   ```

3. **使用 `clippy` 检查代码质量**
   ```bash
   # 检查所有警告
   cargo clippy -- -D warnings
   ```

### 代码格式

#### 行长度

- 最大行长度：100 字符
- 超过限制时考虑拆分

```rust
// ✅ 推荐
let result = some_very_long_function_name(
    arg1,
    arg2,
    arg3,
);

// ❌ 避免
let result = some_very_long_function_name(arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10);
```

#### 缩进

- 使用 4 个空格缩进
- 不使用 Tab

```rust
// ✅ 推荐
fn example() {
    if condition {
        do_something();
    }
}

// ❌ 避免
fn example() {
	if condition {
		do_something();
	}
}
```

#### 空行

- 函数之间空一行
- 相关代码块之间空一行
- 不连续空行

```rust
// ✅ 推荐
fn function_one() {
    // ...
}

fn function_two() {
    // ...
}

// ❌ 避免
fn function_one() {
    // ...
}



fn function_two() {
    // ...
}
```

---

## 命名约定

### 类型命名

使用 `PascalCase`（大驼峰）：

```rust
// ✅ 推荐
struct VirtualMachine;
enum GuestArch;
trait JITCompiler;
type Result<T> = std::result::Result<T, Error>;

// ❌ 避免
struct virtual_machine;
enum guest_arch;
trait jit_compiler;
```

### 函数和方法命名

使用 `snake_case`：

```rust
// ✅ 推荐
fn create_vm(id: VmId, config: VmConfig) -> Result<VirtualMachine>;
fn translate_address(mmu: &mut MMU, addr: GuestAddr) -> Result<GuestPhysAddr>;
fn reset_device(&mut self);

// ❌ 避免
fn CreateVM(id: VmId, config: VmConfig) -> Result<VirtualMachine>;
fn TranslateAddress(mmu: &mut MMU, addr: GuestAddr) -> Result<GuestPhysAddr>;
fn ResetDevice(&mut self);
```

### 常量命名

使用 `SCREAMING_SNAKE_CASE`：

```rust
// ✅ 推荐
const MAX_VCPUS: usize = 256;
const PAGE_SIZE: u64 = 4096;
const DEFAULT_TIMEOUT_MS: u64 = 5000;

// ❌ 避免
const max_vcpus: usize = 256;
const page_size: u64 = 4096;
const default_timeout_ms: u64 = 5000;
```

### 静态变量命名

使用 `SCREAMING_SNAKE_CASE`：

```rust
// ✅ 推荐
static GLOBAL_COUNTER: AtomicU64 = AtomicU64::new(0);

// ❌ 避免
static global_counter: AtomicU64 = AtomicU64::new(0);
```

### 模块命名

使用 `snake_case`：

```rust
// ✅ 推荐
mod device_emulation;
mod jit_compiler;
mod memory_manager;

// ❌ 避免
mod DeviceEmulation;
mod JITCompiler;
mod MemoryManager;
```

### 变量和参数命名

使用 `snake_case`：

```rust
// ✅ 推荐
let vm_id = VmId::new("test-vm")?;
let memory_size = 1024 * 1024 * 1024;
fn compile_block(ir_block: &IRBlock) -> Result<CompiledCode>;

// ❌ 避免
let vmId = VmId::new("test-vm")?;
let MemorySize = 1024 * 1024 * 1024;
fn CompileBlock(ir_block: &IRBlock) -> Result<CompiledCode>;
```

### 特征（Trait）命名

使用 `PascalCase`：

```rust
// ✅ 推荐
trait Device {
    fn read(&mut self, offset: u64, size: usize) -> Result<Vec<u8>>;
    fn write(&mut self, offset: u64, data: &[u8]) -> Result<()>;
}

// ❌ 避免
trait device {
    fn read(&mut self, offset: u64, size: usize) -> Result<Vec<u8>>;
}
```

### 泛型参数命名

使用简洁、描述性的名称：

```rust
// ✅ 推荐
fn process_data<T: Clone>(data: &[T]) -> Vec<T>;
struct Cache<K, V> {
    map: HashMap<K, V>,
}

// ❌ 避免
fn process_data<TypeThatNeedsToBeCloned>(data: &[TypeThatNeedsToBeCloned]) -> Vec<TypeThatNeedsToBeCloned>;
struct Cache<KeyType, ValueType> {
    map: HashMap<KeyType, ValueType>,
}
```

---

## 文档规范

### 文档注释格式

使用 `///` 表示文档注释：

```rust
/// JIT 编译器接口
///
/// 提供将中间表示（IR）编译为目标机器码的功能
pub trait JITCompiler {
    /// 编译 IR 块为机器码
    ///
    /// # 参数
    ///
    /// * `ir_block` - 要编译的 IR 块
    ///
    /// # 返回
    ///
    /// 返回编译后的代码和元数据
    ///
    /// # 错误
    ///
    /// 当 IR 块无效或编译失败时返回错误
    fn compile(&mut self, ir_block: &IRBlock) -> Result<CompiledCode, CompileError>;
}
```

### 文档注释内容

文档应包含：

1. **简要描述**
2. **参数说明**（使用 `# 参数`）
3. **返回值说明**（使用 `# 返回`）
4. **错误说明**（使用 `# 错误`）
5. **示例代码**（使用 `# 示例` 和 ```）

### 模块文档

```rust
//! 设备模拟模块
//!
//! 提供虚拟设备的实现，包括网络、磁盘和 GPU 设备。
//!
//! # 功能
//!
//! - 网络设备模拟（VirtIO-Net）
//! - 磁盘设备模拟（VirtIO-Block）
//! - GPU 设备模拟（VirtIO-GPU）
//!
//! # 使用示例
//!
//! ```rust
//! use vm_core::device_emulation::{VirtualNetworkDevice, NetworkConfig};
//!
//! let config = NetworkConfig::default();
//! let device = VirtualNetworkDevice::new(config);
//! ```

pub struct VirtualNetworkDevice {
    // ...
}
```

---

## 错误处理

### 使用 `Result` 类型

所有可能失败的操作应返回 `Result<T, E>`：

```rust
// ✅ 推荐
pub fn create_vm(id: VmId, config: VmConfig) -> Result<VirtualMachine, VmError> {
    // ...
}

// ❌ 避免
pub fn create_vm(id: VmId, config: VmConfig) -> VirtualMachine {
    // ...
}
pub fn create_vm_or_error(id: VmId, config: VmConfig) -> Option<VirtualMachine> {
    // ...
}
```

### 使用 `?` 运算符

传播错误时使用 `?` 运算符：

```rust
// ✅ 推荐
pub fn run_program(&mut self) -> Result<(), VmError> {
    self.load_program()?;
    self.execute()?;
    self.cleanup()?;
    Ok(())
}

// ❌ 避免
pub fn run_program(&mut self) -> Result<(), VmError> {
    match self.load_program() {
        Ok(_) => {},
        Err(e) => return Err(e),
    }
    match self.execute() {
        Ok(_) => {},
        Err(e) => return Err(e),
    }
    self.cleanup()
}
```

### 错误类型定义

使用 `thiserror` 定义错误类型：

```rust
use thiserror::Error;

/// 虚拟机错误
#[derive(Debug, Error)]
pub enum VmError {
    /// 内存访问违规
    #[error("内存访问违规: 地址 0x{addr:X}")]
    MemoryAccessViolation { addr: u64 },

    /// 指令解码错误
    #[error("指令解码错误: {message}")]
    DecodeError { message: String },

    /// JIT 编译错误
    #[error("JIT 编译错误: {0}")]
    JITError(String),

    /// 设备错误
    #[error("设备错误: {0}")]
    DeviceError(String),
}

impl From<MemoryError> for VmError {
    fn from(err: MemoryError) -> Self {
        VmError::MemoryAccessViolation { addr: err.addr }
    }
}
```

### 错误上下文

使用 `anyhow` 添加错误上下文：

```rust
use anyhow::{Context, Result};

pub fn execute_instruction(&mut self) -> Result<()> {
    self.decode_instruction()
        .context("无法解码指令")?;
    self.translate_address()
        .context("地址转换失败")?;
    Ok(())
}
```

---

## 并发安全

### 共享只读数据

使用 `Arc` 共享只读数据：

```rust
// ✅ 推荐
use std::sync::Arc;

struct SharedConfig {
    inner: Arc<Config>,
}

impl SharedConfig {
    pub fn new(config: Config) -> Self {
        Self {
            inner: Arc::new(config),
        }
    }
}
```

### 可变状态保护

使用 `Mutex` 或 `RwLock` 保护可变状态：

```rust
// ✅ 推荐
use std::sync::{Arc, Mutex};

struct SharedState {
    inner: Arc<Mutex<State>>,
}

impl SharedState {
    pub fn update(&self, value: i32) {
        let mut state = self.inner.lock().unwrap();
        state.value = value;
    }
}

// 对于读多写少的场景，使用 RwLock
use std::sync::RwLock;

struct Cache {
    data: Arc<RwLock<HashMap<u64, u8>>>,
}
```

### 避免锁竞争

1. **减少锁的持有时间**
   ```rust
   // ✅ 推荐
   let value = {
       let guard = self.mutex.lock().unwrap();
       guard.get_value().clone()
   };
   process_value(value);

   // ❌ 避免
   let guard = self.mutex.lock().unwrap();
   process_value(guard.get_value());
   ```

2. **使用更细粒度的锁**
   ```rust
   // ✅ 推荐
   struct TwoCache {
       cache1: Arc<Mutex<HashMap<u64, u8>>>,
       cache2: Arc<Mutex<HashMap<u64, u8>>>,
   }

   // ❌ 避免
   struct TwoCache {
       caches: Arc<Mutex<[HashMap<u64, u8; 2]>>,
   }
   ```

3. **考虑使用无锁数据结构**
   - 项目提供了 `vm-common` 模块，包含无锁队列和哈希表

---

## 性能优化

### 避免不必要的分配

```rust
// ✅ 推荐
fn sum(numbers: &[i32]) -> i32 {
    numbers.iter().sum()
}

// ❌ 避免
fn sum(numbers: &[i32]) -> i32 {
    let mut total = 0;
    for &n in numbers {
        total += n;
    }
    total
}
```

### 使用迭代器

```rust
// ✅ 推荐
fn filter_even(numbers: &[i32]) -> Vec<i32> {
    numbers.iter().filter(|&&n| n % 2 == 0).cloned().collect()
}

// ❌ 避免
fn filter_even(numbers: &[i32]) -> Vec<i32> {
    let mut result = Vec::new();
    for &n in numbers {
        if n % 2 == 0 {
            result.push(n);
        }
    }
    result
}
```

### 预分配容量

```rust
// ✅ 推荐
fn collect_results(results: Vec<i32>) -> Vec<i32> {
    let mut filtered = Vec::with_capacity(results.len());
    for r in results {
        if r > 0 {
            filtered.push(r);
        }
    }
    filtered
}

// ❌ 避免
fn collect_results(results: Vec<i32>) -> Vec<i32> {
    let mut filtered = Vec::new();
    for r in results {
        if r > 0 {
            filtered.push(r);
        }
    }
    filtered
}
```

### 使用 `#[inline]`

对于小型、频繁调用的函数使用 `#[inline]`：

```rust
#[inline]
pub fn align_to_page(addr: u64, page_size: u64) -> u64 {
    (addr + page_size - 1) & !(page_size - 1)
}
```

### 避免使用 `Box`

除非必要，避免使用 `Box`：

```rust
// ✅ 推荐
pub struct Instruction {
    pub opcode: u8,
    pub operands: [u8; 3],
}

// ❌ 避免
pub struct Instruction {
    pub opcode: u8,
    pub operands: Box<[u8]>,
}
```

---

## 测试规范

### 单元测试

每个模块都应有单元测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operation() {
        let result = some_function();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_error_handling() {
        let result = some_function_that_fails();
        assert!(result.is_err());
    }

    #[test]
    #[should_panic(expected = "expected panic message")]
    fn test_panic() {
        panic!("expected panic message");
    }
}
```

### 集成测试

集成测试放在 `tests/` 目录下：

```rust
// tests/integration_tests.rs
use vm_core::{VmConfig, VmId};

#[test]
fn test_vm_lifecycle() {
    let config = VmConfig::default();
    let vm_id = VmId::new("test-vm".to_string()).unwrap();

    // 测试代码
}
```

### 测试命名

使用 `test_` 前缀：

```rust
// ✅ 推荐
#[test]
fn test_memory_allocation() {}
#[test]
fn test_address_translation() {}

// ❌ 避免
#[test]
fn memory_allocation() {}
#[test]
fn it_should_translate_address() {}
```

### 参数化测试

使用 `rstest` 或内置宏进行参数化测试：

```rust
#[test]
fn test_alignment_for_various_sizes() {
    let sizes = [4096, 8192, 16384];
    for size in sizes {
        assert!(is_aligned(size, 4096));
    }
}
```

---

## 模块组织

### 模块结构

遵循项目的模块结构：

```
vm/
├── vm-core/           # 核心领域模型和接口
│   ├── src/
│   │   ├── domain/         # 领域模型
│   │   ├── domain_services/ # 领域服务
│   │   ├── value_objects/  # 值对象
│   │   └── lib.rs
├── vm-engine-jit/     # JIT 编译引擎
├── vm-runtime/        # 运行时环境
├── vm-mem/           # 内存管理
└── vm-cross-arch/     # 跨架构支持
```

### 模块导入顺序

```rust
// 1. 标准库
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// 2. 外部依赖
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// 3. 内部依赖
use vm_core::domain::VirtualMachine;
use vm_core::value_objects::VmId;

// 4. 本地模块
use super::*;
```

### 可见性

- 默认使用 `pub` 导出公开 API
- 使用 `pub(crate)` 限制在 crate 内可见
- 使用 `pub(super)` 限制在父模块可见
- 使用 `pub(in path::to::module)` 限制在特定模块可见

```rust
// ✅ 推荐
pub mod public_module {
    pub fn public_function() {}
    pub(crate) fn crate_function() {}
    pub(super) fn super_function() {}
    pub(in crate::parent::sibling) fn sibling_function() {}
}

// ❌ 避免
mod public_module {
    pub fn public_function() {}
    fn crate_function() {}
}
```

---

## 工具和脚本

### Pre-commit 钩子

项目提供了 pre-commit 脚本：

```bash
#!/bin/bash
# .githooks/pre-commit

echo "Running pre-commit checks..."

cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --quiet

if [ $? -ne 0 ]; then
    echo "Pre-commit checks failed!"
    exit 1
fi
```

### CI 配置

使用 GitHub Actions 进行 CI：

```yaml
name: CI

on: [push, pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          components: rustfmt, clippy
      - run: cargo fmt -- --check
      - run: cargo clippy -- -D warnings
      - run: cargo test
```

---

## 总结

遵循这些代码风格指南将帮助保持代码库的一致性和可维护性：

1. 使用 `cargo fmt` 和 `cargo clippy` 自动检查
2. 遵循命名约定
3. 编写完整的文档
4. 正确处理错误
5. 确保并发安全
6. 优化性能关键路径
7. 编写全面的测试
8. 组织好模块结构

如有疑问，请参考 `CONTRIBUTING.md` 或联系维护者。
