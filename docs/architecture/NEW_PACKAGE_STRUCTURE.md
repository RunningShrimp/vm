# VM 项目包结构 (2025-12-27 更新)

## 当前包列表 (38个)

### 核心包
- **vm-common** - 公共类型和工具
- **vm-core** - 核心虚拟机功能
- **vm-interface** - 接口定义
- **vm-ir** - 中间表示
- **vm-mem** - 内存管理

### 执行引擎
- **vm-engine-jit** - JIT编译执行引擎
- **vm-engine-interpreter** - 解释执行引擎

### 前端解码器
- **vm-frontend** - 统一前端解码器 (x86_64/ARM64/RISC-V)

### 优化器
- **vm-optimizers** - 统一优化器框架 (GC/内存/PGO/ML)

### 执行器
- **vm-executors** - 统一执行器框架 (异步/协程/分布式)

### 设备和I/O
- **vm-device** - 设备模拟
- **vm-accel** - 硬件加速 (KVM/HVF/Whpx)
- **vm-passthrough** - 设备直通
- **vm-smmu** - ARM SMMU支持

### 平台和运行时
- **vm-platform** - 平台抽象
- **vm-runtime** - 运行时服务
- **vm-boot** - 启动加载

### 基础设施
- **vm-foundation** - 基础类型和工具 (错误/验证/资源)
- **vm-cross-arch-support** - 跨架构支持
- **vm-cross-arch** - 跨架构翻译

### 工具和测试
- **vm-service** - VM服务接口
- **vm-monitor** - 监控和性能分析
- **vm-cli** - 命令行工具
- **vm-plugin** - 插件系统
- **vm-codegen** - 代码生成
- **vm-tests** - 测试套件

### SIMD和加速
- **vm-simd** - SIMD指令支持

### 其他
- **vm-osal** - 操作系统抽象层
- **vm-gpu** - GPU支持
- **vm-debug** - 调试支持
- **vm-adaptive** - 自适应优化
- **vm-desktop** - 桌面应用

### 性能测试
- **vm-perf-regression-detector** - 性能回归检测
- **vm-stress-test-runner** - 压力测试
- **perf-bench** - 性能基准

### 编译器
- **tiered-compiler** - 分层编译
- **parallel-jit** - 并行JIT

### 安全和兼容
- **security-sandbox** - 安全沙箱
- **syscall-compat** - 系统调用兼容

### 集成测试
- **vm-cross-arch-integration-tests** - 跨架构集成测试

---

## 新增的统一包 (5个)

### vm-foundation
合并了4个基础包:
- vm-error
- vm-validation
- vm-resource
- vm-support

### vm-cross-arch-support
合并了5个跨架构支持包:
- vm-encoding
- vm-register
- vm-memory-access
- vm-instruction-patterns
- vm-optimization

### vm-optimizers
合并了4个优化器包:
- gc-optimizer
- memory-optimizer
- pgo-optimizer
- ml-guided-compiler

### vm-executors
合并了3个执行器包:
- async-executor
- coroutine-scheduler
- distributed-executor

### vm-frontend
合并了3个架构前端包:
- vm-frontend-x86_64
- vm-frontend-arm64
- vm-frontend-riscv64

---

## 使用示例

### 使用 vm-foundation
```rust
use vm_foundation::{VmResult, VmError, Architecture};
```

### 使用 vm-optimizers
```rust
use vm_optimizers::gc::{OptimizedGc, GcResult};
use vm_optimizers::memory::MemoryOptimizer;
```

### 使用 vm-executors
```rust
use vm_executors::async_executor::JitExecutor;
use vm_executors::coroutine::CoroutineScheduler;
```

### 使用 vm-frontend
```toml
# Cargo.toml
[dependencies]
vm-frontend = { version = "0.1", features = ["all"] }
```

```rust
// 使用特定架构
use vm_frontend::x86_64::X86Decoder;
use vm_frontend::arm64::Arm64Decoder;
use vm_frontend::riscv64::RiscvDecoder;
```

---

## 特性支持

### vm-frontend features
- `x86_64` - 仅启用x86-64支持
- `arm64` - 仅启用ARM64支持
- `riscv64` - 仅启用RISC-V支持
- `all` - 启用所有架构支持

### vm-executors features
- 默认启用所有执行器类型

### vm-optimizers features
- 默认包含所有优化器

---

## 迁移指南

### 从旧包迁移到新包

| 旧包 | 新包 | 迁移步骤 |
|------|------|----------|
| vm-error | vm-foundation | `use vm_foundation::` 替代 `use vm_error::` |
| gc-optimizer | vm-optimizers | `use vm_optimizers::gc::` 替代 `use gc_optimizer::` |
| async-executor | vm-executors | `use vm_executors::async_executor::` |
| vm-frontend-x86_64 | vm-frontend | `use vm_frontend::x86_64::` |

---

## 编译验证

```bash
# 验证所有包编译
cargo build --workspace --lib

# 验证特定包
cargo build -p vm-foundation
cargo build -p vm-optimizers
cargo build -p vm-executors
cargo build -p vm-frontend --features all
```

---

**最后更新**: 2025-12-27
**状态**: ✅ 所有包编译通过
