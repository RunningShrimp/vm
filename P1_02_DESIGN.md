# P1-02: 协程调度器集成 - 设计文档

## 概述

将GMP(Go Memory Processor)风格的协程调度器集成到虚拟机中，实现高效的多vCPU执行和任务调度。

## 核心概念

### 1. 协程(Coroutine)
轻量级执行单位,代表虚拟机中的执行上下文。

```rust
pub struct Coroutine {
    id: u64,
    gvfn: u64,          // 来宾虚拟函数
    state: CoroutineState,
    local_data: HashMap<String, u64>,
}

pub enum CoroutineState {
    Created,
    Ready,
    Running,
    Waiting(WaitReason),
    Suspended,
    Dead,
}
```

### 2. vCPU(虚拟处理器)
绑定到协程的逻辑处理器。

```rust
pub struct VCPU {
    id: u32,
    current_coroutine: Option<Coroutine>,
    local_queue: VecDeque<Coroutine>,
    state: VCPUState,
}

pub enum VCPUState {
    Idle,
    Running,
    WaitingForWork,
    Halted,
}
```

### 3. 全局调度器
协调所有vCPU和协程。

```rust
pub struct GlobalScheduler {
    vcpus: Vec<VCPU>,
    global_queue: Arc<Mutex<VecDeque<Coroutine>>>,
    work_stealing: bool,
    load_balancer: LoadBalancer,
}
```

## 架构设计

```
┌─────────────────────────────────────────┐
│     Global Scheduler                    │
│  ┌────────────────────────────────────┐ │
│  │  Global Ready Queue (work-stealing)│ │
│  └────────────────────────────────────┘ │
│   ▲           ▲           ▲              │
│   │           │           │              │
└───┼───────────┼───────────┼──────────────┘
    │           │           │
┌───▼──┐  ┌────▼──┐  ┌─────▼──┐
│VCPU0 │  │VCPU1  │  │VCPUn   │
│      │  │       │  │        │
│ ┌──┐ │  │ ┌──┐ │  │ ┌──┐   │
│ │Co│ │  │ │Co│ │  │ │Co│   │
│ │  │ │  │ │  │ │  │ │  │   │
│ │ro│ │  │ │ro│ │  │ │ro│   │
│ │ut│ │  │ │ut│ │  │ │ut│   │
│ │in│ │  │ │in│ │  │ │in│   │
│ │e0│ │  │ │e1│ │  │ │en│   │
│ └──┘ │  │ └──┘ │  │ └──┘   │
│      │  │      │  │        │
└──────┘  └──────┘  └────────┘
```

## 关键特性

### 1. Work Stealing(任务窃取)
当vCPU0空闲时，可以从其他vCPU的队列中窃取任务。

```rust
fn try_steal_work(&mut self) -> Option<Coroutine> {
    for other_vcpu_id in 0..VCPU_COUNT {
        if other_vcpu_id != self.id {
            if let Some(coro) = self.vcpus[other_vcpu_id].local_queue.pop_front() {
                return Some(coro);
            }
        }
    }
    None
}
```

### 2. 负载均衡
跟踪每个vCPU的负载并主动平衡。

```rust
pub struct LoadBalancer {
    vcpu_loads: Vec<f64>,
    threshold: f64,
}

fn balance_load(&self) -> Vec<Migration> {
    // 返回需要迁移的协程列表
}
```

### 3. 局部队列与全局队列
- **局部队列**: vCPU特定的就绪协程
- **全局队列**: 所有vCPU共享的等待协程

```rust
pub struct VCPU {
    local_queue: VecDeque<Coroutine>,  // 快速本地访问
    global_queue: Arc<Mutex<...>>,     // 共享全局队列
}
```

## 实现计划

### Phase 1: 基础协程管理(3-4天)

1. **协程数据结构**
   - 文件: `crate::coroutine::Coroutine`
   - 包含: ID, 状态, 本地数据, 执行上下文
   - 单元测试: 创建, 状态转换

2. **vCPU管理**
   - 文件: `crate::vcpu::VCPU`
   - 包含: 本地队列, 当前协程, 状态
   - 单元测试: 分配, 执行, 切换

3. **基础调度器**
   - 文件: `crate::scheduler::BasicScheduler`
   - 包含: vCPU池, 全局队列
   - 单元测试: 创建, 分配, 调度

### Phase 2: 高级调度(4-5天)

1. **Work Stealing**
   - 实现任务窃取算法
   - 测试: 并发窃取, 原子性

2. **负载均衡**
   - 追踪vCPU负载
   - 定期平衡操作
   - 测试: 负载分布

3. **Context Switch**
   - 协程切换
   - 状态保存/恢复
   - 测试: 快速切换

### Phase 3: 集成与优化(3-4天)

1. **与async-executor集成**
   - 使用execute_block进行实际执行
   - 跟踪执行统计
   - 测试: 端到端执行

2. **性能优化**
   - 基准测试
   - 热路径优化
   - 测试: 性能目标

3. **容错机制**
   - 错误恢复
   - 资源清理
   - 测试: 错误处理

## 数据结构定义

```rust
// 协程
pub struct Coroutine {
    pub id: u64,
    pub gvfn: u64,
    pub state: CoroutineState,
    pub local_data: HashMap<String, u64>,
    pub execution_time_us: u64,
    pub context: ExecutionContext,
}

// vCPU
pub struct VCPU {
    pub id: u32,
    pub current_coroutine: Option<Coroutine>,
    pub local_queue: VecDeque<Coroutine>,
    pub state: VCPUState,
    pub stats: VCPUStats,
}

// vCPU统计
pub struct VCPUStats {
    pub executions: u64,
    pub context_switches: u64,
    pub idle_time_us: u64,
    pub busy_time_us: u64,
}

// 调度器
pub struct Scheduler {
    pub vcpus: Vec<VCPU>,
    pub global_queue: Arc<Mutex<VecDeque<Coroutine>>>,
    pub work_stealing_enabled: bool,
    pub load_balancer: Option<LoadBalancer>,
}
```

## 性能目标

| 指标 | 目标 | 说明 |
|-----|------|-----|
| 上下文切换 | <1 μs | 极少开销 |
| Work stealing | <10 μs | 快速窃取 |
| 负载均衡周期 | 1 ms | 定期检查 |
| 协程创建 | <100 ns | 轻量级创建 |
| 吞吐量 | >1M ops/s | 高吞吐 |

## 测试计划

```rust
#[test]
fn test_coroutine_lifecycle() { }

#[test]
fn test_vcpu_scheduling() { }

#[test]
fn test_work_stealing() { }

#[test]
fn test_load_balancing() { }

#[test]
fn test_concurrent_execution() { }

#[test]
fn test_error_recovery() { }
```

## 与现有系统的集成

### 与async-executor的协作
```
async-executor执行块
        ↓
    协程执行
        ↓
    记录执行时间
        ↓
    调整调度策略
```

### 与GC系统的协作
```
协程执行计数
        ↓
    触发GC检查
        ↓
    安全点协调
        ↓
    所有vCPU暂停
```

### 与MMU系统的协作
```
地址翻译缓存
        ↓
    按vCPU缓存
        ↓
    上下文切换时更新
        ↓
    协程特定缓存
```

## 风险与缓解

| 风险 | 缓解 |
|-----|------|
| 死锁 | 使用超时和死锁检测 |
| 性能下降 | 详细的性能测试 |
| 内存泄漏 | 自动清理机制 |
| 不公平调度 | 定期负载均衡 |

## 时间表

- **第1-2天**: 基础数据结构
- **第3-4天**: vCPU调度
- **第5-7天**: Work stealing
- **第8-10天**: 负载均衡
- **第11-14天**: 集成与优化

**总计**: 2周(与P1-01重叠)
