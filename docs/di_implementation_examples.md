# 依赖注入框架实现示例

## 1. 核心框架实现

### 1.1 服务容器实现

```rust
// vm-di/src/container.rs
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock, Mutex};

pub struct ServiceContainer {
    services: RwLock<HashMap<TypeId, Box<dyn ServiceDescriptor>>>,
    singletons: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
    scopes: Mutex<Vec<Scope>>,
}

impl ServiceContainer {
    pub fn new() -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
            singletons: RwLock::new(HashMap::new()),
            scopes: Mutex::new(Vec::new()),
        }
    }

    pub fn register_transient<T: 'static + Send + Sync>(&self) -> &Self {
        let mut services = self.services.write().unwrap();
        services.insert(
            TypeId::of::<T>(),
            Box::new(TransientDescriptor::<T>::new()),
        );
        self
    }

    pub fn register_singleton<T: 'static + Send + Sync>(&self) -> &Self {
        let mut services = self.services.write().unwrap();
        services.insert(
            TypeId::of::<T>(),
            Box::new(SingletonDescriptor::<T>::new()),
        );
        self
    }

    pub fn register_scoped<T: 'static + Send + Sync>(&self) -> &Self {
        let mut services = self.services.write().unwrap();
        services.insert(
            TypeId::of::<T>(),
            Box::new(ScopedDescriptor::<T>::new()),
        );
        self
    }

    pub fn register_instance<T: 'static + Send + Sync>(&self, instance: T) -> &Self {
        let mut singletons = self.singletons.write().unwrap();
        singletons.insert(TypeId::of::<T>(), Arc::new(instance));
        self
    }

    pub fn get_service<T: 'static + Send + Sync>(&self) -> Result<Arc<T>, DIError> {
        let type_id = TypeId::of::<T>();
        
        // 首先检查单例缓存
        {
            let singletons = self.singletons.read().unwrap();
            if let Some(instance) = singletons.get(&type_id) {
                return Ok(instance.clone().downcast::<T>()
                    .map_err(|_| DIError::InvalidCast)?);
            }
        }

        // 获取服务描述符
        let services = self.services.read().unwrap();
        let descriptor = services.get(&type_id)
            .ok_or(DIError::ServiceNotRegistered)?;

        // 根据生命周期创建实例
        match descriptor.lifetime() {
            ServiceLifetime::Singleton => {
                let instance = descriptor.create_instance(self)?;
                let arc_instance: Arc<T> = instance.downcast::<T>()
                    .map_err(|_| DIError::InvalidCast)?;
                
                // 缓存单例
                let mut singletons = self.singletons.write().unwrap();
                singletons.insert(type_id, arc_instance.clone());
                Ok(arc_instance)
            }
            ServiceLifetime::Scoped => {
                // 从当前作用域获取实例
                let scopes = self.scopes.lock().unwrap();
                if let Some(current_scope) = scopes.last() {
                    current_scope.get_service::<T>()
                } else {
                    Err(DIError::NoActiveScope)
                }
            }
            ServiceLifetime::Transient => {
                let instance = descriptor.create_instance(self)?;
                Ok(instance.downcast::<T>()
                    .map_err(|_| DIError::InvalidCast)?)
            }
        }
    }
}

// 服务生命周期枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceLifetime {
    Singleton,
    Transient,
    Scoped,
}

// 错误类型
#[derive(Debug)]
pub enum DIError {
    ServiceNotRegistered,
    InvalidCast,
    CircularDependency,
    NoActiveScope,
    CreationFailed(String),
}

impl std::fmt::Display for DIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DIError::ServiceNotRegistered => write!(f, "Service not registered"),
            DIError::InvalidCast => write!(f, "Invalid type cast"),
            DIError::CircularDependency => write!(f, "Circular dependency detected"),
            DIError::NoActiveScope => write!(f, "No active scope"),
            DIError::CreationFailed(msg) => write!(f, "Service creation failed: {}", msg),
        }
    }
}

impl std::error::Error for DIError {}
```

### 1.2 服务描述符实现

```rust
// vm-di/src/descriptor.rs
use std::any::{Any, TypeId};
use crate::container::{ServiceContainer, ServiceLifetime, DIError};

pub trait ServiceDescriptor: Send + Sync {
    fn service_type(&self) -> TypeId;
    fn lifetime(&self) -> ServiceLifetime;
    fn create_instance(&self, container: &ServiceContainer) -> Result<Box<dyn Any + Send + Sync>, DIError>;
    fn dependencies(&self) -> Vec<TypeId>;
}

// 瞬态服务描述符
pub struct TransientDescriptor<T: 'static + Send + Sync> {
    phantom: std::marker::PhantomData<T>,
}

impl<T: 'static + Send + Sync> TransientDescriptor<T> {
    pub fn new() -> Self {
        Self {
            phantom: std::marker::PhantomData,
        }
    }
}

impl<T: 'static + Send + Sync> ServiceDescriptor for TransientDescriptor<T>
where
    T: Default,
{
    fn service_type(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn lifetime(&self) -> ServiceLifetime {
        ServiceLifetime::Transient
    }

    fn create_instance(&self, _container: &ServiceContainer) -> Result<Box<dyn Any + Send + Sync>, DIError> {
        let instance = T::default();
        Ok(Box::new(instance))
    }

    fn dependencies(&self) -> Vec<TypeId> {
        Vec::new()
    }
}

// 单例服务描述符
pub struct SingletonDescriptor<T: 'static + Send + Sync> {
    phantom: std::marker::PhantomData<T>,
}

impl<T: 'static + Send + Sync> SingletonDescriptor<T> {
    pub fn new() -> Self {
        Self {
            phantom: std::marker::PhantomData,
        }
    }
}

impl<T: 'static + Send + Sync> ServiceDescriptor for SingletonDescriptor<T>
where
    T: Default,
{
    fn service_type(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn lifetime(&self) -> ServiceLifetime {
        ServiceLifetime::Singleton
    }

    fn create_instance(&self, _container: &ServiceContainer) -> Result<Box<dyn Any + Send + Sync>, DIError> {
        let instance = T::default();
        Ok(Box::new(instance))
    }

    fn dependencies(&self) -> Vec<TypeId> {
        Vec::new()
    }
}

// 作用域服务描述符
pub struct ScopedDescriptor<T: 'static + Send + Sync> {
    phantom: std::marker::PhantomData<T>,
}

impl<T: 'static + Send + Sync> ScopedDescriptor<T> {
    pub fn new() -> Self {
        Self {
            phantom: std::marker::PhantomData,
        }
    }
}

impl<T: 'static + Send + Sync> ServiceDescriptor for ScopedDescriptor<T>
where
    T: Default,
{
    fn service_type(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn lifetime(&self) -> ServiceLifetime {
        ServiceLifetime::Scoped
    }

    fn create_instance(&self, _container: &ServiceContainer) -> Result<Box<dyn Any + Send + Sync>, DIError> {
        let instance = T::default();
        Ok(Box::new(instance))
    }

    fn dependencies(&self) -> Vec<TypeId> {
        Vec::new()
    }
}

// 工厂服务描述符
pub struct FactoryDescriptor<T, F>
where
    T: 'static + Send + Sync,
    F: Fn(&ServiceContainer) -> T + Send + Sync + 'static,
{
    factory: F,
    phantom: std::marker::PhantomData<T>,
    lifetime: ServiceLifetime,
}

impl<T, F> FactoryDescriptor<T, F>
where
    T: 'static + Send + Sync,
    F: Fn(&ServiceContainer) -> T + Send + Sync + 'static,
{
    pub fn new(factory: F, lifetime: ServiceLifetime) -> Self {
        Self {
            factory,
            phantom: std::marker::PhantomData,
            lifetime,
        }
    }
}

impl<T, F> ServiceDescriptor for FactoryDescriptor<T, F>
where
    T: 'static + Send + Sync,
    F: Fn(&ServiceContainer) -> T + Send + Sync + 'static,
{
    fn service_type(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn lifetime(&self) -> ServiceLifetime {
        self.lifetime
    }

    fn create_instance(&self, container: &ServiceContainer) -> Result<Box<dyn Any + Send + Sync>, DIError> {
        let instance = (self.factory)(container);
        Ok(Box::new(instance))
    }

    fn dependencies(&self) -> Vec<TypeId> {
        Vec::new() // 实际实现中应该分析工厂函数的依赖
    }
}
```

### 1.3 作用域管理

```rust
// vm-di/src/scope.rs
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct Scope {
    services: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
    parent: Option<Arc<Scope>>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
            parent: None,
        }
    }

    pub fn with_parent(parent: Arc<Scope>) -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
            parent: Some(parent),
        }
    }

    pub fn get_service<T: 'static + Send + Sync>(&self) -> Result<Arc<T>, crate::DIError> {
        let type_id = TypeId::of::<T>();
        
        // 检查当前作用域
        {
            let services = self.services.read().unwrap();
            if let Some(service) = services.get(&type_id) {
                return service.clone().downcast::<T>()
                    .map_err(|_| crate::DIError::InvalidCast);
            }
        }

        // 检查父作用域
        if let Some(parent) = &self.parent {
            return parent.get_service::<T>();
        }

        Err(crate::DIError::ServiceNotRegistered)
    }

    pub fn register_service<T: 'static + Send + Sync>(&self, service: T) {
        let type_id = TypeId::of::<T>();
        let mut services = self.services.write().unwrap();
        services.insert(type_id, Arc::new(service));
    }

    pub fn register_arc_service<T: 'static + Send + Sync>(&self, service: Arc<T>) {
        let type_id = TypeId::of::<T>();
        let mut services = self.services.write().unwrap();
        services.insert(type_id, service);
    }
}
```

## 2. VM项目具体应用示例

### 2.1 JIT编译器依赖注入改造

```rust
// vm-engine-jit/src/di_jit.rs
use vm_di::{ServiceContainer, DIError};
use std::sync::Arc;

// 重构后的JIT编译器，使用依赖注入
pub struct DIJitCompiler {
    cache: Arc<dyn CodeCache>,
    runtime_manager: Arc<JitRuntimeManager>,
    optimizer: Option<Arc<dyn InstructionOptimizer>>,
    numa_optimizer: Arc<NUMAOptimizer>,
    event_bus: Arc<dyn EventBus>,
}

impl DIJitCompiler {
    pub fn new(
        cache: Arc<dyn CodeCache>,
        runtime_manager: Arc<JitRuntimeManager>,
        optimizer: Option<Arc<dyn InstructionOptimizer>>,
        numa_optimizer: Arc<NUMAOptimizer>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            cache,
            runtime_manager,
            optimizer,
            numa_optimizer,
            event_bus,
        }
    }

    pub fn compile(&self, ir_block: &IRBlock) -> Result<CompiledCode, CompileError> {
        // 使用注入的依赖进行编译
        let optimized_ir = if let Some(optimizer) = &self.optimizer {
            optimizer.optimize(ir_block)?
        } else {
            ir_block.clone()
        };

        // 使用NUMA优化器选择最佳编译节点
        let node_id = self.numa_optimizer.select_adaptive_node(
            optimized_ir.size(), 
            None
        )?;

        // 使用缓存检查是否已编译
        if let Some(cached_code) = self.cache.get(&optimized_ir.hash()) {
            self.event_bus.publish(JitEvent::CacheHit {
                ir_hash: optimized_ir.hash(),
            });
            return Ok(cached_code);
        }

        // 执行编译
        let compiled_code = self.runtime_manager.compile_on_node(
            &optimized_ir, 
            node_id
        )?;

        // 缓存编译结果
        self.cache.insert(optimized_ir.hash(), compiled_code.clone());

        // 发布编译完成事件
        self.event_bus.publish(JitEvent::CompilationCompleted {
            ir_hash: optimized_ir.hash(),
            code_size: compiled_code.len(),
            node_id,
        });

        Ok(compiled_code)
    }
}

// 依赖注入配置函数
pub fn configure_jit_services(container: &mut ServiceContainer) -> Result<(), DIError> {
    // 注册核心服务
    container
        .register_singleton::<UnifiedCodeCache>()
        .register_singleton::<JitRuntimeManager>()
        .register_singleton::<NUMAOptimizer>()
        .register_singleton::<UnifiedEventBus>()
        .register_transient::<DIJitCompiler>();

    // 注册工厂函数创建JIT编译器
    container.register_factory(|provider| {
        let cache = provider.get_service::<dyn CodeCache>()?;
        let runtime_manager = provider.get_service::<JitRuntimeManager>()?;
        let numa_optimizer = provider.get_service::<NUMAOptimizer>()?;
        let event_bus = provider.get_service::<dyn EventBus>()?;
        
        // 优化器是可选的
        let optimizer = provider.get_service::<dyn InstructionOptimizer>().ok();

        Arc::new(DIJitCompiler::new(
            cache,
            runtime_manager,
            optimizer,
            numa_optimizer,
            event_bus,
        ))
    });

    Ok(())
}
```

### 2.2 虚拟机状态管理改造

```rust
// vm-core/src/di_vm_state.rs
use vm_di::{ServiceContainer, DIError};
use std::sync::Arc;

// 重构后的虚拟机状态，使用依赖注入
pub struct DIVirtualMachineState {
    mmu: Arc<dyn MMU>,
    snapshot_manager: Arc<SnapshotManager>,
    template_manager: Arc<TemplateManager>,
    event_bus: Arc<dyn EventBus>,
    state_notifier: Arc<StateNotifier>,
}

impl DIVirtualMachineState {
    pub fn new(
        mmu: Arc<dyn MMU>,
        snapshot_manager: Arc<SnapshotManager>,
        template_manager: Arc<TemplateManager>,
        event_bus: Arc<dyn EventBus>,
        state_notifier: Arc<StateNotifier>,
    ) -> Self {
        Self {
            mmu,
            snapshot_manager,
            template_manager,
            event_bus,
            state_notifier,
        }
    }

    pub fn create_snapshot(&self, name: &str) -> Result<SnapshotId, StateError> {
        let snapshot_id = self.snapshot_manager.create_snapshot(name)?;
        
        // 发布快照创建事件
        self.event_bus.publish(VmEvent::SnapshotCreated {
            id: snapshot_id,
            name: name.to_string(),
        });

        // 通知状态变更
        self.state_notifier.notify_state_change(StateChange::SnapshotCreated(snapshot_id));

        Ok(snapshot_id)
    }

    pub fn restore_snapshot(&self, snapshot_id: SnapshotId) -> Result<(), StateError> {
        let snapshot = self.snapshot_manager.get_snapshot(snapshot_id)?;
        
        // 恢复MMU状态
        self.mmu.restore_state(&snapshot.mmu_state)?;
        
        // 发布快照恢复事件
        self.event_bus.publish(VmEvent::SnapshotRestored {
            id: snapshot_id,
        });

        // 通知状态变更
        self.state_notifier.notify_state_change(StateChange::SnapshotRestored(snapshot_id));

        Ok(())
    }
}

// 状态通知器
pub trait StateNotifier: Send + Sync {
    fn notify_state_change(&self, change: StateChange);
}

pub enum StateChange {
    SnapshotCreated(SnapshotId),
    SnapshotRestored(SnapshotId),
    MemoryUpdated(GuestAddr, usize),
    RegisterChanged(RegisterId, u64),
}

// 实现状态通知器
pub struct EventBusStateNotifier {
    event_bus: Arc<dyn EventBus>,
}

impl EventBusStateNotifier {
    pub fn new(event_bus: Arc<dyn EventBus>) -> Self {
        Self { event_bus }
    }
}

impl StateNotifier for EventBusStateNotifier {
    fn notify_state_change(&self, change: StateChange) {
        self.event_bus.publish(VmEvent::StateChanged { change });
    }
}

// 依赖注入配置函数
pub fn configure_vm_state_services(container: &mut ServiceContainer) -> Result<(), DIError> {
    container
        .register_singleton::<AsyncMMU>()
        .register_singleton::<SnapshotManager>()
        .register_singleton::<TemplateManager>()
        .register_singleton::<UnifiedEventBus>()
        .register_singleton::<EventBusStateNotifier>()
        .register_transient::<DIVirtualMachineState>();

    // 注册工厂函数创建虚拟机状态
    container.register_factory(|provider| {
        let mmu = provider.get_service::<dyn MMU>()?;
        let snapshot_manager = provider.get_service::<SnapshotManager>()?;
        let template_manager = provider.get_service::<TemplateManager>()?;
        let event_bus = provider.get_service::<dyn EventBus>()?;
        let state_notifier = provider.get_service::<StateNotifier>()?;

        Arc::new(DIVirtualMachineState::new(
            mmu,
            snapshot_manager,
            template_manager,
            event_bus,
            state_notifier,
        ))
    });

    Ok(())
}
```

### 2.3 事件总线依赖注入改造

```rust
// vm-core/src/di_event_bus.rs
use vm_di::{ServiceContainer, DIError};
use std::sync::Arc;

// 统一事件总线接口
pub trait EventBus: Send + Sync {
    fn publish<T: Event>(&self, event: T);
    fn subscribe<T: Event>(&self, handler: Arc<dyn EventHandler<T>>);
    fn unsubscribe<T: Event>(&self, handler: &Arc<dyn EventHandler<T>>);
}

// 事件特征
pub trait Event: Send + Sync + Clone + 'static {
    fn event_type(&self) -> &'static str;
}

// 事件处理器特征
pub trait EventHandler<T: Event>: Send + Sync {
    fn handle(&self, event: &T);
}

// 依赖注入友好的统一事件总线实现
pub struct DIUnifiedEventBus {
    subscribers: Arc<RwLock<HashMap<TypeId, Vec<Box<dyn Any + Send + Sync>>>>>,
    event_history: Arc<RwLock<Vec<Box<dyn Event>>>>,
    max_history_size: usize,
}

impl DIUnifiedEventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            event_history: Arc::new(RwLock::new(Vec::new())),
            max_history_size: 1000,
        }
    }

    pub fn with_history_size(max_history_size: usize) -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            event_history: Arc::new(RwLock::new(Vec::new())),
            max_history_size,
        }
    }
}

impl EventBus for DIUnifiedEventBus {
    fn publish<T: Event>(&self, event: T) {
        let type_id = TypeId::of::<T>();
        
        // 记录事件历史
        {
            let mut history = self.event_history.write().unwrap();
            history.push(Box::new(event.clone()));
            
            // 限制历史记录大小
            if history.len() > self.max_history_size {
                history.remove(0);
            }
        }

        // 通知订阅者
        let subscribers = self.subscribers.read().unwrap();
        if let Some(handlers) = subscribers.get(&type_id) {
            for handler in handlers {
                if let Some(typed_handler) = handler.downcast_ref::<Arc<dyn EventHandler<T>>>() {
                    typed_handler.handle(&event);
                }
            }
        }
    }

    fn subscribe<T: Event>(&self, handler: Arc<dyn EventHandler<T>>) {
        let type_id = TypeId::of::<T>();
        let mut subscribers = self.subscribers.write().unwrap();
        
        subscribers
            .entry(type_id)
            .or_insert_with(Vec::new)
            .push(Box::new(handler));
    }

    fn unsubscribe<T: Event>(&self, handler: &Arc<dyn EventHandler<T>>) {
        let type_id = TypeId::of::<T>();
        let mut subscribers = self.subscribers.write().unwrap();
        
        if let Some(handlers) = subscribers.get_mut(&type_id) {
            handlers.retain(|h| {
                if let Some(typed_handler) = h.downcast_ref::<Arc<dyn EventHandler<T>>>() {
                    !Arc::ptr_eq(typed_handler, handler)
                } else {
                    true
                }
            });
        }
    }
}

// 依赖注入配置函数
pub fn configure_event_bus_services(container: &mut ServiceContainer) -> Result<(), DIError> {
    container
        .register_singleton::<DIUnifiedEventBus>()
        .register_factory(|provider| {
            let event_bus = provider.get_service::<DIUnifiedEventBus>()?;
            event_bus as Arc<dyn EventBus>
        });

    Ok(())
}
```

## 3. 应用程序启动和配置

### 3.1 主应用程序配置

```rust
// vm-main/src/di_main.rs
use vm_di::ServiceContainer;
use std::sync::Arc;

pub struct VMApplication {
    container: Arc<ServiceContainer>,
}

impl VMApplication {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut container = ServiceContainer::new();
        
        // 配置所有服务
        Self::configure_services(&mut container)?;
        
        Ok(Self {
            container: Arc::new(container),
        })
    }

    fn configure_services(container: &mut ServiceContainer) -> Result<(), Box<dyn std::error::Error>> {
        // 配置基础设施服务
        configure_infrastructure_services(container)?;
        
        // 配置事件总线
        vm_core::di_event_bus::configure_event_bus_services(container)?;
        
        // 配置虚拟机状态管理
        vm_core::di_vm_state::configure_vm_state_services(container)?;
        
        // 配置JIT编译器
        vm_engine_jit::di_jit::configure_jit_services(container)?;
        
        // 配置硬件加速器
        vm_accel::di_accel::configure_accel_services(container)?;
        
        // 配置跨架构支持
        vm_cross_arch::di_cross_arch::configure_cross_arch_services(container)?;
        
        Ok(())
    }

    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        // 获取主要服务
        let vm_state = self.container.get_service::<DIVirtualMachineState>()?;
        let jit_compiler = self.container.get_service::<DIJitCompiler>()?;
        let event_bus = self.container.get_service::<dyn EventBus>()?;

        // 创建虚拟机实例
        let vm = VirtualMachine::new(vm_state, jit_compiler, event_bus)?;
        
        // 运行虚拟机
        vm.run()
    }

    pub fn create_scope(&self) -> Result<vm_di::Scope, vm_di::DIError> {
        self.container.create_scope()
    }
}

fn configure_infrastructure_services(container: &mut ServiceContainer) -> Result<(), Box<dyn std::error::Error>> {
    // 配置日志服务
    container.register_singleton::<Logger>();
    
    // 配置配置管理器
    container.register_singleton::<ConfigManager>();
    
    // 配置线程池
    container.register_factory(|provider| {
        let config = provider.get_service::<ConfigManager>()?;
        Arc::new(ThreadPool::new(config.thread_pool_size()))
    });
    
    Ok(())
}
```

### 3.2 测试配置

```rust
// tests/di_test_setup.rs
use vm_di::ServiceContainer;
use std::sync::Arc;

pub fn create_test_container() -> Arc<ServiceContainer> {
    let mut container = ServiceContainer::new();
    
    // 注册模拟服务
    container
        .register_instance(MockCodeCache::new())
        .register_instance(MockJitRuntimeManager::new())
        .register_instance(MockEventBus::new())
        .register_instance(MockMMU::new())
        .register_instance(MockSnapshotManager::new())
        .register_instance(MockNUMAOptimizer::new());
    
    Arc::new(container)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_jit_compilation_with_di() {
        let container = create_test_container();
        
        // 创建JIT编译器
        let jit_compiler = container.get_service::<DIJitCompiler>().unwrap();
        
        // 测试编译
        let ir_block = create_test_ir_block();
        let result = jit_compiler.compile(&ir_block);
        
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_vm_state_with_di() {
        let container = create_test_container();
        
        // 创建虚拟机状态
        let vm_state = container.get_service::<DIVirtualMachineState>().unwrap();
        
        // 测试快照创建和恢复
        let snapshot_id = vm_state.create_snapshot("test").unwrap();
        let result = vm_state.restore_snapshot(snapshot_id);
        
        assert!(result.is_ok());
    }
}
```

## 4. 性能优化实现

### 4.1 延迟初始化服务

```rust
// vm-di/src/lazy_service.rs
use std::sync::{Arc, Mutex};
use std::any::Any;

pub struct LazyService<T> {
    factory: Arc<dyn Fn() -> T + Send + Sync>,
    instance: Arc<Mutex<Option<T>>>,
}

impl<T> LazyService<T> {
    pub fn new<F>(factory: F) -> Self 
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            factory: Arc::new(factory),
            instance: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get(&self) -> &T {
        let mut instance = self.instance.lock().unwrap();
        if instance.is_none() {
            *instance = Some((self.factory)());
        }
        instance.as_ref().unwrap()
    }
}

// 在服务容器中使用延迟初始化
impl ServiceContainer {
    pub fn register_lazy<T: 'static + Send + Sync, F>(&self, factory: F) -> &Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        let lazy_service = Arc::new(LazyService::new(factory));
        self.register_instance(lazy_service)
    }
}
```

### 4.2 对象池实现

```rust
// vm-di/src/object_pool.rs
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

pub struct ObjectPool<T> {
    objects: Arc<Mutex<VecDeque<T>>>,
    factory: Arc<dyn Fn() -> T + Send + Sync>,
    reset_fn: Option<Arc<dyn Fn(&mut T) + Send + Sync>>,
    max_size: usize,
}

impl<T> ObjectPool<T> {
    pub fn new<F>(factory: F, max_size: usize) -> Self 
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            objects: Arc::new(Mutex::new(VecDeque::new())),
            factory: Arc::new(factory),
            reset_fn: None,
            max_size,
        }
    }

    pub fn with_reset<F, R>(factory: F, reset_fn: R, max_size: usize) -> Self 
    where
        F: Fn() -> T + Send + Sync + 'static,
        R: Fn(&mut T) + Send + Sync + 'static,
    {
        Self {
            objects: Arc::new(Mutex::new(VecDeque::new())),
            factory: Arc::new(factory),
            reset_fn: Some(Arc::new(reset_fn)),
            max_size,
        }
    }

    pub fn acquire(&self) -> PooledObject<T> {
        let mut objects = self.objects.lock().unwrap();
        let object = objects.pop_front().unwrap_or_else(|| (self.factory)());
        
        PooledObject {
            object: Some(object),
            pool: self.clone(),
        }
    }

    pub fn release(&self, mut object: T) {
        if let Some(reset_fn) = &self.reset_fn {
            reset_fn(&mut object);
        }
        
        let mut objects = self.objects.lock().unwrap();
        if objects.len() < self.max_size {
            objects.push_back(object);
        }
    }
}

impl<T> Clone for ObjectPool<T> {
    fn clone(&self) -> Self {
        Self {
            objects: self.objects.clone(),
            factory: self.factory.clone(),
            reset_fn: self.reset_fn.clone(),
            max_size: self.max_size,
        }
    }
}

pub struct PooledObject<T> {
    object: Option<T>,
    pool: ObjectPool<T>,
}

impl<T> std::ops::Deref for PooledObject<T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.object.as_ref().unwrap()
    }
}

impl<T> std::ops::DerefMut for PooledObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.object.as_mut().unwrap()
    }
}

impl<T> Drop for PooledObject<T> {
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            self.pool.release(object);
        }
    }
}
```

## 5. 迁移工具

### 5.1 单例适配器

```rust
// vm-di/src/singleton_adapter.rs
use std::sync::Arc;

/// 单例适配器，用于将现有单例包装为依赖注入服务
pub struct SingletonAdapter<T> {
    instance: Arc<T>,
}

impl<T> SingletonAdapter<T> {
    pub fn new(instance: T) -> Self {
        Self {
            instance: Arc::new(instance),
        }
    }
    
    pub fn get_instance(&self) -> &T {
        &self.instance
    }
    
    pub fn into_arc(self) -> Arc<T> {
        self.instance
    }
}

/// 为现有单例创建适配器的宏
#[macro_export]
macro_rules! adapt_singleton {
    ($type:ty, $instance:expr) => {
        $crate::SingletonAdapter::new($instance)
    };
}

/// 使用示例
pub fn adapt_existing_singletons(container: &mut ServiceContainer) {
    // 适配现有的单例
    let old_jit = adapt_singleton!(OldJitCompiler, OldJitCompiler::instance());
    container.register_instance(old_jit.into_arc());
    
    let old_event_bus = adapt_singleton!(OldEventBus, OldEventBus::instance());
    container.register_instance(old_event_bus.into_arc());
}
```

### 5.2 迁移辅助工具

```rust
// vm-di/src/migration_tools.rs
use std::collections::HashMap;
use std::any::TypeId;

/// 迁移计划，记录需要迁移的单例
pub struct MigrationPlan {
    singletons_to_migrate: HashMap<TypeId, SingletonMigration>,
}

pub struct SingletonMigration {
    pub type_name: String,
    pub current_implementation: String,
    pub target_implementation: String,
    pub migration_priority: MigrationPriority,
    pub dependencies: Vec<TypeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationPriority {
    High,
    Medium,
    Low,
}

impl MigrationPlan {
    pub fn new() -> Self {
        Self {
            singletons_to_migrate: HashMap::new(),
        }
    }

    pub fn add_migration<T: 'static>(&mut self, migration: SingletonMigration) {
        self.singletons_to_migrate.insert(TypeId::of::<T>(), migration);
    }

    pub fn get_migration_order(&self) -> Vec<&SingletonMigration> {
        let mut migrations: Vec<_> = self.singletons_to_migrate.values().collect();
        
        // 按优先级和依赖关系排序
        migrations.sort_by(|a, b| {
            match (a.migration_priority, b.migration_priority) {
                (MigrationPriority::High, MigrationPriority::Medium) => std::cmp::Ordering::Less,
                (MigrationPriority::High, MigrationPriority::Low) => std::cmp::Ordering::Less,
                (MigrationPriority::Medium, MigrationPriority::Low) => std::cmp::Ordering::Less,
                (MigrationPriority::Medium, MigrationPriority::High) => std::cmp::Ordering::Greater,
                (MigrationPriority::Low, MigrationPriority::High) => std::cmp::Ordering::Greater,
                (MigrationPriority::Low, MigrationPriority::Medium) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            }
        });
        
        migrations
    }
}

/// 迁移执行器
pub struct MigrationExecutor {
    container: ServiceContainer,
}

impl MigrationExecutor {
    pub fn new(container: ServiceContainer) -> Self {
        Self { container }
    }

    pub fn execute_migration(&mut self, plan: &MigrationPlan) -> Result<(), Box<dyn std::error::Error>> {
        let migration_order = plan.get_migration_order();
        
        for migration in migration_order {
            println!("Migrating: {}", migration.type_name);
            
            // 执行迁移逻辑
            self.migrate_singleton(migration)?;
        }
        
        Ok(())
    }

    fn migrate_singleton(&mut self, migration: &SingletonMigration) -> Result<(), Box<dyn std::error::Error>> {
        match migration.type_name.as_str() {
            "JitCompiler" => self.migrate_jit_compiler(),
            "EventBus" => self.migrate_event_bus(),
            "VirtualMachineState" => self.migrate_vm_state(),
            _ => Err(format!("Unknown singleton type: {}", migration.type_name).into()),
        }
    }

    fn migrate_jit_compiler(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // 迁移JIT编译器的具体实现
        vm_engine_jit::di_jit::configure_jit_services(&mut self.container)?;
        Ok(())
    }

    fn migrate_event_bus(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // 迁移事件总线的具体实现
        vm_core::di_event_bus::configure_event_bus_services(&mut self.container)?;
        Ok(())
    }

    fn migrate_vm_state(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // 迁移虚拟机状态的具体实现
        vm_core::di_vm_state::configure_vm_state_services(&mut self.container)?;
        Ok(())
    }
}
```

这些实现示例展示了如何在VM项目中应用依赖注入框架，包括核心框架实现、具体应用示例、性能优化和迁移工具。通过这些示例，开发团队可以逐步将现有的单例模式迁移到依赖注入模式，提高代码的可维护性、可测试性和可扩展性。