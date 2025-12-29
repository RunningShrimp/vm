# 统一错误处理策略

本文档定义了整个虚拟机系统的统一错误处理策略，确保所有模块使用一致的错误类型和错误处理模式。

## 目录

- [错误类型层次](#错误类型层次)
- [错误上下文](#错误上下文)
- [错误恢复](#错误恢复)
- [错误收集和聚合](#错误收集和聚合)
- [错误日志](#错误日志)
- [最佳实践](#最佳实践)

---

## 错误类型层次

### 统一错误类型

所有模块应使用 `vm_core::error::VmError` 作为统一的错误类型：

```rust
use vm_core::error::VmError;
use vm_core::error::{CoreError, MemoryError, ExecutionError, DeviceError, PlatformError};

// 返回统一错误类型
fn my_function() -> Result<(), VmError> {
    Ok(())
}
```

### 错误分类

`VmError` 提供以下错误分类：

| 错误类型 | 说明 | 使用场景 |
|---------|------|---------|
| `CoreError` | 核心/基础架构错误 | 配置错误、无效状态、未支持功能、内部错误、资源耗尽、并发错误、无效参数 |
| `MemoryError` | 内存管理错误 | 访问违规、映射失败、分配失败、无效地址、对齐错误、页表错误 |
| `ExecutionError` | 执行引擎错误 | 故障/异常、无效指令、执行暂停、取指失败、超时、JIT编译错误 |
| `DeviceError` | 设备模拟错误 | 设备未找到、初始化失败、IO操作失败、配置错误、设备繁忙、不支持的操作 |
| `PlatformError` | 平台/加速器错误 | 加速器不可用、虚拟化管理程序错误、不支持的架构、权限不足、系统调用失败 |
| `WithContext` | 带上下文的错误包装器 | 为其他错误添加上下文信息 |
| `Multiple` | 多个错误的聚合 | 收集多个错误 |

### 错误示例

```rust
// CoreError 示例
let error = VmError::Core(CoreError::InvalidConfig {
    message: "CPU cores must be at least 1".to_string(),
    field: "cpu.cores".to_string(),
});

// MemoryError 示例
let error = VmError::Memory(MemoryError::AccessViolation {
    addr: GuestAddr(0x8000_0000),
    msg: "Permission denied".to_string(),
    access_type: Some(AccessType::Write),
});

// ExecutionError 示例
let error = VmError::Execution(ExecutionError::Fault(Fault::PageFault));

// DeviceError 示例
let error = VmError::Device(DeviceError::NotFound {
    device_type: "UART".to_string(),
    identifier: "uart0".to_string(),
});
```

---

## 错误上下文

### 添加上下文

使用 `ErrorContext` trait 为错误添加上下文信息：

```rust
use vm_core::error::ErrorContext;

fn read_memory(mmu: &mut dyn MMU, addr: GuestAddr) -> Result<u64, VmError> {
    mmu.read(addr, 8)
        .context("Failed to read instruction at PC")
}

fn with_dynamic_context() -> Result<u64, VmError> {
    let pc = GuestAddr(0x1000);
    mmu.read(pc, 4)
        .with_context(|| format!("Failed to read at PC {:#x}", pc))
}
```

### 错误链

错误支持错误链，可以追踪错误的来源：

```rust
let error = VmError::WithContext {
    error: Box::new(VmError::Memory(MemoryError::AccessViolation { ... })),
    context: "Failed to execute instruction".to_string(),
    backtrace: Some(Arc::new(Backtrace::capture())),
};

// 获取错误链
let mut source = error.source();
while let Some(err) = source {
    println!("Caused by: {}", err);
    source = err.source();
}
```

---

## 错误恢复

### 错误恢复策略

定义错误恢复策略以支持自动重试：

```rust
use vm_core::error::{ErrorRecovery, ErrorRecoveryStrategy, retry_with_strategy};

// 检查错误是否可重试
let error = VmError::Device(DeviceError::Busy { ... });
if error.is_retryable() {
    // 可以重试
}

// 使用重试策略
let result = retry_with_strategy(
    || {
        device.write(offset, value, 4)
    },
    ErrorRecoveryStrategy::Fixed {
        max_attempts: 3,
        delay_ms: 100,
    },
);
```

### 支持的恢复策略

| 策略 | 说明 | 适用场景 |
|-----|------|---------|
| `None` | 不重试 | 不可恢复的错误 |
| `Fixed { max_attempts, delay_ms }` | 固定间隔重试 | 设备繁忙、资源临时不可用 |
| `ExponentialBackoff { ... }` | 指数退避重试 | 网络错误、远程调用 |
| `Immediate { max_attempts }` | 立即重试 | 快速恢复的竞争条件 |

### 可重试的错误类型

以下错误类型默认支持重试：

- `CoreError::Concurrency` - 并发错误
- `MemoryError::MmuLockFailed` - MMU锁失败
- `DeviceError::Busy` - 设备繁忙
- `PlatformError::AcceleratorUnavailable` - 加速器临时不可用

---

## 错误收集和聚合

### ErrorCollector

使用 `ErrorCollector` 收集多个错误：

```rust
use vm_core::error::ErrorCollector;

let mut collector = ErrorCollector::new();

// 添加错误
collector.push(error1);
collector.push(error2);

// 添加结果，如果是错误的话
collector.push_result(some_function());

// 检查是否有错误
if collector.has_errors() {
    let error = collector.into_vm_error().unwrap();
    return Err(error);
}
```

### 多个错误

使用 `VmError::Multiple` 聚合多个错误：

```rust
let errors = vec![error1, error2, error3];
let combined_error = VmError::Multiple(errors);
```

---

## 错误日志

### ErrorLogger

使用 `ErrorLogger` 记录错误：

```rust
use vm_core::error::ErrorLogger;

// 记录错误
ErrorLogger::log_error(&error);

// 记录警告
ErrorLogger::log_warning(&error);

// 记录调试信息
ErrorLogger::log_debug(&error);
```

### 日志格式

```
ERROR VM Error: Memory error: Access violation at 0x80000000 (Write): Permission denied
  Caused by (1): Access violation
  Caused by (2): Page fault
```

---

## 最佳实践

### 1. 使用统一错误类型

```rust
// 推荐
fn execute(&mut self) -> Result<(), VmError> {
    // ...
}

// 不推荐
fn execute(&mut self) -> Result<(), MyCustomError> {
    // ...
}
```

### 2. 为错误添加上下文

```rust
// 推荐
mmu.read(pc, 8).context("Failed to fetch instruction")?;

// 不推荐
mmu.read(pc, 8)?
```

### 3. 使用适当的错误类型

```rust
// 推荐
return Err(VmError::Memory(MemoryError::AllocationFailed {
    message: "Out of memory".to_string(),
    size: Some(1024),
}));

// 不推荐
return Err(VmError::Io("Out of memory".to_string()));
```

### 4. 考虑错误恢复

```rust
// 推荐
retry_with_strategy(
    || device.write(offset, value, 4),
    ErrorRecoveryStrategy::Fixed { max_attempts: 3, delay_ms: 100 },
)?;

// 不推荐
device.write(offset, value, 4)?; // 失败就直接报错
```

### 5. 记录错误

```rust
// 推荐
if let Err(e) = operation() {
    ErrorLogger::log_error(&e);
    return Err(e);
}

// 不推荐
operation()?; // 静默失败
```

---

## 模块特定的错误处理

### JIT 引擎

JIT 引擎应使用 `vm_engine_jit::common::error` 中的类型：

```rust
use vm_engine_jit::common::error::{JITResult, JITErrorBuilder};

fn compile_block(&mut self, block: &IRBlock) -> JITResult<CompiledIRBlock> {
    if block.is_empty() {
        return Err(JITErrorBuilder::compilation("Empty block"));
    }
    // ...
}
```

### 跨架构翻译

跨架构翻译应使用 `TranslationError`，并转换为 `VmError`：

```rust
use vm_cross_arch::types::TranslationError;
use vm_core::error::VmError;

fn translate_instruction(&self, insn: &Instruction) -> Result<IRInstruction, VmError> {
    self.do_translate(insn)
        .map_err(|e| VmError::Core(CoreError::DecodeError {
            message: e.to_string(),
            position: Some(insn.address),
            module: "vm-cross-arch".to_string(),
        }))
}
```

---

## 测试错误处理

### 测试错误上下文

```rust
#[test]
fn test_error_context() {
    let error = MemoryError::AccessViolation { ... };
    let wrapped = VmError::WithContext {
        error: Box::new(VmError::Memory(error)),
        context: "Test context".to_string(),
        backtrace: None,
    };
    
    assert_eq!(wrapped.to_string(), "Test context: Memory error: ...");
}
```

### 测试错误恢复

```rust
#[test]
fn test_error_recovery() {
    let mut attempts = 0;
    let result = retry_with_strategy(
        || {
            attempts += 1;
            if attempts < 3 {
                Err(VmError::Device(DeviceError::Busy { ... }))
            } else {
                Ok(())
            }
        },
        ErrorRecoveryStrategy::Fixed { max_attempts: 3, delay_ms: 0 },
    );
    
    assert!(result.is_ok());
    assert_eq!(attempts, 3);
}
```

---

## 总结

统一的错误处理策略提供了：

1. **类型安全**：使用枚举而不是字符串错误
2. **错误上下文**：通过 `WithContext` 添加额外的错误信息
3. **错误恢复**：通过 `ErrorRecovery` trait 支持重试
4. **错误聚合**：通过 `ErrorCollector` 收集多个错误
5. **错误日志**：通过 `ErrorLogger` 记录错误

遵循本策略，可以确保整个虚拟机系统的错误处理一致、可维护和可调试。
