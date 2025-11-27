# 垃圾回收机制在系统级虚拟机中的适用性研究

## 结论概览
- 适用领域：对“翻译产物与运行时资源”的回收非常适用（JIT/DBT 代码缓存、跳转表、映射索引、统计与日志缓冲）。
- 不适用领域：对“客体 OS 的堆/对象”不进行 GC（由客体自身内核/运行时负责）。
- 推荐方案：引入“代码缓存 GC（分代/热度驱动）+ 失效与并发安全回收（RCU/epoch）+ 主机侧内存合并（KSM）”，并与 AOT+JIT 混合架构协同。

## 可应用的 GC 思路
### 1) 代码缓存 GC（JIT/DBT/AOT）
- 背景：QEMU/TCG 在翻译块（TB）缓存满时执行“全量刷新”，并在自修改代码或页状态变化时进行粒度化失效与链路回滚 [QEMU MTTCG 设计 | https://www.qemu.org/docs/master/devel/multi-thread-tcg.html]。
- 思路：参考 JVM 的“代码缓存分段与清扫器”，将短命编译产物与长命产物分区，按热度与时间进行回收，避免缓存爆满导致编译停摆 [JVM Code Cache | https://www.baeldung.com/jvm-code-cache]。
- 建议：
  - 分区：`短命区（JIT快编）/长命区（JIT高优化）/稳定区（AOT）`；对 AOT 仅做失效不做主动清扫。
  - 热度计数：维护 TB 命中与跳转链被引用次数，低热度与久未使用优先清扫；在“缓存接近阈值”或“定期窗口”触发清扫。
  - 触发：缓存逼近上限、定期清扫窗口、命中率下降、TB 失效链大量累积。
  - 观测：输出 `size/used/max_used/free` 与清扫事件，仿照 JVM 的 `PrintCodeCache` 观测项 [JVM Code Cache | https://www.baeldung.com/jvm-code-cache]。

### 2) 并发安全回收（RCU/Epoch）
- 背景：MTTCG 强调 TB 链接/反链接与查找结构的并发安全（原子补丁、锁、无锁哈希）与“使所有 vCPU 进入静止态后再做全局更新” [QEMU MTTCG | https://qemu.weilnetz.de/doc/9.2/devel/multi-thread-tcg.html]。
- 思路：采用 RCU/epoch 风格回收，读侧无锁，写侧发布新版本并将旧结构加入“延迟回收队列”，在所有 vCPU 过了一个 epoch 后批量释放。
- 适用对象：TB 链表、跳转补丁、页描述与 TB 索引、间接跳转查表。

### 3) 失效与刷新策略（与 DBT 机制协同）
- 事件驱动失效：代码页写入、自修改、页映射变化、断点插入/移除等触发 TB 失效与跳转回滚 [QEMU MTTCG | https://qemu.weilnetz.de/doc/6.0/devel/multi-thread-tcg.html]。
- 局部回收优先：优先清理失效 TB 的链接与缓存占位，减少“全量刷新”的频率；必要时同步所有 vCPU，执行全局刷新与再编译。

### 4) AOT+JIT 混合中的 GC
- 长命区保护：AOT 代码区设置 W^X 与只读映射，不参与主动清扫；通过签名与版本匹配决定是否可用，变更时整体失效。
- JIT 分代：短命区用于“未命中或探测编译”，长命区用于“稳定热路径”；后台清扫器从短命区优先回收，降低停顿。

### 5) 主机侧内存合并与回收（非 GC，但可协同）
- KSM（页面合并）：宿主 Linux 可通过 KSM 合并多个相同内容的页，降低多虚拟机/多进程场景内存占用；需权衡 CPU 扫描成本与潜在侧信道风险 [Linux KSM 文档 | https://docs.kernel.org/admin-guide/mm/ksm.html, Wikipedia | https://en.wikipedia.org/wiki/Kernel_same-page_merging, Red Hat 虚拟化调优 | https://docs.redhat.com/en/documentation/red_hat_enterprise_linux/7/html/virtualization_tuning_and_optimization_guide/chap-ksm, Proxmox 风险说明 | https://pve.proxmox.com/wiki/Kernel_Samepage_Merging_(KSM)]。
- Ballooning：在过度承诺场景配合气球驱动与换页，GC 清扫出来的代码缓存物理页可及时回收给宿主。

## 不适用的 GC 场景澄清
- 客体 OS 内存管理：客体的堆、对象、页框由客体自身内核/运行时管理；宿主侧不介入其 GC。
- 设备仿真数据面：设备寄存器状态须由仿真层精确管理，不能随意 GC；仅对“过期队列、统计缓冲”可用周期性清理。

## 与当前代码的集成建议
- 启动阶段：在 `vm-boot/src/runtime.rs:205` 的 `start()` 挂载“代码缓存管理器”，设定分区大小、热度阈值与清扫周期。
- 事件轮询：在 `vm-boot/src/runtime.rs:145` 的 `poll_events()` 附加清扫触发器（定时与阈值）；在快照分支（`vm-boot/src/runtime.rs:169-171`）持久化命中统计与区域使用状况。
- 状态更新：在 `vm-boot/src/runtime.rs:180` 的 `update_state()` 在停止/关闭时执行安全清扫与资源释放；在恢复/启动时加载统计以优化命中。

## 度量与控制
- 指标：代码缓存大小与占用、分区命中率、清扫次数/时长、失效触发源统计、TB 查找/补丁原子冲突率。
- 控制：清扫周期、阈值、分区比例、KSM 开关与页合并策略、是否允许“接近上限时紧急清扫”。

## 风险与缓解
- 并发一致性：采用原子/锁/RCU/epoch 组合；在需要全局修改时使 vCPU 进入静止态，避免读侧悬挂指针 [QEMU MTTCG | https://www.qemu.org/docs/master/devel/multi-thread-tcg.html]。
- 清扫停顿：分区与后台清扫器减少停顿；必要时分阶段清扫与速率限制。
- 安全：启用 W^X；KSM 仅在评估风险后开启，避免跨 VM 侧信道。

## 参考资料
- QEMU MTTCG 与 TB 失效/并发设计：https://www.qemu.org/docs/master/devel/multi-thread-tcg.html, https://qemu.weilnetz.de/doc/9.2/devel/multi-thread-tcg.html, https://qemu.weilnetz.de/doc/6.0/devel/multi-thread-tcg.html
- QEMU TCG 翻译与 TB 链接机制：https://www.qemu.org/docs/master/devel/tcg.html, https://stackoverflow.com/questions/20675226/qemu-code-flow-instruction-cache-and-tcg
- JVM 代码缓存分段与清扫器（思路借鉴，非客体 GC）：https://www.baeldung.com/jvm-code-cache, IBM OpenJ9 Cookbook（代码缓存/GC 策略综述）：https://publib.boulder.ibm.com/httpserv/cookbook/Java-Java_Virtual_Machines_JVMs-OpenJ9_and_IBM_J9_JVMs.html
- KSM（主机侧页面合并）：https://docs.kernel.org/admin-guide/mm/ksm.html, https://en.wikipedia.org/wiki/Kernel_same-page_merging, https://docs.redhat.com/en/documentation/red_hat_enterprise_linux/7/html/virtualization_tuning_and_optimization_guide/chap-ksm, https://pve.proxmox.com/wiki/Kernel_Samepage_Merging_(KSM)
