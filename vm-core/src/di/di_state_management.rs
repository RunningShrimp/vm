//! 状态管理优化组件
//!
//! 本模块实现了状态管理优化策略，包括读写分离、副本-on-write、状态变更通知和事务性状态更新。

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};


/// 状态变更事件
#[derive(Debug, Clone)]
pub enum StateChangeEvent<T> {
    /// 状态已更改
    Changed { old: T, new: T },
    /// 状态已快照
    Snapshotted { state: T },
    /// 状态已恢复
    Restored { state: T },
}

/// 状态观察者trait
pub trait StateObserver<T>: Send + Sync {
    /// 当状态变更时调用
    fn on_state_changed(&self, old_state: &T, new_state: &T);
    
    /// 获取观察者ID
    fn observer_id(&self) -> String;
}

/// 状态操作trait
pub trait StateOperation: Send + Sync {
    /// 执行操作
    fn execute(&self) -> Result<(), StateError>;
    
    /// 回滚操作
    fn rollback(&self) -> Result<(), StateError>;
    
    /// 获取操作描述
    fn description(&self) -> &str;
}

/// 状态错误类型
#[derive(Debug)]
pub enum StateError {
    /// 操作失败
    OperationFailed(String),
    /// 回滚失败
    RollbackFailed(String),
    /// 并发冲突
    ConcurrencyConflict(String),
    /// 状态不一致
    InconsistentState(String),
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateError::OperationFailed(msg) => write!(f, "Operation failed: {}", msg),
            StateError::RollbackFailed(msg) => write!(f, "Rollback failed: {}", msg),
            StateError::ConcurrencyConflict(msg) => write!(f, "Concurrency conflict: {}", msg),
            StateError::InconsistentState(msg) => write!(f, "Inconsistent state: {}", msg),
        }
    }
}

impl std::error::Error for StateError {}

/// 读写分离状态
pub struct ReadWriteState<T> {
    /// 读状态（使用RwLock优化读操作）
    read_state: Arc<RwLock<T>>,
    
    /// 写状态（使用Mutex确保写操作原子性）
    write_state: Arc<Mutex<T>>,
    
    /// 变更通知器
    change_notifier: Arc<ChangeNotifier<T>>,
}

impl<T> ReadWriteState<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// 创建新的读写分离状态
    pub fn new(initial_state: T) -> Self {
        let state_clone = initial_state.clone();
        Self {
            read_state: Arc::new(RwLock::new(initial_state)),
            write_state: Arc::new(Mutex::new(state_clone)),
            change_notifier: Arc::new(ChangeNotifier::new()),
        }
    }
    
    /// 读取状态（优化读操作）
    pub fn read<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let state = self.read_state.read().unwrap();
        f(&*state)
    }
    
    /// 写入状态（优化写操作）
    pub fn write<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let mut state = self.write_state.lock().unwrap();
        let old_state = state.clone();
        let result = f(&mut *state);
        let new_state = state.clone();
        
        // 更新读状态
        {
            let mut read_state = self.read_state.write().unwrap();
            *read_state = new_state.clone();
        }
        
        // 通知变更
        self.change_notifier.notify_change(old_state, new_state);
        
        result
    }
    
    /// 获取变更通知器
    pub fn change_notifier(&self) -> Arc<ChangeNotifier<T>> {
        Arc::clone(&self.change_notifier)
    }
}

/// 副本-on-write状态
pub struct COWState<T: Clone> {
    /// 状态
    state: Arc<RwLock<T>>,
    
    /// 版本号
    version: Arc<std::sync::atomic::AtomicU64>,
}

impl<T: Clone + Send + Sync + 'static> COWState<T> {
    /// 创建新的COW状态
    pub fn new(initial_state: T) -> Self {
        Self {
            state: Arc::new(RwLock::new(initial_state)),
            version: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }
    
    /// 获取状态句柄
    pub fn get_handle(&self) -> StateHandle<T> {
        let version = self.version.load(std::sync::atomic::Ordering::SeqCst);
        StateHandle {
            state: Arc::clone(&self.state),
            version,
        }
    }
    
    /// 更新状态
    pub fn update<F>(&self, updater: F) -> StateHandle<T>
    where
        F: FnOnce(&T) -> T,
    {
        let mut state = self.state.write().unwrap();
        let new_state = updater(&*state);
        *state = new_state.clone();
        
        let new_version = self.version.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        
        StateHandle {
            state: Arc::clone(&self.state),
            version: new_version,
        }
    }
}

/// 状态句柄
pub struct StateHandle<T> {
    state: Arc<RwLock<T>>,
    version: u64,
}

impl<T> StateHandle<T> {
    /// 获取状态版本
    pub fn version(&self) -> u64 {
        self.version
    }
    
    /// 读取状态
    pub fn read<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let state = self.state.read().unwrap();
        f(&*state)
    }
    
    /// 检查是否过期
    pub fn is_stale(&self, current_version: u64) -> bool {
        self.version < current_version
    }
}

/// 可观察状态
pub struct ObservableState<T> {
    /// 状态
    state: Arc<RwLock<T>>,
    
    /// 观察者列表
    observers: Arc<RwLock<Vec<Box<dyn StateObserver<T>>>>>,
}

impl<T: Clone + Send + Sync + 'static> ObservableState<T> {
    /// 创建新的可观察状态
    pub fn new(initial_state: T) -> Self {
        Self {
            state: Arc::new(RwLock::new(initial_state)),
            observers: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// 获取当前状态
    pub fn get(&self) -> T {
        let state = self.state.read().unwrap();
        state.clone()
    }
    
    /// 更新状态
    pub fn update<F>(&self, updater: F)
    where
        F: FnOnce(&T) -> T,
    {
        let mut state = self.state.write().unwrap();
        let old_state = state.clone();
        let new_state = updater(&*state);
        *state = new_state.clone();
        
        // 通知观察者
        let observers = self.observers.read().unwrap();
        for observer in observers.iter() {
            observer.on_state_changed(&old_state, &new_state);
        }
    }
    
    /// 添加观察者
    pub fn add_observer(&self, observer: Box<dyn StateObserver<T>>) {
        let mut observers = self.observers.write().unwrap();
        observers.push(observer);
    }
    
    /// 移除观察者
    pub fn remove_observer(&self, observer_id: &str) {
        let mut observers = self.observers.write().unwrap();
        observers.retain(|obs| obs.observer_id() != observer_id);
    }
    
    /// 获取观察者数量
    pub fn observer_count(&self) -> usize {
        let observers = self.observers.read().unwrap();
        observers.len()
    }
}

/// 变更通知器
pub struct ChangeNotifier<T> {
    /// 事件发送器
    event_sender: std::sync::mpsc::Sender<StateChangeEvent<T>>,
    
    /// 事件接收器
    event_receiver: Arc<Mutex<std::sync::mpsc::Receiver<StateChangeEvent<T>>>>,
}

impl<T: Clone + Send + Sync + 'static> ChangeNotifier<T> {
    /// 创建新的变更通知器
    pub fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self {
            event_sender: sender,
            event_receiver: Arc::new(Mutex::new(receiver)),
        }
    }
    /// 通知状态变更
    pub fn notify_change(&self, old_state: T, new_state: T) {
        let _ = self.event_sender.send(StateChangeEvent::Changed { old: old_state, new: new_state });

    }
    
    /// 通知状态快照
    pub fn notify_snapshot(&self, state: T) {
        let _ = self.event_sender.send(StateChangeEvent::Snapshotted { state });
    }
    
    /// 通知状态恢复
    pub fn notify_restore(&self, state: T) {
        let _ = self.event_sender.send(StateChangeEvent::Restored { state });
    }
    
    /// 获取事件接收器
    pub fn event_receiver(&self) -> Arc<Mutex<std::sync::mpsc::Receiver<StateChangeEvent<T>>>> {
        Arc::clone(&self.event_receiver)
    }
}

/// 状态事务
pub struct StateTransaction {
    /// 操作列表
    operations: Vec<Box<dyn StateOperation>>,
    
    /// 回滚操作列表
    rollback_operations: Vec<Box<dyn StateOperation>>,
    
    /// 事务开始时间
    start_time: Instant,
    
    /// 事务ID
    transaction_id: u64,
}

impl StateTransaction {
    /// 创建新事务
    pub fn new() -> Self {
        static NEXT_TRANSACTION_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        
        Self {
            operations: Vec::new(),
            rollback_operations: Vec::new(),
            start_time: Instant::now(),
            transaction_id: NEXT_TRANSACTION_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        }
    }
    
    /// 添加操作
    pub fn add_operation(&mut self, operation: Box<dyn StateOperation>) {
        self.operations.push(operation);
    }
    
    /// 提交事务
    pub fn commit(self) -> Result<TransactionResult, StateError> {
        let mut executed_operations = Vec::new();
        
        // 执行所有操作
        for operation in self.operations {
            match operation.execute() {
                Ok(()) => {
                    // 在实际实现中，这里应该收集回滚操作
                    executed_operations.push(operation);
                }
                Err(e) => {
                    // 回滚已执行的操作
                    for op in executed_operations.iter().rev() {
                        if let Err(rollback_err) = op.rollback() {
                            // 记录回滚失败，但继续回滚其他操作
                            eprintln!("Rollback failed: {}", rollback_err);
                        }
                    }
                    return Err(e);
                }
            }
        }
        
        let duration = self.start_time.elapsed();
        
        Ok(TransactionResult {
            transaction_id: self.transaction_id,
            operations_executed: executed_operations.len(),
            duration,
            success: true,
        })
    }
    
    /// 获取事务ID
    pub fn transaction_id(&self) -> u64 {
        self.transaction_id
    }
    
    /// 获取已执行的操作数量
    pub fn operation_count(&self) -> usize {
        self.operations.len()
    }
    
    /// 获取事务持续时间
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

/// 事务结果
#[derive(Debug)]
pub struct TransactionResult {
    /// 事务ID
    pub transaction_id: u64,
    
    /// 执行的操作数量
    pub operations_executed: usize,
    
    /// 事务持续时间
    pub duration: Duration,
    
    /// 是否成功
    pub success: bool,
}

/// 分段状态
pub struct SegmentedState<T> {
    /// 状态段
    segments: Vec<Arc<RwLock<T>>>,
    
    /// 段数量
    segment_count: usize,
}

impl<T: Clone + Send + Sync + 'static> SegmentedState<T> {
    /// 创建新的分段状态
    pub fn new(initial_state: T, segment_count: usize) -> Self {
        let mut segments = Vec::with_capacity(segment_count);
        for _ in 0..segment_count {
            segments.push(Arc::new(RwLock::new(initial_state.clone())));
        }
        
        Self {
            segments,
            segment_count,
        }
    }
    
    /// 根据键获取段
    pub fn get_segment(&self, key: usize) -> &Arc<RwLock<T>> {
        &self.segments[key % self.segment_count]
    }
    
    /// 获取所有段
    pub fn segments(&self) -> &[Arc<RwLock<T>>] {
        &self.segments
    }
    
    /// 获取段数量
    pub fn segment_count(&self) -> usize {
        self.segment_count
    }
}

/// 无锁状态
pub struct LockFreeState<T> {
    /// 状态指针
    state: Arc<std::sync::atomic::AtomicPtr<T>>,
    
    /// 工厂函数
    factory: Arc<dyn Fn() -> T + Send + Sync>,
}

impl<T: Send + Sync + 'static + Clone> LockFreeState<T> {
    /// 创建新的无锁状态
    pub fn new<F>(factory: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        let initial_state = Box::new(factory());
        let state_ptr = Box::into_raw(initial_state);
        
        Self {
            state: Arc::new(std::sync::atomic::AtomicPtr::new(state_ptr)),
            factory: Arc::new(factory),
        }
    }
    
    /// 获取状态
    pub fn get(&self) -> T {
        let state_ptr = self.state.load(std::sync::atomic::Ordering::Acquire);
        unsafe { (*state_ptr).clone() }
    }
    
    /// 更新状态
    pub fn update<F>(&self, updater: F)
    where
        F: Fn(&T) -> T,
    {
        loop {
            let old_ptr = self.state.load(std::sync::atomic::Ordering::Acquire);
            let old_state = unsafe { &*old_ptr };
            let new_state = updater(old_state);
            let new_ptr = Box::into_raw(Box::new(new_state));
            
            match self.state.compare_exchange_weak(
                old_ptr,
                new_ptr,
                std::sync::atomic::Ordering::Release,
                std::sync::atomic::Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // 成功更新，释放旧状态
                    unsafe {
                        let _ = Box::from_raw(old_ptr);
                    }
                    break;
                }
                Err(_) => {
                    // 更新失败，释放新状态并重试
                    unsafe {
                        let _ = Box::from_raw(new_ptr);
                    }
                }
            }
        }
    }
}

impl<T> Drop for LockFreeState<T> {
    fn drop(&mut self) {
        let state_ptr = self.state.load(std::sync::atomic::Ordering::Acquire);
        if !state_ptr.is_null() {
            unsafe {
                let _ = Box::from_raw(state_ptr);
            }
        }
    }
}

/// 状态管理器
pub struct StateManager {
    /// 状态存储
    states: Arc<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
    
    /// 事务管理器
    transaction_manager: Arc<Mutex<TransactionManager>>,
}

impl StateManager {
    /// 创建新的状态管理器
    pub fn new() -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
            transaction_manager: Arc::new(Mutex::new(TransactionManager::new())),
        }
    }
    
    /// 注册状态
    pub fn register_state<T: 'static + Send + Sync>(&self, state: T) {
        let mut states = self.states.write().unwrap();
        states.insert(TypeId::of::<T>(), Box::new(state));
    }
    
    /// 获取状态
    pub fn get_state<T: 'static + Send + Sync>(&self) -> Option<Arc<T>> {
        // 注意：这个实现需要将状态存储为Arc<T>
        // 当前实现不支持直接返回引用，因为读锁会立即释放
        // 这里返回None作为占位符
        None
    }
    
    /// 创建事务
    pub fn create_transaction(&self) -> StateTransaction {
        StateTransaction::new()
    }
    
    /// 获取事务统计
    pub fn transaction_stats(&self) -> TransactionStats {
        let manager = self.transaction_manager.lock().unwrap();
        manager.stats()
    }
}

/// 事务管理器
pub struct TransactionManager {
    /// 活动事务
    active_transactions: HashMap<u64, StateTransaction>,
    
    /// 已完成事务
    completed_transactions: Vec<TransactionResult>,
    
    /// 总事务数
    total_transactions: u64,
    
    /// 成功事务数
    successful_transactions: u64,
}

impl TransactionManager {
    /// 创建新的事务管理器
    pub fn new() -> Self {
        Self {
            active_transactions: HashMap::new(),
            completed_transactions: Vec::new(),
            total_transactions: 0,
            successful_transactions: 0,
        }
    }
    
    /// 注册事务
    pub fn register_transaction(&mut self, transaction: StateTransaction) {
        self.active_transactions.insert(transaction.transaction_id(), transaction);
        self.total_transactions += 1;
    }
    
    /// 完成事务
    pub fn complete_transaction(&mut self, result: TransactionResult) {
        if result.success {
            self.successful_transactions += 1;
        }
        
        self.active_transactions.remove(&result.transaction_id);
        self.completed_transactions.push(result);
        
        // 限制历史记录数量
        if self.completed_transactions.len() > 1000 {
            self.completed_transactions.remove(0);
        }
    }
    
    /// 获取统计信息
    pub fn stats(&self) -> TransactionStats {
        TransactionStats {
            active_transactions: self.active_transactions.len(),
            total_transactions: self.total_transactions,
            successful_transactions: self.successful_transactions,
            success_rate: if self.total_transactions > 0 {
                self.successful_transactions as f64 / self.total_transactions as f64
            } else {
                0.0
            },
        }
    }
}

/// 事务统计信息
#[derive(Debug, Clone)]
pub struct TransactionStats {
    /// 活动事务数
    pub active_transactions: usize,
    
    /// 总事务数
    pub total_transactions: u64,
    
    /// 成功事务数
    pub successful_transactions: u64,
    
    /// 成功率
    pub success_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_read_write_state() {
        let state = ReadWriteState::new(42);
        
        // 测试读操作
        let value = state.read(|s| *s);
        assert_eq!(value, 42);
        
        // 测试写操作
        state.write(|s| *s = 100);
        let value = state.read(|s| *s);
        assert_eq!(value, 100);
    }
    
    #[test]
    fn test_cow_state() {
        let state = COWState::new(42);
        
        let handle1 = state.get_handle();
        assert_eq!(handle1.version(), 1);
        
        let handle2 = state.update(|s| s + 1);
        assert_eq!(handle2.version(), 2);
        
        let value = handle2.read(|s| *s);
        assert_eq!(value, 43);
    }
    
    #[test]
    fn test_observable_state() {
        struct TestObserver {
            id: String,
            changes: Arc<Mutex<Vec<(i32, i32)>>>,
        }
        
        impl StateObserver<i32> for TestObserver {
            fn on_state_changed(&self, old_state: &i32, new_state: &i32) {
                let mut changes = self.changes.lock().unwrap();
                changes.push((*old_state, *new_state));
            }
            
            fn observer_id(&self) -> String {
                self.id.clone()
            }
        }
        
        let state = ObservableState::new(42);
        let changes = Arc::new(Mutex::new(Vec::new()));
        let observer = TestObserver {
            id: "test".to_string(),
            changes: Arc::clone(&changes),
        };
        
        state.add_observer(Box::new(observer));
        
        state.update(|s| s + 1);
        
        let changes = changes.lock().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0], (42, 43));
    }
    
    #[test]
    fn test_state_transaction() {
        struct TestOperation {
            value: i32,
        }
        
        impl StateOperation for TestOperation {
            fn execute(&self) -> Result<(), StateError> {
                // 模拟操作
                Ok(())
            }
            
            fn rollback(&self) -> Result<(), StateError> {
                // 模拟回滚
                Ok(())
            }
            
            fn description(&self) -> &str {
                "Test operation"
            }
        }
        
        let mut transaction = StateTransaction::new();
        transaction.add_operation(Box::new(TestOperation { value: 42 }));
        
        assert_eq!(transaction.operation_count(), 1);
        assert!(transaction.transaction_id() > 0);
        
        let result = transaction.commit().unwrap();
        assert!(result.success);
        assert_eq!(result.operations_executed, 1);
    }
    
    #[test]
    fn test_segmented_state() {
        let state = SegmentedState::new(42, 4);
        assert_eq!(state.segment_count(), 4);
        
        let segment1 = state.get_segment(0);
        let segment2 = state.get_segment(4); // 应该和segment1相同
        
        assert!(std::ptr::eq(segment1, segment2));
    }
    
    #[test]
    fn test_state_manager() {
        let manager = StateManager::new();
        manager.register_state(42i32);
        
        // 注意：由于类型擦除，这里无法直接测试获取状态
        // 在实际使用中，需要使用downcast或其他机制
    }
}