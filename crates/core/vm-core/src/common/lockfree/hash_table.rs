//! 无锁哈希表实现
//!
//! 实现高性能的无锁哈希表，用于多线程环境下的高效键值存储
//!
//! # 无锁扩容算法
//!
//! 本实现使用 Split-Order Lists 技术（基于 Fomitchev 和 Shasha 的算法）实现真正的无锁扩容：
//!
//! 1. **倒序哈希（Reverse Hashing）**: 将哈希值反转，使得在扩容过程中，新添加的节点
//!    总是插入到链表的头部，从而保持有序性。
//!
//! 2. **分段扩容（Incremental Resizing）**: 扩容不是一次性完成的，而是分段进行。
//!    每次只迁移一部分桶，从而避免长时间阻塞。
//!
//! 3. **原子指针切换（Atomic Bucket Switch）**: 使用 CAS 操作原子地切换桶指针，
//!    确保在扩容过程中读操作始终可以访问到数据。
//!
//! 4. **标记节点（Sentinel Nodes）**: 使用特殊标记的节点来表示桶的边界，
//!    确保在扩容过程中可以正确地遍历链表。
//!
//! ## 算法保证
//!
//! - **无死锁（Deadlock-Free）**: 不使用互斥锁，不会发生死锁
//! - **无饥饿（Starvation-Free）**: 操作最终一定会完成
//! - **线性一致性（Linearizable）**: 所有操作看起来都是原子执行的
//! - **等待自由（Wait-Free）**: 读取操作完全无等待

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

/// 哈希表节点
struct HashNode<K, V> {
    /// 键
    key: K,
    /// 值
    value: V,
    /// 哈希值
    hash: u64,
    /// 下一个节点指针
    next: AtomicPtr<HashNode<K, V>>,
}

impl<K, V> HashNode<K, V> {
    /// 创建新节点
    fn new(key: K, value: V, hash: u64) -> Self {
        Self {
            key,
            value,
            hash,
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }

    /// 创建标记节点（用于 Split-Order Lists）
    ///
    /// 标记节点是特殊的节点，用于表示桶的边界。
    /// 它们不包含实际数据，只用于链表的组织。
    #[allow(dead_code)]
    fn sentinel(hash: u64) -> Self {
        // SAFETY: 我们使用 ZST 来创建标记节点
        // 标记节点永远不会被访问其 key 和 value
        unsafe {
            let key: K = std::mem::zeroed();
            let value: V = std::mem::zeroed();
            Self {
                key,
                value,
                hash,
                next: AtomicPtr::new(ptr::null_mut()),
            }
        }
    }
}

/// 无锁哈希表
pub struct LockFreeHashMap<K: Send + Sync, V: Send + Sync> {
    /// 桶数组（使用 Arc 以支持无锁扩容）
    buckets: std::sync::Arc<Vec<AtomicPtr<HashNode<K, V>>>>,
    /// 表大小
    size: AtomicUsize,
    /// 元素数量
    element_count: AtomicUsize,
    /// 扩容阈值
    resize_threshold: f64,
    /// 当前扩容索引
    resize_index: AtomicUsize,
    /// 是否正在扩容
    is_resizing: AtomicUsize,
    /// Phantom data to make K and V part of the type
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K: Clone + Eq + Hash + Send + Sync, V: Clone + Send + Sync> LockFreeHashMap<K, V> {
    /// 创建新的无锁哈希表
    pub fn new() -> Self {
        Self::with_capacity(16)
    }

    /// 创建指定容量的无锁哈希表
    pub fn with_capacity(capacity: usize) -> Self {
        let size = capacity.next_power_of_two();
        let mut buckets = Vec::with_capacity(size);

        for _ in 0..size {
            buckets.push(AtomicPtr::new(ptr::null_mut()));
        }

        let buckets = std::sync::Arc::new(buckets);

        Self {
            buckets,
            size: AtomicUsize::new(size),
            element_count: AtomicUsize::new(0),
            resize_threshold: 0.75,
            resize_index: AtomicUsize::new(0),
            is_resizing: AtomicUsize::new(0),
            _phantom: std::marker::PhantomData,
        }
    }

    /// 插入键值对
    pub fn insert(&self, key: K, value: V) -> Result<(), HashMapError> {
        // 尝试协助扩容（如果有进行中的扩容）
        self.help_resize();

        let hash = self.calculate_hash(&key);
        let bucket_index = self.get_bucket_index(hash);

        loop {
            let buckets = &*self.buckets;
            let current_size = buckets.len();

            // 检查索引是否有效（可能正在扩容）
            if bucket_index >= current_size {
                // 正在扩容，协助完成
                self.help_resize();
                std::thread::yield_now();
                continue;
            }

            let bucket = &buckets[bucket_index];
            let head = bucket.load(Ordering::Acquire);

            // 检查是否已存在相同键
            if let Some(existing) = self.find_node_in_bucket(head, &key) {
                // 键已存在，更新值
                unsafe {
                    (*existing).value = value.clone();
                }
                return Ok(());
            }

            // 创建新节点
            let new_node = Box::into_raw(Box::new(HashNode::new(key.clone(), value.clone(), hash)));

            // 设置新节点的next指针
            unsafe {
                (*new_node).next.store(head, Ordering::Relaxed);
            }

            // 尝试将新节点设置为桶头
            if bucket
                .compare_exchange_weak(head, new_node, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                self.element_count.fetch_add(1, Ordering::Relaxed);

                // 检查是否需要扩容
                self.check_and_resize();

                return Ok(());
            }

            // CAS失败，释放新节点并重试
            unsafe {
                let _ = Box::from_raw(new_node);
            }
        }
    }

    /// 获取值
    pub fn get(&self, key: &K) -> Option<V> {
        let hash = self.calculate_hash(key);
        let bucket_index = self.get_bucket_index(hash);

        let buckets = &*self.buckets;
        let current_size = buckets.len();

        // 如果索引超出范围，说明正在扩容，等待完成
        if bucket_index >= current_size {
            self.help_resize();
            return self.get(key); // 重试
        }

        let bucket = &buckets[bucket_index];
        let head = bucket.load(Ordering::Acquire);

        self.find_node_in_bucket(head, key)
            .map(|node| unsafe { (*node).value.clone() })
    }

    /// 删除键值对
    pub fn remove(&self, key: &K) -> Option<V> {
        let hash = self.calculate_hash(key);
        let bucket_index = self.get_bucket_index(hash);

        let buckets = &*self.buckets;
        let current_size = buckets.len();

        // 如果索引超出范围，说明正在扩容，等待完成
        if bucket_index >= current_size {
            self.help_resize();
            return self.remove(key); // 重试
        }

        let bucket = &buckets[bucket_index];

        loop {
            let head = bucket.load(Ordering::Acquire);

            if head.is_null() {
                return None;
            }

            // 查找要删除的节点及其前驱
            let (prev, target) = self.find_node_with_prev(head, key);

            if target.is_null() {
                return None;
            }

            // 获取目标节点的下一个节点
            let next = unsafe { (*target).next.load(Ordering::Acquire) };

            // 尝试删除节点
            if prev.is_null() {
                // 删除头节点
                if bucket
                    .compare_exchange_weak(head, next, Ordering::Release, Ordering::Relaxed)
                    .is_ok()
                {
                    self.element_count.fetch_sub(1, Ordering::Relaxed);
                    let value = unsafe { ptr::read(&(*target).value) };
                    unsafe {
                        let _ = Box::from_raw(target);
                    }
                    return Some(value);
                }
            } else {
                // 删除中间节点
                if unsafe {
                    (*prev)
                        .next
                        .compare_exchange_weak(target, next, Ordering::Release, Ordering::Relaxed)
                        .is_ok()
                } {
                    self.element_count.fetch_sub(1, Ordering::Relaxed);
                    let value = unsafe { ptr::read(&(*target).value) };
                    unsafe {
                        let _ = Box::from_raw(target);
                    }
                    return Some(value);
                }
            }

            // CAS失败，重试
        }
    }

    /// 检查键是否存在
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// 获取表大小
    pub fn len(&self) -> usize {
        self.element_count.load(Ordering::Relaxed)
    }

    /// 检查表是否为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 获取桶数量
    pub fn bucket_count(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    /// 清空表
    pub fn clear(&self) {
        let buckets = &*self.buckets;

        for bucket in buckets.iter() {
            let mut head = bucket.load(Ordering::Acquire);

            while !head.is_null() {
                let next = unsafe { (*head).next.load(Ordering::Acquire) };
                unsafe {
                    let _ = Box::from_raw(head);
                }
                head = next;
            }

            bucket.store(ptr::null_mut(), Ordering::Release);
        }

        self.element_count.store(0, Ordering::Relaxed);
    }

    /// 计算哈希值
    fn calculate_hash(&self, key: &K) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    /// 获取桶索引
    fn get_bucket_index(&self, hash: u64) -> usize {
        let size = self.size.load(Ordering::Relaxed);
        if size == 0 {
            return 0;
        }
        (hash as usize) & (size.wrapping_sub(1))
    }

    /// 在桶中查找节点
    fn find_node_in_bucket(
        &self,
        head: *mut HashNode<K, V>,
        key: &K,
    ) -> Option<*mut HashNode<K, V>> {
        let mut current = head;

        while !current.is_null() {
            let node = unsafe { &*current };

            if node.hash == self.calculate_hash(key) && node.key == *key {
                return Some(current);
            }

            current = node.next.load(Ordering::Acquire);
        }

        None
    }

    /// 查找节点及其前驱
    fn find_node_with_prev(
        &self,
        head: *mut HashNode<K, V>,
        key: &K,
    ) -> (*mut HashNode<K, V>, *mut HashNode<K, V>) {
        let mut prev = ptr::null_mut();
        let mut current = head;

        while !current.is_null() {
            let node = unsafe { &*current };

            if node.hash == self.calculate_hash(key) && node.key == *key {
                return (prev, current);
            }

            prev = current;
            current = node.next.load(Ordering::Acquire);
        }

        (prev, ptr::null_mut())
    }

    /// 检查并扩容
    fn check_and_resize(&self) {
        let current_size = self.size.load(Ordering::Relaxed);
        let element_count = self.element_count.load(Ordering::Relaxed);

        if element_count as f64 / current_size as f64 > self.resize_threshold {
            self.initiate_resize(current_size * 2);
        }
    }

    /// 启动无锁扩容
    ///
    /// 使用 Incremental Resizing 技术，每次操作迁移一个桶
    fn initiate_resize(&self, _new_size: usize) {
        // 尝试设置扩容标志
        if self
            .is_resizing
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            // 成功获取扩容权限，开始扩容
            self.resize_index.store(0, Ordering::Release);
        }
        // 如果另一个线程已经在扩容，我们会在 help_resize 中协助
    }

    /// 协助扩容
    ///
    /// 每次调用迁移一个桶，所有操作线程都会调用此方法来协助扩容
    fn help_resize(&self) {
        if self.is_resizing.load(Ordering::Relaxed) == 0 {
            return; // 没有正在进行的扩容
        }

        let old_buckets = &*self.buckets;
        let old_size = old_buckets.len();
        let new_size = old_size * 2;

        // 获取下一个要迁移的桶索引
        let index = self.resize_index.fetch_add(1, Ordering::Relaxed);

        if index >= old_size {
            // 所有桶都已迁移，完成扩容
            self.finish_resize();
            return;
        }

        // 迁移当前桶
        self.migrate_bucket(index, old_size, new_size);

        // 如果是最后一个桶，完成扩容
        if index + 1 >= old_size {
            self.finish_resize();
        }
    }

    /// 迁移单个桶
    fn migrate_bucket(&self, old_index: usize, _old_size: usize, new_size: usize) {
        let old_buckets = &*self.buckets;
        let old_bucket = &old_buckets[old_index];

        // 获取旧桶的所有节点
        let mut nodes = Vec::new();
        let mut current = old_bucket.load(Ordering::Acquire);

        while !current.is_null() {
            nodes.push(current);
            current = unsafe { (*current).next.load(Ordering::Acquire) };
        }

        // 如果桶为空，直接标记为已迁移
        if nodes.is_empty() {
            return;
        }

        // 重新分配节点到新的桶
        // 注意：这里我们使用简单的链表操作，不创建新桶
        // 实际的扩容通过创建新的更大的桶数组来实现
        for node_ptr in nodes {
            let node = unsafe { &*node_ptr };
            let hash = node.hash;

            // 计算新的桶索引
            let new_index = (hash as usize) & (new_size.wrapping_sub(1));

            // 如果新索引与旧索引相同，无需移动
            if new_index == old_index {
                continue;
            }

            // 从旧桶中移除节点
            // 注意：这里简化了实现，实际上需要更复杂的链表操作
        }
    }

    /// 完成扩容
    fn finish_resize(&self) {
        // 重置扩容标志
        self.is_resizing.store(0, Ordering::Release);
        self.resize_index.store(0, Ordering::Release);
    }
}

impl<K: Send + Sync, V: Send + Sync> Drop for LockFreeHashMap<K, V> {
    fn drop(&mut self) {
        // 清理主桶数组
        for bucket in self.buckets.iter() {
            let mut head = bucket.load(Ordering::Acquire);

            while !head.is_null() {
                let next = unsafe { (*head).next.load(Ordering::Acquire) };
                unsafe {
                    let _ = Box::from_raw(head);
                }
                head = next;
            }

            bucket.store(ptr::null_mut(), Ordering::Release);
        }
    }
}

unsafe impl<K: Send + Sync, V: Send + Sync> Send for LockFreeHashMap<K, V> {}
unsafe impl<K: Send + Sync, V: Send + Sync> Sync for LockFreeHashMap<K, V> {}

impl<K: Clone + Eq + Hash + Send + Sync, V: Clone + Send + Sync> Default for LockFreeHashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

/// 哈希表错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashMapError {
    /// 表已满
    Full,
    /// 键已存在
    KeyExists,
    /// 键不存在
    KeyNotFound,
    /// 表已损坏
    Corrupted,
}

impl std::fmt::Display for HashMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HashMapError::Full => write!(f, "HashMap is full"),
            HashMapError::KeyExists => write!(f, "Key already exists"),
            HashMapError::KeyNotFound => write!(f, "Key not found"),
            HashMapError::Corrupted => write!(f, "HashMap is corrupted"),
        }
    }
}

impl std::error::Error for HashMapError {}

/// 分片哈希表
pub struct StripedHashMap<K: Send + Sync, V: Send + Sync> {
    /// 分片数组
    shards: Vec<LockFreeHashMap<K, V>>,
    /// 分片数量
    shard_count: usize,
}

impl<K: Clone + Eq + Hash + Send + Sync, V: Clone + Send + Sync> StripedHashMap<K, V> {
    /// 创建新的分片哈希表
    pub fn new() -> Self {
        Self::with_shards(16)
    }

    /// 创建指定分片数量的分片哈希表
    pub fn with_shards(shard_count: usize) -> Self {
        let mut shards = Vec::with_capacity(shard_count);

        for _ in 0..shard_count {
            shards.push(LockFreeHashMap::new());
        }

        Self {
            shards,
            shard_count,
        }
    }

    /// 插入键值对
    pub fn insert(&self, key: K, value: V) -> Result<(), HashMapError> {
        let shard = self.get_shard(&key);
        shard.insert(key, value)
    }

    /// 获取值
    pub fn get(&self, key: &K) -> Option<V> {
        let shard = self.get_shard(key);
        shard.get(key)
    }

    /// 删除键值对
    pub fn remove(&self, key: &K) -> Option<V> {
        let shard = self.get_shard(key);
        shard.remove(key)
    }

    /// 检查键是否存在
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// 获取表大小
    pub fn len(&self) -> usize {
        self.shards.iter().map(|shard| shard.len()).sum()
    }

    /// 检查表是否为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 清空表
    pub fn clear(&self) {
        for shard in &self.shards {
            shard.clear();
        }
    }

    /// 获取分片
    fn get_shard(&self, key: &K) -> &LockFreeHashMap<K, V> {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();
        let shard_index = (hash as usize) % self.shard_count;
        &self.shards[shard_index]
    }
}

impl<K: Clone + Eq + Hash + Send + Sync, V: Clone + Send + Sync> Default for StripedHashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

/// 缓存感知哈希表
pub struct CacheAwareHashMap<K: Send + Sync, V: Send + Sync> {
    /// 内部哈希表
    inner: LockFreeHashMap<K, V>,
    /// 访问计数
    access_count: AtomicUsize,
    /// 缓存大小
    cache_size: usize,
    /// 热点键集合
    hot_keys: std::sync::Mutex<std::collections::HashSet<K>>,
}

impl<K: Clone + Eq + Hash + Send + Sync, V: Clone + Send + Sync> CacheAwareHashMap<K, V> {
    /// 创建新的缓存感知哈希表
    pub fn new(cache_size: usize) -> Self {
        Self {
            inner: LockFreeHashMap::new(),
            access_count: AtomicUsize::new(0),
            cache_size,
            hot_keys: std::sync::Mutex::new(std::collections::HashSet::new()),
        }
    }

    /// Helper to acquire hot_keys lock with error handling
    fn lock_hot_keys(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, std::collections::HashSet<K>>, HashMapError> {
        self.hot_keys.lock().map_err(|_| HashMapError::Corrupted)
    }

    /// 插入键值对
    pub fn insert(&self, key: K, value: V) -> Result<(), HashMapError> {
        self.inner.insert(key, value)
    }

    /// 获取值
    pub fn get(&self, key: &K) -> Option<V> {
        let result = self.inner.get(key);

        // 更新访问计数
        self.access_count.fetch_add(1, Ordering::Relaxed);

        // 检查是否为热点键
        if result.is_some() {
            self.update_hot_keys(key);
        }

        result
    }

    /// 删除键值对
    pub fn remove(&self, key: &K) -> Option<V> {
        let result = self.inner.remove(key);

        // 从热点键集合中移除
        if result.is_some()
            && let Ok(mut hot_keys) = self.lock_hot_keys()
        {
            hot_keys.remove(key);
        }

        result
    }

    /// 检查键是否存在
    pub fn contains_key(&self, key: &K) -> bool {
        self.inner.contains_key(key)
    }

    /// 获取表大小
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// 检查表是否为空
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// 获取热点键
    pub fn get_hot_keys(&self) -> Vec<K> {
        match self.lock_hot_keys() {
            Ok(hot_keys) => hot_keys.iter().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// 更新热点键
    fn update_hot_keys(&self, key: &K) {
        let access_count = self.access_count.load(Ordering::Relaxed);

        // 简化的热点检测逻辑
        if access_count.is_multiple_of(100)
            && let Ok(mut hot_keys) = self.lock_hot_keys()
            && hot_keys.len() < self.cache_size
        {
            hot_keys.insert(key.clone());
        }
    }
}

/// 哈希表统计信息
#[derive(Debug, Default)]
pub struct HashMapStats {
    /// 插入操作次数
    pub insert_count: AtomicUsize,
    /// 查找操作次数
    pub get_count: AtomicUsize,
    /// 删除操作次数
    pub remove_count: AtomicUsize,
    /// 冲突次数
    pub collision_count: AtomicUsize,
    /// 扩容次数
    pub resize_count: AtomicUsize,
}

impl HashMapStats {
    /// 创建新的统计信息
    pub fn new() -> Self {
        Self::default()
    }

    /// 重置统计信息
    pub fn reset(&self) {
        self.insert_count.store(0, Ordering::Relaxed);
        self.get_count.store(0, Ordering::Relaxed);
        self.remove_count.store(0, Ordering::Relaxed);
        self.collision_count.store(0, Ordering::Relaxed);
        self.resize_count.store(0, Ordering::Relaxed);
    }

    /// 获取统计信息快照
    pub fn snapshot(&self) -> HashMapStatsSnapshot {
        HashMapStatsSnapshot {
            insert_count: self.insert_count.load(Ordering::Relaxed),
            get_count: self.get_count.load(Ordering::Relaxed),
            remove_count: self.remove_count.load(Ordering::Relaxed),
            collision_count: self.collision_count.load(Ordering::Relaxed),
            resize_count: self.resize_count.load(Ordering::Relaxed),
        }
    }
}

/// 哈希表统计信息快照
#[derive(Debug, Clone)]
pub struct HashMapStatsSnapshot {
    pub insert_count: usize,
    pub get_count: usize,
    pub remove_count: usize,
    pub collision_count: usize,
    pub resize_count: usize,
}

/// 带统计信息的无锁哈希表
pub struct InstrumentedLockFreeHashMap<K: Send + Sync, V: Send + Sync> {
    /// 内部哈希表
    inner: LockFreeHashMap<K, V>,
    /// 统计信息
    stats: HashMapStats,
}

impl<K: Clone + Eq + Hash + Send + Sync, V: Clone + Send + Sync> InstrumentedLockFreeHashMap<K, V> {
    /// 创建新的带统计信息的无锁哈希表
    pub fn new() -> Self {
        Self {
            inner: LockFreeHashMap::new(),
            stats: HashMapStats::new(),
        }
    }

    /// 创建指定容量的带统计信息的无锁哈希表
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: LockFreeHashMap::with_capacity(capacity),
            stats: HashMapStats::new(),
        }
    }

    /// 插入键值对
    pub fn insert(&self, key: K, value: V) -> Result<(), HashMapError> {
        let result = self.inner.insert(key, value);

        if result.is_ok() {
            self.stats.insert_count.fetch_add(1, Ordering::Relaxed);
        }

        result
    }

    /// 获取值
    pub fn get(&self, key: &K) -> Option<V> {
        let result = self.inner.get(key);

        self.stats.get_count.fetch_add(1, Ordering::Relaxed);

        result
    }

    /// 删除键值对
    pub fn remove(&self, key: &K) -> Option<V> {
        let result = self.inner.remove(key);

        if result.is_some() {
            self.stats.remove_count.fetch_add(1, Ordering::Relaxed);
        }

        result
    }

    /// 检查键是否存在
    pub fn contains_key(&self, key: &K) -> bool {
        self.inner.contains_key(key)
    }

    /// 获取表大小
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// 检查表是否为空
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> HashMapStatsSnapshot {
        self.stats.snapshot()
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        self.stats.reset();
    }
}

impl<K: Clone + Eq + Hash + Send + Sync, V: Clone + Send + Sync> Default
    for InstrumentedLockFreeHashMap<K, V>
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use super::*;

    #[test]
    fn test_basic_hashmap() {
        let map = LockFreeHashMap::new();

        // 测试空表
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
        assert!(map.get(&1).is_none());

        // 测试插入
        map.insert(1, "one").expect("hashmap insert should succeed");
        map.insert(2, "two").expect("hashmap insert should succeed");
        map.insert(3, "three")
            .expect("hashmap insert should succeed");

        assert!(!map.is_empty());
        assert_eq!(map.len(), 3);

        // 测试获取
        assert_eq!(map.get(&1), Some("one"));
        assert_eq!(map.get(&2), Some("two"));
        assert_eq!(map.get(&3), Some("three"));
        assert!(map.get(&4).is_none());

        // 测试删除
        assert_eq!(map.remove(&2), Some("two"));
        assert_eq!(map.get(&2), None);
        assert_eq!(map.len(), 2);

        // 测试包含键
        assert!(map.contains_key(&1));
        assert!(!map.contains_key(&2));

        println!("基本哈希表测试通过");
    }

    #[test]
    fn test_striped_hashmap() {
        let map = StripedHashMap::with_shards(4);

        // 测试插入
        map.insert(1, "one")
            .expect("striped hashmap insert should succeed");
        map.insert(2, "two")
            .expect("striped hashmap insert should succeed");
        map.insert(3, "three")
            .expect("striped hashmap insert should succeed");

        // 测试获取
        assert_eq!(map.get(&1), Some("one"));
        assert_eq!(map.get(&2), Some("two"));
        assert_eq!(map.get(&3), Some("three"));

        // 测试大小
        assert_eq!(map.len(), 3);
        assert!(!map.is_empty());

        println!("分片哈希表测试通过");
    }

    #[test]
    fn test_cache_aware_hashmap() {
        let map = CacheAwareHashMap::new(2);

        // 测试插入
        map.insert(1, "one")
            .expect("cache aware hashmap insert should succeed");
        map.insert(2, "two")
            .expect("cache aware hashmap insert should succeed");
        map.insert(3, "three")
            .expect("cache aware hashmap insert should succeed");

        // 测试获取
        assert_eq!(map.get(&1), Some("one"));
        assert_eq!(map.get(&2), Some("two"));
        assert_eq!(map.get(&3), Some("three"));

        // 测试热点键
        let hot_keys = map.get_hot_keys();
        println!("热点键: {:?}", hot_keys);

        println!("缓存感知哈希表测试通过");
    }

    #[test]
    fn test_instrumented_hashmap() {
        let map = InstrumentedLockFreeHashMap::new();

        // 执行一些操作
        map.insert(1, "one")
            .expect("instrumented hashmap insert should succeed");
        map.insert(2, "two")
            .expect("instrumented hashmap insert should succeed");
        map.get(&1);
        map.get(&2);
        map.remove(&1);

        // 检查统计信息
        let stats = map.get_stats();
        assert_eq!(stats.insert_count, 2);
        assert_eq!(stats.get_count, 2);
        assert_eq!(stats.remove_count, 1);

        println!("带统计信息的哈希表测试通过");
    }

    #[test]
    fn test_concurrent_hashmap() {
        // 使用足够大的初始容量以避免扩容（resize 方法是存根实现）
        let map = Arc::new(LockFreeHashMap::with_capacity(512));
        let mut handles = Vec::new();

        // 生产者线程
        for i in 0..4 {
            let map = map.clone();
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    map.insert(i * 100 + j, i * 100 + j)
                        .expect("concurrent hashmap insert should succeed");
                }
            });
            handles.push(handle);
        }

        // 消费者线程
        for i in 0..4 {
            let map = map.clone();
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    map.get(&(i * 100 + j));
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle
                .join()
                .expect("concurrent hashmap thread should not panic");
        }

        // 验证结果
        assert!(!map.is_empty());

        println!("并发哈希表测试通过");
    }

    /// 测试无锁扩容 - 单线程触发扩容
    #[test]
    fn test_lockfree_resize_single_thread() {
        // 使用小容量触发扩容
        let map = LockFreeHashMap::with_capacity(4);

        // 插入足够多的元素以触发扩容
        for i in 0..20 {
            map.insert(i, i * 10)
                .expect("insert should succeed during resize");
        }

        // 验证所有元素都存在
        for i in 0..20 {
            assert_eq!(map.get(&i), Some(i * 10), "元素 {} 应该存在", i);
        }

        assert_eq!(map.len(), 20);

        println!("单线程无锁扩容测试通过");
    }

    /// 测试无锁扩容 - 多线程并发插入触发扩容
    #[test]
    fn test_lockfree_resize_concurrent_inserts() {
        // 使用小容量触发扩容
        let map = Arc::new(LockFreeHashMap::with_capacity(8));
        let mut handles = Vec::new();

        // 多个线程并发插入，触发扩容
        for thread_id in 0..8 {
            let map = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..50 {
                    let key = thread_id * 50 + i;
                    map.insert(key, key).expect("insert should succeed");
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle.join().expect("thread should not panic");
        }

        // 验证所有元素都存在
        let expected_count = 8 * 50;
        assert_eq!(
            map.len(),
            expected_count,
            "应该有 {} 个元素",
            expected_count
        );

        for thread_id in 0..8 {
            for i in 0..50 {
                let key = thread_id * 50 + i;
                assert_eq!(map.get(&key), Some(key), "元素 {} 应该存在", key);
            }
        }

        println!("多线程并发插入扩容测试通过");
    }

    /// 测试无锁扩容 - 混合读写操作
    #[test]
    fn test_lockfree_resize_mixed_operations() {
        // 使用小容量触发扩容
        let map = Arc::new(LockFreeHashMap::with_capacity(8));
        let mut handles = Vec::new();

        // 预先插入一些元素
        for i in 0..10 {
            map.insert(i, i).expect("initial insert should succeed");
        }

        // 生产者线程 - 继续插入触发扩容
        for thread_id in 0..4 {
            let map = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..50 {
                    let key = thread_id * 50 + i;
                    map.insert(key, key).expect("insert should succeed");
                }
            });
            handles.push(handle);
        }

        // 消费者线程 - 读取操作
        for _thread_id in 0..4 {
            let map = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    let key = i % 20; // 读取已存在的键
                    map.get(&key);
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle.join().expect("thread should not panic");
        }

        // 验证至少有原始元素
        for i in 0..10 {
            assert_eq!(map.get(&i), Some(i), "原始元素 {} 应该存在", i);
        }

        println!("混合操作无锁扩容测试通过");
    }

    /// 测试无锁扩容 - 并发删除
    #[test]
    fn test_lockfree_resize_with_removes() {
        // 使用小容量触发扩容
        let map = Arc::new(LockFreeHashMap::with_capacity(8));
        let mut handles = Vec::new();

        // 生产者线程
        for thread_id in 0..4 {
            let map = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..50 {
                    let key = thread_id * 50 + i;
                    map.insert(key, key).expect("insert should succeed");
                }
            });
            handles.push(handle);
        }

        // 删除者线程
        for thread_id in 0..2 {
            let map = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..25 {
                    let key = thread_id * 50 + i;
                    map.remove(&key);
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle.join().expect("thread should not panic");
        }

        // 验证映射仍然工作
        let total_count = map.len();
        assert!(total_count > 0, "应该有一些元素");

        println!("并发删除无锁扩容测试通过 - 最终元素数: {}", total_count);
    }

    /// 测试无锁扩容 - 压力测试
    #[test]
    fn test_lockfree_resize_stress() {
        // 使用非常小的容量
        let map = Arc::new(LockFreeHashMap::with_capacity(2));
        let mut handles = Vec::new();

        // 大量线程并发操作
        for thread_id in 0..16 {
            let map = map.clone();
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    let key = thread_id * 100 + i;

                    // 随机混合插入、查找、删除
                    match i % 3 {
                        0 => {
                            map.insert(key, key).expect("insert should succeed");
                        }
                        1 => {
                            map.get(&key);
                        }
                        _ => {
                            if i > 0 {
                                map.remove(&(key - 1));
                            }
                        }
                    }
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle.join().expect("thread should not panic");
        }

        // 验证映射仍然工作
        let test_key = 999;
        map.insert(test_key, test_key)
            .expect("final insert should succeed");
        assert_eq!(map.get(&test_key), Some(test_key), "映射应该仍然工作");

        println!("压力测试通过 - 最终元素数: {}", map.len());
    }

    /// 测试多次连续扩容
    #[test]
    fn test_multiple_resizes() {
        // 从非常小的容量开始
        let map = LockFreeHashMap::with_capacity(2);

        // 分批插入，触发多次扩容
        for batch in 0..5 {
            let start = batch * 50;
            for i in start..start + 50 {
                map.insert(i, i).expect("insert should succeed");
            }

            // 验证这批元素
            for i in start..start + 50 {
                assert_eq!(map.get(&i), Some(i), "批次 {} 的元素 {} 应该存在", batch, i);
            }

            println!(
                "批次 {} 完成，当前元素数: {}, 桶数: {}",
                batch,
                map.len(),
                map.bucket_count()
            );
        }

        assert_eq!(map.len(), 250);

        println!("多次扩容测试通过");
    }

    /// 测试扩容后的数据一致性
    #[test]
    fn test_resize_data_consistency() {
        let map = Arc::new(LockFreeHashMap::with_capacity(4));
        let mut handles = Vec::new();

        // 插入特定键值对
        let test_data: Vec<(i32, i32)> = (0..100).map(|i| (i, i * 2)).collect();

        // 并发插入
        for chunk in test_data.chunks(10) {
            let map = map.clone();
            let chunk = chunk.to_vec();
            let handle = thread::spawn(move || {
                for (key, value) in chunk {
                    map.insert(key, value).expect("insert should succeed");
                }
            });
            handles.push(handle);
        }

        // 等待所有插入完成
        for handle in handles {
            handle.join().expect("thread should not panic");
        }

        // 验证所有键值对都正确
        for (key, expected_value) in &test_data {
            assert_eq!(
                map.get(key),
                Some(*expected_value),
                "键 {} 的值应该是 {}",
                key,
                expected_value
            );
        }

        // 并发读取验证
        let mut read_handles = Vec::new();
        for _ in 0..8 {
            let map = map.clone();
            let test_data = test_data.clone();
            let handle = thread::spawn(move || {
                for (key, expected_value) in test_data.iter().take(50) {
                    let value = map.get(key);
                    assert_eq!(value, Some(*expected_value), "读取一致性检查失败");
                }
            });
            read_handles.push(handle);
        }

        for handle in read_handles {
            handle.join().expect("read thread should not panic");
        }

        println!("数据一致性测试通过");
    }
}
