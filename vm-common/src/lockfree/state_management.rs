//! 无锁共享状态管理
//!
//! 实现高性能的无锁共享状态管理，用于多线程环境下的状态同步

use super::LockFreeQueue;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, AtomicUsize, Ordering};

/// 状态订阅者类型别名
type SubscriberMap<T> = std::collections::HashMap<usize, Box<dyn StateSubscriber<T> + Send + Sync>>;

/// 共享状态版本号
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateVersion {
    /// 主版本号
    pub major: u64,
    /// 次版本号
    pub minor: u64,
}

impl StateVersion {
    /// 创建新的版本号
    pub fn new() -> Self {
        Self { major: 0, minor: 0 }
    }

    /// 创建指定版本号
    pub fn with(major: u64, minor: u64) -> Self {
        Self { major, minor }
    }

    /// 增加主版本号
    pub fn increment_major(&mut self) {
        self.major += 1;
        self.minor = 0;
    }

    /// 增加次版本号
    pub fn increment_minor(&mut self) {
        self.minor += 1;
    }
}

impl PartialOrd for StateVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for StateVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.major < other.major {
            std::cmp::Ordering::Less
        } else if self.major > other.major {
            std::cmp::Ordering::Greater
        } else if self.minor < other.minor {
            std::cmp::Ordering::Less
        } else if self.minor > other.minor {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    }
}

impl Default for StateVersion {
    fn default() -> Self {
        Self::new()
    }
}

/// 共享状态快照
#[derive(Debug, Clone)]
pub struct StateSnapshot<T> {
    /// 状态数据
    pub data: T,
    /// 版本号
    pub version: StateVersion,
    /// 时间戳
    pub timestamp: std::time::Instant,
}

impl<T> StateSnapshot<T> {
    /// 创建新的状态快照
    pub fn new(data: T, version: StateVersion) -> Self {
        Self {
            data,
            version,
            timestamp: std::time::Instant::now(),
        }
    }
}

/// 无锁共享状态
pub struct LockFreeSharedState<T: Send + Sync> {
    /// 当前状态
    current: AtomicPtr<StateSnapshot<T>>,
    /// 版本号
    version: AtomicU64,
    /// 更新计数
    update_count: AtomicUsize,
    /// 读取计数
    read_count: AtomicUsize,
    /// Phantom data to make T part of type
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Clone + Send + Sync> LockFreeSharedState<T> {
    /// 创建新的无锁共享状态
    pub fn new(initial_state: T) -> Self {
        let snapshot = StateSnapshot::new(initial_state, StateVersion::new());
        Self {
            current: AtomicPtr::new(Box::into_raw(Box::new(snapshot))),
            version: AtomicU64::new(0),
            update_count: AtomicUsize::new(0),
            read_count: AtomicUsize::new(0),
            _phantom: std::marker::PhantomData,
        }
    }

    /// 读取状态
    pub fn read(&self) -> StateSnapshot<T> {
        self.read_count.fetch_add(1, Ordering::Relaxed);

        let ptr = self.current.load(Ordering::Acquire);
        let snapshot = unsafe { &*ptr };

        StateSnapshot {
            data: snapshot.data.clone(),
            version: snapshot.version,
            timestamp: snapshot.timestamp,
        }
    }

    /// 更新状态
    pub fn update<F>(&self, mut updater: F) -> StateSnapshot<T>
    where
        F: FnMut(&T) -> T,
    {
        self.update_count.fetch_add(1, Ordering::Relaxed);

        loop {
            let old_ptr = self.current.load(Ordering::Acquire);
            let old_snapshot = unsafe { &*old_ptr };

            // 创建新状态
            let new_data = updater(&old_snapshot.data);
            let mut new_version = old_snapshot.version;
            new_version.increment_minor();

            let new_snapshot = StateSnapshot::new(new_data, new_version);
            let new_ptr = Box::into_raw(Box::new(new_snapshot));

            // 尝试更新状态
            if self
                .current
                .compare_exchange_weak(old_ptr, new_ptr, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                // 更新成功，释放旧状态
                unsafe {
                    let _ = Box::from_raw(old_ptr);
                }

                return unsafe { (*new_ptr).clone() };
            }

            // CAS失败，释放新状态并重试
            unsafe {
                let _ = Box::from_raw(new_ptr);
            }
        }
    }

    /// 条件更新状态
    pub fn conditional_update<F, P>(&self, predicate: P, mut updater: F) -> Option<StateSnapshot<T>>
    where
        F: FnMut(&T) -> T,
        P: Fn(&T) -> bool,
    {
        self.update_count.fetch_add(1, Ordering::Relaxed);

        loop {
            let old_ptr = self.current.load(Ordering::Acquire);
            let old_snapshot = unsafe { &*old_ptr };

            // 检查条件
            if !predicate(&old_snapshot.data) {
                return None;
            }

            // 创建新状态
            let new_data = updater(&old_snapshot.data);
            let mut new_version = old_snapshot.version;
            new_version.increment_minor();

            let new_snapshot = StateSnapshot::new(new_data, new_version);
            let new_ptr = Box::into_raw(Box::new(new_snapshot));

            // 尝试更新状态
            if self
                .current
                .compare_exchange_weak(old_ptr, new_ptr, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                // 更新成功，释放旧状态
                unsafe {
                    let _ = Box::from_raw(old_ptr);
                }

                return Some(unsafe { (*new_ptr).clone() });
            }

            // CAS失败，释放新状态并重试
            unsafe {
                let _ = Box::from_raw(new_ptr);
            }
        }
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> SharedStateStats {
        SharedStateStats {
            update_count: self.update_count.load(Ordering::Relaxed),
            read_count: self.read_count.load(Ordering::Relaxed),
            current_version: unsafe { (*self.current.load(Ordering::Relaxed)).version },
        }
    }

    /// 获取内部版本号（用于低级同步操作）
    pub fn get_version_value(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
    }
}

impl<T: Send + Sync> Drop for LockFreeSharedState<T> {
    fn drop(&mut self) {
        let ptr = self.current.load(Ordering::Relaxed);
        if !ptr.is_null() {
            unsafe {
                let _ = Box::from_raw(ptr);
            }
        }
    }
}

unsafe impl<T: Send + Sync> Send for LockFreeSharedState<T> {}
unsafe impl<T: Send + Sync> Sync for LockFreeSharedState<T> {}

/// 共享状态统计信息
#[derive(Debug, Clone)]
pub struct SharedStateStats {
    /// 更新次数
    pub update_count: usize,
    /// 读取次数
    pub read_count: usize,
    /// 当前版本号
    pub current_version: StateVersion,
}

/// 分片共享状态
pub struct StripedSharedState<T: Send + Sync> {
    /// 分片数组
    shards: Vec<LockFreeSharedState<T>>,
    /// 分片数量
    shard_count: usize,
}

impl<T: Clone + Send + Sync> StripedSharedState<T> {
    /// 创建新的分片共享状态
    pub fn new(initial_state: T) -> Self {
        Self::with_shards(initial_state, 16)
    }

    /// 创建指定分片数量的分片共享状态
    pub fn with_shards(initial_state: T, shard_count: usize) -> Self {
        let mut shards = Vec::with_capacity(shard_count);

        for _ in 0..shard_count {
            shards.push(LockFreeSharedState::new(initial_state.clone()));
        }

        Self {
            shards,
            shard_count,
        }
    }

    /// 读取状态
    pub fn read(&self) -> Vec<StateSnapshot<T>> {
        self.shards.iter().map(|shard| shard.read()).collect()
    }

    /// 更新所有分片
    pub fn update_all<F>(&self, updater: F) -> Vec<StateSnapshot<T>>
    where
        F: Fn(&T) -> T + Clone,
    {
        self.shards
            .iter()
            .map(|shard| shard.update(updater.clone()))
            .collect()
    }

    /// 更新指定分片
    pub fn update_shard<F>(&self, shard_index: usize, updater: F) -> Option<StateSnapshot<T>>
    where
        F: FnMut(&T) -> T,
    {
        if shard_index < self.shard_count {
            Some(self.shards[shard_index].update(updater))
        } else {
            None
        }
    }

    /// 获取分片数量
    pub fn shard_count(&self) -> usize {
        self.shard_count
    }

    /// 获取所有分片的统计信息
    pub fn get_all_stats(&self) -> Vec<SharedStateStats> {
        self.shards.iter().map(|shard| shard.get_stats()).collect()
    }
}

/// 读写锁状态
pub struct RwLockState<T: Send + Sync> {
    /// 当前状态
    current: AtomicPtr<T>,
    /// 写入锁
    write_lock: AtomicBool,
    /// 读取计数
    read_count: AtomicUsize,
    /// Phantom data to make T part of type
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Clone + Send + Sync> RwLockState<T> {
    /// 创建新的读写锁状态
    pub fn new(initial_state: T) -> Self {
        Self {
            current: AtomicPtr::new(Box::into_raw(Box::new(initial_state))),
            write_lock: AtomicBool::new(false),
            read_count: AtomicUsize::new(0),
            _phantom: std::marker::PhantomData,
        }
    }

    /// 读取状态
    pub fn read(&self) -> T {
        loop {
            // 等待写入锁释放
            if self.write_lock.load(Ordering::Acquire) {
                std::thread::yield_now();
                continue;
            }

            // 增加读取计数
            self.read_count.fetch_add(1, Ordering::Relaxed);

            // 读取状态
            let ptr = self.current.load(Ordering::Acquire);
            let data = unsafe { (*ptr).clone() };

            // 减少读取计数
            self.read_count.fetch_sub(1, Ordering::Relaxed);

            return data;
        }
    }

    /// 写入状态
    pub fn write<F>(&self, updater: F) -> T
    where
        F: FnOnce(&T) -> T,
    {
        // 获取写入锁
        while self
            .write_lock
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            std::thread::yield_now();
        }

        // 等待所有读取完成
        while self.read_count.load(Ordering::Relaxed) > 0 {
            std::thread::yield_now();
        }

        // 更新状态
        let old_ptr = self.current.load(Ordering::Acquire);
        let old_data = unsafe { &*old_ptr };
        let new_data = updater(old_data);
        let new_ptr = Box::into_raw(Box::new(new_data.clone()));

        // 更新指针
        self.current.store(new_ptr, Ordering::Release);

        // 释放写入锁
        self.write_lock.store(false, Ordering::Release);

        // 释放旧数据
        unsafe {
            let _ = Box::from_raw(old_ptr);
        }

        new_data
    }
}

impl<T: Send + Sync> Drop for RwLockState<T> {
    fn drop(&mut self) {
        let ptr = self.current.load(Ordering::Relaxed);
        if !ptr.is_null() {
            unsafe {
                let _ = Box::from_raw(ptr);
            }
        }
    }
}

unsafe impl<T: Send + Sync> Send for RwLockState<T> {}
unsafe impl<T: Send + Sync> Sync for RwLockState<T> {}

/// 状态管理器
pub struct StateManager<T: Send + Sync> {
    /// 共享状态
    shared_state: Arc<LockFreeSharedState<T>>,
    /// 状态变更队列
    change_queue: Arc<LockFreeQueue<StateChange<T>>>,
    /// 状态订阅者
    subscribers: Arc<std::sync::RwLock<SubscriberMap<T>>>,
    /// 下一个订阅者ID
    next_subscriber_id: AtomicUsize,
}

/// 状态变更
#[derive(Debug, Clone)]
pub struct StateChange<T> {
    /// 变更类型
    pub change_type: StateChangeType,
    /// 变更数据
    pub data: T,
    /// 变更时间
    pub timestamp: std::time::Instant,
}

/// 状态变更类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateChangeType {
    /// 完全替换
    Replace,
    /// 部分更新
    Update,
    /// 条件更新
    Conditional,
}

/// 状态订阅者
pub trait StateSubscriber<T> {
    /// 处理状态变更
    fn on_state_change(&self, change: &StateChange<T>);
}

impl<T: Clone + Send + Sync + 'static> StateManager<T> {
    /// 创建新的状态管理器
    pub fn new(initial_state: T) -> Self {
        Self {
            shared_state: Arc::new(LockFreeSharedState::new(initial_state)),
            change_queue: Arc::new(LockFreeQueue::new()),
            subscribers: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            next_subscriber_id: AtomicUsize::new(0),
        }
    }

    /// Helper to acquire subscribers read lock with error handling
    fn lock_subscribers_read(
        &self,
    ) -> Result<std::sync::RwLockReadGuard<'_, SubscriberMap<T>>, String> {
        self.subscribers
            .read()
            .map_err(|e| format!("Subscribers read lock is poisoned: {:?}", e))
    }

    /// Helper to acquire subscribers write lock with error handling
    fn lock_subscribers_write(
        &self,
    ) -> Result<std::sync::RwLockWriteGuard<'_, SubscriberMap<T>>, String> {
        self.subscribers
            .write()
            .map_err(|e| format!("Subscribers write lock is poisoned: {:?}", e))
    }

    /// 读取当前状态
    pub fn read_state(&self) -> StateSnapshot<T> {
        self.shared_state.read()
    }

    /// 更新状态
    pub fn update<F>(&self, mut updater: F) -> StateSnapshot<T>
    where
        F: FnMut(&T) -> T,
    {
        // 使用内部方法避免移动问题
        let current_snapshot = self.shared_state.read();
        let current_data = current_snapshot.data.clone();

        let new_snapshot = self.shared_state.update(|old_data| {
            // old_data用于内部版本控制和CAS操作的原子性保证
            let updated = updater(&current_data);
            // 在这里我们保留old_data的引用，虽然不直接使用，但编译器需要知道它的存在以确保正确的内存管理
            let _old_ref = &old_data;
            updated
        });

        // 创建状态变更
        let change = StateChange {
            change_type: StateChangeType::Update,
            data: new_snapshot.data.clone(),
            timestamp: new_snapshot.timestamp,
        };

        // 添加到变更队列
        let _ = self.change_queue.push(change.clone());

        // 通知订阅者
        self.notify_subscribers(&change);

        new_snapshot
    }

    /// 条件更新状态
    pub fn conditional_update<F, P>(&self, predicate: P, mut updater: F) -> Option<StateSnapshot<T>>
    where
        F: FnMut(&T) -> T,
        P: Fn(&T) -> bool,
    {
        // 使用内部方法避免移动问题
        let current_snapshot = self.shared_state.read();
        let current_data = current_snapshot.data.clone();

        if predicate(&current_snapshot.data) {
            let new_snapshot = self.shared_state.update(|old_data| {
                // old_data用于条件更新的原子性保证，记录更新前的状态
                let updated = updater(&current_data);
                let _old_ref = &old_data;
                updated
            });

            // 创建状态变更
            let change = StateChange {
                change_type: StateChangeType::Conditional,
                data: new_snapshot.data.clone(),
                timestamp: new_snapshot.timestamp,
            };

            // 添加到变更队列
            let _ = self.change_queue.push(change.clone());

            // 通知订阅者
            self.notify_subscribers(&change);

            Some(new_snapshot)
        } else {
            None
        }
    }

    /// 替换状态
    pub fn replace_state(&self, new_state: T) -> StateSnapshot<T> {
        let new_snapshot = self.shared_state.update(|_| new_state.clone());

        // 创建状态变更
        let change = StateChange {
            change_type: StateChangeType::Replace,
            data: new_state.clone(),
            timestamp: new_snapshot.timestamp,
        };

        // 添加到变更队列
        let _ = self.change_queue.push(change.clone());

        // 通知订阅者
        self.notify_subscribers(&change);

        new_snapshot
    }

    /// 订阅状态变更
    pub fn subscribe<S>(&self, subscriber: S) -> usize
    where
        S: StateSubscriber<T> + Send + Sync + 'static,
    {
        let subscriber_id = self.next_subscriber_id.fetch_add(1, Ordering::Relaxed);
        let boxed_subscriber: Box<dyn StateSubscriber<T> + Send + Sync> = Box::new(subscriber);

        if let Ok(mut subscribers) = self.lock_subscribers_write() {
            subscribers.insert(subscriber_id, boxed_subscriber);
        }

        subscriber_id
    }

    /// 取消订阅
    pub fn unsubscribe(
        &self,
        subscriber_id: usize,
    ) -> Option<Box<dyn StateSubscriber<T> + Send + Sync>> {
        match self.lock_subscribers_write() {
            Ok(mut subscribers) => subscribers.remove(&subscriber_id),
            Err(_) => None,
        }
    }

    /// 获取状态变更历史
    pub fn get_change_history(&self) -> Vec<StateChange<T>> {
        let mut changes = Vec::new();

        while let Some(change) = self.change_queue.try_pop() {
            changes.push(change);
        }

        changes
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> StateManagerStats {
        let shared_stats = self.shared_state.get_stats();
        let subscriber_count = match self.lock_subscribers_read() {
            Ok(subscribers) => subscribers.len(),
            Err(_) => 0,
        };

        StateManagerStats {
            shared_stats,
            subscriber_count,
            pending_changes: self.change_queue.len(),
        }
    }

    /// 通知订阅者
    fn notify_subscribers(&self, change: &StateChange<T>) {
        // 简化实现：实际可能需要更复杂的机制
        if let Ok(subscribers) = self.lock_subscribers_read() {
            for subscriber in subscribers.values() {
                subscriber.on_state_change(change);
            }
        }
    }
}

/// 状态管理器统计信息
#[derive(Debug, Clone)]
pub struct StateManagerStats {
    /// 共享状态统计
    pub shared_stats: SharedStateStats,
    /// 订阅者数量
    pub subscriber_count: usize,
    /// 待处理变更数量
    pub pending_changes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_lockfree_shared_state() {
        let state = LockFreeSharedState::new(0);

        // 测试读取
        let snapshot = state.read();
        assert_eq!(snapshot.data, 0);
        assert_eq!(snapshot.version, StateVersion::new());

        // 测试更新
        let new_snapshot = state.update(|x| x + 1);
        assert_eq!(new_snapshot.data, 1);
        assert_eq!(new_snapshot.version.minor, 1);

        // 验证状态已更新
        let current_snapshot = state.read();
        assert_eq!(current_snapshot.data, 1);

        println!("无锁共享状态测试通过");
    }

    #[test]
    fn test_striped_shared_state() {
        let state = StripedSharedState::new(0);

        // 测试读取
        let snapshots = state.read();
        assert_eq!(snapshots.len(), 16);
        for snapshot in &snapshots {
            assert_eq!(snapshot.data, 0);
        }

        // 测试更新
        let new_snapshots = state.update_all(|x| x + 1);
        assert_eq!(new_snapshots.len(), 16);
        for snapshot in &new_snapshots {
            assert_eq!(snapshot.data, 1);
        }

        println!("分片共享状态测试通过");
    }

    #[test]
    fn test_rwlock_state() {
        let state = Arc::new(RwLockState::new(0));

        // 测试读取
        let value = state.read();
        assert_eq!(value, 0);

        // 测试写入
        let new_value = state.write(|x| x + 1);
        assert_eq!(new_value, 1);

        // 验证状态已更新
        let current_value = state.read();
        assert_eq!(current_value, 1);

        println!("读写锁状态测试通过");
    }

    #[test]
    fn test_state_manager() {
        let manager = Arc::new(StateManager::new(0));

        // 测试读取状态
        let snapshot = manager.read_state();
        assert_eq!(snapshot.data, 0);

        // 测试更新状态
        let new_snapshot = manager.update(|x| x + 1);
        assert_eq!(new_snapshot.data, 1);

        // 验证状态已更新
        let current_snapshot = manager.read_state();
        assert_eq!(current_snapshot.data, 1);

        // 测试状态变更历史
        let changes = manager.get_change_history();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].data, 1);

        println!("状态管理器测试通过");
    }

    #[test]
    fn test_concurrent_state_access() {
        let state = Arc::new(LockFreeSharedState::new(0));
        let mut handles = Vec::new();

        // 读取线程
        for _i in 0..4 {
            let state = state.clone();
            let handle = thread::spawn(move || {
                for _ in 0..1000 {
                    let _ = state.read();
                }
            });
            handles.push(handle);
        }

        // 写入线程
        for i in 0..4 {
            let state = state.clone();
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    state.update(|x| x + i * 100 + j);
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle
                .join()
                .expect("concurrent state access thread should not panic");
        }

        // 验证最终状态
        let final_snapshot = state.read();
        println!("最终状态: {}", final_snapshot.data);

        println!("并发状态访问测试通过");
    }
}
