# Rust 虚拟机实施计划：性能优化与架构重构

基于全面的架构审查，本计划旨在解决性能瓶颈、提高代码可维护性，并确保符合 DDD 贫血模型原则。

## 阶段 1：架构重构与 DDD 合规化 (核心优先)

**目标**: 将业务逻辑从 CLI 层下沉至领域服务层，分离状态与行为。

1.  **定义领域服务 (`VmService`)**
    - 在 `vm-core` 或新建 `vm-service` 模块中创建 `VmRunner` 或 `VmExecutionService`。
    - 职责：管理 VM 生命周期（启动、暂停、停止），封装执行循环。
    - 将 `vm-cli/src/main.rs` 中的指令解码、执行循环逻辑移动到该服务中。

2.  **分离状态模型**
    - 确保 `VirtualMachine` 结构体主要作为聚合根持有状态（配置、设备、内存）。
    - 运行时状态（如当前 PC、寄存器缓存）应由 `ExecutionEngine` 或 `VcpuContext` 管理，而不是混杂在静态配置中。

## 阶段 2：性能优化 - 消除锁竞争 (关键路径)

**目标**: 解除主执行循环对全局 MMU 锁的依赖，实现并行 vCPU 执行。

1.  **引入线程本地 TLB (Thread-Local TLB)**
    - 修改 `ExecutionEngine` 接口，使其支持持有本地缓存。
    - 在每个 vCPU 线程中维护一个私有的 TLB (L1 Cache)。
    - 仅在 TLB Miss 或 MMIO 访问时才请求全局 MMU 锁/总线访问。

2.  **vCPU 线程化**
    - 使用 `std::thread` 为每个 vCPU 创建独立线程。
    - 实现 vCPU 线程与主控制线程（CLI/API）之间的通信机制（如 `crossbeam-channel` 或 `std::sync::mpsc`）用于控制信号（暂停、停止）。

3.  **锁粒度优化**
    - 将 MMU 的 `Mutex` 替换为 `RwLock`，允许多个 vCPU 并发读取内存（取指、读数据）。
    - 仅在内存写入或设备状态变更时使用写锁。

## 阶段 3：异步 I/O 与并发模型

**目标**: 利用 Rust 的异步生态处理 I/O 密集型任务，避免阻塞 CPU 模拟。

1.  **异步设备后端**
    - 确保 `vm-device` 中的块设备和网络设备支持 `async` 接口。
    - 在独立于 vCPU 的线程（Tokio Runtime）中运行 I/O 任务。

2.  **I/O 请求队列**
    - vCPU 线程遇到 I/O 操作时，将请求推入队列（Channel），然后挂起或自旋等待（取决于模拟模型），而不阻塞整个 VM 进程。

## 阶段 4：功能完善与验证

1.  **JIT 集成**
    - 在新的 `VmService` 架构下，确保 JIT 引擎（`vm-engine-jit`）被正确初始化和调用。
    - 验证热点代码检测和编译流程在多线程环境下的线程安全性。

2.  **基准测试**
    - 建立性能基线（当前解释器模式的 IPS - Instructions Per Second）。
    - 验证优化后的性能提升。

## 实施路线图

1.  **Step 1**: 重构 `vm-cli`，提取 `VmServer` / `ExecutionLoop`。
2.  **Step 2**: 优化 MMU 访问，实现 `TLB` 缓存。
3.  **Step 3**: 实现多线程 vCPU 调度器。
4.  **Step 4**: 验证并集成 JIT。
