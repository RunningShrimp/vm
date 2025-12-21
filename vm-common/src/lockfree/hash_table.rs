//! 无锁哈希表实现
//!
//! 实现高性能的无锁哈希表，用于多线程环境下的高效键值存储

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
}

/// 无锁哈希表
pub struct LockFreeHashMap<K: Send + Sync, V: Send + Sync> {
    /// 桶数组
    buckets: Vec<AtomicPtr<HashNode<K, V>>>,
    /// 表大小
    size: AtomicUsize,
    /// 元素数量
    element_count: AtomicUsize,
    /// 扩容阈值
    resize_threshold: f64,
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

        Self {
            buckets,
            size: AtomicUsize::new(size),
            element_count: AtomicUsize::new(0),
            resize_threshold: 0.75,
            _phantom: std::marker::PhantomData,
        }
    }

    /// 插入键值对
    pub fn insert(&self, key: K, value: V) -> Result<(), HashMapError> {
        let hash = self.calculate_hash(&key);
        let bucket_index = self.get_bucket_index(hash);

        loop {
            let bucket = &self.buckets[bucket_index];
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
                drop(Box::from_raw(new_node));
            }
        }
    }

    /// 获取值
    pub fn get(&self, key: &K) -> Option<V> {
        let hash = self.calculate_hash(key);
        let bucket_index = self.get_bucket_index(hash);

        let bucket = &self.buckets[bucket_index];
        let head = bucket.load(Ordering::Acquire);

        self.find_node_in_bucket(head, key)
            .map(|node| unsafe { (*node).value.clone() })
    }

    /// 删除键值对
    pub fn remove(&self, key: &K) -> Option<V> {
        let hash = self.calculate_hash(key);
        let bucket_index = self.get_bucket_index(hash);

        let bucket = &self.buckets[bucket_index];

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
                        drop(Box::from_raw(target));
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
                        drop(Box::from_raw(target));
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
        for bucket in &self.buckets {
            let mut head = bucket.load(Ordering::Acquire);

            while !head.is_null() {
                let next = unsafe { (*head).next.load(Ordering::Acquire) };
                unsafe {
                    drop(Box::from_raw(head));
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
        (hash as usize) & (size - 1)
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
            self.resize(current_size * 2);
        }
    }

    /// 扩容表
    fn resize(&self, new_size: usize) {
        // 简化实现：实际无锁扩容更复杂
        // 这里只是示意，实际生产环境需要更复杂的实现
        if self
            .size
            .compare_exchange(
                self.size.load(Ordering::Relaxed),
                new_size,
                Ordering::Release,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            // 扩容成功，需要重新分配所有节点
            // 实际实现需要更复杂的逻辑来保证无锁
        }
    }
}

impl<K: Send + Sync, V: Send + Sync> Drop for LockFreeHashMap<K, V> {
    fn drop(&mut self) {
        // 手动实现clear逻辑，避免trait bound问题
        for bucket in &self.buckets {
            let mut head = bucket.load(Ordering::Acquire);

            while !head.is_null() {
                let next = unsafe { (*head).next.load(Ordering::Acquire) };
                unsafe {
                    drop(Box::from_raw(head));
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
        if result.is_some() {
            let mut hot_keys = self.hot_keys.lock().unwrap();
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
        let hot_keys = self.hot_keys.lock().unwrap();
        hot_keys.iter().cloned().collect()
    }

    /// 更新热点键
    fn update_hot_keys(&self, key: &K) {
        let access_count = self.access_count.load(Ordering::Relaxed);

        // 简化的热点检测逻辑
        if access_count.is_multiple_of(100) {
            let mut hot_keys = self.hot_keys.lock().unwrap();

            if hot_keys.len() < self.cache_size {
                hot_keys.insert(key.clone());
            }
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
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_basic_hashmap() {
        let map = LockFreeHashMap::new();

        // 测试空表
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
        assert!(map.get(&1).is_none());

        // 测试插入
        map.insert(1, "one").unwrap();
        map.insert(2, "two").unwrap();
        map.insert(3, "three").unwrap();

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
        map.insert(1, "one").unwrap();
        map.insert(2, "two").unwrap();
        map.insert(3, "three").unwrap();

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
        map.insert(1, "one").unwrap();
        map.insert(2, "two").unwrap();
        map.insert(3, "three").unwrap();

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
        map.insert(1, "one").unwrap();
        map.insert(2, "two").unwrap();
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
        let map = Arc::new(LockFreeHashMap::new());
        let mut handles = Vec::new();

        // 生产者线程
        for i in 0..4 {
            let map = map.clone();
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    map.insert(i * 100 + j, i * 100 + j).unwrap();
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
            handle.join().unwrap();
        }

        // 验证结果
        assert!(!map.is_empty());

        println!("并发哈希表测试通过");
    }
}
