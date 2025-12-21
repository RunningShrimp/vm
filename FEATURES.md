# VM Project Features Guide

本文档说明 VM 项目中各个包的特性标志（features）及其用途。

## 特性标志分类

### 1. 核心特性（Core Features）

#### `std` / `no_std`
- **用途**: 控制是否使用标准库
- **默认**: `std`（大多数包）
- **使用场景**:
  - `std`: 标准环境，支持文件I/O、网络等
  - `no_std`: 嵌入式或受限环境

#### `async`
- **用途**: 启用异步运行时支持
- **依赖**: `tokio`, `futures`, `async-trait`
- **使用场景**: 需要异步执行时启用

### 2. 运行时特性（Runtime Features）

#### `tokio` / `async-std` / `smol`
- **用途**: 选择异步运行时
- **默认**: `tokio`
- **使用场景**:
  - `tokio`: 生产环境推荐
  - `async-std`: 替代运行时
  - `smol`: 轻量级运行时

#### `all-runtimes`
- **用途**: 启用所有运行时支持（用于测试和兼容性）

### 3. 编译后端特性（Backend Features）

#### `cranelift-backend`
- **用途**: 使用 Cranelift 作为 JIT 编译后端
- **默认**: 在 `vm-engine-jit` 中启用
- **依赖**: `cranelift`, `cranelift-module`, `cranelift-jit`, `cranelift-native`

#### `llvm-backend`
- **用途**: 使用 LLVM 作为编译后端（实验性）
- **依赖**: `vm-ir/llvm`

#### `direct-backend`
- **用途**: 直接代码生成后端（用于 `vm-codegen`）

### 4. 内存管理特性（Memory Features）

#### `tlb-basic` / `tlb-optimized` / `tlb-concurrent`
- **用途**: TLB（转换后备缓冲区）实现级别
- **默认**: `tlb-basic`
- **互斥性**: 这些特性是互斥的，只能启用其中一个
- **使用场景**:
  - `tlb-basic`: 基础实现，适用于单线程或低并发场景
  - `tlb-optimized`: 优化实现（多级TLB），适用于高性能场景，支持多级缓存
  - `tlb-concurrent`: 并发优化实现，适用于高并发场景，使用无锁数据结构
- **性能对比**:
  - `tlb-basic`: 最低开销，基础功能
  - `tlb-optimized`: 中等开销，更好的缓存命中率
  - `tlb-concurrent`: 较高开销，最佳并发性能

#### `memmap`
- **用途**: 启用内存映射支持
- **依赖**: `memmap2`

### 5. 增强功能特性（Enhanced Features）

#### `enhanced-event-sourcing`
- **用途**: 启用增强的事件溯源功能
- **依赖**: `sqlx`, `serde_json`, `chrono`

#### `enhanced-debugging`
- **用途**: 启用增强的调试功能
- **功能**: 
  - 符号表支持（SymbolTable）
  - 调用栈跟踪（CallStackTracker）
  - 增强断点功能（EnhancedBreakpoints）
  - 统一调试器接口（UnifiedDebugger）
- **使用场景**: 开发和调试时启用，提供更详细的调试信息

#### `monitoring`
- **用途**: 启用性能监控
- **依赖**: `vm-monitor`

#### `experimental`
- **用途**: 启用实验性功能
- **使用场景**: 测试新功能时使用

## 包特定特性

### vm-core
- `std` / `no_std`: 标准库支持
- `async`: 异步支持
- `enhanced-event-sourcing`: 事件溯源
- `enhanced-debugging`: 增强调试功能（符号表、调用栈跟踪、增强断点）

### vm-runtime
- `tokio` / `async-std` / `smol`: 运行时选择
- `all-runtimes`: 所有运行时
- `monitoring`: 性能监控

### vm-mem
- `std` / `no_std`: 标准库支持
- `async`: 异步支持
- `memmap`: 内存映射
- `tlb-basic` / `tlb-optimized` / `tlb-concurrent`: TLB 实现

### vm-engine-jit
- `cranelift-backend`: Cranelift 后端（默认）
- `llvm-backend`: LLVM 后端
- `experimental`: 实验性功能

### vm-codegen
- `direct-backend`: 直接后端（默认）
- `cranelift-backend`: Cranelift 后端
- `llvm-backend`: LLVM 后端

## 推荐配置

### 生产环境
```toml
[dependencies]
vm-core = { path = "../vm-core", features = ["std", "async"] }
vm-runtime = { path = "../vm-runtime", features = ["tokio", "monitoring"] }
vm-mem = { path = "../vm-mem", features = ["std", "tlb-optimized"] }
vm-engine-jit = { path = "../vm-engine-jit", features = ["cranelift-backend"] }
```

### 开发环境
```toml
[dependencies]
vm-core = { path = "../vm-core", features = ["std", "async", "enhanced-debugging"] }
vm-runtime = { path = "../vm-runtime", features = ["tokio"] }
vm-mem = { path = "../vm-mem", features = ["std", "tlb-basic"] }
vm-engine-jit = { path = "../vm-engine-jit", features = ["cranelift-backend", "experimental"] }
```

### 嵌入式环境
```toml
[dependencies]
vm-core = { path = "../vm-core", features = ["no_std"] }
vm-mem = { path = "../vm-mem", features = ["no_std", "tlb-basic"] }
```

## 特性标志优化建议

1. **统一命名**: 使用一致的命名约定（如 `std`/`no_std`）
2. **避免重复**: 合并重复的特性（如 `vm-runtime` 中的 `std` 和 `tokio`）
3. **文档化**: 为每个特性添加清晰的文档说明
4. **默认值**: 设置合理的默认特性组合
5. **移除空特性**: 定期检查并移除未使用的空特性标志

## 已优化的特性标志

### 移除的空特性
- `enhanced-networking`: 已移除（未实现，无实际代码）

### 更新的特性文档
- `enhanced-debugging`: 更新文档，说明实际功能（符号表、调用栈跟踪、增强断点）
- TLB 特性标志: 添加互斥性说明和性能对比信息

## 迁移指南

### 从独立包迁移到合并模块

如果之前使用了独立的包（如 `async-executor`），现在应该使用合并后的模块：

```rust
// 旧方式
use async_executor::AsyncExecutionContext;

// 新方式
use vm_runtime::async_executor::AsyncExecutionContext;
```

特性标志的使用方式保持不变。

