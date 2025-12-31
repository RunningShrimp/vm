## 目标
- 提升I/O与队列处理性能，降低MMU小粒度访问开销
- 统一错误与日志体系，补齐端到端测试、基准与CI
- 引入服务层解耦设备/运行时行为，靠拢DDD贫血模型

## 范围
- 变更crate：`vm-cli`、`vm-device`、`vm-mem`、`vm-engine-{interpreter,jit}`、`vm-boot`、`vm-accel`、`vm-tests`
- 新增内容：集成测试、基准测试、CI配置、服务层模块（在现有crate内新建子模块）

## 阶段一：基础设施与一致性
1. 日志后端初始化
- 在`vm-cli/src/main.rs`初始化`env_logger`（或`tracing_subscriber`）；设置默认级别与环境变量控制
- 统一各crate日志级别使用策略（设备层`debug/trace`，用户可见事件`info/warn/error`）

2. 错误处理统一
- 将`Result<_, String>`迁移为`thiserror`派生的领域错误类型：`BlockError`、`NetError`、`Iso9660Error`、`IoBackendError`
- 在各crate导出统一`Result`别名；保持`vm-core`的`VmError`用于跨层错误收敛

3. 集成测试补齐
- 在`vm-tests`新增`tests/`端到端用例：前端解码→IR→执行引擎（解释器/JIT）→SoftMMU→VirtIO Block/Net交互→运行时控制
- 添加差分测试：同一IR在解释器与JIT执行，比较寄存器与内存状态一致性

4. 基准测试
- 引入`criterion`，新增基准：`vm-mem`（TLB命中/页表遍历）、`vm-device`（VirtIO队列吞吐）、`vm-engine-interpreter/jit`（算术/内存/SIMD热点）、端到端（简化工作负载）

5. CI配置
- 添加GitHub Actions：`cargo build/test/clippy/fmt`、缓存与矩阵（stable/nightly，Linux/macOS/Windows），可选覆盖率（`grcov/llvm-cov`）

## 阶段二：性能优化与架构解耦
6. 批量内存读写
- 扩展`vm-device`中`MmuUtil`：新增`read_slice`/`write_slice`批量接口
- 在`vm-mem`为`SoftMmu`实现高效路径（使用`copy_nonoverlapping`），减少字节级循环
- 更新`VirtioBlock::{handle_read,handle_write}`使用批量接口

7. VirtIO队列优化
- 为队列引入本地镜像（shadow avail/used），批量刷新`used`索引，减少MMU读写次数
- 增加零拷贝策略：将Guest缓冲区映射为Host切片进行批量读写（在`MmuUtil`提供连续区域访问）

8. JIT并行编译与缓存
- 使用`rayon`并行编译IR块；对热点块引入编译缓存与重用策略
- 提供配置开关与线程数控制；在`vm-tests`新增差分与回归用例

9. vCPU事件驱动运行协议
- 在`vm-accel`与`vm-boot`中引入事件循环接口：定时器、设备中断、队列通知统一事件源
- 利用平台后端（KVM/HVF/WHPX）的事件通知替代轮询，明确退出条件与状态管理

10. DDD贫血模型服务化
- 在`vm-boot`引入`runtime_service`子模块，负责运行时状态机与命令处理；`RuntimeController`保留数据/状态
- 在`vm-device`引入`device_service`子模块，负责队列调度与I/O编排；设备对象保留数据与最小操作接口
- 通过服务接口对外暴露行为，设备/MMU对象以数据为主，降低耦合

## 验证与交付
- 测试：所有单元/集成测试通过；新增差分测试覆盖解释器/JIT一致性
- 基准：生成基线结果并归档；在PR显示关键指标变更
- CI：所有平台与Rust通道稳定通过；覆盖率达成约定阈值
- 文档：在根README补充模块图、构建运行、测试/基准与CI指引（在用户确认后实施）

## 风险与回滚
- Trait扩展与零拷贝需注意不破坏现有接口；先以`MmuUtil`扩展避免破坏`MMU`trait
- 并行编译与事件化运行可能引入竞态；通过原子与锁策略、端到端测试与CI确保稳定
- 若性能或稳定性下降，提供编译特性/配置开关回退到旧路径

## 里程碑验收标准
- P1完成：日志/错误统一、CI与端到端测试通过
- P2完成：批量I/O与VirtIO优化落地，基准指标显著改善
- P3完成：JIT并行与事件驱动、服务化重构完成，DDD贫血模型基本达标