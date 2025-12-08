//! 性能和内存优化组件
//!
//! 本模块实现了性能和内存优化策略，包括延迟初始化、对象池管理、多级缓存和资源回收。

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use super::di_service_descriptor::{DIError, ServiceProvider};

/// 延迟服务
pub struct LazyService<T> {
    /// 工厂函数
    factory: Arc<dyn Fn() -> T + Send + Sync>,
    
    /// 实例
    instance: Arc<Mutex<Option<T>>>,
    
    /// 是否已初始化
    initialized: Arc<std::sync::atomic::AtomicBool>,
}

impl<T> LazyService<T>
where
    T: Send + Sync + Clone + 'static,
{
    /// 创建新的延迟服务
    pub fn new<F>(factory: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            factory: Arc::new(factory),
            instance: Arc::new(Mutex::new(None)),
            initialized: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
    
    /// 获取实例
    pub fn get(&self) -> T {
        if self.initialized.load(std::sync::atomic::Ordering::Acquire) {
            let instance = self.instance.lock().unwrap();
            return instance.as_ref().unwrap().clone();
        }
        
        let mut instance = self.instance.lock().unwrap();
        if instance.is_none() {
            *instance = Some((self.factory)());
            self.initialized.store(true, std::sync::atomic::Ordering::Release);
        }
        
        instance.as_ref().unwrap().clone()
    }
    
    /// 检查是否已初始化
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(std::sync::atomic::Ordering::Acquire)
    }
    
    /// 强制初始化
    pub fn force_init(&self) {
        let _ = self.get();
    }
}

/// 服务预热器
pub struct ServiceWarmer {
    /// 关键服务列表
    critical_services: Vec<TypeId>,
    
    /// 预热超时时间
    warmup_timeout: Duration,
    
    /// 并行预热
    parallel_warmup: bool,
}

impl ServiceWarmer {
    /// 创建新的服务预热器
    pub fn new() -> Self {
        Self {
            critical_services: Vec::new(),
            warmup_timeout: Duration::from_secs(30),
            parallel_warmup: true,
        }
    }
    
    /// 添加关键服务
    pub fn add_critical_service<T: 'static + Send + Sync>(mut self) -> Self {
        self.critical_services.push(TypeId::of::<T>());
        self
    }
    
    /// 设置预热超时时间
    pub fn with_warmup_timeout(mut self, timeout: Duration) -> Self {
        self.warmup_timeout = timeout;
        self
    }
    
    /// 设置并行预热
    pub fn with_parallel_warmup(mut self, parallel: bool) -> Self {
        self.parallel_warmup = parallel;
        self
    }
    
    /// 预热服务
    pub fn warm_up(&self, provider: &dyn ServiceProvider) -> Result<WarmupResult, DIError> {
        let start_time = Instant::now();
        let mut successful_services = Vec::new();
        let mut failed_services = Vec::new();
        
        if self.parallel_warmup {
            // 并行预热
            let mut handles = Vec::new();
            
            for &service_type in &self.critical_services {
                let provider: &dyn ServiceProvider = unsafe { std::mem::transmute(provider) };
                let handle = std::thread::spawn(move || {
                    let start = Instant::now();
                    match provider.get_service_by_id(service_type) {
                        Ok(Some(_)) => {
                            let duration = start.elapsed();
                            Ok((service_type, duration))
                        }
                        Ok(None) => Err((service_type, "Service not found".to_string())),
                        Err(e) => Err((service_type, e.to_string())),
                    }
                });
                handles.push(handle);
            }
            
            // 等待所有预热任务完成
            for handle in handles {
                match handle.join() {
                    Ok(Ok((service_type, duration))) => {
                        successful_services.push(WarmupService {
                            service_type,
                            duration,
                            success: true,
                            error: None,
                        });
                    }
                    Ok(Err((service_type, error))) => {
                        failed_services.push(WarmupService {
                            service_type,
                            duration: Duration::from_secs(0),
                            success: false,
                            error: Some(error),
                        });
                    }
                    Err(_) => {
                        // 线程panic
                        failed_services.push(WarmupService {
                            service_type: TypeId::of::<()>(),
                            duration: Duration::from_secs(0),
                            success: false,
                            error: Some("Thread panic".to_string()),
                        });
                    }
                }
            }
        } else {
            // 串行预热
            for &service_type in &self.critical_services {
                let start = Instant::now();
                match provider.get_service_by_id(service_type) {
                    Ok(Some(_)) => {
                        let duration = start.elapsed();
                        successful_services.push(WarmupService {
                            service_type,
                            duration,
                            success: true,
                            error: None,
                        });
                    }
                    Ok(None) => {
                        failed_services.push(WarmupService {
                            service_type,
                            duration: Duration::from_secs(0),
                            success: false,
                            error: Some("Service not found".to_string()),
                        });
                    }
                    Err(e) => {
                        failed_services.push(WarmupService {
                            service_type,
                            duration: Duration::from_secs(0),
                            success: false,
                            error: Some(e.to_string()),
                        });
                    }
                }
            }
        }
        
        let total_duration = start_time.elapsed();
        
        Ok(WarmupResult {
            total_services: self.critical_services.len(),
            successful_services,
            failed_services,
            total_duration,
        })
    }
}

/// 预热结果
#[derive(Debug)]
pub struct WarmupResult {
    /// 总服务数
    pub total_services: usize,
    
    /// 成功预热的服务
    pub successful_services: Vec<WarmupService>,
    
    /// 失败预热的服务
    pub failed_services: Vec<WarmupService>,
    
    /// 总预热时间
    pub total_duration: Duration,
}

/// 预热服务信息
#[derive(Debug)]
pub struct WarmupService {
    /// 服务类型
    pub service_type: TypeId,
    
    /// 预热时间
    pub duration: Duration,
    
    /// 是否成功
    pub success: bool,
    
    /// 错误信息
    pub error: Option<String>,
}

/// 对象池
#[derive(Clone)]
pub struct ObjectPool<T> {
    /// 对象池
    objects: Arc<Mutex<Vec<T>>>,
    
    /// 工厂函数
    factory: Arc<dyn Fn() -> T + Send + Sync + 'static>,
    
    /// 重置函数
    reset_fn: Option<Arc<dyn Fn(&mut T) + Send + Sync + 'static>>,
    
    /// 最大池大小
    max_size: usize,
    
    /// 当前池大小
    current_size: Arc<std::sync::atomic::AtomicUsize>,
    
    /// 统计信息
    stats: Arc<Mutex<PoolStats>>,
}

/// 池统计信息
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    /// 总获取次数
    pub total_acquires: u64,
    
    /// 总释放次数
    pub total_releases: u64,
    
    /// 池命中次数
    pub pool_hits: u64,
    
    /// 池未命中次数
    pub pool_misses: u64,
    
    /// 当前池大小
    pub current_size: usize,
}

impl<T> ObjectPool<T>
where
    T: Send + Sync + Clone + 'static,
{
    /// 创建新的对象池
    pub fn new<F>(factory: F, max_size: usize) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            objects: Arc::new(Mutex::new(Vec::new())),
            factory: Arc::new(factory),
            reset_fn: None,
            max_size,
            current_size: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            stats: Arc::new(Mutex::new(PoolStats::default())),
        }
    }
    
    /// 设置重置函数
    pub fn with_reset_fn<F>(mut self, reset_fn: F) -> Self
    where
        F: Fn(&mut T) + Send + Sync + 'static,
    {
        self.reset_fn = Some(Arc::new(reset_fn));
        self
    }
    
    /// 获取对象
    pub fn acquire(&self) -> PooledObject<T> {
        let mut stats = self.stats.lock().unwrap();
        stats.total_acquires += 1;
        
        let mut objects = self.objects.lock().unwrap();
        if let Some(mut object) = objects.pop() {
            stats.pool_hits += 1;
            
            // 重置对象
            if let Some(ref reset_fn) = self.reset_fn {
                reset_fn(&mut object);
            }
            
            self.current_size.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            stats.current_size = self.current_size.load(std::sync::atomic::Ordering::SeqCst);
            
            PooledObject {
                object: Some(object),
                pool: self.clone(),
            }
        } else {
            stats.pool_misses += 1;
            
            PooledObject {
                object: Some((self.factory)()),
                pool: self.clone(),
            }
        }
    }
    
    /// 释放对象
    fn release_object(&self, object: T) {
        let mut stats = self.stats.lock().unwrap();
        stats.total_releases += 1;
        
        let current_size = self.current_size.load(std::sync::atomic::Ordering::SeqCst);
        if current_size < self.max_size {
            let mut objects = self.objects.lock().unwrap();
            objects.push(object);
            self.current_size.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            stats.current_size = self.current_size.load(std::sync::atomic::Ordering::SeqCst);
        }
    }
    
    /// 获取统计信息
    pub fn stats(&self) -> PoolStats {
        let stats = self.stats.lock().unwrap();
        PoolStats {
            current_size: self.current_size.load(std::sync::atomic::Ordering::SeqCst),
            ..stats.clone()
        }
    }
    
    /// 清空对象池
    pub fn clear(&self) {
        let mut objects = self.objects.lock().unwrap();
        objects.clear();
        self.current_size.store(0, std::sync::atomic::Ordering::SeqCst);
        
        let mut stats = self.stats.lock().unwrap();
        stats.current_size = 0;
    }
}

/// 池化对象
pub struct PooledObject<T>
where
    T: Send + Sync + Clone + 'static,
{
    /// 对象
    object: Option<T>,
    
    /// 对象池引用
    pool: ObjectPool<T>,
}

impl<T> PooledObject<T>
where
    T: Send + Sync + Clone + 'static,
{
    /// 获取对象引用
    pub fn get(&self) -> &T {
        self.object.as_ref().unwrap()
    }
    
    /// 获取可变对象引用
    pub fn get_mut(&mut self) -> &mut T {
        self.object.as_mut().unwrap()
    }
}

impl<T> Drop for PooledObject<T>
where
    T: Send + Sync + Clone + 'static,
{
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            self.pool.release_object(object);
        }
    }
}

impl<T> std::ops::Deref for PooledObject<T>
where
    T: Send + Sync + Clone + 'static,
{
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> std::ops::DerefMut for PooledObject<T>
where
    T: Send + Sync + Clone + 'static,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

/// 多级缓存
pub struct MultiLevelCache<K, V> {
    /// L1缓存（内存）
    l1_cache: Arc<RwLock<HashMap<K, V>>>,
    
    /// L2缓存（后端）
    l2_cache: Option<Arc<dyn CacheBackend<K, V>>>,
    
    /// L3缓存（分布式）
    l3_cache: Option<Arc<dyn CacheBackend<K, V>>>,
    
    /// L1缓存大小限制
    l1_max_size: usize,
    
    /// 缓存策略
    eviction_policy: Arc<dyn EvictionPolicy<K>>,
    
    /// 统计信息
    stats: Arc<Mutex<CacheStats>>,
}

/// 缓存后端trait
pub trait CacheBackend<K, V>: Send + Sync {
    /// 获取值
    fn get(&self, key: &K) -> Result<Option<V>, CacheError>;
    
    /// 设置值
    fn set(&self, key: K, value: V) -> Result<(), CacheError>;
    
    /// 删除值
    fn remove(&self, key: &K) -> Result<(), CacheError>;
    
    /// 清空缓存
    fn clear(&self) -> Result<(), CacheError>;
}

/// 缓存错误
#[derive(Debug)]
pub enum CacheError {
    /// 后端错误
    BackendError(String),
    /// 序列化错误
    SerializationError(String),
    /// 网络错误
    NetworkError(String),
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::BackendError(msg) => write!(f, "Backend error: {}", msg),
            CacheError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            CacheError::NetworkError(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

impl std::error::Error for CacheError {}

/// 淘汰策略trait
pub trait EvictionPolicy<K>: Send + Sync {
    /// 记录访问
    fn on_access(&self, key: &K);
    
    /// 记录插入
    fn on_insert(&self, key: &K);
    
    /// 选择要淘汰的键
    fn select_victim<'a>(&self, keys: &'a [K]) -> Option<&'a K>;
}

/// 缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// L1命中次数
    pub l1_hits: u64,
    
    /// L1未命中次数
    pub l1_misses: u64,
    
    /// L2命中次数
    pub l2_hits: u64,
    
    /// L2未命中次数
    pub l2_misses: u64,
    
    /// L3命中次数
    pub l3_hits: u64,
    
    /// L3未命中次数
    pub l3_misses: u64,
    
    /// 总命中次数
    pub total_hits: u64,
    
    /// 总未命中次数
    pub total_misses: u64,
}

impl<K, V> MultiLevelCache<K, V>
where
    K: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// 创建新的多级缓存
    pub fn new(l1_max_size: usize) -> Self {
        Self {
            l1_cache: Arc::new(RwLock::new(HashMap::new())),
            l2_cache: None,
            l3_cache: None,
            l1_max_size,
            eviction_policy: Arc::new(LFUEvictionPolicy::new()),
            stats: Arc::new(Mutex::new(CacheStats::default())),
        }
    }
    
    /// 设置L2缓存
    pub fn with_l2_cache(mut self, cache: Arc<dyn CacheBackend<K, V>>) -> Self {
        self.l2_cache = Some(cache);
        self
    }
    
    /// 设置L3缓存
    pub fn with_l3_cache(mut self, cache: Arc<dyn CacheBackend<K, V>>) -> Self {
        self.l3_cache = Some(cache);
        self
    }
    
    /// 设置淘汰策略
    pub fn with_eviction_policy(mut self, policy: Arc<dyn EvictionPolicy<K>>) -> Self {
        self.eviction_policy = policy;
        self
    }
    
    /// 获取值
    pub fn get(&self, key: &K) -> Result<Option<V>, CacheError> {
        let mut stats = self.stats.lock().unwrap();
        
        // 尝试L1缓存
        {
            let l1_cache = self.l1_cache.read().unwrap();
            if let Some(value) = l1_cache.get(key) {
                stats.l1_hits += 1;
                stats.total_hits += 1;
                self.eviction_policy.on_access(key);
                return Ok(Some(value.clone()));
            }
        }
        
        stats.l1_misses += 1;
        
        // 尝试L2缓存
        if let Some(ref l2_cache) = self.l2_cache {
            match l2_cache.get(key) {
                Ok(Some(value)) => {
                    stats.l2_hits += 1;
                    stats.total_hits += 1;
                    
                    // 回填L1缓存
                    self.put_l1(key.clone(), value.clone());
                    
                    return Ok(Some(value));
                }
                Ok(None) => {
                    stats.l2_misses += 1;
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        
        // 尝试L3缓存
        if let Some(ref l3_cache) = self.l3_cache {
            match l3_cache.get(key) {
                Ok(Some(value)) => {
                    stats.l3_hits += 1;
                    stats.total_hits += 1;
                    
                    // 回填L2和L1缓存
                    if let Some(ref l2_cache) = self.l2_cache {
                        let _ = l2_cache.set(key.clone(), value.clone());
                    }
                    self.put_l1(key.clone(), value.clone());
                    
                    return Ok(Some(value));
                }
                Ok(None) => {
                    stats.l3_misses += 1;
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        
        stats.total_misses += 1;
        Ok(None)
    }
    
    /// 设置值
    pub fn set(&self, key: K, value: V) -> Result<(), CacheError> {
        // 设置L1缓存
        self.put_l1(key.clone(), value.clone());
        
        // 设置L2缓存
        if let Some(ref l2_cache) = self.l2_cache {
            l2_cache.set(key.clone(), value.clone())?;
        }
        
        // 设置L3缓存
        if let Some(ref l3_cache) = self.l3_cache {
            l3_cache.set(key, value)?;
        }
        
        Ok(())
    }
    
    /// 删除值
    pub fn remove(&self, key: &K) -> Result<(), CacheError> {
        // 从L1缓存删除
        {
            let mut l1_cache = self.l1_cache.write().unwrap();
            l1_cache.remove(key);
        }
        
        // 从L2缓存删除
        if let Some(ref l2_cache) = self.l2_cache {
            l2_cache.remove(key)?;
        }
        
        // 从L3缓存删除
        if let Some(ref l3_cache) = self.l3_cache {
            l3_cache.remove(key)?;
        }
        
        Ok(())
    }
    
    /// 清空缓存
    pub fn clear(&self) -> Result<(), CacheError> {
        // 清空L1缓存
        {
            let mut l1_cache = self.l1_cache.write().unwrap();
            l1_cache.clear();
        }
        
        // 清空L2缓存
        if let Some(ref l2_cache) = self.l2_cache {
            l2_cache.clear()?;
        }
        
        // 清空L3缓存
        if let Some(ref l3_cache) = self.l3_cache {
            l3_cache.clear()?;
        }
        
        // 重置统计信息
        {
            let mut stats = self.stats.lock().unwrap();
            *stats = CacheStats::default();
        }
        
        Ok(())
    }
    
    /// 获取统计信息
    pub fn stats(&self) -> CacheStats {
        let stats = self.stats.lock().unwrap();
        stats.clone()
    }
    
    /// 放入L1缓存
    fn put_l1(&self, key: K, value: V) {
        let mut l1_cache = self.l1_cache.write().unwrap();
        
        // 检查是否需要淘汰
        if l1_cache.len() >= self.l1_max_size {
            let keys: Vec<K> = l1_cache.keys().cloned().collect();
            if let Some(victim_key) = self.eviction_policy.select_victim(&keys) {
                l1_cache.remove(victim_key);
            }
        }
        
        // 在插入之前先克隆key，以便在on_insert中使用
        self.eviction_policy.on_insert(&key);
        l1_cache.insert(key, value);
    }
}

/// LFU淘汰策略
pub struct LFUEvictionPolicy<K> {
    /// 访问频率
    frequencies: Arc<RwLock<HashMap<K, u64>>>,
}

impl<K> LFUEvictionPolicy<K> {
    /// 创建新的LFU淘汰策略
    pub fn new() -> Self {
        Self {
            frequencies: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<K: Clone + Eq + std::hash::Hash + Send + Sync> EvictionPolicy<K> for LFUEvictionPolicy<K> {
    fn on_access(&self, key: &K) {
        let mut frequencies = self.frequencies.write().unwrap();
        *frequencies.entry(key.clone()).or_insert(0) += 1;
    }
    
    fn on_insert(&self, key: &K) {
        let mut frequencies = self.frequencies.write().unwrap();
        frequencies.insert(key.clone(), 1);
    }
    
    fn select_victim<'a>(&self, keys: &'a [K]) -> Option<&'a K> {
        let frequencies = self.frequencies.read().unwrap();
        let mut min_freq = u64::MAX;
        let mut victim_key = None;
        
        for key in keys {
            if let Some(&freq) = frequencies.get(key) {
                if freq < min_freq {
                    min_freq = freq;
                    victim_key = Some(key);
                }
            } else {
                // 如果没有记录，频率为0
                if 0 < min_freq {
                    min_freq = 0;
                    victim_key = Some(key);
                }
            }
        }
        
        victim_key
    }
}

/// 资源清理器
pub struct ResourceCleaner {
    /// 清理任务
    cleanup_tasks: Arc<Mutex<Vec<Box<dyn CleanupTask>>>>,
    
    /// 清理间隔
    cleanup_interval: Duration,
    
    /// 是否正在运行
    running: Arc<std::sync::atomic::AtomicBool>,
    
    /// 清理线程句柄
    cleanup_thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

/// 清理任务trait
pub trait CleanupTask: Send + Sync {
    /// 执行清理
    fn cleanup(&self) -> Result<(), CleanupError>;
    
    /// 获取任务名称
    fn name(&self) -> &str;
}

/// 清理错误
#[derive(Debug)]
pub enum CleanupError {
    /// 清理失败
    CleanupFailed(String),
    /// 资源忙
    ResourceBusy(String),
}

impl std::fmt::Display for CleanupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CleanupError::CleanupFailed(msg) => write!(f, "Cleanup failed: {}", msg),
            CleanupError::ResourceBusy(msg) => write!(f, "Resource busy: {}", msg),
        }
    }
}

impl std::error::Error for CleanupError {}

impl ResourceCleaner {
    /// 创建新的资源清理器
    pub fn new(cleanup_interval: Duration) -> Self {
        Self {
            cleanup_tasks: Arc::new(Mutex::new(Vec::new())),
            cleanup_interval,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            cleanup_thread: Arc::new(Mutex::new(None)),
        }
    }
    
    /// 添加清理任务
    pub fn add_cleanup_task(&self, task: Box<dyn CleanupTask>) {
        let mut tasks = self.cleanup_tasks.lock().unwrap();
        tasks.push(task);
    }
    
    /// 启动清理任务
    pub fn start(&self) {
        if self.running.load(std::sync::atomic::Ordering::Acquire) {
            return;
        }
        
        self.running.store(true, std::sync::atomic::Ordering::Release);
        
        let tasks = Arc::clone(&self.cleanup_tasks);
        let interval = self.cleanup_interval;
        let running = Arc::clone(&self.running);
        
        let handle = std::thread::spawn(move || {
            while running.load(std::sync::atomic::Ordering::Acquire) {
                std::thread::sleep(interval);
                
                let tasks = tasks.lock().unwrap();
                for task in tasks.iter() {
                    if let Err(e) = task.cleanup() {
                        eprintln!("Cleanup task '{}' failed: {}", task.name(), e);
                    }
                }
            }
        });
        
        let mut thread_handle = self.cleanup_thread.lock().unwrap();
        *thread_handle = Some(handle);
    }
    
    /// 停止清理任务
    pub fn stop(&self) {
        self.running.store(false, std::sync::atomic::Ordering::Release);
        
        let mut thread_handle = self.cleanup_thread.lock().unwrap();
        if let Some(handle) = thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ResourceCleaner {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_lazy_service() {
        let lazy = LazyService::new(|| 42);
        
        assert!(!lazy.is_initialized());
        assert_eq!(lazy.get(), 42);
        assert!(lazy.is_initialized());
    }
    
    #[test]
    fn test_service_warmer() {
        let warmer = ServiceWarmer::new()
            .add_critical_service::<String>()
            .with_warmup_timeout(Duration::from_secs(1))
            .with_parallel_warmup(false);
        
        // 注意：这里需要实际的ServiceProvider来测试
        // 由于没有实现，这里只是测试结构创建
        assert_eq!(warmer.critical_services.len(), 1);
    }
    
    #[test]
    fn test_object_pool() {
        let pool = ObjectPool::new(|| 42, 10);
        
        let obj1 = pool.acquire();
        assert_eq!(*obj1, 42);
        
        let obj2 = pool.acquire();
        assert_eq!(*obj2, 42);
        
        let stats = pool.stats();
        assert_eq!(stats.total_acquires, 2);
        assert_eq!(stats.pool_misses, 2);
    }
    
    #[test]
    fn test_multi_level_cache() {
        let cache = MultiLevelCache::<String, i32>::new(10);
        
        cache.set("key1".to_string(), 42).unwrap();
        let value = cache.get(&"key1".to_string()).unwrap();
        assert_eq!(value, Some(42));
        
        let stats = cache.stats();
        assert_eq!(stats.l1_hits, 1);
        assert_eq!(stats.total_hits, 1);
    }
    
    #[test]
    fn test_lfu_eviction_policy() {
        let policy = LFUEvictionPolicy::new();
        
        policy.on_insert(&"key1");
        policy.on_access(&"key1");
        policy.on_insert(&"key2");
        
        let keys = vec!["key1", "key2"];
        let victim = policy.select_victim(&keys);
        assert_eq!(victim, Some(&"key2"));
    }
    
    #[test]
    fn test_resource_cleaner() {
        let cleaner = ResourceCleaner::new(Duration::from_secs(1));
        
        struct TestCleanupTask;
        
        impl CleanupTask for TestCleanupTask {
            fn cleanup(&self) -> Result<(), CleanupError> {
                Ok(())
            }
            
            fn name(&self) -> &str {
                "test_task"
            }
        }
        
        cleaner.add_cleanup_task(Box::new(TestCleanupTask));
        cleaner.start();
        cleaner.stop();
    }
}