## 现状评估
- 当前 Workspace 已按模块化微内核划分：`vm-core`、`vm-ir`、`vm-mem`、`vm-frontend-*`、`vm-engine-interpreter`、`vm-engine-jit`、`vm-accel`、`vm-device`、`vm-osal`、`vm-cli`、`vm-tests`（`/Users/didi/Desktop/project/vm/Cargo.toml:1-16`）。
- IR 已支持内存序与原子操作、SIMD 向量算子（`vm-ir/src/lib.rs:20-23`，`vm-ir/src/lib.rs:65-89`，`vm-ir/src/lib.rs:73-84`）。
- 解释器引擎实现了完整的 IR 语义、原子/屏障、SIMD/多路256位向量等（例如原子与屏障处理 `vm-engine-interpreter/src/lib.rs:194-233`，向量算子 `vm-engine-interpreter/src/lib.rs:245-299`、`445-557`）。
- JIT 已接入 Cranelift 并能按基本块编译/缓存（`vm-engine-jit/src/lib.rs:31-49`，`vm-engine-jit/src/lib.rs:51-63`，`vm-engine-jit/src/lib.rs:251-258`）。
- 前端 Decoder 已有 ARM64、x86_64、RISCV64 基础解码到 IR（示例：x86 SIMD 加载映射 `vm-frontend-x86_64/src/lib.rs:474-498`；ARM64 加载/存储/跳转 `vm-frontend-arm64/src/lib.rs:68-116`）。
- 内存 SoftMMU 与 MMIO 挂载可用，含 VirtIO Console MMIO 雏形（`vm-mem/src/lib.rs:16-21`，`vm-device/src/lib.rs:26-55`）。
- OSAL 提供跨平台内存屏障原语（`vm-osal/src/lib.rs:9-11`）。
- 测试覆盖原子/内存序、SIMD、VRing 使用链与中断链路（内存序一致性测试 `vm-tests/src/lib.rs:62-91`，VRing 链路 `vm-tests/src/lib.rs:664-696`）。

## 总体目标
- 在保持微内核模块边界的前提下，补齐硬件加速、图形/输入/网络设备栈与跨平台 OS 适配，形成可运行真实 Guest OS 的完整 VM。
- 提供自适应加速框架：同构直通（KVM/HVF/WHPX）与异构 JIT/解释器协同，按宿主与来宾架构动态选择最优路径。
- 建立 GUI 桌面体验与基础 VirtIO 设备（Block/Net/Input/GPU）的高性能实现。

## 分阶段实施
1. 基线巩固（IR/前端/引擎）
- 完善 IR 与前端一致性：补齐条件码/标志位、系统调用与异常语义的抽象；增强 x86 分组/前缀与 ARM64 更多类指令解码。
- JIT 侧添加热点追踪与简单内联、死代码消除；在 IR 层记录执行计数以驱动热点识别。
- 解释器与 JIT 双引擎协同：冷路径解释器，热路径 JIT，保持可回退与调试。

2. 自适应加速（vm-accel）
- 定义统一加速接口 Trait（`Accel`）：`init()`, `create_vcpu()`, `map_memory()`, `run()`, `inject_interrupt()` 等。
- 实现后端：`AccelKvm`（Linux）、`AccelHvf`（macOS）、`AccelWhpx`（Windows）。在 `vm-accel` 内封装探测与仲裁逻辑（架构/特性检测、策略选择）。
- 探测维度：`target_arch`、CPUID/ID_AA64 寄存器、HVF/WHPX/KVM 可用性；CPU SIMD（AVX2/AVX-512/NEON/SVE）能力，用于 JIT 与设备路径优化。
- 同构 Guest==Host：优先直通加速；异构 Guest!=Host：JIT+解释器；允许在 vCPU 级别混合（例如 I/O 线程解释，计算线程 JIT）。

3. 内存与 I/O（vm-mem, vm-device）
- SoftMMU 扩展：两级地址转换（GVA->GPA->HVA），引入页表与 TLB 失效策略；
- HugePages/THP：在支持宿主上启用大页映射以降低 TLB Miss；
- Zero-copy I/O：Linux 路径上通过 io_uring（直通模式）与共享内存环绕缓冲；
- VirtIO 设备族：在现有 Console 基础上实现 `virtio-block`、`virtio-net`、`virtio-input` 与 `virtio-gpu`（含多队列/事件索引/中断屏蔽），与 MMIO/PCI 总线抽象对接。

4. 图形与 GUI（vm-device + vm-cli）
- 渲染：接入 `wgpu`（Metal/DX12/Vulkan）统一后端；实现 `virtio-gpu`/VirGL 路径，将 Guest 的绘制命令通过共享内存提交 Host 翻译执行。
- 窗口系统：`winit` 创建跨平台窗口，支持高 DPI；事件捕获与输入设备映射（键盘/鼠标/触控）。

5. 网络栈（vm-device）
- NAT 模式：集成 `smoltcp` 做用户态 TCP/IP；
- 桥接模式：TAP/TUN 在有权限时提供独立 IP；
- `virtio-net`：实现多队列与中断优化（事件索引/延迟唤醒）。

6. OS 适配（vm-osal）
- 为 Windows/macOS/Linux/HarmonyOS 提供文件/内存/线程/定时器/设备访问抽象；
- 线程亲和性：ARM big.LITTLE 绑定策略（vCPU→大核，I/O→小核）。

7. CLI/启动流程（vm-cli）
- 参数解析：选择加速后端、Guest 架构、内存大小、镜像路径、网络/显示选项；
- 启动：最小化 UEFI/BIOS 路径与 `virtio-block` 指向 ISO，引导安装镜像；
- 运行时控制：暂停/恢复/快照、设备插拔。

8. 测试与基准（vm-tests）
- 单元测试：扩展现有 IR/原子/向量覆盖至新指令与设备；
- 兼容性测试：在三大宿主（Win/macOS/Linux）上自动化运行常见 Guest（Ubuntu/Win10/Android-x86）；
- 性能基准：在 VM 内跑 SPEC2017/Geekbench，记录 JIT 开销占比、I/O 吞吐、启动时间等指标。

## 关键技术点与落地细节
- IR 内存序与屏障：已通过 Acquire/Release 语义与 fence 计数验证（`vm-engine-interpreter/src/lib.rs:194-233` 与测试 `vm-tests/src/lib.rs:62-91`）。后续需在 JIT 中映射为宿主指令/调用序列。
- SIMD 映射：x86/ARM 前端已产出向量 IR，解释器具备字节/字宽饱和运算与 128/256 位多路支持；JIT 需按宿主 ISA 映射至 AVX/NEON，缺失的算子以内联 helper 兜底。
- 加速后端隔离：采用 Trait + 后端模块，OSAL 统一系统调用；保障同一上层 VMCore 不感知平台差异。
- 内存/设备安全：严格对齐/越界检查，避免不安全日志/秘钥输出；MMIO 设备使用内存屏障保障队列一致性。

## 优先级与里程碑
- M1：`vm-accel` 自适应加速框架（探测+Trait+KVM/HVF/WHPX 雏形）
- M2：内存两级转换与大页支持；`virtio-block` 完整读写链路
- M3：`wgpu`+`winit` GUI 与 `virtio-gpu` 基本 2D/简单 3D 路径
- M4：`virtio-net`（NAT/桥接），用户态网络跑通
- M5：JIT 热点优化管线与端到端性能基准

## 交付与验收指标
- 启动至固件界面 < 500ms；JIT 开销 < 15% CPU 时间；I/O 吞吐 ≥ 80% 宿主；跨平台自动化测试全部通过。

## 需要确认
- 首批目标 Guest/宿主组合与优先平台（例如 macOS+ARM64 优先 HVF，或 Linux+x86_64 优先 KVM）。
- GUI 范围（仅 2D 桌面 vs 3D 加速要求）与网络模式默认值（NAT/桥接）。
- 是否需要最小化 UEFI/BIOS 的内置/引入策略。