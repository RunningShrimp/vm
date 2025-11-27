# 系统级虚拟机性能提升方案：AOT+JIT混合与代码缓存GC（整合版）

## 背景与目标
- 背景：当前基于 JIT/DBT 动态翻译运行客体 OS 与应用；已设计引入 AOT 以降低冷启动与翻译抖动。
- 目标：形成统一架构，将 AOT+JIT 混合与“代码缓存 GC”协同，提升整体吞吐与时延稳定性；明确实施计划与任务清单。
- 范围：系统级虚拟化（非字节码 VM）；覆盖特权指令、MMU、异常/中断与设备仿真语义；不干预客体 OS 的自身内存管理与 GC。

## 架构设计总览
- 离线 AOT 构建：采样热区→控制流恢复→指令语义抬升（LLVM IR）→优化→编译→打包 AOT 镜像。
- 运行时加载：启动匹配并映射 AOT 镜像；未命中路径由 JIT 翻译并可异步写入缓存以供下次使用。
- 代码缓存 GC：对 JIT/DBT 翻译产物采用分代/热度驱动清扫；AOT 仅做版本化失效，不参与主动清扫。
- 并发与失效：RCU/Epoch 管理 TB 链接与索引；代码页写入、自修改、断点等触发 TB 失效与跳转回滚；必要时全局刷新。

## 关键模块与接口
- AOT 构建器（离线）
  - 热区采集：在 JIT 中导出基本块命中与 CFG，确定候选翻译单元。
  - 指令抬升：用指令语义库（如 `Remill`）抬升至 LLVM IR；用 CFG 工具（如 `McSema`）拼接函数级 IR。
  - 编译与优化：LLVM 优化（可选 PGO），生成宿主代码或共享库。
  - 镜像打包：记录 `build-id`、段映射、跳板表、间接跳转查找表、W^X 标志、版本与校验。

- AOT 镜像格式
  - 元数据：`guest_build_id`、`guest_image_ranges`、`host_text_ranges`、`trampoline_table`、`indirect_lookup`、`abi_version`、`signature`。
  - 双向映射：客体地址↔宿主地址，用于异常回溯与调试。
  - 安全：镜像签名与校验；代码页 W^X；只读段保护与页级失效。

- AOT 加载器（运行时）
  - 匹配：按 `build-id` 与模块版本绑定镜像；不匹配禁用 AOT。
  - 映射与绑定：将宿主代码段映射到代码缓存；绑定运行时 API（CPU 状态、MMU、异常/中断、设备 I/O）。
  - 跳板与查表：支持间接跳转与跨单元调用；未解析目标回退 JIT。

- 运行时语义 API
  - CPU 状态：寄存器/标志位/FPU/SIMD 状态读写。
  - MMU/访存：统一接口执行访存与页表操作，保持异常语义一致。
  - 异常与中断：内联序列处理简单特权指令；复杂上下文切换调用运行时。
  - 设备 I/O：通过仿真层 API 完成端口/内存映射 I/O。

- 代码缓存 GC 管理器
  - 分区：`短命区（JIT快编）/长命区（JIT高优化）/稳定区（AOT）`。
  - 热度计数：维护 TB 命中与引用；阈值与时间窗口驱动清扫，优先短命区。
  - 并发回收：RCU/Epoch；读侧无锁，写侧发布新版本，旧数据延迟释放；必要时 quiesce 所有 vCPU。
  - 观测：暴露 `size/used/max_used/free`、清扫次数/耗时、失效源统计、命中率。

- 失效与刷新策略
  - 事件驱动：代码页写入、自修改、页映射变化、断点插入/移除触发 TB 失效与跳转回滚。
  - 局部优先：优先清理失效 TB 占位与链接；减少全量刷新频率；必要时全局同步刷新。

## 与当前代码的集成点
- 启动加载：在 `vm-boot/src/runtime.rs:205` 的 `start()` 接入 AOT 镜像匹配→映射→运行时绑定，并初始化代码缓存 GC 管理器。
- 事件轮询：在 `vm-boot/src/runtime.rs:145` 的 `poll_events()` 增加定时/阈值清扫触发；在快照分支（`vm-boot/src/runtime.rs:169-171`）持久化命中统计与区域占用。
- 状态更新：在 `vm-boot/src/runtime.rs:180` 的 `update_state()` 在停止/关闭时执行安全清扫与资源释放；在恢复/启动时加载统计优化命中。

## 实施计划（Phase）
- Phase 1（PoC）
  - 在 JIT 中采样基本块热度与导出 CFG（限定单一用户态二进制/常用库）。
  - 离线抬升到 LLVM IR 并编译生成 AOT 镜像；实现最小加载器与镜像匹配。
  - 引入代码缓存 GC 骨架：分区、热度计数、后台清扫与指标输出。
  - 指标采集对比：JIT-only vs Hybrid（冷启动时延、命中率、翻译时间占比）。

- Phase 2（Hybrid 扩展）
  - 扩展到稳定内核模块与常用用户态库；完善写时失效与异常/中断托管。
  - 建立“JIT→离线 AOT 回填→下次启动命中”的闭环与版本化镜像管理。
  - 并发回收完善：RCU/Epoch 引入到 TB 链接/索引与跳转表；优化清扫停顿。
  - 快照持久化：保存/恢复命中统计与镜像引用，提升恢复后性能。

- Phase 3（全面优化）
  - LLVM IR 层面 PGO 与去冗优化；间接跳转预测与跳板加速。
  - 设备仿真协同优化（减少陷入、批处理页表/设备访问）。
  - 主机侧内存合并（KSM）评估开启策略与风险控制；配合 ballooning。

## Todo List（实施任务）
- 添加热区采集与 CFG 导出工具链
- 实现指令抬升到 LLVM IR
- 编译生成 AOT 镜像与签名校验
- 编写 AOT 镜像加载器接口
- 绑定运行时语义 API（CPU/MMU/异常/设备）
- 建立跳板与间接跳转查表
- 引入代码缓存分区与热度计数
- 实现后台清扫器与指标输出
- 设计并接入 RCU/Epoch 回收
- 实现代码页写保护与失效回退
- 集成快照的统计持久化与恢复
- 构建版本化镜像管理与灰度加载
- 打通 JIT→离线回填→AOT 命中闭环
- 建立度量面板与报警（命中率/停顿）
- 评估与配置主机侧 KSM 策略

## 度量与风险
- 指标：冷启动、命中率、翻译时间占比、清扫次数/时长、失效源统计、TB 查找冲突率。
- 风险：并发一致性（用原子/锁/RCU/epoch）、清扫停顿（分区/后台/分阶段）、安全（W^X 与镜像签名）、KSM 侧信道风险。

## LLVM 集成结论
- 运行时不集成 LLVM：不将 LLVM 置于生产运行时进程中，避免体积与复杂性膨胀，保持系统级仿真语义与隔离的纯净。
- 仅离线使用 LLVM（推荐）：在 AOT 构建流水线中使用 LLVM/抬升工具将客体机器码抬升到 `LLVM IR`，进行优化并编译为宿主代码，生成签名完备的 AOT 镜像；运行时仅加载镜像与绑定运行时 API。
- 何时需要 LLVM：需要跨 ISA 的静态重编译与优化时；复用成熟后端与优化管线、降低自研成本。
- 不需要 LLVM 的情况：仅做纯 JIT/DBT 或手写少量指令子集翻译（但维护与可移植性成本高）。
- 对当前集成的影响：既有启动/事件/状态更新集成点不变（`vm-boot/src/runtime.rs:205`、`vm-boot/src/runtime.rs:145`、`vm-boot/src/runtime.rs:180`）；新增独立构建工具链生成 `.aot` 镜像并版本化管理。

## 参考资料
- QEMU MTTCG 与 TB 失效/并发设计：https://www.qemu.org/docs/master/devel/multi-thread-tcg.html、https://qemu.weilnetz.de/doc/9.2/devel/multi-thread-tcg.html、https://qemu.weilnetz.de/doc/6.0/devel/multi-thread-tcg.html
- QEMU TCG 翻译与 TB 链接机制：https://www.qemu.org/docs/master/devel/tcg.html、https://stackoverflow.com/questions/20675226/qemu-code-flow-instruction-cache-and-tcg
- JVM 代码缓存分段与清扫器（理念借鉴）：https://www.baeldung.com/jvm-code-cache、https://publib.boulder.ibm.com/httpserv/cookbook/Java-Java_Virtual_Machines_JVMs-OpenJ9_and_IBM_J9_JVMs.html
- KSM（主机侧页面合并）：https://docs.kernel.org/admin-guide/mm/ksm.html、https://en.wikipedia.org/wiki/Kernel_same-page_merging、https://docs.redhat.com/en/documentation/red_hat_enterprise_linux/7/html/virtualization_tuning_and_optimization_guide/chap-ksm、https://pve.proxmox.com/wiki/Kernel_Samepage_Merging_(KSM)
