# vm-runtime 模块架构说明

## 概述

vm-runtime 是虚拟机系统的运行时模块，负责协程调度、垃圾回收（GC）、沙箱执行、以及虚拟机的整体运行时管理。

## 架构层次

```
vm-runtime
├── lib.rs                    # 主入口
├── gc.rs                     # GC 运行时
├── coroutine_scheduler.rs   # 协程调度器
├── coroutine_pool.rs         # 协程池
├── scheduler.rs                # 调度器核心
├── sandboxed_vm.rs           # 沙箱虚拟机
├── vcpu_coroutine_mapper.rs # vCPU 协程映射器
└── tests/                     # 测试套件
```

## 核心组件

### 1. 协程调度器（CoroutineScheduler）

协程调度器负责管理虚拟机的所有协程：
- 协程生命周期管理
- 优先级队列调度
- 负载均衡
- vCPU 分配

```rust
pub struct CoroutineScheduler {
    coroutines: Arc<DashMap<u64, CoroutineInfo>>,
    vcpus: Arc<DashMap<u32, VCpuInfo>>,
    ready_queues: Arc<Vec<tokio::sync::Mutex<Vec<u64>>>>,
    blocked_tasks: Arc<tokio::sync::Mutex<Vec<BlockedTask>>>,
}
```

#### 调度策略

- **优先级调度**：高优先级任务优先执行
- **公平调度**：轮询保证公平性
- **负载均衡**：跨 vCPU 均衡分配
- **抢占式调度**：高优先级可抢占低优先级

### 2. 协程池（CoroutinePool）

协程池提供协程的复用和管理：
- 协程创建和销毁
- 协程池大小管理
- 空闲协程缓存
- 协程泄漏检测

```rust
pub struct CoroutinePool {
    pool_size: usize,
    active_coroutines: usize,
    idle_coroutines: VecDeque<Coroutine>,
    coroutine_factory: Box<dyn CoroutineFactory>,
}
```

### 3. GC 运行时（GcRuntime）

GC 运行时与 gc-optimizer 集成：
- GC 触发和执行
- GC 统计信息
- 自适应 GC 调度
- 内存压力监控

```rust
pub struct GcRuntime {
    pub gc: Arc<OptimizedGc>,
    pub stats: Arc<RwLock<GcRuntimeStats>>,
    pub enabled: Arc<AtomicBool>,
}
```

#### GC 集成特性

- 无锁写屏障（`gc-optimizer`）
- 并行标记
- 增量收集
- 自适应配额管理

### 4. 沙箱虚拟机（SandboxedVm）

沙箱虚拟机提供安全的执行环境：
- 资源限制
- 系统调用过滤
- 文件系统隔离
- 网络隔离

```rust
pub struct SandboxedVm {
    config: SandboxConfig,
    resource_limits: ResourceLimits,
    syscall_filter: SyscallFilter,
    isolation: IsolationLevel,
}
```

#### 沙箱特性

- **CPU 限制**：vCPU 数量和配额
- **内存限制**：最大内存使用量
- **磁盘限制**：磁盘 I/O 限制
- **网络限制**：网络带宽和连接限制

### 5. vCPU 协程映射器（VCpuCoroutineMapper）

vCPU 到协程的映射：
- vCPU 分配
- 协程绑定
- 亲和性管理
- 迁移支持

```rust
pub struct VCpuCoroutineMapper {
    vcpu_count: u32,
    vcpu_to_coroutine: HashMap<u32, u64>,
    coroutine_to_vcpu: HashMap<u64, u32>,
    affinity_map: HashMap<u32, Vec<u32>>,
}
```

## 协程模型

### 协程类型

| 类型 | 描述 | 优先级 |
|------|--------|--------|
| 高优先级协程 | JIT 编译、GC | High |
| 普通协程 | vCPU 执行 | Medium |
| 低优先级协程 | I/O 任务 | Low |

### 协程状态

```rust
pub enum CoroutineState {
    Ready,        // 就绪
    Running,      // 运行中
    Blocked,       // 阻塞
    Suspended,     // 挂起
    Terminated,   // 终止
}
```

### 协程切换

- 协程保存和恢复
- 上下文切换优化
- 栈管理
- 寄存器保存

## GC 集成

### GC 触发策略

```rust
pub enum GcTriggerPolicy {
    FixedThreshold,           // 固定阈值触发
    Adaptive,                 // 自适应触发
    MemoryPressure,         // 内存压力触发
    TimeBased,              // 基于时间触发
}
```

### GC 执行流程

```
Memory Allocation -> Write Barrier -> Mark Phase -> Sweep Phase -> Compaction
                   |                |             |              |
                   v                v             v              v
            Track Objects    Mark Live    Reclaim Dead    Compact Heap
```

## 调度算法

### 优先级队列调度

```rust
pub struct PriorityScheduler {
    high_priority: VecDeque<Coroutine>,
    medium_priority: VecDeque<Coroutine>,
    low_priority: VecDeque<Coroutine>,
}
```

### 负载均衡

- CPU 负载跟踪
- 动态 vCPU 分配
- 迁移决策
- 热点检测

## 沙箱安全

### 资源限制

```rust
pub struct ResourceLimits {
    pub max_memory: u64,
    pub max_cpu_time: Duration,
    pub max_disk_io: u64,
    pub max_network_bandwidth: u64,
}
```

### 系统调用过滤

```rust
pub struct SyscallFilter {
    pub allowed_syscalls: HashSet<i32>,
    pub denied_syscalls: HashSet<i32>,
    pub default_action: FilterAction,
}
```

### 隔离级别

```rust
pub enum IsolationLevel {
    None,           // 无隔离
    Process,        // 进程隔离
    Container,      // 容器隔离
    Full,           // 完全隔离
}
```

## 配置选项

### 协程调度器配置

```rust
pub struct SchedulerConfig {
    pub max_coroutines: usize,
    pub ready_queue_size: usize,
    pub time_slice: Duration,
    pub preemptive: bool,
}
```

### 沙箱配置

```rust
pub struct SandboxConfig {
    pub enable_seccomp: bool,
    pub enable_namespaces: bool,
    pub enable_cgroups: bool,
    pub isolation_level: IsolationLevel,
}
```

## 使用示例

### 创建协程调度器

```rust
use vm_runtime::{CoroutineScheduler, SchedulerConfig};

let config = SchedulerConfig::default();
let scheduler = CoroutineScheduler::new(config)?;

scheduler.start()?;
```

### 执行协程

```rust
use vm_runtime::{Coroutine, CoroutineState};

let coroutine = Coroutine::new("vcpu-0".to_string())?;
scheduler.schedule(coroutine)?;

while coroutine.state() != CoroutineState::Terminated {
    scheduler.run_one()?;
}
```

### 使用 GC 运行时

```rust
use vm_runtime::{GcRuntime, gc_optimizer::OptimizedGc};

let gc = Arc::new(OptimizedGc::new()?);
let gc_runtime = GcRuntime::new(gc)?;

if gc_runtime.check_and_run_gc_step() {
    println!("GC was executed");
}
```

### 创建沙箱虚拟机

```rust
use vm_runtime::{SandboxedVm, SandboxConfig, ResourceLimits};

let config = SandboxConfig::default();
let limits = ResourceLimits {
    max_memory: 1024 * 1024 * 1024, // 1GB
    ..Default::default()
};

let sandbox = SandboxedVm::new(config, limits)?;
```

## 性能优化

### 1. 协程调度优化

- 上下文切换最小化
- 协程池复用
- 负载感知调度
- 亲和性优化

### 2. GC 优化

- 增量收集
- 并行标记
- 写屏障优化
- 内存压缩

### 3. 沙箱优化

- 轻量级系统调用过滤
- 高效资源跟踪
- 延迟资源分配

## 监控和统计

### 协程统计

```rust
pub struct SchedulerStats {
    pub total_coroutines: usize,
    pub active_coroutines: usize,
    pub context_switches: u64,
    pub cpu_utilization: f64,
}
```

### GC 统计

```rust
pub struct GcRuntimeStats {
    pub heap_usage: u64,
    pub gc_count: u64,
    pub gc_time: Duration,
    pub pause_time: Duration,
}
```

## 测试策略

- 单元测试：组件隔离测试
- 集成测试：协程和 GC 集成
- 性能测试：调度性能测试
- 压力测试：高负载测试
- 安全测试：沙箱绕过测试

## 与其他模块的交互

| 模块 | 交互方式 |
|------|----------|
| `vm-engine-jit` | JIT 编译任务调度 |
| `gc-optimizer` | GC 优化器集成 |
| `vm-core` | VM 状态管理 |
| `vm-mem` | 内存分配和回收 |
| `vm-accel` | 硬件加速集成 |

## 最佳实践

1. **协程复用**：使用协程池减少创建开销
2. **优先级管理**：合理设置任务优先级
3. **GC 调度**：避免在关键路径执行 GC
4. **沙箱配置**：根据需求选择隔离级别
5. **资源限制**：设置合理的资源限制

## 未来改进方向

1. 实现工作窃取调度器
2. 增强沙箱安全功能
3. 优化 GC 性能
4. 改进协程迁移策略
5. 增加更多监控指标
6. 支持更多隔离技术
