# 实施计划与 TodoList V2（基于“审查报告 + 代码证据 + 既有计划”）

> 生成日期：2025-12-13
>
> 本文件目标：在不遗漏既有信息的前提下，把“审查报告结论 + 代码证据 + 现有实施计划”整合为一份可执行、可验收、可排期的实施计划与全量 TodoList。
>
> **不遗漏声明**：
> 1) 本文件包含对 [docs/IMPLEMENTATION_PLAN_AND_TODOS.md] 的**全文镜像**（见第 4 节）。
> 2) 本文件包含本次对关键链路（跨 ISA / JIT / AOT / GC / 硬件加速 / syscall）代码审查形成的**增补证据与闭环验收要求**（见第 2、3、5 节）。
> 3) 本文件包含本轮输出的“全面审查报告”**全文引用**（见附录 A），以保证“审查报告信息不丢失”。

---

## 1. 总体目标与约束（保持不变）

### 1.1 目标

- 让工程具备**可编译、可测试、可回归**的基线（P0）。
- 明确“跨 ISA 用户态仿真 vs 全系统仿真”路线并形成 ADR（P1）。
- 打通 JIT/AOT 真执行闭环：可执行内存 + 真实调用 + 一致性回归（P1/P2）。
- 打通硬件加速闭环（同 ISA 优先），并具备可回退机制（P2/P3）。
- 完成 GC 与堆模型闭环（P2/P3）。
- 提升可维护性：模块收敛、文档一致性、告警治理、发布策略（持续）。

### 1.2 约束（来自需求与现状）

- 需求约束：虚拟机需支持 AMD64、ARM64、RISC-V64 **两两互运行**；并集成高级特性加速（硬件虚拟化扩展、指令集优化等）。
- 事实约束（来自代码证据）：目前关键执行链路存在“回退解释器/占位实现/缓存非可执行代码”等情况（详见第 2 节）。
- 交付约束：所有对外声明必须可证据化（测试/示例/基准/ADR）。

---

## 2. 代码证据增补（用于驱动验收标准，不改变需求）

> 目的：把“审查结论”具体绑定到可检索的代码事实，避免实施计划仅停留在抽象层。

### 2.1 跨 ISA 执行链路现状（证据）

| 主题 | 代码证据（文件） | 现状结论 | 直接影响 |
|---|---|---|---|
| AutoExecutor 的 JIT/Hybrid | vm-cross-arch/src/auto_executor.rs | `ExecMode::Jit | Hybrid` 分支明确回退解释器 | JIT 不可被真实触发，性能与正确性不可验证 |
| AutoExecutor 的 Accelerated | vm-cross-arch/src/auto_executor.rs | 同 ISA 也回退解释器（标注“需要实现硬件加速引擎”） | 硬件加速不闭环 |
| UnifiedExecutor 的 AOT/JIT 执行 | vm-cross-arch/src/unified_executor.rs | AOT/JIT 缓存存 `Vec<u8>`，执行仍 `runtime.execute_block(pc)` | “命中缓存”不等于“直接执行 host code” |
| CrossArchRuntime 的 JIT 编译结果 | vm-cross-arch/src/cross_arch_runtime.rs | `compile_ir_block` 返回“编译成功标记”而非机器码；并写入 jit_cache | JIT 编译与执行语义不一致 |
| syscall 默认行为 | vm-cross-arch/src/os_support.rs | 未处理 syscall `Ok(0)` | 可能产生“假成功/静默错误”，破坏正确性与可观测性 |

### 2.2 JIT 子系统形态不一致（证据）

- vm-engine-jit 同时存在：
  - `core.rs` 中较完整的 `JITEngine` 架构（并行编译、缓存、优化器、调度器等）。
  - `lib.rs` 中占位式 `Jit`（当前 `compile()` 生成 NOP 序列，`run()` 不做真实执行）。
- cross-arch runtime 当前集成路径主要使用 `vm_engine_jit::Jit`（见 vm-cross-arch/src/cross_arch_runtime.rs）。

**结论**：在闭环未完成前，必须先统一“哪一套 JIT 是产品路径”，否则会出现接口存在但执行回退的系统性风险。

### 2.3 硬件加速 feature 与集成现状（证据）

- vm-accel/Cargo.toml 中 `kvm` feature 当前被注释为“Temporarily disabled”，但代码中存在 `kvm_impl.rs` 并带 `#[cfg(feature = "kvm")]`。
- 跨架构执行链路（AutoExecutor/UnifiedExecutor/CrossArchRuntime）未看到把 vm-accel 的 vCPU run/exit 与 vm-core/MMU/设备模型串联的可验证入口。

**结论**：优先做“同 ISA hardware acceleration 闭环（smoke + 回退）”，跨 ISA 不应依赖 VT-x/AMD-V 解决（除非明确引入嵌套虚拟化路线）。

### 2.4 GC 集成现状（证据）

- vm-boot/src/gc_runtime.rs 为简化版/占位式 GC runtime，主要基于 cache 统计触发。
- gc-optimizer 为独立优化库，提供写屏障/并行标记/配额等结构，但与 VM 对象模型、根集合、safepoint 未形成闭环。

**结论**：需要把“对象模型/根集合/写屏障插入点/safepoint 语义”作为 GC 闭环验收前置。

---

## 3. 实施路线图（V2 版本，保持原阶段划分，补充“闭环验收门槛”）

> 说明：阶段 A-H 与原计划保持一致；V2 增补的是每阶段“必须满足的闭环门槛”与“证据化要求”。

### 阶段 A（P0）：恢复可编译 + 最小测试闭环

- 门槛：
  - `cargo test -p vm-core` 必须可通过（不只是 `cargo build`）。
  - `cargo test -p vm-cross-arch` 至少有 1 个 smoke test 覆盖 decode→execute 的基本路径。
  - Rust 2024 关键 unsafe 告警策略明确（允许/禁止范围写入文档）。

### 阶段 B（P1）：路线决策 + 能力矩阵（文档与事实一致）

- 门槛：
  - ADR 明确跨 ISA 范围（用户态 vs 全系统），并把“哪些能力在 roadmap”写清楚。
  - 对外文档每条能力声明都必须链接到：测试/示例/基准/ADR。

### 阶段 C（P1/P2）：JIT/AOT 真执行闭环

- 门槛：
  - 缓存形态必须升级为“可执行代码句柄”（而不是 Vec<u8>）。
  - AOT/JIT 命中必须走“直接调用 host code”路径；禁止继续 `execute_block` 方式伪装。
  - 必须有差分测试证明：解释器 vs JIT/AOT 在一组 IRBlock/机器码样例上结果一致。

### 阶段 D（P2）：跨 ISA 目标补齐（按路线二选一推进）

- 门槛：
  - 用户态仿真：syscall 未实现必须返回 ENOSYS/错误（不可 `Ok(0)`）。
  - 全系统仿真：最小引导/中断/MMIO/特权级/页表必须形成可复现的 early boot 里程碑。

### 阶段 E（P2/P3）：性能、缓存与语义完善

- 门槛：
  - 指标必须来自真实采样（trace/metrics），不能是模拟估算。
  - 基准必须能在 CI 或可复现环境运行，输出稳定。

### 阶段 F（P2/P3）：硬件加速闭环（同 ISA）与回退机制

- 门槛：
  - 至少一个平台（Linux/KVM 或 macOS/HVF）可跑通最小 guest 进入 vCPU run 循环并处理 exit。
  - 必须有回退机制用例（触发回退后状态一致）。

### 阶段 G（P2/P3）：GC 与堆模型闭环

- 门槛：
  - 明确并实现对象模型/根集合/写屏障/safepoint。
  - 至少一个“对象分配+回收”回归测试，且可在 CI 稳定运行。

### 阶段 H（持续）：工程化、收敛、DDD、可观测、发布

- 门槛：
  - 单入口 API：examples 与 service 层只走一个公开入口。
  - 错误与可观测性统一：关键路径必须有 tracing span + 指标。
  - 发布策略可复现：最小 feature 集可跑。

---

## 4. 既有实施计划与 TodoList（全文镜像，不做删改）

> 来源：docs/IMPLEMENTATION_PLAN_AND_TODOS.md（原文全文复制）。

```markdown
# 实施计划与 TodoList（基于审查报告）

> 生成日期：2025-12-12/13
> 
> 目标：把审查报告中所有发现（编译阻断、JIT/AOT闭环缺失、跨ISA路线不清、硬件加速未闭环、OS/syscall覆盖不足、GC未集成、文档宣称与事实偏差、DDD贫血模型合规性风险、可维护性/告警治理等）拆解为可执行、可验收、可排期的工作项。

---

## 1. 总体原则（不改需求前提）

- **先可编译可测试（P0）**：没有稳定构建/测试基线，任何性能/跨架构/加速结论都不可信。
- **先路线后补齐（P1）**：必须明确“跨 ISA 用户态仿真 vs 全系统仿真”的产品路线；否则 syscall/设备/中断/引导/特权级等投入会走偏。
- **以闭环为验收**：JIT/AOT/硬件加速/GC 都必须达到“可运行、可观测、可回归”的闭环，而不是仅有结构或占位实现。
- **声明必须可证据化**：README/对外文档能力声明必须能对应到测试/示例/基准或明确标记为 roadmap。

---

## 2. 分阶段实施计划（按依赖顺序）

### 阶段 A（P0）：恢复可编译 + 最小测试闭环

**目的**：解除当前阻断（vm-core 编译失败），让跨架构/性能验证具备基础条件。

**交付物**：
- `cargo test -p vm-core` 可通过
- `cargo test -p vm-cross-arch` 可编译并可运行（至少 smoke）
- Rust 2024 关键 unsafe 告警策略明确并落地

对应 todo：#1 #2 #3 #4

### 阶段 B（P1）：路线决策 + 能力矩阵（文档与事实一致）

**目的**：把“宣称能力”改造成“可验证能力”；并明确跨 ISA 的产品边界。

**交付物**：
- ADR：跨 ISA 范围（用户态/全系统）
- 能力矩阵 docs/capability-matrix.md
- Host×Guest 支持矩阵 docs/cross-arch-support.md
- 执行入口收敛（减少多入口分叉）

对应 todo：#5 #6 #7 #8

### 阶段 C（P1/P2）：JIT/AOT 真执行闭环（不再“模拟/回退”）

**目的**：把“存在 pipeline 但执行模拟/回退解释器”的现状，升级为“可执行内存 + 真实调用 + 一致性回归”。

**交付物**：
- 可执行内存管理（W^X / icache flush / 异常策略）
- JIT `execute_compiled_code` 真执行
- cache 形态从字节数组升级为可执行代码句柄
- UnifiedExecutor 的 AOT/JIT 分支可触发且可观测

对应 todo：#9 #10 #11 #12 #13 #14 #15

### 阶段 D（P2）：跨 ISA 目标补齐（按路线二选一推进）

- **用户态仿真路线**：优先扩 syscall 兼容（open/read/mmap/futex等），形成最小用户态程序闭环。
- **全系统仿真路线**：定义并实现最小引导/中断/MMIO/特权级/页表集合，目标是到 early boot 日志。

对应 todo：#16 #17（互斥优先级取决于 ADR）

### 阶段 E（P2/P3）：性能、缓存与语义完善

**目的**：在正确性闭环基础上，以指标与基准驱动优化。

对应 todo：#18 #19 #24 #25

### 阶段 F（P2/P3）：硬件加速闭环（同 ISA）与回退机制

**目的**：把 vm-accel 的后端能力从“可编译/可选择”推进到“被执行引擎真实使用”。

对应 todo：#20 #21 #22 #23

### 阶段 G（P2/P3）：GC 与堆模型闭环

**目的**：把 gc-optimizer 与 vm-boot 的占位 GC runtime 接成可回归的内存管理闭环。

对应 todo：#26 #27

### 阶段 H（持续）：工程化、模块收敛、DDD 合规与可观测性

对应 todo：#28 #29 #30 #31 #32 #33 #34 #35

---

## 3. TodoList（全量，不遗漏）

> 状态字段：not-started / in-progress / completed

### P0：构建与测试阻断

1. **修复 vm-core 编译失败**（status: not-started）
   - 修复 workspace 阻断问题：
     - vm-core/src/aggregate_root.rs 内重复定义 `record_event`（需要合并/删除其一）
     - vm-core/src/domain_services/vm_lifecycle_service.rs 与 vm-core/src/lib.rs 中 `VmResourceAvailabilityRule` re-export/导入路径错误（应从 `rules::lifecycle_rules` 暴露）
     - vm-core/src/domain_services/vm_lifecycle_service.rs 事件发布 `publish_event` 参数可变性签名与调用点不一致（统一使用 `&mut VirtualMachineAggregate` 或调整为内部可变）
   - 验收：`cargo test -p vm-core` 通过

2. **建立最小 CI 测试基线**（status: not-started）
   - 确保 `cargo test -p vm-core`、`cargo test -p vm-cross-arch`、`cargo test -p vm-engine-interpreter` 在本地可跑且稳定
   - 将其加入 CI（如 GitHub Actions/本地脚本）
   - 验收：三条命令在干净环境通过

3. **修复 vm-simd Rust2024 unsafe**（status: not-started）
   - 处理 Rust 2024 `unsafe_op_in_unsafe_fn` 告警（示例来源：vm-simd/src/opt/mod.rs）
   - 为每个 unsafe 操作加显式 `unsafe {}` 块或调整函数体/注解
   - 验收：`cargo test -p vm-simd` 无此类告警（或在 clippy/CI 中基于策略允许但需文档化）

4. **补齐编译告警治理策略**（status: not-started）
   - 制定并执行编译告警治理：unused_mut/unused_variables/dead_code/unused_must_use
   - 至少对 `unused_must_use` 的 `Box::from_raw` 处理为 `drop(Box::from_raw(ptr))` 或 `let _ =` 并加安全说明
   - 验收：核心 crates 在 CI 阶段 `-D warnings`（或分阶段白名单）可执行

### P1：路线与能力矩阵

5. **冻结并发布能力矩阵**（status: not-started）
   - 将 README 与现状报告中的能力声明整理为“能力矩阵”：每条能力必须对应至少一个可运行测试/示例
   - 输出文档：docs/capability-matrix.md
   - 验收：矩阵中每项都有链接到测试/示例或标记为 roadmap

6. **明确跨架构产品路线**（status: not-started）
   - 做关键决策：跨 ISA 目标是“用户态仿真”还是“全系统仿真”
   - 输出 ADR：docs/adr/0001-cross-isa-scope.md
   - 验收：团队评审通过并作为后续任务前置条件

7. **定义两两互运行支持矩阵**（status: not-started）
   - 基于三架构（x86_64/arm64/riscv64）定义 Host×Guest 的 6 条路径支持等级：解释执行（IR）、DBT/JIT、AOT、（同 ISA）硬件加速
   - 输出到 docs/cross-arch-support.md
   - 验收：每条路径都有当前状态与缺口清单

8. **收敛执行链路为单入口**（status: not-started）
   - 统一跨架构执行入口：明确 vm-cross-arch 的主入口应是 vm-cross-arch/src/unified_executor.rs 或 CrossArchRuntime
   - 示例与服务层只调用同一入口
   - 验收：examples/ 与 vm-service 只依赖一个公开入口 API

### P1/P2：JIT/AOT 真执行闭环

9. **打通 AutoExecutor 的 JIT**（status: not-started）
   - 将 vm-cross-arch/src/auto_executor.rs 的 ExecMode::Jit/Hybrid 分支从“总是回退解释器”改为可选启用真实 JIT（feature gate + 运行时配置）
   - 验收：启用 feature 时，JIT 路径可被测试触发且统计可见

10. **实现真实可执行内存管理**（status: not-started）
   - 为 JIT/AOT 实现可执行内存：mmap/VirtualAlloc + W^X、对齐与页面权限、icache flush（aarch64/riscv64）、异常/信号处理策略
   - 验收：可把生成机器码映射为函数指针并执行最小样例（mov/add/ret）

11. **实现真实 JIT 执行语义**（status: not-started）
   - 替换 vm-engine-jit/src/core.rs 中 `execute_compiled_code` 的模拟执行
   - 定义 ABI（传入 MMU/寄存器上下文）、回写寄存器与 PC、处理 fault
   - 验收：解释器与 JIT 对同一 IRBlock 结果一致并有回归测试

12. **统一 JIT 代码缓存形态**（status: not-started）
   - cache 从 Vec<u8> 升级为“可执行代码块句柄”（含 host_addr/size/元数据/失效机制）
   - 同步调整 vm-cross-arch/src/unified_executor.rs 的回退逻辑
   - 验收：cache 命中走直接调用而非 runtime.execute_block

13. **实现 AOT 直接调用 host_addr**（status: not-started）
   - UnifiedExecutor 的 AOT 执行不再回退 runtime.execute_block，改为调用 AotLoader 返回的 host_addr
   - 补齐安全边界（指针有效性、可执行权限、签名）
   - 验收：加载 AOT 镜像后至少一个 block 直接执行并通过一致性测试

14. **统一跨架构翻译与 JIT**（status: not-started）
   - 解决“双管线”问题：translator 产 TargetInstruction bytes 与 JIT codegen 并存
   - 统一方案：translator 产可供 JIT 后端消费的低级 IR，或让 JIT 后端成为 translator 的目标
   - 验收：跨架构 DBT 路径只有一套可执行 codegen

15. **构建跨架构正确性测试**（status: not-started）
   - 建立差分测试：同一输入机器码/IR，在解释器 vs JIT/DBT 下输出一致
   - 覆盖：算术/分支/访存/系统指令（受限）
   - 验收：CI 每次跑且稳定

### P2：OS/系统支持（按 ADR 二选一）

16. **扩展 syscall-compat 路线**（status: not-started）
   - 若选择“用户态仿真”：扩展 vm-cross-arch/src/os_support.rs 的 syscall 覆盖（open/read/write/close/mmap/brk/rt_sigaction/futex 等）
   - 未知 syscall 不应默认返回 0：改为可配置错误/ENOSYS
   - 验收：运行最小静态链接用户态程序（hello world + 文件读写）

17. **全系统仿真路线评估**（status: not-started）
   - 若选择“全系统仿真”：定义引导链路（固件/bootloader）、中断控制器、MMIO 设备模型、特权级/页表等最小集合
   - 产出里程碑与 PoC 分支
   - 验收：启动到内核 early boot 日志（不要求完整用户空间）

### P2：语义与性能关键点

18. **完善内存模型与对齐端序**（status: not-started）
   - 审查并加强 translator 的 MemoryAlignmentOptimizer/EndiannessConversionStrategy（vm-cross-arch/src/translator.rs）
   - 验收：新增测试覆盖未对齐 load/store 与端序转换

19. **优化跨架构热点与块缓存**（status: not-started）
   - 验证并优化 block_cache 与 hotspot 统计：LRU 行为/并发锁开销/阈值
   - 验收：bench 中 cache hit rate 提升且锁竞争下降

### P2：硬件加速闭环

20. **硬件加速特性开关整理**（status: not-started）
   - 整理 vm-accel features：恢复/明确 kvm feature（vm-accel/Cargo.toml 当前被注释）
   - 给出平台条件编译与依赖版本策略
   - 验收：Linux 下 `--features vm-accel/kvm` 可编译且 smoke test 可运行

21. **硬件加速与 vm-core 集成**（status: not-started）
   - 实现真正的 ExecMode::Accelerated 执行引擎：把 vm-accel vCPU run/exit 与 vm-core ExecutionEngine/MMU/设备模型串联
   - 替换 AutoExecutor 中“临时回退解释器”
   - 验收：同 ISA 最小 guest 可进入 vCPU run 循环并处理 MMIO/IO 退出

22. **建立硬件加速回退机制**（status: not-started）
   - 将 vm-accel/src/accel_fallback.rs 的回退管理器接入真实执行路径
   - 验收：构造触发回退用例并验证状态一致

23. **IOMMU/VFIO 直通集成**（status: not-started）
   - 集成 vm-device/src/gpu_passthrough.rs 与内存映射/中断/设备模型，明确 Linux-only 边界
   - 验收：Linux 下可枚举设备并完成 prepare（不要求渲染）

### P2/P3：指标与基准

24. **JIT/AOT 性能指标体系**（status: not-started）
   - 定义并实现可测指标：编译吞吐、p50/p99 编译时延、执行吞吐、cache hit rate、暂停时间（GC）、内存占用
   - 统计从模拟估算改为真实采样
   - 验收：perf-bench/benches 输出稳定且可回归

25. **建立跨架构性能基准**（status: not-started）
   - 新增跨 ISA microbench：指令混合、访存模式、分支密度、syscall 密度
   - 对比解释器 vs DBT/JIT
   - 验收：给出明确性能差距与优化方向

### P2/P3：GC 集成闭环

26. **GC 与 VM 堆模型对齐**（status: not-started）
   - 明确目标对象模型与堆布局：vm-boot/src/gc_runtime.rs（占位）与 gc-optimizer/src/lib.rs（独立库）对齐
   - 定义：根集合、写屏障插入点、safepoint、增量/并行策略
   - 验收：至少一个“对象分配+回收”闭环测试

27. **定义 safepoint 与暂停语义**（status: not-started）
   - 为 JIT/解释器/硬件加速统一 safepoint 与 STW/增量语义
   - 验收：GC 触发时各执行模式可安全停顿/恢复

### 持续工程化：并发、收敛、可观测、DDD、发布

28. **并发模型与任务划分**（status: not-started）
   - 明确执行线程 vs 后台线程职责（热点统计/异步编译/设备 I/O/监控采样）
   - 评估 async-executor/coroutine-scheduler 接入点
   - 验收：JIT 编译可后台并行且不阻塞主执行

29. **收敛 JIT 模块与实验代码**（status: not-started）
   - 清理 vm-engine-jit/src/lib.rs 中大量被注释模块：建立 experimental/ 或 feature gate
   - 对外只导出稳定 API
   - 验收：上层 crates 不依赖不稳定模块

30. **统一错误与可观测性**（status: not-started）
   - 统一 vm-core 的 VmError/PlatformError/DeviceError
   - 统一 tracing/log 埋点与指标命名
   - 验收：译码/执行/缓存/加速/回退关键路径都有 trace span

31. **文档与代码一致性校验**（status: not-started）
   - 对 README 与 RUST_VM_COMPREHENSIVE_REVIEW_REPORT.md 做“声明-证据”对齐
   - 未落地能力标注 roadmap 并链接到对应 todo/ADR
   - 验收：对外文档不再宣称未实现的“完整支持”

32. **生成可运行示例集**（status: not-started）
   - 收敛 examples/ 为可运行 smoke：Native+Accelerated、CrossArch+Interpreter、CrossArch+JIT（启用时）
   - 验收：`cargo run --example ...` 在支持平台跑通

33. **DDD 贫血模型对齐**（status: not-started）
   - 若目标严格贫血模型：规则/流程移到 domain/application services，聚合/实体保留纯数据与最小状态转换
   - 修复领域服务与聚合边界不清
   - 验收：领域层无复杂流程逻辑，服务层可单测覆盖规则

34. **引入 ADR 与架构图**（status: not-started）
   - 新增 docs/adr/ 与架构图：执行模式选择、JIT 后端选择、跨 ISA 路线选择、硬件加速边界
   - 验收：新成员可通过 docs 快速理解系统

35. **建立发布与版本策略**（status: not-started）
   - workspace 多 crate 版本与 feature 兼容矩阵、稳定 API 策略
   - 平台相关 feature（kvm/hvf/whpx）默认策略文档化
   - 验收：发布构建可复现且最小 feature 集可跑

---

## 4. 建议的“下一步第一件事”

- 建议立即执行 todo #1：修复 vm-core 编译失败。
- 只有 P0 通过后，才开始推进 JIT/AOT 真执行与硬件加速闭环（否则验收不可控）。
```

---

## 5. V2 增补：把“原 Todo”映射到“可验证证据点”（不新增编号，不删减原任务）

> 目的：确保每个 Todo 都能找到“必须修改/验证”的关键代码位置，避免执行时信息丢失。

### 5.1 Todo #9/#12/#13（JIT/AOT 真执行闭环）关键证据点

- vm-cross-arch/src/auto_executor.rs：JIT/Hybrid 分支回退解释器（必须消除“默认回退”）。
- vm-cross-arch/src/unified_executor.rs：AOT/JIT 缓存为 Vec<u8> 且执行回退 runtime.execute_block（必须改成代码句柄 + 直接调用）。
- vm-cross-arch/src/cross_arch_runtime.rs：compile_ir_block 返回“成功标记”（必须替换为真实机器码或可执行句柄）。
- vm-engine-jit/src/lib.rs：占位式 Jit 的 compile/run（若作为产品路径，必须实现真实 codegen+执行；否则应收敛/隔离）。

### 5.2 Todo #16（syscall）关键证据点

- vm-cross-arch/src/os_support.rs：未处理 syscall 当前返回 Ok(0)（必须改为 ENOSYS/可配置错误，并建立回归测试）。

### 5.3 Todo #20（KVM feature）关键证据点

- vm-accel/Cargo.toml：kvm feature 被注释禁用（需要恢复/明确策略）。
- vm-accel/src/kvm_impl.rs：存在 cfg(feature="kvm") 的实现路径（需要编译/运行 smoke 证明有效）。

### 5.4 Todo #26/#27（GC 闭环）关键证据点

- vm-boot/src/gc_runtime.rs：占位触发逻辑（需对齐对象模型/根集合/写屏障/safepoint）。
- gc-optimizer/src/lib.rs：独立 GC 优化能力（需与 VM 堆/对象布局对齐并接入执行模式）。

---

## 附录 A：全面审查报告（全文引用）

> 说明：以下内容为本轮对仓库的全面审查报告全文引用，用于确保“审查报告信息不遗漏”。

```markdown
# Rust 跨架构虚拟机软件全面架构审查报告（2025-12-13）

> 审查范围：当前工作区代码与文档（Rust 2024 workspace，多 crate 结构），重点覆盖跨 ISA 两两互运行、JIT/AOT/GC、硬件加速（KVM/HVF/WHPX 等）与 DDD 贫血模型合规性。

---

## 0. 执行摘要（结论先行）

该项目在“模块划分/工程规模/对外宣称能力”上呈现完整虚拟机产品形态，但从关键执行链路的代码证据看：**跨 ISA 两两互运行、JIT/AOT 真执行、硬件加速闭环、GC 闭环均处于“接口与结构存在、执行落地不足/回退解释器/占位实现”的阶段**。因此当前阶段更接近“架构原型 + 大量占位实现”，尚不具备对外承诺的稳定能力与性能指标。

最核心的技术风险是：**“宣称能力”与“可验证能力”不一致**（例如 JIT/AOT/跨 ISA/硬件加速在执行时普遍回退），导致性能结论、正确性结论、兼容性结论都缺乏可重复验证的闭环支撑。

---

## 1. 架构分析

### 1.1 Workspace 与分层

Workspace 采用多 crate 组合（见 Cargo.toml），宏观上具备清晰的“前端解码 → IR → 执行/翻译 → 运行时/服务/工具”分层意图：

- 核心抽象：vm-core（地址/架构/执行引擎/MMU/错误等）
- IR：vm-ir
- 前端：vm-frontend-x86_64 / vm-frontend-arm64 / vm-frontend-riscv64
- 跨架构层：vm-cross-arch（translator/optimizer/cache/runtime 等）
- JIT/AOT：vm-engine-jit（存在两套风格明显不同的实现形态）
- 硬件加速：vm-accel（KVM/HVF/WHPX/VZ 抽象）
- GC：gc-optimizer（独立优化库） + vm-boot 的 gc_runtime（集成占位）

**优点**
- “模块名即职责”整体清晰，便于未来做能力矩阵和依赖收敛。
- cross-arch、JIT、AOT、GC、accelerator 均预留了较完整的配置结构与扩展点。

**主要结构性问题**
1. **关键链路未形成单一“可执行闭环”**：跨架构执行路径中，存在多个“入口/缓存/编译器/执行器”并行出现，但执行时主要仍走解释器或运行时回退。
2. **同一能力存在多套实现/概念重复**：例如 JIT 既有较完整的 `JITEngine` 形态，又有 `Jit` 占位式形态，跨模块集成主要依赖后者。
3. **接口/枚举语义漂移风险**：不同模块对执行模式的命名（Interpreter/JIT/Hybrid/Accelerated/HardwareAssisted）存在不一致迹象，这会直接导致上层配置与下层执行行为难以对齐。

### 1.2 跨平台/跨架构模拟层设计评估（重点）

项目文档与 cross-arch 模块注释宣称采用“源架构指令 → 统一 IR → 目标架构指令”的路径。但从执行链路代码证据看：

- **解码到 IR 是实际在用的**：AutoExecutor 依据 guest arch 选择对应前端解码器，产出 IRBlock。
- **IR→目标 ISA 编码/翻译结构存在，但与“真执行”未闭环**：translator 能将 IR 转为目标指令序列，但 runtime/executor 路径中没有把这些目标指令装载到可执行内存并实际调用的完成实现。
- **运行时执行仍以解释器为中心**：AutoExecutor 在 JIT/Hybrid 分支明确“总是回退到解释器”，加速分支同样回退。
- OS/系统层支持以“系统调用模拟”为主，但覆盖极少且默认行为危险：Linux syscall handler 仅实现 write 与 exit，其他 syscall 直接 Ok(0)。

**结论**：跨 ISA 两两互运行目前更像“IR 解释执行 + 少量系统调用模拟”的原型，而非“可运行操作系统级别的跨 ISA 虚拟机”。

---

## 2. 功能完整性评估（按承诺能力对照）

### 2.1 Host×Guest 两两互运行（x86_64 / arm64 / riscv64）

- 三架构前端解码→IR：结构存在。
- DBT/JIT 并执行：结构存在，但真执行闭环不足。
- “运行操作系统”：syscall 覆盖不足且错误语义不安全。

### 2.2 JIT/AOT/统一执行器闭环

- UnifiedExecutor 对外呈现 AOT>JIT>Interpreter，但 AOT/JIT 执行均回退 runtime.execute_block。
- CrossArchRuntime 的 JIT 编译结果为“成功标记”，不是可执行机器码。
- vm-engine-jit 存在两套实现并存，cross-arch 主要使用占位式 Jit。

**结论**：JIT/AOT 当前主要处于“结构准备”，尚未形成“可执行内存 + 真调用 + 一致性回归”的闭环。

### 2.3 硬件加速

- vm-accel 提供统一抽象与平台后端组织，但 feature 与执行引擎集成不闭环。

---

## 3. 性能优化机会识别

- 现状主要走 IR 解释执行，性能上限偏低。
- 大量 Arc/Mutex 共享结构可能成为锁竞争热点。
- 在闭环未完成前，继续扩展优化器数量会放大维护成本。

---

## 4. 可维护性检查

- 存在大量一次性修复脚本/中间产物，建议归档/收敛。
- 文档宣称与事实偏差需要做“声明-证据”对齐。
- 缺少系统性的差分测试与回归体系。

---

## 5. DDD 贫血模型合规性验证

- 可见 Aggregate/Domain Service/Rule/EventBus 等 DDD 结构元素；整体更符合贫血模型方向。
- 风险在于边界约束与与基础设施耦合，需要通过更严格 API 边界与分层治理。

---

## 6. 建议路线

- 冻结跨 ISA 路线（用户态 vs 全系统）。
- 优先完成 JIT/AOT 真执行闭环。
- 硬件加速先同 ISA 闭环，再谈扩展。
- syscall 未实现必须 ENOSYS，不可 Ok(0)。
- 建立能力矩阵与证据化测试。
```

---

## 附录 B：占位/回退关键词扫描（用于后续收敛/清理）

> 说明：本次扫描发现大量 placeholder/fallback/temporary 注释与实现痕迹，建议在任务 #29/#31/#34 推进时纳入治理范围。
>
> 扫描关键词：TODO/FIXME/HACK/temporary/placeholder/占位/回退/fallback

- 代表性结果（非穷举）：
  - vm-boot/src/gc_runtime.rs：完整 GC 为占位。
  - vm-cross-arch/src/auto_executor.rs：JIT/Hybrid/Accelerated 回退解释器。
  - vm-cross-arch/src/unified_executor.rs：AOT/JIT 执行回退 runtime.execute_block。
  - vm-core/src/unified_event_bus.rs：异步路径回退同步处理。

---

## 变更记录

- V2 新增：第 2、3、5 节（代码证据绑定、闭环门槛、证据映射）。
- V2 保留：原计划与 TodoList（第 4 节全文镜像）。
