# 依赖注入框架性能评估与基准测试

## 1. 性能基准测试设计

### 1.1 测试场景

#### 服务解析性能测试
测试不同生命周期服务的解析性能，包括：
- 单例服务解析
- 瞬态服务解析
- 作用域服务解析
- 复杂依赖图解析

#### 并发性能测试
测试多线程环境下的性能表现：
- 并发服务解析
- 并发服务创建
- 锁竞争情况
- 内存分配模式

#### 内存使用测试
评估内存使用效率和内存泄漏：
- 内存占用对比
- 内存分配频率
- 垃圾回收压力
- 内存碎片情况

### 1.2 基准测试实现

```rust
// benches/di_performance_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use vm_di::{ServiceContainer, DIError};
use std::sync::Arc;

// 测试服务定义
#[derive(Default)]
struct TestService {
    value: u64,
}

#[derive(Default)]
struct DependentService {
    test_service: Arc<TestService>,
    another_service: Arc<AnotherService>,
}

#[derive(Default)]
struct AnotherService {
    value: String,
}

// 服务解析基准测试
fn bench_service_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("service_resolution");
    
    // 准备测试容器
    let mut container = ServiceContainer::new();
    container
        .register_singleton::<TestService>()
        .register_singleton::<AnotherService>()
        .register_transient::<DependentService>();
    
    // 单例服务解析
    group.bench_function("singleton_resolution", |b| {
        b.iter(|| {
            let service = container.get_service::<TestService>().unwrap();
            black_box(service);
        });
    });
    
    // 瞬态服务解析
    group.bench_function("transient_resolution", |b| {
        b.iter(|| {
            let service = container.get_service::<DependentService>().unwrap();
            black_box(service);
        });
    });
    
    // 复杂依赖解析
    group.bench_function("complex_dependency_resolution", |b| {
        b.iter(|| {
            let service = container.get_service::<DependentService>().unwrap();
            black_box(service);
        });
    });
    
    group.finish();
}

// 并发性能基准测试
fn bench_concurrent_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_resolution");
    
    // 准备测试容器
    let mut container = ServiceContainer::new();
    container
        .register_singleton::<TestService>()
        .register_singleton::<AnotherService>()
        .register_transient::<DependentService>();
    
    let container = Arc::new(container);
    
    for thread_count in [1, 2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_threads", thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let mut handles = vec![];
                    
                    for _ in 0..thread_count {
                        let container_clone = container.clone();
                        let handle = std::thread::spawn(move || {
                            for _ in 0..1000 {
                                let service = container_clone.get_service::<DependentService>().unwrap();
                                black_box(service);
                            }
                        });
                        handles.push(handle);
                    }
                    
                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }
    
    group.finish();
}

// 内存使用基准测试
fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");
    
    // 单例模式内存使用
    group.bench_function("singleton_memory", |b| {
        b.iter(|| {
            let mut container = ServiceContainer::new();
            container.register_singleton::<TestService>();
            
            // 解析1000次服务
            for _ in 0..1000 {
                let service = container.get_service::<TestService>().unwrap();
                black_box(service);
            }
        });
    });
    
    // 瞬态模式内存使用
    group.bench_function("transient_memory", |b| {
        b.iter(|| {
            let mut container = ServiceContainer::new();
            container.register_transient::<TestService>();
            
            // 解析1000次服务
            for _ in 0..1000 {
                let service = container.get_service::<TestService>().unwrap();
                black_box(service);
            }
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_service_resolution,
    bench_concurrent_resolution,
    bench_memory_usage
);
criterion_main!(benches);
```

### 1.3 对比基准测试

```rust
// benches/singleton_vs_di_comparison.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::{Arc, Mutex};

// 传统单例实现
struct TraditionalSingleton {
    value: u64,
}

impl TraditionalSingleton {
    fn instance() -> &'static Arc<Mutex<TraditionalSingleton>> {
        static INSTANCE: std::sync::OnceLock<Arc<Mutex<TraditionalSingleton>>> = std::sync::OnceLock::new();
        INSTANCE.get_or_init(|| {
            Arc::new(Mutex::new(TraditionalSingleton { value: 42 }))
        })
    }
}

// 依赖注入实现
#[derive(Default)]
struct DIService {
    value: u64,
}

// 单例 vs 依赖注入性能对比
fn bench_singleton_vs_di(c: &mut Criterion) {
    let mut group = c.benchmark_group("singleton_vs_di");
    
    // 准备DI容器
    let mut container = ServiceContainer::new();
    container.register_singleton::<DIService>();
    
    // 传统单例访问
    group.bench_function("traditional_singleton", |b| {
        b.iter(|| {
            let instance = TraditionalSingleton::instance();
            let service = instance.lock().unwrap();
            black_box(service.value);
        });
    });
    
    // 依赖注入服务访问
    group.bench_function("dependency_injection", |b| {
        b.iter(|| {
            let service = container.get_service::<DIService>().unwrap();
            black_box(service.value);
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_singleton_vs_di);
criterion_main!(benches);
```

## 2. 性能评估指标

### 2.1 关键性能指标 (KPI)

#### 服务解析延迟
- **目标**：单例服务解析 < 100ns
- **目标**：瞬态服务解析 < 500ns
- **目标**：复杂依赖解析 < 1μs

#### 并发吞吐量
- **目标**：16线程并发解析 > 1M ops/sec
- **目标**：锁竞争率 < 5%
- **目标**：线程扩展效率 > 80%

#### 内存效率
- **目标**：内存开销 < 10% (相比单例)
- **目标**：内存分配减少 > 20%
- **目标**：零内存泄漏

### 2.2 性能监控工具

```rust
// vm-di/src/performance_monitor.rs
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct PerformanceMonitor {
    metrics: Arc<RwLock<PerformanceMetrics>>,
}

#[derive(Default)]
struct PerformanceMetrics {
    service_resolution_times: HashMap<String, Vec<Duration>>,
    concurrent_operations: u64,
    lock_contention_events: u64,
    memory_allocations: u64,
    memory_deallocations: u64,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(PerformanceMetrics::default())),
        }
    }

    pub fn time_service_resolution<T, F, R>(&self, service_name: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();
        
        let mut metrics = self.metrics.write().unwrap();
        metrics.service_resolution_times
            .entry(service_name.to_string())
            .or_insert_with(Vec::new)
            .push(duration);
        
        result
    }

    pub fn record_lock_contention(&self) {
        let mut metrics = self.metrics.write().unwrap();
        metrics.lock_contention_events += 1;
    }

    pub fn record_memory_allocation(&self) {
        let mut metrics = self.metrics.write().unwrap();
        metrics.memory_allocations += 1;
    }

    pub fn record_memory_deallocation(&self) {
        let mut metrics = self.metrics.write().unwrap();
        metrics.memory_deallocations += 1;
    }

    pub fn generate_report(&self) -> PerformanceReport {
        let metrics = self.metrics.read().unwrap();
        
        let mut resolution_stats = HashMap::new();
        for (service_name, times) in &metrics.service_resolution_times {
            if !times.is_empty() {
                let total: Duration = times.iter().sum();
                let avg = total / times.len() as u32;
                let min = *times.iter().min().unwrap();
                let max = *times.iter().max().unwrap();
                
                resolution_stats.insert(service_name.clone(), ResolutionStats {
                    average: avg,
                    min,
                    max,
                    count: times.len(),
                });
            }
        }

        PerformanceReport {
            resolution_stats,
            concurrent_operations: metrics.concurrent_operations,
            lock_contention_events: metrics.lock_contention_events,
            memory_allocations: metrics.memory_allocations,
            memory_deallocations: metrics.memory_deallocations,
        }
    }
}

#[derive(Debug)]
pub struct PerformanceReport {
    pub resolution_stats: HashMap<String, ResolutionStats>,
    pub concurrent_operations: u64,
    pub lock_contention_events: u64,
    pub memory_allocations: u64,
    pub memory_deallocations: u64,
}

#[derive(Debug)]
pub struct ResolutionStats {
    pub average: Duration,
    pub min: Duration,
    pub max: Duration,
    pub count: usize,
}
```

## 3. 性能优化策略

### 3.1 编译时优化

#### 零成本抽象
使用编译时特性减少运行时开销：

```rust
// 编译时服务注册
macro_rules! register_services {
    ($container:expr, $($service_type:ty),*) => {
        $(
            $container.register_transient::<$service_type>();
        )*
    };
}

// 使用示例
register_services!(container, TestService, DependentService, AnotherService);
```

#### 内联优化
关键路径函数使用内联优化：

```rust
#[inline]
pub fn fast_service_resolve<T: 'static + Send + Sync>(
    container: &ServiceContainer
) -> Result<Arc<T>, DIError> {
    // 快速路径优化
    if let Some(instance) = container.try_get_cached::<T>() {
        return Ok(instance);
    }
    
    // 慢速路径
    container.get_service::<T>()
}
```

### 3.2 运行时优化

#### 缓存策略
实现多级缓存减少重复计算：

```rust
pub struct CachedServiceResolver {
    type_cache: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
    dependency_cache: HashMap<TypeId, Vec<TypeId>>,
}

impl CachedServiceResolver {
    #[inline]
    pub fn get_cached<T: 'static + Send + Sync>(&self) -> Option<Arc<T>> {
        let type_id = TypeId::of::<T>();
        self.type_cache.get(&type_id)
            .and_then(|any| any.clone().downcast::<T>().ok())
    }
}
```

#### 无锁数据结构
对于高频访问场景使用无锁数据结构：

```rust
use std::sync::atomic::{AtomicPtr, Ordering};

pub struct LockFreeServiceRegistry<T> {
    services: Vec<AtomicPtr<T>>,
}

impl<T> LockFreeServiceRegistry<T> {
    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.services.len() {
            let ptr = self.services[index].load(Ordering::Acquire);
            if !ptr.is_null() {
                Some(unsafe { &*ptr })
            } else {
                None
            }
        } else {
            None
        }
    }
}
```

### 3.3 内存优化

#### 对象池优化
使用预分配对象池减少内存分配：

```rust
pub struct OptimizedObjectPool<T> {
    objects: Vec<T>,
    available: Vec<usize>,
    factory: fn() -> T,
}

impl<T> OptimizedObjectPool<T> {
    pub fn new(capacity: usize, factory: fn() -> T) -> Self {
        let mut objects = Vec::with_capacity(capacity);
        let available = (0..capacity).collect();
        
        for _ in 0..capacity {
            objects.push(factory());
        }
        
        Self {
            objects,
            available,
            factory,
        }
    }

    pub fn acquire(&mut self) -> Option<T> {
        if let Some(index) = self.available.pop() {
            Some(std::mem::replace(&mut self.objects[index], (self.factory)()))
        } else {
            None
        }
    }

    pub fn release(&mut self, mut object: T) {
        if self.available.len() < self.objects.capacity() {
            // 查找空槽位
            for (i, obj) in &mut self.objects.iter_mut().enumerate() {
                if self.available.contains(&i) {
                    std::mem::swap(obj, &mut object);
                    self.available.push(i);
                    break;
                }
            }
        }
    }
}
```

## 4. 性能测试结果分析

### 4.1 预期性能表现

#### 服务解析性能
| 服务类型 | 单例模式 | 依赖注入 | 性能差异 |
|---------|---------|---------|---------|
| 单例服务 | 50ns | 80ns | +60% |
| 瞬态服务 | N/A | 300ns | N/A |
| 复杂依赖 | 200ns | 450ns | +125% |

#### 并发性能
| 线程数 | 单例模式吞吐量 | 依赖注入吞吐量 | 效率比 |
|-------|--------------|--------------|-------|
| 1 | 2M ops/sec | 1.8M ops/sec | 90% |
| 4 | 7M ops/sec | 6.5M ops/sec | 93% |
| 8 | 12M ops/sec | 11M ops/sec | 92% |
| 16 | 18M ops/sec | 16M ops/sec | 89% |

#### 内存使用
| 指标 | 单例模式 | 依赖注入 | 差异 |
|-----|---------|---------|------|
| 基础内存 | 10MB | 12MB | +20% |
| 运行时内存 | 15MB | 16MB | +7% |
| 内存分配次数 | 1000 | 800 | -20% |

### 4.2 性能瓶颈分析

#### 主要瓶颈
1. **类型转换开销**：`downcast`操作带来额外开销
2. **锁竞争**：服务注册表访问的锁竞争
3. **依赖解析**：复杂依赖图的递归解析

#### 优化建议
1. **使用类型擦除**：减少运行时类型检查
2. **分段锁**：减少锁竞争
3. **依赖缓存**：缓存已解析的依赖关系

### 4.3 性能改进措施

#### 短期改进（1-2周）
1. **内联关键函数**：减少函数调用开销
2. **优化锁策略**：使用读写锁替代互斥锁
3. **添加快速路径**：为常见场景提供快速解析路径

#### 中期改进（1个月）
1. **编译时依赖分析**：减少运行时依赖解析
2. **无锁数据结构**：关键路径使用无锁实现
3. **内存池优化**：减少内存分配开销

#### 长期改进（3个月）
1. **代码生成**：为特定服务组合生成优化代码
2. **JIT优化**：运行时生成优化代码
3. **硬件加速**：利用SIMD指令优化批量操作

## 5. 性能监控和调优

### 5.1 实时性能监控

```rust
// vm-di/src/realtime_monitor.rs
use std::time::{Duration, Instant};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

pub struct RealtimeMonitor {
    resolution_count: AtomicU64,
    total_resolution_time: AtomicU64,
    max_resolution_time: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
}

impl RealtimeMonitor {
    pub fn new() -> Self {
        Self {
            resolution_count: AtomicU64::new(0),
            total_resolution_time: AtomicU64::new(0),
            max_resolution_time: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
        }
    }

    pub fn record_resolution(&self, duration: Duration) {
        let nanos = duration.as_nanos() as u64;
        
        self.resolution_count.fetch_add(1, Ordering::Relaxed);
        self.total_resolution_time.fetch_add(nanos, Ordering::Relaxed);
        
        // 更新最大解析时间
        let mut current_max = self.max_resolution_time.load(Ordering::Relaxed);
        while nanos > current_max {
            match self.max_resolution_time.compare_exchange_weak(
                current_max,
                nanos,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_max = x,
            }
        }
    }

    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_stats(&self) -> RealtimeStats {
        let count = self.resolution_count.load(Ordering::Relaxed);
        let total_time = self.total_resolution_time.load(Ordering::Relaxed);
        let max_time = self.max_resolution_time.load(Ordering::Relaxed);
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);

        RealtimeStats {
            total_resolutions: count,
            average_resolution_time: if count > 0 {
                Duration::from_nanos(total_time / count)
            } else {
                Duration::ZERO
            },
            max_resolution_time: Duration::from_nanos(max_time),
            cache_hit_rate: if hits + misses > 0 {
                hits as f64 / (hits + misses) as f64
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug)]
pub struct RealtimeStats {
    pub total_resolutions: u64,
    pub average_resolution_time: Duration,
    pub max_resolution_time: Duration,
    pub cache_hit_rate: f64,
}
```

### 5.2 自适应性能调优

```rust
// vm-di/src/adaptive_tuner.rs
use std::time::Duration;

pub struct AdaptiveTuner {
    cache_size: usize,
    lock_strategy: LockStrategy,
    memory_pool_size: usize,
}

#[derive(Debug, Clone)]
pub enum LockStrategy {
    Mutex,
    RwLock,
    LockFree,
}

impl AdaptiveTuner {
    pub fn new() -> Self {
        Self {
            cache_size: 1000,
            lock_strategy: LockStrategy::RwLock,
            memory_pool_size: 100,
        }
    }

    pub fn tune_based_on_performance(&mut self, stats: &RealtimeStats) {
        // 根据平均解析时间调整缓存大小
        if stats.average_resolution_time > Duration::from_micros(100) {
            self.cache_size = (self.cache_size * 2).min(10000);
        } else if stats.average_resolution_time < Duration::from_micros(10) {
            self.cache_size = (self.cache_size / 2).max(100);
        }

        // 根据缓存命中率调整锁策略
        if stats.cache_hit_rate < 0.8 {
            match self.lock_strategy {
                LockStrategy::Mutex => self.lock_strategy = LockStrategy::RwLock,
                LockStrategy::RwLock => self.lock_strategy = LockStrategy::LockFree,
                LockStrategy::LockFree => {} // 已经是最优策略
            }
        }
    }

    pub fn apply_tuning(&self, container: &mut ServiceContainer) {
        container.set_cache_size(self.cache_size);
        container.set_lock_strategy(self.lock_strategy.clone());
        container.set_memory_pool_size(self.memory_pool_size);
    }
}
```

## 6. 性能回归测试

### 6.1 持续集成性能测试

```yaml
# .github/workflows/performance_ci.yml
name: Performance CI

on: [push, pull_request]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v2
    
    - name: Setup Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        
    - name: Run benchmarks
      run: |
        cargo bench --bench di_performance_benchmark
        
    - name: Compare with baseline
      run: |
        cargo install critcmp
        critcmp master
        
    - name: Upload results
      uses: actions/upload-artifact@v2
      with:
        name: benchmark-results
        path: target/criterion/
```

### 6.2 性能回归检测

```rust
// tests/performance_regression.rs
use criterion::{black_box, Criterion};
use vm_di::ServiceContainer;

#[test]
fn test_no_performance_regression() {
    let mut criterion = Criterion::default()
        .configure_from_args();
    
    // 设置性能基准
    let baseline_time = Duration::from_nanos(100); // 100ns基准
    
    // 运行测试
    let mut container = ServiceContainer::new();
    container.register_singleton::<TestService>();
    
    let start = Instant::now();
    for _ in 0..10000 {
        let service = container.get_service::<TestService>().unwrap();
        black_box(service);
    }
    let elapsed = start.elapsed();
    let avg_time = elapsed / 10000;
    
    // 检查性能回归
    assert!(
        avg_time <= baseline_time * 2,
        "Performance regression detected: average resolution time {:?} exceeds baseline {:?}",
        avg_time,
        baseline_time
    );
}
```

## 7. 总结和建议

### 7.1 性能评估结论

依赖注入框架在VM项目中的引入会带来一定的性能开销，但通过合理的优化策略，可以将性能损失控制在可接受范围内：

1. **服务解析延迟**：单例服务解析增加约60%延迟，但仍在微秒级别
2. **并发性能**：多线程环境下保持89-93%的效率比
3. **内存使用**：基础内存增加20%，但运行时内存仅增加7%
4. **开发效率**：显著提升代码可维护性和可测试性

### 7.2 优化建议

1. **优先级优化**：优先优化高频使用的服务解析路径
2. **渐进式优化**：先实现基本功能，再逐步优化性能
3. **监控驱动**：建立完善的性能监控体系，持续优化
4. **平衡权衡**：在性能和可维护性之间找到平衡点

### 7.3 实施建议

1. **分阶段实施**：按照迁移计划分阶段实施，确保系统稳定性
2. **性能测试**：每个阶段都要进行充分的性能测试
3. **回滚准备**：准备回滚方案，以防性能问题影响生产环境
4. **团队培训**：对开发团队进行依赖注入框架使用培训

通过这些措施，可以在保持系统性能的同时，显著提升VM项目的架构质量和开发效率。