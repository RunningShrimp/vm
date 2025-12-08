//! 依赖注入框架使用示例
//!
//! 本模块展示了如何使用依赖注入框架来替代VM项目中的单例模式。

use std::sync::Arc;
use vm_core::di::prelude::*;

/// 示例服务1：虚拟机状态管理器
pub struct VmStateManager {
    state: Arc<std::sync::Mutex<VmState>>,
}

impl VmStateManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(std::sync::Mutex::new(VmState::new())),
        }
    }
    
    pub fn get_state(&self) -> VmState {
        let state = self.state.lock().unwrap();
        state.clone()
    }
    
    pub fn set_state(&self, new_state: VmState) {
        let mut state = self.state.lock().unwrap();
        *state = new_state;
    }
}

/// 示例服务2：内存管理单元
pub struct MemoryManager {
    memory_size: usize,
}

impl MemoryManager {
    pub fn new(memory_size: usize) -> Self {
        Self { memory_size }
    }
    
    pub fn allocate(&self, size: usize) -> Result<usize, String> {
        if size <= self.memory_size {
            Ok(0x1000) // 返回一个虚拟地址
        } else {
            Err("Memory allocation failed".to_string())
        }
    }
    
    pub fn get_memory_size(&self) -> usize {
        self.memory_size
    }
}

/// 示例服务3：执行引擎
pub struct ExecutionEngine {
    vm_state: Arc<VmStateManager>,
    memory: Arc<MemoryManager>,
}

impl ExecutionEngine {
    pub fn new(
        vm_state: Arc<VmStateManager>,
        memory: Arc<MemoryManager>,
    ) -> Self {
        Self { vm_state, memory }
    }
    
    pub fn execute(&self) -> Result<(), String> {
        println!("Executing with memory size: {}", self.memory.get_memory_size());
        let state = self.vm_state.get_state();
        println!("Current VM state: {:?}", state);
        Ok(())
    }
}

/// 虚拟机状态
#[derive(Debug, Clone)]
pub struct VmState {
    pc: u64,
    registers: [u64; 32],
}

impl VmState {
    pub fn new() -> Self {
        Self {
            pc: 0,
            registers: [0; 32],
        }
    }
}

/// 基础依赖注入示例
pub fn basic_di_example() -> Result<(), DIError> {
    println!("=== 基础依赖注入示例 ===");
    
    // 创建服务容器
    let container = ContainerBuilder::new()
        .register_singleton::<VmStateManager>()
        .register_singleton::<MemoryManager>() // 注意：这里需要提供实例
        .register_transient::<ExecutionEngine>()
        .build()?;
    
    // 获取并使用服务
    let vm_state = container.get_service::<VmStateManager>()?;
    let memory = container.get_service::<MemoryManager>()?;
    
    // 设置状态
    vm_state.set_state(VmState {
        pc: 0x1000,
        registers: [1; 32],
    });
    
    // 执行
    let engine = container.get_service::<ExecutionEngine>()?;
    engine.execute().map_err(|e| DIError::ServiceCreationFailed(e))?;
    
    println!("基础依赖注入示例完成");
    Ok(())
}

/// 工厂函数示例
pub fn factory_example() -> Result<(), DIError> {
    println!("=== 工厂函数示例 ===");
    
    // 创建带工厂函数的服务容器
    let container = ContainerBuilder::new()
        .register_singleton::<VmStateManager>()
        .register_factory(|provider| {
            let vm_state = provider.get_required_service::<VmStateManager>()?;
            Ok(MemoryManager::new(1024 * 1024)) // 1MB内存
        })
        .register_factory(|provider| {
            let vm_state = provider.get_required_service::<VmStateManager>()?;
            let memory = provider.get_required_service::<MemoryManager>()?;
            Ok(ExecutionEngine::new(vm_state, memory))
        })
        .build()?;
    
    // 使用服务
    let vm_state = container.get_service::<VmStateManager>()?;
    vm_state.set_state(VmState {
        pc: 0x2000,
        registers: [2; 32],
    });
    
    let engine = container.get_service::<ExecutionEngine>()?;
    engine.execute().map_err(|e| DIError::ServiceCreationFailed(e))?;
    
    println!("工厂函数示例完成");
    Ok(())
}

/// 作用域服务示例
pub fn scoped_service_example() -> Result<(), DIError> {
    println!("=== 作用域服务示例 ===");
    
    // 创建服务容器
    let container = ContainerBuilder::new()
        .register_singleton::<VmStateManager>()
        .register_scoped::<MemoryManager>()
        .register_transient::<ExecutionEngine>()
        .build()?;
    
    // 创建作用域
    let scope = container.create_scope()?;
    
    // 在作用域内获取服务
    let memory = scope.get_service::<MemoryManager>()?;
    println!("Memory size in scope: {}", memory.get_memory_size());
    
    // 获取执行引擎
    let engine = scope.get_service::<ExecutionEngine>()?;
    engine.execute().map_err(|e| DIError::ServiceCreationFailed(e))?;
    
    println!("作用域服务示例完成");
    Ok(())
}

/// 性能优化示例
pub fn performance_optimization_example() -> Result<(), DIError> {
    println!("=== 性能优化示例 ===");
    
    // 创建高性能容器
    let container = ContainerBuilderFactory::create_high_performance()
        .register_singleton::<VmStateManager>()
        .register_factory(|provider| {
            let vm_state = provider.get_required_service::<VmStateManager>()?;
            Ok(MemoryManager::new(2 * 1024 * 1024)) // 2MB内存
        })
        .with_warmup_service::<VmStateManager>()
        .with_warmup_service::<MemoryManager>()
        .build()?;
    
    // 获取服务
    let vm_state = container.get_service::<VmStateManager>()?;
    let memory = container.get_service::<MemoryManager>()?;
    
    println!("高性能容器中的内存大小: {}", memory.get_memory_size());
    
    // 获取容器统计信息
    let stats = container.stats();
    println!("容器统计: {:?}", stats);
    
    println!("性能优化示例完成");
    Ok(())
}

/// 状态管理优化示例
pub fn state_management_example() -> Result<(), DIError> {
    println!("=== 状态管理优化示例 ===");
    
    use vm_core::di::di_state_management::*;
    
    // 创建读写分离状态
    let state = ReadWriteState::new(VmState::new());
    
    // 读取状态
    state.read(|s| {
        println!("初始PC: {}", s.pc);
    });
    
    // 写入状态
    state.write(|s| {
        s.pc = 0x3000;
        s.registers[0] = 42;
    });
    
    // 再次读取
    state.read(|s| {
        println!("更新后PC: {}", s.pc);
        println!("更新后寄存器0: {}", s.registers[0]);
    });
    
    // 创建可观察状态
    let observable_state = ObservableState::new(VmState::new());
    
    // 添加观察者
    struct TestObserver;
    impl StateObserver<VmState> for TestObserver {
        fn on_state_changed(&self, old_state: &VmState, new_state: &VmState) {
            println!("状态变更: PC {} -> {}", old_state.pc, new_state.pc);
        }
        
        fn observer_id(&self) -> String {
            "test_observer".to_string()
        }
    }
    
    observable_state.add_observer(Box::new(TestObserver));
    
    // 更新状态
    observable_state.update(|s| {
        s.pc = 0x4000;
    });
    
    println!("状态管理优化示例完成");
    Ok(())
}

/// 内存优化示例
pub fn memory_optimization_example() -> Result<(), DIError> {
    println!("=== 内存优化示例 ===");
    
    use vm_core::di::di_optimization::*;
    
    // 创建对象池
    let pool = ObjectPool::new(|| VmState::new(), 10);
    
    // 获取对象
    let obj1 = pool.acquire();
    let obj2 = pool.acquire();
    
    println!("对象池对象1 PC: {}", obj1.pc);
    println!("对象池对象2 PC: {}", obj2.pc);
    
    // 获取池统计信息
    let stats = pool.stats();
    println!("池统计: {:?}", stats);
    
    // 创建延迟服务
    let lazy_service = LazyService::new(|| {
        println!("延迟初始化MemoryManager");
        MemoryManager::new(4 * 1024 * 1024) // 4MB内存
    });
    
    // 第一次访问会触发初始化
    let memory = lazy_service.get();
    println!("延迟服务内存大小: {}", memory.get_memory_size());
    
    // 第二次访问不会触发初始化
    let memory2 = lazy_service.get();
    println!("第二次访问内存大小: {}", memory2.get_memory_size());
    
    println!("内存优化示例完成");
    Ok(())
}

/// 迁移示例
pub fn migration_example() -> Result<(), DIError> {
    println!("=== 迁移示例 ===");
    
    use vm_core::di::di_migration::*;
    
    // 创建容器
    let container = Arc::new(ContainerBuilder::new()
        .register_singleton::<VmStateManager>()
        .register_factory(|provider| {
            let vm_state = provider.get_required_service::<VmStateManager>()?;
            Ok(MemoryManager::new(8 * 1024 * 1024)) // 8MB内存
        })
        .build()?);
    
    // 创建迁移工具
    let migration_tool = MigrationTool::new(Arc::clone(&container));
    
    // 注册全局单例（模拟现有单例）
    migration_tool.register_global_arc_singleton(Arc::new(VmStateManager::new()));
    
    // 配置迁移
    migration_tool.configure_migration::<VmStateManager, VmStateManager>(
        MigrationStrategy::DirectReplacement,
        false,
        5000,
    );
    
    // 执行迁移
    let result = migration_tool.migrate()?;
    println!("迁移结果: 成功 {} 个，失败 {} 个", 
              result.successful_migrations.len(), 
              result.failed_migrations.len());
    
    // 获取迁移状态
    let status = migration_tool.migration_status();
    println!("迁移状态: 总 {} 个，待迁移 {} 个，已完成 {} 个",
              status.total_types,
              status.pending_migrations.len(),
              status.completed_migrations.len());
    
    println!("迁移示例完成");
    Ok(())
}

/// 兼容性层示例
pub fn compatibility_layer_example() -> Result<(), DIError> {
    println!("=== 兼容性层示例 ===");
    
    use vm_core::di::di_migration::*;
    
    // 创建容器
    let container = Arc::new(ContainerBuilder::new()
        .register_singleton::<VmStateManager>()
        .register_factory(|provider| {
            let vm_state = provider.get_required_service::<VmStateManager>()?;
            Ok(MemoryManager::new(16 * 1024 * 1024)) // 16MB内存
        })
        .build()?);
    
    // 创建兼容性层
    let compatibility_layer = CompatibilityLayer::new(Arc::clone(&container));
    
    // 初始使用单例模式
    println!("初始模式: 使用单例");
    let vm_state = compatibility_layer.get_service::<VmStateManager>()?;
    vm_state.set_state(VmState {
        pc: 0x5000,
        registers: [5; 32],
    });
    
    // 切换到依赖注入模式
    compatibility_layer.switch_to_di();
    println!("切换后模式: 使用依赖注入");
    
    let vm_state2 = compatibility_layer.get_service::<VmStateManager>()?;
    let state = vm_state2.get_state();
    println!("切换后状态: PC {}", state.pc);
    
    // 检查当前模式
    println!("当前是否使用DI: {}", compatibility_layer.is_using_di());
    
    println!("兼容性层示例完成");
    Ok(())
}

/// 全局服务定位器示例
pub fn global_service_locator_example() -> Result<(), DIError> {
    println!("=== 全局服务定位器示例 ===");
    
    // 创建容器
    let container = Arc::new(ContainerBuilder::new()
        .register_singleton::<VmStateManager>()
        .register_factory(|provider| {
            let vm_state = provider.get_required_service::<VmStateManager>()?;
            Ok(MemoryManager::new(32 * 1024 * 1024)) // 32MB内存
        })
        .build()?);
    
    // 初始化全局服务定位器
    vm_core::di::global::init(Arc::clone(&container));
    
    // 使用全局服务定位器
    let vm_state = vm_core::di::global::get::<VmStateManager>()?;
    vm_state.set_state(VmState {
        pc: 0x6000,
        registers: [6; 32],
    });
    
    let state = vm_state.get_state();
    println!("全局服务定位器状态: PC {}", state.pc);
    
    // 尝试获取服务
    if let Some(memory) = vm_core::di::global::try_get::<MemoryManager>() {
        println!("全局内存大小: {}", memory.get_memory_size());
    } else {
        println!("无法获取全局内存服务");
    }
    
    println!("全局服务定位器示例完成");
    Ok(())
}

/// 主示例函数
pub fn run_all_examples() -> Result<(), DIError> {
    println!("开始运行依赖注入框架示例...\n");
    
    basic_di_example()?;
    println!();
    
    factory_example()?;
    println!();
    
    scoped_service_example()?;
    println!();
    
    performance_optimization_example()?;
    println!();
    
    state_management_example()?;
    println!();
    
    memory_optimization_example()?;
    println!();
    
    migration_example()?;
    println!();
    
    compatibility_layer_example()?;
    println!();
    
    global_service_locator_example()?;
    println!();
    
    println!("所有依赖注入框架示例运行完成！");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_di_example() {
        assert!(basic_di_example().is_ok());
    }
    
    #[test]
    fn test_factory_example() {
        assert!(factory_example().is_ok());
    }
    
    #[test]
    fn test_scoped_service_example() {
        assert!(scoped_service_example().is_ok());
    }
    
    #[test]
    fn test_performance_optimization_example() {
        assert!(performance_optimization_example().is_ok());
    }
    
    #[test]
    fn test_state_management_example() {
        assert!(state_management_example().is_ok());
    }
    
    #[test]
    fn test_memory_optimization_example() {
        assert!(memory_optimization_example().is_ok());
    }
    
    #[test]
    fn test_migration_example() {
        assert!(migration_example().is_ok());
    }
    
    #[test]
    fn test_compatibility_layer_example() {
        assert!(compatibility_layer_example().is_ok());
    }
    
    #[test]
    fn test_global_service_locator_example() {
        assert!(global_service_locator_example().is_ok());
    }
    
    #[test]
    fn test_run_all_examples() {
        assert!(run_all_examples().is_ok());
    }
}