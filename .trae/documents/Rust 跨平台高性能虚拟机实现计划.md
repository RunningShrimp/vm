## 项目目标与范围
- 实现基于 Rust 的高性能跨架构虚拟机，支持 ARM64、x86_64、RISC‑V64 作为 Guest，在 Windows、Linux、macOS、HarmonyOS 作为 Host 高效运行。
- 提供解释执行与 JIT（Cranelift）两条路径，同时集成硬件虚拟化（KVM/HVF/WHV）和 SIMD 加速（AVX/NEON/V 扩展）。
- 架构模块化、可插拔，核心库尽量 `no_std` 兼容，设备以 VirtIO 为主，具备完善测试与基准体系。

## Crate 与目录结构
- `vm-core`：核心类型、错误、配置、日志接口；`no_std` 可选。
- `vm-ir`：IR 定义、构建器、优化通道（常量折叠、死代码消除等）。
- `vm-frontend-*`：各 ISA 解码器（`arm64`、`x86_64`、`riscv64`）。
- `vm-engine-interpreter`：IR 解释执行器与调度器。
- `vm-engine-jit`：Cranelift 集成、代码缓存、块链接、轨迹 JIT。
- `vm-accel`：SIMD 抽象层、运行时特征检测、硬件虚拟化桥接（KVM/HVF/WHV）。
- `vm-mem`：物理内存模拟、MMU、TLB、页表遍历、异常与缺页处理。
- `vm-device`：VirtIO 设备（net/blk/console）、PCI/MMIO 传输与中断框架。
- `vm-osal`：操作系统抽象（内存映射、线程绑定、信号/异常、计时与文件）。
- `vm-cli`：命令行入口、配置加载、镜像启动与控制台。
- `vm-tests`：单元/集成/兼容性/基准测试代码与驱动脚本。

## 核心抽象与接口
- `Decoder`：`fn decode(&mut, mmu, pc) -> IRBlock`；按架构实现。
- `IRBuilder`：面向微指令（micro‑ops）构建与 SSA 化（可选）。
- `ExecutionEngine`：`run(block)`、`link(next)`；解释器与 JIT 共享接口。
- `MMU`：`translate(va, access)`、`fetch_insn(pc)`、TLB 维护与页表遍历。
- `Device`：`reset/queue/irq/read/write`；VirtIO 统一设备 Trait。
- `OSMemory/OSThread/OSSignal`：OSAL 统一接口（mmap/VirtualAlloc、线程绑定、SIGSEGV）。

## IR 设计
- 面向架构无关的精简微指令：算术/逻辑、访存、控制流、SIMD、系统指令。
- 引入寄存器文件抽象：Guest 寄存器映射到 IR 虚拟寄存器；可选择 SSA 以利于 JIT 优化。
- 控制流：`Block`、`Terminator`（`jmp/cond_jmp/call/ret`）；支持异常边（fault/interrupt）。
- 内存语义：显式 `Load/Store`，带原子/对齐/宽度标志；页故障作为显式结果。

## 指令解码器（Frontend）
- 为 ARM64/x86_64/RISC‑V64 分别实现 `Decoder`，输出统一 IR。
- 采用表驱动与位域解析结合（性能与可维护性平衡），分离解码与语义映射。
- 指令类别逐步覆盖：基础 ALU → 访存/分支 → SIMD → 特权/系统。

## 执行引擎（Interpreter/JIT）
- 解释器：紧凑调度器驱动 IR 执行，优先用于冷路径与调试。
- JIT：基于 Cranelift 的后端，将 `IRBlock/Trace` 映射为 Host 机器码；维护代码缓存与跳转表。
- W^X 管控：代码页写入‑执行权限切换；HarmonyOS/Android NDK 适配可执行页策略。

## 动态二进制翻译与热点探测
- 基于轨迹（Trace）收集：以 `PC` 及分支行为构成 trace，记录计数器与边权重。
- 热点判定：门限 + 回边频次；冷热分离（解释器/编译器切换）。
- 块链接：编译块尾部直接跳转下一个块（direct threading），绕过调度器。
- 反优化：回退策略（mis‑speculation/自修改代码检测时撤销）。

## 硬件加速与虚拟化集成
- SIMD：`std::arch` + 运行时特征检测（`raw-cpuid/sysctl`）；为常见向量 IR 提供 AVX2/AVX‑512/NEON/V 扩展路径。
- 虚拟化：同架构时桥接 KVM（`kvm-ioctls`）、macOS HVF（`xhypervisor`/Hypervisor.framework）、Windows WHV；选择性直通 vCPU 执行。
- 能力协商：启动时枚举 Host 特性，选择最佳执行策略（DBT/JIT/虚拟化/混合）。

## 内存与 MMU/TLB
- 物理内存：`memmap2` 映射大块宿主内存，建立 Guest 物理空间模拟。
- MMU：SoftMMU + TLB（多级、LRU；区分指令/数据 TLB）；页表遍历支持三架构风格。
- 异常与缺页：统一 `PageFault/AccessViolation`；信号/异常与 OSAL 对接。
- 地址空间优化：64 位 Host 上以直映策略减少查表层次；跨平台处理对齐与权限。

## 设备模型与 VirtIO
- 设备框架：VirtIO 基类 + 传输（PCI/MMIO）与队列（VRing）解析。
- 设备实现：`virtio‑blk`、`virtio‑net`、`virtio‑console` 优先；中断路由与 MSI/MSI‑X 支持。
- I/O 性能：批处理提交、零拷贝缓冲、宿主异步 I/O 聚合。

## 跨平台 OS 抽象层（OSAL）
- 内存：封装 `mmap`/`VirtualAlloc`；统一 `protect/commit/decommit`。
- 线程：`std::thread` + 绑核（平台特定 API）；计时器与高精度时钟。
- 信号/异常：统一捕获 `SIGSEGV`/结构化异常；用于 MMU 缺页与非法访问处理。
- HarmonyOS：复用 Linux 配置；NDK 适配可执行代码页；如需直通设备，探索 HDF 接口。

## 错误处理、安全与隔离
- 严格区分 Guest/Host 权限边界；不泄露宿主资源。
- W^X 全局策略与最小权限；输入验证与边界检查。
- 自修改代码、非法内存访问与设备异常的健壮处理。

## 性能优化策略
- 块链接、直接线程化调度器；减少调度开销。
- 寄存器分配：优先将 Guest 寄存器映射到 Host 实体寄存器；溢出时轻量 spill。
- 热路径内联与短路；访存聚合与对齐优化。
- 代码缓存管理：分区 + 淘汰；编译时间与运行时间平衡。

## 观测与调试
- 事件与计数器：指令数、缺页、TLB 命中率、编译时间/运行时间。
- 跟踪：块/trace 级日志（可动态开启）；最小化热路径开销。
- 调试接口：单步、断点、寄存器/内存快照。

## 构建与特性开关（feature flags）
- `guest-{arm64,x86_64,riscv64}`、`host-{linux,macos,windows,harmony}`。
- `accel-{kvm,hvf,whv}`、`simd-{avx2,avx512,neon,rvv}`、`no_std`。
- 通过 Cargo features 选择组合；在 `vm-cli` 中暴露运行时能力检测与降级提示。

## 测试与验证方案
- 单元测试：解码器指令覆盖、IR 语义一致性、MMU/TLB 行为、设备 VRing 处理。
- 兼容性测试：`riscv-tests`、`kvm-unit-tests`；跨架构指令子集验证。
- 集成测试：自动启动 Alpine/BusyBox 镜像（ARM/x86/RISCV），执行 shell 与 I/O 场景。
- 基准测试：CoreMark、SPEC CPU 2017（许可条件下）对比 QEMU/原生；采集 CWV 类性能指标（编译/执行比）。

## 交付物与 CLI 使用
- `vm-cli` 支持：加载镜像/内核、选择 Guest/Host、启用 JIT/虚拟化/SIMD、设备配置（`--virtio-net/blk/console`）。
- 输出：性能统计、trace 摘要、错误与异常报告；可选 JSON/文本。

## 增量迭代路径
- 最小可用：实现 RISC‑V 基础 ALU + 访存 + 分支的解释器 + SoftMMU。
- 扩展：加入 JIT（基础块）、TLB、VirtIO‑console；再扩展到 ARM64/x86_64 解码器。
- 高阶：trace‑JIT、SIMD 加速、VirtIO‑net/blk、高级寄存器分配与块链接。

## 风险与应对
- 不同 ISA 语义差异：以 IR 严格定义语义与边界；针对特权/内存序列化单独验证。
- W^X 与可执行页：平台特定 API 适配与降级方案；必要时分离 JIT 进程。
- 性能不可预期：建立可观测性与二分法基准，快速定位热点与退化路径。
- 兼容性：以测试矩阵覆盖主流 OS/架构；保持设备模型与协议版本兼容。
