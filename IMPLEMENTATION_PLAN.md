# FVP 实施计划 (Implementation Plan)

基于 [全面审查报告](./COMPREHENSIVE_REVIEW_REPORT.md) 的建议，制定以下分阶段实施计划。

## 阶段 1: 性能速赢与稳定性 (第 1-2 周)

本阶段专注于低风险、高回报的性能优化和测试覆盖率提升。

### 1.1 TLB 查找优化 (H1)
- **目标**: 将 TLB 查找复杂度从 O(n) 降低到 O(1)。
- **涉及模块**: `vm-mem`
- **文件**: `vm-mem/src/lib.rs`
- **行动**:
    1.  引入 `std::collections::HashMap` 或实现多路组相联 (Set-Associative) 缓存结构。
    2.  重构 `lookup` 方法以使用新的数据结构。
    3.  保留 LRU 替换策略。

### 1.2 批量内存操作 (H4)
- **目标**: 消除大块内存加载时的逐字节拷贝开销。
- **涉及模块**: `vm-core`
- **文件**: `vm-core/src/lib.rs`
- **行动**:
    1.  在 `MMU` trait 中添加 `write_bulk` 和 `read_bulk` 默认方法。
    2.  在 `SoftMmu` 中实现高效的 slice copy。
    3.  更新 `VirtualMachine::load_kernel` 等处调用新 API。

### 1.3 提升测试覆盖率 (M4)
- **目标**: 建立端到端 (E2E) 测试基准，防止回归。
- **涉及模块**: `vm-tests`
- **文件**: `vm-tests/tests/end_to_end.rs`
- **行动**:
    1.  创建完整的 VM 启动生命周期测试 (`boot` -> `run` -> `shutdown`)。
    2.  添加针对不同架构 (RISC-V, ARM64, x86_64) 的基础指令测试用例。

---

## 阶段 2: 核心架构改进 (第 2-4 周)

本阶段涉及核心执行路径和 I/O 模型的重构。

### 2.1 异步 I/O 默认化 (H3)
- **目标**: 解决同步 I/O 阻塞 VM 执行线程的问题。
- **涉及模块**: `vm-device`, `vm-cli`
- **文件**: `vm-device/src/block.rs`, `vm-cli/src/main.rs`
- **行动**:
    1.  将 `vm-device` 中的异步实现 (`block_async.rs`) 提升为默认实现。
    2.  在 `vm-cli` 中引入 `tokio` 运行时。
    3.  重构主循环为协程模型，分离计算与 I/O 任务。

### 2.2 JIT 浮点支持 (H2)
- **目标**: 完善 JIT 编译器，支持浮点密集型工作负载。
- **涉及模块**: `vm-engine-jit`
- **文件**: `vm-engine-jit/src/lib.rs`
- **行动**:
    1.  实现 `IROp::Fadd`, `IROp::Fsub`, `IROp::Fmul`, `IROp::Fdiv` 的 Cranelift 映射。
    2.  添加浮点寄存器状态管理。

### 2.3 MMU 并发优化 (M1)
- **目标**: 减少多 vCPU 场景下的锁竞争。
- **涉及模块**: `vm-core`
- **文件**: `vm-core/src/lib.rs`
- **行动**:
    1.  将 `VirtualMachine` 中的 `Mutex<Box<dyn MMU>>` 替换为 `RwLock<Box<dyn MMU>>`。
    2.  审查并优化所有获取写锁的路径。

---

## 阶段 3: 高级优化与功能完善 (第 4-8 周)

本阶段关注深度优化和代码质量提升。

### 3.1 向量 SIMD 优化 (M2)
- **目标**: 利用宿主机 SIMD 指令加速 Guest 向量运算。
- **涉及模块**: `vm-engine-jit`
- **文件**: `vm-engine-jit/src/lib.rs`
- **行动**:
    1.  识别 `VecAdd`, `VecMul` 等操作。
    2.  使用 Cranelift 的 SIMD 指令集进行映射，而非标量循环模拟。

### 3.2 解释器跳转表优化 (L1)
- **目标**: 提升解释器执行效率。
- **涉及模块**: `vm-engine-interpreter`
- **文件**: `vm-engine-interpreter/src/lib.rs`
- **行动**:
    1.  评估将巨型 `match` 语句重构为函数指针数组或 computed goto (如果 Rust 版本/unsafe 策略允许) 的收益。

### 3.3 错误处理增强 (M3)
- **目标**: 提供更丰富的调试信息。
- **涉及模块**: `vm-core`
- **文件**: `vm-core/src/lib.rs`
- **行动**:
    1.  重构 `VmError`，使用 `source` 字段保留底层错误链。
    2.  确保所有错误传播路径都不丢失上下文信息。
