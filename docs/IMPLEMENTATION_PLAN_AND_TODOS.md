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
