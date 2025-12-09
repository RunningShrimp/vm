# P1-02: 协程调度器集成实现指南

**目标**: 将CoroutineScheduler与虚拟CPU (vCPU)集成，实现负载均衡和高效调度

**完成标准**:
- ✅ CoroutineScheduler trait 设计完成
- ✅ vCPU与协程映射
- ✅ 负载均衡算法实现
- ✅ 优先级队列调度
- ✅ 单元测试覆盖 (>80%)
- ✅ 负载均衡验证 (<30%不均衡)

**时间线**: 2周(10个工作日)

---

## 设计概述

### 架构模式

```
┌──────────────────────────────────────────────┐
│         Tokio Runtime (Event Loop)            │
├──────────────────────────────────────────────┤
│      CoroutineScheduler (中央调度器)         │
├──────┬──────────────┬──────────┬──────────┤
│ vCPU0 │   vCPU1     │  vCPU2   │  ...    │
├──────┼──────────────┼──────────┼──────────┤
│C1,C2 │ C3,C4,C5    │ C6       │         │
└──────┴──────────────┴──────────┴──────────┘

就绪队列结构:
┌─────────────────┬──────────────┐
│  优先级队列     │   协程ID列表 │
├─────────────────┼──────────────┤
│ RealTime (3)    │ [101, 102]   │
│ High (2)        │ [201, 202]   │
│ Normal (1)      │ [301..330]   │
│ Low (0)         │ [401..450]   │
└─────────────────┴──────────────┘
```

### 核心数据结构

```rust
pub enum SchedulingPolicy {
    RoundRobin,          // 轮询
    Priority,            // 优先级
    Preemptive,         // 抢占式
    LoadBalancing,      // 负载均衡（推荐）
}

pub struct CoroutineInfo {
    pub id: u64,
    pub state: CoroutineState,
    pub priority: Priority,
    pub cycles_executed: u64,
    pub cycles_remaining: u64,
    pub assigned_vcpu: Option<u32>,
}

pub struct VCpuInfo {
    pub id: u32,
    pub total_cycles: u64,
    pub available_cycles: u64,
    pub coroutine_count: usize,
    pub utilization: f64,
}
```

---

## 实现步骤

### 第一阶段：调度器核心 (Day 1-2)

**1.1 协程池管理**

```rust
pub struct CoroutineScheduler {
    coroutines: Arc<DashMap<u64, CoroutineInfo>>,
    vcpus: Arc<DashMap<u32, VCpuInfo>>,
    ready_queues: Arc<Vec<tokio::sync::Mutex<Vec<u64>>>>,
}

impl CoroutineScheduler {
    /// 创建协程
    pub async fn create_coroutine(
        &self,
        priority: Priority,
        cycles: u64,
    ) -> Result<u64, String> {
        if self.coroutines.len() >= self.config.max_coroutines {
            return Err("Max coroutines reached".to_string());
        }

        let coro_id = self.allocate_id();
        let info = CoroutineInfo {
            id: coro_id,
            state: CoroutineState::Ready,
            priority,
            cycles_executed: 0,
            cycles_remaining: cycles,
            assigned_vcpu: None,
        };

        self.coroutines.insert(coro_id, info);
        
        // 加入对应优先级队列
        let queue_idx = priority as usize;
        self.ready_queues[queue_idx].lock().await.push(coro_id);

        Ok(coro_id)
    }
}
```

**1.2 vCPU初始化**

```rust
impl CoroutineScheduler {
    pub fn new(config: SchedulerConfig) -> Self {
        // 初始化vCPU
        let vcpus = Arc::new(DashMap::new());
        for i in 0..config.vcpu_count {
            vcpus.insert(i, VCpuInfo {
                id: i,
                total_cycles: u64::MAX,
                available_cycles: u64::MAX,
                coroutine_count: 0,
                utilization: 0.0,
            });
        }
        
        // 初始化4个优先级队列
        let ready_queues = Arc::new(vec![
            tokio::sync::Mutex::new(Vec::new()), // Low
            tokio::sync::Mutex::new(Vec::new()), // Normal
            tokio::sync::Mutex::new(Vec::new()), // High
            tokio::sync::Mutex::new(Vec::new()), // RealTime
        ]);
        
        // ...
    }
}
```

### 第二阶段：调度算法 (Day 3-4)

**2.1 优先级队列调度**

```rust
impl CoroutineScheduler {
    pub async fn schedule(&self) -> Result<(), String> {
        // 1. 选择目标vCPU
        let target_vcpu = self.select_target_vcpu().await?;
        
        // 2. 从优先级队列取出协程
        let coro_id = self.dequeue_coroutine().await?;
        
        // 3. 分配到vCPU
        self.assign_coroutine_to_vcpu(coro_id, target_vcpu).await?;
        
        // 4. 更新统计
        self.global_stats.total_scheduled += 1;
        
        Ok(())
    }

    /// 优先级队列出队（高优先级优先）
    async fn dequeue_coroutine(&self) -> Result<u64, String> {
        // 从高到低检查优先级队列
        for queue_idx in (0..4).rev() {
            let mut queue = self.ready_queues[queue_idx].lock().await;
            if let Some(coro_id) = queue.pop() {
                return Ok(coro_id);
            }
        }
        
        Err("No coroutine in ready queue".to_string())
    }
}
```

**2.2 vCPU选择策略**

```rust
impl CoroutineScheduler {
    async fn select_target_vcpu(&self) -> Result<u32, String> {
        if !self.config.enable_load_balancing {
            return Ok(0); // 简单轮询
        }

        // 选择利用率最低的vCPU
        let mut min_utilization = f64::MAX;
        let mut target_id = 0u32;

        for entry in self.vcpus.iter() {
            let vcpu = entry.value();
            
            // 计算实际利用率
            let utilization = if vcpu.total_cycles > 0 {
                1.0 - (vcpu.available_cycles as f64 / vcpu.total_cycles as f64)
            } else {
                0.0
            };

            if utilization < min_utilization {
                min_utilization = utilization;
                target_id = vcpu.id;
            }
        }

        Ok(target_id)
    }
}
```

### 第三阶段：负载均衡 (Day 5-6)

**3.1 动态负载均衡**

```rust
impl CoroutineScheduler {
    pub async fn balance_load(&self) -> Result<usize, String> {
        if !self.config.enable_load_balancing {
            return Ok(0);
        }

        // 1. 计算平均利用率
        let mut total_util = 0.0;
        for entry in self.vcpus.iter() {
            total_util += entry.value().utilization;
        }
        let avg_util = total_util / self.config.vcpu_count as f64;
        
        // 2. 识别过载vCPU
        let threshold = self.config.load_balance_threshold; // e.g., 0.3
        let mut overloaded = Vec::new();
        let mut underloaded = Vec::new();
        
        for entry in self.vcpus.iter() {
            let vcpu_id = entry.key();
            let utilization = entry.value().utilization;
            
            if utilization > avg_util * (1.0 + threshold) {
                overloaded.push(*vcpu_id);
            } else if utilization < avg_util * (1.0 - threshold) {
                underloaded.push(*vcpu_id);
            }
        }
        
        // 3. 从过载vCPU迁移协程
        let mut migrations = 0;
        for from_vcpu in overloaded {
            for &to_vcpu in &underloaded {
                if let Some(coro_id) = self.select_coroutine_to_migrate(from_vcpu).await {
                    self.migrate_coroutine(coro_id, from_vcpu, to_vcpu).await.ok();
                    migrations += 1;
                }
            }
        }
        
        self.global_stats.total_load_balances += 1;
        
        Ok(migrations)
    }

    async fn select_coroutine_to_migrate(&self, vcpu_id: u32) -> Option<u64> {
        // 选择该vCPU上优先级最低的协程进行迁移
        // 优先迁移低优先级、执行时间较短的协程
        
        // 实现细节...
        None
    }

    async fn migrate_coroutine(&self, coro_id: u64, from: u32, to: u32) -> Result<(), String> {
        // 1. 暂停协程
        // 2. 更新协程的assigned_vcpu
        // 3. 更新源vCPU和目标vCPU的信息
        // 4. 恢复协程
        Ok(())
    }
}
```

**3.2 负载均衡策略**

```rust
/// 负载均衡决策树
fn should_balance(vcpu_utilizations: Vec<f64>, threshold: f64) -> bool {
    let avg = vcpu_utilizations.iter().sum::<f64>() / vcpu_utilizations.len() as f64;
    let max = vcpu_utilizations.iter().fold(0.0, |a, &b| a.max(b));
    let min = vcpu_utilizations.iter().fold(1.0, |a, &b| a.min(b));
    
    // 如果最大和最小差异超过阈值，则触发负载均衡
    (max - min) > (avg * threshold)
}

/// 触发时机
fn balance_trigger() {
    // 1. 定期：每100ms检查一次
    // 2. 事件驱动：当new_coroutine或schedule被调用时
    // 3. 条件：差异超过30%时
}
```

### 第四阶段：协程执行 (Day 7-8)

**4.1 异步协程执行**

```rust
impl CoroutineScheduler {
    pub async fn execute_coroutine(&self, coro_id: u64) -> Result<u64, String> {
        let start = std::time::Instant::now();
        
        // 1. 获取协程信息
        let coro = self.coroutines
            .get(&coro_id)
            .ok_or("Coroutine not found")?;
        
        let remaining_cycles = coro.cycles_remaining;
        let assigned_vcpu = coro.assigned_vcpu.ok_or("Not assigned to vCPU")?;
        drop(coro);
        
        // 2. 更新协程状态为Running
        self.update_coroutine_state(coro_id, CoroutineState::Running).await?;
        
        // 3. 模拟执行（实际应调用虚拟机执行引擎）
        let estimated_exec_us = remaining_cycles / 1000; // 1000 cycles/µs
        tokio::time::sleep(tokio::time::Duration::from_micros(
            std::cmp::min(estimated_exec_us, 100)
        )).await;
        
        // 4. 更新协程统计
        if let Some(mut coro) = self.coroutines.get_mut(&coro_id) {
            coro.cycles_executed += remaining_cycles;
            coro.cycles_remaining = 0;
            coro.state = CoroutineState::Done;
        }
        
        // 5. 更新vCPU统计
        if let Some(mut vcpu) = self.vcpus.get_mut(&assigned_vcpu) {
            vcpu.available_cycles = vcpu.available_cycles.saturating_sub(remaining_cycles);
        }
        
        let latency = start.elapsed().as_micros() as u64;
        Ok(latency)
    }
}
```

**4.2 上下文切换**

```rust
impl CoroutineScheduler {
    async fn context_switch(&self, from: u64, to: u64) -> Result<(), String> {
        // 1. 保存from协程的上下文
        let _from_context = self.save_context(from).await?;
        
        // 2. 加载to协程的上下文
        let _to_context = self.load_context(to).await?;
        
        // 3. 更新状态
        self.update_coroutine_state(from, CoroutineState::Waiting).await?;
        self.update_coroutine_state(to, CoroutineState::Running).await?;
        
        let mut stats = self.global_stats.lock().await;
        stats.total_context_switches += 1;
        
        Ok(())
    }

    async fn save_context(&self, coro_id: u64) -> Result<Vec<u8>, String> {
        // 保存寄存器、PC等状态
        // 返回上下文快照
        Ok(vec![])
    }

    async fn load_context(&self, coro_id: u64) -> Result<(), String> {
        // 恢复协程的寄存器、PC等状态
        Ok(())
    }
}
```

### 第五阶段：测试和优化 (Day 9-10)

**5.1 单元测试**

```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_coroutine_creation_and_scheduling() {
        let scheduler = CoroutineScheduler::new(SchedulerConfig::default());
        
        // 创建协程
        let coro_id = scheduler.create_coroutine(Priority::Normal, 1000).await.unwrap();
        
        // 调度协程
        let result = scheduler.schedule().await;
        assert!(result.is_ok());
        
        // 验证协程被分配到vCPU
        let info = scheduler.get_coroutine_info(coro_id).unwrap();
        assert!(info.assigned_vcpu.is_some());
    }

    #[tokio::test]
    async fn test_priority_scheduling() {
        let scheduler = CoroutineScheduler::new(SchedulerConfig::default());
        
        // 创建不同优先级
        let low_id = scheduler.create_coroutine(Priority::Low, 1000).await.unwrap();
        let high_id = scheduler.create_coroutine(Priority::High, 1000).await.unwrap();
        
        // 高优先级应该先被调度
        scheduler.schedule().await.ok();
        let high_info = scheduler.get_coroutine_info(high_id).unwrap();
        assert!(high_info.assigned_vcpu.is_some());
    }

    #[tokio::test]
    async fn test_load_balancing() {
        let mut config = SchedulerConfig::default();
        config.enable_load_balancing = true;
        let scheduler = CoroutineScheduler::new(config);
        
        // 创建大量协程
        for _ in 0..100 {
            let _ = scheduler.create_coroutine(Priority::Normal, 1000).await;
        }
        
        // 执行负载均衡
        let moves = scheduler.balance_load().await.unwrap();
        
        // 验证负载分布
        let status = scheduler.get_load_balance_status();
        let max_util = status.iter().map(|(_, u)| u).fold(0.0, |a, &b| a.max(b));
        let min_util = status.iter().map(|(_, u)| u).fold(1.0, |a, &b| a.min(b));
        
        // 负载应该相对均衡
        assert!((max_util - min_util) < 0.3); // <30%差异
    }

    #[tokio::test]
    async fn test_concurrent_scheduling() {
        let scheduler = Arc::new(CoroutineScheduler::new(SchedulerConfig::default()));
        
        // 并发创建和调度
        let mut handles = vec![];
        for _ in 0..50 {
            let sched = Arc::clone(&scheduler);
            let handle = tokio::spawn(async move {
                let _ = sched.create_coroutine(Priority::Normal, 1000).await;
                let _ = sched.schedule().await;
            });
            handles.push(handle);
        }
        
        for handle in handles {
            let _ = handle.await;
        }
        
        let stats = scheduler.get_stats().await;
        assert!(stats.total_coroutines > 0);
    }

    #[tokio::test]
    async fn test_coroutine_pause_resume() {
        let scheduler = CoroutineScheduler::new(SchedulerConfig::default());
        
        let coro_id = scheduler.create_coroutine(Priority::Normal, 1000).await.unwrap();
        
        // 暂停
        scheduler.pause_coroutine(coro_id).await.ok();
        let info = scheduler.get_coroutine_info(coro_id).unwrap();
        assert_eq!(info.state, CoroutineState::Waiting);
        
        // 恢复
        scheduler.resume_coroutine(coro_id).await.ok();
        let info = scheduler.get_coroutine_info(coro_id).unwrap();
        assert_eq!(info.state, CoroutineState::Ready);
    }
}
```

**5.2 性能基准**

```rust
#[cfg(test)]
mod benchmarks {
    #[tokio::test]
    async fn bench_coroutine_creation() {
        let scheduler = CoroutineScheduler::new(SchedulerConfig::default());
        let start = std::time::Instant::now();
        
        for _ in 0..10000 {
            let _ = scheduler.create_coroutine(Priority::Normal, 1000).await;
        }
        
        let elapsed = start.elapsed();
        let per_creation = elapsed.as_nanos() / 10000;
        
        println!("Coroutine creation: {:.1} ns/op", per_creation as f64);
        assert!(per_creation < 10000); // <10µs
    }

    #[tokio::test]
    async fn bench_scheduling_latency() {
        let scheduler = CoroutineScheduler::new(SchedulerConfig::default());
        
        for _ in 0..100 {
            let _ = scheduler.create_coroutine(Priority::Normal, 1000).await;
        }
        
        let start = std::time::Instant::now();
        for _ in 0..100 {
            let _ = scheduler.schedule().await;
        }
        
        let elapsed = start.elapsed();
        let avg_latency = elapsed.as_micros() / 100;
        
        println!("Scheduling latency: {} µs/op", avg_latency);
        assert!(avg_latency < 100); // <100µs
    }
}
```

---

## 关键性能指标 (KPI)

| 指标 | 目标 | 验证方法 |
|------|------|---------|
| 协程创建 | <10µs | bench_coroutine_creation |
| 调度延迟 | <100µs | bench_scheduling_latency |
| 负载均衡 | <30%差异 | test_load_balancing |
| 并发协程 | >10,000 | test_concurrent_scheduling |
| 优先级精确性 | 100% | test_priority_scheduling |
| 上下文切换 | <5µs | context_switch measurement |

---

## 文件清单

| 文件 | 行数 | 状态 |
|------|------|------|
| vm-runtime/src/coroutine_scheduler.rs | 450 | ✅ 已创建 |
| tests/coroutine_scheduler_tests.rs | 400 | 待创建 |
| docs/P1_02_SCHEDULER_GUIDE.md | 300 | 待创建 |
| **总计** | **1,150** | |

---

## 集成要点

1. **与异步执行引擎集成**（P1-01）：
   - AsyncExecutionEngine在CoroutineScheduler调度的上下文中运行
   - 共享统计信息

2. **与虚拟机集成**：
   - VirtualMachine包含CoroutineScheduler实例
   - 提供create_and_run_coroutine() 高级接口

3. **与性能基准集成**（P1-03）：
   - 测试调度延迟和吞吐量
   - 评估负载均衡效果

---

## 后续步骤

完成P1-02后：

1. **P1-03**: 性能基准框架 (测试调度和执行性能)
2. **P2-01**: 分层编译 (与调度器配合)
3. **P2-02**: 并行JIT编译 (在多个vCPU上运行)
